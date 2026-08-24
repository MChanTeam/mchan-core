use super::*;
use axum::extract::Json;
use axum::middleware::Next;
use axum::response::IntoResponse;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;
use std::time::Instant;

#[derive(Deserialize)]
pub(super) struct ModerateRequest {
    report_id: u64,
    action: String,
    days: Option<u32>,
    moderator: String,
}

pub(super) const DISCORD_MODERATION_BODY_LIMIT: usize = 64 * 1024;

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
    service: &'static str,
    version: &'static str,
    uptime_seconds: u64,
    database: &'static str,
}

#[derive(Serialize)]
struct MetricsResponse {
    status: &'static str,
    service: &'static str,
    version: &'static str,
    uptime_seconds: u64,
    database: MetricsDatabase,
    content: MetricsContent,
    moderation: MetricsModeration,
    integrations: MetricsIntegrations,
}

#[derive(Serialize)]
struct MetricsDatabase {
    status: &'static str,
}

#[derive(Serialize)]
struct MetricsContent {
    boards: i64,
    threads: i64,
    replies: i64,
}

#[derive(Serialize)]
struct MetricsModeration {
    pending_reports: i64,
    active_board_bans: i64,
    active_site_bans: i64,
}

#[derive(Serialize)]
struct MetricsIntegrations {
    miya_configured: bool,
    image_processor_configured: bool,
}
static PROCESS_START: OnceLock<Instant> = OnceLock::new();

#[derive(Serialize)]
struct ModerateSuccess {
    status: &'static str,
    report_id: u64,
    action: String,
}

#[derive(Serialize)]
struct ModerateError {
    status: &'static str,
    error: &'static str,
}

fn no_store_headers() -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(CACHE_CONTROL, HeaderValue::from_static("no-store, private"));
    headers.insert(PRAGMA, HeaderValue::from_static("no-cache"));
    headers
}

pub(super) async fn health(State(state): State<Arc<HttpDependencies>>) -> impl IntoResponse {
    let healthy = sqlx::query("SELECT 1").execute(&state.pool).await.is_ok();
    let status = if healthy {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    let uptime_seconds = PROCESS_START.get_or_init(Instant::now).elapsed().as_secs();
    let body = Json(HealthResponse {
        status: if healthy { "ok" } else { "unhealthy" },
        service: "mchan",
        version: env!("CARGO_PKG_VERSION"),
        uptime_seconds,
        database: if healthy { "ok" } else { "unhealthy" },
    });
    (status, no_store_headers(), body)
}

pub(super) async fn metrics(
    State(state): State<Arc<HttpDependencies>>,
    headers: HeaderMap,
) -> Response {
    let Some(expected_token) = state.ops_token.as_deref() else {
        return error_response(StatusCode::NOT_FOUND, "endpoint disabled");
    };
    if !bearer_matches(&headers, expected_token) {
        return error_response(StatusCode::UNAUTHORIZED, "unauthorized");
    }

    let metrics = match forum::load_operational_metrics(&state.pool).await {
        Ok(metrics) => metrics,
        Err(_) => return error_response(StatusCode::INTERNAL_SERVER_ERROR, "database error"),
    };
    (
        StatusCode::OK,
        no_store_headers(),
        Json(MetricsResponse {
            status: "ok",
            service: "mchan",
            version: env!("CARGO_PKG_VERSION"),
            uptime_seconds: PROCESS_START.get_or_init(Instant::now).elapsed().as_secs(),
            database: MetricsDatabase { status: "ok" },
            content: MetricsContent {
                boards: metrics.boards,
                threads: metrics.threads,
                replies: metrics.replies,
            },
            moderation: MetricsModeration {
                pending_reports: metrics.pending_reports,
                active_board_bans: metrics.active_board_bans,
                active_site_bans: metrics.active_site_bans,
            },
            integrations: MetricsIntegrations {
                miya_configured: state.miya.is_some(),
                image_processor_configured: state.media_processor.is_some(),
            },
        }),
    )
        .into_response()
}

fn error_response(status: StatusCode, error: &'static str) -> Response {
    (
        status,
        no_store_headers(),
        Json(ModerateError {
            status: "error",
            error,
        }),
    )
        .into_response()
}

pub(super) async fn discord_auth(
    State(state): State<Arc<HttpDependencies>>,
    headers: HeaderMap,
    request: Request,
    next: Next,
) -> Response {
    let Some(expected_token) = state.discord_moderation_token.as_deref() else {
        return error_response(StatusCode::NOT_FOUND, "endpoint disabled");
    };
    if !bearer_matches(&headers, expected_token) {
        return error_response(StatusCode::UNAUTHORIZED, "unauthorized");
    }

    let mut response = next.run(request).await;
    response
        .headers_mut()
        .insert(CACHE_CONTROL, HeaderValue::from_static("no-store, private"));
    response
        .headers_mut()
        .insert(PRAGMA, HeaderValue::from_static("no-cache"));
    response
}

pub(super) async fn moderate(
    State(state): State<Arc<HttpDependencies>>,
    Json(request): Json<ModerateRequest>,
) -> Response {
    let moderator = request.moderator.trim();
    if moderator.is_empty() || moderator.len() > 120 || !moderator.starts_with("discord:") {
        return error_response(StatusCode::UNPROCESSABLE_ENTITY, "invalid moderator");
    }

    let action = request.action.as_str();
    let result = if let Some(moderation_action) = forum::ModerationAction::parse(action) {
        if request.days.is_some() {
            return error_response(
                StatusCode::UNPROCESSABLE_ENTITY,
                "days is only valid for ban actions",
            );
        }
        forum::apply_moderation_action(&state.pool, request.report_id, moderator, moderation_action)
            .await
    } else {
        let scope = match action {
            "ban-board" => forum::BanScope::Board,
            "ban-site" => forum::BanScope::Site,
            _ => return error_response(StatusCode::UNPROCESSABLE_ENTITY, "invalid action"),
        };
        let Some(days) = request.days else {
            return error_response(StatusCode::UNPROCESSABLE_ENTITY, "invalid days");
        };
        let max_days = match scope {
            forum::BanScope::Board => 30,
            forum::BanScope::Site => 365,
        };
        if !(1..=max_days).contains(&days) {
            return error_response(StatusCode::UNPROCESSABLE_ENTITY, "invalid days");
        }
        forum::apply_ban(&state.pool, request.report_id, moderator, scope, days).await
    };

    let result = match result {
        Ok(result) => result,
        Err(_) => return error_response(StatusCode::INTERNAL_SERVER_ERROR, "database error"),
    };
    match result {
        forum::ModerationResult::Applied => (
            StatusCode::OK,
            no_store_headers(),
            Json(ModerateSuccess {
                status: "applied",
                report_id: request.report_id,
                action: request.action,
            }),
        )
            .into_response(),
        forum::ModerationResult::NotFound => {
            error_response(StatusCode::NOT_FOUND, "report not found")
        }
        forum::ModerationResult::AlreadyHandled => {
            error_response(StatusCode::CONFLICT, "report already handled")
        }
        forum::ModerationResult::InvalidTarget => {
            error_response(StatusCode::UNPROCESSABLE_ENTITY, "invalid target")
        }
        forum::ModerationResult::MissingOrigin => {
            error_response(StatusCode::UNPROCESSABLE_ENTITY, "missing origin")
        }
    }
}
