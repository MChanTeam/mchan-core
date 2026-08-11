use super::public::not_found_response;
use super::*;

pub(super) async fn moderate_report(
    Path((id, action)): Path<(String, String)>,
    State(state): State<Arc<HttpDependencies>>,
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

pub(super) async fn moderation_reports(
    State(state): State<Arc<HttpDependencies>>,
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

pub(super) async fn abuse_logs(
    State(state): State<Arc<HttpDependencies>>,
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
