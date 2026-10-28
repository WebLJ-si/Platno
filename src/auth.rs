use axum::{
    body::Body,
    extract::{Request, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    middleware::Next,
    response::{Html, IntoResponse, Response},
    Json,
};
use ring::{digest, pbkdf2, rand::{SecureRandom, SystemRandom}};
use rusqlite::{params, Connection};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{num::NonZeroU32, path::PathBuf, time::Duration};

use crate::App;

const ROUNDS: u32 = 600_000;
const SESSION_DAYS: i64 = 30;
const COOKIE: &str = "smart_media_session";

#[derive(Clone, Debug)]
pub struct Principal {
    pub account_id: i64,
    pub username: String,
    pub email: String,
    pub role: String,
    pub profile_id: i64,
    pub profile_name: String,
}

#[derive(Deserialize)]
pub struct Credentials { pub email: String, pub password: String, pub display_name: Option<String> }
#[derive(Deserialize)]
pub struct ProfileInput { pub name: String }
#[derive(Deserialize)]
pub struct AccountInput { pub email: String, pub password: String, pub display_name: Option<String>, pub role: Option<String> }
#[derive(Deserialize)]
pub struct OwnerCredentialsInput { pub account_id: i64, pub email: String, pub password: String }
#[derive(Deserialize)]
pub struct SettingInput { pub key: String, pub value: String }
#[derive(Deserialize)]
pub struct ProgressQuery { pub path: Option<String> }

fn conn(path: &PathBuf) -> Connection {
    let c = Connection::open(path).expect("auth database open");
    let _ = c.busy_timeout(Duration::from_secs(10));
    let _ = c.execute_batch("PRAGMA foreign_keys=ON;");
    c
}

pub fn init(path: &PathBuf) {
    let c=conn(path);
    c.execute_batch("PRAGMA foreign_keys=ON;
      CREATE TABLE IF NOT EXISTS accounts(id INTEGER PRIMARY KEY, username TEXT NOT NULL COLLATE NOCASE UNIQUE, email TEXT COLLATE NOCASE, password_hash TEXT NOT NULL, role TEXT NOT NULL CHECK(role IN ('owner','admin','user')), created_at INTEGER NOT NULL);
      CREATE UNIQUE INDEX IF NOT EXISTS accounts_single_owner ON accounts(role) WHERE role='owner';
      CREATE TABLE IF NOT EXISTS profiles(id INTEGER PRIMARY KEY, account_id INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE, name TEXT NOT NULL COLLATE NOCASE, created_at INTEGER NOT NULL, UNIQUE(account_id,name));
      CREATE TABLE IF NOT EXISTS auth_sessions(token_hash TEXT PRIMARY KEY, account_id INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE, profile_id INTEGER NOT NULL REFERENCES profiles(id) ON DELETE CASCADE, created_at INTEGER NOT NULL, expires_at INTEGER NOT NULL);
      CREATE TABLE IF NOT EXISTS auth_login_attempts(username TEXT PRIMARY KEY COLLATE NOCASE, attempts INTEGER NOT NULL, window_started INTEGER NOT NULL, blocked_until INTEGER NOT NULL DEFAULT 0);
      CREATE TABLE IF NOT EXISTS profile_watch_progress(profile_id INTEGER NOT NULL REFERENCES profiles(id) ON DELETE CASCADE, path TEXT NOT NULL, position REAL NOT NULL, duration REAL NOT NULL, updated_at INTEGER NOT NULL, completed INTEGER NOT NULL DEFAULT 0, PRIMARY KEY(profile_id,path));
      CREATE TABLE IF NOT EXISTS profile_settings(profile_id INTEGER NOT NULL REFERENCES profiles(id) ON DELETE CASCADE, key TEXT NOT NULL, value TEXT NOT NULL, PRIMARY KEY(profile_id,key));
      CREATE TABLE IF NOT EXISTS server_settings(key TEXT PRIMARY KEY, value TEXT NOT NULL);
      CREATE TABLE IF NOT EXISTS playback_activity(profile_id INTEGER NOT NULL REFERENCES profiles(id) ON DELETE CASCADE, path TEXT NOT NULL, position REAL NOT NULL DEFAULT 0, duration REAL NOT NULL DEFAULT 0, mode TEXT NOT NULL DEFAULT 'direct', active INTEGER NOT NULL DEFAULT 1, started_at INTEGER NOT NULL, updated_at INTEGER NOT NULL, PRIMARY KEY(profile_id,path));
      CREATE INDEX IF NOT EXISTS auth_sessions_expiry ON auth_sessions(expires_at);
      CREATE INDEX IF NOT EXISTS playback_activity_active ON playback_activity(active,updated_at);").unwrap();
    let has_email: bool = c.prepare("PRAGMA table_info(accounts)").and_then(|mut s|s.query_map([],|r|r.get::<_,String>(1))?.collect::<Result<Vec<_>,_>>()).map(|v|v.iter().any(|x|x=="email")).unwrap_or(false);
    if !has_email { let _=c.execute_batch("ALTER TABLE accounts ADD COLUMN email TEXT COLLATE NOCASE;"); }
    let _=c.execute_batch("CREATE UNIQUE INDEX IF NOT EXISTS accounts_email_unique ON accounts(email) WHERE email IS NOT NULL;");
    let _=c.execute("UPDATE accounts SET email=lower(username) WHERE email IS NULL AND instr(username,'@')>1",[]);
}

pub fn has_accounts(path: &PathBuf) -> bool {
    conn(path).query_row("SELECT EXISTS(SELECT 1 FROM accounts)", [], |r| r.get(0)).unwrap_or(false)
}

fn hash_password(password: &str) -> Result<String, ()> {
    if password.len() < 12 || password.len() > 1024 { return Err(()); }
    let rng = SystemRandom::new();
    let mut salt = [0u8; 16];
    rng.fill(&mut salt).map_err(|_| ())?;
    let mut out = [0u8; 32];
    pbkdf2::derive(pbkdf2::PBKDF2_HMAC_SHA256, NonZeroU32::new(ROUNDS).unwrap(), &salt, password.as_bytes(), &mut out);
    Ok(format!("pbkdf2${ROUNDS}${}${}", hex(&salt), hex(&out)))
}
fn verify_password(encoded: &str, password: &str) -> bool {
    let parts: Vec<_> = encoded.split('$').collect();
    if parts.len() != 4 || parts[0] != "pbkdf2" { return false; }
    let Ok(rounds) = parts[1].parse::<u32>() else { return false; };
    if !(100_000..=2_000_000).contains(&rounds) { return false; }
    let (Ok(salt), Ok(expected)) = (unhex(parts[2]), unhex(parts[3])) else { return false; };
    pbkdf2::verify(pbkdf2::PBKDF2_HMAC_SHA256, NonZeroU32::new(rounds).unwrap(), &salt, password.as_bytes(), &expected).is_ok()
}
fn hex(bytes: &[u8]) -> String { bytes.iter().map(|b| format!("{b:02x}")).collect() }
fn unhex(text: &str) -> Result<Vec<u8>, ()> {
    if text.len() % 2 != 0 { return Err(()); }
    (0..text.len()).step_by(2).map(|i| u8::from_str_radix(&text[i..i+2],16).map_err(|_| ())).collect()
}
fn normalize_email(raw:&str)->Option<String>{
    let value=raw.trim().to_lowercase();
    if value.len()>254||value.len()<6||value.chars().any(|c|c.is_control()||c.is_whitespace()){return None}
    let (local,domain)=value.split_once('@')?;
    if local.is_empty()||domain.is_empty()||local.len()>64||!domain.contains('.')||domain.starts_with('.')||domain.ends_with('.')||domain.split('.').any(str::is_empty){return None}
    Some(value)
}
fn account_name(display_name:Option<&str>,email:&str)->Option<String>{
    let name=display_name.map(str::trim).filter(|s|!s.is_empty()).unwrap_or_else(||email.split('@').next().unwrap_or(email));
    if name.len()>64||name.chars().any(char::is_control){None}else{Some(name.to_string())}
}
fn random_token() -> Result<String, ()> {
    let mut bytes = [0u8; 32];
    SystemRandom::new().fill(&mut bytes).map_err(|_| ())?;
    Ok(hex(&bytes))
}
fn token_hash(token: &str) -> String { hex(digest::digest(&digest::SHA256, token.as_bytes()).as_ref()) }
fn session_cookie(token: &str) -> HeaderValue {
    let secure = std::env::var("SMART_MEDIA_COOKIE_SECURE").ok().as_deref() == Some("1");
    let mut value = format!("{COOKIE}={token}; Path=/; HttpOnly; SameSite=Strict; Max-Age={}", SESSION_DAYS * 86400);
    if secure { value.push_str("; Secure"); }
    HeaderValue::from_str(&value).expect("valid cookie")
}
fn clear_cookie() -> HeaderValue { HeaderValue::from_static("smart_media_session=; Path=/; HttpOnly; SameSite=Strict; Max-Age=0") }
fn set_session(path: &PathBuf, account_id: i64, profile_id: i64) -> Result<HeaderValue, ()> {
    let token = random_token()?;
    let now = chrono::Utc::now().timestamp();
    let c = conn(path);
    let _ = c.execute("DELETE FROM auth_sessions WHERE expires_at<?1", [now]);
    c.execute("INSERT INTO auth_sessions(token_hash,account_id,profile_id,created_at,expires_at) VALUES(?1,?2,?3,?4,?5)", params![token_hash(&token),account_id,profile_id,now,now+SESSION_DAYS*86400]).map_err(|_| ())?;
    Ok(session_cookie(&token))
}
fn request_token(headers: &HeaderMap) -> Option<String> {
    headers.get(header::COOKIE)?.to_str().ok()?.split(';').find_map(|part| {
        let (k,v) = part.trim().split_once('=')?;
        (k == COOKIE && v.len() == 64 && v.bytes().all(|b| b.is_ascii_hexdigit())).then(|| v.to_owned())
    })
}
pub(crate) fn principal(path: &PathBuf, headers: &HeaderMap) -> Option<Principal> {
    let token = request_token(headers)?;
    let c = conn(path);
    c.query_row("SELECT a.id,a.username,coalesce(a.email,''),a.role,p.id,p.name FROM auth_sessions s JOIN accounts a ON a.id=s.account_id JOIN profiles p ON p.id=s.profile_id WHERE s.token_hash=?1 AND s.expires_at>?2", params![token_hash(&token),chrono::Utc::now().timestamp()], |r| Ok(Principal{account_id:r.get(0)?,username:r.get(1)?,email:r.get(2)?,role:r.get(3)?,profile_id:r.get(4)?,profile_name:r.get(5)?})).ok()
}

pub async fn guard(State(path): State<PathBuf>, mut req: Request<Body>, next: Next) -> Response {
    let uri = req.uri().path();
    let public = uri == "/" || uri.starts_with("/assets/") || matches!(uri, "/api/auth/setup-status" | "/api/auth/setup" | "/api/auth/login");
    if let Some(p) = principal(&path, req.headers()) {
        let role_ok = if uri.starts_with("/api/owner/") || uri == "/api/subtitles/key" { p.role == "owner" }
            else if uri.starts_with("/api/admin/") { p.role == "owner" || p.role == "admin" }
            else { true };
        if !role_ok { return (StatusCode::FORBIDDEN, Json(json!({"error":"Your role cannot access this feature"}))).into_response(); }
        if p.role == "user" {
            let allowed = uri.starts_with("/api/files") || uri.starts_with("/api/movie-collections") || uri.starts_with("/api/catalogue/") || uri.starts_with("/api/playback/") || uri.starts_with("/api/libraries") || matches!(uri, "/api/server/info" | "/api/auth/me" | "/api/auth/logout" | "/api/auth/profile" | "/api/profiles" | "/api/user/settings");
            if !allowed { return (StatusCode::FORBIDDEN, Json(json!({"error":"User profiles can only browse and watch the library"}))).into_response(); }
        }
        req.extensions_mut().insert(p);
    } else if !public {
        return (StatusCode::UNAUTHORIZED, Json(json!({"error":"Please sign in"}))).into_response();
    }
    next.run(req).await
}

pub async fn setup_status(State(app): State<App>) -> Json<Value> {
    Json(json!({"configured":has_accounts(&app.db)}))
}
pub async fn setup(State(app): State<App>, Json(input): Json<Credentials>) -> Response {
    let Some(email)=normalize_email(&input.email) else{return (StatusCode::BAD_REQUEST,Json(json!({"error":"Enter a valid email address"}))).into_response()};
    let Some(username)=account_name(input.display_name.as_deref(),&email) else{return (StatusCode::BAD_REQUEST,Json(json!({"error":"Display names must be at most 64 characters"}))).into_response()};
    let password=input.password;
    let Ok(Ok(password_hash)) = tokio::task::spawn_blocking(move || hash_password(&password)).await else {
        return (StatusCode::BAD_REQUEST, Json(json!({"error":"Use a password between 12 and 1024 characters"}))).into_response();
    };
    let c = conn(&app.db);
    let Ok(tx) = c.unchecked_transaction() else { return StatusCode::INTERNAL_SERVER_ERROR.into_response(); };
    let exists: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM accounts)",[],|r|r.get(0)).unwrap_or(true);
    if exists { return (StatusCode::CONFLICT, Json(json!({"error":"Owner account setup is already complete"}))).into_response(); }
    let now=chrono::Utc::now().timestamp();
    if tx.execute("INSERT INTO accounts(username,email,password_hash,role,created_at) VALUES(?1,?2,?3,'owner',?4)",params![username,email,password_hash,now]).is_err() { return (StatusCode::CONFLICT,Json(json!({"error":"That email or display name is already in use"}))).into_response(); }
    let id=tx.last_insert_rowid();
    if tx.execute("INSERT INTO profiles(account_id,name,created_at) VALUES(?1,'Default',?2)",params![id,now]).is_err() { return StatusCode::INTERNAL_SERVER_ERROR.into_response(); }
    let profile=tx.last_insert_rowid();
    let _=tx.execute("INSERT INTO profile_watch_progress(profile_id,path,position,duration,updated_at,completed) SELECT ?1,path,position,duration,updated_at,completed FROM watch_progress",[profile]);
    if tx.commit().is_err() { return StatusCode::INTERNAL_SERVER_ERROR.into_response(); }
    match set_session(&app.db,id,profile) { Ok(cookie)=>([(header::SET_COOKIE,cookie)],Json(json!({"ok":true}))).into_response(),Err(_)=>StatusCode::INTERNAL_SERVER_ERROR.into_response() }
}
pub async fn login(State(app): State<App>, Json(input): Json<Credentials>) -> Response {
    let Some(email)=normalize_email(&input.email) else{return (StatusCode::UNAUTHORIZED,Json(json!({"error":"Incorrect email or password"}))).into_response()};
    let now=chrono::Utc::now().timestamp();
    let c=conn(&app.db);
    let blocked:i64=c.query_row("SELECT blocked_until FROM auth_login_attempts WHERE username=?1",[&email],|r|r.get(0)).unwrap_or(0);
    if blocked>now { return (StatusCode::TOO_MANY_REQUESTS,Json(json!({"error":"Too many attempts. Try again later."}))).into_response(); }
    let found=c.query_row("SELECT a.id,a.password_hash,p.id FROM accounts a JOIN profiles p ON p.account_id=a.id WHERE a.email=?1 ORDER BY p.id LIMIT 1",[&email],|r|Ok((r.get::<_,i64>(0)?,r.get::<_,String>(1)?,r.get::<_,i64>(2)?))).ok();
    const DUMMY_HASH:&str="pbkdf2$600000$00000000000000000000000000000000$0000000000000000000000000000000000000000000000000000000000000000";
    let check_hash=found.as_ref().map(|(_,h,_)|h.as_str()).unwrap_or(DUMMY_HASH).to_owned();
    let password=input.password;
    let verified=tokio::task::spawn_blocking(move || verify_password(&check_hash,&password)).await.unwrap_or(false);
    let Some((account,_hash,profile))=found.filter(|_|verified) else {
        let (attempts,started):(i64,i64)=c.query_row("SELECT attempts,window_started FROM auth_login_attempts WHERE username=?1",[&email],|r|Ok((r.get(0)?,r.get(1)?))).unwrap_or((0,now));
        let attempts=if now-started>900 {1}else{attempts+1};let blocked_until=if attempts>=10 {now+900}else{0};
        let _=c.execute("INSERT INTO auth_login_attempts(username,attempts,window_started,blocked_until) VALUES(?1,?2,?3,?4) ON CONFLICT(username) DO UPDATE SET attempts=excluded.attempts,window_started=excluded.window_started,blocked_until=excluded.blocked_until",params![email,attempts,if attempts==1{now}else{started},blocked_until]);
        return (StatusCode::UNAUTHORIZED,Json(json!({"error":"Incorrect email or password"}))).into_response();
    };
    let _=c.execute("DELETE FROM auth_login_attempts WHERE username=?1",[&email]);
    match set_session(&app.db,account,profile) { Ok(cookie)=>([(header::SET_COOKIE,cookie)],Json(json!({"ok":true}))).into_response(),Err(_)=>StatusCode::INTERNAL_SERVER_ERROR.into_response() }
}
pub async fn logout(State(app): State<App>, headers: HeaderMap) -> Response {
    if let Some(token)=request_token(&headers) { let _=conn(&app.db).execute("DELETE FROM auth_sessions WHERE token_hash=?1",[token_hash(&token)]); }
    ([(header::SET_COOKIE,clear_cookie())],Json(json!({"ok":true}))).into_response()
}
pub async fn me(State(app): State<App>, headers: HeaderMap) -> Response {
    let Some(p)=principal(&app.db,&headers) else { return StatusCode::UNAUTHORIZED.into_response(); };
    let profiles:Vec<Value>=conn(&app.db).prepare("SELECT id,name FROM profiles WHERE account_id=?1 ORDER BY id").and_then(|mut s|s.query_map([p.account_id],|r|Ok(json!({"id":r.get::<_,i64>(0)?,"name":r.get::<_,String>(1)?})))?.collect()).unwrap_or_default();
    Json(json!({"account_id":p.account_id,"username":p.username,"email":p.email,"role":p.role,"profile_id":p.profile_id,"profile_name":p.profile_name,"profiles":profiles})).into_response()
}
pub async fn profiles(State(app): State<App>, headers: HeaderMap, Json(input): Json<ProfileInput>) -> Response {
    let Some(p)=principal(&app.db,&headers) else { return StatusCode::UNAUTHORIZED.into_response(); };
    let name=input.name.trim();
    if name.is_empty()||name.len()>48||name.chars().any(char::is_control) { return (StatusCode::BAD_REQUEST,Json(json!({"error":"Profile names must be 1–48 characters"}))).into_response(); }
    match conn(&app.db).execute("INSERT INTO profiles(account_id,name,created_at) VALUES(?1,?2,?3)",params![p.account_id,name,chrono::Utc::now().timestamp()]) { Ok(_)=>Json(json!({"ok":true})).into_response(),Err(_)=>(StatusCode::CONFLICT,Json(json!({"error":"A profile with that name already exists"}))).into_response() }
}
pub async fn switch_profile(State(app): State<App>, headers: HeaderMap, Json(input): Json<Value>) -> Response {
    let Some(p)=principal(&app.db,&headers) else { return StatusCode::UNAUTHORIZED.into_response(); };
    let Some(id)=input["profile_id"].as_i64() else { return StatusCode::BAD_REQUEST.into_response(); };
    let exists:bool=conn(&app.db).query_row("SELECT EXISTS(SELECT 1 FROM profiles WHERE id=?1 AND account_id=?2)",params![id,p.account_id],|r|r.get(0)).unwrap_or(false);
    if !exists { return StatusCode::NOT_FOUND.into_response(); }
    if let Some(t)=request_token(&headers) { let _=conn(&app.db).execute("UPDATE auth_sessions SET profile_id=?1 WHERE token_hash=?2",params![id,token_hash(&t)]); }
    Json(json!({"ok":true})).into_response()
}
pub async fn admin_accounts(State(app): State<App>) -> Json<Value> {
    let rows:Vec<Value>=conn(&app.db).prepare("SELECT a.id,a.username,coalesce(a.email,''),a.role,a.created_at,COUNT(p.id) FROM accounts a LEFT JOIN profiles p ON p.account_id=a.id GROUP BY a.id ORDER BY a.created_at").and_then(|mut s|s.query_map([],|r|Ok(json!({"id":r.get::<_,i64>(0)?,"username":r.get::<_,String>(1)?,"email":r.get::<_,String>(2)?,"role":r.get::<_,String>(3)?,"created_at":r.get::<_,i64>(4)?,"profiles":r.get::<_,i64>(5)?})))?.collect()).unwrap_or_default();
    Json(json!({"accounts":rows}))
}
pub async fn create_account(State(app): State<App>, axum::Extension(actor): axum::Extension<Principal>, Json(input): Json<AccountInput>) -> Response {
    let Some(email)=normalize_email(&input.email) else{return (StatusCode::BAD_REQUEST,Json(json!({"error":"Enter a valid email address"}))).into_response()};
    let Some(username)=account_name(input.display_name.as_deref(),&email) else{return (StatusCode::BAD_REQUEST,Json(json!({"error":"Display names must be at most 64 characters"}))).into_response()};
    let role=input.role.as_deref().unwrap_or("user");
    if !matches!(role,"admin"|"user")||(role=="admin"&&actor.role!="owner") { return (StatusCode::BAD_REQUEST,Json(json!({"error":"Choose a role permitted for your account"}))).into_response(); }
    let password=input.password;
    let Ok(Ok(hash))=tokio::task::spawn_blocking(move || hash_password(&password)).await else { return (StatusCode::BAD_REQUEST,Json(json!({"error":"Passwords must be 12–1024 characters"}))).into_response(); };
    let c=conn(&app.db); let now=chrono::Utc::now().timestamp();
    match c.execute("INSERT INTO accounts(username,email,password_hash,role,created_at) VALUES(?1,?2,?3,?4,?5)",params![username,email,hash,role,now]) {
        Ok(_)=>{let id=c.last_insert_rowid();let _=c.execute("INSERT INTO profiles(account_id,name,created_at) VALUES(?1,'Default',?2)",params![id,now]);Json(json!({"ok":true})).into_response()},
        Err(_)=>(StatusCode::CONFLICT,Json(json!({"error":"That email or display name is already in use"}))).into_response()
    }
}
pub fn reset_owner_credentials(path:&PathBuf,email:&str,password:&str)->Result<(),String>{
    let email=normalize_email(email).ok_or_else(||"Invalid email address".to_string())?;
    let hash=hash_password(password).map_err(|_|"Password must contain 12–1024 characters".to_string())?;
    let c=conn(path);
    let changed=c.execute("UPDATE accounts SET email=?1,password_hash=?2 WHERE role='owner'",params![email,hash]).map_err(|_|"Email is already used by another account".to_string())?;
    if changed!=1{return Err("No owner account exists in this database".to_string())}
    c.execute("DELETE FROM auth_sessions WHERE account_id=(SELECT id FROM accounts WHERE role='owner')",[]).map_err(|e|e.to_string())?;
    Ok(())
}
pub async fn update_account_credentials(State(app): State<App>, axum::Extension(actor): axum::Extension<Principal>, Json(input): Json<OwnerCredentialsInput>) -> Response {
    let Some(email)=normalize_email(&input.email) else{return (StatusCode::BAD_REQUEST,Json(json!({"error":"Enter a valid email address"}))).into_response()};
    let password=input.password;
    let Ok(Ok(hash))=tokio::task::spawn_blocking(move||hash_password(&password)).await else{return (StatusCode::BAD_REQUEST,Json(json!({"error":"Passwords must be 12–1024 characters"}))).into_response()};
    let c=conn(&app.db);
    let exists:bool=c.query_row("SELECT EXISTS(SELECT 1 FROM accounts WHERE id=?1)",[input.account_id],|r|r.get(0)).unwrap_or(false);
    if !exists{return StatusCode::NOT_FOUND.into_response()}
    match c.execute("UPDATE accounts SET email=?1,password_hash=?2 WHERE id=?3",params![email,hash,input.account_id]){
        Ok(_)=>{let _=c.execute("DELETE FROM auth_sessions WHERE account_id=?1",[input.account_id]);Json(json!({"ok":true,"login_again":input.account_id==actor.account_id})).into_response()},
        Err(_)=>(StatusCode::CONFLICT,Json(json!({"error":"That email is already used by another account"}))).into_response()
    }
}
pub async fn admin_dashboard(State(app): State<App>) -> Json<Value> {
    let active:Vec<Value>=conn(&app.db).prepare("SELECT a.username,p.name,x.path,x.position,x.duration,x.mode,x.updated_at,x.started_at,f.size FROM playback_activity x JOIN profiles p ON p.id=x.profile_id JOIN accounts a ON a.id=p.account_id LEFT JOIN files f ON f.path=x.path WHERE x.active=1 AND x.updated_at>?1 ORDER BY x.updated_at DESC").and_then(|mut s|s.query_map([chrono::Utc::now().timestamp()-90],|r|{let position=r.get::<_,f64>(3)?;let duration=r.get::<_,f64>(4)?;let started=r.get::<_,i64>(7)?;let size=r.get::<_,Option<i64>>(8)?.unwrap_or(0);Ok(json!({"username":r.get::<_,String>(0)?,"profile":r.get::<_,String>(1)?,"path":r.get::<_,String>(2)?,"position":position,"duration":duration,"mode":r.get::<_,String>(5)?,"updated_at":r.get::<_,i64>(6)?,"started_at":started,"estimated_bytes":if duration>0.0{(size as f64*(position/duration).clamp(0.0,1.0)) as i64}else{0}}))})?.collect()).unwrap_or_default();
    let accounts: i64=conn(&app.db).query_row("SELECT COUNT(*) FROM accounts",[],|r|r.get(0)).unwrap_or(0);
    Json(json!({"active":active,"account_count":accounts,"usage_note":"Data is estimated from each file's size and playback position; it is not measured network traffic."}))
}
pub async fn user_settings(State(app): State<App>, headers: HeaderMap) -> Response {
    let Some(p)=principal(&app.db,&headers) else{return StatusCode::UNAUTHORIZED.into_response()};
    let rows:Value=conn(&app.db).prepare("SELECT key,value FROM profile_settings WHERE profile_id=?1").and_then(|mut s|s.query_map([p.profile_id],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?)))?.collect::<Result<Vec<_>,_>>()).map(|v|json!(v.into_iter().collect::<std::collections::HashMap<_,_>>())).unwrap_or(json!({})); Json(rows).into_response()
}
pub async fn save_user_setting(State(app): State<App>, headers: HeaderMap, Json(input): Json<SettingInput>) -> Response {
    let Some(p)=principal(&app.db,&headers) else{return StatusCode::UNAUTHORIZED.into_response()};
    let valid=match input.key.as_str(){
        "playback_quality"=>matches!(input.value.as_str(),"auto"|"1080"|"720"|"480"),
        "playback_speed"=>matches!(input.value.as_str(),"0.5"|"0.75"|"1"|"1.25"|"1.5"|"2"),
        "subtitle_language"=>matches!(input.value.as_str(),""|"en"|"hr"|"sl"|"de"|"es"|"fr"|"it"|"ja"|"ko"|"pt"|"ru"|"zh"),
        "auto_play_next"|"show_continue"|"show_recommendations"|"show_popular"|"reduce_motion"=>matches!(input.value.as_str(),"true"|"false"),
        "continue_limit"=>matches!(input.value.as_str(),"6"|"12"|"24"|"50"),
        "accent_color"=>matches!(input.value.as_str(),"amber"|"blue"|"mint"),
        _=>false,
    };
    if !valid{return (StatusCode::BAD_REQUEST,Json(json!({"error":"That profile setting or value is not supported"}))).into_response()}
    match conn(&app.db).execute("INSERT INTO profile_settings(profile_id,key,value) VALUES(?1,?2,?3) ON CONFLICT(profile_id,key) DO UPDATE SET value=excluded.value",params![p.profile_id,input.key,input.value]) { Ok(_)=>Json(json!({"ok":true})).into_response(),Err(_)=>StatusCode::INTERNAL_SERVER_ERROR.into_response() }
}
pub async fn server_info(State(app): State<App>) -> Json<Value> {
    let name=conn(&app.db).query_row("SELECT value FROM server_settings WHERE key='friendly_name'",[],|r|r.get::<_,String>(0)).unwrap_or_else(|_|"Smart Media".to_string());
    Json(json!({"friendly_name":name}))
}
pub async fn server_settings(State(app): State<App>) -> Json<Value> {
    let rows:Value=conn(&app.db).prepare("SELECT key,value FROM server_settings").and_then(|mut s|s.query_map([],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?)))?.collect::<Result<Vec<_>,_>>()).map(|v|json!(v.into_iter().collect::<std::collections::HashMap<_,_>>())).unwrap_or(json!({})); Json(rows)
}
pub async fn save_server_setting(State(app): State<App>, Json(input): Json<SettingInput>) -> Response {
    if input.key=="friendly_name" {
        let name=input.value.trim();
        if name.is_empty()||name.chars().count()>48||name.chars().any(char::is_control){return (StatusCode::BAD_REQUEST,Json(json!({"error":"Server name must be 1–48 characters"}))).into_response()}
        return match conn(&app.db).execute("INSERT INTO server_settings(key,value) VALUES('friendly_name',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value",[name]) { Ok(_)=>Json(json!({"ok":true})).into_response(),Err(_)=>StatusCode::INTERNAL_SERVER_ERROR.into_response() };
    }
    if input.key!="hardware_encoder"||input.value.len()>64{return StatusCode::BAD_REQUEST.into_response()}
    let available=crate::playback::available_h264_encoders().await;
    if input.value!="auto"&&!available.contains(&input.value.as_str()){return (StatusCode::BAD_REQUEST,Json(json!({"error":"That H.264 encoder is not listed by this FFmpeg build"}))).into_response()}
    match conn(&app.db).execute("INSERT INTO server_settings(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value",params![input.key,input.value]) { Ok(_)=>Json(json!({"ok":true})).into_response(),Err(_)=>StatusCode::INTERNAL_SERVER_ERROR.into_response() }
}
pub async fn owner_encoders() -> Json<Value> { Json(json!({"encoders":crate::playback::available_h264_encoders().await,"note":"Each stream smoke-tests the selected encoder and falls back to software if the host or container cannot initialize it."})) }
pub async fn record_activity(State(app): State<App>, headers: HeaderMap, Json(input): Json<Value>) -> Response {
    let Some(p)=principal(&app.db,&headers) else{return StatusCode::UNAUTHORIZED.into_response()};
    let path=input["path"].as_str().unwrap_or("");
    if !crate::is_library_path(&app,path){return StatusCode::NOT_FOUND.into_response()}
    let position=input["position"].as_f64().unwrap_or(0.0).clamp(0.0,86400.0);let duration=input["duration"].as_f64().unwrap_or(0.0).clamp(0.0,86400.0);let mode=input["mode"].as_str().filter(|s|matches!(*s,"direct"|"transcode"|"hls")).unwrap_or("direct");let active=input["active"].as_bool().unwrap_or(true);let now=chrono::Utc::now().timestamp();
    match conn(&app.db).execute("INSERT INTO playback_activity(profile_id,path,position,duration,mode,active,started_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?7) ON CONFLICT(profile_id,path) DO UPDATE SET position=excluded.position,duration=excluded.duration,mode=excluded.mode,active=excluded.active,started_at=CASE WHEN playback_activity.active=0 AND excluded.active=1 THEN excluded.started_at ELSE playback_activity.started_at END,updated_at=excluded.updated_at",params![p.profile_id,path,position,duration,mode,active,now]){Ok(_)=>Json(json!({"ok":true})).into_response(),Err(_)=>StatusCode::INTERNAL_SERVER_ERROR.into_response()}
}

