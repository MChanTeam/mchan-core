use super::*;
use axum::http::{StatusCode, header::LOCATION};

static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!();

fn form(fields: &[(&str, &str)]) -> String {
    fields
        .iter()
        .map(|(name, value)| format!("{}={}", encode(name), encode(value)))
        .collect::<Vec<_>>()
        .join("&")
}

fn encode(value: &str) -> String {
    let mut encoded = String::new();
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            encoded.push(byte as char);
        } else {
            write!(encoded, "%{byte:02X}").unwrap();
        }
    }
    encoded
}

fn oversized_file_multipart(uri: &str) -> axum::http::Request<axum::body::Body> {
    const BOUNDARY: &str = "mchan-oversized-boundary";
    let prefix = format!(
        "--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"title\"\r\n\r\nOversized\r\n\
         --{BOUNDARY}\r\nContent-Disposition: form-data; name=\"body\"\r\n\r\nbody\r\n\
         --{BOUNDARY}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"large.png\"\r\n\
         Content-Type: image/png\r\n\r\n"
    );
    let suffix = format!("\r\n--{BOUNDARY}--\r\n");
    let mut body =
        Vec::with_capacity(prefix.len() + crate::media::MAX_UPLOAD_BYTES + 1 + suffix.len());
    body.extend_from_slice(prefix.as_bytes());
    body.resize(body.len() + crate::media::MAX_UPLOAD_BYTES + 1, 0);
    body.extend_from_slice(suffix.as_bytes());

    axum::http::Request::builder()
        .method("POST")
        .uri(uri)
        .header(
            axum::http::header::CONTENT_TYPE,
            format!("multipart/form-data; boundary={BOUNDARY}"),
        )
        .body(axum::body::Body::from(body))
        .expect("valid oversized multipart request")
}

fn redirected_thread_id(response: &axum::response::Response) -> u64 {
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    let location = response
        .headers()
        .get(LOCATION)
        .expect("successful posting redirects")
        .to_str()
        .unwrap();
    location
        .strip_prefix("/threads/")
        .expect("redirect points to the created thread")
        .parse()
        .unwrap()
}

async fn spend_reply_budget(app: &axum::Router) {
    for index in 0..5 {
        let response = send(
            app,
            with_header(
                post_form(
                    "/threads/1/replies",
                    &form(&[("body", &format!("budget reply {index}"))]),
                ),
                "cf-connecting-ip",
                TEST_CLIENT_IP,
            ),
        )
        .await;
        assert_eq!(response.status(), StatusCode::SEE_OTHER);
    }
}
async fn insert_reply_bump_fixtures(pool: &sqlx::SqlitePool) {
    sqlx::query(
        r#"
        INSERT INTO threads (
            id, board_id, title, body, status, created_at, bumped_at, is_pinned, archived_at
        )
        VALUES
            (
                3001,
                (SELECT id FROM boards WHERE slug = 'engineering'),
                'Older bump target',
                'Older bump target body',
                'visible',
                datetime('now', '-2 days'),
                datetime('now', '-2 days'),
                0,
                NULL
            ),
            (
                3002,
                (SELECT id FROM boards WHERE slug = 'engineering'),
                'Newer baseline',
                'Newer baseline body',
                'visible',
                datetime('now', '-1 day'),
                datetime('now', '-1 day'),
                0,
                NULL
            ),
            (
                3003,
                (SELECT id FROM boards WHERE slug = 'engineering'),
                'Hidden later',
                'Hidden later body',
                'hidden',
                datetime('now'),
                datetime('now', '+1 day'),
                0,
                NULL
            )
        "#,
    )
    .execute(pool)
    .await
    .expect("insert reply bump fixtures");
}

