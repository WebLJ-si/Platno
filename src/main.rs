mod auth;
mod diagnostics;
mod hls;
mod media_features;
mod parser;
mod playback;
use axum::{
    extract::{Path as AxPath, Query, State},
    http::{HeaderMap, StatusCode},
    response::{Html, IntoResponse},
    routing::{get, post},
    Json, Router, middleware,
};
use axum::response::Response;
use chrono::{Local, Utc};
use parser::{normalize, parse, Parsed};
use reqwest::Client;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::sync::Mutex;
use walkdir::WalkDir;

#[derive(Clone)]
pub(crate) struct App {
    pub(crate) db: PathBuf,
    client: Client,
    status: Arc<Mutex<Job>>,
    cancel: Arc<AtomicBool>,
    collections_running: Arc<AtomicBool>,
    collections_attempted: Arc<Mutex<HashSet<i64>>>,
    availability: Arc<std::sync::RwLock<std::collections::HashMap<String, bool>>>,
    availability_running: Arc<AtomicBool>,
    availability_generation: Arc<std::sync::atomic::AtomicU64>,
}
#[derive(Default, Clone, Serialize)]
struct Job {
    running: bool,
    phase: String,
    processed: usize,
    total: usize,
    errors: Vec<String>,
    started_at: Option<String>,
    finished_at: Option<String>,
    current_file: Option<String>,
    current_file_started_at: Option<String>,
    action: String,
    events: VecDeque<ScanEvent>,
}
#[derive(Clone, Serialize)]
struct ScanEvent {
    time: String,
    message: String,
}
impl Job {
    fn log(&mut self, message: impl Into<String>) {
        if self.events.len() >= 100 {
            self.events.pop_front();
        }
        self.events.push_back(ScanEvent {
            time: Utc::now().to_rfc3339(),
            message: message.into(),
        });
    }
}
#[derive(Deserialize)]
struct ScanRequest {
    roots: Vec<String>,
    library_id: Option<i64>,
    #[serde(default)]
    full_scan: bool,
}
#[derive(Deserialize)]
struct LibraryInput { id: Option<i64>, name: String, kind: String, roots: Option<Vec<String>> }
#[derive(Deserialize)]
struct LibraryId { id: i64 }
#[derive(Deserialize)]
struct LibrarySettingsInput { auto_sort: bool, scan_roots: Vec<String> }
#[derive(Deserialize)]
struct Override {
    kind: String,
    provider_id: Option<i64>,
    title: Option<String>,
    season: Option<u32>,
    episodes: Option<Vec<u32>>,
}
#[cfg(test)]
#[derive(Deserialize)]
struct ProgressQuery {
    path: Option<String>,
}
#[cfg(test)]
#[derive(Deserialize)]
struct ProgressUpdate {
    path: String,
    position: f64,
    duration: f64,
    #[serde(default)]
    completed: bool,
}
fn db(app: &App) -> Connection {
    let c = Connection::open(&app.db).expect("database open");
    c.busy_timeout(Duration::from_secs(10)).ok();
    c.execute_batch("PRAGMA foreign_keys=ON;").ok();
    c
}
fn init(c: &Connection) {
    c.execute_batch("PRAGMA journal_mode=WAL; CREATE TABLE IF NOT EXISTS files(path TEXT PRIMARY KEY, root TEXT NOT NULL, size INTEGER NOT NULL, mtime INTEGER NOT NULL, parsed TEXT NOT NULL, result TEXT NOT NULL, locked INTEGER NOT NULL DEFAULT 0, available INTEGER NOT NULL DEFAULT 1, scanned TEXT NOT NULL); CREATE TABLE IF NOT EXISTS cache(key TEXT PRIMARY KEY, value TEXT NOT NULL, saved INTEGER NOT NULL); CREATE TABLE IF NOT EXISTS settings(key TEXT PRIMARY KEY, value TEXT NOT NULL); CREATE TABLE IF NOT EXISTS watch_progress(path TEXT PRIMARY KEY, position REAL NOT NULL, duration REAL NOT NULL, updated_at INTEGER NOT NULL, completed INTEGER NOT NULL DEFAULT 0); CREATE TABLE IF NOT EXISTS libraries(id INTEGER PRIMARY KEY, name TEXT NOT NULL COLLATE NOCASE UNIQUE, kind TEXT NOT NULL CHECK(kind IN ('movie','tv','video','other')), roots TEXT NOT NULL DEFAULT '[]', created_at INTEGER NOT NULL); CREATE TABLE IF NOT EXISTS library_files(path TEXT PRIMARY KEY, library_id INTEGER NOT NULL REFERENCES libraries(id) ON DELETE CASCADE); INSERT OR IGNORE INTO libraries(name,kind,created_at) VALUES('Movies','movie',0),('TV Shows','tv',0),('Videos','video',0); INSERT OR IGNORE INTO settings(key,value) VALUES('library_auto_sort','true'); PRAGMA foreign_keys=ON;").unwrap();
    let has_completed: bool = c
        .prepare("PRAGMA table_info(watch_progress)")
        .and_then(|mut s| {
            s.query_map([], |r| r.get::<_, String>(1))?
                .collect::<Result<Vec<_>, _>>()
        })
        .map(|columns| columns.iter().any(|column| column == "completed"))
        .unwrap_or(false);
    if !has_completed {
        let _ = c.execute_batch(
            "ALTER TABLE watch_progress ADD COLUMN completed INTEGER NOT NULL DEFAULT 0;",
        );
    }
    backfill_library_assignments(c);
}
fn library_kind_for_result(result:&Value)->&'static str {
    match result["kind"].as_str().unwrap_or("") {"movie"=>"movie","tv"|"extra"=>"tv","video"=>"video",_=>"video"}
}
fn backfill_library_assignments(c:&Connection) {
    let rows:Vec<(String,String)>=c.prepare("SELECT path,result FROM files WHERE path NOT IN (SELECT path FROM library_files)").and_then(|mut s|s.query_map([],|r|Ok((r.get(0)?,r.get(1)?)))?.collect()).unwrap_or_default();
    for (path,raw) in rows {let result:Value=serde_json::from_str(&raw).unwrap_or_default();let kind=library_kind_for_result(&result);let _=c.execute("INSERT OR IGNORE INTO library_files(path,library_id) SELECT ?1,id FROM libraries WHERE kind=?2 ORDER BY id LIMIT 1",params![path,kind]);}
}
// Repair old automatic records whose paths identify bonus content. Locked
// user decisions are never changed; no filesystem media reads are required.
fn repair_bonus_records(c: &Connection) {
    let mut stmt = c
        .prepare("SELECT path,parsed,result FROM files WHERE locked=0")
        .unwrap();
    let rows = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        })
        .unwrap();
    let mut changes = Vec::new();
    for (path, parsed, result) in rows.flatten() {
        let old: Value = serde_json::from_str(&parsed).unwrap_or_default();
        let generic = old["title"].as_str().is_some_and(|t| {
            matches!(
                normalize(t).as_str(),
                "extras" | "extra" | "webisodes" | "bonus" | "bonus features"
            )
        });
        if !generic {
            continue;
        }
        let parsed = parse(Path::new(&path));
        if parsed.kind != "extra" {
            continue;
        }
        let old_result: Value = serde_json::from_str(&result).unwrap_or_default();
        let updated = json!({"kind":"extra", "title":parsed.title, "season":parsed.season, "episodes":[], "confidence":0, "explanation":"Bonus material detected from its filename or folder; not an ordinary episode", "subtitles":old_result["subtitles"], "candidates":[], "provider":"", "order":"aired"});
        changes.push((
            path,
            serde_json::to_string(&parsed).unwrap(),
            updated.to_string(),
        ));
    }
    drop(stmt);
    for (path, parsed, result) in changes {
        let _ = c.execute(
            "UPDATE files SET parsed=?2,result=?3 WHERE path=?1 AND locked=0",
            params![path, parsed, result],
        );
    }
}
async fn cached(app: &App, key: &str, url: &str, auth: Option<&str>) -> Result<Value, String> {
    let cached: Option<(String, i64)> = db(app)
        .query_row("SELECT value,saved FROM cache WHERE key=?1", [key], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .ok();
    if let Some((v, t)) = &cached {
        if Utc::now().timestamp() - t < 86400 * 7 {
            if let Ok(v) = serde_json::from_str(v) {
                return Ok(v);
            }
        }
    }
    let mut error = String::new();
    for attempt in 0..3 {
        let mut req = app.client.get(url);
        if let Some(token) = auth {
            req = req.bearer_auth(token);
        }
        match req.send().await {
            Ok(res) if res.status().is_success() => match res.json::<Value>().await {
                Ok(v) => {
                    let _ = db(app).execute(
                        "INSERT OR REPLACE INTO cache VALUES(?1,?2,?3)",
                        params![key, v.to_string(), Utc::now().timestamp()],
                    );
                    return Ok(v);
                }
                Err(e) => error = e.to_string(),
            },
            Ok(res) => {
                error = format!("HTTP {}", res.status());
                if !res.status().is_server_error() && res.status() != StatusCode::TOO_MANY_REQUESTS
                {
                    break;
                }
            }
            Err(e) => error = e.to_string(),
        };
        tokio::time::sleep(Duration::from_millis(400 * (1 << attempt))).await;
    }
    if let Some((v, _)) = cached {
        return serde_json::from_str(&v).map_err(|e| e.to_string());
    }
    Err(error)
}
fn subtitles(path: &Path) -> Vec<String> {
    let mut out = vec![];
    let stem = path
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .to_lowercase();
    if let Some(dir) = path.parent() {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for e in entries.flatten() {
                let p = e.path();
                let ext = p
                    .extension()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_lowercase();
                if ["srt", "ass", "ssa", "vtt", "sub"].contains(&ext.as_str()) {
                    let s = p
                        .file_stem()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_lowercase();
                    if s == stem || s.strip_prefix(&stem).is_some_and(|x| x.starts_with('.')) {
                        out.push(
                            p.file_name()
                                .unwrap_or_default()
                                .to_string_lossy()
                                .to_string(),
                        );
                    }
                }
            }
        }
    }
    out
}
fn videos(path: &Path) -> bool {
    let ext = path
        .extension()
        .unwrap_or_default()
        .to_string_lossy()
        .to_lowercase();
    [
        "mkv", "mp4", "avi", "mov", "m4v", "wmv", "ts", "m2ts", "webm", "mpg", "mpeg",
    ]
    .contains(&ext.as_str())
}
fn tmdb_token(app: &App) -> String {
    std::env::var("TMDB_TOKEN")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .or_else(|| {
            app.db
                .parent()
                .and_then(|p| std::fs::read_to_string(p.join("tmdb-token")).ok())
        })
        .unwrap_or_default()
        .trim()
        .to_owned()
}
fn normalize_episode_title(title: &str) -> String {
    let title = title.replace('&', " and ").replace(['\'', '’'], "");
    normalize(&title)
        .split_whitespace()
        .filter_map(|w| match w {
            "the" | "a" | "an" => None,
            "pt" => Some("part"),
            "tv" => Some("television"),
            "eight" => Some("8"),
            "one" => Some("1"),
            "two" => Some("2"),
            "three" => Some("3"),
            _ => Some(w),
        })
        .collect::<Vec<_>>()
        .join(" ")
}
// Only soften a title mismatch when the numbered episode is also the clear
// best title in the catalogue; close runners-up stay in review.
fn numbered_title_supported(title: &str, expected: &Value, catalogue: &[Value]) -> bool {
    let query = normalize_episode_title(title);
    let target = normalize_episode_title(expected["name"].as_str().unwrap_or(""));
    if query.is_empty() || target.is_empty() {
        return false;
    }
    if query == target {
        return catalogue
            .iter()
            .filter(|e| normalize_episode_title(e["name"].as_str().unwrap_or("")) == query)
            .count()
            == 1;
    }
    if query.len() < 8 || target.len() < 8 {
        return false;
    }
    let score = strsim::jaro_winkler(&query, &target);
    if score < 0.91 {
        return false;
    }
    let others = catalogue
        .iter()
        .filter(|e| {
            e["id"] != expected["id"]
                || e["season"] != expected["season"]
                || e["number"] != expected["number"]
        })
        .map(|e| {
            strsim::jaro_winkler(
                &query,
                &normalize_episode_title(e["name"].as_str().unwrap_or("")),
            )
        })
        .fold(0.0_f64, f64::max);
    score - others >= 0.08
}

