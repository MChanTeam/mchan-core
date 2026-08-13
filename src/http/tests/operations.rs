use super::*;

static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!();
const DISCORD_TOKEN: &str = "discord-test-token";
const DISCORD_MODERATOR: &str = "discord:moderator-123";

fn moderate_request(
    report_id: u64,
    action: &str,
    days: Option<u32>,
    moderator: &str,
) -> Request<Body> {
    let body = serde_json::json!({
        "report_id": report_id,
        "action": action,
        "days": days,
        "moderator": moderator,
    });
    Request::builder()
        .method(Method::POST)
        .uri("/internal/discord/moderate")
        .header(CONTENT_TYPE, "application/json")
        .body(Body::from(
            serde_json::to_vec(&body).expect("moderation request serializes"),
        ))
        .expect("valid moderation request")
}

async fn fixture_thread_id(pool: &SqlitePool) -> u64 {
    sqlx::query_scalar::<_, i64>("SELECT id FROM threads WHERE title = 'Welcome to Engineering'")
        .fetch_one(pool)
        .await
        .expect("seeded thread exists") as u64
}

async fn insert_report(pool: &SqlitePool, thread_id: u64) -> u64 {
    sqlx::query("INSERT INTO reports (thread_id, reason, status) VALUES (?, 'spam', 'pending')")
        .bind(thread_id as i64)
        .execute(pool)
        .await
        .expect("report fixture inserts")
        .last_insert_rowid() as u64
}

async fn insert_origin(pool: &SqlitePool, thread_id: u64) {
    let cipher = abuse::AbuseCipher::from_hex(TEST_ABUSE_KEY).expect("test abuse key is valid");
    let protected = cipher
        .protect("discord-test-client")
        .expect("origin encrypts");
    sqlx::query(
        "INSERT INTO post_origins (thread_id, client_fingerprint, nonce, ciphertext) VALUES (?, ?, ?, ?)",
    )
    .bind(thread_id as i64)
    .bind(protected.fingerprint.as_slice())
    .bind(protected.nonce.as_slice())
    .bind(protected.ciphertext)
    .execute(pool)
    .await
    .expect("origin fixture inserts");
}

fn discord_router(pool: SqlitePool, token: Option<&str>) -> Router {
    router(test_dependencies_with_discord_token(pool, token))
}

