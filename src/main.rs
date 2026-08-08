mod forum;

use askama::Template;
use axum::{
    Router,
    extract::{Form, Path, State},
    http::StatusCode,
    response::{Html, Redirect},
    routing::{get, post},
};
use sqlx::{SqlitePool, sqlite::SqliteConnectOptions};
use std::{str::FromStr, sync::Arc};
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

#[derive(Template)]
#[template(path = "new_thread.html")]
struct NewThreadTemplate<'a> {
    board: &'a forum::Board,
}

#[derive(serde::Deserialize)]
struct NewThreadForm {
    title: String,
    body: String,
}

#[derive(Template)]
#[template(path = "thread.html")]
struct ThreadTemplate<'a> {
    board: &'a forum::Board,
    thread: &'a forum::Thread,
}

struct AppState {
    pool: SqlitePool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let database_url =
        std::env::var("DATABASE_URL").unwrap_or_else(|_| String::from("sqlite://mchan.db"));

    let options = SqliteConnectOptions::from_str(&database_url)?.create_if_missing(true);
    let pool = SqlitePool::connect_with(options).await?;
    sqlx::migrate!().run(&pool).await?;

    let state = Arc::new(AppState { pool });

    let app = Router::new()
        .route("/", get(home))
        .route("/boards/{slug}", get(board))
        .route("/boards/{slug}/new", get(new_thread))
        .route("/boards/{slug}/threads", post(create_thread))
        .route("/threads/{id}", get(thread))
        .nest_service("/static", ServeDir::new("static"))
        .fallback(not_found)
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();

    axum::serve(listener, app).await?;

    Ok(())
}

fn not_found_response() -> (StatusCode, Html<String>) {
    let page = NotFoundTemplate;

    (StatusCode::NOT_FOUND, Html(page.render().unwrap()))
}

async fn not_found() -> (StatusCode, Html<String>) {
    not_found_response()
}

async fn home(State(state): State<Arc<AppState>>) -> Result<Html<String>, StatusCode> {
    let boards = forum::load_approved_boards(&state.pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let template = HomeTemplate {
        site_name: "M-chan",
        boards: &boards,
    };

    Ok(Html(template.render().unwrap()))
}

async fn board(
    Path(slug): Path<String>,
    State(state): State<Arc<AppState>>,
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

async fn new_thread(
    Path(slug): Path<String>,
    State(state): State<Arc<AppState>>,
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

    let template = NewThreadTemplate { board: &board };

    Ok(Html(template.render().unwrap()))
}

async fn create_thread(
    Path(slug): Path<String>,
    State(state): State<Arc<AppState>>,
    Form(form): Form<NewThreadForm>,
) -> Result<Redirect, (StatusCode, Html<String>)> {
    let title = form.title.trim();
    let body = form.body.trim();

    if title.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Html(String::from("Thread title cannot be empty.")),
        ));
    }

    if body.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Html(String::from("Thread body cannot be empty")),
        ));
    }

    if title.chars().count() > 120 {
        return Err((
            StatusCode::BAD_REQUEST,
            Html(String::from("Thread title is too long")),
        ));
    }

    if body.chars().count() > 10_000 {
        return Err((
            StatusCode::BAD_REQUEST,
            Html(String::from("Thread body is too long")),
        ));
    }

    let Some(thread_id) = forum::create_thread(&state.pool, &slug, title, body)
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

async fn thread(
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
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

    let template = ThreadTemplate {
        board: &board,
        thread: &thread,
    };

    Ok(Html(template.render().unwrap()))
}
