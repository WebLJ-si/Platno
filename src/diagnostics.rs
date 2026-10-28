use crate::{db, playback, App, auth::Principal};
use axum::{extract::State, http::StatusCode, response::{IntoResponse, Response}, Extension, Json};
use serde::Deserialize;
use serde_json::{json, Value};
use std::time::Duration;

pub fn init(c: &rusqlite::Connection) {
    c.execute_batch("CREATE TABLE IF NOT EXISTS playback_reports(id INTEGER PRIMARY KEY,profile_id INTEGER NOT NULL,path TEXT NOT NULL,message TEXT NOT NULL,details TEXT NOT NULL,created_at INTEGER NOT NULL);").unwrap();
}
#[derive(Deserialize)]
pub struct Report { path: String, message: String, mode: String, quality: String, video_codec: Option<String>, audio_codec: Option<String> }
pub async fn report(State(app): State<App>, Extension(p): Extension<Principal>, Json(input): Json<Report>) -> Response {
    if playback::library_path(&app,&input.path).is_none() {return StatusCode::NOT_FOUND.into_response();}
    if input.message.trim().is_empty() || input.message.len()>2000 || !matches!(input.mode.as_str(),"direct"|"transcode") || !matches!(input.quality.as_str(),"auto"|"1080"|"720"|"480") || input.video_codec.as_ref().is_some_and(|s|s.len()>64) || input.audio_codec.as_ref().is_some_and(|s|s.len()>64) { return StatusCode::BAD_REQUEST.into_response(); }
    let c=db(&app);let now=chrono::Utc::now().timestamp();
    let recent:i64=c.query_row("SELECT COUNT(*) FROM playback_reports WHERE profile_id=?1 AND created_at>?2",rusqlite::params![p.profile_id,now-10],|r|r.get(0)).unwrap_or(0);
    if recent>0 {return StatusCode::TOO_MANY_REQUESTS.into_response();}
    let details=json!({"mode":input.mode,"quality":input.quality,"video_codec":input.video_codec,"audio_codec":input.audio_codec});
    if c.execute("INSERT INTO playback_reports(profile_id,path,message,details,created_at) VALUES(?1,?2,?3,?4,?5)",rusqlite::params![p.profile_id,input.path,input.message.trim(),details.to_string(),now]).is_err() {return StatusCode::INTERNAL_SERVER_ERROR.into_response();}
    let _=c.execute("DELETE FROM playback_reports WHERE id NOT IN (SELECT id FROM playback_reports ORDER BY id DESC LIMIT 500)",[]);
    Json(json!({"ok":true})).into_response()
}
pub async fn reports(State(app): State<App>) -> Json<Value> {
    let items:Vec<Value>=db(&app).prepare("SELECT r.id,r.path,r.message,r.details,r.created_at,p.name FROM playback_reports r LEFT JOIN profiles p ON p.id=r.profile_id ORDER BY r.id DESC LIMIT 100").and_then(|mut s|s.query_map([],|r|{let details:String=r.get(3)?;Ok(json!({"id":r.get::<_,i64>(0)?,"path":r.get::<_,String>(1)?,"message":r.get::<_,String>(2)?,"details":serde_json::from_str::<Value>(&details).unwrap_or_default(),"created_at":r.get::<_,i64>(4)?,"profile":r.get::<_,Option<String>>(5)?}))})?.collect()).unwrap_or_default();
    Json(json!({"items":items}))
}
async fn version(name: &str) -> Value {
    let mut cmd=tokio::process::Command::new(playback::binary(name));cmd.arg("-version").kill_on_drop(true);
    match tokio::time::timeout(Duration::from_secs(5),cmd.output()).await {Ok(Ok(v)) if v.status.success()=>json!({"available":true,"version":String::from_utf8_lossy(&v.stdout).lines().next().unwrap_or("")}),_=>json!({"available":false})}
}
pub async fn health(State(app): State<App>) -> Json<Value> {
    let (ffmpeg,ffprobe)=tokio::join!(version("ffmpeg"),version("ffprobe"));
    let c=db(&app);
    let rows:Vec<(String,i64,String)>=c.prepare("SELECT path,size,result FROM files").and_then(|mut s|s.query_map([],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?)))?.collect()).unwrap_or_default();
    let attention:Vec<Value>=rows.iter().filter_map(|(path,_,raw)|{let result:Value=serde_json::from_str(raw).ok()?;let matched=result["provider_id"].as_i64().is_some() && result["requires_review"]!=true && result["confidence"].as_f64().unwrap_or(0.0)>40.0;if !matched {Some(json!({"path":path,"reason":result["explanation"]}))}else{None}}).take(100).collect();
    let mut largest:Vec<_>=rows.iter().map(|(p,s,_)|json!({"path":p,"bytes":s})).collect();largest.sort_by_key(|x|std::cmp::Reverse(x["bytes"].as_i64().unwrap_or(0)));largest.truncate(10);
    let raw_roots:Vec<String>=c.prepare("SELECT roots FROM libraries").and_then(|mut s|s.query_map([],|r|r.get(0))?.collect()).unwrap_or_default();
    drop(c);
    let mut roots=std::collections::BTreeSet::new();for raw in raw_roots {if let Ok(values)=serde_json::from_str::<Vec<String>>(&raw){roots.extend(values);}}
    let mut directories=Vec::new();for root in roots.into_iter().take(100) {let readable=matches!(tokio::time::timeout(Duration::from_secs(2),tokio::fs::read_dir(&root)).await,Ok(Ok(_)));directories.push(json!({"path":root,"readable":readable}));}
    Json(json!({"ffmpeg":ffmpeg,"ffprobe":ffprobe,"tmdb_configured":!crate::tmdb_token(&app).is_empty(),"directories":directories,"files":rows.len(),"total_bytes":rows.iter().map(|(_,s,_)|*s).sum::<i64>(),"needs_attention":attention,"largest_files":largest}))
}
