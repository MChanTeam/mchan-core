use super::*;
use axum::extract::Json;
use axum::http::header::AUTHORIZATION;
use axum::response::IntoResponse;
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub(super) struct ModerateRequest {
    report_id: u64,
    action: String,
    days: Option<u32>,
    moderator: String,
}

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
}

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
    let body = Json(HealthResponse {
        status: if healthy { "ok" } else { "unhealthy" },
    });
    (status, no_store_headers(), body)
}

fn constant_time_equal(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    let mut difference = 0u8;
    for (left, right) in left.iter().zip(right) {
        difference |= left ^ right;
    }
    difference == 0
}

fn bearer_matches(headers: &HeaderMap, expected: &str) -> bool {
    let Some(value) = headers
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
    else {
        return false;
    };
    let Some(token) = value.strip_prefix("Bearer ") else {
        return false;
    };
    constant_time_equal(token.as_bytes(), expected.as_bytes())
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

pub(super) async fn moderate(
    State(state): State<Arc<HttpDependencies>>,
    headers: HeaderMap,
    Json(request): Json<ModerateRequest>,
) -> Response {
    let Some(expected_token) = state.discord_moderation_token.as_deref() else {
        return error_response(StatusCode::NOT_FOUND, "endpoint disabled");
    };
    if !bearer_matches(&headers, expected_token) {
        return error_response(StatusCode::UNAUTHORIZED, "unauthorized");
    }

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
