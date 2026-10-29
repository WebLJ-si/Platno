use crate::{db, playback, App};
use axum::{extract::State, Json};
use serde_json::{json, Value};
use std::time::Duration;

async fn version(name: &str) -> Value {
    let mut cmd = tokio::process::Command::new(playback::binary(name));
    cmd.arg("-version").kill_on_drop(true);
    match tokio::time::timeout(Duration::from_secs(5), cmd.output()).await {
        Ok(Ok(v)) if v.status.success() => {
            json!({"available":true,"version":String::from_utf8_lossy(&v.stdout).lines().next().unwrap_or("")})
        }
        _ => json!({"available":false}),
    }
}
pub async fn health(State(app): State<App>) -> Json<Value> {
    let (ffmpeg, ffprobe) = tokio::join!(version("ffmpeg"), version("ffprobe"));
    let c = db(&app);
    let rows: Vec<(String, i64, String)> = c
        .prepare("SELECT path,size,result FROM files")
        .and_then(|mut s| {
            s.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
                .collect()
        })
        .unwrap_or_default();
    let attention: Vec<Value> = rows
        .iter()
        .filter_map(|(path, _, raw)| {
            let result: Value = serde_json::from_str(raw).ok()?;
            let matched = result["provider_id"].as_i64().is_some()
                && result["requires_review"] != true
                && result["confidence"].as_f64().unwrap_or(0.0) > 40.0;
            if !matched {
                Some(json!({"path":path,"reason":result["explanation"]}))
            } else {
                None
            }
        })
        .take(100)
        .collect();
    let mut largest: Vec<_> = rows
        .iter()
        .map(|(p, s, _)| json!({"path":p,"bytes":s}))
        .collect();
    largest.sort_by_key(|x| std::cmp::Reverse(x["bytes"].as_i64().unwrap_or(0)));
    largest.truncate(10);
    let raw_roots: Vec<String> = c
        .prepare("SELECT roots FROM libraries")
        .and_then(|mut s| s.query_map([], |r| r.get(0))?.collect())
        .unwrap_or_default();
    drop(c);
    let mut roots = std::collections::BTreeSet::new();
    for raw in raw_roots {
        if let Ok(values) = serde_json::from_str::<Vec<String>>(&raw) {
            roots.extend(values);
        }
    }
    use futures_util::StreamExt;
    let directories: Vec<Value> = futures_util::stream::iter(roots.into_iter().take(100))
        .map(|root| async move {
            let readable = matches!(
                tokio::time::timeout(Duration::from_secs(2), tokio::fs::read_dir(&root)).await,
                Ok(Ok(_))
            );
            json!({"path":root,"readable":readable})
        })
        .buffered(8)
        .collect()
        .await;
    Json(
        json!({"ffmpeg":ffmpeg,"ffprobe":ffprobe,"tmdb_configured":!crate::tmdb_token(&app).is_empty(),"directories":directories,"files":rows.len(),"total_bytes":rows.iter().map(|(_,s,_)|*s).sum::<i64>(),"needs_attention":attention,"largest_files":largest}),
    )
}
