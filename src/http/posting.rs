use super::public::not_found_response;
use super::*;

async fn new_thread_challenge_response(
    state: &HttpDependencies,
    slug: &str,
    title_value: &str,
    body_value: &str,
) -> Response {
    let board = match forum::load_board(&state.pool, slug).await {
        Ok(Some(board)) => board,
        Ok(None) => return not_found_response().into_response(),
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Html(String::from("Database error")),
            )
                .into_response();
        }
    };

    let template = NewThreadTemplate {
        board: &board,
        captcha_required: true,
        captcha_site_key: state
            .captcha
            .as_ref()
            .map(|captcha| captcha.site_key().to_owned())
            .unwrap_or_default(),
        title_value: title_value.to_owned(),
        body_value: body_value.to_owned(),
    };
    (StatusCode::FORBIDDEN, Html(template.render().unwrap())).into_response()
}

async fn thread_challenge_response(
    state: &HttpDependencies,
    thread_id: u64,
    reply_body_value: &str,
) -> Response {
    let found = match forum::load_thread(&state.pool, thread_id).await {
        Ok(Some(found)) => found,
        Ok(None) => return not_found_response().into_response(),
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Html(String::from("Database error")),
            )
                .into_response();
        }
    };
    let (board, thread) = found;
    let template = ThreadTemplate {
        board: &board,
        thread: &thread,
        captcha_required: true,
        captcha_site_key: state
            .captcha
            .as_ref()
            .map(|captcha| captcha.site_key().to_owned())
            .unwrap_or_default(),
        reply_body_value: reply_body_value.to_owned(),
    };
    (StatusCode::FORBIDDEN, Html(template.render().unwrap())).into_response()
}

pub(super) async fn new_thread(
    Path(slug): Path<String>,
    State(state): State<Arc<HttpDependencies>>,
    headers: HeaderMap,
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

    let (captcha_required, captcha_site_key) = captcha_context(&state, &headers, "thread", 1);
    let template = NewThreadTemplate {
        board: &board,
        captcha_required,
        captcha_site_key,
        title_value: String::new(),
        body_value: String::new(),
    };

    Ok(Html(template.render().unwrap()))
}

pub(super) async fn create_thread(
    Path(slug): Path<String>,
    State(state): State<Arc<HttpDependencies>>,
    headers: HeaderMap,
    Form(form): Form<NewThreadForm>,
) -> Result<(HeaderMap, Redirect), Response> {
    let title = form.title.trim();
    let body = form.body.trim();

    if title.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Html(String::from("Thread title cannot be empty.")),
        )
            .into_response());
    }

    if body.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Html(String::from("Thread body cannot be empty")),
        )
            .into_response());
    }

    if title.chars().count() > 120 {
        return Err((
            StatusCode::BAD_REQUEST,
            Html(String::from("Thread title is too long")),
        )
            .into_response());
    }

    if body.chars().count() > 2_000 {
        return Err((
            StatusCode::BAD_REQUEST,
            Html(String::from("Thread body is too long")),
        )
            .into_response());
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
                .into_response()
        })?
    {
        return Err((
            StatusCode::FORBIDDEN,
            Html(format!(
                "Posting is blocked by an active {} ban until {}.",
                ban.scope, ban.expires_at
            )),
        )
            .into_response());
    }

    let rate_key = namespaced_rate_key("thread", &fingerprint_key(&fingerprint));
    if let Some(captcha) = state.captcha.as_ref() {
        if state
            .rate_limiter
            .suspicious(&rate_key, 1, Duration::from_secs(60))
        {
            let token = form
                .captcha_token
                .as_deref()
                .map(str::trim)
                .filter(|token| !token.is_empty());
            let Some(token) = token else {
                return Err(
                    new_thread_challenge_response(&state, &slug, &form.title, &form.body).await,
                );
            };
            match captcha.verify(token, &key).await {
                Ok(true) => {}
                Ok(false) => {
                    return Err(new_thread_challenge_response(
                        &state,
                        &slug,
                        &form.title,
                        &form.body,
                    )
                    .await);
                }
                Err(_) => {
                    return Err((
                        StatusCode::SERVICE_UNAVAILABLE,
                        Html(String::from("CAPTCHA service unavailable")),
                    )
                        .into_response());
                }
            }
        }
    }
    if !state
        .rate_limiter
        .allow(&rate_key, 2, Duration::from_secs(60))
    {
        return Err((
            StatusCode::TOO_MANY_REQUESTS,
            Html(String::from(
                "Too many threads. Please wait before posting again.",
            )),
        )
            .into_response());
    }

    let origin = state.abuse_cipher.protect(&key).map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Html(String::from("Could not protect operational data")),
        )
            .into_response()
    })?;
    let (token, is_new) = anonymous_token(&headers);
    let Some(thread_id) = forum::create_thread(&state.pool, &slug, title, body, &token, &origin)
        .await
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Html(String::from("Database error")),
            )
                .into_response()
        })?
    else {
        return Err(not_found_response().into_response());
    };

    let mut response_headers = HeaderMap::new();

    if should_set_anonymous_cookie(is_new, &headers) {
        response_headers.insert(
            SET_COOKIE,
            HeaderValue::from_str(&anonymous_cookie(&token, &headers)).unwrap(),
        );
    }

    Ok((
        response_headers,
        Redirect::to(&format!("/threads/{thread_id}")),
    ))
}