// Some numbered "complete collection" folders contain standalone animated
// shorts, not television episodes. Accept one only when exact title, contextual
// character names, animation genre and a short runtime all agree.
async fn identify_collection_short(app: &App, p: &Parsed, subs: &[String]) -> Option<Value> {
    let collection = p.collection_title.as_ref()?;
    let context: Vec<String> = normalize(collection)
        .split_whitespace()
        .filter(|w| w.len() >= 3 && !["the", "and", "for", "with"].contains(w))
        .map(str::to_owned)
        .collect();
    if context.len() < 2 || p.episode_title.len() < 4 {
        return None;
    }
    let token = tmdb_token(app);
    if token.is_empty() {
        return None;
    }
    let title = &p.episode_title;
    let url = format!(
        "https://api.themoviedb.org/3/search/movie?query={}",
        urlencoding::encode(title)
    );
    let data = cached(app, &format!("tmdb:movie:{}", title), &url, Some(&token))
        .await
        .ok()?;
    let mut eligible = Vec::new();
    for film in data["results"].as_array()? {
        if normalize(film["title"].as_str().unwrap_or("")) != normalize(title) {
            continue;
        }
        if !film["genre_ids"]
            .as_array()
            .is_some_and(|g| g.iter().any(|id| id.as_i64() == Some(16)))
        {
            continue;
        }
        let overview = normalize(film["overview"].as_str().unwrap_or(""));
        if !context
            .iter()
            .all(|name| overview.split_whitespace().any(|word| word == name))
        {
            continue;
        }
        let id = film["id"].as_i64()?;
        let details = cached(
            app,
            &format!("tmdb:movie-detail:{id}"),
            &format!("https://api.themoviedb.org/3/movie/{id}"),
            Some(&token),
        )
        .await
        .ok()?;
        if !details["runtime"]
            .as_u64()
            .is_some_and(|m| m > 0 && m <= 40)
        {
            continue;
        }
        eligible.push((film.clone(), details));
    }
    if eligible.len() != 1 {
        return None;
    }
    let (film, detail) = eligible.pop()?;
    let mut result = json!({"kind":"movie","title":film["title"],"provider":"tmdb","provider_id":film["id"],"year":film["release_date"],"description":film["overview"],"confidence":95,"explanation":"Standalone animated short: exact title, collection characters, genre and runtime verified with TMDB", "subtitles":subs, "candidates":[], "order":"aired", "requires_review":false,"matcher_version":6,"collection_title":collection,"runtime":detail["runtime"]});
    if let Some(poster) = film["poster_path"].as_str() {
        if let Ok(config) = cached(
            app,
            "tmdb:configuration",
            "https://api.themoviedb.org/3/configuration",
            Some(&token),
        )
        .await
        {
            let base = config["images"]["secure_base_url"].as_str().unwrap_or("");
            let sizes = config["images"]["poster_sizes"]
                .as_array()
                .cloned()
                .unwrap_or_default();
            let size = sizes
                .iter()
                .filter_map(Value::as_str)
                .find(|s| *s == "w342")
                .or_else(|| sizes.first().and_then(Value::as_str))
                .unwrap_or("");
            if !base.is_empty() && !size.is_empty() {
                result["poster"] = json!(format!("{base}{size}{poster}"));
            }
        }
    }
    Some(result)
}
async fn identify(app: &App, p: &Parsed, subs: Vec<String>) -> Value {
    let mut result = json!({"kind":p.kind,"title":p.title,"season":p.season,"episodes":p.episodes,"confidence":0,"explanation":p.explanation,"subtitles":subs,"candidates":[],"provider":"","order":"aired","requires_review":false,"matcher_version":6});
    if p.kind == "extra" {
        return result;
    }
    let query = urlencoding::encode(&p.title);
    if p.kind == "movie" {
        let token = tmdb_token(app);
        if token.is_empty() {
            result["explanation"] =
                json!("TMDB_TOKEN is not configured; movie identification unverified");
            return result;
        }
        let url = format!("https://api.themoviedb.org/3/search/movie?query={query}");
        match cached(app, &format!("tmdb:movie:{}", p.title), &url, Some(&token)).await {
            Ok(data) => {
                let items = data["results"].as_array().cloned().unwrap_or_default();
                let mut candidates: Vec<(f64, Value)> = items
                    .into_iter()
                    .map(|x| {
                        let name = x["title"].as_str().unwrap_or("");
                        let sim = strsim::jaro_winkler(&normalize(&p.title), &normalize(name));
                        let yr = x["release_date"]
                            .as_str()
                            .unwrap_or("")
                            .get(..4)
                            .and_then(|s| s.parse::<u32>().ok());
                        let score = sim * 80.0
                            + if p.year.is_some() && p.year == yr {
                                20.0
                            } else {
                                0.0
                            };
                        (score, x)
                    })
                    .collect();
                candidates.sort_by(|a, b| b.0.total_cmp(&a.0));
                result["candidates"]=json!(candidates.iter().take(5).map(|(s,x)|json!({"id":x["id"],"title":x["title"],"year":x["release_date"],"score":s})).collect::<Vec<_>>());
                if let Some((score, x)) = candidates.first() {
                    let margin = score - candidates.get(1).map(|x| x.0).unwrap_or(0.0);
                    result["title"] = x["title"].clone();
                    result["provider_id"] = x["id"].clone();
                    result["provider"] = json!("tmdb");
                    result["year"] = x["release_date"].clone();
                    result["description"] = x["overview"].clone();
                    result["rating"] = x["vote_average"].clone();
                    result["language"] = x["original_language"].clone();
                    if let Some(poster) = x["poster_path"].as_str() {
                        if let Ok(config) = cached(
                            app,
                            "tmdb:configuration",
                            "https://api.themoviedb.org/3/configuration",
                            Some(&token),
                        )
                        .await
                        {
                            let base = config["images"]["secure_base_url"].as_str().unwrap_or("");
                            let sizes = config["images"]["poster_sizes"]
                                .as_array()
                                .cloned()
                                .unwrap_or_default();
                            let size = sizes
                                .iter()
                                .filter_map(Value::as_str)
                                .find(|s| *s == "w342")
                                .or_else(|| sizes.first().and_then(Value::as_str))
                                .unwrap_or("");
                            if !base.is_empty() && !size.is_empty() {
                                result["poster"] = json!(format!("{base}{size}{poster}"));
                            }
                        }
                    }
                    result["confidence"] = json!(if margin < 5.0 {
                        score.min(89.0)
                    } else {
                        *score
                    });
                    result["explanation"] = json!(format!(
                        "Title similarity and release year; candidate margin {:.0}",
                        margin
                    )); /* image configuration deliberately not guessed */
                }
            }
            Err(e) => result["explanation"] = json!(format!("TMDB unavailable: {e}")),
        }
        return result;
    }
    let mut search_title = p.title.clone();
    let mut data = match cached(
        app,
        &format!("tvmaze:search:{}", normalize(&search_title)),
        &format!(
            "https://api.tvmaze.com/search/shows?q={}",
            urlencoding::encode(&search_title)
        ),
        None,
    )
    .await
    {
        Ok(x) => x,
        Err(e) => {
            result["explanation"] = json!(format!("TVmaze unavailable: {e}"));
            return result;
        }
    };
    // Search a shorter corroborating folder title only when the filename title
    // produced no candidates; it must already be a prefix of the filename title.
    if data.as_array().is_none_or(|items| items.is_empty()) {
        for alternate in &p.alternate_titles {
            match cached(
                app,
                &format!("tvmaze:search:{}", normalize(alternate)),
                &format!(
                    "https://api.tvmaze.com/search/shows?q={}",
                    urlencoding::encode(alternate)
                ),
                None,
            )
            .await
            {
                Ok(found) if found.as_array().is_some_and(|a| !a.is_empty()) => {
                    data = found;
                    search_title = alternate.clone();
                    break;
                }
                Err(e) => {
                    result["explanation"] =
                        json!(format!("TVmaze alternate search unavailable: {e}"));
                }
                _ => {}
            }
        }
    }
    let mut candidates: Vec<(f64, Value)> = data
        .as_array()
        .unwrap_or(&vec![])
        .iter()
        .map(|v| {
            let x = v["show"].clone();
            let sim = strsim::jaro_winkler(
                &normalize(&search_title),
                &normalize(x["name"].as_str().unwrap_or("")),
            );
            (sim * 100.0, x)
        })
        .collect();
    // A season's year is not necessarily a show's premiere year. Use year
    // only to distinguish shows with the same exact title, never to rank a
    // different title above the exact show.
    let query_name = normalize(&search_title);
    let exact_count = candidates
        .iter()
        .filter(|(_, x)| normalize(x["name"].as_str().unwrap_or("")) == query_name)
        .count();
    if exact_count > 1 {
        for (score, show) in &mut candidates {
            if normalize(show["name"].as_str().unwrap_or("")) == query_name {
                let show_year = show["premiered"]
                    .as_str()
                    .and_then(|d| d.get(..4))
                    .and_then(|y| y.parse::<u32>().ok());
                if matches!((p.year,show_year),(Some(a),Some(b)) if a.abs_diff(b)>2) {
                    *score -= 15.0;
                }
            }
        }
    }
    candidates.sort_by(|a, b| b.0.total_cmp(&a.0));
    result["candidates"]=json!(candidates.iter().take(5).map(|(s,x)|json!({"id":x["id"],"title":x["name"],"year":x["premiered"],"score":s,"poster":x["image"]["medium"]})).collect::<Vec<_>>());
    if let Some((score, show)) = candidates.first() {
        result["genres"]=show["genres"].clone();
        result["rating"]=show["rating"]["average"].clone();
        result["runtime"]=show["averageRuntime"].clone();
        result["language"]=show["language"].clone();
        let margin = score - candidates.get(1).map(|x| x.0).unwrap_or(0.0);
        if *score < 80.0 {
            result["explanation"] = json!("No sufficiently similar show; review candidates");
            return result;
        }
        let id = show["id"].as_i64().unwrap_or(0);
        result["provider"] = json!("tvmaze");
        result["provider_id"] = json!(id);
        result["title"] = show["name"].clone();
        result["poster"] = show["image"]["medium"].clone();
        result["year"] = show["premiered"].clone();
        result["description"] = show["summary"].clone();
        let catalogue = cached(
            app,
            &format!("tvmaze:episodes:{id}"),
            &format!("https://api.tvmaze.com/shows/{id}/episodes?specials=1"),
            None,
        )
        .await;
        if let Ok(cat) = catalogue {
            let episodes = cat.as_array().cloned().unwrap_or_default();
            let mut matching: Vec<&Value> = vec![];
            if let Some(s) = p.season {
                if !p.episodes.is_empty() {
                    matching = episodes
                        .iter()
                        .filter(|e| {
                            e["season"].as_u64() == Some(s as u64)
                                && p.episodes
                                    .contains(&(e["number"].as_u64().unwrap_or(999999) as u32))
                        })
                        .collect();
                }
            }
            if matching.is_empty() && p.episodes.is_empty() && !p.episode_title.is_empty() {
                matching = episodes
                    .iter()
                    .filter(|e| {
                        normalize(e["name"].as_str().unwrap_or("")) == normalize(&p.episode_title)
                            && p.season
                                .is_none_or(|s| e["season"].as_u64() == Some(s as u64))
                    })
                    .collect();
            }
            let unique = if p.episodes.is_empty() {
                matching.len() == 1
            } else {
                matching.len() == p.episodes.len()
            };
            // A unique exact show name can beat a similarly named spin-off;
            // exact-name remakes still need year evidence or review.
            let exact_show =
                normalize(&search_title) == normalize(show["name"].as_str().unwrap_or(""));
            let duplicate_exact = candidates.iter().skip(1).any(|(other_score, other)| {
                normalize(other["name"].as_str().unwrap_or("")) == normalize(&search_title)
                    && score - other_score < 5.0
            });
            let show_clear = margin >= 5.0 || (exact_show && !duplicate_exact);
            let mut title_conflict = !p.episodes.is_empty()
                && !p.episode_title.is_empty()
                && !matching.is_empty()
                && if p.episodes.len() == 1 {
                    !numbered_title_supported(&p.episode_title, matching[0], &episodes)
                } else {
                    // Every part must be supported by the combined filename.
                    let title = normalize_episode_title(&p.episode_title);
                    !matching.iter().all(|e| {
                        let name = normalize_episode_title(e["name"].as_str().unwrap_or(""));
                        !name.is_empty() && title.contains(&name)
                    })
                };
            // If title and numbering disagree, use an independently unique title
            // match from the same provider rather than silently mixing orders.
            let mut remapped = false;
            if (title_conflict || !unique) && p.episodes.len() == 1 && !p.episode_title.is_empty() {
                let title = normalize_episode_title(&p.episode_title);
                let title_matches: Vec<&Value> = episodes
                    .iter()
                    .filter(|e| {
                        let candidate = normalize_episode_title(e["name"].as_str().unwrap_or(""));
                        !title.is_empty()
                            && (candidate == title
                                || (title.len() >= 12
                                    && strsim::jaro_winkler(&candidate, &title) >= 0.97))
                    })
                    .collect();
                if title_matches.len() == 1 {
                    matching = title_matches;
                    title_conflict = false;
                    remapped = true;
                }
            }
            if (unique || remapped) && show_clear && !title_conflict {
                let first = matching[0];
                result["season"] = first["season"].clone();
                result["episodes"] = json!(matching
                    .iter()
                    .filter_map(|e| e["number"].as_u64())
                    .collect::<Vec<_>>());
                result["confidence"] = json!((*score).min(100.0));
                result["explanation"] = json!(if search_title != p.title {
                    "Series matched using a corroborating folder title; episode verified in the TVmaze aired catalogue"
                } else if remapped {
                    "Unique episode title matched the TVmaze aired catalogue at a different number; filename numbering retained in parser details"
                } else if p.episodes.is_empty() {
                    "Unique exact episode title in TVmaze aired catalogue, supported by folder title"
                } else {
                    "Filename episode numbers found in TVmaze aired catalogue; show title similarity supports match"
                });
            } else {
                result["confidence"] = json!((*score).min(79.0));
                result["requires_review"] = json!(true);
                result["explanation"] = json!(if title_conflict {
                    "Filename episode title conflicts with aired numbering; review alternative ordering"
                } else if matching.len() > 1 && p.episodes.is_empty() {
                    "Episode title occurs more than once; manual review required"
                } else {
                    "Episode or show candidate is ambiguous, or numbering was not found in the catalogue"
                });
            }
        } else {
            result["confidence"] = json!((*score).min(64.0));
            result["requires_review"] = json!(true);
            result["explanation"] = json!("Episode catalogue unavailable; match unverified");
        }
    }
    if result["requires_review"] == true || result["provider_id"].is_null() {
        if let Some(short) = identify_collection_short(app, p, &subs).await {
            return short;
        }
    }
    result
}
fn assign_scanned_file(c:&Connection,path:&str,result:&Value,library_id:Option<i64>,auto_sort:bool) {
    let result_kind=library_kind_for_result(result);
    let target=if auto_sort && matches!(result_kind,"movie"|"tv") {c.query_row("SELECT id FROM libraries WHERE kind=?1 ORDER BY id LIMIT 1",[result_kind],|r|r.get::<_,i64>(0)).ok()} else {library_id.or_else(||c.query_row("SELECT id FROM libraries WHERE kind=?1 ORDER BY id LIMIT 1",[result_kind],|r|r.get::<_,i64>(0)).ok())};
    if let Some(id)=target {let _=c.execute("INSERT INTO library_files(path,library_id) VALUES(?1,?2) ON CONFLICT(path) DO UPDATE SET library_id=excluded.library_id",params![path,id]);}
}
async fn scan(app: App, roots: Vec<String>, library_id:Option<i64>, auto_sort:bool, full_scan:bool) {
    let mut paths = vec![];
    let mut seen = HashSet::new();
    for root in roots {
        if app.cancel.load(Ordering::Relaxed) {
            break;
        }
        {
            let mut s = app.status.lock().await;
            s.action = format!("Searching folder: {root}");
            s.log(format!("Searching folder: {root}"));
        }
        let canonical = match std::fs::canonicalize(&root) {
            Ok(x) if x.is_dir() => x,
            _ => {
                let mut s = app.status.lock().await;
                let message = format!("Unavailable root: {root}");
                s.errors.push(message.clone());
                s.log(message);
                continue;
            }
        };
        for entry in WalkDir::new(&canonical).follow_links(false).into_iter() {
            if app.cancel.load(Ordering::Relaxed) {
                break;
            }
            tokio::task::yield_now().await;
            match entry {
                Ok(e) => {
                    if e.file_type().is_file() && videos(e.path()) {
                        let path = e.path().to_path_buf();
                        if seen.insert(path.clone()) {
                            paths.push((canonical.clone(), path));
                            app.status.lock().await.total = paths.len();
                        }
                    }
                }
                Err(e) => {
                    let mut s = app.status.lock().await;
                    let message = e.to_string();
                    s.errors.push(message.clone());
                    s.log(message);
                }
            }
        }
    }
    {
        let mut s = app.status.lock().await;
        s.total = paths.len();
        s.phase = "Identifying media".into();
        s.action = "Preparing files for identification".into();
        s.log(format!(
            "Discovery complete: {} video files found",
            paths.len()
        ));
    }
    for (root, path) in paths {
        if app.cancel.load(Ordering::Relaxed) {
            break;
        }
        {
            let mut s = app.status.lock().await;
            s.current_file = Some(path.to_string_lossy().to_string());
            s.current_file_started_at = Some(Utc::now().to_rfc3339());
            s.action = "Reading file details".into();
        }
        let md = match std::fs::metadata(&path) {
            Ok(x) => x,
            Err(e) => {
                let mut s = app.status.lock().await;
                s.processed += 1;
                s.log(format!("Could not read {}: {e}", path.display()));
                continue;
            }
        };
        let mtime = md
            .modified()
            .ok()
            .and_then(|x| x.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|x| x.as_secs() as i64)
            .unwrap_or(0);
        let ps = path.to_string_lossy().to_string();
        let old: Option<(i64, i64, i64, String)> = db(&app)
            .query_row(
                "SELECT size,mtime,locked,result FROM files WHERE path=?1",
                [&ps],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .ok();
        if old.as_ref().is_some_and(|(size, mt, locked, previous)| {
            let previous_result = serde_json::from_str::<Value>(previous).unwrap_or_default();
            let version = previous_result["matcher_version"].as_u64().unwrap_or(0);
            let old_matcher = (matches!(previous_result["kind"].as_str(), Some("tv" | "extra"))
                && version < 3)
                || (previous_result["kind"] == "tv"
                    && previous_result["provider_id"].is_null()
                    && version < 4);
            let stale_bonus = path.ancestors().any(|p| {
                p.file_name()
                    .is_some_and(|name| name.to_string_lossy().eq_ignore_ascii_case("Featurettes"))
            }) && previous_result["kind"] == "tv";
            let old_region_name = previous_result["provider_id"].is_null()
                && previous_result["kind"] == "tv"
                && (previous_result["title"]
                    .as_str()
                    .unwrap_or("")
                    .ends_with("(US")
                    || previous_result["title"]
                        .as_str()
                        .unwrap_or("")
                        .ends_with("(UK")
                    || previous_result["title"] == "Featurettes");
            let stale_review = previous_result["kind"] == "tv"
                && (previous_result["requires_review"] == true
                    || previous_result["provider_id"].is_null())
                && version < 5;
            let unmatched_movie = previous_result["kind"] == "movie"
                && previous_result["provider_id"].is_null()
                && version < 6;
            let old_range_count = previous_result["episodes"]
                .as_array()
                .map_or(0, |a| a.len());
            let stale_range = old_range_count > 1
                && previous_result["requires_review"] == true
                && parse(&path).episodes.len() < old_range_count;
            let missing_token = Some(&previous_result).is_some_and(|v| {
                v["kind"] == "movie"
                    && v["explanation"]
                        .as_str()
                        .unwrap_or("")
                        .starts_with("TMDB_TOKEN is not configured")
            });
            *locked == 1
                || (!full_scan && *size == md.len() as i64
                    && *mt == mtime
                    && !old_matcher
                    && !stale_range
                    && !unmatched_movie
                    && !stale_review
                    && !old_region_name
                    && !stale_bonus
                    && !(missing_token && !tmdb_token(&app).is_empty()))
        }) {
            {
                let mut s = app.status.lock().await;
                s.action = "Keeping previous identification".into();
                s.log(format!("Skipped unchanged or locked: {ps}"));
            }
            let _ = db(&app).execute(
                "UPDATE files SET available=1, scanned=?2 WHERE path=?1",
                params![ps, Utc::now().to_rfc3339()],
            );
            if let Some((_,_,_,previous))=&old {let result:Value=serde_json::from_str(previous).unwrap_or_default();assign_scanned_file(&db(&app),&ps,&result,library_id,auto_sort);}
        } else {
            {
                let mut s = app.status.lock().await;
                s.action = "Parsing filename and looking up metadata".into();
                s.log(format!("Identifying: {ps}"));
            }
            let p = parse(&path);
            let result = identify(&app, &p, subtitles(&path)).await;
            {
                let mut s = app.status.lock().await;
                s.action = "Saving identification".into();
                s.log(format!(
                    "{}: {} — {}",
                    ps,
                    result["title"].as_str().unwrap_or("Unidentified"),
                    result["explanation"].as_str().unwrap_or("")
                ));
            }
            let _=db(&app).execute("INSERT INTO files(path,root,size,mtime,parsed,result,scanned) VALUES(?1,?2,?3,?4,?5,?6,?7) ON CONFLICT(path) DO UPDATE SET root=excluded.root,size=excluded.size,mtime=excluded.mtime,parsed=excluded.parsed,result=excluded.result,available=1,scanned=excluded.scanned",params![ps,root.to_string_lossy(),md.len() as i64,mtime,serde_json::to_string(&p).unwrap(),result.to_string(),Utc::now().to_rfc3339()]);
            assign_scanned_file(&db(&app),&ps,&result,library_id,auto_sort);
        }
        app.status.lock().await.processed += 1;
    }
    app.availability_generation.fetch_add(1, Ordering::AcqRel);
    app.availability.write().unwrap().clear();
    let mut s = app.status.lock().await;
    s.running = false;
    s.finished_at = Some(Utc::now().to_rfc3339());
    s.current_file = None;
    s.current_file_started_at = None;
    s.action = String::new();
    s.phase = if app.cancel.load(Ordering::Relaxed) {
        "Cancelled"
    } else {
        "Complete"
    }
    .into();
    let message = format!(
        "{}: {} of {} files processed",
        s.phase, s.processed, s.total
    );
    s.log(message);
}
// The browser cannot expose an absolute folder path through a file input.
// Open a local native chooser instead; never interpolate browser input into AppleScript.
async fn choose_folder() -> impl IntoResponse {
    #[cfg(target_os = "macos")]
    {
        let result = tokio::process::Command::new("osascript")
            .args([
                "-e",
                "POSIX path of (choose folder with prompt \"Choose a media folder to scan\")",
            ])
            .output()
            .await;
        match result {
            Ok(output) if output.status.success() => {
                let path = String::from_utf8_lossy(&output.stdout)
                    .trim_end_matches(['\r', '\n'])
                    .to_owned();
                (StatusCode::OK, Json(json!({"path":path})))
            }
            Ok(output) if String::from_utf8_lossy(&output.stderr).contains("-128") => {
                (StatusCode::OK, Json(json!({"cancelled":true})))
            }
            Ok(output) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(
                    json!({"error":format!("Folder chooser failed: {}", String::from_utf8_lossy(&output.stderr).trim())}),
                ),
            ),
            Err(error) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error":format!("Cannot open folder chooser: {error}")})),
            ),
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        (
            StatusCode::NOT_IMPLEMENTED,
            Json(
                json!({"error":"The folder chooser is currently available on macOS. Enter an absolute path instead."}),
            ),
        )
    }
}
async fn start(State(app): State<App>, Json(req): Json<ScanRequest>) -> impl IntoResponse {
    let mut s = app.status.lock().await;
    if s.running {
        return (
            StatusCode::CONFLICT,
            Json(json!({"error":"Scan already running"})),
        );
    }
    if req.roots.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error":"Add at least one directory"})),
        );
    }
    let c=db(&app);
    let auto_sort=c.query_row("SELECT value FROM settings WHERE key='library_auto_sort'",[],|r|r.get::<_,String>(0)).map(|v|v=="true").unwrap_or(true);
    let valid_target=req.library_id.is_some_and(|id|c.query_row("SELECT EXISTS(SELECT 1 FROM libraries WHERE id=?1)",[id],|r|r.get::<_,bool>(0)).unwrap_or(false));
    if req.library_id.is_some()&&!valid_target{return (StatusCode::BAD_REQUEST,Json(json!({"error":"The selected library no longer exists"}))) }
    if !auto_sort&&req.library_id.is_none(){return (StatusCode::BAD_REQUEST,Json(json!({"error":"Choose a library before scanning while auto-sort is off"}))) }
    let roots_json=serde_json::to_string(&req.roots).unwrap_or_else(|_|"[]".into());
    let _=c.execute("INSERT INTO settings(key,value) VALUES('library_scan_roots',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value",[roots_json]);
    *s = Job {
        running: true,
        phase: "Discovering files".into(),
        started_at: Some(Utc::now().to_rfc3339()),
        action: "Preparing scan".into(),
        ..Job::default()
    };
    s.log(if req.full_scan { "Full scan started; rechecking unlocked files" } else { "Incremental scan started; unchanged files will be skipped" });
    app.cancel.store(false, Ordering::Relaxed);
    drop(s);
    let full_scan=req.full_scan;
    tokio::task::spawn_blocking(move || {
        tokio::runtime::Handle::current().block_on(scan(app, req.roots, req.library_id, auto_sort, full_scan));
    });
    (StatusCode::OK, Json(json!({"ok":true})))
}
async fn cancel(State(app): State<App>) -> Json<Value> {
    app.cancel.store(true, Ordering::Relaxed);
    Json(json!({"ok":true}))
}
async fn status(State(app): State<App>) -> Json<Value> {
    let mut status = serde_json::to_value(app.status.lock().await.clone()).unwrap();
    status["availability_checking"] = json!(app.availability_running.load(Ordering::Acquire));
    status["availability_checked"] = json!(app.availability.read().unwrap().len());
    Json(status)
}
fn movie_ids_for_collections(app: &App) -> Vec<i64> {
    let c = db(app);
    let mut stmt = c.prepare("SELECT result FROM files").unwrap();
    let mut ids = HashSet::new();
    for raw in stmt
        .query_map([], |r| r.get::<_, String>(0))
        .unwrap()
        .flatten()
    {
        let result: Value = serde_json::from_str(&raw).unwrap_or_default();
        if result["kind"] == "movie"
            && result["provider"] == "tmdb"
            && result["collection_title"].is_null()
        {
            if let Some(id) = result["provider_id"].as_i64().filter(|id| *id > 0) {
                ids.insert(id);
            }
        }
    }
    let mut ids: Vec<_> = ids.into_iter().collect();
    ids.sort_unstable();
    ids
}
async fn fetch_movie_collections(app: App, ids: Vec<i64>) {
    let token = tmdb_token(&app);
    if !token.is_empty() {
        for id in ids {
            if !app.collections_attempted.lock().await.insert(id) {
                continue;
            }
            let key = format!("tmdb:movie-detail:{id}");
            let _ = cached(
                &app,
                &key,
                &format!("https://api.themoviedb.org/3/movie/{id}"),
                Some(&token),
            )
            .await;
            // Keep well below the provider's documented upper rate limit.
            tokio::time::sleep(Duration::from_millis(125)).await;
        }
    }
    app.collections_running.store(false, Ordering::Release);
}
async fn movie_collections(State(app): State<App>) -> Json<Value> {
    let ids = movie_ids_for_collections(&app);
    let pending: Vec<i64> = {
        let attempted = app.collections_attempted.lock().await;
        ids.iter()
            .copied()
            .filter(|id| !attempted.contains(id))
            .collect()
    };
    if !pending.is_empty()
        && !tmdb_token(&app).is_empty()
        && app
            .collections_running
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    {
        let task_app = app.clone();
        let task_ids = pending;
        tokio::spawn(async move {
            fetch_movie_collections(task_app, task_ids).await;
        });
    }
    let result = movie_collection_snapshot(&app, &ids);
    Json(json!({"collections":result,"loading":app.collections_running.load(Ordering::Acquire)}))
}

