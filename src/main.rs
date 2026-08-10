mod forum;

use askama::{Result, Template};
use axum::{
    Router,
    extract::{Form, Path, State},
    http::{HeaderMap, HeaderValue, StatusCode, header::SET_COOKIE},
    response::{Html, Redirect},
    routing::{get, post},
};
use sha2::{Digest, Sha256};
use sqlx::{SqlitePool, sqlite::SqliteConnectOptions};
use std::{
    collections::{HashMap, VecDeque},
    str::FromStr,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tower_http::services::ServeDir;
use uuid::Uuid;

#[derive(Template)]
#[template(path = "home.html")]
struct HomeTemplate<'a> {
    site_name: &'a str,
    boards: &'a [forum::Board],
}

#[derive(Template)]
#[template(path = "404.html")]
struct NotFoundTemplate;

#[derive(Template)]
#[template(path = "board.html")]
struct BoardTemplate<'a> {
    board: &'a forum::Board,
}

#[derive(Template)]
#[template(path = "new_thread.html")]
struct NewThreadTemplate<'a> {
    board: &'a forum::Board,
}

#[derive(serde::Deserialize)]
struct NewThreadForm {
    title: String,
    body: String,
}

#[derive(serde::Deserialize)]
struct ReplyForm {
    body: String,
}

#[derive(serde::Deserialize)]
struct ReportForm {
    reason: String,
}

#[derive(Template)]
#[template(path = "thread.html")]
struct ThreadTemplate<'a> {
    board: &'a forum::Board,
    thread: &'a forum::Thread,
}

struct RateLimiter {
    requests: Mutex<HashMap<String, VecDeque<Instant>>>,
}

impl RateLimiter {
    fn new() -> Self {
        Self {
            requests: Mutex::new(HashMap::new()),
        }
    }

    fn allow(&self, key: &str, limit: usize, window: Duration) -> bool {
        let now = Instant::now();
        let cutoff = now - window;
        let mut requests = self.requests.lock().expect("Rate limiter mutex poisoned");

        let timestamps = requests.entry(key.to_owned()).or_default();

        while timestamps.front().is_some_and(|time| *time <= cutoff) {
            timestamps.pop_front();
        }

        if timestamps.len() >= limit {
            return false;
        }

        timestamps.push_back(now);
        true
    }
}

struct AppState {
    pool: SqlitePool,
    rate_limiter: RateLimiter,
}

const ANONYMOUS_COOKIE: &str = "mchan_anon";

fn anonymous_token(headers: &HeaderMap) -> (String, bool) {
    let Some(cookie_header) = headers.get("cookie").and_then(|value| value.to_str().ok()) else {
        return (Uuid::new_v4().to_string(), true);
    };

    for part in cookie_header.split(';') {
        let Some((name, value)) = part.trim().split_once('=') else {
            continue;
        };

        if name == ANONYMOUS_COOKIE && !value.is_empty() {
            return (value.to_owned(), false);
        }
    }
    (Uuid::new_v4().to_string(), true)
}

fn client_key(headers: &HeaderMap) -> String {
    if let Some(value) = headers
        .get("cf-connecting-ip")
        .and_then(|value| value.to_str().ok())
    {
        return value.to_owned();
    }

    if let Some(value) = headers
        .get("x-forwarded-for")
        .and_then(|value| value.to_str().ok())
    {
        if let Some(first_ip) = value.split(',').next() {
            return first_ip.trim().to_owned();
        }
    }

    String::from("local")
}