pub async fn progress_get(State(app): State<App>, headers: HeaderMap, axum::extract::Query(query): axum::extract::Query<ProgressQuery>) -> Response {
    let Some(p)=principal(&app.db,&headers) else{return StatusCode::UNAUTHORIZED.into_response()};
    let c=conn(&app.db);
    if let Some(path)=query.path {
        if !crate::is_library_path(&app,&path){return StatusCode::NOT_FOUND.into_response()}
        let progress=c.query_row("SELECT position,duration,updated_at FROM profile_watch_progress WHERE profile_id=?1 AND path=?2 AND completed=0",params![p.profile_id,path],|r|Ok(json!({"position":r.get::<_,f64>(0)?,"duration":r.get::<_,f64>(1)?,"updated_at":r.get::<_,i64>(2)?}))).ok();
        let completed=c.query_row("SELECT EXISTS(SELECT 1 FROM profile_watch_progress WHERE profile_id=?1 AND path=?2 AND completed=1)",params![p.profile_id,path],|r|r.get::<_,bool>(0)).unwrap_or(false);
        return Json(json!({"progress":progress,"completed":completed})).into_response()
    }
    let items:Vec<Value>=c.prepare("SELECT w.path,w.position,w.duration,w.updated_at,f.result FROM profile_watch_progress w JOIN files f ON f.path=w.path WHERE w.profile_id=?1 AND w.completed=0 AND w.position>10 AND w.duration>w.position+10 ORDER BY w.updated_at DESC").and_then(|mut s|s.query_map([p.profile_id],|r|{let raw:String=r.get(4)?;Ok(json!({"path":r.get::<_,String>(0)?,"position":r.get::<_,f64>(1)?,"duration":r.get::<_,f64>(2)?,"updated_at":r.get::<_,i64>(3)?,"result":serde_json::from_str::<Value>(&raw).unwrap_or_default()}))})?.collect()).unwrap_or_default();
    let history:Vec<Value>=c.prepare("SELECT w.path,w.position,w.duration,w.updated_at,w.completed,f.result FROM profile_watch_progress w JOIN files f ON f.path=w.path WHERE w.profile_id=?1 ORDER BY w.updated_at DESC LIMIT 200").and_then(|mut s|s.query_map([p.profile_id],|r|{let raw:String=r.get(5)?;Ok(json!({"path":r.get::<_,String>(0)?,"position":r.get::<_,f64>(1)?,"duration":r.get::<_,f64>(2)?,"updated_at":r.get::<_,i64>(3)?,"completed":r.get::<_,bool>(4)?,"result":serde_json::from_str::<Value>(&raw).unwrap_or_default()}))})?.collect()).unwrap_or_default();
    let watched:i64=c.query_row("SELECT COUNT(*) FROM profile_watch_progress WHERE profile_id=?1 AND completed=1",[p.profile_id],|r|r.get(0)).unwrap_or(0);
    Json(json!({"items":items,"history":history,"watched_count":watched})).into_response()
}
pub async fn progress_post(State(app): State<App>, headers: HeaderMap, Json(input): Json<Value>) -> Response {
    let Some(p)=principal(&app.db,&headers) else{return StatusCode::UNAUTHORIZED.into_response()};
    let path=input["path"].as_str().unwrap_or("");let pos=input["position"].as_f64().unwrap_or(-1.0);let dur=input["duration"].as_f64().unwrap_or(-1.0);let completed=input["completed"].as_bool().unwrap_or(false);
    if !crate::is_library_path(&app,path)||!pos.is_finite()||!(0.0..=86400.0).contains(&pos)||!dur.is_finite()||!(0.0..=86400.0).contains(&dur){return (StatusCode::BAD_REQUEST,Json(json!({"error":"Invalid playback progress"}))).into_response()}
    let completed=completed||(dur>0.0&&pos>=dur-8.0);let now=chrono::Utc::now().timestamp();
    let result=if completed{conn(&app.db).execute("INSERT INTO profile_watch_progress(profile_id,path,position,duration,updated_at,completed) VALUES(?1,?2,?3,?4,?5,1) ON CONFLICT(profile_id,path) DO UPDATE SET position=excluded.position,duration=excluded.duration,updated_at=excluded.updated_at,completed=1",params![p.profile_id,path,dur,dur,now])}else{conn(&app.db).execute("INSERT INTO profile_watch_progress(profile_id,path,position,duration,updated_at,completed) VALUES(?1,?2,?3,?4,?5,0) ON CONFLICT(profile_id,path) DO UPDATE SET position=excluded.position,duration=excluded.duration,updated_at=excluded.updated_at,completed=0",params![p.profile_id,path,pos,dur,now])};
    match result{Ok(_)=>(StatusCode::OK,Json(json!({"ok":true,"completed":completed}))).into_response(),Err(_)=>StatusCode::INTERNAL_SERVER_ERROR.into_response()}
}

