use crate::{abuse, captcha, forum, media, miya};
use askama::{Result, Template};
use axum::extract::multipart::MultipartError;
use axum::{
    Router,
    extract::{DefaultBodyLimit, Form, FromRequest, Multipart, Path, Query, Request, State},
    http::{
        HeaderMap, HeaderValue, StatusCode,
        header::{CACHE_CONTROL, CONTENT_TYPE, PRAGMA},
    },
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post},
};
use pulldown_cmark::{Options, Parser, html};
use sqlx::SqlitePool;
use std::{
    collections::{HashMap, HashSet, VecDeque},
    fmt::Write as FmtWrite,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tower_http::services::ServeDir;
mod moderation;
mod operations;
mod posting;
mod public;

#[derive(Template)]
#[template(path = "home.html")]
struct HomeTemplate<'a> {
    site_name: &'a str,
    version: &'static str,
    boards: &'a [forum::Board],
    is_moderator: bool,
}

#[derive(Template)]
#[template(path = "admin.html")]
struct AdminTemplate;
#[derive(Template)]
#[template(path = "404.html")]
struct NotFoundTemplate;

#[derive(Template)]
#[template(path = "policy.html")]
struct PolicyTemplate<'a> {
    title: &'a str,
    content_html: &'a str,
}

const PRIVACY_MARKDOWN: &str = include_str!("../../docs/PRIVACY.md");
const RULES_MARKDOWN: &str = include_str!("../../docs/RULES.md");
const CHANGELOG_MARKDOWN: &str = include_str!("../../docs/CHANGELOG.md");

fn render_trusted_markdown(markdown: &str) -> String {
    let options = Options::ENABLE_TABLES;
    let parser = Parser::new_ext(markdown, options);
    let mut html = String::new();
    html::push_html(&mut html, parser);
    html
}

fn format_malaysia_post_time(created_at: &str) -> Result<String> {
    let utc_datetime = time::PrimitiveDateTime::parse(
        created_at,
        &time::macros::format_description!("[year]-[month]-[day] [hour]:[minute]:[second]"),
    )
    .map_err(askama::Error::custom)?
    .assume_utc();
    let malaysia_datetime =
        utc_datetime.to_offset(time::UtcOffset::from_hms(8, 0, 0).map_err(askama::Error::custom)?);

    malaysia_datetime
        .format(&time::macros::format_description!(
            "[day]/[month]/[year repr:last_two] [hour]:[minute]"
        ))
        .map_err(askama::Error::custom)
}

mod filters {
    use super::format_malaysia_post_time;
    use askama::{Result, Values};

    #[askama::filter_fn]
    pub fn malaysia_post_time(created_at: &str, _: &dyn Values) -> Result<String> {
        format_malaysia_post_time(created_at)
    }
}

#[derive(Template)]
#[template(path = "board.html")]
struct BoardTemplate<'a> {
    board: &'a forum::Board,
    current_page: i64,
    has_previous: bool,
    has_next: bool,
    is_archived: bool,
}
#[derive(Template)]
#[template(path = "archive.html")]
struct ArchiveTemplate<'a> {
    board: &'a forum::Board,
    current_page: i64,
    has_previous: bool,
    has_next: bool,
}

#[derive(Template)]
#[template(path = "new_thread.html")]
struct NewThreadTemplate<'a> {
    board: &'a forum::Board,
    captcha_required: bool,
    captcha_site_key: String,
    title_value: String,
    body_value: String,
}

#[derive(serde::Deserialize)]
struct UrlEncodedThreadForm {
    title: Option<String>,
    body: Option<String>,
    #[serde(rename = "cf-turnstile-response", default)]
    captcha_token: Option<String>,
}

#[derive(serde::Deserialize)]
struct UrlEncodedReplyForm {
    body: Option<String>,
    #[serde(rename = "cf-turnstile-response", default)]
    captcha_token: Option<String>,
}