pub(super) async fn create_reply(
    Path(id): Path<String>,
    State(state): State<Arc<HttpDependencies>>,
    headers: HeaderMap,
    Form(form): Form<ReplyForm>,
) -> Result<(HeaderMap, Redirect), Response> {
    let Ok(thread_id) = id.parse::<u64>() else {
        return Err(not_found_response().into_response());
    };

    let body = form.body.trim();

    if body.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Html(String::from("Reply body cannot be empty")),
        )
            .into_response());
    }

    if body.chars().count() > 2_000 {
        return Err((
            StatusCode::BAD_REQUEST,
            Html(String::from("Reply body is too long.")),
        )
            .into_response());
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
                .into_response()
        })?
    {
        return Err((
            StatusCode::FORBIDDEN,
            Html(format!(
                "Posting is blocked by an active {} ban until {}.",
                ban.scope, ban.expires_at
            )),
        )
            .into_response());
    }

    let rate_key = namespaced_rate_key("reply", &fingerprint_key(&fingerprint));
    if let Some(captcha) = state.captcha.as_ref() {
        if state
            .rate_limiter
            .suspicious(&rate_key, 5, Duration::from_secs(60))
        {
            let token = form
                .captcha_token
                .as_deref()
                .map(str::trim)
                .filter(|token| !token.is_empty());
            let Some(token) = token else {
                return Err(thread_challenge_response(&state, thread_id, &form.body).await);
            };
            match captcha.verify(token, &key).await {
                Ok(true) => {}
                Ok(false) => {
                    return Err(thread_challenge_response(&state, thread_id, &form.body).await);
                }
                Err(_) => {
                    return Err((
                        StatusCode::SERVICE_UNAVAILABLE,
                        Html(String::from("CAPTCHA service unavailable")),
                    )
                        .into_response());
                }
            }
        }
    }

    if !state
        .rate_limiter
        .allow(&rate_key, 10, Duration::from_secs(60))
    {
        return Err((
            StatusCode::TOO_MANY_REQUESTS,
            Html(String::from(
                "Too many replies. Please wait before posting again.",
            )),
        )
            .into_response());
    }

    let origin = state.abuse_cipher.protect(&key).map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Html(String::from("Could not protect operational data")),
        )
            .into_response()
    })?;
    let (token, is_new) = anonymous_token(&headers);

    match forum::create_reply(&state.pool, thread_id, body, &token, &origin)
        .await
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Html(String::from("Database error")),
            )
                .into_response()
        })? {
        forum::CreateReplyResult::Created => {}
        forum::CreateReplyResult::NotFound => return Err(not_found_response().into_response()),
        forum::CreateReplyResult::Locked => {
            return Err((
                StatusCode::CONFLICT,
                Html(String::from("This thread is locked")),
            )
                .into_response());
        }
        forum::CreateReplyResult::Archived => {
            return Err((
                StatusCode::CONFLICT,
                Html(String::from("This thread is archived and read-only")),
            )
                .into_response());
        }
    }

    let mut response_headers = HeaderMap::new();

    if should_set_anonymous_cookie(is_new, &headers) {
        response_headers.insert(
            SET_COOKIE,
            HeaderValue::from_str(&anonymous_cookie(&token, &headers)).unwrap(),
        );
    }

    Ok((
        response_headers,
        Redirect::to(&format!("/threads/{thread_id}")),
    ))
}

fn report_details(form: &ReportForm) -> Result<Option<&str>, (StatusCode, Html<String>)> {
    let Some(details) = form.details.as_deref() else {
        return Ok(None);
    };

    let details = details.trim();

    if details.chars().count() > 400 {
        return Err((
            StatusCode::BAD_REQUEST,
            Html(String::from(
                "Report message cannot be longer than 400 characters.",
            )),
        ));
    }

    if details.is_empty() {
        return Ok(None);
    }

    Ok(Some(details))
}

pub(super) async fn report_thread(
    Path(id): Path<String>,
    State(state): State<Arc<HttpDependencies>>,
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

    let details = report_details(&form)?;

    let key = client_key(&headers);

    let rate_key = namespaced_rate_key("report", &key);
    if !state
        .rate_limiter
        .allow(&rate_key, 5, Duration::from_secs(60))
    {
        return Err((
            StatusCode::TOO_MANY_REQUESTS,
            Html(String::from(
                "Too many reports. Please wait before reporting again.",
            )),
        ));
    }

    let reported = forum::report_thread(&state.pool, thread_id, reason, details)
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

pub(super) async fn report_reply(
    Path(id): Path<String>,
    State(state): State<Arc<HttpDependencies>>,
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

    let details = report_details(&form)?;

    let key = client_key(&headers);

    let rate_key = namespaced_rate_key("report", &key);
    if !state
        .rate_limiter
        .allow(&rate_key, 5, Duration::from_secs(60))
    {
        return Err((
            StatusCode::TOO_MANY_REQUESTS,
            Html(String::from(
                "Too many reports. Please wait before reporting again.",
            )),
        ));
    }

    let Some(thread_id) = forum::report_reply(&state.pool, reply_id, reason, details)
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
