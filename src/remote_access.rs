//! Opt-in HTTPS listener and finite, application-owned UPnP router leases.
use crate::{auth, db, App};
use axum::{
    body::Body,
    extract::{Request, State},
    http::{header, HeaderMap, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    Json, Router,
};
use axum_server::{tls_rustls::RustlsConfig, Handle};
use igd_next::{
    aio::{
        tokio::{search_gateway, Tokio},
        Gateway,
    },
    PortMappingProtocol, SearchOptions,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::PathBuf,
    sync::{Arc, OnceLock},
    time::{Duration, Instant},
};
use tokio::{sync::Mutex, task::JoinHandle};

const LEASE: u32 = 600;
const KEY: &str = "remote_access";
type Shared = Arc<Mutex<Manager>>;
static MANAGER: OnceLock<Shared> = OnceLock::new();

#[derive(Clone)]
struct Config {
    cert: PathBuf,
    key: PathBuf,
    hostname: String,
    local: SocketAddr,
}
impl Config {
    fn from_env() -> Result<Self, String> {
        let get = |key| {
            std::env::var(key)
                .ok()
                .filter(|s| !s.trim().is_empty())
                .ok_or_else(|| format!("Configure {key} on the server, then restart."))
        };
        let hostname = get("SMART_MEDIA_REMOTE_HOSTNAME")?.to_ascii_lowercase();
        if !valid_hostname(&hostname) {
            return Err(
                "Remote hostname must be a DNS name covered by your trusted TLS certificate."
                    .into(),
            );
        }
        let ip: Ipv4Addr = get("SMART_MEDIA_REMOTE_IP")?
            .parse()
            .map_err(|_| "Remote IP must be this server's LAN IPv4 address.")?;
        if !ip.is_private() && !ip.is_loopback() {
            return Err("Remote IP must be a private LAN IPv4 address.".into());
        }
        let port = std::env::var("SMART_MEDIA_REMOTE_PORT")
            .unwrap_or_else(|_| "8943".into())
            .parse::<u16>()
            .ok()
            .filter(|p| *p > 0)
            .ok_or("Invalid remote HTTPS port.")?;
        Ok(Self {
            cert: get("SMART_MEDIA_TLS_CERT")?.into(),
            key: get("SMART_MEDIA_TLS_KEY")?.into(),
            hostname,
            local: (ip, port).into(),
        })
    }
}
fn valid_hostname(s: &str) -> bool {
    s.len() <= 253
        && s.contains('.')
        && s.parse::<IpAddr>().is_err()
        && s.split('.').all(|p| {
            !p.is_empty()
                && p.len() <= 63
                && !p.starts_with('-')
                && !p.ends_with('-')
                && p.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-')
        })
}
fn public_ipv4(ip: IpAddr) -> bool {
    let IpAddr::V4(ip) = ip else {
        return false;
    };
    let [a, b, c, _] = ip.octets();
    !(ip.is_private()
        || ip.is_loopback()
        || ip.is_link_local()
        || ip.is_unspecified()
        || ip.is_multicast()
        || a == 0
        || a >= 240
        || (a == 100 && (64..=127).contains(&b))
        || (a == 192 && b == 0 && (c == 0 || c == 2))
        || (a == 198 && (b == 18 || b == 19 || (b == 51 && c == 100)))
        || (a == 203 && b == 0 && c == 113))
}
#[derive(Clone, Copy, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Options {
    enabled: bool,
    #[serde(default)]
    automatic: bool,
}
struct Mapping {
    gateway: Gateway<Tokio>,
    port: u16,
    target: SocketAddr,
    label: String,
    renewed: Instant,
    public_ip: IpAddr,
}
struct Active {
    handle: Handle<SocketAddr>,
    task: JoinHandle<std::io::Result<()>>,
    tls: RustlsConfig,
    mapping: Option<Mapping>,
}
struct Manager {
    app: App,
    routes: Router,
    config: Result<Config, String>,
    desired: Options,
    active: Option<Active>,
    message: String,
}

