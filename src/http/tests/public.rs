use super::*;
use axum::http::StatusCode;
use std::fs;

static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!();

struct TemporaryMediaRoot {
    path: std::path::PathBuf,
}

impl TemporaryMediaRoot {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("mchan-media-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(path.join("images/img_test")).expect("create temporary media root");
        Self { path }
    }
}

impl Drop for TemporaryMediaRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn media_image_markup<'a>(body: &'a str, src: &str) -> &'a str {
    let src_offset = body
        .find(&format!(r#"src="{src}""#))
        .expect("media image source");
    let image_start = body[..src_offset].rfind("<img").expect("media image tag");
    let image_end = body[src_offset..]
        .find("/>")
        .map(|offset| src_offset + offset + 2)
        .expect("media image tag end");
    &body[image_start..image_end]
}
async fn insert_pagination_fixtures(pool: &sqlx::SqlitePool) {
    sqlx::query(
        r#"
        WITH RECURSIVE seq(id) AS (
            SELECT 1001
            UNION ALL
            SELECT id + 1 FROM seq WHERE id < 1021
        )
        INSERT INTO threads (
            id, board_id, title, body, status, created_at, archived_at
        )
        SELECT
            id,
            (SELECT id FROM boards WHERE slug = 'engineering'),
            'active-page-marker-' || id,
            'active-page-body-' || id,
            'visible',
            '9999-01-01 00:00:00',
            NULL
        FROM seq
        "#,
    )
    .execute(pool)
    .await
    .expect("insert active pagination fixtures");

    sqlx::query(
        r#"
        WITH RECURSIVE seq(id) AS (
            SELECT 2001
            UNION ALL
            SELECT id + 1 FROM seq WHERE id < 2051
        )
        INSERT INTO threads (
            id, board_id, title, body, status, created_at, archived_at
        )
        SELECT
            id,
            (SELECT id FROM boards WHERE slug = 'engineering'),
            'archive-page-marker-' || id,
            'archive-page-body-' || id,
            'visible',
            '9999-01-02 00:00:00',
            '9999-01-02 00:00:00'
        FROM seq
        "#,
    )
    .execute(pool)
    .await
    .expect("insert archive pagination fixtures");

    sqlx::query(
        r#"
        WITH RECURSIVE seq(id) AS (
            SELECT 9001
            UNION ALL
            SELECT id + 1 FROM seq WHERE id < 9004
        )
        INSERT INTO replies (id, thread_id, body, status, created_at, poster_id)
        SELECT
            id,
            1021,
            'page-preview-marker-' || id,
            'visible',
            '9999-01-03 00:00:00',
            'pagination-fixture'
        FROM seq
        "#,
    )
    .execute(pool)
    .await
    .expect("insert recent-reply pagination fixtures");
}

fn assert_in_order(body: &str, earlier: &str, later: &str) {
    assert!(
        body.find(earlier).expect("earlier marker") < body.find(later).expect("later marker"),
        "{earlier:?} should precede {later:?}"
    );
}
#[sqlx::test(migrator = "MIGRATOR")]
async fn active_board_pages_boundaries_order_and_recent_previews(pool: sqlx::SqlitePool) {
    insert_pagination_fixtures(&pool).await;
    let app = test_router(pool);

    let page_one = send(&app, get_request("/boards/engineering")).await;
    assert_eq!(page_one.status(), StatusCode::OK);
    let page_one_body = response_text(page_one).await;
    for id in (1002..=1021).rev() {
        assert!(page_one_body.contains(&format!("active-page-marker-{id}")));
    }
    assert!(!page_one_body.contains("active-page-marker-1001"));
    assert_in_order(
        &page_one_body,
        "active-page-marker-1021",
        "active-page-marker-1002",
    );
    assert!(page_one_body.contains("page-preview-marker-9004"));
    assert!(page_one_body.contains("page-preview-marker-9003"));
    assert!(page_one_body.contains("page-preview-marker-9002"));
    assert!(!page_one_body.contains("page-preview-marker-9001"));
    assert!(page_one_body.contains(r#"href="/boards/engineering?page=2""#));
    assert!(!page_one_body.contains(">Previous</a>"));

    let page_two = send(&app, get_request("/boards/engineering?page=2")).await;
    assert_eq!(page_two.status(), StatusCode::OK);
    let page_two_body = response_text(page_two).await;
    assert!(page_two_body.contains("active-page-marker-1001"));
    assert!(page_two_body.contains("Welcome to Engineering"));
    assert!(!page_two_body.contains("active-page-marker-1021"));
    assert!(!page_two_body.contains("page-preview-marker-9004"));
    assert!(page_two_body.contains(r#"href="/boards/engineering?page=1""#));
    assert!(!page_two_body.contains(">Next</a>"));
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn archive_pages_use_fifty_records_and_navigate(pool: sqlx::SqlitePool) {
    insert_pagination_fixtures(&pool).await;
    let app = test_router(pool);

    let page_one = send(&app, get_request("/boards/engineering/archive")).await;
    assert_eq!(page_one.status(), StatusCode::OK);
    let page_one_body = response_text(page_one).await;
    for id in (2002..=2051).rev() {
        assert!(page_one_body.contains(&format!("archive-page-marker-{id}")));
    }
    assert!(!page_one_body.contains("archive-page-marker-2001"));
    assert!(page_one_body.contains(r#"href="/boards/engineering/archive?page=2""#));
    assert!(!page_one_body.contains(">Previous</a>"));

    let page_two = send(&app, get_request("/boards/engineering/archive?page=2")).await;
    assert_eq!(page_two.status(), StatusCode::OK);
    let page_two_body = response_text(page_two).await;
    assert!(page_two_body.contains("archive-page-marker-2001"));
    assert!(page_two_body.contains("Study group ideas"));
    assert!(!page_two_body.contains("archive-page-marker-2051"));
    assert!(!page_two_body.contains(r#"href="/boards/engineering/archive?page=3""#));
    assert!(page_two_body.contains(r#"href="/boards/engineering/archive?page=1""#));
    assert!(!page_two_body.contains(">Next</a>"));
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn pagination_page_one_and_invalid_pages_are_contractual(pool: sqlx::SqlitePool) {
    insert_pagination_fixtures(&pool).await;
    let app = test_router(pool);

    let implicit = send(&app, get_request("/boards/engineering")).await;
    let explicit = send(&app, get_request("/boards/engineering?page=1")).await;
    assert_eq!(implicit.status(), StatusCode::OK);
    assert_eq!(explicit.status(), StatusCode::OK);
    assert_eq!(response_text(implicit).await, response_text(explicit).await);

    let archive_implicit = send(&app, get_request("/boards/engineering/archive")).await;
    let archive_explicit = send(&app, get_request("/boards/engineering/archive?page=1")).await;
    assert_eq!(archive_implicit.status(), StatusCode::OK);
    assert_eq!(archive_explicit.status(), StatusCode::OK);
    assert_eq!(
        response_text(archive_implicit).await,
        response_text(archive_explicit).await
    );

    for path in ["/boards/engineering", "/boards/engineering/archive"] {
        for page in ["0", "-1", "not-a-number", "999999999999999999999999999999"] {
            let response = send(&app, get_request(&format!("{path}?page={page}"))).await;
            assert_eq!(
                response.status(),
                StatusCode::BAD_REQUEST,
                "{path}?page={page}"
            );
        }
    }

    let past_end = send(&app, get_request("/boards/engineering?page=999")).await;
    assert_eq!(past_end.status(), StatusCode::OK);
    let past_end_body = response_text(past_end).await;
    assert!(!past_end_body.contains("active-page-marker-"));
    assert!(past_end_body.contains(r#"href="/boards/engineering?page=998""#));
    assert!(!past_end_body.contains(">Next</a>"));
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn public_home_and_policy_routes_render(pool: sqlx::SqlitePool) {
    let app = test_router(pool);

    let home = send(&app, get_request("/")).await;
    assert_eq!(home.status(), StatusCode::OK);
    let home_body = response_text(home).await;
    assert!(
        home_body.contains(r#"<img class="site-wordmark" src="/static/mlogo.png" alt="MChan" />"#)
    );
    assert!(!home_body.contains("<h1>MChan</h1>"));
    assert!(home_body.contains("/engineering/ - Engineering"));
    assert!(home_body.contains("/b/ - Random"));
    assert!(home_body.contains("/pasum/ - PASUM"));
    assert!(home_body.contains("/asid/ - ASID"));

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

    let changelog = send(&app, get_request("/changelog")).await;
    assert_eq!(changelog.status(), StatusCode::OK);
    let changelog_body = response_text(changelog).await;
    assert!(changelog_body.contains("<nav class=\"site-nav\""));
    assert!(changelog_body.contains("<h1>Changelog</h1>"));
    assert!(changelog_body.contains("[0.7]"));
    assert!(changelog_body.contains("GET /health"));
    assert!(changelog_body.contains("Discord moderation"));
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
async fn media_files_are_served_from_the_shared_storage_root(pool: sqlx::SqlitePool) {
    let media_root = TemporaryMediaRoot::new();
    let image_bytes = b"test webp bytes";
    fs::write(
        media_root.path.join("images/img_test/display.webp"),
        image_bytes,
    )
    .expect("write temporary media file");

    let app = router(test_dependencies_with_media_storage_root(
        pool,
        HashSet::new(),
        None,
        None,
        media_root.path.clone(),
    ));
    let response = send(&app, get_request("/images/img_test/display.webp")).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("read served media");
    assert_eq!(body.as_ref(), &image_bytes[..]);
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
        assert!(body.contains("<h1>404 Page Not Found</h1>"), "{uri}");
    }
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn archived_board_listing_and_direct_reads_remain_available(pool: sqlx::SqlitePool) {
    sqlx::query("UPDATE boards SET status = 'archived' WHERE slug = 'engineering'")
        .execute(&pool)
        .await
        .expect("archive seeded board");

    let app = test_router(pool);

    let home = send(&app, get_request("/")).await;
    assert_eq!(home.status(), StatusCode::OK);
    let home_body = response_text(home).await;
    assert!(!home_body.contains("/engineering/ - Engineering"));

    let board = send(&app, get_request("/boards/engineering")).await;
    assert_eq!(board.status(), StatusCode::OK);
    let board_body = response_text(board).await;
    assert!(board_body.contains("/engineering/ - Engineering"));
    assert!(board_body.contains("Welcome to Engineering"));
    assert!(!board_body.contains("Start a New Thread"));
    assert!(!board_body.contains("[Reply]"));

    let thread = send(&app, get_request("/threads/1")).await;
    assert_eq!(thread.status(), StatusCode::OK);
    let thread_body = response_text(thread).await;
    assert!(thread_body.contains("Welcome to Engineering"));
    assert!(thread_body.contains("Introduce yourself and share useful resources."));
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

#[sqlx::test(migrator = "MIGRATOR")]
async fn media_uses_thumbnail_in_board_and_archive_and_display_in_thread(pool: sqlx::SqlitePool) {
    sqlx::query(
        r#"
        INSERT INTO post_media (
            thread_id, thumbnail_path, display_path, mime_type, width, height
        )
        VALUES (1, '/images/thread-1/thumb.webp', '/images/thread-1/display.webp', 'image/webp', 1200, 800)
        "#,
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        r#"
        INSERT INTO post_media (
            reply_id, thumbnail_path, display_path, mime_type, width, height
        )
        VALUES (1, '/images/reply-1/thumb.webp', '/images/reply-1/display.webp', 'image/webp', 640, 480)
        "#,
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        r#"
        INSERT INTO post_media (
            thread_id, thumbnail_path, display_path, mime_type, width, height
        )
        VALUES (2, '/images/thread-2/thumb.webp', '/images/thread-2/display.webp', 'image/webp', 900, 600)
        "#,
    )
    .execute(&pool)
    .await
    .unwrap();

    let app = test_router(pool);
    let board = send(&app, get_request("/boards/engineering")).await;
    assert_eq!(board.status(), StatusCode::OK);
    let board_body = response_text(board).await;
    assert!(board_body.contains(r#"<img"#));
    assert!(board_body.contains(r#"src="/images/thread-1/thumb.webp""#));
    assert!(board_body.contains(r#"src="/images/reply-1/thumb.webp""#));
    assert!(!board_body.contains(r#"src="/images/thread-1/display.webp""#));

    let archive = send(&app, get_request("/boards/engineering/archive")).await;
    assert_eq!(archive.status(), StatusCode::OK);
    let archive_body = response_text(archive).await;
    assert!(archive_body.contains(r#"src="/images/thread-2/thumb.webp""#));
    assert!(!archive_body.contains(r#"src="/images/thread-2/display.webp""#));

    let thread = send(&app, get_request("/threads/1")).await;
    assert_eq!(thread.status(), StatusCode::OK);
    let thread_body = response_text(thread).await;
    assert!(thread_body.contains(r#"src="/images/thread-1/display.webp""#));
    assert!(thread_body.contains(r#"src="/images/reply-1/display.webp""#));
    let thread_image = media_image_markup(&thread_body, "/images/thread-1/display.webp");
    assert!(thread_image.contains(r#"width="1200""#));
    assert!(thread_image.contains(r#"height="800""#));
    let reply_image = media_image_markup(&thread_body, "/images/reply-1/display.webp");
    assert!(reply_image.contains(r#"width="640""#));
    assert!(reply_image.contains(r#"height="480""#));
}