#[cfg(test)]
async fn playback_progress(State(app): State<App>, Query(query): Query<ProgressQuery>) -> Response {
    if let Some(path) = query.path {
        if !is_library_path(&app, &path) {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({"error":"File is not in the library"})),
            )
                .into_response();
        }
        let progress = db(&app)
            .query_row(
                "SELECT position,duration,updated_at FROM watch_progress WHERE path=?1 AND completed=0",
                [&path],
                |row| Ok(json!({"position":row.get::<_,f64>(0)?,"duration":row.get::<_,f64>(1)?,"updated_at":row.get::<_,i64>(2)?})),
            )
            .ok();
        let completed = db(&app)
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM watch_progress WHERE path=?1 AND completed=1)",
                [&path],
                |row| row.get::<_, bool>(0),
            )
            .unwrap_or(false);
        return Json(json!({"progress":progress,"completed":completed})).into_response();
    }
    let items = db(&app)
        .prepare("SELECT w.path,w.position,w.duration,w.updated_at,f.result FROM watch_progress w JOIN files f ON f.path=w.path WHERE w.completed=0 AND w.position>10 AND w.duration>w.position+10 ORDER BY w.updated_at DESC")
        .and_then(|mut statement| {
            statement.query_map([], |row| {
                let raw: String = row.get(4)?;
                let result: Value = serde_json::from_str(&raw).unwrap_or_default();
                Ok(json!({"path":row.get::<_,String>(0)?,"position":row.get::<_,f64>(1)?,"duration":row.get::<_,f64>(2)?,"updated_at":row.get::<_,i64>(3)?,"result":result}))
            })?.collect::<Result<Vec<_>,_>>()
        })
        .unwrap_or_default();
    let history = db(&app)
        .prepare("SELECT w.path,w.position,w.duration,w.updated_at,w.completed,f.result FROM watch_progress w JOIN files f ON f.path=w.path ORDER BY w.updated_at DESC LIMIT 200")
        .and_then(|mut statement| {
            statement.query_map([], |row| {
                let raw: String = row.get(5)?;
                let result: Value = serde_json::from_str(&raw).unwrap_or_default();
                Ok(json!({"path":row.get::<_,String>(0)?,"position":row.get::<_,f64>(1)?,"duration":row.get::<_,f64>(2)?,"updated_at":row.get::<_,i64>(3)?,"completed":row.get::<_,bool>(4)?,"result":result}))
            })?.collect::<Result<Vec<_>,_>>()
        })
        .unwrap_or_default();
    let watched_count = db(&app)
        .query_row(
            "SELECT COUNT(*) FROM watch_progress WHERE completed=1",
            [],
            |row| row.get::<_, i64>(0),
        )
        .unwrap_or(0);
    Json(json!({"items":items,"history":history,"watched_count":watched_count})).into_response()
}

