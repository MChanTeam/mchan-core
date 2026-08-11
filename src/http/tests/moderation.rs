use super::*;

static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!();

async fn fixture_thread_id(pool: &SqlitePool, title: &str) -> u64 {
    sqlx::query_scalar::<_, i64>("SELECT id FROM threads WHERE title = ? LIMIT 1")
        .bind(title)
        .fetch_one(pool)
        .await
        .expect("seeded thread exists") as u64
}

async fn fixture_reply_id(pool: &SqlitePool, body: &str) -> u64 {
    sqlx::query_scalar::<_, i64>("SELECT id FROM replies WHERE body = ? LIMIT 1")
        .bind(body)
        .fetch_one(pool)
        .await
        .expect("seeded reply exists") as u64
}

async fn insert_report(
    pool: &SqlitePool,
    target_column: &str,
    target_id: u64,
    reason: &str,
    created_at: Option<&str>,
) -> u64 {
    let query = match target_column {
        "thread_id" => {
            "INSERT INTO reports (thread_id, reason, status, created_at) VALUES (?, ?, 'pending', COALESCE(?, CURRENT_TIMESTAMP))"
        }
        "reply_id" => {
            "INSERT INTO reports (reply_id, reason, status, created_at) VALUES (?, ?, 'pending', COALESCE(?, CURRENT_TIMESTAMP))"
        }
        _ => panic!("unsupported report target"),
    };

    sqlx::query(query)
        .bind(target_id as i64)
        .bind(reason)
        .bind(created_at)
        .execute(pool)
        .await
        .expect("report fixture inserts")
        .last_insert_rowid() as u64
}

