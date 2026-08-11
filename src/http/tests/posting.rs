use super::*;
use axum::http::{
    StatusCode,
    header::{LOCATION, SET_COOKIE},
};
use std::fmt::Write as _;

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

#[sqlx::test(migrator = "MIGRATOR")]
async fn thread_and_reply_creation_persist_and_redirect(pool: sqlx::SqlitePool) {
    let app = test_router(pool.clone());
    let cookie = "mchan_anon=thread-scoped-token";

    let thread_response = send(
        &app,
        with_cookie(
            post_form(
                "/boards/engineering/threads",
                &form(&[("title", "A new thread"), ("body", "A useful message")]),
            ),
            cookie,
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

    let reply_response = send(
        &app,
        with_cookie(
            post_form(
                &format!("/threads/{thread_id}/replies"),
                &form(&[("body", "A useful reply")]),
            ),
            cookie,
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
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn existing_cookie_is_reissued_secure_for_forwarded_https(pool: sqlx::SqlitePool) {
    let app = test_router(pool);
    let response = send(
        &app,
        with_header(
            with_cookie(
                post_form(
                    "/boards/engineering/threads",
                    &form(&[("title", "Forwarded"), ("body", "Secure cookie")]),
                ),
                "mchan_anon=existing-forwarded",
            ),
            "x-forwarded-proto",
            "https",
        ),
    )
    .await;

    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    let cookie = response
        .headers()
        .get(SET_COOKIE)
        .unwrap()
        .to_str()
        .unwrap();
    assert!(cookie.contains("mchan_anon=existing-forwarded"));
    assert!(cookie.contains("Secure"));
    assert!(cookie.contains("HttpOnly"));
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn existing_cookie_is_reissued_secure_for_cloudflare_https(pool: sqlx::SqlitePool) {
    let app = test_router(pool);
    let response = send(
        &app,
        with_header(
            with_cookie(
                post_form(
                    "/threads/1/replies",
                    &form(&[("body", "Cloudflare secure cookie")]),
                ),
                "mchan_anon=existing-cloudflare",
            ),
            "cf-visitor",
            r#"{"scheme":"https"}"#,
        ),
    )
    .await;

    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    let cookie = response
        .headers()
        .get(SET_COOKIE)
        .unwrap()
        .to_str()
        .unwrap();
    assert!(cookie.contains("mchan_anon=existing-cloudflare"));
    assert!(cookie.contains("Secure"));
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn cookie_security_negatives_use_request_scheme(pool: sqlx::SqlitePool) {
    let response = send(
        &test_router(pool.clone()),
        post_form(
            "/boards/engineering/threads",
            &form(&[("title", "No proxy"), ("body", "No proxy cookie")]),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    let cookie = response
        .headers()
        .get(SET_COOKIE)
        .expect("new anonymous cookie is issued")
        .to_str()
        .unwrap();
    assert!(cookie.contains("mchan_anon="));
    assert!(!cookie.contains("Secure"));

    let response = send(
        &test_router(pool.clone()),
        with_header(
            with_cookie(
                post_form(
                    "/boards/engineering/threads",
                    &form(&[("title", "Forwarded HTTP"), ("body", "Existing cookie")]),
                ),
                "mchan_anon=existing-http",
            ),
            "x-forwarded-proto",
            "http",
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert!(response.headers().get(SET_COOKIE).is_none());

    let response = send(
        &test_router(pool),
        with_header(
            post_form(
                "/boards/engineering/threads",
                &form(&[("title", "Malformed visitor"), ("body", "New cookie")]),
            ),
            "cf-visitor",
            "malformed",
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    let cookie = response
        .headers()
        .get(SET_COOKIE)
        .expect("new anonymous cookie is issued")
        .to_str()
        .unwrap();
    assert!(cookie.contains("mchan_anon="));
    assert!(!cookie.contains("Secure"));
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

    let long_body = "b".repeat(10_001);
    let response = send(
        &app,
        post_form("/threads/1/replies", &form(&[("body", &long_body)])),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
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
