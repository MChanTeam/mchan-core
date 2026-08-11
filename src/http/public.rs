use super::*;

pub(super) fn not_found_response() -> (StatusCode, Html<String>) {
    let page = NotFoundTemplate;

    (StatusCode::NOT_FOUND, Html(page.render().unwrap()))
}

pub(super) async fn not_found() -> (StatusCode, Html<String>) {
    not_found_response()
}
fn render_policy_page(
    title: &'static str,
    markdown: &'static str,
) -> Result<Html<String>, StatusCode> {
    let content_html = render_trusted_markdown(markdown);
    let page = PolicyTemplate {
        title,
        content_html: &content_html,
    };

    page.render()
        .map(Html)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

pub(super) async fn privacy() -> Result<Html<String>, StatusCode> {
    render_policy_page("Privacy Policy", PRIVACY_MARKDOWN)
}

pub(super) async fn rules() -> Result<Html<String>, StatusCode> {
    render_policy_page("Community Rules", RULES_MARKDOWN)
}

pub(super) async fn home(
    State(state): State<Arc<HttpDependencies>>,
) -> Result<Html<String>, StatusCode> {
    let boards = forum::load_approved_boards(&state.pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let template = HomeTemplate {
        site_name: "MChan",
        boards: &boards,
    };

    Ok(Html(template.render().unwrap()))
}

pub(super) async fn board(
    Path(slug): Path<String>,
    State(state): State<Arc<HttpDependencies>>,
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

pub(super) async fn archive(
    Path(slug): Path<String>,
    State(state): State<Arc<HttpDependencies>>,
) -> Result<Html<String>, (StatusCode, Html<String>)> {
    let board = forum::load_archive(&state.pool, &slug).await.map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Html(String::from("Database error")),
        )
    })?;

    let Some(board) = board else {
        return Err(not_found_response());
    };

    let template = ArchiveTemplate { board: &board };

    Ok(Html(template.render().unwrap()))
}

pub(super) async fn thread(
    Path(id): Path<String>,
    State(state): State<Arc<HttpDependencies>>,
    headers: HeaderMap,
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

    let (captcha_required, captcha_site_key) = captcha_context(&state, &headers, "reply", 5);
    let template = ThreadTemplate {
        board: &board,
        thread: &thread,
        captcha_required,
        captcha_site_key,
        reply_body_value: String::new(),
    };

    Ok(Html(template.render().unwrap()))
}
