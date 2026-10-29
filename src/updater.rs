//! Owner-started updates from GitHub Releases. Nothing is checked or installed unless the owner
//! presses a button: no background phone-home. The new program is downloaded over HTTPS, verified
//! against the release's SHA256SUMS, smoke-tested, swapped in with the old one kept as
//! `<name>.previous`, and the server restarts itself. Docker and distribution packages are updated
//! with their own tools, so there this only explains how.
use crate::App;
use axum::{extract::State, http::StatusCode, response::{IntoResponse, Response}, Json};
use ring::digest;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        OnceLock,
    },
    time::Duration,
};
use tokio::{io::AsyncWriteExt, sync::{Mutex, Notify}};

const DEFAULT_API: &str = "https://api.github.com/repos/WebLJ-si/Platno";
const MAX_DOWNLOAD: u64 = 200 * 1024 * 1024;
static LATEST: Mutex<Option<(Release, i64)>> = Mutex::const_new(None);
static APPLYING: Mutex<()> = Mutex::const_new(());
static RESTART: AtomicBool = AtomicBool::new(false);
static RESTART_NOTIFY: Notify = Notify::const_new();
static START: OnceLock<(PathBuf, i64)> = OnceLock::new();

pub(crate) fn init() {
    // The path is remembered now: after the swap, current_exe() would name the renamed old file.
    let _ = START.set((std::env::current_exe().unwrap_or_default(), chrono::Utc::now().timestamp()));
}
pub(crate) async fn restart_requested() {
    RESTART_NOTIFY.notified().await
}
pub(crate) fn take_restart() -> bool {
    RESTART.swap(false, Ordering::AcqRel)
}
// Replaces this process with the freshly installed program, keeping arguments and environment.
pub(crate) fn restart_into_new_program() -> ! {
    let exe = START.get().map(|s| s.0.clone()).unwrap_or_default();
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let mut command = std::process::Command::new(&exe);
    command.args(&args);
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        let error = command.exec();
        eprintln!("Could not start the updated program: {error}");
        // Put the previous version back so a supervisor restart still works.
        let previous = previous_path(&exe);
        if previous.is_file() && std::fs::rename(&previous, &exe).is_ok() {
            let _ = std::process::Command::new(&exe).args(&args).exec();
        }
        std::process::exit(1)
    }
    #[cfg(not(unix))]
    {
        match command.spawn() {
            Ok(_) => std::process::exit(0),
            Err(error) => {
                eprintln!("Could not start the updated program: {error}");
                let previous = previous_path(&exe);
                if previous.is_file() && std::fs::rename(&previous, &exe).is_ok() {
                    let _ = std::process::Command::new(&exe).args(&args).spawn();
                }
                std::process::exit(1)
            }
        }
    }
}