async fn insert_protected_origin(pool: &SqlitePool, thread_id: u64, client_key: &str) {
    let cipher = abuse::AbuseCipher::from_hex(TEST_ABUSE_KEY).expect("test abuse key is valid");
    let protected = cipher
        .protect(client_key)
        .expect("origin encryption succeeds");

    sqlx::query(
        "INSERT INTO post_origins (thread_id, client_fingerprint, nonce, ciphertext) VALUES (?, ?, ?, ?)",
    )
    .bind(thread_id as i64)
    .bind(protected.fingerprint.as_slice())
    .bind(protected.nonce.as_slice())
    .bind(protected.ciphertext)
    .execute(pool)
    .await
    .expect("protected origin fixture inserts");
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn public_reports_accept_valid_targets_reject_invalid_reasons_and_rate_limit(
    pool: SqlitePool,
) {
    let app = test_router(pool.clone());
    let thread_id = fixture_thread_id(&pool, "Welcome to Engineering").await;
    let reply_id = fixture_reply_id(&pool, "Glad to be here.").await;

    let invalid = send(
        &app,
        with_header(
            post_form(
                &format!("/threads/{thread_id}/report"),
                "reason=not-a-reason",
            ),
            "cf-connecting-ip",
            "198.51.100.10",
        ),
    )
    .await;
    assert_eq!(invalid.status(), StatusCode::BAD_REQUEST);
    assert!(
        response_text(invalid)
            .await
            .contains("Invalid report reason")
    );

    let valid_thread_location = format!("/threads/{thread_id}");
    let valid_thread = send(
        &app,
        with_header(
            post_form(&format!("/threads/{thread_id}/report"), "reason=spam"),
            "cf-connecting-ip",
            "198.51.100.10",
        ),
    )
    .await;
    assert_eq!(valid_thread.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        valid_thread
            .headers()
            .get("location")
            .and_then(|value| value.to_str().ok()),
        Some(valid_thread_location.as_str())
    );
    let valid_reply_location = format!("/replies/{reply_id}/report");
    let valid_reply = send(
        &app,
        with_header(
            post_form(&valid_reply_location, "reason=harassment"),
            "cf-connecting-ip",
            "198.51.100.11",
        ),
    )
    .await;
    assert_eq!(valid_reply.status(), StatusCode::SEE_OTHER);
    let expected_reply_redirect = format!("/threads/{thread_id}");
    assert_eq!(
        valid_reply
            .headers()
            .get("location")
            .and_then(|value| value.to_str().ok()),
        Some(expected_reply_redirect.as_str())
    );

    for _ in 0..5 {
        let response = send(
            &app,
            with_header(
                post_form(&format!("/threads/{thread_id}/report"), "reason=other"),
                "cf-connecting-ip",
                "198.51.100.12",
            ),
        )
        .await;
        assert_eq!(response.status(), StatusCode::SEE_OTHER);
    }

    let limited = send(
        &app,
        with_header(
            post_form(&format!("/threads/{thread_id}/report"), "reason=other"),
            "cf-connecting-ip",
            "198.51.100.12",
        ),
    )
    .await;
    assert_eq!(limited.status(), StatusCode::TOO_MANY_REQUESTS);
    assert!(response_text(limited).await.contains("Too many reports"));

    let pending_count =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM reports WHERE status = 'pending'")
            .fetch_one(&pool)
            .await
            .expect("report count is readable");
    assert_eq!(pending_count, 7);
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn moderator_queue_requires_allowlist_and_renders_pending_targets(pool: SqlitePool) {
    let thread_id = fixture_thread_id(&pool, "Welcome to Engineering").await;
    let reply_id = fixture_reply_id(&pool, "Glad to be here.").await;
    let first_report = insert_report(
        &pool,
        "thread_id",
        thread_id,
        "spam",
        Some("2020-01-01 00:00:00"),
    )
    .await;
    let second_report = insert_report(
        &pool,
        "reply_id",
        reply_id,
        "harassment",
        Some("2020-01-02 00:00:00"),
    )
    .await;
    assert!(first_report < second_report);

    let app = moderator_router(pool);

    let missing_identity = send(&app, get_request("/mod/reports")).await;
    assert_eq!(missing_identity.status(), StatusCode::FORBIDDEN);

    let unallowlisted = send(
        &app,
        with_header(
            get_request("/mod/reports"),
            "cf-access-authenticated-user-email",
            "other@example.com",
        ),
    )
    .await;
    assert_eq!(unallowlisted.status(), StatusCode::FORBIDDEN);

    let queue = send(
        &app,
        with_header(
            get_request("/mod/reports"),
            "cf-access-authenticated-user-email",
            "MODERATOR@example.com",
        ),
    )
    .await;
    assert_eq!(queue.status(), StatusCode::OK);
    let queue_html = response_text(queue).await;
    assert!(queue_html.contains("Pending Reports"));
    assert!(queue_html.contains("spam"));
    assert!(queue_html.contains("harassment"));
    let first_position = queue_html
        .find("2020-01-01 00:00:00")
        .expect("first report is rendered");
    let second_position = queue_html
        .find("2020-01-02 00:00:00")
        .expect("second report is rendered");
    assert!(first_position < second_position);
    assert!(queue_html.contains(&format!("/threads/{thread_id}#post-{thread_id}")));
    assert!(queue_html.contains(&format!("/threads/{thread_id}#reply-{reply_id}")));
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn moderator_mutations_require_allowlisted_identity_and_preserve_pending_report(
    pool: SqlitePool,
) {
    let thread_id = fixture_thread_id(&pool, "Welcome to Engineering").await;
    let report_id = insert_report(&pool, "thread_id", thread_id, "spam", None).await;
    let initial_target_status =
        sqlx::query_scalar::<_, String>("SELECT status FROM threads WHERE id = ?")
            .bind(thread_id as i64)
            .fetch_one(&pool)
            .await
            .expect("target status is readable");
    let app = moderator_router(pool.clone());
    let action_uri = format!("/mod/reports/{report_id}/hide");

    let missing_identity = send(&app, post_form(&action_uri, "")).await;
    assert_eq!(missing_identity.status(), StatusCode::FORBIDDEN);

    let unallowlisted = send(
        &app,
        with_header(
            post_form(&action_uri, ""),
            "cf-access-authenticated-user-email",
            "other@example.com",
        ),
    )
    .await;
    assert_eq!(unallowlisted.status(), StatusCode::FORBIDDEN);

    let report_status = sqlx::query_scalar::<_, String>("SELECT status FROM reports WHERE id = ?")
        .bind(report_id as i64)
        .fetch_one(&pool)
        .await
        .expect("report status is readable");
    let target_status = sqlx::query_scalar::<_, String>("SELECT status FROM threads WHERE id = ?")
        .bind(thread_id as i64)
        .fetch_one(&pool)
        .await
        .expect("target status is readable");
    let moderation_action_count =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM moderation_actions WHERE report_id = ?")
            .bind(report_id as i64)
            .fetch_one(&pool)
            .await
            .expect("moderation action count is readable");
    let ban_count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM bans WHERE report_id = ?")
        .bind(report_id as i64)
        .fetch_one(&pool)
        .await
        .expect("ban count is readable");

    assert_eq!(report_status, "pending");
    assert_eq!(target_status, initial_target_status);
    assert_eq!(moderation_action_count, 0);
    assert_eq!(ban_count, 0);
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn moderator_content_action_redirects_and_commits_status_and_audit_atomically(
    pool: SqlitePool,
) {
    let thread_id = fixture_thread_id(&pool, "Welcome to Engineering").await;
    let report_id = insert_report(&pool, "thread_id", thread_id, "spam", None).await;
    let app = moderator_router(pool.clone());

    let applied = send(
        &app,
        with_header(
            post_form(&format!("/mod/reports/{report_id}/hide"), ""),
            "cf-access-authenticated-user-email",
            MODERATOR_EMAIL,
        ),
    )
    .await;
    assert_eq!(applied.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        applied
            .headers()
            .get("location")
            .and_then(|value| value.to_str().ok()),
        Some("/mod/reports")
    );

    let thread_status = sqlx::query_scalar::<_, String>("SELECT status FROM threads WHERE id = ?")
        .bind(thread_id as i64)
        .fetch_one(&pool)
        .await
        .expect("content status is readable");
    let report_status = sqlx::query_scalar::<_, String>("SELECT status FROM reports WHERE id = ?")
        .bind(report_id as i64)
        .fetch_one(&pool)
        .await
        .expect("report status is readable");
    let audit = sqlx::query_as::<_, (String, String, i64)>(
        "SELECT action, moderator_email, target_id FROM moderation_actions WHERE report_id = ?",
    )
    .bind(report_id as i64)
    .fetch_one(&pool)
    .await
    .expect("moderation audit is readable");
    assert_eq!(thread_status, "hidden");
    assert_eq!(report_status, "resolved");
    assert_eq!(
        audit,
        (
            String::from("hide"),
            String::from(MODERATOR_EMAIL),
            thread_id as i64
        )
    );

    let second_action = send(
        &app,
        with_header(
            post_form(&format!("/mod/reports/{report_id}/resolve"), ""),
            "cf-access-authenticated-user-email",
            MODERATOR_EMAIL,
        ),
    )
    .await;
    assert_eq!(second_action.status(), StatusCode::CONFLICT);
    assert!(
        response_text(second_action)
            .await
            .contains("already been handled")
    );

    let audit_count =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM moderation_actions WHERE report_id = ?")
            .bind(report_id as i64)
            .fetch_one(&pool)
            .await
            .expect("audit count is readable");
    assert_eq!(audit_count, 1);
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn moderator_rejects_invalid_target_actions_without_handling_report(pool: SqlitePool) {
    let reply_id = fixture_reply_id(&pool, "Glad to be here.").await;
    let report_id = insert_report(&pool, "reply_id", reply_id, "spam", None).await;
    let app = moderator_router(pool.clone());

    let invalid_target = send(
        &app,
        with_header(
            post_form(&format!("/mod/reports/{report_id}/lock"), ""),
            "cf-access-authenticated-user-email",
            MODERATOR_EMAIL,
        ),
    )
    .await;
    assert_eq!(invalid_target.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert!(
        response_text(invalid_target)
            .await
            .contains("not valid for this report")
    );

    let report_status = sqlx::query_scalar::<_, String>("SELECT status FROM reports WHERE id = ?")
        .bind(report_id as i64)
        .fetch_one(&pool)
        .await
        .expect("report status is readable");
    let audit_count =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM moderation_actions WHERE report_id = ?")
            .bind(report_id as i64)
            .fetch_one(&pool)
            .await
            .expect("audit count is readable");
    assert_eq!(report_status, "pending");
    assert_eq!(audit_count, 0);

    let invalid_duration = send(
        &app,
        with_header(
            post_form(&format!("/mod/reports/{report_id}/ban-board"), "days=31"),
            "cf-access-authenticated-user-email",
            MODERATOR_EMAIL,
        ),
    )
    .await;
    assert_eq!(invalid_duration.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert!(
        response_text(invalid_duration)
            .await
            .contains("between 1 and 30 days")
    );
    let missing_origin = send(
        &app,
        with_header(
            post_form(&format!("/mod/reports/{report_id}/ban-board"), "days=7"),
            "cf-access-authenticated-user-email",
            MODERATOR_EMAIL,
        ),
    )
    .await;
    assert_eq!(missing_origin.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert!(
        response_text(missing_origin)
            .await
            .contains("no protected origin")
    );
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn moderator_ban_uses_retained_protected_origin_and_resolves_report(pool: SqlitePool) {
    let thread_id = fixture_thread_id(&pool, "Welcome to Engineering").await;
    let report_id = insert_report(&pool, "thread_id", thread_id, "threats", None).await;
    let client_key = "203.0.113.44";
    insert_protected_origin(&pool, thread_id, client_key).await;
    let app = moderator_router(pool.clone());

    let response = send(
        &app,
        with_header(
            post_form(&format!("/mod/reports/{report_id}/ban-site"), "days=30"),
            "cf-access-authenticated-user-email",
            MODERATOR_EMAIL,
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::SEE_OTHER);

    let cipher = abuse::AbuseCipher::from_hex(TEST_ABUSE_KEY).expect("test abuse key is valid");
    let expected_fingerprint = cipher.fingerprint(client_key).to_vec();
    let ban = sqlx::query_as::<_, (Vec<u8>, String, i64, i64)>(
        "SELECT client_fingerprint, scope, report_id, CAST((julianday(expires_at) - julianday('now')) * 86400 AS INTEGER) FROM bans WHERE report_id = ?",
    )
    .bind(report_id as i64)
    .fetch_one(&pool)
    .await
    .expect("ban is readable");
    assert_eq!(ban.0, expected_fingerprint);
    assert_eq!(ban.1, "site");
    assert_eq!(ban.2, report_id as i64);
    assert!(ban.3 > 0 && ban.3 <= 30 * 86_400);

    let report_status = sqlx::query_scalar::<_, String>("SELECT status FROM reports WHERE id = ?")
        .bind(report_id as i64)
        .fetch_one(&pool)
        .await
        .expect("report status is readable");
    let audit_action = sqlx::query_scalar::<_, String>(
        "SELECT action FROM moderation_actions WHERE report_id = ?",
    )
    .bind(report_id as i64)
    .fetch_one(&pool)
    .await
    .expect("ban audit is readable");
    assert_eq!(report_status, "resolved");
    assert_eq!(audit_action, "ban_site");
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn abuse_logs_require_moderator_and_return_decrypted_view_with_access_audit_and_cache_guards(
    pool: SqlitePool,
) {
    let thread_id = fixture_thread_id(&pool, "Welcome to Engineering").await;
    let client_key = "198.51.100.77";
    insert_protected_origin(&pool, thread_id, client_key).await;
    let app = moderator_router(pool.clone());

    let denied = send(&app, get_request("/mod/abuse-logs")).await;
    assert_eq!(denied.status(), StatusCode::FORBIDDEN);

    let response = send(
        &app,
        with_header(
            get_request("/mod/abuse-logs"),
            "cf-access-authenticated-user-email",
            "MODERATOR@example.com",
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
    assert_eq!(
        response
            .headers()
            .get(PRAGMA)
            .and_then(|value| value.to_str().ok()),
        Some("no-cache")
    );
    let html = response_text(response).await;
    assert!(html.contains("Restricted abuse logs"));
    assert!(html.contains(client_key));

    let access = sqlx::query_as::<_, (String, i64)>(
        "SELECT moderator_email, COUNT(*) FROM abuse_log_accesses GROUP BY moderator_email",
    )
    .fetch_one(&pool)
    .await
    .expect("abuse access audit is readable");
    assert_eq!(access, (String::from(MODERATOR_EMAIL), 1));
}
