use crate::{playback, App};
use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;
use serde_json::json;
use std::{
    collections::HashMap,
    path::PathBuf,
    process::Stdio,
    sync::{
        atomic::{AtomicU8, Ordering},
        Arc, LazyLock, Mutex, Once,
    },
    time::{Duration, Instant},
};
use tokio::{process::Command, sync::oneshot};
use tokio_util::io::ReaderStream;

struct Session {
    profile_id: i64,
    finished: oneshot::Receiver<()>,
    dir: PathBuf,
    last: Instant,
    stop: Option<oneshot::Sender<()>>,
    status: Arc<AtomicU8>,
}
static SESSIONS: LazyLock<Mutex<HashMap<String, Session>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));
static REAPER: Once = Once::new();
// Cancelling setup must not leave a temporary streaming directory behind.
struct StreamingDir(Option<PathBuf>);
impl Drop for StreamingDir {
    fn drop(&mut self) {
        if let Some(dir)=self.0.take() {tokio::spawn(async move {let _=tokio::fs::remove_dir_all(dir).await;});}
    }
}
#[derive(Deserialize)]
pub struct Start {
    path: String,
    start: Option<f64>,
    quality: Option<String>,
    audio: Option<u32>,
}
#[derive(Deserialize)]
pub struct Stop {
    id: String,
}
fn error(status: StatusCode, msg: &str) -> Response {
    (status, Json(json!({"error":msg}))).into_response()
}
async fn remove(id: &str) {
    let session = SESSIONS.lock().unwrap().remove(id);
    if let Some(mut session) = session {
        if let Some(sender)=session.stop.take() {let _=sender.send(());}
        let _=session.finished.await;
    }
}
pub async fn shutdown_all() {
    let ids:Vec<_>=SESSIONS.lock().unwrap().keys().cloned().collect();
    for id in ids {remove(&id).await;}
}
fn start_reaper() {
    REAPER.call_once(|| {tokio::spawn(async {
        loop {
            tokio::time::sleep(Duration::from_secs(30)).await;
            // Remove and signal under the same lock as file()'s last-access update.
            let stale:Vec<_>={
                let mut sessions=SESSIONS.lock().unwrap();
                let ids:Vec<_>=sessions.iter().filter(|(_,s)|s.last.elapsed()>Duration::from_secs(150)).map(|(id,_)|id.clone()).collect();
                ids.into_iter().filter_map(|id|sessions.remove(&id)).collect()
            };
            for mut session in stale {
                if let Some(sender)=session.stop.take() {let _=sender.send(());}
                let _=session.finished.await;
            }
        }
    });});
}
pub async fn stop(axum::Extension(actor): axum::Extension<crate::auth::Principal>, Json(input): Json<Stop>) -> Response {
    let owned=SESSIONS.lock().unwrap().get(&input.id).is_some_and(|s|s.profile_id==actor.profile_id);
    if !owned {return error(StatusCode::NOT_FOUND,"Playback session expired");}
    remove(&input.id).await;
    Json(json!({"ok":true})).into_response()
}
pub async fn start(State(app): State<App>, axum::Extension(actor): axum::Extension<crate::auth::Principal>, Json(input): Json<Start>) -> Response {
    let Some(path) = playback::library_path(&app, &input.path) else {
        return error(StatusCode::NOT_FOUND, "File is not in the library");
    };
    let start = input.start.unwrap_or(0.0);
    if !start.is_finite() || !(0.0..=1_000_000.0).contains(&start) {
        return error(StatusCode::BAD_REQUEST, "Invalid playback position");
    }
    let quality=input.quality.as_deref().unwrap_or("1080");
    let tiers: Vec<&str> = if quality=="auto" {vec!["480","720","1080"]} else {vec![quality]};
    if tiers.iter().any(|q|playback::video_bitrate(&app,q).is_none()) { return error(StatusCode::BAD_REQUEST,"Invalid quality"); }
    let Ok(permit)=playback::CONVERSION_LIMIT.clone().try_acquire_owned() else {return error(StatusCode::TOO_MANY_REQUESTS,"All four conversion slots are busy. Try again shortly.");};
    let audio_map = match playback::audio_map(&path, input.audio).await { Ok(v)=>v, Err(e)=>return error(StatusCode::BAD_REQUEST,&e) };
    use ring::rand::SecureRandom;
    let mut random=[0u8;32];
    if ring::rand::SystemRandom::new().fill(&mut random).is_err() {return error(StatusCode::INTERNAL_SERVER_ERROR,"Cannot create playback session");}
    let id=random.iter().map(|b|format!("{b:02x}")).collect::<String>();
    let dir = std::env::temp_dir().join(format!("smart-media-hls-{id}"));
    if tokio::fs::create_dir(&dir).await.is_err() {
        return error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Cannot create streaming cache",
        );
    }
    let mut directory=StreamingDir(Some(dir.clone()));
    let hdr=match playback::hdr_filter(&path).await {Ok(v)=>v,Err(e)=>{let _=tokio::fs::remove_dir_all(&dir).await;return error(StatusCode::BAD_GATEWAY,&e)}};
    let (encoder,_available)=playback::h264_encoder(&app).await;
    let mut command = Command::new(playback::binary("ffmpeg"));
    command.args(["-v","error","-nostdin","-re","-ss",&format!("{start:.3}"),"-i"]).arg(&path);
    let adaptive=quality=="auto";
    let mut master=String::from("#EXTM3U\n#EXT-X-VERSION:3\n#EXT-X-INDEPENDENT-SEGMENTS\n");
    for tier in tiers {
        let (width,height,rate)=playback::video_bitrate(&app,tier).unwrap();
        let scale=format!("{hdr}scale=w='min({width},iw)':h='min({height},ih)':force_original_aspect_ratio=decrease:force_divisible_by=2");
        let prefix=if adaptive {format!("v{tier}-")}else{String::new()};
        let playlist=if adaptive {format!("v{tier}.m3u8")}else{"index.m3u8".into()};
        command.args(["-map","0:v:0","-map",&audio_map])
            .args(playback::encoder_options(&encoder))
            .args(["-pix_fmt","yuv420p","-vf",&scale,"-b:v",&rate,"-maxrate",&rate,"-bufsize","12000k",
                   "-force_key_frames","expr:gte(t,n_forced*2)","-sc_threshold","0",
                   "-c:a","aac","-ac","2","-b:a","160k","-f","hls","-hls_time","2","-hls_list_size","10",
                   "-hls_delete_threshold","3","-hls_flags","delete_segments+temp_file+independent_segments","-hls_segment_filename"])
            .arg(dir.join(format!("{prefix}segment%06d.ts"))).arg(dir.join(&playlist));
        let bandwidth=rate.trim_end_matches('k').parse::<u32>().unwrap_or(6000)*1200+192_000;
        master.push_str(&format!("#EXT-X-STREAM-INF:BANDWIDTH={bandwidth}\n{playlist}\n"));
    }
    if adaptive && tokio::fs::write(dir.join("master.m3u8"),master).await.is_err() {
        let _=tokio::fs::remove_dir_all(&dir).await;
        return error(StatusCode::INTERNAL_SERVER_ERROR,"Cannot create adaptive playlist");
    }
    command.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).kill_on_drop(true);
    let mut child = match command.spawn() {
        Ok(c) => c,
        Err(_) => {
            let _ = tokio::fs::remove_dir_all(&dir).await;
            return error(StatusCode::SERVICE_UNAVAILABLE, "FFmpeg is unavailable");
        }
    };
    let (tx, rx) = oneshot::channel();
    let (done, finished) = oneshot::channel();
    let status = Arc::new(AtomicU8::new(0));
    let worker_status = status.clone();
    SESSIONS.lock().unwrap().insert(id.clone(), Session {
        profile_id: actor.profile_id, finished, dir:dir.clone(), last:Instant::now(), stop:Some(tx), status,
    });
    tokio::spawn(async move {
        // Retain the slot and files for completed short videos until stop/expiry.
        let mut rx=rx;
        tokio::select! {
            result=child.wait()=>{worker_status.store(if result.is_ok_and(|s|s.success()){1}else{2},Ordering::Release); let _=rx.await;},
            _=&mut rx=>{let _=child.kill().await;worker_status.store(2,Ordering::Release);}
        }
        let _=child.wait().await;
        let _=tokio::fs::remove_dir_all(&dir).await;
        directory.0.take();
        drop(permit);
        let _=done.send(());
    });
    start_reaper();
    Json(json!({"id":id,"adaptive":adaptive,"url":format!("/api/playback/hls/{id}/{}",if adaptive {"master.m3u8"}else{"index.m3u8"})})).into_response()
}
pub async fn file(axum::Extension(actor): axum::Extension<crate::auth::Principal>, Path((id, name)): Path<(String, String)>) -> Response {
    if !allowed_name(&name) {
        return error(StatusCode::NOT_FOUND, "Not found");
    }
    let (dir, status) = {
        let mut sessions = SESSIONS.lock().unwrap();
        let Some(session) = sessions.get_mut(&id) else {
            return error(StatusCode::NOT_FOUND, "Playback session expired");
        };
        if session.profile_id!=actor.profile_id {return error(StatusCode::NOT_FOUND,"Playback session expired");}
        session.last = Instant::now();
        (session.dir.clone(), session.status.clone())
    };
    let path = dir.join(&name);
    if name.ends_with(".m3u8") {
        for _ in 0..600 {
            match tokio::fs::read(&path).await {
                Ok(content) if name=="master.m3u8" || playlist_ready(&content) => {
                    return (
                        [
                            (header::CONTENT_TYPE, "application/vnd.apple.mpegurl"),
                            (header::CACHE_CONTROL, "no-store"),
                        ],
                        content,
                    )
                        .into_response()
                }
                Ok(_) | Err(_) if status.load(Ordering::Acquire) != 0 => {
                    return error(StatusCode::BAD_GATEWAY, "Could not start conversion")
                }
                _ => tokio::time::sleep(Duration::from_millis(200)).await,
            }
        }
        return error(
            StatusCode::GATEWAY_TIMEOUT,
            "Video is taking too long to load",
        );
    }
    match tokio::fs::File::open(path).await {
        Ok(f) => Response::builder()
            .header(header::CONTENT_TYPE, "video/mp2t")
            .header(header::CACHE_CONTROL, "no-store")
            .body(Body::from_stream(ReaderStream::new(f)))
            .unwrap(),
        Err(_) => error(StatusCode::NOT_FOUND, "Video segment unavailable"),
    }
}