/* ---------- pure helpers ---------- */
pub(crate) fn parse_version(text: &str) -> Option<(u64, u64, u64, bool)> {
    let text = text.trim().trim_start_matches('v');
    let (core, pre) = match text.split_once('-') {
        Some((core, pre)) => (core, !pre.is_empty()),
        None => (text, false),
    };
    let mut parts = core.split('.').map(|p| p.parse::<u64>().ok());
    let version = (parts.next()??, parts.next()??, parts.next()??, pre);
    parts.next().is_none().then_some(version)
}
// A release is newer when its number is higher; a pre-release is older than the same number.
pub(crate) fn is_newer(candidate: &str, current: &str) -> bool {
    match (parse_version(candidate), parse_version(current)) {
        (Some(a), Some(b)) => (a.0, a.1, a.2, !a.3) > (b.0, b.1, b.2, !b.3),
        _ => false,
    }
}
pub(crate) fn asset_suffix(os: &str, arch: &str) -> Option<&'static str> {
    match (os, arch) {
        ("linux", "x86_64") => Some("linux-x86_64"),
        ("linux", "aarch64") => Some("linux-aarch64"),
        ("macos", "aarch64" | "x86_64") => Some("macos-universal"),
        ("windows", "x86_64") => Some("windows-x86_64"),
        _ => None,
    }
}
fn deb_arch(arch: &str) -> Option<&'static str> {
    match arch {
        "x86_64" => Some("amd64"),
        "aarch64" => Some("arm64"),
        _ => None,
    }
}
pub(crate) fn asset_name(version: &str, suffix: &str) -> String {
    format!("platno-{version}-{suffix}{}", if suffix.starts_with("windows") { ".exe" } else { "" })
}
pub(crate) fn checksum_for(sums: &str, file: &str) -> Option<String> {
    sums.lines().find_map(|line| {
        let (hash, name) = line.split_once(char::is_whitespace)?;
        (name.trim().trim_start_matches('*') == file && hash.len() == 64 && hash.bytes().all(|b| b.is_ascii_hexdigit())).then(|| hash.to_ascii_lowercase())
    })
}
fn trusted_url(raw: &str, api: &str) -> bool {
    let Ok(url) = reqwest::Url::parse(raw) else { return false };
    let origin = |u: &reqwest::Url| u.host_str().map(|h| (h.to_owned(), u.port_or_known_default()));
    let same_server = reqwest::Url::parse(api).ok().is_some_and(|a| origin(&a) == origin(&url));
    let host = url.host_str().unwrap_or("");
    let loopback = matches!(host, "127.0.0.1" | "localhost" | "[::1]");
    let github = host == "github.com" || host.ends_with(".githubusercontent.com");
    url.username().is_empty() && url.password().is_none() && ((url.scheme() == "https" && (github || same_server)) || (url.scheme() == "http" && loopback && same_server))
}
fn trusted_api(raw: &str) -> bool {
    reqwest::Url::parse(raw).is_ok_and(|u| {
        u.query().is_none() && u.fragment().is_none() && u.username().is_empty() && (u.scheme() == "https" || (u.scheme() == "http" && matches!(u.host_str(), Some("127.0.0.1" | "localhost" | "[::1]"))))
    })
}
fn api_base() -> String {
    std::env::var("SMART_MEDIA_UPDATE_API").ok().filter(|u| trusted_api(u)).map(|u| u.trim_end_matches('/').to_owned()).unwrap_or_else(|| DEFAULT_API.to_owned())
}
fn previous_path(exe: &Path) -> PathBuf {
    let mut name = exe.file_name().unwrap_or_default().to_os_string();
    name.push(".previous");
    exe.with_file_name(name)
}

#[derive(Clone, Serialize, Debug, PartialEq)]
pub(crate) struct Asset {
    name: String,
    url: String,
    size: u64,
}
#[derive(Clone, Serialize, Debug, PartialEq)]
pub(crate) struct Release {
    version: String,
    name: String,
    notes: String,
    published_at: String,
    page_url: String,
    asset: Option<Asset>,
    checksums: Option<String>,
    package: Option<Asset>,
}
pub(crate) fn parse_release(json: &Value, os: &str, arch: &str) -> Option<Release> {
    if json["draft"] == true || json["prerelease"] == true {
        return None;
    }
    let tag = json["tag_name"].as_str()?;
    parse_version(tag)?;
    let version = tag.trim_start_matches('v').to_owned();
    let assets = json["assets"].as_array()?;
    let find = |wanted: &str| {
        assets.iter().find(|a| a["name"].as_str() == Some(wanted)).and_then(|a| Some(Asset { name: wanted.to_owned(), url: a["browser_download_url"].as_str()?.to_owned(), size: a["size"].as_u64()? }))
    };
    Some(Release {
        asset: asset_suffix(os, arch).and_then(|s| find(&asset_name(&version, s))),
        package: deb_arch(arch).filter(|_| os == "linux").and_then(|a| find(&format!("platno_{version}_{a}.deb"))),
        checksums: find("SHA256SUMS").map(|a| a.url),
        name: json["name"].as_str().unwrap_or(tag).chars().take(120).collect(),
        notes: json["body"].as_str().unwrap_or("").chars().take(4000).collect(),
        published_at: json["published_at"].as_str().unwrap_or("").to_owned(),
        page_url: json["html_url"].as_str().filter(|u| u.starts_with("https://") || u.starts_with("http://127.0.0.1")).unwrap_or("").to_owned(),
        version,
    })
}

