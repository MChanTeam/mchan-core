use super::public::not_found_response;
use super::*;

fn board_form_error(message: &str) -> (StatusCode, Html<String>) {
    (StatusCode::BAD_REQUEST, Html(message.to_owned()))
}

pub(super) async fn admin(
    State(state): State<Arc<HttpDependencies>>,
    headers: HeaderMap,
) -> Result<Html<String>, (StatusCode, Html<String>)> {
    require_admin(&headers, &state)
        .map_err(|status| (status, Html(String::from("Admin access required"))))?;
    Ok(Html(AdminTemplate.render().unwrap()))
}

pub(super) async fn admin_boards(
    State(state): State<Arc<HttpDependencies>>,
    headers: HeaderMap,
) -> Result<Html<String>, (StatusCode, Html<String>)> {
    require_admin(&headers, &state)
        .map_err(|status| (status, Html(String::from("Admin access required"))))?;
    let boards = forum::load_managed_boards(&state.pool).await.map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Html(String::from("Database error")),
        )
    })?;
    let mut board_views = Vec::with_capacity(boards.len());
    for board in boards {
        let assigned_emails = forum::load_board_moderators(&state.pool, &board.slug)
            .await
            .map_err(|_| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Html(String::from("Database error")),
                )
            })?;
        board_views.push(ManagedBoardView {
            slug: board.slug,
            name: board.name,
            description: board.description,
            status: board.status,
            assigned_emails,
        });
    }
    Ok(Html(
        AdminBoardsTemplate {
            boards: &board_views,
        }
        .render()
        .unwrap(),
    ))
}

pub(super) async fn create_board(
    State(state): State<Arc<HttpDependencies>>,
    headers: HeaderMap,
    Form(form): Form<BoardForm>,
) -> Result<Redirect, (StatusCode, Html<String>)> {
    require_admin(&headers, &state)
        .map_err(|status| (status, Html(String::from("Admin access required"))))?;
    let slug = form.slug.trim();
    let name = form.name.trim();
    let description = form.description.trim();
    if slug.is_empty()
        || slug.len() > 64
        || !slug
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
    {
        return Err(board_form_error("Invalid board slug"));
    }
    if name.is_empty() || name.chars().count() > 120 {
        return Err(board_form_error("Invalid board name"));
    }
    if description.chars().count() > 400 {
        return Err(board_form_error("Board description is too long"));
    }
    match forum::create_board(&state.pool, slug, name, description)
        .await
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Html(String::from("Database error")),
            )
        })? {
        forum::BoardManagementResult::Created => Ok(Redirect::to("/admin/boards")),
        forum::BoardManagementResult::DuplicateSlug => Err((
            StatusCode::CONFLICT,
            Html(String::from("A board with that slug already exists")),
        )),
        _ => Err(board_form_error("Could not create board")),
    }
}

pub(super) async fn add_board_moderator(
    Path(slug): Path<String>,
    State(state): State<Arc<HttpDependencies>>,
    headers: HeaderMap,
    Form(form): Form<ModeratorForm>,
) -> Result<Redirect, (StatusCode, Html<String>)> {
    require_admin(&headers, &state)
        .map_err(|status| (status, Html(String::from("Admin access required"))))?;
    let email = form.email.trim();
    if email.is_empty() || email.chars().count() > 254 {
        return Err(board_form_error("Invalid moderator email"));
    }
    if !board_exists(&state, &slug).await? {
        return Err(not_found_response());
    }
    let added = forum::add_board_moderator(&state.pool, &slug, email)
        .await
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Html(String::from("Database error")),
            )
        })?;
    if !added {
        return Err((
            StatusCode::CONFLICT,
            Html(String::from("That moderator is already assigned")),
        ));
    }
    Ok(Redirect::to("/admin/boards"))
}

pub(super) async fn remove_board_moderator(
    Path(slug): Path<String>,
    State(state): State<Arc<HttpDependencies>>,
    headers: HeaderMap,
    Form(form): Form<ModeratorForm>,
) -> Result<Redirect, (StatusCode, Html<String>)> {
    require_admin(&headers, &state)
        .map_err(|status| (status, Html(String::from("Admin access required"))))?;
    let email = form.email.trim();
    if email.is_empty() || email.chars().count() > 254 {
        return Err(board_form_error("Invalid moderator email"));
    }
    if !board_exists(&state, &slug).await? {
        return Err(not_found_response());
    }
    let removed = forum::remove_board_moderator(&state.pool, &slug, email)
        .await
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Html(String::from("Database error")),
            )
        })?;
    if !removed {
        return Err(not_found_response());
    }
    Ok(Redirect::to("/admin/boards"))
}

