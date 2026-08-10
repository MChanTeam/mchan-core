mod abuse;
mod forum;

use askama::{Result, Template};
use axum::{
    Router,
    extract::{Form, Path, State},
    http::{
        HeaderMap, HeaderValue, StatusCode,
        header::{CACHE_CONTROL, PRAGMA, SET_COOKIE},
    },
    response::{Html, Redirect},
    routing::{get, post},
};
use sha2::{Digest, Sha256};
use sqlx::{SqlitePool, sqlite::SqliteConnectOptions};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    fmt::Write as FmtWrite,
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

#[derive(serde::Deserialize)]
struct ModerationForm {
    days: Option<u32>,
}

#[derive(Template)]
#[template(path = "thread.html")]
struct ThreadTemplate<'a> {
    board: &'a forum::Board,
    thread: &'a forum::Thread,
}

#[derive(Template)]
#[template(path = "mod_reports.html")]
struct ModerationReportsTemplate<'a> {
    reports: &'a [forum::ModerationReport],
}

struct AbuseLogView {
    target_kind: String,
    target_id: u64,
    client_key: String,
    created_at: String,
    retain_until: String,
}

#[derive(Template)]
#[template(path = "abuse_logs.html")]
struct AbuseLogsTemplate<'a> {
    logs: &'a [AbuseLogView],
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
    moderator_emails: HashSet<String>,
    abuse_cipher: abuse::AbuseCipher,
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

fn require_moderator(
    headers: &HeaderMap,
    allowed_emails: &HashSet<String>,
) -> Result<String, StatusCode> {
    let Some(email) = headers
        .get("cf-access-authenticated-user-email")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|email| !email.is_empty())
    else {
        return Err(StatusCode::FORBIDDEN);
    };

    let normalized_email = email.to_ascii_lowercase();
    if allowed_emails.contains(&normalized_email) {
        Ok(normalized_email)
    } else {
        Err(StatusCode::FORBIDDEN)
    }
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
fn fingerprint_key(fingerprint: &[u8; 32]) -> String {
    let mut key = String::with_capacity(64);
    for byte in fingerprint {
        let _ = write!(key, "{byte:02x}");
    }
    key
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
    let moderator_emails = std::env::var("MCHAN_MODERATOR_EMAILS")
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|email| !email.is_empty())
        .map(|email| email.to_ascii_lowercase())
        .collect::<HashSet<_>>();

    let abuse_key = std::env::var("MCHAN_ABUSE_KEY").map_err(|_| {
        std::io::Error::other(
            "MCHAN_ABUSE_KEY is required; generate one with `openssl rand -hex 32`",
        )
    })?;
    let abuse_cipher = abuse::AbuseCipher::from_hex(&abuse_key)?;

    let database_url =
        std::env::var("DATABASE_URL").unwrap_or_else(|_| String::from("sqlite://mchan.db"));

    let options = SqliteConnectOptions::from_str(&database_url)?.create_if_missing(true);
    let pool = SqlitePool::connect_with(options).await?;
    sqlx::migrate!().run(&pool).await?;
    forum::purge_expired_abuse_logs(&pool).await?;

    let cleanup_pool = pool.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(60 * 60));
        interval.tick().await;
        loop {
            interval.tick().await;
            if let Err(error) = forum::purge_expired_abuse_logs(&cleanup_pool).await {
                eprintln!("Could not purge expired abuse logs: {error}");
            }
        }
    });

    let state = Arc::new(AppState {
        pool,
        rate_limiter: RateLimiter::new(),
        moderator_emails,
        abuse_cipher,
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
        .route("/mod/reports", get(moderation_reports))
        .route("/mod/reports/{id}/{action}", post(moderate_report))
        .route("/mod/abuse-logs", get(abuse_logs))
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
    let fingerprint = state.abuse_cipher.fingerprint(&key);
    if let Some(ban) = forum::load_active_ban_for_board(&state.pool, &fingerprint, &slug)
        .await
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Html(String::from("Database error")),
            )
        })?
    {
        return Err((
            StatusCode::FORBIDDEN,
            Html(format!(
                "Posting is blocked by an active {} ban until {}.",
                ban.scope, ban.expires_at
            )),
        ));
    }

    let rate_key = fingerprint_key(&fingerprint);
    if !state
        .rate_limiter
        .allow(&rate_key, 2, Duration::from_secs(60))
    {
        return Err((
            StatusCode::TOO_MANY_REQUESTS,
            Html(String::from(
                "Too many threads. Please wait before posting again.",
            )),
        ));
    }

    let origin = state.abuse_cipher.protect(&key).map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Html(String::from("Could not protect operational data")),
        )
    })?;
    let (token, is_new) = anonymous_token(&headers);
    let Some(thread_id) = forum::create_thread(&state.pool, &slug, title, body, &token, &origin)
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
    let fingerprint = state.abuse_cipher.fingerprint(&key);
    if let Some(ban) = forum::load_active_ban_for_thread(&state.pool, &fingerprint, thread_id)
        .await
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Html(String::from("Database error")),
            )
        })?
    {
        return Err((
            StatusCode::FORBIDDEN,
            Html(format!(
                "Posting is blocked by an active {} ban until {}.",
                ban.scope, ban.expires_at
            )),
        ));
    }

    let rate_key = fingerprint_key(&fingerprint);
    if !state
        .rate_limiter
        .allow(&rate_key, 10, Duration::from_secs(60))
    {
        return Err((
            StatusCode::TOO_MANY_REQUESTS,
            Html(String::from(
                "Too many replies. Please wait before posting again.",
            )),
        ));
    }

    let origin = state.abuse_cipher.protect(&key).map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Html(String::from("Could not protect operational data")),
        )
    })?;
    let (token, is_new) = anonymous_token(&headers);
    let poster_id = thread_poster_id(&token, thread_id);

    match forum::create_reply(&state.pool, thread_id, body, &poster_id, &origin)
        .await
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Html(String::from("Database error")),
            )
        })? {
        forum::CreateReplyResult::Created => {}
        forum::CreateReplyResult::NotFound => return Err(not_found_response()),
        forum::CreateReplyResult::Locked => {
            return Err((
                StatusCode::CONFLICT,
                Html(String::from("This thread is locked")),
            ));
        }
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

async fn moderate_report(
    Path((id, action)): Path<(String, String)>,
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Form(form): Form<ModerationForm>,
) -> Result<Redirect, (StatusCode, Html<String>)> {
    let moderator_email = require_moderator(&headers, &state.moderator_emails)
        .map_err(|status| (status, Html(String::from("Moderator access required"))))?;

    let report_id = id.parse::<u64>().map_err(|_| {
        (
            StatusCode::UNPROCESSABLE_ENTITY,
            Html(String::from("Invalid report ID")),
        )
    })?;

    if let Some(moderation_action) = forum::ModerationAction::parse(&action) {
        let result = forum::apply_moderation_action(
            &state.pool,
            report_id,
            &moderator_email,
            moderation_action,
        )
        .await
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Html(String::from("Database error")),
            )
        })?;

        return moderation_result_response(result);
    }

    let scope = match action.as_str() {
        "ban-board" => forum::BanScope::Board,
        "ban-site" => forum::BanScope::Site,
        _ => {
            return Err((
                StatusCode::UNPROCESSABLE_ENTITY,
                Html(String::from("Invalid moderation action")),
            ));
        }
    };

    let max_days = match scope {
        forum::BanScope::Board => 30,
        forum::BanScope::Site => 365,
    };
    let days = form
        .days
        .filter(|days| (1..=max_days).contains(days))
        .ok_or_else(|| {
            (
                StatusCode::UNPROCESSABLE_ENTITY,
                Html(format!(
                    "Ban duration must be between 1 and {max_days} days"
                )),
            )
        })?;

    let result = forum::apply_ban(&state.pool, report_id, &moderator_email, scope, days)
        .await
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Html(String::from("Database error")),
            )
        })?;

    moderation_result_response(result)
}

