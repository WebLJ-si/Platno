//! Works without any API key: Wikidata lookups for movies, names taken from file names, and manual
//! editing of titles, episodes and covers. Hand edits lock a file so rescans never overwrite them.
use crate::{db, parser::{normalize, Parsed}, App};
use axum::{
    body::Bytes,
    extract::{Path as AxPath, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use ring::digest;
use rusqlite::params;
use serde::Deserialize;
use serde_json::{json, Value};
use std::{path::PathBuf, time::Duration};

const WIKIDATA: &str = "https://www.wikidata.org/w/api.php";
// Kept far above any real TMDB or TVmaze id so a local id can never be mistaken for one.
const LOCAL_ID_BASE: i64 = 1_000_000_000_000;

pub(crate) fn wikidata_enabled() -> bool {
    std::env::var("SMART_MEDIA_WIKIDATA").ok().as_deref() != Some("off")
}
// Stable identity for a title that no provider recognised: the same name always groups together.
pub(crate) fn local_id(kind: &str, title: &str, year: Option<u32>) -> i64 {
    let year = if kind == "movie" { year.map(|y| y.to_string()).unwrap_or_default() } else { String::new() };
    let hash = digest::digest(&digest::SHA256, format!("{kind}|{}|{year}", normalize(title)).as_bytes());
    LOCAL_ID_BASE + hash.as_ref()[..5].iter().fold(0i64, |n, b| (n << 8) | *b as i64)
}

/* ---------- Wikidata ---------- */
#[derive(Debug, PartialEq)]
pub(crate) struct Pick {
    pub id: String,
    pub description: String,
    pub year_verified: bool,
}
// Only an unambiguous film is accepted: guessing wrong is worse than keeping the file name.
pub(crate) fn wikidata_pick(search: &Value, title: &str, year: Option<u32>) -> Option<Pick> {
    let wanted = normalize(title);
    let films: Vec<(&str, &str)> = search["search"]
        .as_array()?
        .iter()
        .filter(|x| normalize(x["label"].as_str().unwrap_or("")) == wanted)
        .filter_map(|x| Some((x["id"].as_str()?, x["description"].as_str().unwrap_or(""))))
        .filter(|(_, d)| {
            let d = d.to_lowercase();
            d.contains("film") && !["series", "television", "video game", "album", "song"].iter().any(|bad| d.contains(bad))
        })
        .collect();
    let has_year = |d: &str| d.split(|c: char| !c.is_ascii_digit()).any(|w| w.len() == 4 && (w.starts_with("19") || w.starts_with("20")));
    if let Some(year) = year {
        let exact: Vec<_> = films.iter().filter(|(_, d)| d.contains(&year.to_string())).collect();
        return match exact.as_slice() {
            [(id, d)] => Some(Pick { id: (*id).into(), description: (*d).into(), year_verified: true }),
            [] => {
                let undated: Vec<_> = films.iter().filter(|(_, d)| !has_year(d)).collect();
                match undated.as_slice() {
                    [(id, d)] if films.len() == 1 => Some(Pick { id: (*id).into(), description: (*d).into(), year_verified: false }),
                    _ => None,
                }
            }
            _ => None,
        };
    }
    match films.as_slice() {
        [(id, d)] => Some(Pick { id: (*id).into(), description: (*d).into(), year_verified: false }),
        _ => None,
    }
}
#[derive(Debug, Default, PartialEq)]
pub(crate) struct Film {
    pub year: Option<u32>,
    pub runtime: Option<u32>,
    pub genre_ids: Vec<String>,
    pub imdb: Option<String>,
    pub tmdb: Option<i64>,
}
pub(crate) fn wikidata_film(entities: &Value, id: &str) -> Film {
    let claims = &entities["entities"][id]["claims"];
    let values = |property: &str| -> Vec<&Value> { claims[property].as_array().into_iter().flatten().map(|c| &c["mainsnak"]["datavalue"]["value"]).collect() };
    let year = values("P577").iter().filter_map(|v| v["time"].as_str()).filter_map(|t| t.trim_start_matches('+').split('-').next()?.parse::<u32>().ok()).min();
    let runtime = values("P2047").iter().find_map(|v| {
        let amount: f64 = v["amount"].as_str()?.trim_start_matches('+').parse().ok()?;
        let minutes = match v["unit"].as_str()?.rsplit('/').next()? {
            "Q7727" => amount,
            "Q25235" => amount * 60.0,
            "Q11574" => amount / 60.0,
            _ => return None,
        };
        (1.0..=1500.0).contains(&minutes).then(|| minutes.round() as u32)
    });
    Film {
        year,
        runtime,
        genre_ids: values("P136").iter().filter_map(|v| v["id"].as_str().map(str::to_owned)).take(6).collect(),
        imdb: values("P345").iter().find_map(|v| v.as_str().filter(|s| s.starts_with("tt")).map(str::to_owned)),
        tmdb: values("P4947").iter().find_map(|v| v.as_str()?.parse().ok()),
    }
}
fn qnumber(id: &str) -> Option<i64> {
    id.strip_prefix('Q')?.parse().ok()
}
async fn wikidata_get(app: &App, key: &str, query: &str) -> Result<Value, String> {
    let value = crate::cached(app, key, &format!("{WIKIDATA}?format=json&{query}"), None).await;
    // Wikimedia asks clients to be gentle: one request at a time with a short pause.
    tokio::time::sleep(Duration::from_millis(100)).await;
    value
}
pub(crate) async fn wikidata_movie(app: &App, title: &str, year: Option<u32>) -> Result<Option<Value>, String> {
    let clean = normalize(title);
    if clean.is_empty() {
        return Ok(None);
    }
    let search = wikidata_get(app, &format!("wikidata:search:{clean}"), &format!("action=wbsearchentities&type=item&language=en&uselang=en&limit=10&search={}", urlencoding::encode(title))).await?;
    let Some(pick) = wikidata_pick(&search, title, year) else { return Ok(None) };
    let entities = wikidata_get(app, &format!("wikidata:entity:{}", pick.id), &format!("action=wbgetentities&props=labels|claims&languages=en&ids={}", pick.id)).await?;
    let film = wikidata_film(&entities, &pick.id);
    let label = entities["entities"][&pick.id]["labels"]["en"]["value"].as_str().unwrap_or(title).to_owned();
    let mut genres = Vec::new();
    if !film.genre_ids.is_empty() {
        let ids = film.genre_ids.join("|");
        let labels = wikidata_get(app, &format!("wikidata:labels:{ids}"), &format!("action=wbgetentities&props=labels&languages=en&ids={ids}")).await?;
        genres = film.genre_ids.iter().filter_map(|g| labels["entities"][g]["labels"]["en"]["value"].as_str().map(|s| s[..1].to_uppercase() + &s[1..])).collect();
    }
    Ok(Some(json!({
        "title": label, "year": film.year.map(|y| y.to_string()), "description": pick.description, "runtime": film.runtime,
        "genres": genres, "imdb_id": film.imdb, "tmdb_id": film.tmdb, "provider_id": qnumber(&pick.id), "year_verified": pick.year_verified,
    })))
}

/* ---------- identity for titles nothing recognised ---------- */
// Runs after the online providers. Without a TMDB key a movie is tried on Wikidata; anything still unmatched
// keeps the name from its file name, so every title appears in the library and can be edited.
pub(crate) async fn finish_unmatched(app: &App, p: &Parsed, result: &mut Value) {
    if !result["provider_id"].is_null() || !matches!(p.kind.as_str(), "movie" | "tv") {
        return;
    }
    if result["explanation"].as_str().is_some_and(|e| e.contains("unavailable")) {
        result["lookup_failed"] = json!(true);
    }
    // Wikidata is only the free stand-in for a missing key; with a TMDB key the name comes from the file.
    if p.kind == "movie" && wikidata_enabled() && crate::tmdb_token(app).is_empty() {
        match wikidata_movie(app, &p.title, p.year).await {
            Ok(Some(found)) if found["provider_id"].as_i64().is_some() => {
                for key in ["title", "year", "description", "runtime", "genres", "imdb_id", "provider_id"] {
                    result[key] = found[key].clone();
                }
                result["provider"] = json!("wikidata");
                result["confidence"] = json!(if found["year_verified"] == true { 72 } else { 60 });
                result["requires_review"] = json!(false);
                result["explanation"] = json!("Matched on Wikidata, a free source, by title and year. A free TMDB key adds posters and ratings.");
                result.as_object_mut().map(|o| o.remove("lookup_failed"));
                return;
            }
            Ok(_) => {}
            Err(_) => result["lookup_failed"] = json!(true),
        }
    }
    // Without episode evidence or an online match, an arbitrary clip is a
    // video, rather than a fabricated local series.
    let kind = if p.kind=="tv" && p.season.is_none() && p.episodes.is_empty() {"video"} else {&p.kind};
    result["kind"] = json!(kind);
    let title = if kind=="video" && !p.episode_title.is_empty() {&p.episode_title} else {&p.title};
    result["title"] = json!(title);
    let id = local_id(kind, title, p.year);
    result["provider"] = json!("local");
    result["provider_id"] = json!(id);
    result["confidence"] = json!(55);
    result["requires_review"] = json!(false);
    result["explanation"] = json!("Named from the file name; no online match was found. You can edit the name, cover and episodes by hand.");
    // Hand edits belong to the whole title, so a new episode of an edited show joins it.
    let edited: Option<String> = db(app)
        .query_row("SELECT result FROM files WHERE locked=1 AND json_extract(result,'$.provider')='local' AND json_extract(result,'$.provider_id')=?1 LIMIT 1", [id], |r| r.get(0))
        .ok();
    if let Some(edited) = edited.and_then(|raw| serde_json::from_str::<Value>(&raw).ok()) {
        for key in ["title", "year", "description", "genres", "poster", "poster_original"] {
            if !edited[key].is_null() {
                result[key] = edited[key].clone();
            }
        }
    }
}

/* ---------- hand edits ---------- */
fn clean_text(value: &str, max: usize, multiline: bool) -> Option<String> {
    let value = value.trim();
    (value.chars().count() <= max && !value.chars().any(|c| c.is_control() && !(multiline && (c == '\n' || c == '\r')))).then(|| value.to_owned())
}
pub(crate) fn valid_cover_name(name: &str) -> bool {
    let Some((stem, ext)) = name.split_once('.') else { return false };
    stem.len() == 32 && stem.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')) && matches!(ext, "jpg" | "png" | "webp")
}
fn covers_dir(app: &App) -> PathBuf {
    app.db.parent().unwrap_or(std::path::Path::new(".")).join("covers")
}
fn fail(status: StatusCode, message: &str) -> Response {
    (status, Json(json!({"error": message}))).into_response()
}
#[derive(Deserialize)]
pub struct TitleEdit {
    paths: Vec<String>,
    title: Option<String>,
    year: Option<String>,
    description: Option<String>,
    genres: Option<Vec<String>>,
    poster: Option<String>,
}
// Returns the problem in words the person editing can act on.
pub(crate) fn check_title_edit(edit: &TitleEdit) -> Result<(), &'static str> {
    if edit.paths.is_empty() || edit.paths.len() > 5000 {
        return Err("Choose at least one file to edit.");
    }
    if edit.title.as_deref().is_some_and(|t| t.trim().is_empty() || clean_text(t, 200, false).is_none()) {
        return Err("Enter a title of 1 to 200 characters.");
    }
    if edit.year.as_deref().is_some_and(|y| !y.is_empty() && !(y.len() == 4 && y.bytes().all(|b| b.is_ascii_digit()) && (1870..=2100).contains(&y.parse::<u32>().unwrap_or(0)))) {
        return Err("Enter a four-digit year between 1870 and 2100, or leave it empty.");
    }
    if edit.description.as_deref().is_some_and(|d| clean_text(d, 4000, true).is_none()) {
        return Err("The description may have at most 4000 characters.");
    }
    if edit.genres.as_ref().is_some_and(|g| g.len() > 12 || g.iter().any(|x| x.trim().is_empty() || clean_text(x, 40, false).is_none())) {
        return Err("Use at most 12 genres of up to 40 characters each.");
    }
    if edit.poster.as_deref().is_some_and(|p| !p.is_empty() && !p.strip_prefix("/api/covers/").is_some_and(valid_cover_name)) {
        return Err("Upload the cover again.");
    }
    Ok(())
}
pub async fn edit_title(State(app): State<App>, Json(edit): Json<TitleEdit>) -> Response {
    if let Err(message) = check_title_edit(&edit) {
        return fail(StatusCode::BAD_REQUEST, message);
    }
    if let Some(poster) = edit.poster.as_deref().filter(|p| !p.is_empty()) {
        if !covers_dir(&app).join(poster.trim_start_matches("/api/covers/")).is_file() {
            return fail(StatusCode::BAD_REQUEST, "That cover is no longer on the server. Upload it again.");
        }
    }
    let connection = db(&app);
    let Ok(tx) = connection.unchecked_transaction() else { return StatusCode::INTERNAL_SERVER_ERROR.into_response() };
    for path in &edit.paths {
        let Ok(raw) = tx.query_row("SELECT result FROM files WHERE path=?1", [path], |r| r.get::<_, String>(0)) else {
            return fail(StatusCode::NOT_FOUND, "A file of this title is no longer in the library. Refresh and try again.");
        };
        let mut result: Value = serde_json::from_str(&raw).unwrap_or_else(|_| json!({}));
        apply_title_edit(&mut result, &edit);
        if tx.execute("UPDATE files SET result=?2,locked=1 WHERE path=?1", params![path, result.to_string()]).is_err() {
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    }
    match tx.commit() {
        Ok(()) => Json(json!({"ok": true, "files": edit.paths.len()})).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}
pub(crate) fn apply_title_edit(result: &mut Value, edit: &TitleEdit) {
    if let Some(title) = &edit.title {
        result["title"] = json!(title.trim());
    }
    if let Some(year) = &edit.year {
        result["year"] = if year.is_empty() { Value::Null } else { json!(year) };
    }
    if let Some(description) = &edit.description {
        result["description"] = json!(description.trim());
    }
    if let Some(genres) = &edit.genres {
        result["genres"] = json!(genres.iter().map(|g| g.trim()).collect::<Vec<_>>());
    }
    if let Some(poster) = &edit.poster {
        if poster.is_empty() {
            // Removing a custom cover brings back whatever the provider supplied.
            if result.get("poster_original").is_some() {
                let original = result["poster_original"].take();
                result.as_object_mut().map(|o| o.remove("poster_original"));
                if original.is_null() { result.as_object_mut().map(|o| o.remove("poster")); } else { result["poster"] = original; }
            }
        } else {
            if result.get("poster_original").is_none() {
                result["poster_original"] = result.get("poster").cloned().unwrap_or(Value::Null);
            }
            result["poster"] = json!(poster);
        }
    }
    result["requires_review"] = json!(false);
    result["manual_edit"] = json!(true);
    result["explanation"] = json!("Edited by hand; protected from rescans");
}
#[derive(Deserialize)]
pub struct EpisodeEdit {
    path: String,
    season: Option<u32>,
    episodes: Vec<u32>,
    title: Option<String>,
}
pub(crate) fn check_episode_edit(edit: &EpisodeEdit) -> Result<(), &'static str> {
    if edit.season.is_some_and(|s| s > 1000) {
        return Err("Use a season number between 0 and 1000.");
    }
    if edit.episodes.len() > 50 || edit.episodes.iter().any(|n| *n == 0 || *n > 100_000) {
        return Err("Use up to 50 episode numbers between 1 and 100000.");
    }
    if edit.title.as_deref().is_some_and(|t| clean_text(t, 200, false).is_none()) {
        return Err("Use an episode title of at most 200 characters.");
    }
    Ok(())
}
pub async fn edit_episode(State(app): State<App>, Json(edit): Json<EpisodeEdit>) -> Response {
    if let Err(message) = check_episode_edit(&edit) {
        return fail(StatusCode::BAD_REQUEST, message);
    }
    let connection = db(&app);
    let Ok(raw) = connection.query_row("SELECT result FROM files WHERE path=?1", [&edit.path], |r| r.get::<_, String>(0)) else {
        return fail(StatusCode::NOT_FOUND, "That file is no longer in the library.");
    };
    let mut result: Value = serde_json::from_str(&raw).unwrap_or_else(|_| json!({}));
    result["season"] = json!(edit.season);
    result["episodes"] = json!(edit.episodes);
    if let Some(title) = &edit.title {
        result["episode_title"] = json!(title.trim());
    }
    result["requires_review"] = json!(false);
    result["manual_edit"] = json!(true);
    result["explanation"] = json!("Edited by hand; protected from rescans");
    match connection.execute("UPDATE files SET result=?2,locked=1 WHERE path=?1", params![edit.path, result.to_string()]) {
        Ok(1) => Json(json!({"ok": true})).into_response(),
        _ => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

/* ---------- covers ---------- */
// SVG is refused on purpose: it can carry scripts. Only these three formats are recognised by content.
pub(crate) fn cover_extension(bytes: &[u8]) -> Option<&'static str> {
    if bytes.len() < 64 {
        None
    } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("jpg")
    } else if bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
        Some("png")
    } else if &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some("webp")
    } else {
        None
    }
}
pub async fn upload_cover(State(app): State<App>, body: Bytes) -> Response {
    let Some(ext) = cover_extension(&body) else {
        return fail(StatusCode::BAD_REQUEST, "Use a JPEG, PNG or WebP picture of at most 8 MB.");
    };
    let hash: String = digest::digest(&digest::SHA256, &body).as_ref()[..16].iter().map(|b| format!("{b:02x}")).collect();
    let name = format!("{hash}.{ext}");
    let dir = covers_dir(&app);
    let written = async {
        tokio::fs::create_dir_all(&dir).await?;
        let temporary = dir.join(format!("{hash}.part"));
        tokio::fs::write(&temporary, &body).await?;
        tokio::fs::rename(&temporary, dir.join(&name)).await
    }
    .await;
    match written {
        Ok(()) => Json(json!({"id": name, "url": format!("/api/covers/{name}")})).into_response(),
        Err(_) => fail(StatusCode::INTERNAL_SERVER_ERROR, "The picture could not be saved on the server."),
    }
}
pub async fn cover(State(app): State<App>, AxPath(name): AxPath<String>) -> Response {
    if !valid_cover_name(&name) {
        return StatusCode::NOT_FOUND.into_response();
    }
    let Ok(bytes) = tokio::fs::read(covers_dir(&app).join(&name)).await else { return StatusCode::NOT_FOUND.into_response() };
    let kind = match name.rsplit('.').next() {
        Some("png") => "image/png",
        Some("webp") => "image/webp",
        _ => "image/jpeg",
    };
    ([(header::CONTENT_TYPE, kind), (header::CACHE_CONTROL, "private, max-age=86400"), (header::X_CONTENT_TYPE_OPTIONS, "nosniff")], bytes).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn found(items: Value) -> Value {
        json!({"search": items})
    }
    #[test]
    fn local_ids_are_stable_distinct_and_far_from_provider_ids() {
        let a = local_id("movie", "The Matrix", Some(1999));
        assert_eq!(a, local_id("movie", "  the MATRIX ", Some(1999)));
        assert_ne!(a, local_id("movie", "The Matrix", Some(2003)));
        assert_ne!(a, local_id("tv", "The Matrix", Some(1999)));
        assert_eq!(local_id("tv", "Demo Show", Some(2001)), local_id("tv", "Demo Show", None));
        assert!(a > LOCAL_ID_BASE && a < LOCAL_ID_BASE + (1 << 40));
    }
    #[test]
    fn wikidata_choice_accepts_only_unambiguous_films() {
        let matrix = found(json!([
            {"id": "Q83495", "label": "The Matrix", "description": "1999 science fiction action film directed by the Wachowskis"},
            {"id": "Q1", "label": "The Matrix", "description": "American media franchise"},
            {"id": "Q2", "label": "The Matrix", "description": "film series"},
            {"id": "Q3", "label": "Matrix Reloaded", "description": "2003 film"},
        ]));
        assert_eq!(wikidata_pick(&matrix, "The Matrix", Some(1999)).map(|p| (p.id, p.year_verified)), Some(("Q83495".into(), true)));
        assert_eq!(wikidata_pick(&matrix, "The Matrix", None).map(|p| p.id), Some("Q83495".into()), "one film candidate is enough without a year");
        assert_eq!(wikidata_pick(&matrix, "The Matrix", Some(2003)), None, "wrong year must not match");
        let remakes = found(json!([
            {"id": "Q10", "label": "Solaris", "description": "1972 film by Andrei Tarkovsky"},
            {"id": "Q11", "label": "Solaris", "description": "2002 film by Steven Soderbergh"},
        ]));
        assert_eq!(wikidata_pick(&remakes, "Solaris", None), None, "two films with the same name are ambiguous");
        assert_eq!(wikidata_pick(&remakes, "Solaris", Some(2002)).map(|p| p.id), Some("Q11".into()));
        assert_eq!(wikidata_pick(&found(json!([])), "Nothing", None), None);
        assert_eq!(wikidata_pick(&json!({}), "Nothing", None), None);
    }
    #[test]
    fn wikidata_entity_facts_are_read_defensively() {
        let entity = json!({"entities": {"Q83495": {"claims": {
            "P577": [{"mainsnak": {"datavalue": {"value": {"time": "+1999-03-31T00:00:00Z"}}}}, {"mainsnak": {"datavalue": {"value": {"time": "+1999-03-24T00:00:00Z"}}}}],
            "P2047": [{"mainsnak": {"datavalue": {"value": {"amount": "+136", "unit": "http://www.wikidata.org/entity/Q7727"}}}}],
            "P136": [{"mainsnak": {"datavalue": {"value": {"id": "Q471839"}}}}, {"mainsnak": {"datavalue": {"value": {"id": "Q319221"}}}}],
            "P345": [{"mainsnak": {"datavalue": {"value": "tt0133093"}}}],
            "P4947": [{"mainsnak": {"datavalue": {"value": "603"}}}]
        }}}});
        assert_eq!(wikidata_film(&entity, "Q83495"), Film { year: Some(1999), runtime: Some(136), genre_ids: vec!["Q471839".into(), "Q319221".into()], imdb: Some("tt0133093".into()), tmdb: Some(603) });
        let hours = json!({"entities": {"Q1": {"claims": {"P2047": [{"mainsnak": {"datavalue": {"value": {"amount": "+2", "unit": "http://www.wikidata.org/entity/Q25235"}}}}]}}}});
        assert_eq!(wikidata_film(&hours, "Q1").runtime, Some(120));
        assert_eq!(wikidata_film(&json!({}), "Q404"), Film::default());
        let broken = json!({"entities": {"Q2": {"claims": {"P577": [{"mainsnak": {}}], "P2047": [{"mainsnak": {"datavalue": {"value": {"amount": "x", "unit": "?"}}}}]}}}});
        assert_eq!(wikidata_film(&broken, "Q2"), Film::default());
    }
    #[test]
    fn covers_are_recognised_by_content_not_by_name() {
        let mut png = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
        png.resize(100, 0);
        let mut jpeg = vec![0xFF, 0xD8, 0xFF, 0xE0];
        jpeg.resize(100, 0);
        let mut webp = b"RIFF\0\0\0\0WEBP".to_vec();
        webp.resize(100, 0);
        assert_eq!((cover_extension(&png), cover_extension(&jpeg), cover_extension(&webp)), (Some("png"), Some("jpg"), Some("webp")));
        let mut svg = b"<svg xmlns='http://www.w3.org/2000/svg'><script>alert(1)</script></svg>".to_vec();
        svg.resize(100, b' ');
        for bad in [svg, b"GIF89a".repeat(20), vec![0xFF, 0xD8, 0xFF], Vec::new(), b"RIFFxxxxWAVE".repeat(10)] {
            assert_eq!(cover_extension(&bad), None);
        }
        assert!(valid_cover_name("0123456789abcdef0123456789abcdef.png"));
        for bad in ["../etc/passwd", "0123456789abcdef0123456789abcdef.svg", "0123456789ABCDEF0123456789abcdef.png", "short.png", "0123456789abcdef0123456789abcdef", "0123456789abcdef0123456789abcdef.png/../x"] {
            assert!(!valid_cover_name(bad), "{bad}");
        }
    }
    fn edit(value: Value) -> TitleEdit {
        let mut value = value;
        value["paths"] = json!(["/a.mkv"]);
        serde_json::from_value(value).unwrap()
    }
    #[test]
    fn title_edits_reject_values_the_interface_cannot_show() {
        assert!(check_title_edit(&edit(json!({"title": " Fine ", "year": "1999", "genres": ["Drama"], "description": "Line 1\nLine 2"}))).is_ok());
        assert!(check_title_edit(&edit(json!({"year": ""}))).is_ok());
        for bad in [
            json!({"title": "   "}), json!({"title": "x".repeat(201)}), json!({"title": "a\u{0}b"}), json!({"year": "99"}), json!({"year": "1000"}),
            json!({"year": "20x0"}), json!({"description": "x".repeat(4001)}), json!({"description": "bell\u{7}"}), json!({"genres": [""]}),
            json!({"genres": vec!["g"; 13]}), json!({"poster": "https://evil.test/x.png"}), json!({"poster": "/api/covers/../../etc/passwd"}),
        ] {
            assert!(check_title_edit(&edit(bad.clone())).is_err(), "{bad}");
        }
        let none: TitleEdit = serde_json::from_value(json!({"paths": []})).unwrap();
        assert!(check_title_edit(&none).is_err());
    }
    #[test]
    fn removing_a_custom_cover_restores_the_provider_poster() {
        let cover = "/api/covers/0123456789abcdef0123456789abcdef.png";
        let mut result = json!({"title": "Old", "poster": "https://image.test/p.jpg"});
        apply_title_edit(&mut result, &edit(json!({"title": "New", "poster": cover})));
        assert_eq!((result["title"].as_str(), result["poster"].as_str(), result["poster_original"].as_str()), (Some("New"), Some(cover), Some("https://image.test/p.jpg")));
        apply_title_edit(&mut result, &edit(json!({"poster": cover})));
        assert_eq!(result["poster_original"], "https://image.test/p.jpg", "a second upload must not overwrite the original");
        apply_title_edit(&mut result, &edit(json!({"poster": ""})));
        assert_eq!((result["poster"].as_str(), result.get("poster_original").is_none()), (Some("https://image.test/p.jpg"), true));
        let mut bare = json!({"title": "No art"});
        apply_title_edit(&mut bare, &edit(json!({"poster": cover})));
        apply_title_edit(&mut bare, &edit(json!({"poster": ""})));
        assert!(bare.get("poster").is_none() && bare.get("poster_original").is_none());
        assert_eq!(bare["manual_edit"], true);
    }
    #[test]
    fn episode_edits_are_bounded() {
        let ok = |v: Value| check_episode_edit(&serde_json::from_value(v).unwrap());
        assert!(ok(json!({"path": "/a", "season": 2, "episodes": [3, 4], "title": "Pilot"})).is_ok());
        assert!(ok(json!({"path": "/a", "season": null, "episodes": []})).is_ok());
        for bad in [json!({"path": "/a", "season": 1001, "episodes": []}), json!({"path": "/a", "season": 1, "episodes": [0]}), json!({"path": "/a", "season": 1, "episodes": vec![1; 51]}), json!({"path": "/a", "season": 1, "episodes": [1], "title": "a\u{0}"})] {
            assert!(ok(bad.clone()).is_err(), "{bad}");
        }
    }
}