pub(crate) fn thread_poster_id(token: &str, thread_id: u64) -> String {
    let mut digest = Sha256::new();
    digest.update(token.as_bytes());
    digest.update(thread_id.to_be_bytes());

    let hash = digest.finalize();

    format!(
        "Anonymous ##{:02x}{:02x}{:02x}{:02x}",
        hash[0], hash[1], hash[2], hash[3]
    )
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let database_url =
        std::env::var("DATABASE_URL").unwrap_or_else(|_| String::from("sqlite://mchan.db"));

    let options = SqliteConnectOptions::from_str(&database_url)?.create_if_missing(true);
    let pool = SqlitePool::connect_with(options).await?;
    sqlx::migrate!().run(&pool).await?;

    let state = Arc::new(AppState {
        pool,
        rate_limiter: RateLimiter::new(),
    });

    let app = Router::new()
        .route("/", get(home))
        .route("/boards/{slug}", get(board))
        .route("/boards/{slug}/new", get(new_thread))
        .route("/boards/{slug}/threads", post(create_thread))
        .route("/threads/{id}", get(thread))
        .route("/threads/{id}/replies", post(create_reply))
        .route("/threads/{id}/report", post(report_thread))
        .route("/replies/{id}/report", post(report_reply))
        .nest_service("/static", ServeDir::new("static"))
        .fallback(not_found)
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();

    axum::serve(listener, app).await?;

    Ok(())
}

fn not_found_response() -> (StatusCode, Html<String>) {
    let page = NotFoundTemplate;

    (StatusCode::NOT_FOUND, Html(page.render().unwrap()))
}

async fn not_found() -> (StatusCode, Html<String>) {
    not_found_response()
}

async fn home(State(state): State<Arc<AppState>>) -> Result<Html<String>, StatusCode> {
    let boards = forum::load_approved_boards(&state.pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let template = HomeTemplate {
        site_name: "M-chan",
        boards: &boards,
    };

    Ok(Html(template.render().unwrap()))
}

async fn board(
    Path(slug): Path<String>,
    State(state): State<Arc<AppState>>,
) -> Result<Html<String>, (StatusCode, Html<String>)> {
    let board = forum::load_board(&state.pool, &slug).await.map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Html(String::from("Database error")),
        )
    })?;

    let Some(board) = board else {
        return Err(not_found_response());
    };

    let template = BoardTemplate { board: &board };

    Ok(Html(template.render().unwrap()))
}

async fn new_thread(
    Path(slug): Path<String>,
    State(state): State<Arc<AppState>>,
) -> Result<Html<String>, (StatusCode, Html<String>)> {
    let board = forum::load_board(&state.pool, &slug).await.map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Html(String::from("Database error")),
        )
    })?;

    let Some(board) = board else {
        return Err(not_found_response());
    };

    let template = NewThreadTemplate { board: &board };

    Ok(Html(template.render().unwrap()))
}

async fn create_thread(
    Path(slug): Path<String>,
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Form(form): Form<NewThreadForm>,
) -> Result<(HeaderMap, Redirect), (StatusCode, Html<String>)> {
    let title = form.title.trim();
    let body = form.body.trim();

    if title.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Html(String::from("Thread title cannot be empty.")),
        ));
    }

    if body.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Html(String::from("Thread body cannot be empty")),
        ));
    }

    if title.chars().count() > 120 {
        return Err((
            StatusCode::BAD_REQUEST,
            Html(String::from("Thread title is too long")),
        ));
    }

    if body.chars().count() > 10_000 {
        return Err((
            StatusCode::BAD_REQUEST,
            Html(String::from("Thread body is too long")),
        ));
    }

    let key = client_key(&headers);

    if !state.rate_limiter.allow(&key, 2, Duration::from_secs(60)) {
        return Err((
            StatusCode::TOO_MANY_REQUESTS,
            Html(String::from(
                "Too many threads. Please wait before posting again.",
            )),
        ));
    }

    let (token, is_new) = anonymous_token(&headers);
    let Some(thread_id) = forum::create_thread(&state.pool, &slug, title, body, &token)
        .await
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Html(String::from("Database error")),
            )
        })?
    else {
        return Err(not_found_response());
    };

    let mut response_headers = HeaderMap::new();

    if is_new {
        response_headers.insert(
            SET_COOKIE,
            HeaderValue::from_str(&format!(
                "{ANONYMOUS_COOKIE}={token}; Path=/; HttpOnly; SameSite=Lax"
            ))
            .unwrap(),
        );
    }

    Ok((
        response_headers,
        Redirect::to(&format!("/threads/{thread_id}")),
    ))
}