// Require exact browser origin for mutations. Non-browser JSON clients may omit Origin.
pub(crate) fn same_origin(headers: &HeaderMap) -> bool {
    if headers.get("sec-fetch-site").and_then(|s| s.to_str().ok()) == Some("cross-site") {
        return false;
    }
    let Some(origin) = headers.get(header::ORIGIN) else {
        return true;
    };
    let Some(host) = headers.get(header::HOST).and_then(|s| s.to_str().ok()) else {
        return false;
    };
    let Ok(origin) = origin.to_str() else {
        return false;
    };
    let Ok(uri) = origin.parse::<axum::http::Uri>() else {
        return false;
    };
    matches!(uri.scheme_str(), Some("http" | "https"))
        && uri
            .authority()
            .is_some_and(|a| a.as_str().eq_ignore_ascii_case(host))
}
async fn secure_guard(
    State((app, hostname)): State<(App, String)>,
    req: Request<Body>,
    next: Next,
) -> Response {
    let host = req
        .headers()
        .get(header::HOST)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<axum::http::uri::Authority>().ok());
    if !host.is_some_and(|h| h.host().eq_ignore_ascii_case(&hostname)) {
        return (
            StatusCode::MISDIRECTED_REQUEST,
            "Use the configured remote hostname.",
        )
            .into_response();
    }
    if req.uri().path() == "/api/auth/setup" || !auth::has_accounts(&app.db) {
        return (
            StatusCode::FORBIDDEN,
            "Complete account setup locally before enabling remote access.",
        )
            .into_response();
    }
    let mut response = next.run(req).await;
    // This listener is real TLS; never trust X-Forwarded-Proto supplied by a client.
    let cookies: Vec<_> = response
        .headers()
        .get_all(header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .map(|s| {
            if s.split(';')
                .any(|v| v.trim().eq_ignore_ascii_case("secure"))
            {
                s.to_owned()
            } else {
                format!("{s}; Secure")
            }
        })
        .collect();
    response.headers_mut().remove(header::SET_COOKIE);
    for cookie in cookies {
        if let Ok(value) = cookie.parse() {
            response.headers_mut().append(header::SET_COOKIE, value);
        }
    }
    response.headers_mut().insert(
        header::STRICT_TRANSPORT_SECURITY,
        "max-age=86400".parse().unwrap(),
    );
    response
}
async fn bounded<T, E: std::fmt::Display>(
    f: impl std::future::Future<Output = Result<T, E>>,
) -> Result<T, String> {
    tokio::time::timeout(Duration::from_secs(8), f)
        .await
        .map_err(|_| {
            "Router request timed out; check the router for any unconfirmed mapping. HTTPS is stopped if setup fails."
                .to_string()
        })?
        .map_err(|e| e.to_string())
}
impl Mapping {
    async fn create(config: &Config) -> Result<Self, String> {
        if !matches!(config.local.ip(),IpAddr::V4(ip) if ip.is_private()) {
            return Err("Automatic mapping requires a private LAN address, not loopback.".into());
        }
        let gateway = search_gateway(SearchOptions { bind_addr: (config.local.ip(),0).into(), timeout: Some(Duration::from_secs(5)), ..Default::default() }).await.map_err(|_| "No UPnP router found. Enable UPnP on a trusted home router or choose manual forwarding. NAT-PMP-only routers are not supported.".to_string())?;
        Self::with_gateway(config, gateway).await
    }
    async fn with_gateway(config: &Config, gateway: Gateway<Tokio>) -> Result<Self, String> {
        let public_ip = bounded(gateway.get_external_ip()).await?;
        if !public_ipv4(public_ip) {
            return Err("Router has no public IPv4 address. CGNAT or double NAT may require a VPN or an upstream router change.".into());
        }
        let mut bytes = [0u8; 8];
        ring::rand::SecureRandom::fill(&ring::rand::SystemRandom::new(), &mut bytes)
            .map_err(|_| "Cannot create mapping identifier.")?;
        let label = format!("Platno-{:016x}", u64::from_be_bytes(bytes));
        // A finite lease is mandatory: never fall back to an infinite lease.
        let port =
            bounded(gateway.add_any_port(PortMappingProtocol::TCP, config.local, LEASE, &label))
                .await?;
        let mapping = Self {
            gateway,
            port,
            target: config.local,
            label,
            renewed: Instant::now(),
            public_ip,
        };
        if !mapping.finite().await.unwrap_or(false) {
            let _ = mapping.remove().await;
            return Err("Router did not confirm a finite application-owned mapping. HTTPS stopped; check the router for leftover rules.".into());
        }
        Ok(mapping)
    }
    async fn owned(&self) -> Result<bool, String> {
        Ok(self.entry().await?.is_some())
    }
    async fn entry(&self) -> Result<Option<igd_next::PortMappingEntry>, String> {
        bounded(async {
            for index in 0..256 {
                let entry = match self.gateway.get_generic_port_mapping_entry(index).await {
                    Ok(entry) => entry,
                    Err(igd_next::GetGenericPortMappingEntryError::SpecifiedArrayIndexInvalid) => {
                        return Ok(None)
                    }
                    Err(e) => return Err(e.to_string()),
                };
                if entry.external_port == self.port && entry.protocol == PortMappingProtocol::TCP {
                    return Ok((entry.internal_client.parse::<IpAddr>().ok()
                        == Some(self.target.ip())
                        && entry.internal_port == self.target.port()
                        && entry.port_mapping_description == self.label
                        && entry.remote_host.is_empty())
                    .then_some(entry));
                }
            }
            Err("Router mapping table exceeds the inspection limit.".to_string())
        })
        .await
    }
    async fn finite(&self) -> Result<bool, String> {
        Ok(self
            .entry()
            .await?
            .is_some_and(|e| e.enabled && e.lease_duration > 0 && e.lease_duration <= LEASE))
    }
    async fn renew(&mut self) -> Result<(), String> {
        if self.renewed.elapsed() >= Duration::from_secs(LEASE as u64) || !self.finite().await? {
            return Err(
                "Router lease was lost; re-enable remote access to create a new mapping.".into(),
            );
        }
        bounded(self.gateway.add_port(
            PortMappingProtocol::TCP,
            self.port,
            self.target,
            LEASE,
            &self.label,
        ))
        .await?;
        if !self.finite().await? {
            return Err("Router did not confirm a finite renewed lease.".into());
        }
        self.renewed = Instant::now();
        self.public_ip = bounded(self.gateway.get_external_ip()).await?;
        if !public_ipv4(self.public_ip) {
            return Err("Router no longer has a public IPv4 address.".into());
        }
        Ok(())
    }
    async fn remove(&self) -> Result<(), String> {
        if self.renewed.elapsed() < Duration::from_secs(LEASE as u64) && self.owned().await? {
            bounded(
                self.gateway
                    .remove_port(PortMappingProtocol::TCP, self.port),
            )
            .await?;
        }
        Ok(())
    }
}
impl Manager {
    fn save(&self, desired: Options) -> Result<(), String> {
        db(&self.app).execute("INSERT INTO settings(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value",[KEY, &serde_json::to_string(&desired).unwrap()]).map_err(|_| "Could not save remote access settings.".to_string())?;
        Ok(())
    }
    async fn stop(&mut self) {
        if let Some(mut active) = self.active.take() {
            // Stop accepting connections immediately; allow the disabling response to finish.
            active
                .handle
                .graceful_shutdown(Some(Duration::from_secs(2)));
            // Serve returns after its listener closes, before existing requests drain.
            // Never wait for the request that is currently disabling this listener.
            let _ = tokio::time::timeout(Duration::from_millis(100), &mut active.task).await;
            if let Some(mapping) = active.mapping {
                if let Err(e) = mapping.remove().await {
                    self.message = format!("HTTPS stopped. Router cleanup failed: {e} The lease expires within ten minutes.");
                }
            }
        }
    }
    async fn start(&mut self, desired: Options) -> Result<(), String> {
        let config = self.config.clone()?;
        if !auth::has_accounts(&self.app.db) {
            return Err("Create the owner account locally first.".into());
        }
        let tls = RustlsConfig::from_pem_file(&config.cert,&config.key).await.map_err(|_| "Could not load TLS certificate and key. Configure a trusted certificate for the remote hostname.".to_string())?;
        let socket = tokio::net::TcpSocket::new_v4().map_err(|e| e.to_string())?;
        socket.set_reuseaddr(true).map_err(|e| e.to_string())?;
        socket
            .bind(config.local)
            .map_err(|e| format!("Cannot bind remote HTTPS address: {e}"))?;
        let listener = socket
            .listen(128)
            .map_err(|e| e.to_string())?
            .into_std()
            .map_err(|e| e.to_string())?;
        let handle = Handle::new();
        let server = axum_server::from_tcp_rustls(listener, tls.clone())
            .map_err(|e| e.to_string())?
            .handle(handle.clone());
        let routes = self.routes.clone().layer(middleware::from_fn_with_state(
            (self.app.clone(), config.hostname.clone()),
            secure_guard,
        ));
        let task = tokio::spawn(server.serve(routes.into_make_service()));
        self.active = Some(Active {
            handle,
            task,
            tls,
            mapping: None,
        });
        if desired.automatic {
            match Mapping::create(&config).await {
                Ok(mapping) => self.active.as_mut().unwrap().mapping = Some(mapping),
                Err(e) => {
                    self.stop().await;
                    return Err(e);
                }
            }
        }
        self.message = if desired.automatic { "Router mapping created. Internet reachability has not been independently verified. DNS must point to your public address." } else { "HTTPS listener ready. Forward a TCP port manually to the address below. Internet reachability has not been independently verified." }.into();
        Ok(())
    }
    fn status(&self) -> Value {
        let active = self.active.as_ref().filter(|a| !a.task.is_finished());
        let mapping = active.and_then(|a| a.mapping.as_ref());
        let config = self.config.as_ref().ok();
        let port = mapping
            .map(|m| m.port)
            .or_else(|| config.map(|c| c.local.port()));
        json!({"enabled":active.is_some(),"automatic":self.desired.automatic,"configured":config.is_some(),"configuration_error":self.config.as_ref().err(),"message":self.message,"local_address":config.map(|c|c.local.to_string()),"url":config.zip(port).map(|(c,p)|format!("https://{}:{p}",c.hostname)),"mapped":mapping.is_some(),"public_ip":mapping.map(|m|m.public_ip.to_string()),"lease_seconds":mapping.map(|_|LEASE),"reachable":null})
    }
}
pub async fn status() -> Response {
    let Some(manager) = MANAGER.get() else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({"error":"Remote access is initializing."})),
        )
            .into_response();
    };
    Json(manager.lock().await.status()).into_response()
}
pub async fn configure(Json(options): Json<Options>) -> Response {
    let Some(manager) = MANAGER.get() else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };
    let manager = manager.clone();
    // Router mutations complete even if the browser navigates away mid-request.
    tokio::spawn(async move { configure_inner(manager, options).await }).await.unwrap_or_else(|_| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error":"Remote access operation failed; check status before retrying."}))).into_response())
}
async fn configure_inner(manager: Shared, options: Options) -> Response {
    let mut manager = manager.lock().await;
    // Persist disabled first; failure must never silently leave a requested enable active.
    if let Err(e) = manager.save(Options::default()) {
        manager.stop().await;
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error":e}))).into_response();
    }
    manager.desired = Options::default();
    manager.message = "Remote access disabled.".into();
    manager.stop().await;
    if options.enabled {
        if let Err(e) = manager.start(options).await {
            manager.message = e.clone();
            return (StatusCode::BAD_REQUEST, Json(json!({"error":e}))).into_response();
        }
        if let Err(e) = manager.save(options) {
            manager.stop().await;
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error":e}))).into_response();
        }
        manager.desired = options;
    }
    Json(manager.status()).into_response()
}
pub async fn init(app: App, routes: Router) {
    let desired = db(&app)
        .query_row("SELECT value FROM settings WHERE key=?1", [KEY], |r| {
            r.get::<_, String>(0)
        })
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    let manager = Arc::new(Mutex::new(Manager {
        app,
        routes,
        config: Config::from_env(),
        desired,
        active: None,
        message: "Remote access disabled.".into(),
    }));
    if MANAGER.set(manager.clone()).is_err() {
        return;
    }
    tokio::spawn(async move {
        {
            let mut m = manager.lock().await;
            if m.desired.enabled {
                let desired = m.desired;
                if let Err(e) = m.start(desired).await {
                    m.message = format!("Remote access could not restart: {e}");
                }
            }
        }
        loop {
            tokio::time::sleep(Duration::from_secs(60)).await;
            let mut m = manager.lock().await;
            let config = m.config.clone();
            if let Some(active) = m.active.as_mut() {
                let mut failure = active
                    .task
                    .is_finished()
                    .then(|| "HTTPS listener stopped.".to_string());
                if let Some(mapping) = active.mapping.as_mut() {
                    if mapping.renewed.elapsed() >= Duration::from_secs(240) {
                        if let Err(e) = mapping.renew().await {
                            failure = Some(format!("Router renewal failed: {e}"));
                        }
                    }
                }
                // Renewal tools can atomically replace PEM files; successful reloads apply without a restart.
                if let Ok(config) = config {
                    let _ = active
                        .tls
                        .reload_from_pem_file(config.cert, config.key)
                        .await;
                }
                if let Some(failure) = failure {
                    m.stop().await;
                    m.message = format!("{failure} Remote HTTPS stopped. Re-enable to retry.");
                }
            }
        }
    });
}
pub async fn shutdown() {
    if let Some(manager) = MANAGER.get() {
        manager.lock().await.stop().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_non_public_router_addresses() {
        for ip in [
            "10.0.0.1",
            "172.16.0.1",
            "192.168.1.1",
            "100.64.0.1",
            "100.127.255.254",
            "127.0.0.1",
            "169.254.1.1",
            "0.1.2.3",
            "192.0.2.1",
            "198.51.100.1",
            "203.0.113.1",
            "198.18.0.1",
            "224.0.0.1",
            "255.255.255.255",
            "::1",
        ] {
            assert!(!public_ipv4(ip.parse().unwrap()), "{ip}");
        }
        assert!(public_ipv4("8.8.8.8".parse().unwrap()));
    }
    #[test]
    fn hostname_cannot_inject_urls_or_headers() {
        for host in [
            "localhost",
            "https://media.test",
            "media.test:443",
            "media.test/path",
            "-media.test",
            "media..test",
            "127.0.0.1",
            "media.test\r\nx:a",
        ] {
            assert!(!valid_hostname(host), "{host}");
        }
        assert!(valid_hostname("media.example.test"));
    }
    #[test]
    fn browser_mutations_require_same_origin() {
        let mut h = HeaderMap::new();
        h.insert(header::HOST, "media.test:8943".parse().unwrap());
        assert!(same_origin(&h));
        for origin in [
            "null",
            "https://evil.test",
            "https://media.test:443",
            "https://media.test.evil:8943",
        ] {
            h.insert(header::ORIGIN, origin.parse().unwrap());
            assert!(!same_origin(&h));
        }
        h.insert(header::ORIGIN, "https://media.test:8943".parse().unwrap());
        assert!(same_origin(&h));
        h.insert("sec-fetch-site", "cross-site".parse().unwrap());
        assert!(!same_origin(&h));
    }
    #[derive(Clone)]
    struct FakeRouter {
        description: Arc<std::sync::Mutex<String>>,
        lease: u32,
        calls: Arc<std::sync::Mutex<Vec<String>>>,
    }
    async fn fake_router(
        State(fake): State<FakeRouter>,
        headers: HeaderMap,
        body: String,
    ) -> String {
        let action = headers
            .get("soapaction")
            .unwrap()
            .to_str()
            .unwrap()
            .trim_matches('"')
            .split('#')
            .next_back()
            .unwrap()
            .to_owned();
        fake.calls.lock().unwrap().push(action.clone());
        let fields = match action.as_str() {
            "GetExternalIPAddress" => "<NewExternalIPAddress>8.8.8.8</NewExternalIPAddress>".into(),
            "AddAnyPortMapping" => {
                assert!(body.contains("<NewLeaseDuration>600</NewLeaseDuration>"));
                let label = body.split("<NewPortMappingDescription>").nth(1).unwrap().split('<').next().unwrap();
                *fake.description.lock().unwrap() = label.into();
                "<NewReservedPort>54321</NewReservedPort>".into()
            }
            "GetGenericPortMappingEntry" => format!("<NewRemoteHost></NewRemoteHost><NewExternalPort>54321</NewExternalPort><NewProtocol>TCP</NewProtocol><NewInternalPort>8943</NewInternalPort><NewInternalClient>192.168.1.50</NewInternalClient><NewEnabled>1</NewEnabled><NewPortMappingDescription>{}</NewPortMappingDescription><NewLeaseDuration>{}</NewLeaseDuration>",fake.description.lock().unwrap(),fake.lease),
            "AddPortMapping" => { assert!(body.contains("<NewLeaseDuration>600</NewLeaseDuration>")); String::new() }
            "DeletePortMapping" => String::new(),
            _ => panic!("unexpected action {action}"),
        };
        format!(
            r#"<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/"><s:Body><u:{action}Response xmlns:u="urn:schemas-upnp-org:service:WANIPConnection:1">{fields}</u:{action}Response></s:Body></s:Envelope>"#
        )
    }
    async fn gateway(lease: u32) -> (Gateway<Tokio>, FakeRouter, JoinHandle<()>) {
        let fake = FakeRouter {
            description: Arc::new(std::sync::Mutex::new(String::new())),
            lease,
            calls: Arc::new(std::sync::Mutex::new(Vec::new())),
        };
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let routes = Router::new()
            .route("/control", axum::routing::post(fake_router))
            .with_state(fake.clone());
        let task = tokio::spawn(async move {
            axum::serve(listener, routes).await.unwrap();
        });
        let schema: Vec<String> = [
            "NewRemoteHost",
            "NewExternalPort",
            "NewProtocol",
            "NewInternalPort",
            "NewInternalClient",
            "NewEnabled",
            "NewPortMappingDescription",
            "NewLeaseDuration",
        ]
        .map(str::to_string)
        .into();
        let gateway = Gateway {
            addr,
            root_url: String::new(),
            control_url: "/control".into(),
            control_schema_url: String::new(),
            control_schema: [
                ("AddAnyPortMapping".into(), schema.clone()),
                ("AddPortMapping".into(), schema.clone()),
                ("DeletePortMapping".into(), schema),
            ]
            .into(),
            provider: Tokio,
        };
        (gateway, fake, task)
    }
    fn config() -> Config {
        Config {
            cert: "unused".into(),
            key: "unused".into(),
            hostname: "media.example.test".into(),
            local: "192.168.1.50:8943".parse().unwrap(),
        }
    }
    #[tokio::test]
    async fn leases_create_renew_and_remove_only_the_owned_mapping() {
        let (gateway, fake, task) = gateway(600).await;
        let mut mapping = Mapping::with_gateway(&config(), gateway).await.unwrap();
        mapping.renew().await.unwrap();
        mapping.remove().await.unwrap();
        let calls = fake.calls.lock().unwrap().clone();
        assert!(calls.contains(&"AddAnyPortMapping".into()));
        assert!(calls.contains(&"AddPortMapping".into()));
        assert!(calls.contains(&"DeletePortMapping".into()));
        *fake.description.lock().unwrap() = "Another application".into();
        fake.calls.lock().unwrap().clear();
        assert!(mapping.renew().await.is_err());
        mapping.remove().await.unwrap();
        assert!(!fake
            .calls
            .lock()
            .unwrap()
            .iter()
            .any(|c| c == "AddPortMapping" || c == "DeletePortMapping"));
        task.abort();
    }
    #[tokio::test]
    async fn permanent_leases_are_rejected_and_cleaned_up() {
        let (gateway, fake, task) = gateway(0).await;
        assert!(Mapping::with_gateway(&config(), gateway).await.is_err());
        assert!(fake
            .calls
            .lock()
            .unwrap()
            .contains(&"DeletePortMapping".into()));
        task.abort();
    }
    #[tokio::test]
    async fn expired_lease_does_not_modify_a_potentially_reused_port() {
        let (gateway, fake, task) = gateway(600).await;
        let mut mapping = Mapping::with_gateway(&config(), gateway).await.unwrap();
        mapping.renewed = Instant::now() - Duration::from_secs(601);
        fake.calls.lock().unwrap().clear();
        assert!(mapping.renew().await.is_err());
        mapping.remove().await.unwrap();
        assert!(fake.calls.lock().unwrap().is_empty());
        task.abort();
    }
}
