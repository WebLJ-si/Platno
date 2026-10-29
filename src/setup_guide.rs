//! Read-only facts for the first-run guide: hardware, encoders and which provider keys exist.
use crate::{playback, App};
use axum::{extract::State, Json};
use serde_json::{json, Value};
use std::time::Duration;
use tokio::{process::Command, sync::OnceCell};

static GPU: OnceCell<Option<String>> = OnceCell::const_new();

async fn memory_bytes() -> Option<u64> {
    #[cfg(target_os = "macos")]
    {
        let out = crate::processes::output(Command::new("sysctl").args(["-n", "hw.memsize"]), Duration::from_secs(3), 1024).await.ok()?;
        String::from_utf8_lossy(&out.stdout).trim().parse().ok()
    }
    #[cfg(target_os = "linux")]
    {
        let text = tokio::fs::read_to_string("/proc/meminfo").await.ok()?;
        let kib: u64 = text.lines().find_map(|l| l.strip_prefix("MemTotal:"))?.split_whitespace().next()?.parse().ok()?;
        Some(kib * 1024)
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        None
    }
}
// Best effort: macOS names the graphics chip; elsewhere the encoder list is the only reliable hint.
async fn gpu_name() -> Option<String> {
    GPU.get_or_init(|| async {
        if !cfg!(target_os = "macos") {
            return None;
        }
        let out = crate::processes::output(Command::new("system_profiler").arg("SPDisplaysDataType"), Duration::from_secs(10), 200_000).await.ok()?;
        let names: Vec<String> = String::from_utf8_lossy(&out.stdout).lines().filter_map(|l| l.trim().strip_prefix("Chipset Model:")).map(|n| n.trim().to_owned()).collect();
        (!names.is_empty()).then(|| names.join(", "))
    })
    .await
    .clone()
}
async fn ffmpeg_works() -> bool {
    matches!(crate::processes::output(Command::new(playback::binary("ffmpeg")).arg("-version"), Duration::from_secs(5), 100_000).await, Ok(out) if out.status.success())
}
// Every conversion is a separate FFmpeg process (adaptive streaming runs three), so memory and
// cores both bound how many are sensible at once.
pub(crate) fn recommended_slots(memory_gb: Option<f64>, cores: usize) -> usize {
    let by_memory = memory_gb.map_or(2, |gb| (gb / 4.0) as usize);
    by_memory.min(cores / 2).clamp(1, 6)
}
pub async fn system(State(app): State<App>) -> Json<Value> {
    let memory = memory_bytes().await;
    let memory_gb = memory.map(|b| (b as f64 / 1_073_741_824.0 * 10.0).round() / 10.0);
    let cores = std::thread::available_parallelism().map_or(1, |n| n.get());
    let encoders = playback::available_h264_encoders().await;
    // An FFmpeg build can list an encoder (NVENC on a machine without an NVIDIA GPU) that cannot start.
    let usable: Vec<&str> = futures_util::future::join_all(encoders.iter().map(|e| async move { (*e, playback::encoder_works(e).await) })).await.into_iter().filter_map(|(e, ok)| ok.then_some(e)).collect();
    let (gpu_hint, gpu) = (
        usable.iter().find_map(|e| match *e {
            "h264_videotoolbox" => Some("Apple VideoToolbox"),
            "h264_nvenc" => Some("NVIDIA NVENC"),
            "h264_qsv" => Some("Intel Quick Sync"),
            _ => None,
        }),
        gpu_name().await,
    );
    Json(json!({
        "memory_gb": memory_gb, "cpu_threads": cores, "os": std::env::consts::OS, "arch": std::env::consts::ARCH,
        "gpu": gpu, "hardware_encoder_hint": gpu_hint, "encoders": encoders, "usable_encoders": usable, "ffmpeg": ffmpeg_works().await,
        "data_dir": app.db.parent().map(|p| p.to_string_lossy().into_owned()),
        "conversion_slots": playback::conversion_slots(&app), "recommended_slots": recommended_slots(memory_gb, cores),
        "max_slots": playback::MAX_CONVERSION_SLOTS,
        "listen_host": std::env::var("SMART_MEDIA_HOST").unwrap_or_else(|_| "127.0.0.1".into()),
        "port": std::env::var("SMART_MEDIA_PORT").ok().and_then(|p| p.parse::<u16>().ok()).unwrap_or(8787),
    }))
}
// Never returns the secrets themselves.
pub async fn providers(State(app): State<App>) -> Json<Value> {
    Json(json!({"tmdb": !crate::tmdb_token(&app).is_empty(), "opensubtitles": !crate::subtitle_key(&app).is_empty()}))
}

#[derive(serde::Deserialize)]
pub struct CheckFolder {
    path: String,
}
// Lets the guide say "found" or "not found" before a folder is saved. The slow-drive timeout
// matters: an unplugged network share must not hang the guide.
pub async fn check_folder(Json(input): Json<CheckFolder>) -> Json<Value> {
    let path = input.path.trim().to_owned();
    if path.is_empty() || path.len() > 1024 || path.chars().any(char::is_control) || !std::path::Path::new(&path).is_absolute() {
        return Json(json!({"ok": false, "reason": "invalid"}));
    }
    let probe = tokio::task::spawn_blocking(move || match std::fs::metadata(&path) {
        Ok(m) if m.is_dir() => if std::fs::read_dir(&path).is_ok() { "ok" } else { "unreadable" },
        Ok(_) => "not_folder",
        Err(_) => "missing",
    });
    let reason = match tokio::time::timeout(Duration::from_secs(5), probe).await {
        Ok(Ok(reason)) => reason,
        _ => "timeout",
    };
    Json(json!({"ok": reason == "ok", "reason": reason}))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn folder_check_reports_each_failure_mode() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("file.txt");
        std::fs::write(&file, b"x").unwrap();
        let check = |p: String| async move { check_folder(Json(CheckFolder { path: p })).await.0["reason"].as_str().unwrap().to_owned() };
        assert_eq!(check(dir.path().to_string_lossy().into()).await, "ok");
        assert_eq!(check(file.to_string_lossy().into()).await, "not_folder");
        assert_eq!(check(dir.path().join("nope").to_string_lossy().into()).await, "missing");
        for bad in ["", "relative/dir", "/tmp/a\u{0}b"] {
            assert_eq!(check(bad.into()).await, "invalid", "{bad:?}");
        }
    }
    #[test]
    fn slot_recommendation_follows_the_scarcer_resource() {
        assert_eq!(recommended_slots(Some(4.0), 16), 1);
        assert_eq!(recommended_slots(Some(16.0), 16), 4);
        assert_eq!(recommended_slots(Some(64.0), 4), 2);
        assert_eq!(recommended_slots(Some(256.0), 64), 6);
        assert_eq!(recommended_slots(Some(0.5), 1), 1);
        assert_eq!(recommended_slots(None, 8), 2);
    }
}