async fn board_exists(
    state: &HttpDependencies,
    slug: &str,
) -> Result<bool, (StatusCode, Html<String>)> {
    Ok(forum::load_managed_boards(&state.pool)
        .await
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Html(String::from("Database error")),
            )
        })?
        .iter()
        .any(|board| board.slug == slug))
}

pub(super) async fn archive_board(
    Path(slug): Path<String>,
    State(state): State<Arc<HttpDependencies>>,
    headers: HeaderMap,
) -> Result<Redirect, (StatusCode, Html<String>)> {
    board_state_change(&state, &headers, &slug, true).await
}

pub(super) async fn restore_board(
    Path(slug): Path<String>,
    State(state): State<Arc<HttpDependencies>>,
    headers: HeaderMap,
) -> Result<Redirect, (StatusCode, Html<String>)> {
    board_state_change(&state, &headers, &slug, false).await
}

async fn board_state_change(
    state: &HttpDependencies,
    headers: &HeaderMap,
    slug: &str,
    archive: bool,
) -> Result<Redirect, (StatusCode, Html<String>)> {
    require_admin(headers, state)
        .map_err(|status| (status, Html(String::from("Admin access required"))))?;
    let result = if archive {
        forum::archive_board(&state.pool, slug).await
    } else {
        forum::restore_board(&state.pool, slug).await
    }
    .map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Html(String::from("Database error")),
        )
    })?;
    match result {
        forum::BoardManagementResult::Archived | forum::BoardManagementResult::Restored => {
            Ok(Redirect::to("/admin/boards"))
        }
        forum::BoardManagementResult::NotFound => Err(not_found_response()),
        forum::BoardManagementResult::InvalidTransition => Err((
            StatusCode::CONFLICT,
            Html(String::from("Invalid board state transition")),
        )),
        _ => Err((
            StatusCode::CONFLICT,
            Html(String::from("Invalid board state transition")),
        )),
    }
}

pub(super) async fn direct_hide_thread(
    Path(id): Path<String>,
    State(state): State<Arc<HttpDependencies>>,
    headers: HeaderMap,
    Form(form): Form<DirectHideForm>,
) -> Result<Redirect, (StatusCode, Html<String>)> {
    direct_hide(&state, &headers, "thread", id, form).await
}

pub(super) async fn direct_hide_reply(
    Path(id): Path<String>,
    State(state): State<Arc<HttpDependencies>>,
    headers: HeaderMap,
    Form(form): Form<DirectHideForm>,
) -> Result<Redirect, (StatusCode, Html<String>)> {
    direct_hide(&state, &headers, "reply", id, form).await
}

async fn direct_hide(
    state: &HttpDependencies,
    headers: &HeaderMap,
    kind: &str,
    id: String,
    form: DirectHideForm,
) -> Result<Redirect, (StatusCode, Html<String>)> {
    let target_id = id.parse::<u64>().map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            Html(String::from("Invalid target ID")),
        )
    })?;
    let board_slug = match kind {
        "thread" => forum::load_thread_board_slug(&state.pool, target_id).await,
        "reply" => forum::load_reply_board_slug(&state.pool, target_id).await,
        _ => unreachable!(),
    }
    .map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Html(String::from("Database error")),
        )
    })?
    .ok_or_else(not_found_response)?;
    let moderator = require_board_moderator(headers, state, &board_slug)
        .await
        .map_err(|status| (status, Html(String::from("Moderator access required"))))?;
    let reason = forum::DirectHideReason::parse(form.reason.trim()).ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            Html(String::from("Invalid hide reason")),
        )
    })?;
    let note = form
        .note
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    if note.is_some_and(|s| s.chars().count() > 400) {
        return Err((
            StatusCode::BAD_REQUEST,
            Html(String::from(
                "Hide note cannot be longer than 400 characters.",
            )),
        ));
    }
    match forum::apply_direct_hide(&state.pool, kind, target_id, &moderator, reason, note)
        .await
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Html(String::from("Database error")),
            )
        })? {
        forum::DirectHideResult::Applied => Ok(Redirect::to(&safe_return_to(
            form.return_to.as_deref(),
            target_id,
        ))),
        forum::DirectHideResult::NotFound => Err(not_found_response()),
        forum::DirectHideResult::InvalidTarget => Err(board_form_error("Invalid hide target")),
    }
}