pub(super) struct NewThreadForm {
    pub(super) title: String,
    pub(super) body: String,
    pub(super) captcha_token: Option<String>,
    pub(super) file: Option<media::MediaUpload>,
}

pub(super) struct ReplyForm {
    pub(super) body: String,
    pub(super) captcha_token: Option<String>,
    pub(super) file: Option<media::MediaUpload>,
}

pub(super) enum FormParseError {
    BadRequest,
    PayloadTooLarge,
}

fn map_multipart_error(error: MultipartError) -> FormParseError {
    if error.status() == StatusCode::PAYLOAD_TOO_LARGE {
        FormParseError::PayloadTooLarge
    } else {
        FormParseError::BadRequest
    }
}

impl IntoResponse for FormParseError {
    fn into_response(self) -> Response {
        match self {
            Self::BadRequest => (
                StatusCode::BAD_REQUEST,
                Html(String::from("Malformed form")),
            )
                .into_response(),
            Self::PayloadTooLarge => (
                StatusCode::PAYLOAD_TOO_LARGE,
                Html(String::from("Uploaded file is too large")),
            )
                .into_response(),
        }
    }
}

struct MultipartSubmission {
    title: String,
    body: String,
    captcha_token: Option<String>,
    file: Option<media::MediaUpload>,
}

async fn parse_multipart<S>(
    request: Request,
    state: &S,
    allow_title: bool,
) -> Result<MultipartSubmission, FormParseError>
where
    S: Send + Sync,
{
    let mut multipart = Multipart::from_request(request, state)
        .await
        .map_err(|_| FormParseError::BadRequest)?;
    let mut title = None;
    let mut body = None;
    let mut captcha_token = None;
    let mut file = None;
    let mut file_seen = false;

    while let Some(mut field) = multipart.next_field().await.map_err(map_multipart_error)? {
        let name = field.name().ok_or(FormParseError::BadRequest)?.to_owned();
        match name.as_str() {
            "title" if allow_title => {
                if title.is_some() {
                    return Err(FormParseError::BadRequest);
                }
                title = Some(field.text().await.map_err(map_multipart_error)?);
            }
            "body" => {
                if body.is_some() {
                    return Err(FormParseError::BadRequest);
                }
                body = Some(field.text().await.map_err(map_multipart_error)?);
            }
            "cf-turnstile-response" => {
                if captcha_token.is_some() {
                    return Err(FormParseError::BadRequest);
                }
                captcha_token = Some(field.text().await.map_err(map_multipart_error)?);
            }
            "file" => {
                if file_seen {
                    return Err(FormParseError::BadRequest);
                }
                file_seen = true;
                let filename = field.file_name().map(str::to_owned);
                let content_type = field.content_type().map(str::to_owned);
                let mut bytes = Vec::new();
                while let Some(chunk) = field.chunk().await.map_err(map_multipart_error)? {
                    if bytes.len().saturating_add(chunk.len()) > media::MAX_UPLOAD_BYTES {
                        return Err(FormParseError::PayloadTooLarge);
                    }
                    bytes.extend_from_slice(&chunk);
                }
                if !bytes.is_empty() {
                    file = Some(media::MediaUpload {
                        filename,
                        content_type,
                        bytes,
                    });
                }
            }
            _ => return Err(FormParseError::BadRequest),
        }
    }

    Ok(MultipartSubmission {
        title: title.unwrap_or_default(),
        body: body.unwrap_or_default(),
        captcha_token,
        file,
    })
}

fn is_content_type(headers: &HeaderMap, expected: &str) -> bool {
    headers
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(';').next())
        .is_some_and(|value| value.trim().eq_ignore_ascii_case(expected))
}

