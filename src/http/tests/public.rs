use super::*;
use axum::http::StatusCode;

static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!();

#[sqlx::test(migrator = "MIGRATOR")]
async fn public_home_and_policy_routes_render(pool: sqlx::SqlitePool) {
    let app = test_router(pool);

    let home = send(&app, get_request("/")).await;
    assert_eq!(home.status(), StatusCode::OK);
    let home_body = response_text(home).await;
    assert!(home_body.contains("<h1>MChan</h1>"));
    assert!(home_body.contains("/engineering/ - Engineering"));
    assert!(home_body.contains("/b/ - Random"));
    assert!(home_body.contains("/pasum/ - PASUM"));

    let privacy = send(&app, get_request("/privacy")).await;
    assert_eq!(privacy.status(), StatusCode::OK);
    let privacy_body = response_text(privacy).await;
    assert!(privacy_body.contains("<h1>MChan Privacy Policy</h1>"));
    assert!(privacy_body.contains("<h2>What MChan stores</h2>"));
    assert!(privacy_body.contains("Published content can be read, copied,"));
    assert!(privacy_body.contains("quoted, and archived by other people."));

    let rules = send(&app, get_request("/rules")).await;
    assert_eq!(rules.status(), StatusCode::OK);
    let rules_body = response_text(rules).await;
    assert!(rules_body.contains("<h1>MChan Community Rules</h1>"));
    assert!(rules_body.contains("<h2>Prohibited content</h2>"));
    assert!(rules_body.contains("<li>Illegal content.</li>"));
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn approved_board_archive_and_thread_reads_render(pool: sqlx::SqlitePool) {
    let app = test_router(pool);

    let board = send(&app, get_request("/boards/engineering")).await;
    assert_eq!(board.status(), StatusCode::OK);
    let board_body = response_text(board).await;
    assert!(board_body.contains("/engineering/ - Engineering"));
    assert!(board_body.contains("Welcome to Engineering"));
    assert!(!board_body.contains("Study group ideas"));

    let archive = send(&app, get_request("/boards/engineering/archive")).await;
    assert_eq!(archive.status(), StatusCode::OK);
    let archive_body = response_text(archive).await;
    assert!(archive_body.contains("Engineering Archive"));
    assert!(archive_body.contains("Read-only archive"));
    assert!(archive_body.contains("Study group ideas"));

    let thread = send(&app, get_request("/threads/1")).await;
    assert_eq!(thread.status(), StatusCode::OK);
    let thread_body = response_text(thread).await;
    assert!(thread_body.contains("Welcome to Engineering"));
    assert!(thread_body.contains("Introduce yourself and share useful resources."));
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn new_thread_and_static_routes_render_or_not_found(pool: sqlx::SqlitePool) {
    let app = test_router(pool);

    let new_thread = send(&app, get_request("/boards/engineering/new")).await;
    assert_eq!(new_thread.status(), StatusCode::OK);
    let new_thread_body = response_text(new_thread).await;
    assert!(new_thread_body.contains("<h2>Start a new thread</h2>"));
    assert!(new_thread_body.contains("action=\"/boards/engineering/threads\""));

    let unknown_board = send(&app, get_request("/boards/no-such-board/new")).await;
    assert_eq!(unknown_board.status(), StatusCode::NOT_FOUND);

    let stylesheet = send(&app, get_request("/static/style.css")).await;
    assert_eq!(stylesheet.status(), StatusCode::OK);
    let stylesheet_body = response_text(stylesheet).await;
    assert!(stylesheet_body.contains("body {"));

    let missing_asset = send(&app, get_request("/static/no-such-asset.css")).await;
    assert_eq!(missing_asset.status(), StatusCode::NOT_FOUND);
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn unknown_public_paths_boards_and_threads_are_not_found(pool: sqlx::SqlitePool) {
    let app = test_router(pool);

    for uri in [
        "/does-not-exist",
        "/boards/no-such-board",
        "/threads/999999",
    ] {
        let response = send(&app, get_request(uri)).await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND, "{uri}");
        let body = response_text(response).await;
        assert!(body.contains("<h1>404</h1>"), "{uri}");
    }
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn disabled_board_blocks_board_and_direct_thread_reads(pool: sqlx::SqlitePool) {
    sqlx::query("UPDATE boards SET status = 'archived' WHERE slug = 'engineering'")
        .execute(&pool)
        .await
        .expect("disable seeded board");

    let app = test_router(pool);

    for uri in ["/boards/engineering", "/threads/1"] {
        let response = send(&app, get_request(uri)).await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND, "{uri}");
    }
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn archived_thread_read_remains_visible_and_read_only(pool: sqlx::SqlitePool) {
    let app = test_router(pool);

    let response = send(&app, get_request("/threads/2")).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = response_text(response).await;
    assert!(body.contains("Study group ideas"));
    assert!(body.contains("What subject should we study for"));
    assert!(body.contains("<h2 id=\"archived-thread-heading\">Archived thread</h2>"));
    assert!(!body.contains("id=\"reply-form\""));
    assert!(!body.contains("<button type=\"submit\">Post reply</button>"));
}
