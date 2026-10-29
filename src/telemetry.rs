//! Optional, bounded beta diagnostics. No raw messages, paths, identities or media.
use crate::{auth, db, App};
use axum::{
    body::Body,
    extract::{Request, State},
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::{json, Value};
use std::{
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, OnceLock,
    },
    time::{Duration, Instant},
};
use tokio::sync::{mpsc, Mutex};
static TELEMETRY: OnceLock<Arc<Telemetry>> = OnceLock::new();
struct Telemetry {
    endpoint: Option<reqwest::Url>,
    beta: bool,
    started: Instant,
    session: String,
    sender: mpsc::Sender<(u64, Value)>,
    allowed: AtomicBool,
    generation: AtomicU64,
    limits: Mutex<std::collections::HashMap<i64, (Instant, u8)>>,
}
const SCENES: &[&str] = &[
    "dashboard",
    "library",
    "collections",
    "watchlist",
    "settings",
    "search",
    "player",
    "unknown",
    "server",
];
fn random_id() -> String {
    let mut bytes = [0u8; 16];
    if ring::rand::SecureRandom::fill(&ring::rand::SystemRandom::new(), &mut bytes).is_err() {
        return String::new();
    }
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
fn endpoint(raw: &str) -> Option<reqwest::Url> {
    let url = reqwest::Url::parse(raw).ok()?;
    // Plain HTTP is allowed only to this machine, so local tests never send data off-host.
    let loopback = matches!(url.host_str(), Some("127.0.0.1" | "localhost" | "[::1]"));
    ((url.scheme() == "https" || (url.scheme() == "http" && loopback))
        && url.host_str().is_some()
        && url.username().is_empty()
        && url.password().is_none()
        && url.query().is_none()
        && url.fragment().is_none()
        && url.path() == "/api/ingest")
        .then_some(url)
}
fn enabled(t: &Telemetry) -> bool {
    t.beta && t.endpoint.is_some() && !t.session.is_empty() && t.allowed.load(Ordering::Acquire)
}

fn uptime(t: &Telemetry) -> u64 {
    t.started.elapsed().as_secs().min(31536000)
}
fn server_context() -> Value {
    json!({"runtime":"rust","os":match std::env::consts::OS{"linux"=>"linux","macos"=>"macos","windows"=>"windows",_=>"other"},"arch":match std::env::consts::ARCH{"x86_64"=>"x86_64","aarch64"=>"aarch64",_=>"other"}})
}
fn server_report(
    t: &Telemetry,
    kind: &str,
    code: &str,
    route: Option<&str>,
    status: Option<u16>,
) -> Value {
    let mut value = json!({"schema":1,"sessionId":t.session,"kind":kind,"code":code,"scene":"server","uptimeSeconds":uptime(t),"serverUptimeSeconds":uptime(t),"release":env!("CARGO_PKG_VERSION"),"context":server_context(),"frames":[],"replay":[]});
    if let Some(route) = route {
        value["route"] = json!(route)
    }
    if let Some(status) = status {
        value["status"] = json!(status)
    }
    value
}
fn enqueue(kind: &str, code: &str, route: Option<&str>, status: Option<u16>) {
    if let Some(t) = TELEMETRY.get() {
        if enabled(t) {
            let _ = t.sender.try_send((
                t.generation.load(Ordering::Acquire),
                server_report(t, kind, code, route, status),
            ));
        }
    }
}
fn route_kind(path: &str) -> &'static str {
    if path.starts_with("/api/playback/") {
        "playback"
    } else if path.starts_with("/api/subtitles/") {
        "subtitles"
    } else if path.starts_with("/api/metadata/") {
        "metadata"
    } else if path.starts_with("/api/auth/") {
        "auth"
    } else if path.contains("libraries") || path.contains("library") {
        "library"
    } else if path.starts_with("/api/scan") || path.starts_with("/api/status") {
        "scan"
    } else if path.starts_with("/api/owner/") || path.starts_with("/api/user/") {
        "settings"
    } else {
        "other"
    }
}
pub async fn observe(req: Request<Body>, next: Next) -> Response {
    let track =
        req.uri().path().starts_with("/api/") && !req.uri().path().starts_with("/api/telemetry/");
    let route = route_kind(req.uri().path());
    let response = next.run(req).await;
    if track && response.status().is_server_error() {
        enqueue(
            "server_error",
            "http5xx",
            Some(route),
            Some(response.status().as_u16()),
        )
    }
    response
}
pub async fn config() -> Json<Value> {
    if let Some(t) = TELEMETRY.get() {
        Json(
            json!({"available":t.beta&&t.endpoint.is_some(),"enabled":enabled(t),"destination":t.endpoint.as_ref().and_then(|u|u.host_str()),"serverUptimeSeconds":uptime(t),"release":env!("CARGO_PKG_VERSION"),"channel":if t.beta{"beta"}else{"stable"}}),
        )
    } else {
        Json(json!({"available":false,"enabled":false,"channel":"stable"}))
    }
}
pub async fn owner_setting(State(app): State<App>, Json(value): Json<Value>) -> Response {
    let Some(active) = value.get("enabled").and_then(Value::as_bool) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    if db(&app).execute("INSERT INTO server_settings(key,value) VALUES('beta_telemetry',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value",[if active{"true"}else{"false"}]).is_err(){return StatusCode::INTERNAL_SERVER_ERROR.into_response()}
    if let Some(t) = TELEMETRY.get() {
        t.allowed.store(active, Ordering::Release);
        t.generation.fetch_add(1, Ordering::AcqRel);
    }
    config().await.into_response()
}
// Fail closed on strings or property names that could contain identifying content.
fn safe_report(v: &Value, depth: usize) -> bool {
    if depth > 8 {
        return false;
    }
    match v {
        Value::Null | Value::Bool(_) => true,
        Value::Number(n) => n
            .as_f64()
            .is_some_and(|n| n.is_finite() && (0.0..=86400000.0).contains(&n)),
        Value::String(s) => {
            SCENES.contains(&s.as_str())
                || [
                    "browser_error",
                    "rejection",
                    "http_error",
                    "playback_error",
                    "server_error",
                    "panic",
                    "heartbeat",
                    "test",
                    "read-null",
                    "read-undefined",
                    "not-callable",
                    "not-defined",
                    "syntax",
                    "network",
                    "resource",
                    "unknown",
                    "http5xx",
                    "media-decode",
                    "media-network",
                    "hls-fatal",
                    "Error",
                    "TypeError",
                    "ReferenceError",
                    "SyntaxError",
                    "RangeError",
                    "DOMException",
                    "Other",
                    "app",
                    "hls",
                    "telemetry",
                    "server",
                    "browser",
                    "rust",
                    "Chrome",
                    "Firefox",
                    "Safari",
                    "Edge",
                    "Windows",
                    "macOS",
                    "Linux",
                    "Android",
                    "iOS",
                    "linux",
                    "macos",
                    "windows",
                    "other",
                    "x86_64",
                    "aarch64",
                    "direct",
                    "progressive",
                    "auto",
                    "1080",
                    "720",
                    "480",
                    "playback",
                    "subtitles",
                    "metadata",
                    "auth",
                    "scan",
                    "snapshot",
                    "click",
                    "navigation",
                    "scroll",
                    "resize",
                    "input",
                    "error",
                    "button",
                    "select",
                    "panel",
                    "media",
                    "link",
                ]
                .contains(&s.as_str())
        }
        Value::Array(a) => a.len() <= 100 && a.iter().all(|v| safe_report(v, depth + 1)),
        Value::Object(o) => {
            o.len() <= 32
                && o.iter().all(|(k, v)| {
                    if k == "sessionId" {
                        return v.as_str().is_some_and(|s| {
                            s.len() == 32 && s.bytes().all(|b| b.is_ascii_hexdigit())
                        });
                    }
                    if k == "release" {
                        return v.as_str() == Some(env!("CARGO_PKG_VERSION"));
                    }
                    [
                        "schema",
                        "kind",
                        "code",
                        "scene",
                        "uptimeSeconds",
                        "serverUptimeSeconds",
                        "status",
                        "route",
                        "errorType",
                        "frames",
                        "asset",
                        "line",
                        "column",
                        "context",
                        "browser",
                        "browserMajor",
                        "platform",
                        "width",
                        "height",
                        "pixelRatio",
                        "cores",
                        "online",
                        "mse",
                        "wasm",
                        "fullscreen",
                        "pip",
                        "player",
                        "active",
                        "mode",
                        "quality",
                        "timeBucket",
                        "durationBucket",
                        "paused",
                        "readyState",
                        "error",
                        "runtime",
                        "os",
                        "arch",
                        "replay",
                        "offsetMs",
                        "target",
                        "x",
                        "y",
                        "w",
                        "h",
                        "boxes",
                    ]
                    .contains(&k.as_str())
                        && safe_report(v, depth + 1)
                })
        }
    }
}
pub async fn report(
    State(app): State<App>,
    headers: HeaderMap,
    Json(mut value): Json<Value>,
) -> Response {
    let Some(t) = TELEMETRY.get() else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };
    if !enabled(t) {
        return StatusCode::NO_CONTENT.into_response();
    }
    let Some(p) = auth::principal(&app.db, &headers) else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    if !safe_report(&value, 0)
        || value.as_object().is_none()
        || !value
            .get("scene")
            .and_then(Value::as_str)
            .is_some_and(|s| SCENES.contains(&s))
    {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error":"Only sanitized diagnostic fields are accepted"})),
        )
            .into_response();
    }
    let mut limits = t.limits.lock().await;
    limits.retain(|_, (at, _)| at.elapsed() < Duration::from_secs(600));
    if limits.len() >= 1024 && !limits.contains_key(&p.profile_id) {
        return StatusCode::TOO_MANY_REQUESTS.into_response();
    }
    let entry = limits.entry(p.profile_id).or_insert((Instant::now(), 0));
    if entry.1 >= 30 {
        return StatusCode::TOO_MANY_REQUESTS.into_response();
    }
    entry.1 += 1;
    drop(limits);
    value["serverUptimeSeconds"] = json!(uptime(t));
    value["release"] = json!(env!("CARGO_PKG_VERSION"));
    if t.sender
        .try_send((t.generation.load(Ordering::Acquire), value))
        .is_err()
    {
        return StatusCode::TOO_MANY_REQUESTS.into_response();
    }
    StatusCode::ACCEPTED.into_response()
}
pub fn init(app: App) {
    let raw = std::env::var("SMART_MEDIA_TELEMETRY_ENDPOINT")
        .ok()
        .or_else(|| option_env!("PLATNO_TELEMETRY_ENDPOINT").map(str::to_owned));
    let beta = std::env::var("SMART_MEDIA_RELEASE_CHANNEL")
        .ok()
        .as_deref()
        .unwrap_or(option_env!("PLATNO_RELEASE_CHANNEL").unwrap_or("stable"))
        == "beta"
        && std::env::var("SMART_MEDIA_TELEMETRY").ok().as_deref() != Some("off");
    let (sender, mut receiver) = mpsc::channel::<(u64, Value)>(32);
    let allowed = db(&app)
        .query_row(
            "SELECT value FROM server_settings WHERE key='beta_telemetry'",
            [],
            |r| r.get::<_, String>(0),
        )
        .map(|s| s != "false")
        .unwrap_or(true);
    let t = Arc::new(Telemetry {
        endpoint: raw.as_deref().and_then(endpoint),
        beta,
        started: Instant::now(),
        session: random_id(),
        sender,
        allowed: AtomicBool::new(allowed),
        generation: AtomicU64::new(0),
        limits: Mutex::new(std::collections::HashMap::new()),
    });
    if TELEMETRY.set(t.clone()).is_err() {
        return;
    }
    if enabled(&t) {
        println!("BETA DIAGNOSTICS ENABLED: anonymous errors and heartbeats go to {}. Masked browser replay is automatic after sign-in. Disable in Settings or SMART_MEDIA_TELEMETRY=off. See BETA.md.",t.endpoint.as_ref().unwrap().host_str().unwrap_or("collector"));
    }
    let transport = t.clone();
    tokio::spawn(async move {
        let Ok(client) = reqwest::Client::builder()
            .timeout(Duration::from_secs(8))
            .redirect(reqwest::redirect::Policy::none())
            .build()
        else {
            return;
        };
        let mut window = Instant::now();
        let mut sent = 0;
        while let Some((generation, value)) = receiver.recv().await {
            if generation != transport.generation.load(Ordering::Acquire) || !enabled(&transport) {
                continue;
            }
            if window.elapsed() > Duration::from_secs(600) {
                window = Instant::now();
                sent = 0
            }
            if sent >= 100 {
                continue;
            }
            sent += 1;
            if let Some(url) = transport.endpoint.as_ref() {
                let _ = client.post(url.clone()).json(&value).send().await;
            }
        }
    });
    let heartbeat = t.clone();
    tokio::spawn(async move {
        loop {
            if enabled(&heartbeat) {
                let _ = heartbeat.sender.try_send((
                    heartbeat.generation.load(Ordering::Acquire),
                    server_report(&heartbeat, "heartbeat", "heartbeat", None, None),
                ));
            }
            tokio::time::sleep(Duration::from_secs(300)).await;
        }
    });
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        enqueue("panic", "panic", None, None);
        previous(info)
    }));
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn endpoints_require_https_and_fixed_ingest_path() {
        for s in [
            "http://example.test/api/ingest",
            "http://127.0.0.1.evil.test/api/ingest",
            "http://127.0.0.1:9/other",
            "ftp://127.0.0.1/api/ingest",
            "https://a:b@example.test/api/ingest",
            "https://example.test/api/ingest?key=secret",
            "https://example.test/other",
        ] {
            assert!(endpoint(s).is_none())
        }
        assert!(endpoint("https://example.test/api/ingest").is_some());
        assert!(endpoint("http://127.0.0.1:8123/api/ingest").is_some());
        assert!(endpoint("http://localhost:8123/api/ingest").is_some())
    }
    #[test]
    fn diagnostic_schema_rejects_private_data() {
        let safe = json!({"schema":1,"sessionId":"1234567890abcdef1234567890abcdef","kind":"browser_error","code":"read-null","scene":"library","uptimeSeconds":42,"release":env!("CARGO_PKG_VERSION"),"frames":[{"asset":"app","line":42,"column":3}],"context":{"runtime":"browser"},"replay":[{"kind":"click","scene":"library","offsetMs":42,"target":"button"}]});
        assert!(safe_report(&safe, 0));
        for v in [
            json!({"message":"private movie title"}),
            json!({"path":"/Users/private/movie.mkv"}),
            json!({"context":{"email":"user@example.test"}}),
            json!({"replay":[{"snapshot":"<html>private</html>"}]}),
            json!({"code":"secret-token-value"}),
        ] {
            assert!(!safe_report(&v, 0));
        }
    }
}