impl<S> FromRequest<S> for NewThreadForm
where
    S: Send + Sync,
{
    type Rejection = FormParseError;

    async fn from_request(request: Request, state: &S) -> Result<Self, Self::Rejection> {
        let is_multipart = is_content_type(request.headers(), "multipart/form-data");
        let is_urlencoded = is_content_type(request.headers(), "application/x-www-form-urlencoded");
        if is_multipart {
            let form = parse_multipart(request, state, true).await?;
            Ok(Self {
                title: form.title,
                body: form.body,
                captcha_token: form.captcha_token,
                file: form.file,
            })
        } else if is_urlencoded {
            let Form(form) = Form::<UrlEncodedThreadForm>::from_request(request, state)
                .await
                .map_err(|_| FormParseError::BadRequest)?;
            Ok(Self {
                title: form.title.unwrap_or_default(),
                body: form.body.unwrap_or_default(),
                captcha_token: form.captcha_token,
                file: None,
            })
        } else {
            Err(FormParseError::BadRequest)
        }
    }
}

impl<S> FromRequest<S> for ReplyForm
where
    S: Send + Sync,
{
    type Rejection = FormParseError;

    async fn from_request(request: Request, state: &S) -> Result<Self, Self::Rejection> {
        let is_multipart = is_content_type(request.headers(), "multipart/form-data");
        let is_urlencoded = is_content_type(request.headers(), "application/x-www-form-urlencoded");
        if is_multipart {
            let form = parse_multipart(request, state, false).await?;
            Ok(Self {
                body: form.body,
                captcha_token: form.captcha_token,
                file: form.file,
            })
        } else if is_urlencoded {
            let Form(form) = Form::<UrlEncodedReplyForm>::from_request(request, state)
                .await
                .map_err(|_| FormParseError::BadRequest)?;
            Ok(Self {
                body: form.body.unwrap_or_default(),
                captcha_token: form.captcha_token,
                file: None,
            })
        } else {
            Err(FormParseError::BadRequest)
        }
    }
}

#[derive(serde::Deserialize)]
struct ReportForm {
    reason: String,
    details: Option<String>,
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
    captcha_required: bool,
    captcha_site_key: String,
    reply_body_value: String,
    is_moderator: bool,
}

#[derive(serde::Deserialize)]
struct BoardForm {
    slug: String,
    name: String,
    description: String,
}

#[derive(serde::Deserialize)]
struct DirectHideForm {
    reason: String,
    note: Option<String>,
    return_to: Option<String>,
}

