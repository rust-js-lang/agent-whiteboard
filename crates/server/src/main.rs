//! Same document engine, native SQLite authority. Serve through HTTPS for remote use.
use axum::{
    Json, Router,
    extract::{Path, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::{Value, json};
use std::{
    env,
    sync::{Arc, Mutex},
};
use tower_http::{
    limit::RequestBodyLimitLayer,
    services::{ServeDir, ServeFile},
};
use uuid::Uuid;
use whiteboard_core::{Command, Journal};

#[derive(Clone)]
struct AppState {
    db: Arc<Mutex<Connection>>,
    token: Arc<str>,
    secure_cookie: bool,
}
type ApiResult = Result<Response, ApiError>;
struct ApiError(StatusCode, String);
impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.0,
            [(header::CACHE_CONTROL, "no-store")],
            Json(json!({"error": self.1})),
        )
            .into_response()
    }
}
fn error(message: impl ToString) -> ApiError {
    ApiError(StatusCode::BAD_REQUEST, message.to_string())
}
fn internal(message: impl ToString) -> ApiError {
    ApiError(StatusCode::INTERNAL_SERVER_ERROR, message.to_string())
}
fn valid_room(room: &str) -> bool {
    !room.is_empty()
        && room.len() <= 64
        && room
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"_-".contains(&c))
}
fn check_origin(headers: &HeaderMap) -> Result<(), ApiError> {
    let host = headers
        .get(header::HOST)
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| error("Missing host."))?;
    let origin = headers
        .get(header::ORIGIN)
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| {
            ApiError(
                StatusCode::FORBIDDEN,
                "A same-origin request is required.".into(),
            )
        })?;
    if origin != format!("http://{host}") && origin != format!("https://{host}") {
        return Err(ApiError(
            StatusCode::FORBIDDEN,
            "Origin is not allowed.".into(),
        ));
    }
    Ok(())
}
fn actor(db: &Connection, headers: &HeaderMap) -> Result<String, ApiError> {
    let cookie = headers
        .get(header::COOKIE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let session = cookie
        .split(';')
        .find_map(|part| part.trim().strip_prefix("whiteboard_session="))
        .ok_or_else(|| {
            ApiError(
                StatusCode::UNAUTHORIZED,
                "Sign in to join this room.".into(),
            )
        })?;
    db.query_row(
        "SELECT actor FROM sessions WHERE id = ?1",
        [session],
        |row| row.get(0),
    )
    .optional()
    .map_err(internal)?
    .ok_or_else(|| {
        ApiError(
            StatusCode::UNAUTHORIZED,
            "Session expired. Sign in again.".into(),
        )
    })
}
async fn blocking<T: Send + 'static>(
    state: AppState,
    action: impl FnOnce(&mut Connection) -> Result<T, ApiError> + Send + 'static,
) -> Result<T, ApiError> {
    tokio::task::spawn_blocking(move || {
        let mut db = state
            .db
            .lock()
            .map_err(|_| internal("Storage lock failed."))?;
        action(&mut db)
    })
    .await
    .map_err(internal)?
}
async fn login(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<Value>,
) -> ApiResult {
    check_origin(&headers)?;
    if input.get("token").and_then(Value::as_str) != Some(state.token.as_ref()) {
        return Err(ApiError(
            StatusCode::UNAUTHORIZED,
            "Incorrect access key.".into(),
        ));
    }
    let session = Uuid::new_v4().to_string();
    let cookie = format!(
        "whiteboard_session={session}; Path=/; HttpOnly; SameSite=Strict; Max-Age=2592000{}",
        if state.secure_cookie { "; Secure" } else { "" }
    );
    blocking(state, move |db| {
        db.execute(
            "INSERT INTO sessions(id, actor) VALUES (?1, ?2)",
            params![session, Uuid::new_v4().to_string()],
        )
        .map_err(internal)?;
        Ok(())
    })
    .await?;
    let mut response = Json(json!({"ok":true})).into_response();
    response.headers_mut().insert(
        header::SET_COOKIE,
        HeaderValue::from_str(&cookie).map_err(internal)?,
    );
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    Ok(response)
}
async fn read_board(
    State(state): State<AppState>,
    Path(room): Path<String>,
    headers: HeaderMap,
) -> ApiResult {
    if !valid_room(&room) {
        return Err(error("Invalid room ID."));
    }
    let result = blocking(state, move |db| {
        let actor = actor(db, &headers)?;
        let record: Option<String> = db
            .query_row("SELECT record FROM boards WHERE id = ?1", [&room], |row| {
                row.get(0)
            })
            .optional()
            .map_err(internal)?;
        Ok(json!({"record":record.unwrap_or_else(whiteboard_core::initial_record),"actor":actor}))
    })
    .await?;
    Ok(([(header::CACHE_CONTROL, "no-store")], Json(result)).into_response())
}
async fn submit(
    State(state): State<AppState>,
    Path(room): Path<String>,
    headers: HeaderMap,
    Json(command): Json<Command>,
) -> ApiResult {
    check_origin(&headers)?;
    if !valid_room(&room) {
        return Err(error("Invalid room ID."));
    }
    let result = blocking(state, move |db| {
        let actor = actor(db, &headers)?;
        let tx = db.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate).map_err(internal)?;
        let record: Option<String> = tx.query_row("SELECT record FROM boards WHERE id = ?1", [&room], |row| row.get(0)).optional().map_err(internal)?;
        let journal: Journal = match record { Some(record) => serde_json::from_str(&record).map_err(error)?, None => Journal::default() };
        let accepted = journal.accept(command, &actor).map_err(|message| ApiError(if message.starts_with("Conflict:") { StatusCode::CONFLICT } else { StatusCode::BAD_REQUEST }, message))?;
        let record = serde_json::to_string(&accepted.journal).map_err(internal)?;
        if record.len() > 32 * 1024 * 1024 { return Err(error("Room storage limit reached. Export before starting another room.")); }
        if accepted.accepted { tx.execute("INSERT INTO boards(id,record) VALUES (?1,?2) ON CONFLICT(id) DO UPDATE SET record=excluded.record", params![room,record]).map_err(internal)?; }
        tx.commit().map_err(internal)?;
        Ok(json!({"record":record,"actor":actor,"receipt":accepted.receipt,"accepted":accepted.accepted}))
    }).await?;
    Ok(([(header::CACHE_CONTROL, "no-store")], Json(result)).into_response())
}
#[tokio::main]
async fn main() {
    let token =
        env::var("WHITEBOARD_TOKEN").expect("Set WHITEBOARD_TOKEN to the server access key.");
    assert!(
        token.len() >= 12,
        "WHITEBOARD_TOKEN must have at least 12 characters."
    );
    let database = env::var("WHITEBOARD_DB").unwrap_or_else(|_| "whiteboard.db".into());
    let db = Connection::open(database).expect("open SQLite database");
    db.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL; CREATE TABLE IF NOT EXISTS boards(id TEXT PRIMARY KEY, record TEXT NOT NULL); CREATE TABLE IF NOT EXISTS sessions(id TEXT PRIMARY KEY, actor TEXT NOT NULL);").expect("initialize database");
    let state = AppState {
        db: Arc::new(Mutex::new(db)),
        token: Arc::from(token),
        secure_cookie: env::var("WHITEBOARD_SECURE_COOKIE").as_deref() != Ok("false"),
    };
    let dist = env::var("WHITEBOARD_DIST").unwrap_or_else(|_| "dist".into());
    let assets =
        ServeDir::new(&dist).not_found_service(ServeFile::new(format!("{dist}/index.html")));
    let app = Router::new()
        .route("/api/session", post(login))
        .route("/api/rooms/{room}", get(read_board).post(submit))
        .route(
            "/api/health",
            get(|| async { Json(json!({"service":"agent-whiteboard"})) }),
        )
        .layer(RequestBodyLimitLayer::new(1024 * 1024))
        .fallback_service(assets)
        .with_state(state);
    let bind = env::var("WHITEBOARD_BIND").unwrap_or_else(|_| "127.0.0.1:4080".into());
    let listener = tokio::net::TcpListener::bind(&bind)
        .await
        .expect("bind server");
    println!("AgentWhiteboard server: http://{bind}/#room=studio");
    axum::serve(listener, app).await.expect("serve whiteboard");
}
