mod forum;

use askama::Template;
use axum::{
    Router,
    extract::{Path, State},
    http::StatusCode,
    response::Html,
    routing::get,
};
use std::sync::Arc;
use tower_http::services::ServeDir;

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

struct AppState {
    boards: Vec<forum::Board>,
}

#[tokio::main]
async fn main() {
    let state = std::sync::Arc::new(AppState {
        boards: forum::seed_boards(),
    });

    let app = Router::new()
        .route("/", get(home))
        .route("/boards/{slug}", get(board))
        .nest_service("/static", ServeDir::new("static"))
        .fallback(not_found)
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();

    axum::serve(listener, app).await.unwrap();
}

fn not_found_response() -> (StatusCode, Html<String>) {
    let page = NotFoundTemplate;

    (StatusCode::NOT_FOUND, Html(page.render().unwrap()))
}

async fn not_found() -> (StatusCode, Html<String>) {
    not_found_response()
}

async fn home(State(state): State<Arc<AppState>>) -> Html<String> {
    let template = HomeTemplate {
        site_name: "M-chan",
        boards: &state.boards,
    };

    Html(template.render().unwrap())
}

async fn board(
    Path(slug): Path<String>,
    State(state): State<Arc<AppState>>,
) -> Result<Html<String>, (StatusCode, Html<String>)> {
    let Some(board) = state.boards.iter().find(|board| board.slug == slug) else {
        return Err(not_found_response());
    };

    let template = BoardTemplate { board };

    Ok(Html(template.render().unwrap()))
}