#[derive(Template)]
#[template(path = "admin_boards.html")]
struct AdminBoardsTemplate<'a> {
    boards: &'a [forum::ManagedBoard],
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

    fn suspicious(&self, key: &str, threshold: usize, window: Duration) -> bool {
        let cutoff = Instant::now() - window;
        let mut requests = self.requests.lock().expect("Rate limiter mutex poisoned");
        let timestamps = requests.entry(key.to_owned()).or_default();

        while timestamps.front().is_some_and(|time| *time <= cutoff) {
            timestamps.pop_front();
        }

        timestamps.len() >= threshold
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

pub(crate) struct HttpDependencies {
    pool: SqlitePool,
    rate_limiter: RateLimiter,
    moderator_emails: HashSet<String>,
    abuse_cipher: abuse::AbuseCipher,
    captcha: Option<Arc<dyn captcha::CaptchaVerifier>>,
    pub(super) media_processor: Option<Arc<dyn media::MediaProcessor>>,
    pub(super) media_storage_root: PathBuf,
    pub(super) miya: Option<Arc<miya::Miya>>,
    discord_moderation_token: Option<String>,
    ops_token: Option<String>,
}

impl HttpDependencies {
    pub(crate) fn new(
        pool: SqlitePool,
        moderator_emails: HashSet<String>,
        abuse_cipher: abuse::AbuseCipher,
        captcha: Option<Arc<dyn captcha::CaptchaVerifier>>,
        media_processor: Option<Arc<dyn media::MediaProcessor>>,
        media_storage_root: PathBuf,
        miya: Option<Arc<miya::Miya>>,
        discord_moderation_token: Option<String>,
        ops_token: Option<String>,
    ) -> Self {
        Self {
            pool,
            rate_limiter: RateLimiter::new(),
            moderator_emails,
            abuse_cipher,
            captcha,
            media_processor,
            media_storage_root,
            miya,
            discord_moderation_token,
            ops_token,
        }
    }
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
fn namespaced_rate_key(action: &str, key: &str) -> String {
    format!("{action}:{key}")
}

fn captcha_context(
    state: &HttpDependencies,
    headers: &HeaderMap,
    action: &str,
    threshold: usize,
) -> (bool, String) {
    let captcha_required = state.captcha.is_some() && {
        let client = client_key(headers);
        let fingerprint = state.abuse_cipher.fingerprint(&client);
        let key = namespaced_rate_key(action, &fingerprint_key(&fingerprint));
        state
            .rate_limiter
            .suspicious(&key, threshold, Duration::from_secs(60))
    };
    let captcha_site_key = if captcha_required {
        state
            .captcha
            .as_ref()
            .map(|captcha| captcha.site_key().to_owned())
            .unwrap_or_default()
    } else {
        String::new()
    };
    (captcha_required, captcha_site_key)
}

pub(crate) fn router(dependencies: HttpDependencies) -> Router {
    let media_root = dependencies.media_storage_root.join("images");
    let state = Arc::new(dependencies);

    Router::new()
        .route("/health", get(operations::health))
        .route("/internal/metrics", get(operations::metrics))
        .route("/admin", get(moderation::admin))
        .route(
            "/admin/boards",
            get(moderation::admin_boards).post(moderation::create_board),
        )
        .route(
            "/admin/boards/{slug}/archive",
            post(moderation::archive_board),
        )
        .route(
            "/admin/boards/{slug}/restore",
            post(moderation::restore_board),
        )
        .route(
            "/mod/threads/{id}/hide",
            post(moderation::direct_hide_thread),
        )
        .route(
            "/mod/replies/{id}/hide",
            post(moderation::direct_hide_reply),
        )
        .route("/internal/discord/moderate", post(operations::moderate))
        .route("/", get(public::home))
        .route("/privacy", get(public::privacy))
        .route("/rules", get(public::rules))
        .route("/changelog", get(public::changelog))
        .route("/boards/{slug}", get(public::board))
        .route("/boards/{slug}/archive", get(public::archive))
        .route("/boards/{slug}/new", get(posting::new_thread))
        .route("/boards/{slug}/threads", post(posting::create_thread))
        .route("/threads/{id}", get(public::thread))
        .route("/threads/{id}/replies", post(posting::create_reply))
        .route("/threads/{id}/report", post(posting::report_thread))
        .route("/replies/{id}/report", post(posting::report_reply))
        .nest_service("/static", ServeDir::new("static"))
        .nest_service("/images", ServeDir::new(media_root))
        .route("/mod/reports", get(moderation::moderation_reports))
        .route(
            "/mod/reports/{id}/{action}",
            post(moderation::moderate_report),
        )
        .route("/mod/abuse-logs", get(moderation::abuse_logs))
        .fallback(public::not_found)
        .layer(DefaultBodyLimit::max(media::MAX_UPLOAD_BYTES + 1024 * 1024))
        .with_state(state)
}

#[cfg(test)]
mod malaysia_post_time_tests {
    use super::format_malaysia_post_time;

    #[test]
    fn formats_same_day_in_malaysia_time() {
        assert_eq!(
            format_malaysia_post_time("2026-08-14 13:35:00").expect("timestamp should parse"),
            "14/08/26 21:35"
        );
    }

    #[test]
    fn formats_next_day_rollover_in_malaysia_time() {
        assert_eq!(
            format_malaysia_post_time("2024-01-02 16:30:00").expect("timestamp should parse"),
            "03/01/24 00:30"
        );
    }
}

#[cfg(test)]
mod tests;