fn metrics_router(pool: SqlitePool, token: Option<&str>) -> Router {
    router(test_dependencies_with_ops_token(pool, token))
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn health_reports_metadata_and_cache_headers(pool: SqlitePool) {
    let app = discord_router(pool.clone(), None);
    let response = send(&app, get_request("/health")).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|v| v.to_str().ok()),
        Some("application/json")
    );
    assert_eq!(
        response
            .headers()
            .get(CACHE_CONTROL)
            .and_then(|v| v.to_str().ok()),
        Some("no-store, private")
    );
    assert_eq!(
        response.headers().get(PRAGMA).and_then(|v| v.to_str().ok()),
        Some("no-cache")
    );
    let healthy: serde_json::Value =
        serde_json::from_str(&response_text(response).await).expect("health JSON");
    assert_eq!(healthy["status"], "ok");
    assert_eq!(healthy["service"], "mchan");
    assert_eq!(healthy["version"], env!("CARGO_PKG_VERSION"));
    assert!(healthy["uptime_seconds"].is_u64());
    assert_eq!(healthy["database"], "ok");

    pool.close().await;
    let unhealthy = send(&app, get_request("/health")).await;
    assert_eq!(unhealthy.status(), StatusCode::SERVICE_UNAVAILABLE);
    let unhealthy: serde_json::Value =
        serde_json::from_str(&response_text(unhealthy).await).expect("health JSON");
    assert_eq!(unhealthy["status"], "unhealthy");
    assert_eq!(unhealthy["service"], "mchan");
    assert_eq!(unhealthy["version"], env!("CARGO_PKG_VERSION"));
    assert!(unhealthy["uptime_seconds"].is_u64());
    assert_eq!(unhealthy["database"], "unhealthy");
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn metrics_requires_configured_matching_bearer(pool: SqlitePool) {
    let disabled = metrics_router(pool.clone(), None);
    let response = send(
        &disabled,
        with_header(
            get_request("/internal/metrics"),
            "authorization",
            "Bearer ops-test-token",
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    let enabled = metrics_router(pool, Some("ops-test-token"));
    for request in [
        get_request("/internal/metrics"),
        with_header(
            get_request("/internal/metrics"),
            "authorization",
            "Bearer wrong-token",
        ),
    ] {
        let response = send(&enabled, request).await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(
            response
                .headers()
                .get(CACHE_CONTROL)
                .and_then(|value| value.to_str().ok()),
            Some("no-store, private")
        );
    }
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn metrics_returns_aggregate_counts_without_sensitive_data(pool: SqlitePool) {
    let thread_id = fixture_thread_id(&pool).await;
    insert_report(&pool, thread_id).await;
    insert_origin(&pool, thread_id).await;
    sqlx::query(
        r#"
        INSERT INTO bans (
            client_fingerprint,
            scope,
            board_id,
            report_id,
            moderator_email,
            reason,
            expires_at
        )
        SELECT X'0102', 'board', board_id, ?, 'sensitive@example.test', 'secret report',
               datetime('now', '+1 day')
        FROM threads
        WHERE id = ?
        "#,
    )
    .bind(1_i64)
    .bind(thread_id as i64)
    .execute(&pool)
    .await
    .expect("active board ban fixture inserts");

    let expected_boards =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM boards WHERE status = 'approved'")
            .fetch_one(&pool)
            .await
            .expect("approved board count");
    let expected_threads = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM threads WHERE status IN ('visible', 'locked') AND archived_at IS NULL",
    )
    .fetch_one(&pool)
    .await
    .expect("active thread count");
    let expected_replies =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM replies WHERE status = 'visible'")
            .fetch_one(&pool)
            .await
            .expect("visible reply count");
    let expected_pending_reports =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM reports WHERE status = 'pending'")
            .fetch_one(&pool)
            .await
            .expect("pending report count");
    let app = metrics_router(pool, Some("ops-test-token"));
    let response = send(
        &app,
        with_header(
            get_request("/internal/metrics"),
            "authorization",
            "Bearer ops-test-token",
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get(CACHE_CONTROL)
            .and_then(|value| value.to_str().ok()),
        Some("no-store, private")
    );
    let body = response_text(response).await;
    assert!(!body.contains("Welcome to Engineering"));
    assert!(!body.contains("discord-test-client"));
    assert!(!body.contains("sensitive@example.test"));
    assert!(!body.contains("secret report"));
    assert!(!body.contains("ops-test-token"));

    let metrics: serde_json::Value = serde_json::from_str(&body).expect("metrics JSON");
    assert_eq!(metrics["status"], "ok");
    assert_eq!(metrics["service"], "mchan");
    assert_eq!(metrics["version"], env!("CARGO_PKG_VERSION"));
    assert!(metrics["uptime_seconds"].is_u64());
    assert_eq!(metrics["database"]["status"], "ok");
    assert_eq!(metrics["content"]["boards"], expected_boards);
    assert_eq!(metrics["content"]["threads"], expected_threads);
    assert_eq!(metrics["content"]["replies"], expected_replies);
    assert_eq!(
        metrics["moderation"]["pending_reports"],
        expected_pending_reports
    );
    assert_eq!(metrics["moderation"]["active_board_bans"], 1);
    assert_eq!(metrics["moderation"]["active_site_bans"], 0);
    assert_eq!(metrics["integrations"]["miya_configured"], false);
    assert_eq!(metrics["integrations"]["image_processor_configured"], false);
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn moderation_endpoint_requires_enabled_bearer_without_mutating_db(pool: SqlitePool) {
    let thread_id = fixture_thread_id(&pool).await;
    let report_id = insert_report(&pool, thread_id).await;
    let app = discord_router(pool.clone(), None);
    let disabled = send(
        &app,
        with_header(
            moderate_request(report_id, "hide", None, DISCORD_MODERATOR),
            "authorization",
            "Bearer discord-test-token",
        ),
    )
    .await;
    assert_eq!(disabled.status(), StatusCode::NOT_FOUND);

    let enabled = discord_router(pool.clone(), Some(DISCORD_TOKEN));
    for request in [
        moderate_request(report_id, "hide", None, DISCORD_MODERATOR),
        with_header(
            moderate_request(report_id, "hide", None, DISCORD_MODERATOR),
            "authorization",
            "Bearer wrong-token",
        ),
    ] {
        let response = send(&enabled, request).await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT status FROM reports WHERE id = ?")
            .bind(report_id as i64)
            .fetch_one(&pool)
            .await
            .unwrap(),
        "pending"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM moderation_actions WHERE report_id = ?")
            .bind(report_id as i64)
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn discord_hide_applies_resolves_and_audits(pool: SqlitePool) {
    let thread_id = fixture_thread_id(&pool).await;
    let report_id = insert_report(&pool, thread_id).await;
    let app = discord_router(pool.clone(), Some(DISCORD_TOKEN));
    let response = send(
        &app,
        with_header(
            moderate_request(report_id, "hide", None, DISCORD_MODERATOR),
            "authorization",
            "Bearer discord-test-token",
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get(CACHE_CONTROL)
            .and_then(|v| v.to_str().ok()),
        Some("no-store, private")
    );
    assert_eq!(
        response_text(response).await,
        format!(r#"{{"status":"applied","report_id":{report_id},"action":"hide"}}"#)
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT status FROM threads WHERE id = ?")
            .bind(thread_id as i64)
            .fetch_one(&pool)
            .await
            .unwrap(),
        "hidden"
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT status FROM reports WHERE id = ?")
            .bind(report_id as i64)
            .fetch_one(&pool)
            .await
            .unwrap(),
        "resolved"
    );
    let audit = sqlx::query_as::<_, (String, String, String)>(
        "SELECT moderator_email, action, target_kind FROM moderation_actions WHERE report_id = ?",
    )
    .bind(report_id as i64)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        audit,
        (
            DISCORD_MODERATOR.to_owned(),
            "hide".to_owned(),
            "thread".to_owned()
        )
    );

    let repeat = send(
        &app,
        with_header(
            moderate_request(report_id, "hide", None, DISCORD_MODERATOR),
            "authorization",
            "Bearer discord-test-token",
        ),
    )
    .await;
    assert_eq!(repeat.status(), StatusCode::CONFLICT);
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn discord_board_ban_stores_ban_and_resolves_report(pool: SqlitePool) {
    let thread_id = fixture_thread_id(&pool).await;
    let report_id = insert_report(&pool, thread_id).await;
    insert_origin(&pool, thread_id).await;
    let app = discord_router(pool.clone(), Some(DISCORD_TOKEN));
    let response = send(
        &app,
        with_header(
            moderate_request(report_id, "ban-board", Some(7), DISCORD_MODERATOR),
            "authorization",
            "Bearer discord-test-token",
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT status FROM reports WHERE id = ?")
            .bind(report_id as i64)
            .fetch_one(&pool)
            .await
            .unwrap(),
        "resolved"
    );
    let ban = sqlx::query_as::<_, (String, i64, String, String)>(
        "SELECT scope, board_id, moderator_email, reason FROM bans WHERE report_id = ?",
    )
    .bind(report_id as i64)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(ban.0, "board");
    assert!(ban.1 > 0);
    assert_eq!(ban.2, DISCORD_MODERATOR);
    assert_eq!(ban.3, "spam");
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn discord_moderation_validates_inputs_and_missing_reports(pool: SqlitePool) {
    let app = discord_router(pool.clone(), Some(DISCORD_TOKEN));
    for (action, days, moderator, error) in [
        ("hide", None, "", "invalid moderator"),
        ("not-an-action", None, DISCORD_MODERATOR, "invalid action"),
        ("ban-board", Some(0), DISCORD_MODERATOR, "invalid days"),
        ("ban-board", None, DISCORD_MODERATOR, "invalid days"),
        (
            "hide",
            Some(1),
            DISCORD_MODERATOR,
            "days is only valid for ban actions",
        ),
    ] {
        let response = send(
            &app,
            with_header(
                moderate_request(999_999, action, days, moderator),
                "authorization",
                "Bearer discord-test-token",
            ),
        )
        .await;
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        assert!(response_text(response).await.contains(error));
    }
    let missing = send(
        &app,
        with_header(
            moderate_request(999_999, "hide", None, DISCORD_MODERATOR),
            "authorization",
            "Bearer discord-test-token",
        ),
    )
    .await;
    assert_eq!(missing.status(), StatusCode::NOT_FOUND);
}
