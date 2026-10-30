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
    let result = tokio::select! {result=run(&app,&input,&path,&source,stream,actor.profile_id)=>result,_=cancelled=>return error(StatusCode::REQUEST_TIMEOUT,"Subtitle synchronization cancelled")};
    match result {
        Ok(value) => Json(value).into_response(),
        Err(e) => error(StatusCode::UNPROCESSABLE_ENTITY, &e),
    }
}
// Short clips instead of long ones: up to CLIPS_WANTED samples of CLIP_SECONDS each, one per equal slice of
// the film, so timing drift shows up. A clip without speech is replaced by another random one.
const CLIP_SECONDS: f64 = 10.;
const CLIPS_WANTED: usize = 5;
const MAX_ATTEMPTS: usize = 12;
const MISSES_PER_SLICE: usize = 3;
// Keep taking clips (up to MAX_ATTEMPTS) until this many phrases have matched.
const WANTED_ANCHORS: usize = 5;
fn random_unit() -> f64 {
    let mut bytes = [0u8; 8];
    if ring::rand::SecureRandom::fill(&ring::rand::SystemRandom::new(), &mut bytes).is_err() {
        return 0.5;
    }
    (u64::from_le_bytes(bytes) >> 11) as f64 / (1u64 << 53) as f64
}
// Picks where to listen inside slice `slice` of `slices`: a random spot where the subtitles say someone talks
// (so silent scenes are rare), or any random spot if the slice has no such line.
fn pick_clip_start(dialogue: &[f64], duration: f64, slice: usize, slices: usize, tried: &mut HashSet<u64>, roll: &mut dyn FnMut() -> f64) -> f64 {
    let low = duration * 0.04;
    let high = (duration * 0.96 - CLIP_SECONDS).max(low + 1.);
    let band = (high - low) / slices as f64;
    let (from, to) = (low + band * slice as f64, low + band * (slice + 1) as f64);
    let candidates: Vec<f64> = dialogue.iter().copied().filter(|t| *t >= from && *t < to && !tried.contains(&t.to_bits())).collect();
    let centre = if candidates.is_empty() { from + roll() * band } else { candidates[((roll() * candidates.len() as f64) as usize).min(candidates.len() - 1)] };
    tried.insert(centre.to_bits());
    // Centre the clip on the cue: subtitles are usually a few seconds off, and the whole sentence must fit.
    (centre - 4.).clamp(0., (duration - CLIP_SECONDS).max(0.))
}
// Whisper starts a line where the previous one ended, so silence before a sentence leaks into its timestamp.
// `silences` are (start, end) pairs inside the clip from ffmpeg's silencedetect; the line starts at the first
// voiced moment inside it.
fn parse_silences(log: &str, clip: f64) -> Vec<(f64, f64)> {
    let (mut out, mut open) = (Vec::new(), None);
    for line in log.lines() {
        if let Some(rest) = line.split("silence_start:").nth(1) {
            open = rest.trim().parse::<f64>().ok().map(|v| v.max(0.));
        } else if let Some(rest) = line.split("silence_end:").nth(1) {
            let end = rest.split('|').next().and_then(|v| v.trim().parse::<f64>().ok());
            if let (Some(a), Some(b)) = (open.take(), end) {
                out.push((a, b));
            }
        }
    }
    if let Some(a) = open {
        out.push((a, clip));
    }
    out
}
fn first_voiced(silences: &[(f64, f64)], from: f64, to: f64) -> f64 {
    // Voiced runs inside the line; the start of a clip often holds the tail of the previous sentence, so a
    // short leading run is skipped in favour of the first run that lasts at least 0.8 s (else the longest).
    let (mut runs, mut cursor) = (Vec::new(), from);
    for (a, b) in silences {
        if *b <= cursor || *a >= to {
            continue;
        }
        if *a > cursor {
            runs.push((cursor, *a));
        }
        cursor = cursor.max(*b);
    }
    if cursor < to {
        runs.push((cursor, to));
    }
    runs.iter()
        .find(|(a, b)| b - a >= 0.8)
        .or_else(|| runs.iter().max_by(|x, y| (x.1 - x.0).total_cmp(&(y.1 - y.0))))
        .map_or(from, |run| run.0)
}
async fn recognise_clip(cli: &Path, model: &Path, path: &Path, audio: &str, temp: &Path, index: usize, start: f64) -> Result<Vec<Cue>, String> {
    let wav = temp.join(format!("sample-{index}.wav"));
    let out = temp.join(format!("speech-{index}"));
    let mut extract = Command::new(playback::binary("ffmpeg"));
    extract
        .args(["-v", "error", "-nostdin", "-ss", &format!("{start:.3}"), "-i"])
        .arg(path)
        .args(["-t", &format!("{CLIP_SECONDS}"), "-map", audio, "-vn", "-ac", "1", "-ar", "16000", "-c:a", "pcm_s16le", "-y"])
        .arg(&wav);
    let output = crate::processes::output(&mut extract, Duration::from_secs(90), 100_000)
        .await
        .map_err(|_| "Audio sample extraction timed out")?;
    if !output.status.success() {
        return Err("This audio track cannot be decoded for speech alignment".into());
    }
    let mut detect = Command::new(playback::binary("ffmpeg"));
    detect
        .args(["-v", "info", "-nostdin", "-i"])
        .arg(&wav)
        .args(["-af", "silencedetect=noise=-38dB:d=0.35", "-f", "null", "-"]);
    let silences = match crate::processes::output(&mut detect, Duration::from_secs(30), 100_000).await {
        Ok(out) if out.status.success() => parse_silences(&String::from_utf8_lossy(&out.stderr), CLIP_SECONDS),
        _ => Vec::new(),
    };
    let mut recognize = Command::new(cli);
    recognize
        .arg("-m")
        .arg(model)
        .arg("-f")
        .arg(&wav)
        .args(["-l", "auto", "-oj", "-ml", "64", "-sow", "-t", "4", "-of"])
        .arg(&out);
    let output = crate::processes::output(&mut recognize, Duration::from_secs(90), 1_000_000)
        .await
        .map_err(|_| "Local speech recognition timed out or could not start")?;
    if !output.status.success() {
        return Err("Local speech recognition failed; check the whisper-cli/model installation".into());
    }
    let data = tokio::fs::read(out.with_extension("json"))
        .await
        .map_err(|_| "Speech recognizer did not return timestamps")?;
    if data.len() > 2_000_000 {
        return Err("Speech output exceeded its limit".into());
    }
    let value: Value = serde_json::from_slice(&data).map_err(|_| "Invalid speech timestamps")?;
    let _ = tokio::fs::remove_file(&wav).await;
    Ok(value["transcription"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|s| {
            let (from, end) = (s["offsets"]["from"].as_f64()? / 1000., s["offsets"]["to"].as_f64()? / 1000.);
            Some(Cue { start: first_voiced(&silences, from, end) + start, end: end + start, text: s["text"].as_str()?.into() })
        })
        .collect())
}
async fn run(
    app: &App,
    input: &SyncRequest,
    path: &Path,
    source: &Path,
    stream: Option<u32>,
    profile: i64,
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
        "subtitle-sync:v2:{}",
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
    let dialogue: Vec<f64> = cues.iter().filter(|c| words(&c.text).len() >= 4).map(|c| c.start).collect();
    let mut tried = HashSet::new();
    let mut matches = Vec::new();
    let (mut slice, mut attempts, mut clips) = (0usize, 0usize, 0usize);
    let mut misses = 0usize;
    let mut roll = random_unit;
    while attempts < MAX_ATTEMPTS && (clips < CLIPS_WANTED || matches.len() < WANTED_ANCHORS) {
        // After the first round extra clips keep cycling through the slices.
        let start = pick_clip_start(&dialogue, duration, slice % CLIPS_WANTED, CLIPS_WANTED, &mut tried, &mut roll);
        let spoken = recognise_clip(&cli, &model, path, &audio, temp.path(), attempts, start).await?;
        attempts += 1;
        if spoken.iter().map(|c| words(&c.text).len()).sum::<usize>() >= 4 {
            clips += 1;
            matches.extend(anchors(&cues, &spoken));
            (slice, misses) = (slice + 1, 0);
        } else {
            misses += 1;
            if misses >= MISSES_PER_SLICE {
                (slice, misses) = (slice + 1, 0);
            }
        }
    }
    if clips == 0 {
        return Err("No speech could be recognised in the audio samples. Try a different audio track.".into());
    }
    let alignment = fit(&matches, clips)?;
    let value = json!({"alignment":alignment,"cached":false,"local":true,"clips":clips,"attempts":attempts});
    let c = db(app);
    c.execute(
        "INSERT OR REPLACE INTO cache(key,value,saved) VALUES(?1,?2,?3)",
        rusqlite::params![key, value.to_string(), chrono::Utc::now().timestamp()],
    )
    .map_err(|_| "Could not save synchronization result")?;
    let _=c.execute("DELETE FROM cache WHERE key LIKE 'subtitle-sync:v2:%' AND key NOT IN (SELECT key FROM cache WHERE key LIKE 'subtitle-sync:v2:%' ORDER BY saved DESC LIMIT 256)",[]);
    Ok(value)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn line_starts_move_past_leading_silence() {
        let log = "[silencedetect @ 0x1] silence_start: 0\n[silencedetect @ 0x1] silence_end: 3.4 | silence_duration: 3.4\n[silencedetect @ 0x1] silence_start: 8.75";
        let silences = parse_silences(log, 10.);
        assert_eq!(silences, vec![(0., 3.4), (8.75, 10.)]);
        assert!((first_voiced(&silences, 0., 8.) - 3.4).abs() < 1e-9, "starts at the first voiced moment");
        assert_eq!(first_voiced(&silences, 4., 8.), 4., "already voiced stays put");
        assert_eq!(first_voiced(&silences, 9., 10.), 9., "a line entirely inside silence keeps its time");
        assert_eq!(first_voiced(&[], 1.2, 5.), 1.2, "no silence detected, no change");
        // The tail of the previous sentence (0.6 s) is skipped; the sentence starts at the long run.
        let tail = [(0.6, 3.8), (6.5, 9.8)];
        assert_eq!(first_voiced(&tail, 0., 10.), 3.8);
        // Only short runs: the longest wins.
        assert!((first_voiced(&[(0.5, 1.0), (1.5, 2.4)], 0., 3.) - 2.4).abs() < 1e-9);
    }
    #[test]
    fn clips_are_spread_over_the_film_prefer_dialogue_and_never_repeat() {
        let duration = 3000.;
        let dialogue: Vec<f64> = (0..300).map(|i| 30. + i as f64 * 9.7).collect();
        let mut tried = HashSet::new();
        let mut value = 0.37;
        let mut roll = || { value = (value * 7.31 + 0.113) % 1.; value };
        let mut starts = Vec::new();
        for slice in 0..CLIPS_WANTED {
            let start = pick_clip_start(&dialogue, duration, slice, CLIPS_WANTED, &mut tried, &mut roll);
            assert!(start >= 0. && start + CLIP_SECONDS <= duration, "clip inside the file: {start}");
            let (low, high) = (duration * 0.04, duration * 0.96);
            let band = (high - low) / CLIPS_WANTED as f64;
            assert!(start + 4. >= low + band * slice as f64 - 0.001 && start + 4. < low + band * (slice + 1) as f64, "slice {slice} start {start}");
            starts.push(start);
        }
        assert!(starts.windows(2).all(|w| w[1] > w[0]), "slices are in order: {starts:?}");
        // Asking the same slice again moves to a different line; with no dialogue it still picks inside the slice.
        let again = pick_clip_start(&dialogue, duration, 2, CLIPS_WANTED, &mut tried, &mut roll);
        assert!((again - starts[2]).abs() > 1.);
        let mut empty = HashSet::new();
        let any = pick_clip_start(&[], duration, 4, CLIPS_WANTED, &mut empty, &mut roll);
        assert!(any > duration * 0.04 && any + CLIP_SECONDS <= duration);
        // A very short file still yields a valid start.
        assert!(pick_clip_start(&[], 12., 0, CLIPS_WANTED, &mut HashSet::new(), &mut roll) >= 0.);
    }
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
