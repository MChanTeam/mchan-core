use axum::{
    Router,
    http::StatusCode,
    response::Html,
    routing::get,
};
use askama::Template;
use tower_http::services::ServeDir;

#[derive(Template)]
#[template(path = "home.html")]
struct HomeTemplate<'a> {
    site_name: &'a str,
}

#[derive(Template)]
#[template(path = "404.html")]
struct NotFoundTemplate;

#[tokio::main]
async fn main() {
    let app = Router::new()
        .route("/", get(home))
        .nest_service("/static", ServeDir::new("static"))
        .fallback(not_found);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000")
        .await
        .unwrap();

    axum::serve(listener, app).await.unwrap();
}

async fn not_found() -> (StatusCode, Html<String>) {
    let page = NotFoundTemplate;
    (
        StatusCode::NOT_FOUND,
        Html(page.render().unwrap()),
    )
}

async fn home() -> Html<String> {
    let template = HomeTemplate {
        site_name: "M-chan",
    };

    Html(template.render().unwrap())
}