pub(crate) fn is_library_path(app: &App, path: &str) -> bool {
    db(app)
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM files WHERE path=?1)",
            [path],
            |row| row.get(0),
        )
        .unwrap_or(false)
}

#[cfg(test)]
async fn update_playback_progress(
    State(app): State<App>,
    Json(update): Json<ProgressUpdate>,
) -> Response {
    if !is_library_path(&app, &update.path)
        || !update.position.is_finite()
        || !(0.0..=86_400.0).contains(&update.position)
        || !update.duration.is_finite()
        || !(0.0..=86_400.0).contains(&update.duration)
    {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error":"Invalid playback progress"})),
        )
            .into_response();
    }
    let complete =
        update.completed || (update.duration > 0.0 && update.position >= (update.duration - 8.0).max(update.duration * 0.9));
    let conn = db(&app);
    let result = if complete {
        conn.execute("INSERT INTO watch_progress(path,position,duration,updated_at,completed) VALUES(?1,?2,?3,?4,1) ON CONFLICT(path) DO UPDATE SET position=excluded.position,duration=excluded.duration,updated_at=excluded.updated_at,completed=1", params![update.path, update.duration, update.duration, Utc::now().timestamp()])
    } else {
        conn.execute("INSERT INTO watch_progress(path,position,duration,updated_at,completed) VALUES(?1,?2,?3,?4,0) ON CONFLICT(path) DO UPDATE SET position=excluded.position,duration=excluded.duration,updated_at=excluded.updated_at,completed=0", params![update.path, update.position, update.duration, Utc::now().timestamp()])
    };
    match result {
        Ok(_) => (
            StatusCode::OK,
            Json(json!({"ok":true,"completed":complete})),
        )
            .into_response(),
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error":"Could not save playback progress"})),
        )
            .into_response(),
    }
}
fn movie_collection_snapshot(app: &App, ids: &[i64]) -> serde_json::Map<String, Value> {
    let c = db(app);
    let mut result = serde_json::Map::new();
    for id in ids {
        let raw: Option<String> = c
            .query_row(
                "SELECT value FROM cache WHERE key=?1",
                [format!("tmdb:movie-detail:{id}")],
                |r| r.get(0),
            )
            .ok();
        if let Some(collection) = raw
            .and_then(|v| serde_json::from_str::<Value>(&v).ok())
            .and_then(|v| v.get("belongs_to_collection").cloned())
            .filter(|v| v["id"].as_i64().is_some() && v["name"].as_str().is_some())
        {
            result.insert(
                id.to_string(),
                json!({"id":collection["id"],"name":collection["name"]}),
            );
        }
    }
    result
}
// Embed only a read-only SQLite snapshot; do not probe the media drive here.
fn render_index(app: &App) -> String {
    let Json(files) = files_blocking(app);
    let collections = movie_collection_snapshot(app, &movie_ids_for_collections(app));
    let snapshot = json!({"files":files,"collections":collections})
        .to_string()
        .replace('<', "\\u003c")
        .replace('>', "\\u003e")
        .replace('&', "\\u0026");
    include_str!("../static/index.html").replace(
        "<!-- LIBRARY_BOOTSTRAP -->",
        &format!("<script id=\"library-bootstrap\" type=\"application/json\">{snapshot}</script>"),
    )
}
async fn hls_javascript() -> impl IntoResponse {
    (
        [
            (
                axum::http::header::CONTENT_TYPE,
                "application/javascript; charset=utf-8",
            ),
            (axum::http::header::CACHE_CONTROL, "public, max-age=604800"),
        ],
        include_str!("../static/vendor/hls.light.min.js"),
    )
}
async fn index(State(app): State<App>, headers: HeaderMap) -> Html<String> {
    if auth::principal(&app.db, &headers).is_none() {
        return auth::auth_page(State(app)).await;
    }
    let html = tokio::task::spawn_blocking(move || render_index(&app))
        .await
        .unwrap_or_else(|_| include_str!("../static/index.html").to_string());
    Html(html)
}
fn ensure_availability_check(app: &App, rows: &Value) {
    let paths: Vec<String> = rows
        .as_array()
        .into_iter()
        .flatten()
        .filter(|row| row["available"].is_null())
        .filter_map(|row| row["path"].as_str().map(str::to_owned))
        .collect();
    if paths.is_empty()
        || app
            .availability_running
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
    {
        return;
    }
    let task_app = app.clone();
    let generation = app.availability_generation.load(Ordering::Acquire);
    tokio::task::spawn_blocking(move || {
        for path in paths {
            if task_app.availability_generation.load(Ordering::Acquire) != generation {
                break;
            }
            let available = std::fs::metadata(&path).is_ok_and(|metadata| metadata.is_file());
            if task_app.availability_generation.load(Ordering::Acquire) != generation {
                break;
            }
            task_app
                .availability
                .write()
                .unwrap()
                .insert(path, available);
        }
        task_app
            .availability_running
            .store(false, Ordering::Release);
    });
}

async fn files(State(app): State<App>) -> Json<Value> {
    let snapshot_app = app.clone();
    let Json(rows) = tokio::task::spawn_blocking(move || files_blocking(&snapshot_app))
        .await
        .unwrap_or_else(|_| Json(json!([])));
    ensure_availability_check(&app, &rows);
    Json(rows)
}
// A database snapshot must never synchronously touch a slow or offline drive.
fn files_blocking(app: &App) -> Json<Value> {
    let c = db(app);
    let mut memberships:HashMap<String,Vec<i64>>=HashMap::new();
    if let Ok(mut map_stmt)=c.prepare("SELECT path,library_id FROM library_files") {
        if let Ok(rows)=map_stmt.query_map([],|r|Ok((r.get::<_,String>(0)?,r.get::<_,i64>(1)?))) {
            for (path,id) in rows.flatten(){memberships.entry(path).or_default().push(id);}
        }
    }
    let mut stmt = c
        .prepare("SELECT path,parsed,result,locked,size,mtime,scanned FROM files ORDER BY path")
        .unwrap();
    let availability = app.availability.read().unwrap().clone();
    let mut rows=stmt.query_map([],|r|{
        let path:String=r.get(0)?; let p:String=r.get(1)?;let v:String=r.get(2)?;
        Ok(json!({"available":availability.get(&path),"path":path,"parsed":serde_json::from_str::<Value>(&p).unwrap_or_default(),"result":serde_json::from_str::<Value>(&v).unwrap_or_default(),"locked":r.get::<_,i64>(3)?==1,"size":r.get::<_,i64>(4)?,"mtime":r.get::<_,i64>(5)?,"scanned":r.get::<_,String>(6)?,"library_ids":memberships.get(&path).cloned().unwrap_or_default()}))
    }).unwrap().flatten().collect::<Vec<_>>();
    let details:HashMap<String,Value>=c.prepare("SELECT key,value FROM cache WHERE key LIKE 'tmdb:movie-detail:%' OR key LIKE 'tvmaze:show:%'").and_then(|mut s|s.query_map([],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?)))?.collect::<Result<Vec<_>,_>>()).unwrap_or_default().into_iter().filter_map(|(k,v)|serde_json::from_str(&v).ok().map(|v|(k,v))).collect();
    for row in &mut rows {
        let result=&mut row["result"];
        let key=if result["kind"]=="movie" {format!("tmdb:movie-detail:{}",result["provider_id"])} else {format!("tvmaze:show:{}",result["provider_id"])};
        if let Some(detail)=details.get(&key) {
            let movie=result["kind"]=="movie";
            result["genres"]=if movie {json!(detail["genres"].as_array().into_iter().flatten().filter_map(|x|x["name"].as_str()).collect::<Vec<_>>())} else {detail["genres"].clone()};
            result["rating"]=if movie {detail["vote_average"].clone()} else {detail["rating"]["average"].clone()};
            result["runtime"]=detail["runtime"].clone();
            result["language"]=if movie {detail["original_language"].clone()} else {detail["language"].clone()};
        }
    }
    Json(json!(rows))
}

fn library_list(c:&Connection)->Vec<Value>{
    c.prepare("SELECT l.id,l.name,l.kind,l.roots,(SELECT COUNT(*) FROM library_files f WHERE f.library_id=l.id) FROM libraries l ORDER BY l.created_at,l.id").and_then(|mut s|s.query_map([],|r|{let raw:String=r.get(3)?;Ok(json!({"id":r.get::<_,i64>(0)?,"name":r.get::<_,String>(1)?,"kind":r.get::<_,String>(2)?,"roots":serde_json::from_str::<Value>(&raw).unwrap_or(json!([])),"file_count":r.get::<_,i64>(4)?}))})?.collect()).unwrap_or_default()
}
async fn get_libraries(State(app):State<App>)->Json<Value>{let c=db(&app);let auto_sort=c.query_row("SELECT value FROM settings WHERE key='library_auto_sort'",[],|r|r.get::<_,String>(0)).map(|v|v=="true").unwrap_or(true);Json(json!({"libraries":library_list(&c),"auto_sort":auto_sort}))}
async fn get_library_files(State(app): State<App>, AxPath(id): AxPath<i64>) -> Response {
    let c = db(&app);
    let exists: bool = c
        .query_row("SELECT EXISTS(SELECT 1 FROM libraries WHERE id=?1)", [id], |r| r.get(0))
        .unwrap_or(false);
    if !exists {
        return StatusCode::NOT_FOUND.into_response();
    }
    let Json(rows) = files_blocking(&app);
    ensure_availability_check(&app, &rows);
    let visible: Vec<Value> = rows
        .as_array()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter(|row| {
            row["library_ids"]
                .as_array()
                .is_some_and(|ids| ids.iter().any(|x| x.as_i64() == Some(id)))
        })
        .collect();
    Json(json!({"files":visible})).into_response()
}
async fn save_library(State(app):State<App>,Json(input):Json<LibraryInput>)->Response{
    let name=input.name.trim();if name.is_empty()||name.chars().count()>48||name.chars().any(char::is_control)||!matches!(input.kind.as_str(),"movie"|"tv"|"video"|"other"){return (StatusCode::BAD_REQUEST,Json(json!({"error":"Enter a 1–48 character name and a supported library type"}))).into_response()}
    let roots=input.roots.unwrap_or_default();if roots.len()>128||roots.iter().any(|x|x.trim().is_empty()||x.len()>1024||x.chars().any(char::is_control)){return (StatusCode::BAD_REQUEST,Json(json!({"error":"Library roots must be valid, non-empty folder paths"}))).into_response()}
    let roots=serde_json::to_string(&roots).unwrap_or_else(|_|"[]".into());let c=db(&app);
    let result=if let Some(id)=input.id{c.execute("UPDATE libraries SET name=?1,kind=?2,roots=?3 WHERE id=?4",params![name,input.kind,roots,id])}else{c.execute("INSERT INTO libraries(name,kind,roots,created_at) VALUES(?1,?2,?3,?4)",params![name,input.kind,roots,Utc::now().timestamp()])};
    match result{Ok(1)=>Json(json!({"ok":true})).into_response(),Ok(_)=>StatusCode::NOT_FOUND.into_response(),Err(_)=>(StatusCode::CONFLICT,Json(json!({"error":"A library with that name already exists"}))).into_response()}
}
async fn delete_library(State(app):State<App>,Json(input):Json<LibraryId>)->Response{
    let c=db(&app);let kind:Option<String>=c.query_row("SELECT kind FROM libraries WHERE id=?1",[input.id],|r|r.get(0)).ok();let Some(kind)=kind else{return StatusCode::NOT_FOUND.into_response()};let count:i64=c.query_row("SELECT COUNT(*) FROM libraries WHERE kind=?1",[kind.as_str()],|r|r.get(0)).unwrap_or(0);if count<=1{return (StatusCode::CONFLICT,Json(json!({"error":"Keep at least one library of each type; rename it if needed"}))).into_response()}
    let _=c.execute("UPDATE library_files SET library_id=(SELECT id FROM libraries WHERE kind=?1 AND id<>?2 ORDER BY id LIMIT 1) WHERE library_id=?2",params![kind,input.id]);
    match c.execute("DELETE FROM libraries WHERE id=?1",[input.id]){Ok(1)=>Json(json!({"ok":true,"media_kept":true})).into_response(),Ok(_)=>StatusCode::NOT_FOUND.into_response(),Err(_)=>StatusCode::INTERNAL_SERVER_ERROR.into_response()}
}
async fn get_library_settings(State(app):State<App>)->Json<Value>{let c=db(&app);let auto=c.query_row("SELECT value FROM settings WHERE key='library_auto_sort'",[],|r|r.get::<_,String>(0)).map(|v|v=="true").unwrap_or(true);let raw=c.query_row("SELECT value FROM settings WHERE key='library_scan_roots'",[],|r|r.get::<_,String>(0)).unwrap_or_else(|_|"[]".into());Json(json!({"auto_sort":auto,"scan_roots":serde_json::from_str::<Value>(&raw).unwrap_or(json!([]))}))}
async fn save_library_settings(State(app):State<App>,Json(input):Json<LibrarySettingsInput>)->Response{
    if input.scan_roots.len()>128||input.scan_roots.iter().any(|x|x.trim().is_empty()||x.len()>1024||x.chars().any(char::is_control)){return StatusCode::BAD_REQUEST.into_response()}
    let roots=serde_json::to_string(&input.scan_roots).unwrap_or_else(|_|"[]".into());let c=db(&app);
    if c.execute("INSERT INTO settings(key,value) VALUES('library_auto_sort',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value",[if input.auto_sort{"true"}else{"false"}]).is_err()||c.execute("INSERT INTO settings(key,value) VALUES('library_scan_roots',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value",[roots]).is_err(){return StatusCode::INTERNAL_SERVER_ERROR.into_response()}
    if input.auto_sort {let rows:Vec<(String,String)>=c.prepare("SELECT path,result FROM files").and_then(|mut s|s.query_map([],|r|Ok((r.get(0)?,r.get(1)?)))?.collect()).unwrap_or_default();for (path,raw) in rows{let result:Value=serde_json::from_str(&raw).unwrap_or_default();assign_scanned_file(&c,&path,&result,None,true);}}
    Json(json!({"ok":true})).into_response()
}