fn assert_in_order(body: &str, earlier: &str, later: &str) {
    assert!(
        body.find(earlier).expect("earlier marker") < body.find(later).expect("later marker"),
        "{earlier:?} should precede {later:?}"
    );
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn miya_allow_publishes_without_report(pool: sqlx::SqlitePool) {
    let (miya, server) = scripted_miya(200, r#"{"action":"allow"}"#).await;
    let app = miya_router(pool.clone(), miya);
    let response = send(
        &app,
        post_form(
            "/boards/engineering/threads",
            &form(&[("title", "Miya allow"), ("body", "safe body")]),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM threads WHERE title = 'Miya allow'")
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM reports")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    server.await.unwrap();
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn miya_block_returns_plain_422_without_inserting(pool: sqlx::SqlitePool) {
    let (miya, server) = scripted_miya(
        200,
        r#"{"action":"block","categories":[{"name":"unsafe","score":1.0}],"reasons":["<b>bad</b>"]}"#,
    )
    .await;
    let app = miya_router(pool.clone(), miya);
    let response = send(
        &app,
        post_form(
            "/boards/engineering/threads",
            &form(&[("title", "Miya block"), ("body", "blocked body")]),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        response
            .headers()
            .get("content-type")
            .unwrap()
            .to_str()
            .unwrap(),
        "text/plain; charset=utf-8"
    );
    let body = response_text(response).await;
    assert!(body.contains("Your post was blocked by moderation:"));
    assert!(body.contains("<b>bad</b>"));
    assert!(!body.contains("<html"));
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM threads WHERE title = 'Miya block'")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM reports")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    server.await.unwrap();
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn miya_review_publishes_pending_report(pool: sqlx::SqlitePool) {
    let (miya, server) = scripted_miya(
        200,
        r#"{"action":"review","categories":[{"name":"spam","score":0.9},{"name":"harassment","score":0.95}],"reasons":["needs review","highest category reason"]}"#,
    )
    .await;
    let app = miya_router(pool.clone(), miya);
    let response = send(
        &app,
        post_form(
            "/boards/engineering/threads",
            &form(&[("title", "Miya review"), ("body", "review body")]),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    let report = sqlx::query_as::<_, (String, String, String, i64)>(
        "SELECT reason, status, details, thread_id FROM reports",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(report.0, "other");
    assert_eq!(report.1, "pending");
    assert!(report.2.contains("harassment — highest category reason"));
    assert!(report.3 > 0);
    server.await.unwrap();
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn miya_failure_publishes_unchecked_pending_report(pool: sqlx::SqlitePool) {
    let (miya, server) = scripted_miya(503, r#"{"error":"down"}"#).await;
    let app = miya_router(pool.clone(), miya);
    let response = send(
        &app,
        post_form(
            "/boards/engineering/threads",
            &form(&[("title", "Miya failure"), ("body", "unchecked body")]),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    let details = sqlx::query_scalar::<_, String>("SELECT details FROM reports")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(details, "Miya unavailable — content was not checked.");
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT status FROM reports")
            .fetch_one(&pool)
            .await
            .unwrap(),
        "pending"
    );
    server.await.unwrap();
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn miya_disabled_publishes_without_report(pool: sqlx::SqlitePool) {
    let app = test_router(pool.clone());
    let response = send(
        &app,
        post_form(
            "/boards/engineering/threads",
            &form(&[("title", "Miya disabled"), ("body", "unchecked body")]),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM reports")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn thread_and_reply_creation_persist_and_redirect(pool: sqlx::SqlitePool) {
    let app = test_router(pool.clone());
    let thread_response = send(
        &app,
        with_header(
            post_form(
                "/boards/engineering/threads",
                &form(&[("title", "A new thread"), ("body", "A useful message")]),
            ),
            "cf-connecting-ip",
            TEST_CLIENT_IP,
        ),
    )
    .await;
    let thread_id = redirected_thread_id(&thread_response);

    let thread = sqlx::query_as::<_, (String, String, String)>(
        "SELECT title, body, poster_id FROM threads WHERE id = ?",
    )
    .bind(thread_id as i64)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(thread.0, "A new thread");
    assert_eq!(thread.1, "A useful message");
    assert!(thread.2.starts_with("Anonymous ##"));
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM post_origins WHERE thread_id = ?")
            .bind(thread_id as i64)
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );

    let board_response = send(&app, get_request("/boards/engineering")).await;
    assert_eq!(board_response.status(), StatusCode::OK);
    assert!(
        response_text(board_response)
            .await
            .contains(&format!(r#"<span class="post-author">{}</span>"#, thread.2))
    );

    let reply_response = send(
        &app,
        with_header(
            post_form(
                &format!("/threads/{thread_id}/replies"),
                &form(&[("body", "A useful reply")]),
            ),
            "cf-connecting-ip",
            TEST_CLIENT_IP,
        ),
    )
    .await;
    assert_eq!(reply_response.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        reply_response
            .headers()
            .get(LOCATION)
            .unwrap()
            .to_str()
            .unwrap(),
        format!("/threads/{thread_id}")
    );

    let reply = sqlx::query_as::<_, (String, String)>(
        "SELECT body, poster_id FROM replies WHERE thread_id = ? ORDER BY id DESC LIMIT 1",
    )
    .bind(thread_id as i64)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(reply.0, "A useful reply");
    assert_eq!(reply.1, thread.2);

    let other_ip_response = send(
        &app,
        with_header(
            post_form(
                &format!("/threads/{thread_id}/replies"),
                &form(&[("body", "A reply from another IP")]),
            ),
            "cf-connecting-ip",
            "198.51.100.55",
        ),
    )
    .await;
    assert_eq!(other_ip_response.status(), StatusCode::SEE_OTHER);
    let other_poster_id = sqlx::query_scalar::<_, String>(
        "SELECT poster_id FROM replies WHERE thread_id = ? ORDER BY id DESC LIMIT 1",
    )
    .bind(thread_id as i64)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_ne!(other_poster_id, thread.2);
}
#[sqlx::test(migrator = "MIGRATOR")]
async fn successful_reply_bumps_older_thread_but_failed_reply_does_not(pool: sqlx::SqlitePool) {
    insert_reply_bump_fixtures(&pool).await;
    let app = test_router(pool);

    let initial = send(&app, get_request("/boards/engineering")).await;
    assert_eq!(initial.status(), StatusCode::OK);
    let initial_body = response_text(initial).await;
    assert_in_order(&initial_body, "Newer baseline", "Older bump target");
    assert!(!initial_body.contains("Hidden later"));

    let failed = send(
        &app,
        post_form("/threads/3001/replies", &form(&[("body", "")])),
    )
    .await;
    assert_eq!(failed.status(), StatusCode::BAD_REQUEST);

    let after_failed = send(&app, get_request("/boards/engineering")).await;
    assert_eq!(after_failed.status(), StatusCode::OK);
    let after_failed_body = response_text(after_failed).await;
    assert_in_order(&after_failed_body, "Newer baseline", "Older bump target");

    let successful = send(
        &app,
        with_header(
            post_form(
                "/threads/3001/replies",
                &form(&[("body", "Bump this older thread")]),
            ),
            "cf-connecting-ip",
            "203.0.113.50",
        ),
    )
    .await;
    assert_eq!(successful.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        successful
            .headers()
            .get(LOCATION)
            .unwrap()
            .to_str()
            .unwrap(),
        "/threads/3001"
    );

    let after_success = send(&app, get_request("/boards/engineering")).await;
    assert_eq!(after_success.status(), StatusCode::OK);
    let after_success_body = response_text(after_success).await;
    assert_in_order(&after_success_body, "Older bump target", "Newer baseline");
    assert!(!after_success_body.contains("Hidden later"));
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn posting_validation_failures_are_bad_requests(pool: sqlx::SqlitePool) {
    let app = test_router(pool);

    for (uri, body, expected) in [
        (
            "/boards/engineering/threads",
            form(&[("title", ""), ("body", "body")]),
            "Thread title cannot be empty.",
        ),
        (
            "/boards/engineering/threads",
            form(&[("title", "title"), ("body", "")]),
            "Thread body cannot be empty",
        ),
        (
            "/threads/1/replies",
            form(&[("body", "")]),
            "Reply body cannot be empty",
        ),
    ] {
        let response = send(&app, post_form(uri, &body)).await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert!(response_text(response).await.contains(expected));
    }

    let long_title = "t".repeat(121);
    let response = send(
        &app,
        post_form(
            "/boards/engineering/threads",
            &form(&[("title", &long_title), ("body", "body")]),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let long_body = "b".repeat(2_001);
    let response = send(
        &app,
        post_form("/threads/1/replies", &form(&[("body", &long_body)])),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let response = send(
        &app,
        post_form(
            "/boards/engineering/threads",
            &form(&[("title", "title"), ("body", &long_body)]),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let limit_body = "b".repeat(2_000);
    let response = send(
        &app,
        post_form("/threads/1/replies", &form(&[("body", &limit_body)])),
    )
    .await;
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn locked_and_archived_reply_conflicts_do_not_insert(pool: sqlx::SqlitePool) {
    let archived_id =
        sqlx::query_scalar::<_, i64>("SELECT id FROM threads WHERE title = 'Study group ideas'")
            .fetch_one(&pool)
            .await
            .unwrap();
    sqlx::query("UPDATE threads SET status = 'locked' WHERE id = 1")
        .execute(&pool)
        .await
        .unwrap();

    let app = test_router(pool.clone());
    let locked = send(
        &app,
        post_form("/threads/1/replies", &form(&[("body", "locked draft")])),
    )
    .await;
    assert_eq!(locked.status(), StatusCode::CONFLICT);

    let archived = send(
        &app,
        post_form(
            &format!("/threads/{archived_id}/replies"),
            &form(&[("body", "archived draft")]),
        ),
    )
    .await;
    assert_eq!(archived.status(), StatusCode::CONFLICT);
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM replies WHERE body IN ('locked draft', 'archived draft')"
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        0
    );
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn disabled_board_rejects_thread_write(pool: sqlx::SqlitePool) {
    sqlx::query("UPDATE boards SET status = 'archived' WHERE slug = 'pasum'")
        .execute(&pool)
        .await
        .unwrap();
    let app = test_router(pool.clone());

    let response = send(
        &app,
        post_form(
            "/boards/pasum/threads",
            &form(&[("title", "Disabled"), ("body", "Should not persist")]),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM threads WHERE title = 'Disabled'")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn archived_board_rejects_posts_until_restored(pool: sqlx::SqlitePool) {
    let initial_threads = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM threads WHERE board_id = (SELECT id FROM boards WHERE slug = 'engineering')",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let initial_replies =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM replies WHERE thread_id = 1")
            .fetch_one(&pool)
            .await
            .unwrap();

    sqlx::query("UPDATE boards SET status = 'archived' WHERE slug = 'engineering'")
        .execute(&pool)
        .await
        .unwrap();
    let app = test_router(pool.clone());

    let rejected_thread = send(
        &app,
        with_header(
            post_form(
                "/boards/engineering/threads",
                &form(&[
                    ("title", "Archived board thread"),
                    ("body", "Should not persist"),
                ]),
            ),
            "cf-connecting-ip",
            TEST_CLIENT_IP,
        ),
    )
    .await;
    assert_eq!(rejected_thread.status(), StatusCode::NOT_FOUND);
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM threads WHERE board_id = (SELECT id FROM boards WHERE slug = 'engineering')",
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        initial_threads
    );

    let rejected_reply = send(
        &app,
        with_header(
            post_form(
                "/threads/1/replies",
                &form(&[("body", "Archived board reply")]),
            ),
            "cf-connecting-ip",
            TEST_CLIENT_IP,
        ),
    )
    .await;
    assert_eq!(rejected_reply.status(), StatusCode::NOT_FOUND);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM replies WHERE thread_id = 1")
            .fetch_one(&pool)
            .await
            .unwrap(),
        initial_replies
    );

    sqlx::query("UPDATE boards SET status = 'approved' WHERE slug = 'engineering'")
        .execute(&pool)
        .await
        .unwrap();

    let restored_thread = send(
        &app,
        with_header(
            post_form(
                "/boards/engineering/threads",
                &form(&[
                    ("title", "Restored board thread"),
                    ("body", "Persists after restore"),
                ]),
            ),
            "cf-connecting-ip",
            TEST_CLIENT_IP,
        ),
    )
    .await;
    assert_eq!(restored_thread.status(), StatusCode::SEE_OTHER);
    let restored_thread_id = redirected_thread_id(&restored_thread);
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM threads WHERE board_id = (SELECT id FROM boards WHERE slug = 'engineering')",
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        initial_threads + 1
    );

    let restored_reply = send(
        &app,
        with_header(
            post_form(
                &format!("/threads/{restored_thread_id}/replies"),
                &form(&[("body", "Persists after board restore")]),
            ),
            "cf-connecting-ip",
            TEST_CLIENT_IP,
        ),
    )
    .await;
    assert_eq!(restored_reply.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM replies WHERE thread_id = ?",)
            .bind(restored_thread_id as i64)
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM replies WHERE thread_id = 1")
            .fetch_one(&pool)
            .await
            .unwrap(),
        initial_replies
    );
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn thread_and_reply_rate_limits_are_namespaced(pool: sqlx::SqlitePool) {
    let app = test_router(pool);

    for index in 0..2 {
        let response = send(
            &app,
            post_form(
                "/boards/engineering/threads",
                &form(&[("title", &format!("Rate thread {index}")), ("body", "body")]),
            ),
        )
        .await;
        assert_eq!(response.status(), StatusCode::SEE_OTHER);
    }
    let thread_limited = send(
        &app,
        post_form(
            "/boards/engineering/threads",
            &form(&[("title", "Rate thread blocked"), ("body", "body")]),
        ),
    )
    .await;
    assert_eq!(thread_limited.status(), StatusCode::TOO_MANY_REQUESTS);

    for index in 0..10 {
        let response = send(
            &app,
            post_form(
                "/threads/1/replies",
                &form(&[("body", &format!("namespaced reply {index}"))]),
            ),
        )
        .await;
        assert_eq!(response.status(), StatusCode::SEE_OTHER);
    }
    let reply_limited = send(
        &app,
        post_form(
            "/threads/1/replies",
            &form(&[("body", "namespaced reply blocked")]),
        ),
    )
    .await;
    assert_eq!(reply_limited.status(), StatusCode::TOO_MANY_REQUESTS);
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn active_ban_maps_to_forbidden_for_thread_and_reply(pool: sqlx::SqlitePool) {
    let fingerprint = crate::abuse::AbuseCipher::from_hex(TEST_ABUSE_KEY)
        .unwrap()
        .fingerprint("local");
    sqlx::query("INSERT INTO reports (thread_id, reason, status) VALUES (1, 'spam', 'pending')")
        .execute(&pool)
        .await
        .unwrap();
    let report_id = sqlx::query_scalar::<_, i64>(
        "SELECT id FROM reports WHERE thread_id = 1 ORDER BY id DESC LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query(
        r#"
        INSERT INTO bans (
            client_fingerprint, scope, board_id, report_id, moderator_email,
            reason, expires_at
        )
        VALUES (
            ?, 'board', (SELECT id FROM boards WHERE slug = 'engineering'), ?,
            'moderator@example.com', 'spam', datetime('now', '+1 day')
        )
        "#,
    )
    .bind(fingerprint.as_slice())
    .bind(report_id)
    .execute(&pool)
    .await
    .unwrap();

    let app = test_router(pool);
    let thread = send(
        &app,
        post_form(
            "/boards/engineering/threads",
            &form(&[("title", "Banned"), ("body", "blocked")]),
        ),
    )
    .await;
    assert_eq!(thread.status(), StatusCode::FORBIDDEN);

    let reply = send(
        &app,
        post_form("/threads/1/replies", &form(&[("body", "also blocked")])),
    )
    .await;
    assert_eq!(reply.status(), StatusCode::FORBIDDEN);
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn thread_captcha_allow_continues_after_suspicious_threshold(pool: sqlx::SqlitePool) {
    let app = captcha_router(pool.clone(), [CaptchaOutcome::Allow]);
    let first = send(
        &app,
        with_header(
            post_form(
                "/boards/engineering/threads",
                &form(&[("title", "Captcha first"), ("body", "first")]),
            ),
            "cf-connecting-ip",
            TEST_CLIENT_IP,
        ),
    )
    .await;
    assert_eq!(first.status(), StatusCode::SEE_OTHER);

    let second = send(
        &app,
        with_header(
            post_form(
                "/boards/engineering/threads",
                &form(&[
                    ("title", "Captcha allowed"),
                    ("body", "allowed after challenge"),
                    ("cf-turnstile-response", "scripted-token"),
                ]),
            ),
            "cf-connecting-ip",
            TEST_CLIENT_IP,
        ),
    )
    .await;
    assert_eq!(second.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM threads WHERE title = 'Captcha allowed'"
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        1
    );
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn thread_captcha_reject_preserves_draft_after_threshold(pool: sqlx::SqlitePool) {
    let app = captcha_router(pool.clone(), [CaptchaOutcome::Reject]);
    let first = send(
        &app,
        with_header(
            post_form(
                "/boards/engineering/threads",
                &form(&[("title", "Captcha first"), ("body", "first")]),
            ),
            "cf-connecting-ip",
            TEST_CLIENT_IP,
        ),
    )
    .await;
    assert_eq!(first.status(), StatusCode::SEE_OTHER);

    let response = send(
        &app,
        with_header(
            post_form(
                "/boards/engineering/threads",
                &form(&[
                    ("title", "Draft thread title"),
                    ("body", "Draft thread body"),
                    ("cf-turnstile-response", "scripted-token"),
                ]),
            ),
            "cf-connecting-ip",
            TEST_CLIENT_IP,
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    let body = response_text(response).await;
    assert!(body.contains("test-site-key"));
    assert!(body.contains("Draft thread title"));
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM threads WHERE title = 'Draft thread title'"
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        0
    );
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn thread_captcha_unavailable_maps_to_service_unavailable(pool: sqlx::SqlitePool) {
    let app = captcha_router(pool, [CaptchaOutcome::Unavailable]);
    let first = send(
        &app,
        with_header(
            post_form(
                "/boards/engineering/threads",
                &form(&[("title", "Captcha first"), ("body", "first")]),
            ),
            "cf-connecting-ip",
            TEST_CLIENT_IP,
        ),
    )
    .await;
    assert_eq!(first.status(), StatusCode::SEE_OTHER);

    let response = send(
        &app,
        with_header(
            post_form(
                "/boards/engineering/threads",
                &form(&[
                    ("title", "Unavailable thread"),
                    ("body", "body"),
                    ("cf-turnstile-response", "scripted-token"),
                ]),
            ),
            "cf-connecting-ip",
            TEST_CLIENT_IP,
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn reply_captcha_allow_continues_after_suspicious_threshold(pool: sqlx::SqlitePool) {
    let app = captcha_router(pool.clone(), [CaptchaOutcome::Allow]);
    spend_reply_budget(&app).await;

    let response = send(
        &app,
        with_header(
            post_form(
                "/threads/1/replies",
                &form(&[
                    ("body", "allowed reply after challenge"),
                    ("cf-turnstile-response", "scripted-token"),
                ]),
            ),
            "cf-connecting-ip",
            TEST_CLIENT_IP,
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM replies WHERE body = 'allowed reply after challenge'"
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        1
    );
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn reply_captcha_reject_preserves_draft_after_threshold(pool: sqlx::SqlitePool) {
    let app = captcha_router(pool, [CaptchaOutcome::Reject]);
    spend_reply_budget(&app).await;

    let response = send(
        &app,
        with_header(
            post_form(
                "/threads/1/replies",
                &form(&[
                    ("body", "Draft reply body"),
                    ("cf-turnstile-response", "scripted-token"),
                ]),
            ),
            "cf-connecting-ip",
            TEST_CLIENT_IP,
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    let body = response_text(response).await;
    assert!(body.contains("test-site-key"));
    assert!(body.contains("Draft reply body"));
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn reply_captcha_unavailable_maps_to_service_unavailable(pool: sqlx::SqlitePool) {
    let app = captcha_router(pool, [CaptchaOutcome::Unavailable]);
    spend_reply_budget(&app).await;

    let response = send(
        &app,
        with_header(
            post_form(
                "/threads/1/replies",
                &form(&[
                    ("body", "Unavailable reply"),
                    ("cf-turnstile-response", "scripted-token"),
                ]),
            ),
            "cf-connecting-ip",
            TEST_CLIENT_IP,
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn image_thread_upload_persists_media_and_redirects(pool: sqlx::SqlitePool) {
    let (app, media) = media_router(
        pool.clone(),
        [ScriptedMediaOutcome::success(
            "image-thread-1",
            "/images/image-thread-1/thumbnail.webp",
            "/images/image-thread-1/display.webp",
            1280,
            720,
        )],
    );
    let response = send(
        &app,
        post_multipart(
            "/boards/engineering/threads",
            &[("title", "Image thread"), ("body", "A body with an image")],
            Some(("photo.png", "image/png", b"png bytes")),
        ),
    )
    .await;
    let thread_id = redirected_thread_id(&response);
    assert_eq!(
        sqlx::query_as::<_, (i64, String, String, String, i64, i64)>(
            "SELECT thread_id, thumbnail_path, display_path, mime_type, width, height FROM post_media WHERE thread_id = ?",
        )
        .bind(thread_id as i64)
        .fetch_one(&pool)
        .await
        .unwrap(),
        (
            thread_id as i64,
            "/images/image-thread-1/thumbnail.webp".to_owned(),
            "/images/image-thread-1/display.webp".to_owned(),
            "image/webp".to_owned(),
            1280,
            720,
        )
    );
    assert_eq!(
        media.uploads(),
        vec![MediaUpload {
            filename: Some("photo.png".to_owned()),
            content_type: Some("image/png".to_owned()),
            bytes: b"png bytes".to_vec(),
        }]
    );
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn oversized_multipart_file_returns_payload_too_large_without_processing(
    pool: sqlx::SqlitePool,
) {
    let (app, media) = media_router(
        pool.clone(),
        [ScriptedMediaOutcome::success(
            "oversized-image",
            "/images/oversized-image/thumbnail.webp",
            "/images/oversized-image/display.webp",
            800,
            600,
        )],
    );
    let response = send(
        &app,
        oversized_file_multipart("/boards/engineering/threads"),
    )
    .await;

    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
    assert!(media.uploads().is_empty());
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM threads WHERE title = 'Oversized'",)
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );

    let malformed = axum::http::Request::builder()
        .method("POST")
        .uri("/boards/engineering/threads")
        .header(
            axum::http::header::CONTENT_TYPE,
            "multipart/form-data; boundary=mchan-oversized-boundary",
        )
        .body(axum::body::Body::from("--mchan-oversized-boundary\r\n"))
        .expect("valid malformed multipart request");
    let malformed_response = send(&app, malformed).await;
    assert_eq!(malformed_response.status(), StatusCode::BAD_REQUEST);
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn image_reply_upload_persists_reply_media_and_redirects(pool: sqlx::SqlitePool) {
    let (app, media) = media_router(
        pool.clone(),
        [ScriptedMediaOutcome::success(
            "image-reply-1",
            "/images/image-reply-1/thumbnail.webp",
            "/images/image-reply-1/display.webp",
            640,
            480,
        )],
    );
    let response = send(
        &app,
        post_multipart(
            "/threads/1/replies",
            &[("body", "A reply with an image")],
            Some(("reply.jpg", "image/jpeg", b"jpeg bytes")),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        response.headers().get(LOCATION).unwrap().to_str().unwrap(),
        "/threads/1"
    );
    let reply_id =
        sqlx::query_scalar::<_, i64>("SELECT id FROM replies WHERE body = 'A reply with an image'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        sqlx::query_as::<_, (i64, String, String, String, i64, i64)>(
            "SELECT reply_id, thumbnail_path, display_path, mime_type, width, height FROM post_media WHERE reply_id = ?",
        )
        .bind(reply_id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        (
            reply_id,
            "/images/image-reply-1/thumbnail.webp".to_owned(),
            "/images/image-reply-1/display.webp".to_owned(),
            "image/webp".to_owned(),
            640,
            480,
        )
    );
    assert_eq!(media.deleted_image_ids(), Vec::<String>::new());
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn media_unconfigured_rejects_image_but_allows_text_post(pool: sqlx::SqlitePool) {
    let app = test_router(pool.clone());
    let image = send(
        &app,
        post_multipart(
            "/boards/engineering/threads",
            &[("title", "Unavailable image"), ("body", "body")],
            Some(("photo.png", "image/png", b"bytes")),
        ),
    )
    .await;
    assert_eq!(image.status(), StatusCode::SERVICE_UNAVAILABLE);

    let text = send(
        &app,
        post_form(
            "/boards/engineering/threads",
            &form(&[("title", "Text after image"), ("body", "body")]),
        ),
    )
    .await;
    assert_eq!(text.status(), StatusCode::SEE_OTHER);
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn media_processor_errors_map_to_stable_http_statuses(pool: sqlx::SqlitePool) {
    for (error, expected) in [
        (MediaError::TooLarge, StatusCode::PAYLOAD_TOO_LARGE),
        (
            MediaError::UnsupportedType,
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
        ),
        (MediaError::InvalidImage, StatusCode::UNPROCESSABLE_ENTITY),
        (MediaError::Timeout, StatusCode::GATEWAY_TIMEOUT),
    ] {
        let (app, _media) = media_router(pool.clone(), [ScriptedMediaOutcome::Error(error)]);
        let response = send(
            &app,
            post_multipart(
                "/boards/engineering/threads",
                &[("title", "Rejected image"), ("body", "body")],
                Some(("photo.png", "image/png", b"bytes")),
            ),
        )
        .await;
        assert_eq!(response.status(), expected);
    }
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn malformed_media_response_fails_safely(pool: sqlx::SqlitePool) {
    let (app, _media) = media_router(
        pool,
        [ScriptedMediaOutcome::Error(MediaError::MalformedResponse)],
    );
    let response = send(
        &app,
        post_multipart(
            "/boards/engineering/threads",
            &[("title", "Malformed image"), ("body", "body")],
            Some(("photo.png", "image/png", b"bytes")),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    assert!(
        response_text(response)
            .await
            .contains("Image processing failed")
    );
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn failed_media_insert_rolls_back_and_cleans_up_image(pool: sqlx::SqlitePool) {
    sqlx::query(
        "CREATE TRIGGER fail_post_media BEFORE INSERT ON post_media BEGIN SELECT RAISE(ABORT, 'test post_media failure'); END",
    )
    .execute(&pool)
    .await
    .unwrap();
    let (app, media) = media_router(
        pool.clone(),
        [ScriptedMediaOutcome::success(
            "image-rollback",
            "/images/image-rollback/thumbnail.webp",
            "/images/image-rollback/display.webp",
            800,
            600,
        )],
    );
    let response = send(
        &app,
        post_multipart(
            "/boards/engineering/threads",
            &[("title", "Rolled back image"), ("body", "body")],
            Some(("photo.png", "image/png", b"bytes")),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(media.deleted_image_ids(), vec!["image-rollback".to_owned()]);
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM threads WHERE title = 'Rolled back image'",
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM post_origins po JOIN threads t ON t.id = po.thread_id WHERE t.title = 'Rolled back image'",
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        0
    );
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn locked_reply_after_media_processing_cleans_up_image(pool: sqlx::SqlitePool) {
    sqlx::query("UPDATE threads SET status = 'locked' WHERE id = 1")
        .execute(&pool)
        .await
        .unwrap();
    let (app, media) = media_router(
        pool.clone(),
        [ScriptedMediaOutcome::success(
            "image-locked-reply",
            "/images/image-locked-reply/thumbnail.webp",
            "/images/image-locked-reply/display.webp",
            320,
            240,
        )],
    );
    let response = send(
        &app,
        post_multipart(
            "/threads/1/replies",
            &[("body", "locked image reply")],
            Some(("reply.png", "image/png", b"bytes")),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CONFLICT);
    assert_eq!(
        media.deleted_image_ids(),
        vec!["image-locked-reply".to_owned()]
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM replies WHERE body = 'locked image reply'",
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        0
    );
}
