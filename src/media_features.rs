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
        let output = match tokio::time::timeout(
            std::time::Duration::from_secs(15),
            command.output(),
        )
        .await
        {
            Ok(Ok(v)) if v.status.success() && v.stdout.len() > 4 && v.stdout.len() < 1_000_000 => {
                v.stdout
            }
            _ => {
                return (
                    StatusCode::BAD_GATEWAY,
                    Json(json!({"error":"Preview unavailable"})),
                )
                    .into_response()
            }
        };
        let _ = tokio::fs::create_dir_all(&cache).await;
        // Keep the cache bounded even when browsing many long videos.
        if let Ok(mut entries) = tokio::fs::read_dir(&cache).await {
            let mut names = Vec::new();
            while let Ok(Some(entry)) = entries.next_entry().await {
                names.push(entry.path());
            }
            names.sort();
            for old in names.iter().take(names.len().saturating_sub(255)) {
                let _ = tokio::fs::remove_file(old).await;
            }
        }
        let _ = tokio::fs::write(cached, &output).await;
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
