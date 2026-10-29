use crate::{playback, App};
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;
use serde_json::json;

#[derive(Deserialize)]
pub struct ThumbnailQuery {
    path: String,
    time: f64,
}
static PREVIEW_JOBS: std::sync::LazyLock<tokio::sync::Semaphore> =
    std::sync::LazyLock::new(|| tokio::sync::Semaphore::new(2));
pub async fn thumbnail(State(app): State<App>, Query(input): Query<ThumbnailQuery>) -> Response {
    let Some(path) = playback::library_path(&app, &input.path) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    if !input.time.is_finite() || !(0.0..=86400.0).contains(&input.time) {
        return StatusCode::BAD_REQUEST.into_response();
    }
    let Ok(permit) = PREVIEW_JOBS.try_acquire() else {
        return (
            StatusCode::TOO_MANY_REQUESTS,
            Json(json!({"error":"Preview generation is busy"})),
        )
            .into_response();
    };
    let stat = match tokio::time::timeout(
        std::time::Duration::from_secs(3),
        tokio::fs::metadata(&path),
    )
    .await
    {
        Ok(Ok(v)) => v,
        _ => return StatusCode::NOT_FOUND.into_response(),
    };
    let time = (input.time / 10.0).floor() * 10.0;
    let signature = format!("{}:{}:{:?}:{time}", input.path, stat.len(), stat.modified());
    let digest = ring::digest::digest(&ring::digest::SHA256, signature.as_bytes());
    let key = digest
        .as_ref()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    let cache = app.db.parent().unwrap().join("previews");
    let cached = cache.join(format!("{key}.jpg"));
    let jpeg = if let Ok(v) = tokio::fs::read(&cached).await {
        v
    } else {
        let mut command = tokio::process::Command::new(playback::binary("ffmpeg"));
        command
            .args([
                "-v",
                "error",
                "-nostdin",
                "-ss",
                &format!("{time:.3}"),
                "-i",
            ])
            .arg(path)
            .args([
                "-map",
                "0:v:0",
                "-frames:v",
                "1",
                "-vf",
                "scale=240:-2",
                "-f",
                "image2pipe",
                "-c:v",
                "mjpeg",
                "pipe:1",
            ])
            .kill_on_drop(true);
        let output = match crate::processes::output(&mut command,std::time::Duration::from_secs(15),1_000_000).await {
            Ok(v) if v.status.success() && v.stdout.len() > 4 => v.stdout,
            _ => {
                return (
                    StatusCode::BAD_GATEWAY,
                    Json(json!({"error":"Preview unavailable"})),
                )
                    .into_response()
            }
        };
        let saved=output.clone();
        let _=tokio::task::spawn_blocking(move||cache_preview(&cache,&cached,&saved)).await;
        output
    };
    drop(permit);
    (
        [
            (axum::http::header::CONTENT_TYPE, "image/jpeg"),
            (axum::http::header::CACHE_CONTROL, "private, max-age=300"),
        ],
        jpeg,
    )
        .into_response()
}

// Serialize pruning and publish each JPEG by rename, never as a partial file.
fn cache_preview(cache:&std::path::Path,cached:&std::path::Path,data:&[u8])->std::io::Result<()> {
    use std::{io::Write,sync::{Mutex,atomic::{AtomicU64,Ordering}}};
    static WRITER:Mutex<()>=Mutex::new(());
    static SEQUENCE:AtomicU64=AtomicU64::new(0);
    let _lock=WRITER.lock().unwrap_or_else(|e|e.into_inner());
    std::fs::create_dir_all(cache)?;
    let mut names:Vec<_>=std::fs::read_dir(cache)?.filter_map(Result::ok).map(|entry|entry.path()).filter(|p|p.file_name().and_then(|n|n.to_str()).is_some_and(|n|n.strip_suffix(".jpg").is_some_and(|stem|stem.len()==64&&stem.bytes().all(|b|b.is_ascii_hexdigit())))).collect();
    names.sort();
    for old in names.iter().take(names.len().saturating_sub(255)) {let _=std::fs::remove_file(old);}
    let temporary=cache.join(format!(".preview-{}-{}.tmp",std::process::id(),SEQUENCE.fetch_add(1,Ordering::Relaxed)));
    let result=(||{let mut f=std::fs::OpenOptions::new().create_new(true).write(true).open(&temporary)?;f.write_all(data)?;drop(f);std::fs::rename(&temporary,cached)})();
    if result.is_err(){let _=std::fs::remove_file(&temporary);}
    result
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preview_cache_is_bounded_atomic_and_preserves_unrelated_files() {
        let dir=tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("keep.txt"),b"keep").unwrap();
        for i in 0..270 {
            let path=dir.path().join(format!("{i:064x}.jpg"));
            cache_preview(dir.path(),&path,b"complete jpeg fixture").unwrap();
            assert_eq!(std::fs::read(path).unwrap(),b"complete jpeg fixture");
        }
        assert_eq!(std::fs::read(dir.path().join("keep.txt")).unwrap(),b"keep");
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(),257);
    }
}
