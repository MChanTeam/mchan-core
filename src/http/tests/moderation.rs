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
    assert!(queue_html.contains(">Hide</button>"));
    assert!(queue_html.contains(">Lock thread</button>"));
    assert!(!queue_html.contains(">Remove</button>"));
    assert!(!queue_html.contains(">Quarantine</button>"));
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn admin_landing_requires_allowlist_and_links_to_staff_tools(pool: SqlitePool) {
    let app = moderator_router(pool);

    let missing_identity = send(&app, get_request("/admin")).await;
    assert_eq!(missing_identity.status(), StatusCode::FORBIDDEN);

    let unallowlisted = send(
        &app,
        with_header(
            get_request("/admin"),
            "cf-access-authenticated-user-email",
            "other@example.com",
        ),
    )
    .await;
    assert_eq!(unallowlisted.status(), StatusCode::FORBIDDEN);

    let admin = send(
        &app,
        with_header(
            get_request("/admin"),
            "cf-access-authenticated-user-email",
            "MODERATOR@example.com",
        ),
    )
    .await;
    assert_eq!(admin.status(), StatusCode::OK);
    let admin_body = response_text(admin).await;
    assert!(admin_body.contains(r#"href="/admin/boards""#));
    assert!(admin_body.contains(r#"href="/mod/reports""#));
    assert!(admin_body.contains(r#"href="/mod/abuse-logs""#));
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn moderator_dismiss_and_resolve_remove_reports_from_pending_queue(pool: SqlitePool) {
    let thread_id = fixture_thread_id(&pool, "Welcome to Engineering").await;
    let reply_id = fixture_reply_id(&pool, "Glad to be here.").await;
    let dismiss_report_id =
        insert_report(&pool, "thread_id", thread_id, "dismiss-regression", None).await;
    let resolve_report_id =
        insert_report(&pool, "reply_id", reply_id, "resolve-regression", None).await;
    let app = moderator_router(pool.clone());

    let initial_queue = send(
        &app,
        with_header(
            get_request("/mod/reports"),
            "cf-access-authenticated-user-email",
            MODERATOR_EMAIL,
        ),
    )
    .await;
    assert_eq!(initial_queue.status(), StatusCode::OK);
    let initial_queue_body = response_text(initial_queue).await;
    assert!(initial_queue_body.contains(&format!("Report #{dismiss_report_id}")));
    assert!(initial_queue_body.contains(&format!("Report #{resolve_report_id}")));
    assert!(initial_queue_body.contains("dismiss-regression"));
    assert!(initial_queue_body.contains("resolve-regression"));

    let dismissed = send(
        &app,
        with_header(
            post_form(&format!("/mod/reports/{dismiss_report_id}/dismiss"), ""),
            "cf-access-authenticated-user-email",
            MODERATOR_EMAIL,
        ),
    )
    .await;
    assert_eq!(dismissed.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        dismissed
            .headers()
            .get("location")
            .and_then(|value| value.to_str().ok()),
        Some("/mod/reports")
    );
    let dismiss_status = sqlx::query_scalar::<_, String>("SELECT status FROM reports WHERE id = ?")
        .bind(dismiss_report_id as i64)
        .fetch_one(&pool)
        .await
        .expect("dismissed report status is readable");
    assert_eq!(dismiss_status, "dismissed");

    let queue_after_dismiss = send(
        &app,
        with_header(
            get_request("/mod/reports"),
            "cf-access-authenticated-user-email",
            MODERATOR_EMAIL,
        ),
    )
    .await;
    assert_eq!(queue_after_dismiss.status(), StatusCode::OK);
    let queue_after_dismiss_body = response_text(queue_after_dismiss).await;
    assert!(!queue_after_dismiss_body.contains(&format!("Report #{dismiss_report_id}")));
    assert!(!queue_after_dismiss_body.contains("dismiss-regression"));
    assert!(queue_after_dismiss_body.contains(&format!("Report #{resolve_report_id}")));
    assert!(queue_after_dismiss_body.contains("resolve-regression"));

    let resolved = send(
        &app,
        with_header(
            post_form(&format!("/mod/reports/{resolve_report_id}/resolve"), ""),
            "cf-access-authenticated-user-email",
            MODERATOR_EMAIL,
        ),
    )
    .await;
    assert_eq!(resolved.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        resolved
            .headers()
            .get("location")
            .and_then(|value| value.to_str().ok()),
        Some("/mod/reports")
    );
    let resolve_status = sqlx::query_scalar::<_, String>("SELECT status FROM reports WHERE id = ?")
        .bind(resolve_report_id as i64)
        .fetch_one(&pool)
        .await
        .expect("resolved report status is readable");
    assert_eq!(resolve_status, "resolved");

    let queue_after_resolve = send(
        &app,
        with_header(
            get_request("/mod/reports"),
            "cf-access-authenticated-user-email",
            MODERATOR_EMAIL,
        ),
    )
    .await;
    assert_eq!(queue_after_resolve.status(), StatusCode::OK);
    let queue_after_resolve_body = response_text(queue_after_resolve).await;
    assert!(!queue_after_resolve_body.contains(&format!("Report #{dismiss_report_id}")));
    assert!(!queue_after_resolve_body.contains(&format!("Report #{resolve_report_id}")));
    assert!(!queue_after_resolve_body.contains("dismiss-regression"));
    assert!(!queue_after_resolve_body.contains("resolve-regression"));
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

#[sqlx::test(migrator = "MIGRATOR")]
async fn report_details_are_stored_trimmed_and_shown_in_the_queue(pool: SqlitePool) {
    let app = test_router(pool.clone());
    let thread_id = fixture_thread_id(&pool, "Welcome to Engineering").await;
    let reply_id = fixture_reply_id(&pool, "Glad to be here.").await;

    let with_message = send(
        &app,
        with_header(
            post_form(
                &format!("/threads/{thread_id}/report"),
                "reason=spam&details=%20%20this%20is%20a%20bot%20advert%20%20",
            ),
            "cf-connecting-ip",
            "198.51.100.20",
        ),
    )
    .await;
    assert_eq!(with_message.status(), StatusCode::SEE_OTHER);

    let stored = sqlx::query_scalar::<_, Option<String>>(
        "SELECT details FROM reports WHERE thread_id = ? ORDER BY id DESC LIMIT 1",
    )
    .bind(thread_id as i64)
    .fetch_one(&pool)
    .await
    .expect("thread report is readable");
    assert_eq!(stored.as_deref(), Some("this is a bot advert"));

    let blank_message = send(
        &app,
        with_header(
            post_form(
                &format!("/replies/{reply_id}/report"),
                "reason=other&details=%20%20%20",
            ),
            "cf-connecting-ip",
            "198.51.100.21",
        ),
    )
    .await;
    assert_eq!(blank_message.status(), StatusCode::SEE_OTHER);

    let blank_stored = sqlx::query_scalar::<_, Option<String>>(
        "SELECT details FROM reports WHERE reply_id = ? ORDER BY id DESC LIMIT 1",
    )
    .bind(reply_id as i64)
    .fetch_one(&pool)
    .await
    .expect("reply report is readable");
    assert_eq!(blank_stored, None);

    let too_long = format!("reason=spam&details={}", "a".repeat(401));
    let rejected = send(
        &app,
        with_header(
            post_form(&format!("/threads/{thread_id}/report"), &too_long),
            "cf-connecting-ip",
            "198.51.100.22",
        ),
    )
    .await;
    assert_eq!(rejected.status(), StatusCode::BAD_REQUEST);
    assert!(
        response_text(rejected)
            .await
            .contains("Report message cannot be longer than 400 characters.")
    );

    let reports_before_rejection =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM reports WHERE thread_id = ?")
            .bind(thread_id as i64)
            .fetch_one(&pool)
            .await
            .expect("report count is readable");
    assert_eq!(reports_before_rejection, 1);

    let queue = send(
        &moderator_router(pool.clone()),
        with_header(
            get_request("/mod/reports"),
            "cf-access-authenticated-user-email",
            MODERATOR_EMAIL,
        ),
    )
    .await;
    assert_eq!(queue.status(), StatusCode::OK);
    let queue_body = response_text(queue).await;
    assert!(queue_body.contains("Reporter said:"));
    assert!(queue_body.contains("this is a bot advert"));
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn moderator_can_create_approve_view_archive_and_restore_board(pool: SqlitePool) {
    let app = moderator_router(pool.clone());
    let slug = "moderated-board";
    let created = send(
        &app,
        with_header(
            post_form(
                "/admin/boards",
                "slug=moderated-board&name=Moderated+Board&description=Board+for+moderation",
            ),
            "cf-access-authenticated-user-email",
            MODERATOR_EMAIL,
        ),
    )
    .await;
    assert_eq!(created.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        created
            .headers()
            .get("location")
            .and_then(|value| value.to_str().ok()),
        Some("/admin/boards")
    );

    let board = sqlx::query_as::<_, (String, String, String, String)>(
        "SELECT slug, name, description, status FROM boards WHERE slug = ?",
    )
    .bind(slug)
    .fetch_one(&pool)
    .await
    .expect("created board is readable");
    assert_eq!(
        board,
        (
            String::from(slug),
            String::from("Moderated Board"),
            String::from("Board for moderation"),
            String::from("approved"),
        )
    );

    let admin_page = send(
        &app,
        with_header(
            get_request("/admin/boards"),
            "cf-access-authenticated-user-email",
            MODERATOR_EMAIL,
        ),
    )
    .await;
    assert_eq!(admin_page.status(), StatusCode::OK);
    assert!(response_text(admin_page).await.contains(slug));

    let public_page = send(&app, get_request("/boards/moderated-board")).await;
    assert_eq!(public_page.status(), StatusCode::OK);

    let archived = send(
        &app,
        with_header(
            post_form("/admin/boards/moderated-board/archive", ""),
            "cf-access-authenticated-user-email",
            MODERATOR_EMAIL,
        ),
    )
    .await;
    assert_eq!(archived.status(), StatusCode::SEE_OTHER);
    let archived_status =
        sqlx::query_scalar::<_, String>("SELECT status FROM boards WHERE slug = ?")
            .bind(slug)
            .fetch_one(&pool)
            .await
            .expect("archived board status is readable");
    assert_eq!(archived_status, "archived");

    let restored = send(
        &app,
        with_header(
            post_form("/admin/boards/moderated-board/restore", ""),
            "cf-access-authenticated-user-email",
            MODERATOR_EMAIL,
        ),
    )
    .await;
    assert_eq!(restored.status(), StatusCode::SEE_OTHER);
    let restored_status =
        sqlx::query_scalar::<_, String>("SELECT status FROM boards WHERE slug = ?")
            .bind(slug)
            .fetch_one(&pool)
            .await
            .expect("restored board status is readable");
    assert_eq!(restored_status, "approved");
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn board_creation_requires_moderator_and_rejects_invalid_or_duplicate_slug(pool: SqlitePool) {
    let app = moderator_router(pool.clone());
    let initial_count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM boards")
        .fetch_one(&pool)
        .await
        .expect("initial board count is readable");

    let missing_moderator = send(
        &app,
        post_form(
            "/admin/boards",
            "slug=unauthorized-board&name=Unauthorized&description=Nope",
        ),
    )
    .await;
    assert_eq!(missing_moderator.status(), StatusCode::FORBIDDEN);

    let invalid = send(
        &app,
        with_header(
            post_form(
                "/admin/boards",
                "slug=Not_Valid&name=Invalid&description=Rejected",
            ),
            "cf-access-authenticated-user-email",
            MODERATOR_EMAIL,
        ),
    )
    .await;
    assert_eq!(invalid.status(), StatusCode::BAD_REQUEST);

    let duplicate = send(
        &app,
        with_header(
            post_form(
                "/admin/boards",
                "slug=engineering&name=Duplicate&description=Rejected",
            ),
            "cf-access-authenticated-user-email",
            MODERATOR_EMAIL,
        ),
    )
    .await;
    assert_eq!(duplicate.status(), StatusCode::CONFLICT);

    let final_count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM boards")
        .fetch_one(&pool)
        .await
        .expect("final board count is readable");
    assert_eq!(final_count, initial_count);
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn moderator_direct_hide_thread_without_report_persists_reason_and_note(pool: SqlitePool) {
    let thread_id = fixture_thread_id(&pool, "Welcome to Engineering").await;
    let app = moderator_router(pool.clone());
    let report_count =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM reports WHERE thread_id = ?")
            .bind(thread_id as i64)
            .fetch_one(&pool)
            .await
            .expect("thread report count is readable");
    assert_eq!(report_count, 0);

    let response = send(
        &app,
        with_header(
            post_form(
                &format!("/mod/threads/{thread_id}/hide"),
                "reason=harassment&note=Exact+direct+moderation+note",
            ),
            "cf-access-authenticated-user-email",
            MODERATOR_EMAIL,
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    let expected_location = format!("/threads/{thread_id}");
    assert_eq!(
        response
            .headers()
            .get("location")
            .and_then(|value| value.to_str().ok()),
        Some(expected_location.as_str())
    );

    let status = sqlx::query_scalar::<_, String>("SELECT status FROM threads WHERE id = ?")
        .bind(thread_id as i64)
        .fetch_one(&pool)
        .await
        .expect("hidden thread status is readable");
    assert_eq!(status, "hidden");
    let audit = sqlx::query_as::<_, (String, String, i64, String, Option<String>)>(
        "SELECT moderator_email, target_kind, target_id, reason, note FROM direct_moderation_actions WHERE target_kind = 'thread' AND target_id = ?",
    )
    .bind(thread_id as i64)
    .fetch_one(&pool)
    .await
    .expect("direct moderation audit is readable");
    assert_eq!(
        audit,
        (
            String::from(MODERATOR_EMAIL),
            String::from("thread"),
            thread_id as i64,
            String::from("harassment"),
            Some(String::from("Exact direct moderation note")),
        )
    );
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn unauthorized_direct_hide_thread_leaves_status_and_audit_unchanged(pool: SqlitePool) {
    let thread_id = fixture_thread_id(&pool, "Welcome to Engineering").await;
    let app = moderator_router(pool.clone());
    let response = send(
        &app,
        post_form(
            &format!("/mod/threads/{thread_id}/hide"),
            "reason=harassment&note=should+not+persist",
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);

    let status = sqlx::query_scalar::<_, String>("SELECT status FROM threads WHERE id = ?")
        .bind(thread_id as i64)
        .fetch_one(&pool)
        .await
        .expect("thread status is readable");
    let audit_count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM direct_moderation_actions WHERE target_kind = 'thread' AND target_id = ?",
    )
    .bind(thread_id as i64)
    .fetch_one(&pool)
    .await
    .expect("direct moderation audit count is readable");
    assert_eq!(status, "visible");
    assert_eq!(audit_count, 0);
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn moderator_session_cookie_cannot_authorize_direct_hide(pool: SqlitePool) {
    let thread_id = fixture_thread_id(&pool, "Welcome to Engineering").await;
    let app = moderator_router(pool.clone());

    let bootstrap = send(
        &app,
        with_header(
            get_request("/authenticate"),
            "cf-access-authenticated-user-email",
            MODERATOR_EMAIL,
        ),
    )
    .await;
    assert_eq!(bootstrap.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        bootstrap
            .headers()
            .get("location")
            .and_then(|value| value.to_str().ok()),
        Some("/")
    );
    let set_cookie = bootstrap
        .headers()
        .get("set-cookie")
        .and_then(|value| value.to_str().ok())
        .expect("moderator session cookie is set");
    let cookie = set_cookie
        .split(';')
        .next()
        .expect("moderator session cookie has a value");
    assert!(cookie.starts_with("__Host-mchan-moderator="));

    let mut request = post_form(
        &format!("/mod/threads/{thread_id}/hide"),
        "reason=harassment&note=session+cookie+must+not+authorize",
    );
    request.headers_mut().insert(
        HeaderName::from_static("cookie"),
        HeaderValue::from_bytes(cookie.as_bytes()).expect("moderator session cookie is valid"),
    );
    let response = send(&app, request).await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);

    let status = sqlx::query_scalar::<_, String>("SELECT status FROM threads WHERE id = ?")
        .bind(thread_id as i64)
        .fetch_one(&pool)
        .await
        .expect("thread status is readable");
    let audit_count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM direct_moderation_actions WHERE target_kind = 'thread' AND target_id = ?",
    )
    .bind(thread_id as i64)
    .fetch_one(&pool)
    .await
    .expect("direct moderation audit count is readable");
    assert_eq!(status, "visible");
    assert_eq!(audit_count, 0);
}
