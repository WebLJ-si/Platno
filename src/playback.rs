use crate::{db, App};
use axum::{
    body::Body,
    extract::{Query, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{path::PathBuf, process::Stdio, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncSeekExt},
    process::Command,
};
use tokio_util::io::ReaderStream;
use std::sync::{Arc, LazyLock};
pub(crate) static CONVERSION_LIMIT: LazyLock<Arc<tokio::sync::Semaphore>> = LazyLock::new(|| Arc::new(tokio::sync::Semaphore::new(4)));
static DIRECT_LIMIT: LazyLock<Arc<tokio::sync::Semaphore>> = LazyLock::new(|| Arc::new(tokio::sync::Semaphore::new(64)));
static PROBE_LIMIT: LazyLock<Arc<tokio::sync::Semaphore>> = LazyLock::new(|| Arc::new(tokio::sync::Semaphore::new(8)));
static SUBTITLE_LIMIT: LazyLock<Arc<tokio::sync::Semaphore>> = LazyLock::new(|| Arc::new(tokio::sync::Semaphore::new(4)));

#[derive(Deserialize)]
pub struct MediaQuery {
    path: String,
    start: Option<f64>,
    quality: Option<String>,
    subtitle: Option<String>,
    audio: Option<u32>,
}

pub(crate) fn library_path(app: &App, path: &str) -> Option<PathBuf> {
    let present: bool = db(app)
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM files WHERE path=?1)",
            [path],
            |r| r.get(0),
        )
        .unwrap_or(false);
    if present {
        Some(PathBuf::from(path))
    } else {
        None
    }
}
fn fail(status: StatusCode, message: &str) -> Response {
    (status, Json(json!({"error":message}))).into_response()
}
pub(crate) fn video_bitrate(app: &App, quality: &str) -> Option<(u32, u32, String)> {
    let (width, height, base_rate) = match quality {
        "480" => (854, 480, 1200u32),
        "720" => (1280, 720, 3000u32),
        "1080" => (1920, 1080, 6000u32),
        _ => return None,
    };
    let balance: String = db(app)
        .query_row("SELECT value FROM server_settings WHERE key='transcode_quality'", [], |r| r.get(0))
        .unwrap_or_else(|_| "balanced".into());
    let multiplier = match balance.as_str() { "speed" => 0.7, "quality" => 1.5, _ => 1.0 };
    Some((width, height, format!("{}k", (base_rate as f64 * multiplier) as u32)))
}
pub(crate) fn binary(name: &str) -> String {
    let var = if name == "ffmpeg" {
        "SMART_MEDIA_FFMPEG"
    } else {
        "SMART_MEDIA_FFPROBE"
    };
    std::env::var(var)
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| {
            for dir in ["/opt/homebrew/bin", "/usr/local/bin"] {
                let path = PathBuf::from(dir).join(name);
                if path.is_file() {
                    return path.to_string_lossy().into_owned();
                }
            }
            name.to_owned()
        })
}
pub async fn available_h264_encoders() -> Vec<&'static str> {
    let Ok(output)=crate::processes::output(Command::new(binary("ffmpeg")).args(["-hide_banner","-encoders"]),Duration::from_secs(5),1_000_000).await else{return vec!["libx264"]};
    let text=String::from_utf8_lossy(&output.stdout).to_string()+&String::from_utf8_lossy(&output.stderr);
    ["libx264","h264_videotoolbox","h264_nvenc","h264_qsv"].into_iter().filter(|name|text.lines().any(|line|line.split_whitespace().any(|word|word==*name))).collect()
}
async fn encoder_works(encoder: &str) -> bool {
    let mut cmd=Command::new(binary("ffmpeg"));
    cmd.args(["-v","error","-nostdin","-f","lavfi","-i","color=c=black:s=64x64:d=0.04","-frames:v","1","-an","-c:v",encoder]);
    match encoder { "h264_nvenc"=>{cmd.args(["-preset","p4"]);},"h264_qsv"=>{cmd.args(["-preset","veryfast"]);},"h264_videotoolbox"=>{cmd.args(["-realtime","1","-allow_sw","1"]);},_=>{cmd.args(["-preset","ultrafast"]);} }
    cmd.args(["-f","null","-"]);
    matches!(crate::processes::output(&mut cmd,Duration::from_secs(4),100_000).await,Ok(result) if result.status.success())
}
pub async fn h264_encoder(app: &App) -> (String, Vec<&'static str>) {
    let available=available_h264_encoders().await;
    let requested:String=db(app).query_row("SELECT value FROM server_settings WHERE key='hardware_encoder'",[],|r|r.get(0)).unwrap_or_else(|_|"auto".to_string());
    let selected=if requested=="auto" {"libx264"} else {requested.as_str()};
    let encoder=if available.contains(&selected) && encoder_works(selected).await {selected.to_string()} else {"libx264".to_string()};
    (encoder,available)
}
pub fn encoder_options(encoder: &str) -> Vec<String> {
    let mut args=vec!["-c:v".to_string(),encoder.to_string()];
    match encoder {"h264_nvenc"=>args.extend(["-preset".into(),"p4".into(),"-tune".into(),"ll".into()]),"h264_qsv"=>args.extend(["-preset".into(),"veryfast".into()]),"h264_videotoolbox"=>args.extend(["-realtime".into(),"1".into(),"-allow_sw".into(),"1".into()]),_=>args.extend(["-preset".into(),"veryfast".into(),"-tune".into(),"zerolatency".into()])}
    args
}
const DRIVE_OPEN_TIMEOUT: Duration = Duration::from_secs(120);
const PROBE_TIMEOUT: Duration = Duration::from_secs(180);
async fn probe_file(path: &std::path::Path) -> Result<Value, String> {
    probe_file_with(path, &binary("ffprobe")).await
}
async fn probe_file_with(path: &std::path::Path, ffprobe: &str) -> Result<Value, String> {
    let _permit=PROBE_LIMIT.clone().try_acquire_owned().map_err(|_| "Media information slots are busy. Try again shortly.".to_string())?;
    match tokio::time::timeout(DRIVE_OPEN_TIMEOUT, tokio::fs::File::open(path)).await {
        Err(_) => {
            return Err(
                "The video drive is not responding. Check the connection and try again.".into(),
            )
        }
        Ok(Err(e)) if e.kind() == std::io::ErrorKind::NotFound => return Err(
            "The video file is unavailable. Reconnect its drive or network share, then try again."
                .into(),
        ),
        Ok(Err(e)) if e.kind() == std::io::ErrorKind::PermissionDenied => {
            return Err("The server does not have permission to read this video file.".into())
        }
        Ok(Err(_)) => {
            return Err(
                "Could not open this video. Check the drive connection and file permissions."
                    .into(),
            )
        }
        Ok(Ok(_)) => {}
    }
    let output = crate::processes::output(
        Command::new(ffprobe)
            .args([
                "-v",
                "error",
                "-show_entries",
                "format=duration,format_name,bit_rate:stream=index,codec_type,codec_name,channels,channel_layout,width,height,color_transfer:stream_tags=language,title",
                "-of",
                "json",
            ])
            .arg(path), PROBE_TIMEOUT, 2_000_000).await.map_err(|e| {
            if e.kind()==std::io::ErrorKind::TimedOut {"Reading video information timed out. The drive may be slow or disconnected".to_string()}
            else {"ffprobe is unavailable or returned excessive output; install FFmpeg and ffprobe".to_string()}
        })?;
    if !output.status.success() {
        return Err("Could not read this video; check that the drive is available".into());
    }
    serde_json::from_slice(&output.stdout).map_err(|_| "Invalid media information".into())
}
pub async fn info(State(app): State<App>, Query(q): Query<MediaQuery>) -> Response {
    let Some(path) = library_path(&app, &q.path) else {
        return fail(StatusCode::NOT_FOUND, "File is not in the library");
    };
    let signature =
        match tokio::time::timeout(Duration::from_secs(2), tokio::fs::metadata(&path)).await {
            Ok(Ok(m)) => Some(format!(
                "{}:{}",
                m.len(),
                m.modified()
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|t| t.as_nanos())
                    .unwrap_or(0)
            )),
            _ => None,
        };
    let cache_key = signature.as_ref().map(|_| format!("playback:probe:v4:{}", q.path));
    let cached_data = cache_key
        .as_ref()
        .and_then(|key| {
            db(&app)
                .query_row("SELECT value FROM cache WHERE key=?1", [key], |row| {
                    row.get::<_, String>(0)
                })
                .ok()
        })
        .and_then(|raw| serde_json::from_str::<Value>(&raw).ok())
        .filter(|value| value["signature"].as_str()==signature.as_deref())
        .map(|value| value["probe"].clone());
    let probed = if let Some(data) = cached_data {
        Ok(data)
    } else {
        let result = probe_file(&path).await;
        if let (Some(key), Ok(data)) = (&cache_key, &result) {
            let _ = db(&app).execute(
                "INSERT OR REPLACE INTO cache VALUES(?1,?2,?3)",
                rusqlite::params![key, json!({"signature":signature,"probe":data}).to_string(), chrono::Utc::now().timestamp()],
            );
        }
        result
    };
    match probed {
        Err(e) => fail(StatusCode::BAD_GATEWAY, &e),
        Ok(data) => {
            let streams = data["streams"].as_array().cloned().unwrap_or_default();
            let video = streams.iter().find(|x| x["codec_type"] == "video");
            if video.is_none() {
                return fail(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "No video stream was found",
                );
            }
            let audio = streams.iter().find(|x| x["codec_type"] == "audio");
            let format = data["format"]["format_name"].as_str().unwrap_or("");
            let hdr = video.is_some_and(|x|matches!(x["color_transfer"].as_str(),Some("smpte2084"|"arib-std-b67")));
            let direct = !hdr && format.split(',').any(|s| s == "mp4" || s == "mov")
                && video.is_some_and(|x| x["codec_name"] == "h264")
                && audio.is_none_or(|x| x["codec_name"] == "aac");
            let duration = data["format"]["duration"]
                .as_str()
                .and_then(|x| x.parse::<f64>().ok())
                .filter(|d| d.is_finite() && *d > 0.0);
            let audio_tracks: Vec<Value> = streams.iter().filter(|x| x["codec_type"] == "audio").enumerate().map(|(i, x)| json!({
                "id":i,"language":x["tags"]["language"],"codec":x["codec_name"],"channels":x["channels"],
                "label":format!("{} · {} · {} ch{}", x["tags"]["language"].as_str().unwrap_or("Unknown language"), x["codec_name"].as_str().unwrap_or("audio").to_uppercase(), x["channels"].as_u64().unwrap_or(0), x["tags"]["title"].as_str().map(|t|format!(" · {t}")).unwrap_or_default())
            })).collect();
            let mut subtitles = Vec::new();
            for sub in streams.iter().filter(|x| {
                x["codec_type"] == "subtitle"
                    && matches!(
                        x["codec_name"].as_str(),
                        Some("subrip" | "ass" | "ssa" | "webvtt" | "mov_text")
                    )
            }) {
                if let Some(index) = sub["index"].as_u64() {
                    subtitles.push(json!({"id":format!("embedded:{index}"),"language":sub["tags"]["language"],"label":format!("{} · {}",sub["tags"]["language"].as_str().unwrap_or("Unknown language"),sub["tags"]["title"].as_str().unwrap_or("Embedded subtitle"))}));
                }
            }
            for (i, name) in local_subtitles(&app, &q.path).iter().enumerate() {
                if is_text_subtitle(name) {
                    subtitles.push(json!({"id":format!("file:{i}"),"label":name}));
                }
            }
            Json(json!({"duration":duration,"direct":direct,"video_codec":video.map(|x|x["codec_name"].clone()),"audio_codec":audio.map(|x|x["codec_name"].clone()),"subtitles":subtitles,"audio_tracks":audio_tracks,"hdr":hdr,"bit_rate":data["format"]["bit_rate"]})).into_response()
        }
    }
}
fn parse_range(input: &str, size: u64) -> Option<(u64, u64)> {
    if size == 0 || !input.starts_with("bytes=") || input.contains(',') {
        return None;
    }
    let (a, b) = input[6..].split_once('-')?;
    if a.is_empty() {
        let n = b.parse::<u64>().ok()?;
        if n == 0 {
            return None;
        }
        return Some((size.saturating_sub(n), size - 1));
    }
    let start = a.parse::<u64>().ok()?;
    if start >= size {
        return None;
    }
    let end = if b.is_empty() {
        size - 1
    } else {
        b.parse::<u64>().ok()?.min(size - 1)
    };
    if end < start {
        return None;
    }
    Some((start, end))
}
pub async fn direct(
    State(app): State<App>,
    Query(q): Query<MediaQuery>,
    headers: HeaderMap,
) -> Response {
    let Some(path) = library_path(&app, &q.path) else {
        return fail(StatusCode::NOT_FOUND, "File is not in the library");
    };
    let Ok(permit)=DIRECT_LIMIT.clone().try_acquire_owned() else {return fail(StatusCode::TOO_MANY_REQUESTS,"File streaming slots are busy. Try again shortly.");};
    let mut file=match tokio::time::timeout(DRIVE_OPEN_TIMEOUT,tokio::fs::File::open(path)).await {
        Ok(Ok(file))=>file,
        Err(_)=>return fail(StatusCode::GATEWAY_TIMEOUT,"The video drive is not responding"),
        _=>return fail(StatusCode::NOT_FOUND,"The video file is unavailable"),
    };
    let size = match file.metadata().await {
        Ok(v) => v.len(),
        Err(_) => return fail(StatusCode::NOT_FOUND, "The video file is unavailable"),
    };
    let range = headers.get(header::RANGE).and_then(|v| v.to_str().ok());
    let (start, end) = if let Some(r) = range {
        match parse_range(r, size) {
            Some(v) => v,
            None => {
                return Response::builder()
                    .status(StatusCode::RANGE_NOT_SATISFIABLE)
                    .header(header::CONTENT_RANGE, format!("bytes */{size}"))
                    .body(Body::empty())
                    .unwrap()
            }
        }
    } else if size > 0 {
        (0, size - 1)
    } else {
        return fail(StatusCode::UNPROCESSABLE_ENTITY, "The video is empty");
    };
    if file.seek(std::io::SeekFrom::Start(start)).await.is_err() {
        return fail(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Could not seek in this file",
        );
    }
    let mut response = Response::builder()
        .status(if range.is_some() {
            StatusCode::PARTIAL_CONTENT
        } else {
            StatusCode::OK
        })
        .header(header::CONTENT_TYPE, "video/mp4")
        .header(header::ACCEPT_RANGES, "bytes")
        .header(header::CONTENT_LENGTH, end - start + 1)
        .header(header::CACHE_CONTROL, "no-store");
    if range.is_some() {
        response = response.header(header::CONTENT_RANGE, format!("bytes {start}-{end}/{size}"));
    }
    let stream=async_stream::stream! {let _permit=permit;let mut reader=ReaderStream::new(file.take(end-start+1));use futures_util::StreamExt;while let Some(item)=reader.next().await {yield item;}};
    response.body(Body::from_stream(stream)).unwrap()
}
pub(crate) async fn hdr_filter(path: &std::path::Path) -> Result<String, String> {
    let data=probe_file(path).await?;
    let hdr=data["streams"].as_array().is_some_and(|s|s.iter().any(|x|x["codec_type"]=="video"&&matches!(x["color_transfer"].as_str(),Some("smpte2084"|"arib-std-b67"))));
    Ok(if hdr {"zscale=t=linear:npl=100,format=gbrpf32le,zscale=p=bt709,tonemap=tonemap=mobius:desat=0,zscale=t=bt709:m=bt709:r=tv,format=yuv420p,".into()} else {String::new()})
}
pub(crate) async fn audio_map(path: &std::path::Path, audio: Option<u32>) -> Result<String, String> {
    let Some(index) = audio else { return Ok("0:a:0?".into()); };
    let data = probe_file(path).await?;
    let count = data["streams"].as_array().map(|s|s.iter().filter(|x|x["codec_type"]=="audio").count()).unwrap_or(0);
    if index as usize >= count { return Err("This audio track is unavailable".into()); }
    Ok(format!("0:a:{index}"))
}
pub async fn transcode(State(app): State<App>, Query(q): Query<MediaQuery>) -> Response {
    let Some(path) = library_path(&app, &q.path) else {
        return fail(StatusCode::NOT_FOUND, "File is not in the library");
    };
    let start = q.start.unwrap_or(0.0);
    if !start.is_finite() || !(0.0..=1_000_000.0).contains(&start) {
        return fail(StatusCode::BAD_REQUEST, "Invalid playback position");
    }
    let Ok(permit)=CONVERSION_LIMIT.clone().try_acquire_owned() else {return fail(StatusCode::TOO_MANY_REQUESTS,"All four conversion slots are busy. Try again shortly.");};
    if tokio::fs::metadata(&path).await.is_err() {
        return fail(StatusCode::NOT_FOUND, "The video file is unavailable");
    }
    let Some((width, height, rate)) = video_bitrate(&app, q.quality.as_deref().unwrap_or("1080")) else {
        return fail(StatusCode::BAD_REQUEST, "Invalid quality");
    };
    let hdr=match hdr_filter(&path).await {Ok(v)=>v,Err(e)=>return fail(StatusCode::BAD_GATEWAY,&e)};
    let scale=format!("{hdr}scale=w='min({width},iw)':h='min({height},ih)':force_original_aspect_ratio=decrease:force_divisible_by=2");
    let audio_map = match audio_map(&path, q.audio).await { Ok(v)=>v, Err(e)=>return fail(StatusCode::BAD_REQUEST,&e) };
    let (encoder,_available)=h264_encoder(&app).await;
    let mut cmd = Command::new(binary("ffmpeg"));
    cmd.args([
        "-v",
        "error",
        "-nostdin",
        "-ss",
        &format!("{start:.3}"),
        "-i",
    ])
    .arg(path)
    .args([
        "-map",
        "0:v:0",
        "-map",
        &audio_map,
    ])
    .args(encoder_options(&encoder))
    .args([
        "-pix_fmt",
        "yuv420p",
        "-vf",
        &scale,
        "-b:v",
        &rate,
        "-maxrate",
        &rate,
        "-bufsize",
        "12000k",
        "-g",
        "24",
        "-c:a",
        "aac",
        "-b:a",
        "160k",
        "-movflags",
        "frag_keyframe+empty_moov+default_base_moof",
        "-f",
        "mp4",
        "pipe:1",
    ])
    .stdout(Stdio::piped())
    .stderr(Stdio::null())
    .kill_on_drop(true);
    let mut child = match cmd.spawn() {
        Ok(v) => v,
        Err(_) => {
            return fail(
                StatusCode::SERVICE_UNAVAILABLE,
                "FFmpeg is unavailable; install FFmpeg",
            )
        }
    };
    let stdout = child.stdout.take().unwrap();
    let managed=crate::processes::ManagedChild::new(child, Some(permit));
    let stream = async_stream::stream! { let _child=managed; let mut reader=ReaderStream::new(stdout); use futures_util::StreamExt; while let Some(item)=reader.next().await { yield item; } };
    let mut response = Response::new(Body::from_stream(stream));
    response
        .headers_mut()
        .insert(header::CONTENT_TYPE, HeaderValue::from_static("video/mp4"));
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

fn local_subtitles(app: &App, path: &str) -> Vec<String> {
    let raw: String = db(app)
        .query_row("SELECT result FROM files WHERE path=?1", [path], |r| {
            r.get(0)
        })
        .unwrap_or_default();
    let value: Value = serde_json::from_str(&raw).unwrap_or_default();
    value["subtitles"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .filter(|name| PathBuf::from(name).file_name().is_some_and(|n| n == *name))
        .map(str::to_string)
        .collect()
}
fn is_text_subtitle(name: &str) -> bool {
    PathBuf::from(name)
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| {
            ["srt", "vtt", "ass", "ssa"]
                .iter()
                .any(|x| e.eq_ignore_ascii_case(x))
        })
}
pub async fn subtitle(State(app): State<App>, Query(q): Query<MediaQuery>) -> Response {
    let Ok(permit)=SUBTITLE_LIMIT.clone().try_acquire_owned() else {return fail(StatusCode::TOO_MANY_REQUESTS,"Subtitle conversion slots are busy. Try again shortly.");};
    let Some(path) = library_path(&app, &q.path) else {
        return fail(StatusCode::NOT_FOUND, "File is not in the library");
    };
    let start = q.start.unwrap_or(0.0);
    if !start.is_finite() || !(0.0..=1_000_000.0).contains(&start) {
        return fail(StatusCode::BAD_REQUEST, "Invalid playback position");
    }
    let id = q.subtitle.as_deref().unwrap_or("");
    let mut cmd = Command::new(binary("ffmpeg"));
    cmd.args(["-v", "error", "-nostdin"]);
    let external = id.starts_with("file:");
    if !external {
        cmd.args(["-ss", &format!("{start:.3}")]);
    }
    cmd.arg("-i");
    if let Some(index) = id
        .strip_prefix("embedded:")
        .and_then(|s| s.parse::<u32>().ok())
    {
        let info = match probe_file(&path).await {
            Ok(v) => v,
            Err(_) => return fail(StatusCode::BAD_GATEWAY, "Cannot read subtitle tracks"),
        };
        let valid = info["streams"].as_array().is_some_and(|streams| {
            streams.iter().any(|s| {
                s["index"].as_u64() == Some(index as u64)
                    && s["codec_type"] == "subtitle"
                    && matches!(
                        s["codec_name"].as_str(),
                        Some("subrip" | "ass" | "ssa" | "webvtt" | "mov_text")
                    )
            })
        });
        if !valid {
            return fail(StatusCode::BAD_REQUEST, "Subtitle track unavailable");
        }
        cmd.arg(&path).args(["-map", &format!("0:{index}")]);
    } else if let Some(index) = id
        .strip_prefix("file:")
        .and_then(|s| s.parse::<usize>().ok())
    {
        let names = local_subtitles(&app, &q.path);
        let Some(name) = names.get(index).filter(|name| is_text_subtitle(name)) else {
            return fail(StatusCode::BAD_REQUEST, "Subtitle file unavailable");
        };
        let Some(parent) = path.parent() else {
            return fail(StatusCode::BAD_REQUEST, "Subtitle file unavailable");
        };
        cmd.arg(parent.join(name));
    } else {
        return fail(StatusCode::BAD_REQUEST, "Invalid subtitle selection");
    }
    if !external {
        cmd.args(["-flush_packets", "1", "-f", "webvtt", "pipe:1"])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true);
        let mut child = match cmd.spawn() {
            Ok(c) => c,
            Err(_) => return fail(StatusCode::SERVICE_UNAVAILABLE, "FFmpeg is unavailable"),
        };
        let stdout = child.stdout.take().unwrap();
        let managed=crate::processes::ManagedChild::new(child, Some(permit));
        let stream = async_stream::stream! { let _child=managed; let mut reader=ReaderStream::new(stdout); use futures_util::StreamExt; while let Some(item)=reader.next().await {yield item;} };
        return Response::builder()
            .header(header::CONTENT_TYPE, "text/vtt; charset=utf-8")
            .header(header::CACHE_CONTROL, "no-store")
            .body(Body::from_stream(stream))
            .unwrap();
    }
    let output = crate::processes::output(cmd.args(["-f", "webvtt", "pipe:1"]), Duration::from_secs(30), 5_000_000).await;
    match output {
        Ok(data) if data.status.success() && data.stdout.len() <= 5_000_000 => (
            [
                (header::CONTENT_TYPE, "text/vtt; charset=utf-8"),
                (header::CACHE_CONTROL, "no-store"),
            ],
            if external && start > 0.0 {
                shift_webvtt(&data.stdout, start)
            } else {
                data.stdout
            },
        )
            .into_response(),
        _ => fail(StatusCode::BAD_GATEWAY, "Could not load this subtitle"),
    }
}
fn shift_webvtt(data: &[u8], offset: f64) -> Vec<u8> {
    fn seconds(value: &str) -> Option<f64> {
        let parts: Vec<_> = value.split(':').collect();
        match parts.as_slice() {
            [m, s] => Some(m.parse::<f64>().ok()? * 60.0 + s.parse::<f64>().ok()?),
            [h, m, s] => Some(
                h.parse::<f64>().ok()? * 3600.0
                    + m.parse::<f64>().ok()? * 60.0
                    + s.parse::<f64>().ok()?,
            ),
            _ => None,
        }
    }
    fn stamp(value: f64) -> String {
        let ms = (value.max(0.0) * 1000.0).round() as u64;
        format!(
            "{:02}:{:02}:{:02}.{:03}",
            ms / 3_600_000,
            (ms / 60_000) % 60,
            (ms / 1000) % 60,
            ms % 1000
        )
    }
    let text = String::from_utf8_lossy(data).replace("\r\n", "\n");
    let mut result = Vec::new();
    for block in text.split("\n\n") {
        let mut lines: Vec<String> = block.lines().map(str::to_owned).collect();
        if let Some(i) = lines.iter().position(|line| line.contains(" --> ")) {
            let Some((left, right)) = lines[i].split_once(" --> ") else {
                continue;
            };
            let end = right.split_whitespace().next().unwrap_or("");
            if let (Some(a), Some(b)) = (seconds(left), seconds(end)) {
                if b <= offset {
                    continue;
                }
                let settings = &right[end.len()..];
                lines[i] = format!(
                    "{} --> {}{}",
                    stamp(a - offset),
                    stamp(b - offset),
                    settings
                );
            }
        }
        result.push(lines.join("\n"));
    }
    result.join("\n\n").into_bytes()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn missing_file_explains_how_to_reconnect() {
        let directory = tempfile::tempdir().unwrap();
        let error = probe_file(&directory.path().join("missing.mkv"))
            .await
            .unwrap_err();
        assert!(error.contains("Reconnect"));
    }
    #[cfg(unix)]
    #[tokio::test]
    async fn slow_drive_open_waits_beyond_old_eight_second_limit() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let fifo = dir.path().join("slow-video");
        assert!(std::process::Command::new("mkfifo")
            .arg(&fifo)
            .status()
            .unwrap()
            .success());
        let mock = dir.path().join("mock-ffprobe");
        std::fs::write(&mock, "#!/bin/sh\nprintf '{\"streams\":[]}'\n").unwrap();
        std::fs::set_permissions(&mock, std::fs::Permissions::from_mode(0o700)).unwrap();
        let writer_path = fifo.clone();
        let writer = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(9)).await;
            tokio::task::spawn_blocking(move || {
                std::fs::OpenOptions::new().write(true).open(writer_path)
            })
            .await
            .unwrap()
            .unwrap();
        });
        let result = probe_file_with(&fifo, mock.to_str().unwrap()).await;
        writer.await.unwrap();
        assert!(result.is_ok(), "slow open should succeed: {result:?}");
    }
    #[test]
    fn subtitle_seek_preserves_correct_timing() {
        let source =
            b"WEBVTT\n\n00:02.000 --> 00:06.000\nHello\n\n00:08.000 --> 00:09.000\nLater\n";
        let output = String::from_utf8(shift_webvtt(source, 3.0)).unwrap();
        assert!(output.contains("00:00:00.000 --> 00:00:03.000"));
        assert!(output.contains("00:00:05.000 --> 00:00:06.000"));
    }
    #[test]
    fn byte_ranges_reject_malformed_overflow_and_empty_files() {
        for range in ["", "bytes=", "bytes=-0", "bytes=5-1", "bytes=18446744073709551616-", "bytes=0-1,3-4", "items=0-1"] {
            assert_eq!(parse_range(range,10),None,"{range}");
        }
        assert_eq!(parse_range("bytes=0-",0),None);
        assert_eq!(parse_range("bytes=-99",10),Some((0,9)));
        assert_eq!(parse_range("bytes=8-99",10),Some((8,9)));
    }
    #[test]
    fn subtitle_shift_drops_expired_cues_and_preserves_settings_and_unicode() {
        let source="WEBVTT\n\n00:00:01.000 --> 00:00:02.000\nExpired\n\ncue-2\n00:00:04.000 --> 00:00:07.500 line:80%\nŽivjo čšž\n";
        let output=String::from_utf8(shift_webvtt(source.as_bytes(),3.0)).unwrap();
        assert!(output.starts_with("WEBVTT"));
        assert!(!output.contains("Expired"));
        assert!(output.contains("00:00:01.000 --> 00:00:04.500 line:80%"));
        assert!(output.contains("cue-2"));
        assert!(output.contains("Živjo čšž"));
    }
    #[cfg(unix)]
    #[tokio::test]
    async fn invalid_probe_output_and_missing_executable_are_actionable() {
        use std::os::unix::fs::PermissionsExt;
        let directory=tempfile::tempdir().unwrap();let media=directory.path().join("video");std::fs::write(&media,b"fixture").unwrap();
        let missing=probe_file_with(&media,"/definitely/missing/ffprobe").await.unwrap_err();
        assert!(missing.contains("ffprobe is unavailable"));
        let mock=directory.path().join("probe");std::fs::write(&mock,"#!/bin/sh\nprintf 'invalid json'\n").unwrap();std::fs::set_permissions(&mock,std::fs::Permissions::from_mode(0o700)).unwrap();
        assert!(probe_file_with(&media,mock.to_str().unwrap()).await.unwrap_err().contains("Invalid media information"));
    }
    #[test]
    fn byte_ranges() {
        assert_eq!(parse_range("bytes=2-4", 10), Some((2, 4)));
        assert_eq!(parse_range("bytes=7-", 10), Some((7, 9)));
        assert_eq!(parse_range("bytes=-3", 10), Some((7, 9)));
        assert_eq!(parse_range("bytes=10-", 10), None);
        assert_eq!(parse_range("bytes=0-1,3-4", 10), None);
    }
}