async fn update(State(app): State<App>, Json(req): Json<Value>) -> impl IntoResponse {
    let path = req["path"].as_str().unwrap_or("");
    let ov: Override = match serde_json::from_value(req["override"].clone()) {
        Ok(v) => v,
        Err(_) => return StatusCode::BAD_REQUEST,
    };
    let c = db(&app);
    let old: Result<String, _> =
        c.query_row("SELECT result FROM files WHERE path=?1", [path], |r| {
            r.get(0)
        });
    if let Ok(old) = old {
        let mut v: Value = serde_json::from_str(&old).unwrap_or(json!({}));
        v["kind"] = json!(ov.kind);
        v["provider_id"] = json!(ov.provider_id);
        v["season"] = json!(ov.season);
        if let Some(id) = ov.provider_id {
            v["provider_id"] = json!(id);
            v["provider"] = json!(if ov.kind == "movie" { "tmdb" } else { "tvmaze" });
        }
        if let Some(t) = ov.title {
            v["title"] = json!(t)
        }
        if let Some(s) = ov.season {
            v["season"] = json!(s)
        }
        if let Some(e) = ov.episodes {
            v["episodes"] = json!(e)
        }
        v["requires_review"] = json!(false);
        v["confidence"] = json!(100);
        v["explanation"] = json!("Manually confirmed and locked");
        let _ = c.execute(
            "UPDATE files SET result=?2,locked=1 WHERE path=?1",
            params![path, v.to_string()],
        );
        StatusCode::OK
    } else {
        StatusCode::NOT_FOUND
    }
}

#[derive(Deserialize)]
struct MetadataSearch {
    q: String,
    kind: String,
}
async fn poster_url(app: &App, path: &Value, token: &str) -> Option<String> {
    let path = path.as_str()?;
    let config = cached(
        app,
        "tmdb:configuration",
        "https://api.themoviedb.org/3/configuration",
        Some(token),
    )
    .await
    .ok()?;
    let base = config["images"]["secure_base_url"].as_str()?;
    let sizes = config["images"]["poster_sizes"].as_array()?;
    let size = sizes
        .iter()
        .filter_map(Value::as_str)
        .find(|s| *s == "w342")
        .or_else(|| sizes.first().and_then(Value::as_str))?;
    Some(format!("{base}{size}{path}"))
}
async fn metadata_search(
    State(app): State<App>,
    Query(q): Query<MetadataSearch>,
) -> (StatusCode, Json<Value>) {
    let title = q.q.trim();
    if title.is_empty() || title.len() > 200 || !["movie", "tv"].contains(&q.kind.as_str()) {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error":"Enter a title and choose movies or TV shows."})),
        );
    }
    let (key, url, token) = if q.kind == "movie" {
        let token = tmdb_token(&app);
        if token.is_empty() {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error":"Configure your TMDB token to search movies."})),
            );
        }
        (
            format!("tmdb:movie:{title}"),
            format!(
                "https://api.themoviedb.org/3/search/movie?query={}",
                urlencoding::encode(title)
            ),
            Some(token),
        )
    } else {
        (
            format!("tvmaze:search:{}", normalize(title)),
            format!(
                "https://api.tvmaze.com/search/shows?q={}",
                urlencoding::encode(title)
            ),
            None,
        )
    };
    match cached(&app, &key, &url, token.as_deref()).await {
        Err(_) => (
            StatusCode::BAD_GATEWAY,
            Json(json!({"error":"Metadata search is unavailable. Please try again."})),
        ),
        Ok(data) => {
            let items = if q.kind == "movie" {
                data["results"].as_array()
            } else {
                data.as_array()
            };
            let mut results = Vec::new();
            for entry in items.into_iter().flatten().take(20) {
                let item = if q.kind == "tv" {
                    &entry["show"]
                } else {
                    entry
                };
                if item["id"].as_i64().is_none() {
                    continue;
                }
                let poster = if q.kind == "movie" {
                    poster_url(&app, &item["poster_path"], token.as_deref().unwrap_or("")).await
                } else {
                    item["image"]["medium"].as_str().map(str::to_string)
                };
                results.push(json!({"id":item["id"],"title":if q.kind=="movie" { &item["title"] } else { &item["name"] },"year":if q.kind=="movie" { &item["release_date"] } else { &item["premiered"] },"poster":poster,"description":if q.kind=="movie" { &item["overview"] } else { &item["summary"] }}));
            }
            (StatusCode::OK, Json(json!({"results":results})))
        }
    }
}
#[derive(Deserialize)]
struct ManualMatch {
    path: String,
    kind: String,
    id: i64,
    season: Option<u32>,
    episode: Option<u32>,
}
async fn manual_match(
    State(app): State<App>,
    Json(req): Json<ManualMatch>,
) -> (StatusCode, Json<Value>) {
    if req.id <= 0 || !["movie", "tv"].contains(&req.kind.as_str()) {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error":"Invalid selection."})),
        );
    }
    let old: String =
        match db(&app).query_row("SELECT result FROM files WHERE path=?1", [&req.path], |r| {
            r.get(0)
        }) {
            Ok(v) => v,
            Err(_) => {
                return (
                    StatusCode::NOT_FOUND,
                    Json(json!({"error":"File no longer exists in the library."})),
                )
            }
        };
    let previous: Value = serde_json::from_str(&old).unwrap_or_default();
    let (detail, provider, poster) = if req.kind == "movie" {
        let token = tmdb_token(&app);
        if token.is_empty() {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error":"TMDB token is missing."})),
            );
        }
        let detail = match cached(
            &app,
            &format!("tmdb:movie-detail:{}", req.id),
            &format!("https://api.themoviedb.org/3/movie/{}", req.id),
            Some(&token),
        )
        .await
        {
            Ok(v) => v,
            Err(_) => {
                return (
                    StatusCode::BAD_GATEWAY,
                    Json(json!({"error":"Could not load this movie from TMDB."})),
                )
            }
        };
        let poster = poster_url(&app, &detail["poster_path"], &token).await;
        (detail, "tmdb", poster)
    } else {
        let (season, episode) = match (req.season, req.episode) {
            (Some(s), Some(e)) => (s, e),
            _ => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(json!({"error":"Choose an episode from the catalogue."})),
                )
            }
        };
        let detail = match cached(
            &app,
            &format!("tvmaze:show:{}", req.id),
            &format!("https://api.tvmaze.com/shows/{}", req.id),
            None,
        )
        .await
        {
            Ok(v) => v,
            Err(_) => {
                return (
                    StatusCode::BAD_GATEWAY,
                    Json(json!({"error":"Could not load this TV show."})),
                )
            }
        };
        let episodes = match cached(
            &app,
            &format!("tvmaze:episodes:{}", req.id),
            &format!(
                "https://api.tvmaze.com/shows/{}/episodes?specials=1",
                req.id
            ),
            None,
        )
        .await
        {
            Ok(v) => v,
            Err(_) => {
                return (
                    StatusCode::BAD_GATEWAY,
                    Json(json!({"error":"Could not verify this episode."})),
                )
            }
        };
        if !episodes.as_array().is_some_and(|list| {
            list.iter().any(|e| {
                e["season"].as_u64() == Some(season as u64)
                    && e["number"].as_u64() == Some(episode as u64)
            })
        }) {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error":"That episode is not in the show catalogue."})),
            );
        }
        let poster = detail["image"]["medium"].as_str().map(str::to_string);
        (detail, "tvmaze", poster)
    };
    let title = if req.kind == "movie" {
        &detail["title"]
    } else {
        &detail["name"]
    };
    if detail["id"].as_i64() != Some(req.id) || title.as_str().is_none_or(|s| s.is_empty()) {
        return (
            StatusCode::BAD_GATEWAY,
            Json(json!({"error":"The provider returned an invalid match."})),
        );
    }
    let result = json!({"kind":req.kind,"title":title,"provider":provider,"provider_id":req.id,"year":if req.kind=="movie" { &detail["release_date"] } else { &detail["premiered"] },"description":if req.kind=="movie" { &detail["overview"] } else { &detail["summary"] },"poster":poster,"season":req.season,"episodes":req.episode.into_iter().collect::<Vec<_>>(),"confidence":100,"requires_review":false,"explanation":"Manually selected and locked across rescans","subtitles":previous["subtitles"],"candidates":[],"order":"aired"});
    match db(&app).execute(
        "UPDATE files SET result=?2,locked=1 WHERE path=?1",
        params![req.path, result.to_string()],
    ) {
        Ok(1) => (StatusCode::OK, Json(json!({"ok":true}))),
        _ => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error":"Could not save the match."})),
        ),
    }
}
fn subtitle_key(app: &App) -> String {
    std::env::var("OPENSUBTITLES_API_KEY")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .or_else(|| {
            app.db
                .parent()
                .and_then(|p| std::fs::read_to_string(p.join("opensubtitles-api-key")).ok())
        })
        .unwrap_or_default()
        .trim()
        .to_owned()
}
async fn subtitle_status(State(app): State<App>) -> Json<Value> {
    Json(json!({"configured":!subtitle_key(&app).is_empty()}))
}
#[derive(Deserialize)]
struct SubtitleKey {
    key: String,
}
fn save_private_provider_key(app: &App, filename: &str, key: &str) -> std::io::Result<()> {
    let path=app.db.parent().unwrap_or(Path::new(".")).join(filename);
    if key.is_empty(){return match std::fs::remove_file(path){Ok(())=>Ok(()),Err(e)if e.kind()==std::io::ErrorKind::NotFound=>Ok(()),Err(e)=>Err(e)}}
    let mut opts=std::fs::OpenOptions::new();opts.write(true).create(true).truncate(true);
    #[cfg(unix)]{use std::os::unix::fs::OpenOptionsExt;opts.mode(0o600);}
    let mut file=opts.open(path)?;
    #[cfg(unix)]{use std::os::unix::fs::PermissionsExt;file.set_permissions(std::fs::Permissions::from_mode(0o600))?;}
    use std::io::Write;file.write_all(key.as_bytes())
}
async fn provider_status(State(app): State<App>) -> Json<Value> {
    Json(json!({"tmdb_configured":!tmdb_token(&app).is_empty(),"opensubtitles_configured":!subtitle_key(&app).is_empty(),"tmdb_environment_override":std::env::var("TMDB_TOKEN").ok().is_some_and(|x|!x.trim().is_empty()),"opensubtitles_environment_override":std::env::var("OPENSUBTITLES_API_KEY").ok().is_some_and(|x|!x.trim().is_empty())}))
}
async fn save_tmdb_key(State(app): State<App>, Json(input): Json<SubtitleKey>) -> (StatusCode, Json<Value>) {
    let key=input.key.trim();
    if key.len()>4096||key.chars().any(char::is_whitespace){return (StatusCode::BAD_REQUEST,Json(json!({"error":"Enter a valid TMDB Read Access Token."})))}
    match save_private_provider_key(&app,"tmdb-token",key){Ok(())=>(StatusCode::OK,Json(json!({"ok":true}))),Err(_)=>(StatusCode::INTERNAL_SERVER_ERROR,Json(json!({"error":"Could not save the token locally."})))}
}
async fn save_subtitle_key(
    State(app): State<App>,
    Json(input): Json<SubtitleKey>,
) -> (StatusCode, Json<Value>) {
    let key = input.key.trim();
    if key.len() > 256 || key.chars().any(char::is_whitespace) {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error":"Enter a valid application API key."})),
        );
    }
    let result=save_private_provider_key(&app,"opensubtitles-api-key",key);
    if result.is_ok() {
        (StatusCode::OK, Json(json!({"ok":true})))
    } else {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error":"Could not save the key locally."})),
        )
    }
}
#[derive(Deserialize)]
struct SubtitleSearch {
    q: String,
    language: Option<String>,
}
async fn subtitle_search(
    State(app): State<App>,
    Query(q): Query<SubtitleSearch>,
) -> (StatusCode, Json<Value>) {
    let key = subtitle_key(&app);
    if key.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error":"Add an OpenSubtitles application API key to search online."})),
        );
    }
    let title = q.q.trim();
    if title.is_empty() || title.len() > 200 {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error":"Enter a title to search."})),
        );
    }
    let language = q.language.unwrap_or_default();
    if !language.is_empty()
        && (language.len() > 8
            || !language
                .chars()
                .all(|c| c.is_ascii_alphabetic() || c == '-'))
    {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error":"Invalid language."})),
        );
    }
    let mut request = app
        .client
        .get("https://api.opensubtitles.com/api/v1/subtitles")
        .header("Api-Key", key)
        .query(&[("query", title)]);
    if !language.is_empty() {
        request = request.query(&[("languages", language.as_str())]);
    }
    match request.send().await {
        Ok(response) if response.status().is_success() => match response.json::<Value>().await {
            Ok(data) => {
                let results:Vec<Value>=data["data"].as_array().into_iter().flatten().take(50).map(|entry| { let a=&entry["attributes"]; json!({"language":a["language"],"release":a["release"],"file":a["files"][0]["file_name"],"rating":a["ratings"],"votes":a["votes"],"downloads":a["download_count"],"hearing_impaired":a["hearing_impaired"],"url":a["url"]}) }).collect();
                (StatusCode::OK, Json(json!({"results":results})))
            }
            Err(_) => (
                StatusCode::BAD_GATEWAY,
                Json(json!({"error":"OpenSubtitles returned an invalid response."})),
            ),
        },
        Ok(response)
            if response.status() == StatusCode::UNAUTHORIZED
                || response.status() == StatusCode::FORBIDDEN =>
        {
            (
                StatusCode::BAD_GATEWAY,
                Json(
                    json!({"error":"OpenSubtitles rejected the API key. Check your application key."}),
                ),
            )
        }
        _ => (
            StatusCode::BAD_GATEWAY,
            Json(json!({"error":"OpenSubtitles is unavailable. Please try again."})),
        ),
    }
}
async fn catalogue(State(app): State<App>, AxPath(id): AxPath<i64>) -> impl IntoResponse {
    let episodes_url = format!("https://api.tvmaze.com/shows/{id}/episodes?specials=1");
    let seasons_url = format!("https://api.tvmaze.com/shows/{id}/seasons");
    let episodes_key = format!("tvmaze:episodes:{id}");
    let seasons_key = format!("tvmaze:seasons:{id}");
    let (episodes_result, seasons_result) = tokio::join!(
        cached(&app, &episodes_key, &episodes_url, None),
        cached(&app, &seasons_key, &seasons_url, None),
    );
    match episodes_result {
        Ok(episodes) => {
            let saved: i64 = db(&app)
                .query_row("SELECT saved FROM cache WHERE key=?1", [&episodes_key], |r| r.get(0))
                .unwrap_or(0);
            let seasons = seasons_result.unwrap_or_else(|_| json!([]));
            (
                StatusCode::OK,
                Json(json!({
                    "episodes": episodes,
                    "seasons": seasons,
                    "today": Local::now().format("%Y-%m-%d").to_string(),
                    "verified": Utc::now().timestamp() - saved < 86400 * 7
                })),
            )
        }
        Err(e) => (
            StatusCode::BAD_GATEWAY,
            Json(json!({"error":e,"verified":false})),
        ),
    }
}