/* ---------- how this copy was installed ---------- */
#[derive(Debug, PartialEq)]
pub(crate) enum Method {
    SelfUpdate,
    Docker,
    Package,
    ReadOnly,
    Disabled,
}
fn writable_directory(dir: &Path) -> bool {
    let probe = dir.join(format!(".platno-write-test-{}", std::process::id()));
    std::fs::OpenOptions::new().write(true).create_new(true).open(&probe).is_ok_and(|_| std::fs::remove_file(&probe).is_ok())
}
pub(crate) fn method(distribution: &str, checks_enabled: bool, directory_writable: bool) -> Method {
    match (checks_enabled, distribution) {
        (false, _) => Method::Disabled,
        (_, "docker") => Method::Docker,
        (_, "deb") => Method::Package,
        _ if directory_writable => Method::SelfUpdate,
        _ => Method::ReadOnly,
    }
}
fn current_method() -> Method {
    let exe = START.get().map(|s| s.0.clone()).unwrap_or_default();
    let distribution = std::env::var("SMART_MEDIA_DISTRIBUTION").unwrap_or_default();
    method(&distribution, std::env::var("SMART_MEDIA_UPDATE_CHECK").ok().as_deref() != Some("off"), exe.parent().is_some_and(writable_directory))
}
fn instructions(method: &Method, latest: Option<&Release>) -> String {
    match method {
        Method::SelfUpdate => String::new(),
        Method::Docker => "This server runs in Docker. Update it by pulling the new image: docker compose pull && docker compose up -d".into(),
        Method::Package => latest.and_then(|r| r.package.as_ref()).map_or_else(
            || "This server was installed from a .deb package. Download the new .deb from the release page and run: sudo apt install ./platno_VERSION_ARCH.deb".into(),
            |p| format!("This server was installed from a .deb package. Download {} from the release page and run: sudo apt install ./{}", p.name, p.name),
        ),
        Method::ReadOnly => "The program folder is not writable for this user, so Platno cannot replace itself. Download the new version from the release page and replace the program file, or run Platno from a folder you own.".into(),
        Method::Disabled => "Update checks are turned off with SMART_MEDIA_UPDATE_CHECK=off.".into(),
    }
}
async fn describe() -> Value {
    let method = current_method();
    let latest = LATEST.lock().await.clone();
    let current = env!("CARGO_PKG_VERSION");
    let newer = latest.as_ref().filter(|(r, _)| is_newer(&r.version, current));
    json!({
        "current": current, "running_since": START.get().map(|s| s.1), "os": std::env::consts::OS, "arch": std::env::consts::ARCH,
        "enabled": method != Method::Disabled, "can_install": method == Method::SelfUpdate && newer.is_some_and(|(r, _)| r.asset.is_some() && r.checksums.is_some()),
        "method": format!("{method:?}").to_lowercase(), "instructions": instructions(&method, latest.as_ref().map(|l| &l.0)),
        "checked_at": latest.as_ref().map(|l| l.1), "latest": latest.as_ref().map(|l| &l.0), "update_available": newer.is_some(),
        "platform_supported": asset_suffix(std::env::consts::OS, std::env::consts::ARCH).is_some(),
    })
}
fn fail(status: StatusCode, message: &str) -> Response {
    (status, Json(json!({"error": message}))).into_response()
}

pub async fn status() -> Json<Value> {
    Json(describe().await)
}
pub async fn check(State(app): State<App>) -> Response {
    if current_method() == Method::Disabled {
        return fail(StatusCode::FORBIDDEN, "Update checks are turned off on this server.");
    }
    let response = app.client.get(format!("{}/releases/latest", api_base())).header("Accept", "application/vnd.github+json").send().await;
    let json = match response {
        Ok(r) if r.status() == StatusCode::NOT_FOUND => return fail(StatusCode::NOT_FOUND, "No published release was found yet."),
        Ok(r) if r.status().is_success() => r.json::<Value>().await.ok(),
        Ok(r) => return fail(StatusCode::BAD_GATEWAY, &format!("GitHub answered with HTTP {}. Try again later.", r.status().as_u16())),
        Err(_) => return fail(StatusCode::BAD_GATEWAY, "Could not reach GitHub. Check the internet connection."),
    };
    let Some(release) = json.and_then(|j| parse_release(&j, std::env::consts::OS, std::env::consts::ARCH)) else {
        return fail(StatusCode::BAD_GATEWAY, "GitHub returned a release this version cannot read.");
    };
    *LATEST.lock().await = Some((release, chrono::Utc::now().timestamp()));
    Json(describe().await).into_response()
}

