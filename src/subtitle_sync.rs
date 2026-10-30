//! Local, bounded speech-to-subtitle alignment. Never changes source media/subtitles.
use crate::{auth::Principal, db, playback, App};
use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    Extension, Json,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::{Arc, LazyLock, Mutex},
    time::Duration,
};
use tokio::process::Command;
use unicode_normalization::UnicodeNormalization;
static SYNC_LIMIT: LazyLock<Arc<tokio::sync::Semaphore>> =
    LazyLock::new(|| Arc::new(tokio::sync::Semaphore::new(1)));
type ActiveSync = Option<(i64, String, tokio::sync::oneshot::Sender<()>)>;
static ACTIVE_SYNC: LazyLock<Mutex<ActiveSync>> = LazyLock::new(|| Mutex::new(None));
struct ActiveSyncGuard;
impl Drop for ActiveSyncGuard {
    fn drop(&mut self) {
        ACTIVE_SYNC.lock().unwrap().take();
    }
}
#[derive(Deserialize)]
pub struct CancelRequest {
    request_id: String,
}
pub async fn cancel(
    Extension(actor): Extension<Principal>,
    Json(input): Json<CancelRequest>,
) -> Json<Value> {
    let mut active = ACTIVE_SYNC.lock().unwrap();
    let owned = active
        .as_ref()
        .is_some_and(|(profile, id, _)| *profile == actor.profile_id && *id == input.request_id);
    if owned {
        if let Some((_, _, sender)) = active.take() {
            let _ = sender.send(());
        }
    }
    Json(json!({"cancelled":owned}))
}
#[derive(Deserialize)]
pub struct SyncRequest {
    request_id: String,
    path: String,
    subtitle: String,
    audio: Option<u32>,
    position: Option<f64>,
    #[serde(default)]
    refresh: bool,
}
#[derive(Clone, Debug)]
struct Cue {
    start: f64,
    end: f64,
    text: String,
}
#[derive(Clone, Debug)]
struct Anchor {
    subtitle: f64,
    audio: f64,
    score: f64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Alignment {
    offset: f64,
    scale: f64,
    confidence: f64,
    anchors: usize,
    samples: usize,
}
fn error(status: StatusCode, text: &str) -> Response {
    (status, Json(json!({"error":text}))).into_response()
}
fn runtime(app: &App) -> (PathBuf, PathBuf) {
    let executable = std::env::var_os("SMART_MEDIA_WHISPER")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            [
                "/usr/local/lib/platno/whisper-cli", // bundled in the Docker image
                "/usr/lib/platno/whisper-cli",
                "/opt/homebrew/bin/whisper-cli",
                "/usr/local/bin/whisper-cli",
            ]
            .iter()
            .map(PathBuf::from)
            .chain(
                std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()).map(|p| {
                    p.join(if cfg!(windows) {
                        "whisper-cli.exe"
                    } else {
                        "whisper-cli"
                    })
                }),
            )
            .find(|p| p.is_file())
            .unwrap_or_else(|| PathBuf::from("whisper-cli"))
        });
    let model = std::env::var_os("SMART_MEDIA_WHISPER_MODEL")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            let installed = app.db.parent().unwrap_or(Path::new(".")).join("models/ggml-base.bin");
            // A model the owner installed wins; otherwise use the one shipped inside the image or package.
            ["/usr/local/share/platno/ggml-base.bin", "/usr/share/platno/ggml-base.bin"]
                .iter()
                .map(PathBuf::from)
                .find(|p| !installed.is_file() && p.is_file())
                .unwrap_or(installed)
        });
    (executable, model)
}
pub fn status(app: &App) -> Value {
    let (cli, model) = runtime(app);
    json!({"available":cli.is_file()&&model.is_file(),"engine":"whisper.cpp","local":true})
}
fn seconds(stamp: &str) -> Option<f64> {
    let parts: Vec<_> = stamp
        .trim()
        .replace(',', ".")
        .split(':')
        .map(str::to_owned)
        .collect();
    let result = match parts.as_slice() {
        [m, s] => m.parse::<f64>().ok()? * 60. + s.parse::<f64>().ok()?,
        [h, m, s] => {
            h.parse::<f64>().ok()? * 3600. + m.parse::<f64>().ok()? * 60. + s.parse::<f64>().ok()?
        }
        _ => return None,
    };
    (result.is_finite() && result >= 0.).then_some(result)
}
fn parse_vtt(text: &str) -> Vec<Cue> {
    let tags = regex::Regex::new(r"<[^>]*>").unwrap();
    text.replace("\r\n", "\n")
        .split("\n\n")
        .filter_map(|block| {
            let mut lines = block.lines();
            let line = lines.find(|line| line.contains("-->"))?;
            let (a, b) = line.split_once("-->")?;
            let start = seconds(a)?;
            let end = seconds(b.split_whitespace().next()?)?;
            let text = tags
                .replace_all(&lines.collect::<Vec<_>>().join(" "), "")
                .chars()
                .take(512)
                .collect::<String>();
            (end > start && !text.trim().is_empty()).then_some(Cue { start, end, text })
        })
        .take(20_000)
        .collect()
}
fn words(text: &str) -> Vec<String> {
    text.nfkd()
        .filter(|c| !unicode_normalization::char::is_combining_mark(*c))
        .collect::<String>()
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|s| s.chars().count() > 1)
        .map(str::to_owned)
        .collect()
}
fn similarity(a: &[String], b: &[String]) -> f64 {
    if a.is_empty() || b.is_empty() {
        return 0.;
    }
    let mut counts = HashMap::new();
    for w in b {
        *counts.entry(w).or_insert(0usize) += 1
    }
    let mut shared = 0;
    for w in a {
        if let Some(n) = counts.get_mut(w) {
            if *n > 0 {
                shared += 1;
                *n -= 1
            }
        }
    }
    let overlap = 2. * shared as f64 / (a.len() + b.len()) as f64;
    0.7 * overlap + 0.3 * strsim::normalized_levenshtein(&a.join(" "), &b.join(" "))
}
fn anchors(cues: &[Cue], spoken: &[Cue]) -> Vec<Anchor> {
    let tokens: Vec<_> = cues.iter().map(|c| words(&c.text)).collect();
    let mut index: HashMap<&str, Vec<usize>> = HashMap::new();
    for (i, list) in tokens.iter().enumerate() {
        for token in list.iter().collect::<HashSet<_>>() {
            index.entry(token.as_str()).or_default().push(i)
        }
    }
    let mut out = Vec::new();
    for line in spoken {
        let text = words(&line.text);
        if text.len() < 4 || line.end <= line.start {
            continue;
        }
        let mut candidates = HashSet::new();
        for token in &text {
            if let Some(ids) = index.get(token.as_str()) {
                if ids.len() > 100 {
                    continue;
                }
                for id in ids {
                    for start in id.saturating_sub(2)..=*id {
                        candidates.insert(start);
                    }
                }
            }
        }
        let mut ranked = Vec::new();
        for i in candidates {
            if (cues[i].start - line.start).abs() > 300. {
                continue;
            }
            let mut joined = Vec::new();
            for n in 1..=3 {
                if i + n > cues.len() {
                    break;
                }
                joined.extend(tokens[i + n - 1].clone());
                let score = similarity(&text, &joined);
                if score >= 0.72 {
                    ranked.push((score, i));
                }
            }
        }
        ranked.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
        if let Some(&(score, i)) = ranked.first() {
            let competitor = ranked
                .iter()
                .find(|(_, j)| (cues[*j].start - cues[i].start).abs() > 3.)
                .map(|x| x.0)
                .unwrap_or(0.);
            if score - competitor >= 0.08
                && !out
                    .iter()
                    .any(|a: &Anchor| (a.subtitle - cues[i].start).abs() < 1.)
            {
                out.push(Anchor {
                    subtitle: cues[i].start,
                    audio: line.start,
                    score,
                })
            }
        }
    }
    out
}
fn median(values: &mut [f64]) -> f64 {
    values.sort_by(f64::total_cmp);
    values[values.len() / 2]
}
fn fit(anchors: &[Anchor], samples: usize) -> Result<Alignment, String> {
    if anchors.len() < 3 {
        return Err("Not enough matching speech. Try a dialogue scene; audio and subtitle text must be in the same language/release.".into());
    }
    let mut offsets: Vec<_> = anchors.iter().map(|a| a.audio - a.subtitle).collect();
    let offset = median(&mut offsets);
    let mut inliers: Vec<_> = anchors
        .iter()
        .filter(|a| (a.audio - a.subtitle - offset).abs() < 2.5)
        .collect();
    let mut scale = 1.;
    let mut intercept = offset;
    // Only estimate drift from anchors far apart; short samples cannot establish a rate.
    if samples >= 2 {
        let mut slopes = Vec::new();
        for a in anchors {
            for b in anchors {
                if b.subtitle - a.subtitle > 300. {
                    let slope = (b.audio - a.audio) / (b.subtitle - a.subtitle);
                    if (0.96..=1.04).contains(&slope) {
                        slopes.push(slope)
                    }
                }
            }
        }
        if !slopes.is_empty() {
            scale = median(&mut slopes);
            let mut residuals: Vec<_> = anchors
                .iter()
                .map(|a| a.audio - scale * a.subtitle)
                .collect();
            intercept = median(&mut residuals);
            inliers = anchors
                .iter()
                .filter(|a| (a.audio - scale * a.subtitle - intercept).abs() < 2.5)
                .collect();
        }
    }
    if inliers.len() < 3 || inliers.len() * 2 < anchors.len() || intercept.abs() > 300. {
        return Err("Speech matches disagree. No timing change was applied; these subtitles may be from a different edit.".into());
    }
    if scale != 1. {
        let min = inliers
            .iter()
            .map(|a| a.subtitle)
            .fold(f64::INFINITY, f64::min);
        let max = inliers.iter().map(|a| a.subtitle).fold(0., f64::max);
        if max - min < 300. {
            return Err("Not enough independent matches to verify subtitle drift.".into());
        }
    }
    let confidence = inliers.iter().map(|a| a.score).sum::<f64>() / inliers.len() as f64
        * inliers.len() as f64
        / anchors.len() as f64;
    if confidence < 0.72 {
        return Err("Speech matches were not strong enough to apply automatically. Original timing was kept.".into());
    }
    Ok(Alignment {
        offset: intercept,
        scale,
        confidence,
        anchors: inliers.len(),
        samples,
    })
}
async fn subtitle_source(
    app: &App,
    path: &Path,
    id: &str,
) -> Result<(PathBuf, Option<u32>), String> {
    if let Some(i) = id
        .strip_prefix("file:")
        .and_then(|s| s.parse::<usize>().ok())
    {
        let list = playback::local_subtitles(app, &path.to_string_lossy());
        let name = list
            .get(i)
            .filter(|n| playback::is_text_subtitle(n))
            .ok_or("Subtitle file unavailable")?;
        return Ok((
            path.parent()
                .ok_or("Subtitle folder unavailable")?
                .join(name),
            None,
        ));
    }
    if let Some(i) = id
        .strip_prefix("embedded:")
        .and_then(|s| s.parse::<u32>().ok())
    {
        let info = playback::probe_file(path).await?;
        if info["streams"].as_array().is_some_and(|streams| {
            streams.iter().any(|s| {
                s["index"].as_u64() == Some(i as u64)
                    && s["codec_type"] == "subtitle"
                    && matches!(
                        s["codec_name"].as_str(),
                        Some("subrip" | "ass" | "ssa" | "webvtt" | "mov_text")
                    )
            })
        }) {
            return Ok((path.into(), Some(i)));
        }
    }
    Err("Invalid or unsupported subtitle selection".into())
}
async fn signature(path: &Path) -> Result<String, String> {
    let meta = tokio::time::timeout(Duration::from_secs(10), tokio::fs::metadata(path))
        .await
        .map_err(|_| "Drive did not respond")?
        .map_err(|_| "Media or subtitle is unavailable")?;
    Ok(format!("{}:{:?}", meta.len(), meta.modified().ok()))
}
pub async fn sync(
    State(app): State<App>,
    Extension(actor): Extension<Principal>,
    Json(input): Json<SyncRequest>,
) -> Response {
    let Some(path) = playback::library_path(&app, &input.path) else {
        return error(StatusCode::NOT_FOUND, "File is not in the library");
    };
    if input.request_id.is_empty()
        || input.request_id.len() > 64
        || !input
            .request_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-')
    {
        return error(
            StatusCode::BAD_REQUEST,
            "Invalid synchronization request ID",
        );
    }
    let position = input.position.unwrap_or(0.);
    if !position.is_finite() || !(0.0..=1_000_000.).contains(&position) {
        return error(StatusCode::BAD_REQUEST, "Invalid sample position");
    }
    let Ok(_permit) = SYNC_LIMIT.clone().try_acquire_owned() else {
        return error(
            StatusCode::TOO_MANY_REQUESTS,
            "Another subtitle synchronization is running. Try again shortly.",
        );
    };
    let (source, stream) = match subtitle_source(&app, &path, &input.subtitle).await {
        Ok(v) => v,
        Err(e) => return error(StatusCode::BAD_REQUEST, &e),
    };
    let (sender, cancelled) = tokio::sync::oneshot::channel();
    *ACTIVE_SYNC.lock().unwrap() = Some((actor.profile_id, input.request_id.clone(), sender));
    let _active = ActiveSyncGuard;
    let result = tokio::select! {result=run(&app,&input,&path,&source,stream,actor.profile_id,position)=>result,_=cancelled=>return error(StatusCode::REQUEST_TIMEOUT,"Subtitle synchronization cancelled")};
    match result {
        Ok(value) => Json(value).into_response(),
        Err(e) => error(StatusCode::UNPROCESSABLE_ENTITY, &e),
    }
}
async fn run(
    app: &App,
    input: &SyncRequest,
    path: &Path,
    source: &Path,
    stream: Option<u32>,
    profile: i64,
    position: f64,
) -> Result<Value, String> {
    let (cli, model) = runtime(app);
    if !cli.is_file() || !model.is_file() {
        return Err("Local speech sync is not installed. Install whisper-cli and place ggml-base.bin in the server data models folder, or set SMART_MEDIA_WHISPER and SMART_MEDIA_WHISPER_MODEL.".into());
    }
    let audio = playback::audio_map(path, input.audio).await?;
    let fingerprint = format!(
        "{profile}:{}:{}:{audio}:{}:{}:{}",
        input.path,
        input.subtitle,
        signature(path).await?,
        signature(source).await?,
        signature(&model).await?
    );
    let digest = ring::digest::digest(&ring::digest::SHA256, fingerprint.as_bytes());
    let key = format!(
        "subtitle-sync:v1:{}",
        digest
            .as_ref()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    );
    if !input.refresh {
        if let Ok(raw) = db(app).query_row("SELECT value FROM cache WHERE key=?1", [&key], |r| {
            r.get::<_, String>(0)
        }) {
            if let Ok(mut value) = serde_json::from_str::<Value>(&raw) {
                value["cached"] = json!(true);
                return Ok(value);
            }
        }
    }
    let work_root = crate::hls::work_root(app);
    let _ = std::fs::create_dir_all(&work_root);
    let temp = tempfile::Builder::new()
        .prefix("plexv2-subtitle-sync-")
        .tempdir_in(&work_root)
        .map_err(|_| "Cannot create private sync workspace")?;
    let mut subtitles = Command::new(playback::binary("ffmpeg"));
    subtitles
        .args(["-v", "error", "-nostdin", "-i"])
        .arg(source);
    if let Some(i) = stream {
        subtitles.args(["-map", &format!("0:{i}")]);
    }
    subtitles.args(["-f", "webvtt", "pipe:1"]);
    let output = crate::processes::output(&mut subtitles, Duration::from_secs(120), 5_000_000)
        .await
        .map_err(|_| "Subtitle extraction timed out or exceeded its size limit")?;
    if !output.status.success() {
        return Err("Could not extract text subtitles".into());
    }
    let cues = parse_vtt(&String::from_utf8_lossy(&output.stdout));
    if cues.len() < 3 {
        return Err("At least three subtitle cues are needed for reliable synchronization".into());
    }
    let info = playback::probe_file(path).await?;
    let duration = info["format"]["duration"]
        .as_str()
        .and_then(|s| s.parse::<f64>().ok())
        .filter(|v| v.is_finite() && *v > 0.)
        .ok_or("Media duration is unavailable")?;
    let first = cues
        .iter()
        .find(|c| c.start >= position && words(&c.text).len() >= 4)
        .map(|c| c.start)
        .unwrap_or(position);
    let mut windows = vec![(first - 15.).max(0.).min((duration - 45.).max(0.))];
    if duration > 660. {
        windows.push((windows[0] + 600.).min((duration - 75.).max(0.)));
    }
    let mut matches = Vec::new();
    for (i, start) in windows.iter().enumerate() {
        let wav = temp.path().join(format!("sample-{i}.wav"));
        let out = temp.path().join(format!("speech-{i}"));
        let mut extract = Command::new(playback::binary("ffmpeg"));
        extract
            .args(["-v", "error", "-nostdin", "-i"])
            .arg(path)
            .args([
                "-ss",
                &format!("{start:.3}"),
                "-t",
                "75",
                "-map",
                &audio,
                "-vn",
                "-ac",
                "1",
                "-ar",
                "16000",
                "-c:a",
                "pcm_s16le",
                "-y",
            ])
            .arg(&wav);
        let output = crate::processes::output(&mut extract, Duration::from_secs(180), 100_000)
            .await
            .map_err(|_| "Audio sample extraction timed out")?;
        if !output.status.success() {
            return Err("This audio track cannot be decoded for speech alignment".into());
        }
        let mut recognize = Command::new(&cli);
        recognize
            .arg("-m")
            .arg(&model)
            .arg("-f")
            .arg(&wav)
            .args(["-l", "auto", "-oj", "-ml", "64", "-sow", "-t", "4", "-of"])
            .arg(&out);
        let output = crate::processes::output(&mut recognize, Duration::from_secs(240), 1_000_000)
            .await
            .map_err(|_| "Local speech recognition timed out or could not start")?;
        if !output.status.success() {
            return Err(
                "Local speech recognition failed; check the whisper-cli/model installation".into(),
            );
        }
        let data = tokio::fs::read(out.with_extension("json"))
            .await
            .map_err(|_| "Speech recognizer did not return timestamps")?;
        if data.len() > 2_000_000 {
            return Err("Speech output exceeded its limit".into());
        }
        let value: Value =
            serde_json::from_slice(&data).map_err(|_| "Invalid speech timestamps")?;
        let spoken: Vec<_> = value["transcription"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|s| {
                let from = s["offsets"]["from"].as_f64()? / 1000. + start;
                let end = s["offsets"]["to"].as_f64()? / 1000. + start;
                Some(Cue {
                    start: from,
                    end,
                    text: s["text"].as_str()?.into(),
                })
            })
            .collect();
        matches.extend(anchors(&cues, &spoken));
    }
    let alignment = fit(&matches, windows.len())?;
    let value = json!({"alignment":alignment,"cached":false,"local":true});
    let c = db(app);
    c.execute(
        "INSERT OR REPLACE INTO cache(key,value,saved) VALUES(?1,?2,?3)",
        rusqlite::params![key, value.to_string(), chrono::Utc::now().timestamp()],
    )
    .map_err(|_| "Could not save synchronization result")?;
    let _=c.execute("DELETE FROM cache WHERE key LIKE 'subtitle-sync:v1:%' AND key NOT IN (SELECT key FROM cache WHERE key LIKE 'subtitle-sync:v1:%' ORDER BY saved DESC LIMIT 256)",[]);
    Ok(value)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn cancellation_requires_matching_profile_and_request() {
        let (sender, receiver) = tokio::sync::oneshot::channel();
        *ACTIVE_SYNC.lock().unwrap() = Some((7, "owned-job".into(), sender));
        let actor = Principal {
            account_id: 1,
            username: "fixture".into(),
            email: "fixture@example.test".into(),
            role: "user".into(),
            profile_id: 8,
            profile_name: "Other".into(),
        };
        let Json(result) = cancel(
            Extension(actor.clone()),
            Json(CancelRequest {
                request_id: "owned-job".into(),
            }),
        )
        .await;
        assert_eq!(result["cancelled"], false);
        let owner = Principal {
            profile_id: 7,
            ..actor
        };
        let Json(result) = cancel(
            Extension(owner.clone()),
            Json(CancelRequest {
                request_id: "different-job".into(),
            }),
        )
        .await;
        assert_eq!(result["cancelled"], false);
        let Json(result) = cancel(
            Extension(owner),
            Json(CancelRequest {
                request_id: "owned-job".into(),
            }),
        )
        .await;
        assert_eq!(result["cancelled"], true);
        assert!(receiver.await.is_ok());
    }
    #[test]
    fn positive_negative_offset_and_drift() {
        for (offset, scale) in [(4., 1.), (-7., 1.), (2., 1.025)] {
            let anchors: Vec<_> = [10., 22., 38., 610., 625., 645.]
                .iter()
                .map(|t| Anchor {
                    subtitle: *t,
                    audio: *t * scale + offset,
                    score: 0.95,
                })
                .collect();
            let a = fit(&anchors, 2).unwrap();
            assert!((a.offset - offset).abs() < 0.01);
            assert!((a.scale - scale).abs() < 0.0001)
        }
    }
    #[test]
    fn refuses_sparse_and_conflicting_matches() {
        assert!(fit(&[], 1).is_err());
        let a: Vec<_> = [1., 20., 50.]
            .iter()
            .enumerate()
            .map(|(i, t)| Anchor {
                subtitle: *t,
                audio: *t + i as f64 * 10.,
                score: 0.8,
            })
            .collect();
        assert!(fit(&a, 1).is_err())
    }
    #[test]
    fn unicode_and_duplicate_dialogue() {
        let cues=parse_vtt("WEBVTT\n\n00:00:10.000 --> 00:00:13.000\nThis is a unique spoken sentence.\n\n00:00:40.000 --> 00:00:43.000\nThis is a unique spoken sentence.\n");
        assert_eq!(
            anchors(
                &cues,
                &[Cue {
                    start: 15.,
                    end: 18.,
                    text: "This is a unique spoken sentence.".into()
                }]
            )
            .len(),
            0
        );
        assert_eq!(words("Še čudovit DAN!"), vec!["se", "cudovit", "dan"])
    }
    #[test]
    fn finds_spoken_phrase_offset() {
        let cues = vec![Cue {
            start: 10.,
            end: 13.,
            text: "We should leave before the sun goes down".into(),
        }];
        let spoken = vec![Cue {
            start: 14.,
            end: 17.,
            text: "We should leave before the sun goes down".into(),
        }];
        let a = anchors(&cues, &spoken);
        assert_eq!(a.len(), 1);
        assert_eq!(a[0].audio - a[0].subtitle, 4.)
    }
}
