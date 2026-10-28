use crate::{db, playback, App};
use axum::{extract::{Query, State}, http::StatusCode, response::{IntoResponse, Response}, Json};
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Deserialize)]
pub struct FileQuery { path: String }
#[derive(Deserialize, Serialize, Clone)]
pub struct Marker { kind: String, start: f64, end: f64 }
#[derive(Deserialize)]
pub struct MarkersInput { path: String, markers: Vec<Marker> }
pub fn init(c: &rusqlite::Connection) {
    c.execute_batch("CREATE TABLE IF NOT EXISTS playback_markers(path TEXT PRIMARY KEY, markers TEXT NOT NULL);").unwrap();
}
fn valid_markers(markers: &[Marker]) -> bool {
    let mut kinds=std::collections::HashSet::new();
    markers.len()<=3 && markers.iter().all(|m| matches!(m.kind.as_str(),"intro"|"recap"|"credits") && kinds.insert(&m.kind) && m.start.is_finite() && m.end.is_finite() && m.start>=0.0 && m.end>m.start && m.end<=86400.0)
}
pub async fn markers_get(State(app): State<App>, Query(input): Query<FileQuery>) -> Response {
    if playback::library_path(&app,&input.path).is_none() { return StatusCode::NOT_FOUND.into_response(); }
    let raw:String=db(&app).query_row("SELECT markers FROM playback_markers WHERE path=?1",[input.path],|r|r.get(0)).unwrap_or_else(|_|"[]".into());
    Json(json!({"markers":serde_json::from_str::<serde_json::Value>(&raw).unwrap_or(json!([]))})).into_response()
}
pub async fn markers_post(State(app): State<App>, Json(input): Json<MarkersInput>) -> Response {
    if playback::library_path(&app,&input.path).is_none() { return StatusCode::NOT_FOUND.into_response(); }
    if !valid_markers(&input.markers) { return (StatusCode::BAD_REQUEST,Json(json!({"error":"Use one intro, recap or credits range with 0 ≤ start < end ≤ 86400 seconds"}))).into_response(); }
    match db(&app).execute("INSERT INTO playback_markers(path,markers) VALUES(?1,?2) ON CONFLICT(path) DO UPDATE SET markers=excluded.markers",rusqlite::params![input.path,serde_json::to_string(&input.markers).unwrap()]) {
        Ok(_)=>Json(json!({"ok":true})).into_response(),Err(_)=>StatusCode::INTERNAL_SERVER_ERROR.into_response()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn markers_reject_ambiguous_or_invalid_ranges() {
        let marker=|kind:&str,start,end|Marker{kind:kind.into(),start,end};
        assert!(valid_markers(&[marker("intro",1.0,45.0),marker("credits",1800.0,1900.0)]));
        assert!(!valid_markers(&[marker("intro",0.0,0.0)]));
        assert!(!valid_markers(&[marker("intro",0.0,f64::NAN)]));
        assert!(!valid_markers(&[marker("intro",0.0,10.0),marker("intro",20.0,30.0)]));
        assert!(!valid_markers(&[marker("unknown",0.0,10.0)]));
    }
}