#[derive(Deserialize)]
pub struct Apply {
    version: String,
}
async fn download(client: &reqwest::Client, url: &str, api: &str, limit: u64, mut file: Option<&mut tokio::fs::File>) -> Result<(Vec<u8>, String), String> {
    if !trusted_url(url, api) {
        return Err("The release points to an address that is not trusted.".into());
    }
    let mut response = client.get(url).send().await.map_err(|_| "The download could not start. Check the internet connection.".to_string())?;
    if !response.status().is_success() || !trusted_url(response.url().as_str(), api) {
        return Err(format!("The download failed (HTTP {}).", response.status().as_u16()));
    }
    let (mut total, mut hasher, mut kept) = (0u64, digest::Context::new(&digest::SHA256), Vec::new());
    while let Some(chunk) = response.chunk().await.map_err(|_| "The download was interrupted.".to_string())? {
        total += chunk.len() as u64;
        if total > limit {
            return Err("The download is larger than expected and was stopped.".into());
        }
        hasher.update(&chunk);
        match file.as_deref_mut() {
            Some(f) => f.write_all(&chunk).await.map_err(|_| "The update could not be written to disk.".to_string())?,
            None => kept.extend_from_slice(&chunk),
        }
    }
    let hash: String = hasher.finish().as_ref().iter().map(|b| format!("{b:02x}")).collect();
    Ok((kept, hash))
}
pub async fn apply(Json(input): Json<Apply>) -> Response {
    if current_method() != Method::SelfUpdate {
        return fail(StatusCode::CONFLICT, &instructions(&current_method(), LATEST.lock().await.as_ref().map(|l| &l.0)));
    }
    let Ok(_busy) = APPLYING.try_lock() else { return fail(StatusCode::CONFLICT, "An update is already being installed.") };
    let Some((release, _)) = LATEST.lock().await.clone().filter(|(r, _)| r.version == input.version && is_newer(&r.version, env!("CARGO_PKG_VERSION"))) else {
        return fail(StatusCode::BAD_REQUEST, "Check for updates first, then install the version that was found.");
    };
    let (Some(asset), Some(sums_url)) = (release.asset.clone(), release.checksums.clone()) else {
        return fail(StatusCode::CONFLICT, "This release has no download for this computer or no checksum file.");
    };
    match install(&release, &asset, &sums_url).await {
        Ok(()) => {
            tokio::spawn(async {
                tokio::time::sleep(Duration::from_millis(600)).await;
                RESTART.store(true, Ordering::Release);
                RESTART_NOTIFY.notify_one();
            });
            Json(json!({"ok": true, "restarting": true, "version": release.version})).into_response()
        }
        Err(message) => fail(StatusCode::BAD_GATEWAY, &message),
    }
}
async fn install(_release: &Release, asset: &Asset, sums_url: &str) -> Result<(), String> {
    let exe = START.get().map(|s| s.0.clone()).filter(|p| p.is_file()).ok_or("The program file could not be found.")?;
    let dir = exe.parent().ok_or("The program folder could not be found.")?.to_path_buf();
    let api = api_base();
    let client = reqwest::Client::builder().user_agent("Platno-updater").connect_timeout(Duration::from_secs(15)).timeout(Duration::from_secs(900)).build().map_err(|_| "The download client could not start.".to_string())?;
    let (sums, _) = download(&client, sums_url, &api, 64 * 1024, None).await?;
    let wanted = checksum_for(&String::from_utf8_lossy(&sums), &asset.name).ok_or("The release's checksum file does not list this download, so it was not installed.")?;
    let staged = dir.join(format!(".platno-update-{}{}", std::process::id(), if cfg!(windows) { ".exe" } else { "" }));
    let result = stage_and_swap(&client, &api, asset, &wanted, &staged, &exe).await;
    if result.is_err() {
        let _ = tokio::fs::remove_file(&staged).await;
    }
    result
}
async fn stage_and_swap(client: &reqwest::Client, api: &str, asset: &Asset, wanted: &str, staged: &Path, exe: &Path) -> Result<(), String> {
    let mut file = tokio::fs::File::create(staged).await.map_err(|_| "The program folder is not writable.".to_string())?;
    let (_, hash) = download(client, &asset.url, api, MAX_DOWNLOAD, Some(&mut file)).await?;
    file.flush().await.map_err(|_| "The update could not be written to disk.".to_string())?;
    drop(file);
    if hash != wanted {
        return Err("The downloaded file does not match its published checksum, so it was discarded.".into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(staged, std::fs::Permissions::from_mode(0o755)).map_err(|_| "The update could not be made executable.".to_string())?;
    }
    let probe = crate::processes::output(tokio::process::Command::new(staged).arg("--version"), Duration::from_secs(10), 4096).await;
    if !probe.is_ok_and(|o| o.status.success() && String::from_utf8_lossy(&o.stdout).starts_with("platno ")) {
        return Err("The downloaded program does not run on this computer, so it was not installed.".into());
    }
    let previous = previous_path(exe);
    let _ = std::fs::remove_file(&previous);
    std::fs::rename(exe, &previous).map_err(|_| "The current program could not be set aside.".to_string())?;
    if std::fs::rename(staged, exe).is_err() {
        let _ = std::fs::rename(&previous, exe);
        return Err("The new program could not be put in place; nothing was changed.".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn versions_compare_like_releases() {
        for (a, b, newer) in [("0.2.0", "0.1.9", true), ("v1.0.0", "0.9.9", true), ("0.1.0", "0.1.0", false), ("0.1.0", "0.1.1", false), ("1.0.0", "1.0.0-beta.1", true), ("1.0.0-beta.2", "1.0.0", false), ("1.10.0", "1.9.0", true), ("garbage", "0.1.0", false), ("1.0", "0.1.0", false), ("1.0.0.1", "0.1.0", false)] {
            assert_eq!(is_newer(a, b), newer, "{a} vs {b}");
        }
    }
    #[test]
    fn each_platform_gets_exactly_its_own_download() {
        assert_eq!(asset_suffix("linux", "x86_64"), Some("linux-x86_64"));
        assert_eq!(asset_suffix("macos", "aarch64"), asset_suffix("macos", "x86_64"));
        assert_eq!(asset_suffix("freebsd", "x86_64"), None);
        assert_eq!(asset_name("1.2.3", "windows-x86_64"), "platno-1.2.3-windows-x86_64.exe");
        assert_eq!(asset_name("1.2.3", "linux-aarch64"), "platno-1.2.3-linux-aarch64");
    }
    fn release_json() -> Value {
        json!({"tag_name": "v0.2.0", "name": "Platno 0.2.0", "body": "Notes", "published_at": "2026-11-01T10:00:00Z", "html_url": "https://github.com/WebLJ-si/Platno/releases/tag/v0.2.0",
            "assets": [
                {"name": "platno-0.2.0-linux-x86_64", "size": 100, "browser_download_url": "https://github.com/x/platno-0.2.0-linux-x86_64"},
                {"name": "platno-0.2.0-macos-universal", "size": 200, "browser_download_url": "https://github.com/x/platno-0.2.0-macos-universal"},
                {"name": "platno_0.2.0_amd64.deb", "size": 300, "browser_download_url": "https://github.com/x/platno_0.2.0_amd64.deb"},
                {"name": "SHA256SUMS", "size": 400, "browser_download_url": "https://github.com/x/SHA256SUMS"}]})
    }
    #[test]
    fn release_parsing_picks_the_matching_files_and_ignores_unfit_releases() {
        let linux = parse_release(&release_json(), "linux", "x86_64").unwrap();
        assert_eq!((linux.version.as_str(), linux.asset.as_ref().map(|a| a.size), linux.package.as_ref().map(|a| a.name.as_str())), ("0.2.0", Some(100), Some("platno_0.2.0_amd64.deb")));
        assert!(linux.checksums.is_some());
        let mac = parse_release(&release_json(), "macos", "aarch64").unwrap();
        assert_eq!((mac.asset.map(|a| a.size), mac.package), (Some(200), None));
        assert_eq!(parse_release(&release_json(), "linux", "aarch64").unwrap().asset, None, "no download for an architecture that was not built");
        assert_eq!(parse_release(&release_json(), "windows", "x86_64").unwrap().asset, None);
        for (key, value) in [("prerelease", json!(true)), ("draft", json!(true)), ("tag_name", json!("nightly"))] {
            let mut bad = release_json();
            bad[key] = value;
            assert_eq!(parse_release(&bad, "linux", "x86_64"), None, "{key}");
        }
        assert_eq!(parse_release(&json!({}), "linux", "x86_64"), None);
    }
    #[test]
    fn checksums_are_matched_by_exact_file_name() {
        let sums = format!("{} *platno-0.2.0-linux-x86_64\n{}  platno-0.2.0-linux-x86_64.sig\nnot-a-hash  other\n", "a".repeat(64), "b".repeat(64));
        assert_eq!(checksum_for(&sums, "platno-0.2.0-linux-x86_64"), Some("a".repeat(64)));
        assert_eq!(checksum_for(&sums, "platno-0.2.0-linux-x86_64.sig"), Some("b".repeat(64)));
        assert_eq!(checksum_for(&sums, "other"), None);
        assert_eq!(checksum_for(&sums, "missing"), None);
        assert_eq!(checksum_for("", "x"), None);
    }
    #[test]
    fn only_github_or_the_configured_test_server_may_be_downloaded_from() {
        let api = DEFAULT_API;
        for ok in ["https://github.com/WebLJ-si/Platno/releases/download/v1/x", "https://release-assets.githubusercontent.com/x"] {
            assert!(trusted_url(ok, api), "{ok}");
        }
        for bad in ["http://github.com/x", "https://github.com.evil.test/x", "https://evil.test/x", "https://user:pw@github.com/x", "ftp://github.com/x", "http://127.0.0.1:1/x", "not a url", "https://notgithubusercontent.com/x"] {
            assert!(!trusted_url(bad, api), "{bad}");
        }
        assert!(trusted_url("http://127.0.0.1:9/x", "http://127.0.0.1:9/repos/a"));
        assert!(!trusted_url("http://127.0.0.1:9/x", "http://127.0.0.1:10/repos/a"), "a different local port is a different server");
        assert!(trusted_api("https://api.github.com/repos/a/b") && trusted_api("http://localhost:5/x"));
        assert!(!trusted_api("http://example.test/x") && !trusted_api("https://x/?a=1"));
    }
    #[test]
    fn the_update_method_follows_how_the_program_was_installed() {
        assert_eq!(method("", true, true), Method::SelfUpdate);
        assert_eq!(method("", true, false), Method::ReadOnly);
        assert_eq!(method("docker", true, true), Method::Docker, "a container is updated by pulling an image even if the path is writable");
        assert_eq!(method("deb", true, true), Method::Package);
        assert_eq!(method("docker", false, true), Method::Disabled);
        assert!(instructions(&Method::Docker, None).contains("docker compose pull"));
        assert!(instructions(&Method::Package, parse_release(&release_json(), "linux", "x86_64").as_ref()).contains("platno_0.2.0_amd64.deb"));
    }
    #[test]
    fn writability_is_probed_without_leaving_files() {
        let dir = tempfile::tempdir().unwrap();
        assert!(writable_directory(dir.path()));
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
        assert!(!writable_directory(&dir.path().join("missing")));
        assert_eq!(previous_path(Path::new("/opt/platno/platno")), PathBuf::from("/opt/platno/platno.previous"));
    }
}