fn playlist_ready(content: &[u8]) -> bool {
    let text = String::from_utf8_lossy(content);
    text.contains("#EXT-X-ENDLIST")
        || text
            .lines()
            .filter(|line| line.starts_with("#EXTINF:"))
            .count()
            >= 3
}

fn allowed_name(name: &str) -> bool {
    if matches!(name,"index.m3u8"|"master.m3u8"|"v480.m3u8"|"v720.m3u8"|"v1080.m3u8") { return true; }
    let segment=name.strip_prefix("v480-").or_else(||name.strip_prefix("v720-")).or_else(||name.strip_prefix("v1080-")).unwrap_or(name);
    segment.strip_prefix("segment").and_then(|s|s.strip_suffix(".ts")).is_some_and(|digits|digits.len()==6&&digits.bytes().all(|b|b.is_ascii_digit()))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn waits_for_playable_live_playlist_or_finished_short_video() {
        assert!(!playlist_ready(b"#EXTM3U\n#EXTINF:2,\nsegment000000.ts\n"));
        assert!(playlist_ready(
            b"#EXTM3U\n#EXTINF:2,\na.ts\n#EXTINF:2,\nb.ts\n#EXTINF:2,\nc.ts\n"
        ));
        assert!(playlist_ready(
            b"#EXTM3U\n#EXTINF:1,\na.ts\n#EXT-X-ENDLIST\n"
        ));
    }
    #[test]
    fn only_playlist_and_numbered_segments_are_accessible() {
        assert!(!allowed_name("segmentaaaaé.ts"));
        assert!(allowed_name("v480-segment000001.ts"));
        assert!(allowed_name("index.m3u8"));
        assert!(allowed_name("segment000123.ts"));
        assert!(!allowed_name("../index.m3u8"));
        assert!(!allowed_name("segment00012a.ts"));
        assert!(!allowed_name("segment000123.ts.tmp"));
    }
}