async fn create_reply(
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Form(form): Form<ReplyForm>,
) -> Result<(HeaderMap, Redirect), (StatusCode, Html<String>)> {
    let Ok(thread_id) = id.parse::<u64>() else {
        return Err(not_found_response());
    };

    let body = form.body.trim();

    if body.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Html(String::from("Reply body cannot be empty")),
        ));
    }

    if body.chars().count() > 10_000 {
        return Err((
            StatusCode::BAD_REQUEST,
            Html(String::from("Reply body is too long.")),
        ));
    }

    let key = client_key(&headers);

    if !state.rate_limiter.allow(&key, 10, Duration::from_secs(60)) {
        return Err((
            StatusCode::TOO_MANY_REQUESTS,
            Html(String::from(
                "Too many replies. Please wait before posting again.",
            )),
        ));
    }

    let (token, is_new) = anonymous_token(&headers);
    let poster_id = thread_poster_id(&token, thread_id);

    let created = forum::create_reply(&state.pool, thread_id, body, &poster_id)
        .await
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Html(String::from("Database error")),
            )
        })?;

    if !created {
        return Err(not_found_response());
    }

    let mut response_headers = HeaderMap::new();

    if is_new {
        response_headers.insert(
            SET_COOKIE,
            HeaderValue::from_str(&format!(
                "{ANONYMOUS_COOKIE}={token}; Path=/; HttpOnly; SameSite=Lax"
            ))
            .unwrap(),
        );
    }

    Ok((
        response_headers,
        Redirect::to(&format!("/threads/{thread_id}")),
    ))
}

async fn report_thread(
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Form(form): Form<ReportForm>,
) -> Result<Redirect, (StatusCode, Html<String>)> {
    let Ok(thread_id) = id.parse::<u64>() else {
        return Err(not_found_response());
    };

    let reason = form.reason.trim();
    let valid_reason = matches!(
        reason,
        "spam" | "harassment" | "doxxing" | "threats" | "illegal" | "other"
    );

    if !valid_reason {
        return Err((
            StatusCode::BAD_REQUEST,
            Html(String::from("Invalid report reason")),
        ));
    }

    let key = client_key(&headers);

    if !state.rate_limiter.allow(&key, 5, Duration::from_secs(60)) {
        return Err((
            StatusCode::TOO_MANY_REQUESTS,
            Html(String::from(
                "Too many reports. Please wait before reporting again.",
            )),
        ));
    }

    let reported = forum::report_thread(&state.pool, thread_id, reason)
        .await
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Html(String::from("Database error")),
            )
        })?;

    if !reported {
        return Err(not_found_response());
    }

    Ok(Redirect::to(&format!("/threads/{thread_id}")))
}

async fn report_reply(
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Form(form): Form<ReportForm>,
) -> Result<Redirect, (StatusCode, Html<String>)> {
    let Ok(reply_id) = id.parse::<u64>() else {
        return Err(not_found_response());
    };

    let reason = form.reason.trim();

    let valid_reason = matches!(
        reason,
        "spam" | "harassment" | "doxxing" | "threats" | "illegal" | "other"
    );
    if !valid_reason {
        return Err((
            StatusCode::BAD_REQUEST,
            Html(String::from("Invalid report reason")),
        ));
    }

    let key = client_key(&headers);

    if !state.rate_limiter.allow(&key, 5, Duration::from_secs(60)) {
        return Err((
            StatusCode::TOO_MANY_REQUESTS,
            Html(String::from(
                "Too many reports. Please wait before reporting again.",
            )),
        ));
    }

    let Some(thread_id) = forum::report_reply(&state.pool, reply_id, reason)
        .await
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Html(String::from("Database error")),
            )
        })?
    else {
        return Err(not_found_response());
    };

    Ok(Redirect::to(&format!("/threads/{thread_id}")))
}

async fn thread(
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> Result<Html<String>, (StatusCode, Html<String>)> {
    let Ok(id) = id.parse::<u64>() else {
        return Err(not_found_response());
    };

    let found = forum::load_thread(&state.pool, id).await.map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Html(String::from("Database error")),
        )
    })?;

    let Some((board, thread)) = found else {
        return Err(not_found_response());
    };

    let template = ThreadTemplate {
        board: &board,
        thread: &thread,
    };

    Ok(Html(template.render().unwrap()))
}