#[derive(Deserialize)]
struct MetadataDetailQuery { kind: String, id: i64 }
async fn metadata_detail(State(app): State<App>, axum::extract::Query(q): axum::extract::Query<MetadataDetailQuery>) -> impl IntoResponse {
    if q.id<=0||!matches!(q.kind.as_str(),"movie"|"tv") { return (StatusCode::BAD_REQUEST,Json(json!({"error":"Invalid metadata request."}))); }
    if q.kind=="movie" {
        let token=tmdb_token(&app);
        if token.is_empty() { return (StatusCode::OK,Json(json!({"rating":null,"runtime":null,"poster":null,"description":null}))); }
        let detail=match cached(&app,&format!("tmdb:movie-detail:{}",q.id),&format!("https://api.themoviedb.org/3/movie/{}",q.id),Some(&token)).await {Ok(v)=>v,Err(_)=>return (StatusCode::BAD_GATEWAY,Json(json!({"error":"Could not load movie information."})))};
        let poster=poster_url(&app,&detail["poster_path"],&token).await;
        return (StatusCode::OK,Json(json!({"title":detail["title"],"year":detail["release_date"],"description":detail["overview"],"rating":detail["vote_average"],"rating_count":detail["vote_count"],"runtime":detail["runtime"],"poster":poster})));
    }
    let detail=match cached(&app,&format!("tvmaze:show:{}",q.id),&format!("https://api.tvmaze.com/shows/{}",q.id),None).await {Ok(v)=>v,Err(_)=>return (StatusCode::BAD_GATEWAY,Json(json!({"error":"Could not load series information."})))};
    (StatusCode::OK,Json(json!({"title":detail["name"],"year":detail["premiered"],"description":detail["summary"],"rating":detail["rating"]["average"],"rating_count":null,"runtime":detail["averageRuntime"].as_i64().or_else(||detail["runtime"].as_i64()),"poster":detail["image"]["original"].as_str().or_else(||detail["image"]["medium"].as_str())})))
}
fn app_data_directory() -> PathBuf {
    std::env::var_os("SMART_MEDIA_DATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            dirs::data_local_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("smart-media-scanner")
        })
}
#[cfg(unix)]
struct TerminalEcho;
#[cfg(unix)]
impl Drop for TerminalEcho {
    fn drop(&mut self) { let _=std::process::Command::new("stty").arg("echo").status(); }
}
fn read_hidden_password(prompt:&str)->Result<String,String>{
    use std::io::{self,Write};
    print!("{prompt}");let _=io::stdout().flush();
    #[cfg(unix)]
    let _echo= {
        let status=std::process::Command::new("stty").arg("-echo").status().map_err(|e|format!("Could not hide terminal input: {e}"))?;
        if !status.success(){return Err("Run this command in an interactive terminal so password input can be hidden.".into())}
        TerminalEcho
    };
    #[cfg(not(unix))]
    return Err("Hidden password entry is currently supported by the macOS/Linux shell recovery command.".into());
    let mut value=String::new();let result=io::stdin().read_line(&mut value).map_err(|e|e.to_string());
    #[cfg(unix)]
    { drop(_echo);println!(); }
    result?;Ok(value.trim_end_matches(['\r','\n']).to_string())
}
async fn reset_owner_password_cli()->Result<(),String>{
    use std::io::{self,Write};
    let db_path=app_data_directory().join("library.sqlite3");
    if !db_path.is_file(){return Err(format!("No existing media-server database at {}",db_path.display()))}
    print!("New owner email: ");let _=io::stdout().flush();let mut email=String::new();io::stdin().read_line(&mut email).map_err(|e|e.to_string())?;
    let first=read_hidden_password("New owner password (12+ characters): ")?;
    let second=read_hidden_password("Confirm new password: ")?;
    if first!=second{return Err("Passwords did not match; no changes were made.".into())}
    auth::init(&db_path);
    auth::reset_owner_credentials(&db_path,email.trim(),&first)?;
    println!("Owner email and password updated. Existing owner sessions were signed out.");
    Ok(())
}
#[tokio::main]
async fn main() {
    if std::env::args().nth(1).as_deref()==Some("reset-owner-password") {
        if let Err(error)=reset_owner_password_cli().await{eprintln!("Owner credential reset failed: {error}");std::process::exit(1)}
        return;
    }
    let dir = app_data_directory();
    std::fs::create_dir_all(&dir).expect("create data directory");
    let app = App {
        db: dir.join("library.sqlite3"),
        client: Client::builder()
            .timeout(Duration::from_secs(12))
            .user_agent("SmartMediaScanner/0.1")
            .build()
            .unwrap(),
        status: Arc::new(Mutex::new(Job::default())),
        cancel: Arc::new(AtomicBool::new(false)),
        collections_running: Arc::new(AtomicBool::new(false)),
        collections_attempted: Arc::new(Mutex::new(HashSet::new())),
        availability: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
        availability_running: Arc::new(AtomicBool::new(false)),
        availability_generation: Arc::new(std::sync::atomic::AtomicU64::new(0)),
    };
    init(&db(&app));
    auth::init(&app.db);
    repair_bonus_records(&db(&app));
    let auth_db = app.db.clone();
    let routes = Router::new()
        .route("/", get(index))
        .route("/help", get(||async {Html(include_str!("../static/help.html"))}))
        .route("/manifest.webmanifest", get(||async { ([(axum::http::header::CONTENT_TYPE,"application/manifest+json")],include_str!("../static/manifest.webmanifest")) }))
        .route("/sw.js", get(||async { ([(axum::http::header::CONTENT_TYPE,"application/javascript"),(axum::http::header::CACHE_CONTROL,"no-cache")],include_str!("../static/sw.js")) }))
        .route("/offline.html", get(||async {Html(include_str!("../static/offline.html"))}))
        .route("/assets/icon-192.png", get(||async { ([(axum::http::header::CONTENT_TYPE,"image/png")],include_bytes!("../static/icons/icon-192.png").as_slice()) }))
        .route("/assets/icon-512.png", get(||async { ([(axum::http::header::CONTENT_TYPE,"image/png")],include_bytes!("../static/icons/icon-512.png").as_slice()) }))
        .route("/api/auth/setup-status", get(auth::setup_status))
        .route("/api/auth/setup", post(auth::setup))
        .route("/api/auth/login", post(auth::login))
        .route("/api/auth/logout", post(auth::logout))
        .route("/api/auth/sessions", get(auth::sessions_get).post(auth::sessions_post))
        .route("/api/auth/me", get(auth::me))
        .route("/api/auth/profile", post(auth::switch_profile))
        .route("/api/profiles", post(auth::profiles))
        .route("/api/server/info", get(auth::server_info))
        .route("/api/libraries", get(get_libraries))
        .route("/api/libraries/{id}/files", get(get_library_files))
        .route("/api/admin/libraries", get(get_libraries).post(save_library))
        .route("/api/admin/libraries/delete", post(delete_library))
        .route("/api/admin/library-settings", get(get_library_settings).post(save_library_settings))
        .route("/api/admin/accounts", get(auth::admin_accounts).post(auth::create_account))
        .route("/api/admin/health", get(diagnostics::health))
        .route("/api/admin/dashboard", get(auth::admin_dashboard))
        .route("/api/owner/settings", get(auth::server_settings).post(auth::save_server_setting))
        .route("/api/owner/encoders", get(auth::owner_encoders))
        .route("/api/owner/accounts/credentials", post(auth::update_account_credentials))
        .route("/api/owner/providers", get(provider_status))
        .route("/api/owner/providers/tmdb", post(save_tmdb_key))
        .route("/api/owner/providers/opensubtitles", post(save_subtitle_key))
        .route("/api/user/settings", get(auth::user_settings).post(auth::save_user_setting))
        .route("/api/watchlist", get(auth::watchlist_get))
        .route("/api/watchlist/toggle", post(auth::watchlist_toggle))
        .route("/api/playback/watched", get(auth::watched_get).post(auth::watched_post))
        .route("/api/playback/activity", post(auth::record_activity))
        .route("/assets/hls-1.7.3.js", get(hls_javascript))
        .route("/api/files", get(files))
        .route("/api/movie-collections", get(movie_collections))
        .route(
            "/api/playback/progress",
            get(auth::progress_get).post(auth::progress_post),
        )
        .route("/api/scan", post(start))
        .route("/api/choose-folder", post(choose_folder))
        .route("/api/cancel", post(cancel))
        .route("/api/status", get(status))
        .route("/api/override", post(update))
        .route("/api/metadata/search", get(metadata_search))
        .route("/api/metadata/detail", get(metadata_detail))
        .route("/api/metadata/match", post(manual_match))
        .route("/api/subtitles/status", get(subtitle_status))
        .route("/api/subtitles/key", post(save_subtitle_key))
        .route("/api/subtitles/search", get(subtitle_search))
        .route("/api/catalogue/{id}", get(catalogue))
        .route("/api/playback/thumbnail", get(media_features::thumbnail))
        .route("/api/playback/info", get(playback::info))
        .route("/api/playback/hls/start", post(hls::start))
        .route("/api/playback/hls/stop", post(hls::stop))
        .route("/api/playback/hls/{id}/{file}", get(hls::file))
        .route("/api/playback/file", get(playback::direct))
        .route("/api/playback/subtitle", get(playback::subtitle))
        .route("/api/playback/transcode", get(playback::transcode))
        .route_layer(middleware::from_fn_with_state(auth_db, auth::guard))
        .with_state(app);
    let port = std::env::var("SMART_MEDIA_PORT")
        .ok()
        .and_then(|p| p.parse::<u16>().ok())
        .unwrap_or(8787);
    let host = std::env::var("SMART_MEDIA_HOST").ok().and_then(|h| h.parse::<std::net::IpAddr>().ok()).unwrap_or(std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST));
    let listener = tokio::net::TcpListener::bind((host, port))
        .await
        .expect("bind configured address");
    println!("Smart Media Scanner: http://{host}:{port}");
    axum::serve(listener, routes).await.unwrap();
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    fn fixture() -> (tempfile::TempDir, App) {
        let temp = tempfile::tempdir().unwrap();
        let app = App {
            db: temp.path().join("db.sqlite"),
            client: Client::builder()
                .timeout(Duration::from_millis(100))
                .build()
                .unwrap(),
            status: Arc::new(Mutex::new(Job::default())),
            cancel: Arc::new(AtomicBool::new(false)),
            collections_running: Arc::new(AtomicBool::new(false)),
            collections_attempted: Arc::new(Mutex::new(HashSet::new())),
            availability: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
            availability_running: Arc::new(AtomicBool::new(false)),
            availability_generation: Arc::new(std::sync::atomic::AtomicU64::new(0)),
        };
        init(&db(&app));
        let show =
            json!([{ "show": {"id": 101, "name":"Regular Show", "premiered":"2010-01-01"} }]);
        let eps = json!([
            {"season":1,"number":1,"name":"The Power","airdate":"2010-01-01"},
            {"season":1,"number":2,"name":"Just Set Up the Chairs","airdate":"2010-01-02"},
            {"season":1,"number":3,"name":"Repeated","airdate":"2010-01-03"},
            {"season":2,"number":1,"name":"Repeated","airdate":"2011-01-01"}
        ]);
        for (key, value) in [
            ("tvmaze:search:regular show", show),
            ("tvmaze:episodes:101", eps),
        ] {
            db(&app)
                .execute(
                    "INSERT INTO cache VALUES(?1,?2,?3)",
                    params![key, value.to_string(), Utc::now().timestamp()],
                )
                .unwrap();
        }
        (temp, app)
    }
    #[test]
    fn transcode_quality_balance_changes_bitrate_for_both_stream_modes() {
        let (_temp, app) = fixture();
        auth::init(&app.db);
        assert_eq!(playback::video_bitrate(&app, "720"), Some((1280, 720, "3000k".into())));
        db(&app).execute("INSERT INTO server_settings(key,value) VALUES('transcode_quality','speed')", []).unwrap();
        assert_eq!(playback::video_bitrate(&app, "720"), Some((1280, 720, "2100k".into())));
        db(&app).execute("UPDATE server_settings SET value='quality' WHERE key='transcode_quality'", []).unwrap();
        assert_eq!(playback::video_bitrate(&app, "720"), Some((1280, 720, "4500k".into())));
        assert!(playback::video_bitrate(&app, "4k").is_none());
    }
    #[tokio::test]
    async fn account_passwords_require_confirmation_and_owner_reauthentication() {
        let (_temp, app) = fixture();
        auth::init(&app.db);
        let setup = |password: &str, confirm: &str| auth::Credentials {
            email: "owner@example.com".into(), password: password.into(),
            confirm_password: Some(confirm.into()), display_name: Some("Owner".into()),
        };
        let mismatch = auth::setup(State(app.clone()), Json(setup("owner password 123", "different pass 456"))).await;
        assert_eq!(mismatch.status(), StatusCode::BAD_REQUEST);
        let created = auth::setup(State(app.clone()), Json(setup("original owner password", "original owner password"))).await;
        assert_eq!(created.status(), StatusCode::OK);
        let c=db(&app);
        let (id,profile):(i64,i64)=c.query_row("SELECT a.id,p.id FROM accounts a JOIN profiles p ON p.account_id=a.id WHERE a.role='owner'",[],|r|Ok((r.get(0)?,r.get(1)?))).unwrap();
        let actor=auth::Principal{account_id:id,username:"Owner".into(),email:"owner@example.com".into(),role:"owner".into(),profile_id:profile,profile_name:"Default".into()};
        let update=|password:&str,confirm:&str,current:Option<&str>|auth::OwnerCredentialsInput{account_id:id,email:"owner@example.com".into(),password:password.into(),confirm_password:confirm.into(),current_owner_password:current.map(str::to_string)};
        let mismatch=auth::update_account_credentials(State(app.clone()),axum::Extension(actor.clone()),Json(update("replacement owner password","different replacement",Some("original owner password")))).await;
        assert_eq!(mismatch.status(),StatusCode::BAD_REQUEST);
        let no_reauth=auth::update_account_credentials(State(app.clone()),axum::Extension(actor.clone()),Json(update("replacement owner password","replacement owner password",None))).await;
        assert_eq!(no_reauth.status(),StatusCode::FORBIDDEN);
        let wrong_reauth=auth::update_account_credentials(State(app.clone()),axum::Extension(actor.clone()),Json(update("replacement owner password","replacement owner password",Some("wrong current password")))).await;
        assert_eq!(wrong_reauth.status(),StatusCode::FORBIDDEN);
        let changed=auth::update_account_credentials(State(app.clone()),axum::Extension(actor.clone()),Json(update("replacement owner password","replacement owner password",Some("original owner password")))).await;
        assert_eq!(changed.status(),StatusCode::OK);
        let login=auth::login(State(app.clone()),Json(auth::Credentials{email:"owner@example.com".into(),password:"replacement owner password".into(),confirm_password:None,display_name:None})).await;
        assert_eq!(login.status(),StatusCode::OK);
        let create=|password:&str,confirm:&str|auth::AccountInput{email:"viewer@example.com".into(),password:password.into(),confirm_password:confirm.into(),display_name:Some("Viewer".into()),role:Some("user".into())};
        let mismatch=auth::create_account(State(app.clone()),axum::Extension(actor.clone()),Json(create("viewer password 123","not the same"))).await;
        assert_eq!(mismatch.status(),StatusCode::BAD_REQUEST);
        let created=auth::create_account(State(app.clone()),axum::Extension(actor),Json(create("viewer password 123","viewer password 123"))).await;
        assert_eq!(created.status(),StatusCode::OK);
    }

    #[tokio::test]
    async fn profile_creation_and_switching_require_a_verified_pin() {
        let (_temp, app) = fixture();
        auth::init(&app.db);
        let setup=auth::setup(State(app.clone()),Json(auth::Credentials{email:"owner@example.com".into(),password:"owner password 123".into(),confirm_password:Some("owner password 123".into()),display_name:Some("Owner".into())})).await;
        assert_eq!(setup.status(),StatusCode::OK);
        let cookie=setup.headers().get(axum::http::header::SET_COOKIE).unwrap().to_str().unwrap().split(';').next().unwrap();
        let mut headers=HeaderMap::new(); headers.insert(axum::http::header::COOKIE,axum::http::HeaderValue::from_str(cookie).unwrap());
        let mismatch=auth::profiles(State(app.clone()),headers.clone(),Json(auth::ProfileInput{name:"Kids".into(),pin:"1234".into(),confirm_pin:"4321".into()})).await;
        assert_eq!(mismatch.status(),StatusCode::BAD_REQUEST);
        let created=auth::profiles(State(app.clone()),headers.clone(),Json(auth::ProfileInput{name:"Kids".into(),pin:"1234".into(),confirm_pin:"1234".into()})).await;
        assert_eq!(created.status(),StatusCode::OK);
        let id:i64=db(&app).query_row("SELECT id FROM profiles WHERE name='Kids'",[],|r|r.get(0)).unwrap();
        let wrong=auth::switch_profile(State(app.clone()),headers.clone(),Json(auth::ProfileSwitchInput{profile_id:id,pin:"9999".into(),confirm_pin:None})).await;
        assert_eq!(wrong.status(),StatusCode::FORBIDDEN);
        assert_eq!(auth::principal(&app.db,&headers).unwrap().profile_name,"Default");
        let switched=auth::switch_profile(State(app.clone()),headers.clone(),Json(auth::ProfileSwitchInput{profile_id:id,pin:"1234".into(),confirm_pin:None})).await;
        assert_eq!(switched.status(),StatusCode::OK);
        assert_eq!(auth::principal(&app.db,&headers).unwrap().profile_name,"Kids");
        let legacy_id:i64={let c=db(&app); c.execute("INSERT INTO profiles(account_id,name,created_at) VALUES(1,'Legacy',?1)",[Utc::now().timestamp()]).unwrap(); c.last_insert_rowid()};
        let no_confirmation=auth::switch_profile(State(app.clone()),headers.clone(),Json(auth::ProfileSwitchInput{profile_id:legacy_id,pin:"2345".into(),confirm_pin:None})).await;
        assert_eq!(no_confirmation.status(),StatusCode::BAD_REQUEST);
        let first_setup=auth::switch_profile(State(app.clone()),headers,Json(auth::ProfileSwitchInput{profile_id:legacy_id,pin:"2345".into(),confirm_pin:Some("2345".into())})).await;
        assert_eq!(first_setup.status(),StatusCode::OK);
    }

    #[tokio::test]
    async fn watchlist_is_persisted_separately_for_each_profile() {
        let (_temp, app) = fixture();
        auth::init(&app.db);
        let setup=auth::setup(State(app.clone()),Json(auth::Credentials{email:"owner@example.com".into(),password:"owner password 123".into(),confirm_password:Some("owner password 123".into()),display_name:Some("Owner".into())})).await;
        let cookie=setup.headers().get(axum::http::header::SET_COOKIE).unwrap().to_str().unwrap().split(';').next().unwrap();
        let mut headers=HeaderMap::new(); headers.insert(axum::http::header::COOKIE,axum::http::HeaderValue::from_str(cookie).unwrap());
        let path="/Media/Example Movie.mkv";
        db(&app).execute("INSERT INTO files(path,root,size,mtime,parsed,result,scanned) VALUES(?1,'/Media',100,0,'{}',?2,'now')",params![path,json!({"kind":"movie","title":"Example Movie","provider":"tmdb","provider_id":22}).to_string()]).unwrap();
        let add=auth::watchlist_toggle(State(app.clone()),headers.clone(),Json(auth::WatchlistInput{path:path.into(),enabled:true})).await;
        assert_eq!(add.status(),StatusCode::OK);
        assert_eq!(db(&app).query_row("SELECT COUNT(*) FROM profile_watchlist WHERE profile_id=1",[],|r|r.get::<_,i64>(0)).unwrap(),1);
        let created=auth::profiles(State(app.clone()),headers.clone(),Json(auth::ProfileInput{name:"Guest".into(),pin:"2468".into(),confirm_pin:"2468".into()})).await;
        assert_eq!(created.status(),StatusCode::OK);
        let guest:i64=db(&app).query_row("SELECT id FROM profiles WHERE name='Guest'",[],|r|r.get(0)).unwrap();
        let switched=auth::switch_profile(State(app.clone()),headers.clone(),Json(auth::ProfileSwitchInput{profile_id:guest,pin:"2468".into(),confirm_pin:None})).await;
        assert_eq!(switched.status(),StatusCode::OK);
        let other=auth::watchlist_get(State(app.clone()),headers.clone()).await;
        assert_eq!(other.status(),StatusCode::OK);
        assert_eq!(db(&app).query_row("SELECT COUNT(*) FROM profile_watchlist WHERE profile_id=?1",[guest],|r|r.get::<_,i64>(0)).unwrap(),0);
        let removed=auth::watchlist_toggle(State(app.clone()),headers,Json(auth::WatchlistInput{path:path.into(),enabled:false})).await;
        assert_eq!(removed.status(),StatusCode::OK);
        assert_eq!(db(&app).query_row("SELECT COUNT(*) FROM profile_watchlist WHERE profile_id=1",[],|r|r.get::<_,i64>(0)).unwrap(),1);
    }

    #[tokio::test]
    async fn libraries_can_be_named_and_auto_sort_preserves_movie_tv_types() {
        let (_temp, app) = fixture();
        let created=save_library(State(app.clone()),Json(LibraryInput{id:None,name:"YouTube videos".into(),kind:"video".into(),roots:None})).await;
        assert_eq!(created.status(),StatusCode::OK);
        let c=db(&app);
        let video_id:i64=c.query_row("SELECT id FROM libraries WHERE name='YouTube videos'",[],|r|r.get(0)).unwrap();
        let movie_id:i64=c.query_row("SELECT id FROM libraries WHERE kind='movie'",[],|r|r.get(0)).unwrap();
        let show_id:i64=c.query_row("SELECT id FROM libraries WHERE kind='tv'",[],|r|r.get(0)).unwrap();
        assign_scanned_file(&c,"/Media/movie.mp4",&json!({"kind":"movie"}),Some(video_id),true);
        assign_scanned_file(&c,"/Media/episode.mp4",&json!({"kind":"tv"}),Some(video_id),true);
        assign_scanned_file(&c,"/Media/clip.mp4",&json!({"kind":"video"}),Some(video_id),true);
        assert_eq!(c.query_row("SELECT library_id FROM library_files WHERE path='/Media/movie.mp4'",[],|r|r.get::<_,i64>(0)).unwrap(),movie_id);
        assert_eq!(c.query_row("SELECT library_id FROM library_files WHERE path='/Media/episode.mp4'",[],|r|r.get::<_,i64>(0)).unwrap(),show_id);
        assert_eq!(c.query_row("SELECT library_id FROM library_files WHERE path='/Media/clip.mp4'",[],|r|r.get::<_,i64>(0)).unwrap(),video_id);
        assign_scanned_file(&c,"/Media/manual.mp4",&json!({"kind":"tv"}),Some(video_id),false);
        assert_eq!(c.query_row("SELECT library_id FROM library_files WHERE path='/Media/manual.mp4'",[],|r|r.get::<_,i64>(0)).unwrap(),video_id);
    }

    #[tokio::test]
    async fn playback_progress_persists_resumes_and_marks_completed_items() {
        let (_temp, app) = fixture();
        db(&app).execute(
            "INSERT INTO files(path,root,size,mtime,parsed,result,scanned) VALUES('/Media/movie.mp4','/Media',1,0,'{}',?1,'now')",
            [json!({"kind":"movie","title":"Example"}).to_string()],
        ).unwrap();
        let saved = update_playback_progress(
            State(app.clone()),
            Json(ProgressUpdate {
                path: "/Media/movie.mp4".into(),
                position: 42.0,
                duration: 120.0,
                completed: false,
            }),
        )
        .await;
        assert_eq!(saved.status(), StatusCode::OK);
        let resume = playback_progress(
            State(app.clone()),
            Query(ProgressQuery {
                path: Some("/Media/movie.mp4".into()),
            }),
        )
        .await;
        let resume: Value = serde_json::from_slice(
            &axum::body::to_bytes(resume.into_body(), 4096)
                .await
                .unwrap(),
        )
        .unwrap();
        assert_eq!(resume["progress"]["position"], 42.0);
        assert!(!resume["completed"].as_bool().unwrap());

        let completed = update_playback_progress(
            State(app.clone()),
            Json(ProgressUpdate {
                path: "/Media/movie.mp4".into(),
                position: 118.0,
                duration: 120.0,
                completed: false,
            }),
        )
        .await;
        assert_eq!(completed.status(), StatusCode::OK);
        let resume = playback_progress(
            State(app.clone()),
            Query(ProgressQuery {
                path: Some("/Media/movie.mp4".into()),
            }),
        )
        .await;
        let resume: Value = serde_json::from_slice(
            &axum::body::to_bytes(resume.into_body(), 4096)
                .await
                .unwrap(),
        )
        .unwrap();
        assert!(resume["progress"].is_null());
        assert!(resume["completed"].as_bool().unwrap());

        let history =
            playback_progress(State(app.clone()), Query(ProgressQuery { path: None })).await;
        let history: Value = serde_json::from_slice(
            &axum::body::to_bytes(history.into_body(), 4096)
                .await
                .unwrap(),
        )
        .unwrap();
        assert_eq!(history["watched_count"], 1);

        let missing = playback_progress(
            State(app.clone()),
            Query(ProgressQuery {
                path: Some("/outside/library.mp4".into()),
            }),
        )
        .await;
        assert_eq!(missing.status(), StatusCode::NOT_FOUND);
        let invalid = update_playback_progress(
            State(app),
            Json(ProgressUpdate {
                path: "/outside/library.mp4".into(),
                position: 30.0,
                duration: 120.0,
                completed: false,
            }),
        )
        .await;
        assert_eq!(invalid.status(), StatusCode::BAD_REQUEST);
    }
    #[tokio::test]
    async fn manual_tv_match_verifies_episode_and_replaces_stale_metadata() {
        let (_temp, app) = fixture();
        let c = db(&app);
        c.execute("INSERT INTO files(path,root,size,mtime,parsed,result,locked,scanned) VALUES('/test.mkv','/',1,0,'{}',?1,0,'now')", [json!({"kind":"movie","title":"Wrong","provider":"tmdb","provider_id":9,"poster":"old","subtitles":["test.en.srt"]}).to_string()]).unwrap();
        c.execute(
            "INSERT INTO cache VALUES(?1,?2,?3)",
            params![
                "tvmaze:show:101",
                json!({"id":101,"name":"Regular Show","premiered":"2010-01-01"}).to_string(),
                Utc::now().timestamp()
            ],
        )
        .unwrap();
        let req = |episode| ManualMatch {
            path: "/test.mkv".into(),
            kind: "tv".into(),
            id: 101,
            season: Some(1),
            episode: Some(episode),
        };
        assert_eq!(
            manual_match(State(app.clone()), Json(req(99))).await.0,
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            manual_match(State(app.clone()), Json(req(2))).await.0,
            StatusCode::OK
        );
        let (raw, locked): (String, i64) = c
            .query_row(
                "SELECT result,locked FROM files WHERE path='/test.mkv'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        let result: Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(locked, 1);
        assert_eq!(result["provider"], "tvmaze");
        assert_eq!(result["title"], "Regular Show");
        assert_eq!(result["episodes"], json!([2]));
        assert!(result["poster"].is_null());
        assert_eq!(result["subtitles"], json!(["test.en.srt"]));
    }
    #[tokio::test]
    async fn subtitle_key_is_local_and_search_requires_it() {
        let (temp, app) = fixture();
        if std::env::var("OPENSUBTITLES_API_KEY").is_ok() {
            return;
        }
        assert!(!subtitle_status(State(app.clone())).await.0["configured"]
            .as_bool()
            .unwrap());
        assert_eq!(
            subtitle_search(
                State(app.clone()),
                Query(SubtitleSearch {
                    q: "Example".into(),
                    language: None
                })
            )
            .await
            .0,
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            save_subtitle_key(
                State(app.clone()),
                Json(SubtitleKey {
                    key: "test-key".into()
                })
            )
            .await
            .0,
            StatusCode::OK
        );
        assert_eq!(
            std::fs::read_to_string(temp.path().join("opensubtitles-api-key")).unwrap(),
            "test-key"
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(temp.path().join("opensubtitles-api-key"))
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }
    }
    #[tokio::test]
    async fn scan_title_number_duplicates_and_persistent_override() {
        let (temp, app) = fixture();
        let dir = temp.path().join("Regular Show");
        std::fs::create_dir(&dir).unwrap();
        let dir = std::fs::canonicalize(dir).unwrap();
        for name in ["The Power.mkv", "S01E02.mkv", "Repeated.mkv"] {
            std::fs::write(dir.join(name), b"not video data").unwrap();
        }
        std::fs::write(dir.join("The Power.en.srt"), b"subtitle").unwrap();
        scan(app.clone(), vec![dir.to_string_lossy().to_string()], None, true, false).await;
        let c = db(&app);
        let get = |name: &str| -> Value {
            let s: String = c
                .query_row(
                    "SELECT result FROM files WHERE path=?1",
                    [dir.join(name).to_string_lossy().to_string()],
                    |r| r.get(0),
                )
                .unwrap();
            serde_json::from_str(&s).unwrap()
        };
        assert_eq!(get("The Power.mkv")["episodes"], json!([1]));
        assert_eq!(
            get("The Power.mkv")["subtitles"],
            json!(["The Power.en.srt"])
        );
        assert_eq!(get("S01E02.mkv")["episodes"], json!([2]));
        assert_eq!(get("Repeated.mkv")["requires_review"], true);
        let path = dir.join("The Power.mkv").to_string_lossy().to_string();
        let response = update(
            State(app.clone()),
            Json(json!({"path":path,"override":{"kind":"extra"}})),
        )
        .await
        .into_response();
        assert_eq!(response.status(), StatusCode::OK);
        let repeated_path = dir.join("Repeated.mkv").to_string_lossy().to_string();
        c.execute(
            "UPDATE files SET result=?2 WHERE path=?1",
            params![repeated_path, json!({"kind":"tv","title":"stale title","matcher_version":6}).to_string()],
        ).unwrap();
        scan(app.clone(), vec![dir.to_string_lossy().to_string()], None, true, true).await;
        assert_ne!(get("Repeated.mkv")["title"], "stale title");
        assert_eq!(get("The Power.mkv")["kind"], "extra");
        std::fs::write(dir.join("The Power.mkv"), b"changed").unwrap();
        scan(app.clone(), vec![dir.to_string_lossy().to_string()], None, true, false).await;
        assert_eq!(get("The Power.mkv")["kind"], "extra");
        scan(
            app.clone(),
            vec![temp
                .path()
                .join("unavailable")
                .to_string_lossy()
                .to_string()],
            None,
            true,
            false,
        )
        .await;
        assert_eq!(get("The Power.mkv")["kind"], "extra");
    }
    #[tokio::test]
    async fn no_token_does_not_fabricate_movie_match() {
        let (_temp, app) = fixture();
        let p = parse(Path::new("/Movies/Film.2020.mkv"));
        // Tests do not rely on network; this checks a missing or unusable credential leaves no confirmed match.
        if std::env::var("TMDB_TOKEN").is_err() {
            let v = identify(&app, &p, vec![]).await;
            assert_eq!(v["confidence"], 0);
            assert!(v.get("provider_id").is_none());
        }
    }
    #[test]
    fn repair_bonus_preserves_manual_locks() {
        let (_temp, app) = fixture();
        let c = db(&app);
        let path = "/Media/Suits (2011)/Extras/Season 06/Gag Reel.mkv";
        let locked = "/Media/Suits (2011)/Extras/Season 06/Deleted Scene.mkv";
        let old = json!({"title":"Extras", "kind":"tv", "season":6,"episodes":[],"confidence":79,"provider_id":1270,"subtitles":[]});
        for (p, lock) in [(path, 0), (locked, 1)] {
            c.execute("INSERT INTO files(path,root,size,mtime,parsed,result,locked,scanned) VALUES(?1,'/Media',1,0,?2,?2,?3,'now')",params![p,old.to_string(),lock]).unwrap();
        }
        repair_bonus_records(&c);
        let read = |p: &str| -> Value {
            let s: String = c
                .query_row("SELECT result FROM files WHERE path=?1", [p], |r| r.get(0))
                .unwrap();
            serde_json::from_str(&s).unwrap()
        };
        assert_eq!(read(path)["kind"], "extra");
        assert_eq!(read(locked)["kind"], "tv");
    }
    #[tokio::test]
    async fn exact_show_beats_spinoff_and_year_distinguishes_remakes() {
        let (_temp, app) = fixture();
        let put = |key: &str, value: Value| {
            db(&app)
                .execute(
                    "INSERT OR REPLACE INTO cache VALUES(?1,?2,?3)",
                    params![key, value.to_string(), Utc::now().timestamp()],
                )
                .unwrap();
        };
        put(
            "tvmaze:search:adventure time",
            json!([{"show":{"id":1,"name":"Adventure Time","premiered":"2008-01-01"}},{"show":{"id":2,"name":"Adventure Time","premiered":"1967-01-01"}},{"show":{"id":3,"name":"Adventure Time: Side Quests","premiered":"2026-01-01"}}]),
        );
        put(
            "tvmaze:episodes:1",
            json!([{"season":1,"number":1,"name":"The Start","airdate":"2010-01-01"},{"season":1,"number":2,"name":"The End","airdate":"2010-01-02"}]),
        );
        let p = parse(Path::new(
            "/Media/Adventure Time (2010)/Season 01/Adventure Time S01E01 The Start.mkv",
        ));
        let r = identify(&app, &p, vec![]).await;
        assert_eq!(r["provider_id"], 1);
        assert_eq!(r["requires_review"], false);
        assert_eq!(r["episodes"], json!([1]));
        let p = parse(Path::new(
            "/Media/Adventure Time (2010)/Season 01/Adventure Time S01E01 The End.mkv",
        ));
        let r = identify(&app, &p, vec![]).await;
        assert_eq!(r["episodes"], json!([2]));
        assert!(r["explanation"]
            .as_str()
            .unwrap()
            .contains("different number"));
        let p = parse(Path::new(
            "/Media/Adventure Time/Season 01/Adventure Time S01E01 The Start.mkv",
        ));
        let r = identify(&app, &p, vec![]).await;
        assert_eq!(r["requires_review"], true);
    }
    #[tokio::test]
    async fn season_year_does_not_favor_spinoff() {
        let (_temp, app) = fixture();
        for (key, value) in [
            (
                "tvmaze:search:example show",
                json!([{"show":{"id":1,"name":"Example Show","premiered":"2013-01-01"}},{"show":{"id":2,"name":"Example Show: The Anime","premiered":"2024-01-01"}}]),
            ),
            (
                "tvmaze:episodes:1",
                json!([{"season":7,"number":5,"name":"First Part","airdate":"2023-01-01"}]),
            ),
        ] {
            db(&app)
                .execute(
                    "INSERT OR REPLACE INTO cache VALUES(?1,?2,?3)",
                    params![key, value.to_string(), Utc::now().timestamp()],
                )
                .unwrap();
        }
        let p=parse(Path::new("/Media/Example Show/Example Show - Season 7 (2023)/Example Show - S07E05 - First Pt.mkv"));
        let r = identify(&app, &p, vec![]).await;
        assert_eq!(r["provider_id"], 1);
        assert_eq!(r["requires_review"], false);
    }
    #[tokio::test]
    async fn empty_search_uses_corroborating_parent_title() {
        let (_temp, app) = fixture();
        let p = parse(Path::new(
            "/Media/jeen yuhs/jeen-yuhs.A.Kanye.Trilogy.S01E02.720p.WEBRip.mkv",
        ));
        assert_eq!(p.alternate_titles, vec!["jeen yuhs"]);
        for (key, value) in [
            ("tvmaze:search:jeen yuhs a kanye trilogy", json!([])),
            (
                "tvmaze:search:jeen yuhs",
                json!([{"show":{"id":59901,"name":"jeen-yuhs","premiered":"2022-02-16"}}]),
            ),
            (
                "tvmaze:episodes:59901",
                json!([{"season":1,"number":2,"name":"act ii: PURPOSE","airdate":"2022-02-23"}]),
            ),
        ] {
            db(&app)
                .execute(
                    "INSERT OR REPLACE INTO cache VALUES(?1,?2,?3)",
                    params![key, value.to_string(), Utc::now().timestamp()],
                )
                .unwrap();
        }
        let r = identify(&app, &p, vec![]).await;
        assert_eq!(r["provider_id"], 59901);
        assert_eq!(r["episodes"], json!([2]));
        assert_eq!(r["requires_review"], false);
        assert!(r["explanation"].as_str().unwrap().contains("folder title"));
        let other = parse(Path::new("/Media/Unrelated/Example.Show.S01E02.mkv"));
        assert!(other.alternate_titles.is_empty());
    }
    #[test]
    fn fuzzy_titles_need_number_and_clear_margin() {
        let episodes = vec![
            json!({"id":1,"season":1,"number":1,"name":"The Perfect Castaway"}),
            json!({"id":2,"season":1,"number":2,"name":"The Other Voyage"}),
        ];
        assert!(numbered_title_supported(
            "Perfect Castaway",
            &episodes[0],
            &episodes
        ));
        assert!(!numbered_title_supported(
            "Perfect Castaway",
            &episodes[1],
            &episodes
        ));
        let similar = vec![
            json!({"id":1,"season":1,"number":1,"name":"A Perfect Day"}),
            json!({"id":2,"season":1,"number":2,"name":"The Perfect Day"}),
        ];
        assert!(!numbered_title_supported(
            "Perfect Day",
            &similar[0],
            &similar
        ));
    }
    #[tokio::test]
    async fn collection_shorts_require_context_and_runtime() {
        let (temp, app) = fixture();
        std::fs::write(temp.path().join("tmdb-token"), "test-token").unwrap();
        let put = |key: &str, value: Value| {
            db(&app)
                .execute(
                    "INSERT OR REPLACE INTO cache VALUES(?1,?2,?3)",
                    params![key, value.to_string(), Utc::now().timestamp()],
                )
                .unwrap();
        };
        let p = parse(Path::new(
            "/Media/Tom and Jerry - Complete Classic Collection/029 - The Cat Concerto.mp4",
        ));
        assert_eq!(p.collection_title.as_deref(), Some("Tom and Jerry"));
        put(
            "tmdb:movie:The Cat Concerto",
            json!({"results":[{"id":10,"title":"The Cat Concerto","release_date":"1947-01-01","genre_ids":[16],"overview":"Tom chases Jerry."}]}),
        );
        put("tmdb:movie-detail:10", json!({"runtime":7}));
        let result = identify_collection_short(&app, &p, &[]).await.unwrap();
        assert_eq!(result["kind"], "movie");
        assert_eq!(result["provider_id"], 10);
        put("tmdb:movie-detail:10", json!({"runtime":90}));
        assert!(identify_collection_short(&app, &p, &[]).await.is_none());
        put(
            "tmdb:movie:The Cat Concerto",
            json!({"results":[{"id":10,"title":"The Cat Concerto","genre_ids":[16],"overview":"A piano concert."}]}),
        );
        assert!(identify_collection_short(&app, &p, &[]).await.is_none());
    }
    #[tokio::test]
    async fn movie_collection_metadata_is_cached_and_deduplicated() {
        let (_temp, app) = fixture();
        let conn = db(&app);
        let result = json!({"kind":"movie","provider":"tmdb","provider_id":12});
        for path in ["/Media/a.mkv", "/Media/b.mkv"] {
            conn.execute("INSERT INTO files(path,root,size,mtime,parsed,result,scanned) VALUES(?1,'/Media',1,0,'{}',?2,'now')",params![path,result.to_string()]).unwrap();
        }
        let local = json!({"kind":"movie","provider":"tmdb","provider_id":13,"collection_title":"Cartoons"});
        conn.execute("INSERT INTO files(path,root,size,mtime,parsed,result,scanned) VALUES('/Media/c.mkv','/Media',1,0,'{}',?1,'now')",[local.to_string()]).unwrap();
        conn.execute(
            "INSERT INTO cache VALUES(?1,?2,?3)",
            params![
                "tmdb:movie-detail:12",
                json!({"belongs_to_collection":{"id":9,"name":"Example Collection"}}).to_string(),
                Utc::now().timestamp()
            ],
        )
        .unwrap();
        assert_eq!(movie_ids_for_collections(&app), vec![12]);
        let Json(value) = movie_collections(State(app)).await;
        assert_eq!(value["collections"]["12"]["id"], 9);
        assert!(value["collections"].get("13").is_none());
    }
    #[test]
    fn library_snapshot_does_not_guess_availability() {
        let (_temp, app) = fixture();
        let conn = db(&app);
        conn.execute("INSERT INTO files(path,root,size,mtime,parsed,result,scanned) VALUES('/unreachable/film.mkv','/unreachable',1,0,'{}','{}','now')",[]).unwrap();
        let Json(before) = files_blocking(&app);
        assert!(before[0]["available"].is_null());
        app.availability
            .write()
            .unwrap()
            .insert("/unreachable/film.mkv".into(), false);
        let Json(after) = files_blocking(&app);
        assert_eq!(after[0]["available"], false);
        app.availability
            .write()
            .unwrap()
            .insert("/unreachable/film.mkv".into(), true);
        let Json(after) = files_blocking(&app);
        assert_eq!(after[0]["available"], true);
    }
    #[tokio::test]
    async fn availability_check_confirms_connected_files_after_startup() {
        let (temp, app) = fixture();
        let media = temp.path().join("episode.mkv");
        std::fs::write(&media, b"media").unwrap();
        let rows = json!([{"path":media,"available":null}]);
        ensure_availability_check(&app, &rows);
        for _ in 0..100 {
            if app.availability.read().unwrap().contains_key(media.to_str().unwrap()) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert_eq!(app.availability.read().unwrap().get(media.to_str().unwrap()), Some(&true));
    }

    #[test]
    fn page_bootstraps_saved_library_and_escapes_script_markup() {
        let (_temp, app) = fixture();
        let conn = db(&app);
        let result = json!({"kind":"movie","title":"Saved <Film>","provider":"tmdb","provider_id":12,"confidence":100});
        conn.execute("INSERT INTO files(path,root,size,mtime,parsed,result,scanned) VALUES(?1,'/Media',1,0,'{}',?2,'now')",params!["/Media/</script>.mkv",result.to_string()]).unwrap();
        let html = render_index(&app);
        let start = html
            .find("<script id=\"library-bootstrap\" type=\"application/json\">")
            .unwrap();
        let content = &html[start..];
        let json_start = content.find('>').unwrap() + 1;
        let json_end = content.find("</script>").unwrap();
        let data: Value = serde_json::from_str(&content[json_start..json_end]).unwrap();
        assert_eq!(data["files"][0]["path"], "/Media/</script>.mkv");
        assert_eq!(data["files"][0]["result"]["title"], "Saved <Film>");
        assert!(!content[json_start..json_end].contains('<'));
        assert!(html.contains("function renderNav()"));
        assert!(html.contains("function renderWatchlist()"));
        assert!(html.contains("id=\"detail-season\""));
        assert!(html.contains("id=\"detail-rating\""));
        assert!(!html.contains("% confidence"));
        assert!(html.contains("episode-legend"));
        assert!(html.contains("season-summary"));
        assert!(html.contains("episode-counts"));
        assert!(html.contains("setEpisodeFilter"));
        assert!(html.contains("availabilityRefreshDelay"));
    }
}