pub async fn auth_page(State(app): State<App>) -> Html<String> {
    let setup=!has_accounts(&app.db);
    let title=if setup{"Create your server owner account"}else{"Sign in to your media server"};
    let fields=if setup{"<label>Email<input id=email type=email autocomplete=email required maxlength=254></label><label>Display name<input id=display-name autocomplete=name maxlength=64></label><label>Password (12 characters minimum)<input id=password type=password autocomplete=new-password required minlength=12></label><label>Confirm password<input id=confirm type=password autocomplete=new-password required minlength=12></label>"}else{"<label>Email<input id=email type=email autocomplete=username required maxlength=254></label><label>Password<input id=password type=password autocomplete=current-password required></label>"};
    let recovery=if setup{""}else{"<p class=recovery>Forgot the server owner's password? Run <code>./target/release/smart-media-scanner reset-owner-password</code> in the server shell. For Docker, run that command in an interactive shell inside the app container (use the executable's path if needed). It updates the owner credentials and signs out existing owner sessions.</p>"};
    Html(format!("<!doctype html><html><meta name=viewport content='width=device-width,initial-scale=1'><title>{title}</title><style>body{{margin:0;background:#0c1017;color:#eef2f7;font:16px system-ui;display:grid;place-items:center;min-height:100vh}}form{{width:min(390px,88vw);background:#171d28;padding:30px;border:1px solid #2c3544;border-radius:16px}}h1{{font-size:24px}}p{{color:#98a5b7;line-height:1.5}}.recovery{{border-top:1px solid #2c3544;margin:22px 0 0;padding-top:16px;font-size:13px}}code{{color:#e6ad55;overflow-wrap:anywhere}}label{{display:block;margin:18px 0;color:#aeb8c6}}input{{display:block;box-sizing:border-box;width:100%;padding:12px;margin-top:6px;color:white;background:#0d131c;border:1px solid #364255;border-radius:8px}}button{{width:100%;padding:12px;background:#e6ad55;border:0;border-radius:8px;font-weight:700;cursor:pointer}}#error{{color:#ff9c9c}}</style><form id=form><h1>{title}</h1><p>{}</p>{fields}<p id=error></p><button>{}</button>{recovery}</form><script>const formEl=document.getElementById('form'),errorEl=document.getElementById('error');formEl.onsubmit=async e=>{{e.preventDefault();errorEl.textContent='';const email=document.getElementById('email').value,password=document.getElementById('password').value,display_name=document.getElementById('display-name')?.value||undefined;if({setup}&&password!==document.getElementById('confirm').value){{errorEl.textContent='Passwords do not match';return}}let r=await fetch('/api/auth/{}',{{method:'POST',headers:{{'Content-Type':'application/json'}},body:JSON.stringify({{email,password,display_name}})}});let d=await r.json().catch(()=>({{}}));if(!r.ok){{errorEl.textContent=d.error||'Could not sign in';return}}location.href='/'}}</script></html>",if setup{"The owner controls critical server settings. Use an email you can recover; passwords are stored as a salted, slow hash."}else{"Sign in with the email linked to your account."},if setup{"Create owner account"}else{"Sign in"},if setup{"setup"}else{"login"}))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn password_hash_round_trips_and_is_salted() {
        let first=hash_password("a sufficiently long password").unwrap();
        let second=hash_password("a sufficiently long password").unwrap();
        assert_ne!(first,second);
        assert!(verify_password(&first,"a sufficiently long password"));
        assert!(!verify_password(&first,"another password"));
        assert_eq!(first.split('$').count(),4);
    }
    #[test]
    fn only_one_owner_can_be_created() {
        let dir=tempfile::tempdir().unwrap();let path=dir.path().join("auth.sqlite");
        init(&path);let c=conn(&path);
        c.execute("INSERT INTO accounts(username,password_hash,role,created_at) VALUES('owner','hash','owner',0)",[]).unwrap();
        assert!(c.execute("INSERT INTO accounts(username,password_hash,role,created_at) VALUES('second','hash','owner',0)",[]).is_err());
    }
    #[test]
    fn email_login_migration_and_shell_reset_update_owner_identity() {
        let dir=tempfile::tempdir().unwrap();let path=dir.path().join("auth.sqlite");
        Connection::open(&path).unwrap().execute_batch("CREATE TABLE accounts(id INTEGER PRIMARY KEY,username TEXT NOT NULL COLLATE NOCASE UNIQUE,password_hash TEXT NOT NULL,role TEXT NOT NULL,created_at INTEGER NOT NULL); INSERT INTO accounts(username,password_hash,role,created_at) VALUES('owner','old','owner',0);").unwrap();
        init(&path);let c=conn(&path);
        c.execute("INSERT INTO profiles(account_id,name,created_at) VALUES(1,'Default',0)",[]).unwrap();
        c.execute("INSERT INTO auth_sessions(token_hash,account_id,profile_id,created_at,expires_at) VALUES('session',1,1,0,9999999999)",[]).unwrap();
        reset_owner_credentials(&path,"OWNER@EXAMPLE.COM","a new owner password").unwrap();
        let (email,hash):(String,String)=c.query_row("SELECT email,password_hash FROM accounts WHERE role='owner'",[],|r|Ok((r.get(0)?,r.get(1)?))).unwrap();
        assert_eq!(email,"owner@example.com");assert!(verify_password(&hash,"a new owner password"));
        assert_eq!(c.query_row("SELECT COUNT(*) FROM auth_sessions",[],|r|r.get::<_,i64>(0)).unwrap(),0);
        assert!(normalize_email("not-an-email").is_none());
    }
}