fn safe_return_to(value: Option<&str>, id: u64) -> String {
    let fallback = format!("/threads/{id}");
    let Some(value) = value else { return fallback };
    if value.starts_with("/threads/")
        && value[9..]
            .chars()
            .all(|c| c.is_ascii_digit() || c == '#' || c == '-' || c.is_ascii_alphabetic())
    {
        value.to_owned()
    } else {
        fallback
    }
}

pub(super) async fn pin_thread(
    Path(id): Path<String>,
    State(state): State<Arc<HttpDependencies>>,
    headers: HeaderMap,
) -> Result<Redirect, (StatusCode, Html<String>)> {
    set_thread_pin(&state, &headers, id, true).await
}

pub(super) async fn unpin_thread(
    Path(id): Path<String>,
    State(state): State<Arc<HttpDependencies>>,
    headers: HeaderMap,
) -> Result<Redirect, (StatusCode, Html<String>)> {
    set_thread_pin(&state, &headers, id, false).await
}

async fn set_thread_pin(
    state: &HttpDependencies,
    headers: &HeaderMap,
    id: String,
    pinned: bool,
) -> Result<Redirect, (StatusCode, Html<String>)> {
    let thread_id = id.parse::<u64>().map_err(|_| not_found_response())?;
    let board_slug = forum::load_thread_board_slug(&state.pool, thread_id)
        .await
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Html(String::from("Database error")),
            )
        })?
        .ok_or_else(not_found_response)?;
    require_board_moderator(headers, state, &board_slug)
        .await
        .map_err(|status| (status, Html(String::from("Moderator access required"))))?;
    let result = if pinned {
        forum::pin_thread(&state.pool, thread_id).await
    } else {
        forum::unpin_thread(&state.pool, thread_id).await
    }
    .map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Html(String::from("Database error")),
        )
    })?;
    match result {
        forum::ThreadPinResult::Applied => Ok(Redirect::to(&format!("/threads/{thread_id}"))),
        forum::ThreadPinResult::NotFound => Err(not_found_response()),
    }
}
pub(super) async fn moderate_report(
    Path((id, action)): Path<(String, String)>,
    State(state): State<Arc<HttpDependencies>>,

    headers: HeaderMap,
    Form(form): Form<ModerationForm>,
) -> Result<Redirect, (StatusCode, Html<String>)> {
    let report_id = id.parse::<u64>().map_err(|_| {
        (
            StatusCode::UNPROCESSABLE_ENTITY,
            Html(String::from("Invalid report ID")),
        )
    })?;

    let is_ban = matches!(action.as_str(), "ban-board" | "ban-site");
    let moderator_email = if is_ban {
        require_admin(&headers, &state)
            .map_err(|status| (status, Html(String::from("Admin access required"))))?
    } else {
        let board_slug = forum::load_report_board_slug(&state.pool, report_id)
            .await
            .map_err(|_| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Html(String::from("Database error")),
                )
            })?
            .ok_or_else(not_found_response)?;
        require_board_moderator(&headers, &state, &board_slug)
            .await
            .map_err(|status| (status, Html(String::from("Moderator access required"))))?
    };

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
    let is_admin = require_admin(&headers, &state).is_ok();
    let reports = if is_admin {
        forum::load_pending_reports(&state.pool).await
    } else {
        let email = normalized_cf_email(&headers).ok_or_else(|| {
            (
                StatusCode::FORBIDDEN,
                Html(String::from("Moderator access required")),
            )
        })?;
        let boards = forum::load_managed_boards(&state.pool).await.map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Html(String::from("Database error")),
            )
        })?;
        let mut assigned_slugs = Vec::new();
        for board in boards {
            let moderators = forum::load_board_moderators(&state.pool, &board.slug)
                .await
                .map_err(|_| {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Html(String::from("Database error")),
                    )
                })?;
            if moderators.iter().any(|moderator| moderator == &email) {
                assigned_slugs.push(board.slug);
            }
        }
        if assigned_slugs.is_empty() {
            return Err((
                StatusCode::FORBIDDEN,
                Html(String::from("Moderator access required")),
            ));
        }
        forum::load_pending_reports_for_boards(&state.pool, &assigned_slugs).await
    }
    .map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Html(String::from("Database error")),
        )
    })?;

    let page = ModerationReportsTemplate {
        reports: &reports,
        is_admin,
    };

    Ok(Html(page.render().unwrap()))
}

pub(super) async fn abuse_logs(
    State(state): State<Arc<HttpDependencies>>,
    headers: HeaderMap,
) -> Result<(HeaderMap, Html<String>), (StatusCode, Html<String>)> {
    let moderator_email = require_admin(&headers, &state)
        .map_err(|status| (status, Html(String::from("Admin access required"))))?;

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
