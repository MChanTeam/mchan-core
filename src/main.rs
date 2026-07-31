mod forum;

use askama::Template;
use axum::{Router, extract::State, http::StatusCode, response::Html, routing::get};
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
        .nest_service("/static", ServeDir::new("static"))
        .fallback(not_found)
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();

    axum::serve(listener, app).await.unwrap();
}

async fn not_found() -> (StatusCode, Html<String>) {
    let page = NotFoundTemplate;
    (StatusCode::NOT_FOUND, Html(page.render().unwrap()))
}

async fn home(State(state): State<Arc<AppState>>) -> Html<String> {
    let template = HomeTemplate {
        site_name: "M-chan",
        boards: &state.boards,
    };

    Html(template.render().unwrap())
}