fn moderation_result_response(
    result: forum::ModerationResult,
) -> Result<Redirect, (StatusCode, Html<String>)> {
    match result {
        forum::ModerationResult::Applied => Ok(Redirect::to("/mod/reports")),
        forum::ModerationResult::NotFound => Err(not_found_response()),
        forum::ModerationResult::AlreadyHandled => Err((
            StatusCode::CONFLICT,
            Html(String::from("Report has already been handled")),
        )),
        forum::ModerationResult::InvalidTarget => Err((
            StatusCode::UNPROCESSABLE_ENTITY,
            Html(String::from(
                "Moderation action is not valid for this report",
            )),
        )),
        forum::ModerationResult::MissingOrigin => Err((
            StatusCode::UNPROCESSABLE_ENTITY,
            Html(String::from("The reported post has no protected origin")),
        )),
    }
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

async fn moderation_reports(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Html<String>, (StatusCode, Html<String>)> {
    require_moderator(&headers, &state.moderator_emails)
        .map_err(|status| (status, Html(String::from("Moderator access required"))))?;

    let reports = forum::load_pending_reports(&state.pool)
        .await
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Html(String::from("Database error")),
            )
        })?;

    let page = ModerationReportsTemplate { reports: &reports };

    Ok(Html(page.render().unwrap()))
}

async fn abuse_logs(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<(HeaderMap, Html<String>), (StatusCode, Html<String>)> {
    let moderator_email = require_moderator(&headers, &state.moderator_emails)
        .map_err(|status| (status, Html(String::from("Moderator access required"))))?;

    forum::record_abuse_log_access(&state.pool, &moderator_email)
        .await
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Html(String::from("Database error")),
            )
        })?;

    let encrypted_logs = forum::load_abuse_logs(&state.pool).await.map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Html(String::from("Database error")),
        )
    })?;

    let mut logs = Vec::with_capacity(encrypted_logs.len());
    for log in encrypted_logs {
        let client_key = state
            .abuse_cipher
            .decrypt(&log.nonce, &log.ciphertext)
            .map_err(|_| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Html(String::from("Could not decrypt abuse log")),
                )
            })?;
        logs.push(AbuseLogView {
            target_kind: log.target_kind,
            target_id: log.target_id,
            client_key,
            created_at: log.created_at,
            retain_until: log.retain_until,
        });
    }

    let page = AbuseLogsTemplate { logs: &logs };
    let mut response_headers = HeaderMap::new();
    response_headers.insert(CACHE_CONTROL, HeaderValue::from_static("no-store, private"));
    response_headers.insert(PRAGMA, HeaderValue::from_static("no-cache"));
    Ok((response_headers, Html(page.render().unwrap())))
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
