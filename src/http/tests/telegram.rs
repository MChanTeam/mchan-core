use super::*;
use crate::http::telegram;
use axum::http::{StatusCode, header::AUTHORIZATION};
use sha2::{Digest, Sha256};

static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!();

const TG_TOKEN: &str = "internal-test-bearer-token";

fn tg_dependencies(pool: SqlitePool, token: Option<&str>) -> HttpDependencies {
    test_dependencies(pool, HashSet::new(), None, None)
        .with_telegram_service_token(token.map(str::to_owned))
}

fn tg_router(pool: SqlitePool) -> Router {
    telegram::telegram_router(Arc::new(tg_dependencies(pool, Some(TG_TOKEN))))
}

fn json_request(uri: &str, value: serde_json::Value) -> Request<Body> {
    Request::builder()
        .method(Method::POST)
        .uri(uri)
        .header(CONTENT_TYPE, "application/json")
        .body(Body::from(value.to_string()))
        .expect("valid JSON request")
}

fn bearer(request: Request<Body>, token: &str) -> Request<Body> {
    let (mut parts, body) = request.into_parts();
    parts.headers.insert(
        AUTHORIZATION,
        HeaderValue::from_str(&format!("Bearer {token}")).expect("valid bearer header"),
    );
    Request::from_parts(parts, body)
}

fn thread_create_body(principal: &str, key: &str, title: &str, body: &str) -> serde_json::Value {
    serde_json::json!({
        "principal": principal,
        "idempotency_key": key,
        "title": title,
        "body": body,
    })
}

fn raw_json_request(uri: &str, body: String) -> Request<Body> {
    Request::builder()
        .method(Method::POST)
        .uri(uri)
        .header(CONTENT_TYPE, "application/json")
        .body(Body::from(body))
        .expect("valid raw JSON request")
}

fn reply_create_body(principal: &str, key: &str, body: &str) -> serde_json::Value {
    serde_json::json!({
        "principal": principal,
        "idempotency_key": key,
        "body": body,
    })
}

fn report_body(
    principal: &str,
    key: &str,
    reason: &str,
    details: Option<&str>,
) -> serde_json::Value {
    serde_json::json!({
        "principal": principal,
        "idempotency_key": key,
        "reason": reason,
        "details": details,
    })
}

async fn response_json(response: Response<Body>) -> serde_json::Value {
    let text = response_text(response).await;
    serde_json::from_str(&text).unwrap_or_else(|error| panic!("response is JSON: {text}: {error}"))
}

fn assert_telegram_no_store_headers(response: &Response<Body>) {
    let headers = response.headers();
    assert_eq!(
        headers
            .get(axum::http::header::CACHE_CONTROL)
            .and_then(|value| value.to_str().ok()),
        Some("no-store, private")
    );
    assert_eq!(
        headers
            .get_all(axum::http::header::CACHE_CONTROL)
            .iter()
            .count(),
        1
    );
    assert_eq!(
        headers
            .get(axum::http::header::PRAGMA)
            .and_then(|value| value.to_str().ok()),
        Some("no-cache")
    );
    assert_eq!(
        headers.get_all(axum::http::header::PRAGMA).iter().count(),
        1
    );
}

async fn assert_telegram_error(response: Response<Body>, status: StatusCode, message: &str) {
    assert_eq!(response.status(), status);
    let body = response_json(response).await;
    assert_eq!(body["status"], "error");
    let error = body["error"].as_str().expect("error message string");
    assert!(
        error.contains(message),
        "expected error containing {message:?}, got {error:?}"
    );
}

async fn count(pool: &SqlitePool, sql: &str) -> i64 {
    sqlx::query_scalar::<_, i64>(sql)
        .fetch_one(pool)
        .await
        .expect("aggregate query")
}
async fn seed_mismatched_idempotency(
    pool: &SqlitePool,
    operation: &str,
    key: &str,
    result_id: Option<i64>,
) {
    sqlx::query(
        "INSERT INTO machine_idempotency \
         (service, operation, opaque_key, request_hash, result_id) \
         VALUES ('telegram', ?, ?, ?, ?)",
    )
    .bind(operation)
    .bind(key)
    .bind(vec![0_u8; 32])
    .bind(result_id)
    .execute(pool)
    .await
    .expect("seed idempotency row");
}

async fn mark_idempotency_committed(pool: &SqlitePool, operation: &str, key: &str) {
    sqlx::query(
        "UPDATE machine_idempotency SET result_id = 1 \
         WHERE service = 'telegram' AND operation = ? AND opaque_key = ?",
    )
    .bind(operation)
    .bind(key)
    .execute(pool)
    .await
    .expect("mark idempotency row committed");
}

// --- utility helpers ---
async fn outbox_rows(pool: &SqlitePool) -> Vec<(String, Option<i64>, Option<i64>, Option<i64>)> {
    sqlx::query_as("SELECT kind, thread_id, reply_id, report_id FROM projection_outbox ORDER BY id")
        .fetch_all(pool)
        .await
        .expect("outbox rows")
}

fn collect_ids(value: &serde_json::Value) -> Vec<u64> {
    value
        .as_array()
        .expect("id array")
        .iter()
        .map(|entry| {
            entry["id"]
                .as_u64()
                .or_else(|| entry.as_u64())
                .expect("numeric id")
        })
        .collect()
}

fn telegram_origin_key(principal: &str) -> String {
    format!("\0mchan-client:telegram:\0{}", principal)
}

fn expected_poster_id(cipher: &abuse::AbuseCipher, origin_key: &str, thread_id: u64) -> String {
    let fingerprint = cipher.fingerprint(origin_key);
    let mut digest = Sha256::new();
    digest.update(fingerprint);
    digest.update(thread_id.to_be_bytes());
    let hash = digest.finalize();
    format!(
        "Anonymous ##{:02x}{:02x}{:02x}{:02x}",
        hash[0], hash[1], hash[2], hash[3]
    )
}

async fn fixture_thread(
    pool: &SqlitePool,
    slug: &str,
    title: &str,
    status: &str,
    pinned: bool,
    age_minutes: i64,
    archived: bool,
) -> u64 {
    let sql = format!(
        r#"
        INSERT INTO threads (
            board_id, title, body, status, poster_id, created_at, bumped_at, is_pinned, archived_at
        )
        VALUES (
            (SELECT id FROM boards WHERE slug = '{slug}'),
            '{title}', 'Fixture body for {title}', '{status}', 'Anonymous',
            datetime('now', '-{age_minutes} minutes'),
            datetime('now', '-{age_minutes} minutes'),
            {pinned}, {archived_clause}
        )
        RETURNING id
        "#,
        archived_clause = if archived {
            "datetime('now', '-1 minute')"
        } else {
            "NULL"
        },
    );
    sqlx::query_scalar::<_, i64>(&sql)
        .fetch_one(pool)
        .await
        .map(|id| id as u64)
        .expect("fixture thread inserts")
}

async fn fixture_reply(
    pool: &SqlitePool,
    thread_id: u64,
    marker: &str,
    status: &str,
    age_minutes: i64,
) -> u64 {
    let sql = format!(
        r#"
        INSERT INTO replies (thread_id, body, poster_id, status, created_at)
        VALUES ({thread_id}, 'Fixture reply {marker}', 'Anonymous', '{status}',
                datetime('now', '-{age_minutes} minutes'))
        RETURNING id
        "#
    );

    sqlx::query_scalar::<_, i64>(&sql)
        .fetch_one(pool)
        .await
        .map(|id| id as u64)
        .expect("fixture reply inserts")
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn telegram_no_store_headers_cover_handler_and_router_rejections(pool: SqlitePool) {
    let app = tg_router(pool);

    let requests = [
        (
            "handler",
            bearer(
                get_request("/internal/telegram/boards/pasum/threads"),
                TG_TOKEN,
            ),
            StatusCode::OK,
        ),
        (
            "duplicate scalar query",
            bearer(
                get_request("/internal/telegram/threads/1?limit=1&limit=2"),
                TG_TOKEN,
            ),
            StatusCode::BAD_REQUEST,
        ),
        (
            "malformed percent encoding",
            bearer(get_request("/internal/telegram/threads/%C3%28"), TG_TOKEN),
            StatusCode::BAD_REQUEST,
        ),
        (
            "authenticated unknown route",
            bearer(get_request("/internal/telegram/not-a-route"), TG_TOKEN),
            StatusCode::NOT_FOUND,
        ),
        (
            "authenticated wrong method",
            bearer(
                Request::builder()
                    .method(Method::POST)
                    .uri("/internal/telegram/threads/1")
                    .body(Body::empty())
                    .unwrap(),
                TG_TOKEN,
            ),
            StatusCode::METHOD_NOT_ALLOWED,
        ),
    ];

    for (label, request, status) in requests {
        let response = send(&app, request).await;
        assert_eq!(response.status(), status, "{label}");
        assert_telegram_no_store_headers(&response);
    }
}

// --- router boundary and public absence ---

#[sqlx::test(migrator = "MIGRATOR")]
async fn internal_telegram_endpoints_are_absent_from_public_router(pool: SqlitePool) {
    let app = test_router(pool);

    let backfill = send(&app, get_request("/internal/telegram/boards/pasum/threads")).await;
    assert_eq!(backfill.status(), StatusCode::NOT_FOUND);

    let snapshot = send(&app, get_request("/internal/telegram/threads/1")).await;
    assert_eq!(snapshot.status(), StatusCode::NOT_FOUND);

    let lease = send(
        &app,
        json_request("/internal/telegram/outbox/lease", serde_json::json!({})),
    )
    .await;
    assert_eq!(lease.status(), StatusCode::NOT_FOUND);
}

// --- fail-closed token ---

#[sqlx::test(migrator = "MIGRATOR")]
async fn unset_token_fail_closed_every_endpoint_even_with_credentials(pool: SqlitePool) {
    let app = telegram::telegram_router(Arc::new(tg_dependencies(pool.clone(), None)));

    let routes = [
        ("/internal/telegram/boards/pasum/threads", Method::GET, None),
        (
            "/internal/telegram/boards/engineering/threads",
            Method::POST,
            Some(thread_create_body("p", "k", "t", "b")),
        ),
        ("/internal/telegram/threads/1", Method::GET, None),
        (
            "/internal/telegram/outbox/lease",
            Method::POST,
            Some(serde_json::json!({})),
        ),
        (
            "/internal/telegram/outbox/ack",
            Method::POST,
            Some(serde_json::json!({"event_id": 1, "lease_token": "t"})),
        ),
    ];

    for (uri, method, body) in routes {
        let request = match (method, body) {
            (Method::GET, _) => get_request(uri),
            (_, Some(body)) => json_request(uri, body),
            (_, None) => unreachable!("POST routes always carry a body here"),
        };
        // Even a correct-looking credential must not unlock a disabled listener.
        let authorized = bearer(request, TG_TOKEN);
        let response = send(&app, authorized).await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND, "{uri}");
        let cache_control = response
            .headers()
            .get(axum::http::header::CACHE_CONTROL)
            .and_then(|value| value.to_str().ok())
            .expect("no-store header present");
        assert_eq!(cache_control, "no-store, private");
        let body = response_json(response).await;
        assert_eq!(body["error"], "endpoint disabled");
    }
    assert_eq!(
        count(&pool, "SELECT COUNT(*) FROM projection_outbox").await,
        0
    );
}

// --- wrong/correct auth ---

#[sqlx::test(migrator = "MIGRATOR")]
async fn bearer_auth_rejects_missing_wrong_and_accepts_correct_token(pool: SqlitePool) {
    let app = tg_router(pool);
    let uri = "/internal/telegram/boards/pasum/threads";

    let missing = send(&app, get_request(uri)).await;
    assert_eq!(missing.status(), StatusCode::UNAUTHORIZED);

    let wrong_scheme = send(
        &app,
        with_header(
            get_request(uri),
            "authorization",
            "Basic internal-test-bearer-token",
        ),
    )
    .await;
    assert_eq!(wrong_scheme.status(), StatusCode::UNAUTHORIZED);

    let wrong_token = send(&app, bearer(get_request(uri), "not-the-token")).await;
    assert_eq!(wrong_token.status(), StatusCode::UNAUTHORIZED);
    let body = response_json(wrong_token).await;
    assert_eq!(body["status"], "error");
    assert_eq!(body["error"], "unauthorized");

    let correct = send(&app, bearer(get_request(uri), TG_TOKEN)).await;
    assert_eq!(correct.status(), StatusCode::OK);
    let cache_control = correct
        .headers()
        .get(axum::http::header::CACHE_CONTROL)
        .and_then(|value| value.to_str().ok())
        .expect("no-store header present");
    assert_eq!(cache_control, "no-store, private");
    let pragma = correct
        .headers()
        .get(axum::http::header::PRAGMA)
        .and_then(|value| value.to_str().ok())
        .expect("pragma header present");
    assert_eq!(pragma, "no-cache");
    let body = response_json(correct).await;
    assert_eq!(body["thread_ids"], serde_json::json!([]));
}

// --- constant-time comparison helper ---

#[test]
fn constant_time_helpers_match_exact_bytes_only() {
    assert!(constant_time_equal(b"", b""));
    assert!(constant_time_equal(b"token", b"token"));
    assert!(!constant_time_equal(b"token", b"tokeN"));
    assert!(!constant_time_equal(b"tok", b"token"));
    assert!(!constant_time_equal(b"token", b"tok"));

    let headers_with = |value: &'static str| {
        let mut headers = axum::http::HeaderMap::new();
        headers.insert(AUTHORIZATION, HeaderValue::from_static(value));
        headers
    };

    assert!(bearer_matches(&headers_with("Bearer secret"), "secret"));
    assert!(!bearer_matches(&headers_with("Bearer secreT"), "secret"));
    assert!(!bearer_matches(&headers_with("Bearer sec"), "secret"));
    assert!(!bearer_matches(&headers_with("bearer secret"), "secret"));
    assert!(!bearer_matches(&headers_with("Token secret"), "secret"));
    assert!(!bearer_matches(&headers_with("Bearer"), "secret"));
    assert!(!bearer_matches(&axum::http::HeaderMap::new(), "secret"));
}

// --- successful thread creation and idempotent replay ---

#[sqlx::test(migrator = "MIGRATOR")]
async fn thread_create_success_and_replay_returns_original_conflicts_on_new_hash(pool: SqlitePool) {
    let app = tg_router(pool.clone());
    let uri = "/internal/telegram/boards/engineering/threads";
    let payload = thread_create_body(
        "idem-principal",
        "idem-key-1",
        "Idempotent",
        "original body",
    );

    let first = send(&app, bearer(json_request(uri, payload.clone()), TG_TOKEN)).await;
    assert_eq!(first.status(), StatusCode::CREATED);
    let created = response_json(first).await;
    let created_id = created["thread_id"].as_u64().expect("thread id");
    assert_eq!(created["id"], serde_json::json!(created_id));

    let stored = sqlx::query_as::<_, (String, String, String)>(
        "SELECT title, body, poster_id FROM threads WHERE id = ?",
    )
    .bind(created_id as i64)
    .fetch_one(&pool)
    .await
    .expect("stored thread");
    assert_eq!(stored.0, "Idempotent");
    assert_eq!(stored.1, "original body");
    assert!(
        stored.2.starts_with("Anonymous ##"),
        "machine threads get poster ids"
    );

    let replay = send(&app, bearer(json_request(uri, payload), TG_TOKEN)).await;
    assert_eq!(replay.status(), StatusCode::OK);
    assert_eq!(
        response_json(replay).await["thread_id"],
        serde_json::json!(created_id)
    );

    let mutated = send(
        &app,
        bearer(
            json_request(
                uri,
                thread_create_body(
                    "idem-principal",
                    "idem-key-1",
                    "Idempotent",
                    "different body",
                ),
            ),
            TG_TOKEN,
        ),
    )
    .await;
    assert_telegram_error(mutated, StatusCode::CONFLICT, "idempotency conflict").await;

    // A different opaque key with identical content is a genuinely new object.
    let fresh = send(
        &app,
        bearer(
            json_request(
                uri,
                thread_create_body(
                    "idem-principal",
                    "idem-key-2",
                    "Idempotent",
                    "original body",
                ),
            ),
            TG_TOKEN,
        ),
    )
    .await;
    assert_eq!(fresh.status(), StatusCode::CREATED);
    assert_ne!(
        response_json(fresh).await["thread_id"],
        serde_json::json!(created_id)
    );

    let events = outbox_rows(&pool).await;
    assert_eq!(
        events.len(),
        2,
        "only real creations emit thread_created events"
    );
    assert!(events.iter().all(|(kind, ..)| kind == "thread_created"));

    let stored =
        sqlx::query_as::<_, (String, String)>("SELECT title, body FROM threads WHERE id = ?")
            .bind(created_id as i64)
            .fetch_one(&pool)
            .await
            .expect("stored thread");
    assert_eq!(stored, ("Idempotent".into(), "original body".into()));

    // Conflicts are detected before a claim row is written; two keys exist.
    assert_eq!(
        count(&pool, "SELECT COUNT(*) FROM machine_idempotency").await,
        2
    );
}

// --- machine identity namespace ---

#[sqlx::test(migrator = "MIGRATOR")]
async fn machine_identity_is_namespaced_stable_thread_scoped_and_never_leaks(pool: SqlitePool) {
    let cipher = abuse::AbuseCipher::from_hex(TEST_ABUSE_KEY).expect("valid test abuse key");
    let app = tg_router(pool.clone());
    let uri = "/internal/telegram/boards/engineering/threads";

    let first = send(
        &app,
        bearer(
            json_request(
                uri,
                thread_create_body("principal-A", "key-1", "Identity one", "body"),
            ),
            TG_TOKEN,
        ),
    )
    .await;
    assert_eq!(first.status(), StatusCode::CREATED);
    let thread_a = response_json(first).await["thread_id"]
        .as_u64()
        .expect("thread id");

    // Second thread by the same principal consumes the last thread-rate slot.
    let second = send(
        &app,
        bearer(
            json_request(
                uri,
                thread_create_body("principal-A", "key-2", "Identity two", "body"),
            ),
            TG_TOKEN,
        ),
    )
    .await;
    assert_eq!(second.status(), StatusCode::CREATED);
    let thread_b = response_json(second).await["thread_id"]
        .as_u64()
        .expect("thread id");
    assert_ne!(thread_a, thread_b);

    let reply = send(
        &app,
        bearer(
            json_request(
                &format!("/internal/telegram/threads/{thread_a}/replies"),
                reply_create_body("principal-A", "key-3", "identity reply"),
            ),
            TG_TOKEN,
        ),
    )
    .await;
    assert_eq!(reply.status(), StatusCode::CREATED);

    let namespaced_a = telegram_origin_key("principal-A");
    let namespaced_b = telegram_origin_key("principal-B");

    // Namespace transform is applied before fingerprinting: a bare principal
    // can never collide with the machine namespace.
    assert_ne!(
        cipher.fingerprint("principal-A"),
        cipher.fingerprint(&namespaced_a),
    );

    let poster_a = sqlx::query_scalar::<_, String>("SELECT poster_id FROM threads WHERE id = ?")
        .bind(thread_a as i64)
        .fetch_one(&pool)
        .await
        .expect("thread A poster id");
    let poster_b = sqlx::query_scalar::<_, String>("SELECT poster_id FROM threads WHERE id = ?")
        .bind(thread_b as i64)
        .fetch_one(&pool)
        .await
        .expect("thread B poster id");
    let reply_poster =
        sqlx::query_scalar::<_, String>("SELECT poster_id FROM replies WHERE thread_id = ?")
            .bind(thread_a as i64)
            .fetch_one(&pool)
            .await
            .expect("reply poster id");

    // Stable within one thread, scoped per thread.
    assert_eq!(
        poster_a,
        expected_poster_id(&cipher, &namespaced_a, thread_a)
    );
    assert_eq!(
        reply_poster, poster_a,
        "same principal is stable within one thread"
    );
    // Same fingerprint, different thread id: poster id stays thread-scoped.
    assert_eq!(
        poster_b,
        expected_poster_id(&cipher, &namespaced_a, thread_b),
        "poster id derives from the same principal namespace"
    );
    assert_ne!(poster_a, poster_b, "per-thread ids differ across threads");
    assert_ne!(
        expected_poster_id(&cipher, &namespaced_a, thread_a),
        expected_poster_id(&cipher, &namespaced_b, thread_a),
        "different principals never share a poster id"
    );
}

// --- posting validations and transport limits ---

#[sqlx::test(migrator = "MIGRATOR")]
async fn thread_create_validations_reject_bad_input_without_side_effects(pool: SqlitePool) {
    let app = tg_router(pool.clone());
    let uri = "/internal/telegram/boards/engineering/threads";
    let before = count(&pool, "SELECT COUNT(*) FROM threads").await;

    struct Case {
        name: &'static str,
        body: serde_json::Value,
        status: StatusCode,
        message: &'static str,
    }

    let long_title = "x".repeat(121);
    let long_body = "y".repeat(2_001);
    let cases = [
        Case {
            name: "empty title",
            body: thread_create_body("p-v1", "k-v1", "   ", "body"),
            status: StatusCode::BAD_REQUEST,
            message: "Thread title cannot be empty.",
        },
        Case {
            name: "oversized title",
            body: thread_create_body("p-v2", "k-v2", &long_title, "body"),
            status: StatusCode::BAD_REQUEST,
            message: "Thread title is too long",
        },
        Case {
            name: "empty body",
            body: thread_create_body("p-v3", "k-v3", "Title", ""),
            status: StatusCode::BAD_REQUEST,
            message: "Thread body cannot be empty",
        },
        Case {
            name: "oversized body",
            body: thread_create_body("p-v4", "k-v4", "Title", &long_body),
            status: StatusCode::BAD_REQUEST,
            message: "Thread body is too long",
        },
        Case {
            name: "empty principal",
            body: thread_create_body("", "k-v5", "Title", "body"),
            status: StatusCode::BAD_REQUEST,
            message: "principal must be 1..256 bytes UTF-8",
        },
        Case {
            name: "oversized principal",
            body: thread_create_body(&"p".repeat(257), "k-v6", "Title", "body"),
            status: StatusCode::BAD_REQUEST,
            message: "principal must be 1..256 bytes UTF-8",
        },
        Case {
            name: "empty idempotency key",
            body: thread_create_body("p-v7", "", "Title", "body"),
            status: StatusCode::BAD_REQUEST,
            message: "idempotency_key must be 1..512 bytes",
        },
        Case {
            name: "oversized idempotency key",
            body: thread_create_body("p-v8", &"k".repeat(513), "Title", "body"),
            status: StatusCode::BAD_REQUEST,
            message: "idempotency_key must be 1..512 bytes",
        },
    ];

    for case in cases {
        let response = send(&app, bearer(json_request(uri, case.body), TG_TOKEN)).await;
        assert_telegram_error(response, case.status, case.message).await;
    }

    // Unknown board surfaces as 404 after the canonical path rejects persistence.
    let unknown = send(
        &app,
        bearer(
            json_request(
                "/internal/telegram/boards/ghost-board/threads",
                thread_create_body("p-v9", "k-v9", "Title", "body"),
            ),
            TG_TOKEN,
        ),
    )
    .await;
    assert_telegram_error(unknown, StatusCode::NOT_FOUND, "board not found").await;

    // Unsupported content type.
    let unsupported = send(&app, bearer(post_form(uri, "title=T&body=b"), TG_TOKEN)).await;
    assert_telegram_error(
        unsupported,
        StatusCode::BAD_REQUEST,
        "unsupported Content-Type",
    )
    .await;

    // JSON body beyond the 64 KiB machine limit.
    let oversized_payload = thread_create_body("p-v10", "k-v10", "Title", &"z".repeat(70 * 1024));
    let oversized = send(
        &app,
        bearer(
            raw_json_request(uri, oversized_payload.to_string()),
            TG_TOKEN,
        ),
    )
    .await;
    assert_telegram_error(
        oversized,
        StatusCode::PAYLOAD_TOO_LARGE,
        "JSON body too large",
    )
    .await;

    assert_eq!(
        count(&pool, "SELECT COUNT(*) FROM threads").await,
        before,
        "rejected requests must not persist anything"
    );
    assert_eq!(
        count(&pool, "SELECT COUNT(*) FROM projection_outbox").await,
        0
    );
}

// --- report validations, visibility, and replay ---

#[sqlx::test(migrator = "MIGRATOR")]
async fn report_validations_visibility_and_replay_are_enforced(pool: SqlitePool) {
    let app = tg_router(pool.clone());
    let visible_id = fixture_thread(
        &pool,
        "engineering",
        "Reportable target",
        "visible",
        false,
        30,
        false,
    )
    .await;
    let hidden_id = fixture_thread(
        &pool,
        "engineering",
        "Hidden target",
        "hidden",
        false,
        29,
        false,
    )
    .await;
    let reply_id = fixture_reply(&pool, visible_id, "report me", "visible", 5).await;
    let buried_reply_id = fixture_reply(&pool, hidden_id, "buried", "visible", 4).await;
    let uri = |id: u64| format!("/internal/telegram/threads/{id}/reports");
    let reply_uri = |id: u64| format!("/internal/telegram/replies/{id}/reports");

    // Invalid reason.
    let bad_reason = send(
        &app,
        bearer(
            json_request(
                &uri(visible_id),
                report_body("r-a", "rk-a", "not-a-reason", None),
            ),
            TG_TOKEN,
        ),
    )
    .await;
    assert_telegram_error(bad_reason, StatusCode::BAD_REQUEST, "Invalid report reason").await;

    // Details beyond the 400-character bound.
    let long_details = send(
        &app,
        bearer(
            json_request(
                &uri(visible_id),
                report_body("r-b", "rk-b", "spam", Some(&"d".repeat(401))),
            ),
            TG_TOKEN,
        ),
    )
    .await;
    assert_telegram_error(
        long_details,
        StatusCode::BAD_REQUEST,
        "Report message cannot be longer than 400 characters.",
    )
    .await;
    // Hidden thread targets stay invisible to the machine boundary.
    let hidden_target = send(
        &app,
        bearer(
            json_request(&uri(hidden_id), report_body("r-c", "rk-c", "spam", None)),
            TG_TOKEN,
        ),
    )
    .await;
    assert_telegram_error(hidden_target, StatusCode::NOT_FOUND, "thread not found").await;

    // A still-visible reply inside a hidden thread remains canonically
    // reportable (same predicate as the web path): reply visibility only.
    let buried_target = send(
        &app,
        bearer(
            json_request(
                &reply_uri(buried_reply_id),
                report_body("r-e", "rk-e", "spam", None),
            ),
            TG_TOKEN,
        ),
    )
    .await;
    assert_eq!(buried_target.status(), StatusCode::CREATED);
    let buried_report_id = response_json(buried_target).await["report_id"]
        .as_u64()
        .expect("buried report id");

    // Valid thread report creates exactly one pending report plus one event.
    let valid = send(
        &app,
        bearer(
            json_request(
                &uri(visible_id),
                report_body("r-d", "rk-d", "spam", Some("machine report details")),
            ),
            TG_TOKEN,
        ),
    )
    .await;
    assert_eq!(valid.status(), StatusCode::CREATED);
    let created = response_json(valid).await;
    let report_id = created["report_id"].as_u64().expect("report id");
    assert_eq!(created["thread_id"], serde_json::json!(visible_id));
    let status = sqlx::query_scalar::<_, String>("SELECT status FROM reports WHERE id = ?")
        .bind(report_id as i64)
        .fetch_one(&pool)
        .await
        .expect("report row");
    assert_eq!(status, "pending");

    // Exact replay returns the original report without a new event.
    let replay = send(
        &app,
        bearer(
            json_request(
                &uri(visible_id),
                report_body("r-d", "rk-d", "spam", Some("machine report details")),
            ),
            TG_TOKEN,
        ),
    )
    .await;
    assert_eq!(replay.status(), StatusCode::OK);
    assert_eq!(
        response_json(replay).await["report_id"],
        serde_json::json!(report_id)
    );

    // Same opaque key with a different hash conflicts without side effects.
    let conflict = send(
        &app,
        bearer(
            json_request(
                &uri(visible_id),
                report_body("r-d", "rk-d", "harassment", None),
            ),
            TG_TOKEN,
        ),
    )
    .await;
    assert_telegram_error(conflict, StatusCode::CONFLICT, "idempotency conflict").await;

    // Reply report carries the reply id in its event.
    let reply_valid = send(
        &app,
        bearer(
            json_request(
                &reply_uri(reply_id),
                report_body("r-f", "rk-f", "other", None),
            ),
            TG_TOKEN,
        ),
    )
    .await;
    assert_eq!(reply_valid.status(), StatusCode::CREATED);
    let reply_created = response_json(reply_valid).await;
    let reply_report_id = reply_created["report_id"].as_u64().expect("report id");
    assert_eq!(reply_created["thread_id"], serde_json::json!(visible_id));

    let events = outbox_rows(&pool).await;
    assert_eq!(
        events,
        vec![
            (
                "report_created".into(),
                Some(hidden_id as i64),
                Some(buried_reply_id as i64),
                Some(buried_report_id as i64)
            ),
            (
                "report_created".into(),
                Some(visible_id as i64),
                None,
                Some(report_id as i64)
            ),
            (
                "report_created".into(),
                Some(visible_id as i64),
                Some(reply_id as i64),
                Some(reply_report_id as i64)
            ),
        ]
    );
}

// --- atomic idempotency conflicts ---

#[sqlx::test(migrator = "MIGRATOR")]
async fn atomic_conflicts_are_409_for_all_machine_mutations_and_target_states(pool: SqlitePool) {
    let app = tg_router(pool.clone());
    let report_thread_id = fixture_thread(
        &pool,
        "engineering",
        "Atomic conflict target",
        "visible",
        false,
        30,
        false,
    )
    .await;
    let report_reply_id = fixture_reply(
        &pool,
        report_thread_id,
        "atomic conflict reply",
        "visible",
        5,
    )
    .await;

    let thread_uri = "/internal/telegram/boards/pasum/threads";
    let thread_body = thread_create_body(
        "atomic-thread-principal",
        "atomic-thread-key",
        "Atomic conflict thread",
        "body",
    );
    let reply_uri = format!("/internal/telegram/threads/{report_thread_id}/replies");
    let reply_body = reply_create_body("atomic-reply-principal", "atomic-reply-key", "body");
    let thread_report_uri = format!("/internal/telegram/threads/{report_thread_id}/reports");
    let thread_report_body = report_body(
        "atomic-thread-report-principal",
        "atomic-thread-report-key",
        "spam",
        None,
    );
    let reply_report_uri = format!("/internal/telegram/replies/{report_reply_id}/reports");
    let reply_report_body = report_body(
        "atomic-reply-report-principal",
        "atomic-reply-report-key",
        "spam",
        None,
    );

    // A pending row with a different hash passes the HTTP preflight as New,
    // then deterministically reaches each domain mutation's Conflict branch.
    seed_mismatched_idempotency(&pool, "thread.create", "atomic-thread-key", None).await;
    seed_mismatched_idempotency(&pool, "reply.create", "atomic-reply-key", None).await;
    seed_mismatched_idempotency(&pool, "thread.report", "atomic-thread-report-key", None).await;
    seed_mismatched_idempotency(&pool, "reply.report", "atomic-reply-report-key", None).await;

    let thread_conflict = send(
        &app,
        bearer(json_request(thread_uri, thread_body.clone()), TG_TOKEN),
    )
    .await;
    assert_telegram_error(
        thread_conflict,
        StatusCode::CONFLICT,
        "idempotency conflict",
    )
    .await;
    let reply_conflict = send(
        &app,
        bearer(json_request(&reply_uri, reply_body.clone()), TG_TOKEN),
    )
    .await;
    assert_telegram_error(reply_conflict, StatusCode::CONFLICT, "idempotency conflict").await;
    let thread_report_conflict = send(
        &app,
        bearer(
            json_request(&thread_report_uri, thread_report_body.clone()),
            TG_TOKEN,
        ),
    )
    .await;
    assert_telegram_error(
        thread_report_conflict,
        StatusCode::CONFLICT,
        "idempotency conflict",
    )
    .await;
    let reply_report_conflict = send(
        &app,
        bearer(
            json_request(&reply_report_uri, reply_report_body.clone()),
            TG_TOKEN,
        ),
    )
    .await;
    assert_telegram_error(
        reply_report_conflict,
        StatusCode::CONFLICT,
        "idempotency conflict",
    )
    .await;

    // Once each target has moved to a state that would normally classify it
    // differently, a committed mismatched key still returns the same 409
    // directly from the idempotency preflight.
    sqlx::query("UPDATE boards SET status = 'archived' WHERE slug = 'pasum'")
        .execute(&pool)
        .await
        .expect("archive thread-create board");
    mark_idempotency_committed(&pool, "thread.create", "atomic-thread-key").await;
    let thread_after_transition = send(
        &app,
        bearer(json_request(thread_uri, thread_body), TG_TOKEN),
    )
    .await;
    assert_telegram_error(
        thread_after_transition,
        StatusCode::CONFLICT,
        "idempotency conflict",
    )
    .await;

    sqlx::query("UPDATE threads SET status = 'locked' WHERE id = ?")
        .bind(report_thread_id as i64)
        .execute(&pool)
        .await
        .expect("lock reply target thread");
    mark_idempotency_committed(&pool, "reply.create", "atomic-reply-key").await;
    let reply_after_transition =
        send(&app, bearer(json_request(&reply_uri, reply_body), TG_TOKEN)).await;
    assert_telegram_error(
        reply_after_transition,
        StatusCode::CONFLICT,
        "idempotency conflict",
    )
    .await;

    sqlx::query("UPDATE threads SET status = 'hidden' WHERE id = ?")
        .bind(report_thread_id as i64)
        .execute(&pool)
        .await
        .expect("hide thread-report target");
    mark_idempotency_committed(&pool, "thread.report", "atomic-thread-report-key").await;
    let thread_report_after_transition = send(
        &app,
        bearer(
            json_request(&thread_report_uri, thread_report_body),
            TG_TOKEN,
        ),
    )
    .await;
    assert_telegram_error(
        thread_report_after_transition,
        StatusCode::CONFLICT,
        "idempotency conflict",
    )
    .await;

    sqlx::query("UPDATE replies SET status = 'hidden' WHERE id = ?")
        .bind(report_reply_id as i64)
        .execute(&pool)
        .await
        .expect("hide reply-report target");
    mark_idempotency_committed(&pool, "reply.report", "atomic-reply-report-key").await;
    let reply_report_after_transition = send(
        &app,
        bearer(json_request(&reply_report_uri, reply_report_body), TG_TOKEN),
    )
    .await;
    assert_telegram_error(
        reply_report_after_transition,
        StatusCode::CONFLICT,
        "idempotency conflict",
    )
    .await;
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn active_bans_block_machine_threads_and_replies_by_fingerprint(pool: SqlitePool) {
    let cipher = abuse::AbuseCipher::from_hex(TEST_ABUSE_KEY).expect("valid test abuse key");
    let app = tg_router(pool.clone());
    let thread_uri = "/internal/telegram/boards/engineering/threads";
    let banned_principal = "banned-principal";
    let banned_fingerprint = cipher.fingerprint(&telegram_origin_key(banned_principal));

    // Fixture report satisfies the bans foreign key; then board-scoped ban.
    let fixture_thread_id = fixture_thread(
        &pool,
        "engineering",
        "Ban fixture thread",
        "visible",
        false,
        30,
        false,
    )
    .await;
    let report = send(
        &app,
        bearer(
            json_request(
                &format!("/internal/telegram/threads/{fixture_thread_id}/reports"),
                report_body("ban-fixture", "ban-fixture-key", "spam", None),
            ),
            TG_TOKEN,
        ),
    )
    .await;
    let report_id = response_json(report).await["report_id"]
        .as_u64()
        .expect("report id");
    sqlx::query(
        "INSERT INTO bans (client_fingerprint, scope, board_id, report_id, moderator_email, reason, expires_at) \
         VALUES (?, 'board', (SELECT id FROM boards WHERE slug = 'engineering'), ?, 'security@example.com', 'unit test', datetime('now', '+1 day'))",
    )
    .bind(banned_fingerprint.as_slice())
    .bind(report_id as i64)
    .execute(&pool)
    .await
    .expect("fixture ban inserts");

    let banned_thread = send(
        &app,
        bearer(
            json_request(
                thread_uri,
                thread_create_body(banned_principal, "bt-1", "Blocked", "body"),
            ),
            TG_TOKEN,
        ),
    )
    .await;
    assert_telegram_error(
        banned_thread,
        StatusCode::FORBIDDEN,
        "Posting is blocked by an active board ban until",
    )
    .await;

    // Replies resolve the ban through the thread's board.
    let banned_reply = send(
        &app,
        bearer(
            json_request(
                &format!("/internal/telegram/threads/{fixture_thread_id}/replies"),
                reply_create_body(banned_principal, "br-1", "blocked reply"),
            ),
            TG_TOKEN,
        ),
    )
    .await;
    assert_telegram_error(
        banned_reply,
        StatusCode::FORBIDDEN,
        "Posting is blocked by an active board ban until",
    )
    .await;

    // A different principal is unaffected: bans are fingerprint-scoped.
    let control = send(
        &app,
        bearer(
            json_request(
                thread_uri,
                thread_create_body("clean-principal", "ct-1", "Allowed", "body"),
            ),
            TG_TOKEN,
        ),
    )
    .await;
    assert_eq!(control.status(), StatusCode::CREATED);

    assert_eq!(
        count(
            &pool,
            "SELECT COUNT(*) FROM threads WHERE title IN ('Blocked')"
        )
        .await,
        0
    );
}

// --- rate limits ---

#[sqlx::test(migrator = "MIGRATOR")]
async fn machine_rates_limit_threads_replies_and_reports(pool: SqlitePool) {
    let app = tg_router(pool.clone());
    let thread_uri = "/internal/telegram/boards/engineering/threads";
    let thread_id = fixture_thread(
        &pool,
        "engineering",
        "Rate fixture thread",
        "visible",
        false,
        30,
        false,
    )
    .await;
    let reply_uri = format!("/internal/telegram/threads/{thread_id}/replies");
    let report_uri = format!("/internal/telegram/threads/{thread_id}/reports");

    for index in 0..2 {
        let response = send(
            &app,
            bearer(
                json_request(
                    thread_uri,
                    thread_create_body("rate-thread", &format!("rt-{index}"), "Thread", "body"),
                ),
                TG_TOKEN,
            ),
        )
        .await;
        assert_eq!(response.status(), StatusCode::CREATED, "thread {index}");
    }
    let third_thread = send(
        &app,
        bearer(
            json_request(
                thread_uri,
                thread_create_body("rate-thread", "rt-final", "Thread", "body"),
            ),
            TG_TOKEN,
        ),
    )
    .await;
    assert_telegram_error(
        third_thread,
        StatusCode::TOO_MANY_REQUESTS,
        "Too many threads.",
    )
    .await;

    for index in 0..10 {
        let response = send(
            &app,
            bearer(
                json_request(
                    &reply_uri,
                    reply_create_body("rate-reply", &format!("rr-{index}"), "reply body"),
                ),
                TG_TOKEN,
            ),
        )
        .await;
        assert_eq!(response.status(), StatusCode::CREATED, "reply {index}");
    }
    let eleventh_reply = send(
        &app,
        bearer(
            json_request(
                &reply_uri,
                reply_create_body("rate-reply", "rr-final", "reply body"),
            ),
            TG_TOKEN,
        ),
    )
    .await;
    assert_telegram_error(
        eleventh_reply,
        StatusCode::TOO_MANY_REQUESTS,
        "Too many replies.",
    )
    .await;

    for index in 0..5 {
        let response = send(
            &app,
            bearer(
                json_request(
                    &report_uri,
                    report_body("rate-report", &format!("rp-{index}"), "spam", None),
                ),
                TG_TOKEN,
            ),
        )
        .await;
        assert_eq!(response.status(), StatusCode::CREATED, "report {index}");
    }
    let sixth_report = send(
        &app,
        bearer(
            json_request(
                &report_uri,
                report_body("rate-report", "rp-final", "spam", None),
            ),
            TG_TOKEN,
        ),
    )
    .await;
    assert_telegram_error(
        sixth_report,
        StatusCode::TOO_MANY_REQUESTS,
        "Too many reports.",
    )
    .await;
}

// --- Miya screening states ---

#[sqlx::test(migrator = "MIGRATOR")]
async fn miya_block_rejects_machine_post_without_persistence_or_events(pool: SqlitePool) {
    let (miya, _server) = scripted_miya(
        200,
        r#"{"action":"block","categories":[{"name":"harassment","score":0.97}],"reasons":["abusive"]}"#,
    )
    .await;
    let dependencies = test_dependencies_with_miya(pool.clone(), HashSet::new(), Some(miya))
        .with_telegram_service_token(Some(TG_TOKEN.to_owned()));
    let app = telegram::telegram_router(Arc::new(dependencies));
    let before = count(&pool, "SELECT COUNT(*) FROM threads").await;

    let blocked = send(
        &app,
        bearer(
            json_request(
                "/internal/telegram/boards/engineering/threads",
                thread_create_body("miya-blocked", "mk-1", "Blocked title", "body"),
            ),
            TG_TOKEN,
        ),
    )
    .await;
    assert_telegram_error(
        blocked,
        StatusCode::UNPROCESSABLE_ENTITY,
        "Your post was blocked by moderation: Miya blocked this content: harassment — abusive",
    )
    .await;

    assert_eq!(count(&pool, "SELECT COUNT(*) FROM threads").await, before);
    assert_eq!(
        count(&pool, "SELECT COUNT(*) FROM projection_outbox").await,
        0
    );
    assert_eq!(count(&pool, "SELECT COUNT(*) FROM reports").await, 0);
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn miya_review_publishes_thread_and_auto_reports_it(pool: SqlitePool) {
    let (miya, _server) = scripted_miya(
        200,
        r#"{"action":"review","categories":[{"name":"spam","score":0.8}],"reasons":["advert-like"]}"#,
    )
    .await;
    let dependencies = test_dependencies_with_miya(pool.clone(), HashSet::new(), Some(miya))
        .with_telegram_service_token(Some(TG_TOKEN.to_owned()));
    let app = telegram::telegram_router(Arc::new(dependencies));

    let published = send(
        &app,
        bearer(
            json_request(
                "/internal/telegram/boards/engineering/threads",
                thread_create_body("miya-review", "mv-1", "Review me", "body"),
            ),
            TG_TOKEN,
        ),
    )
    .await;
    assert_eq!(published.status(), StatusCode::CREATED);
    let thread_id = response_json(published).await["thread_id"]
        .as_u64()
        .expect("thread id");

    let report = sqlx::query_as::<_, (String, Option<String>, String)>(
        "SELECT reason, details, status FROM reports WHERE thread_id = ?",
    )
    .bind(thread_id as i64)
    .fetch_one(&pool)
    .await
    .expect("auto report row");
    assert_eq!(report.0, "other");
    assert_eq!(report.1.as_deref(), Some("Miya: spam — advert-like"));
    assert_eq!(report.2, "pending");
    let report_id = sqlx::query_scalar::<_, i64>("SELECT id FROM reports WHERE thread_id = ?")
        .bind(thread_id as i64)
        .fetch_one(&pool)
        .await
        .expect("auto report id") as u64;

    let events = outbox_rows(&pool).await;
    assert_eq!(
        events,
        vec![
            ("thread_created".into(), Some(thread_id as i64), None, None),
            (
                "report_created".into(),
                Some(thread_id as i64),
                None,
                Some(report_id as i64)
            ),
        ]
    );
}

// --- media processing on the machine path ---

#[sqlx::test(migrator = "MIGRATOR")]
async fn machine_media_success_unavailable_and_replay_cleanup(pool: SqlitePool) {
    let media = ScriptedMedia::new([
        ScriptedMediaOutcome::success("img-machine", "thumb.webp", "display.webp", 800, 600),
        ScriptedMediaOutcome::success("img-machine", "thumb.webp", "display.webp", 800, 600),
    ]);
    let dependencies = test_dependencies(
        pool.clone(),
        HashSet::new(),
        None,
        Some(media.clone() as Arc<dyn MediaProcessor>),
    )
    .with_telegram_service_token(Some(TG_TOKEN.to_owned()));
    let app = telegram::telegram_router(Arc::new(dependencies));

    let uri = "/internal/telegram/boards/engineering/threads";
    let create = || {
        post_multipart(
            uri,
            &[
                ("principal", "media-principal"),
                ("idempotency_key", "media-key-1"),
                ("title", "Media thread"),
                ("body", "body"),
            ],
            Some((
                "pixel.png",
                "image/png",
                b"\x89PNG\r\n\x1a\npayload".as_slice(),
            )),
        )
    };
    let first = send(&app, bearer(create(), TG_TOKEN)).await;
    assert_eq!(first.status(), StatusCode::CREATED);
    let thread_id = response_json(first).await["thread_id"]
        .as_u64()
        .expect("thread id");
    assert_eq!(media.uploads().len(), 1);
    assert!(media.deleted_image_ids().is_empty());

    let snapshot = send(
        &app,
        bearer(
            get_request(&format!("/internal/telegram/threads/{thread_id}")),
            TG_TOKEN,
        ),
    )
    .await;
    assert_eq!(snapshot.status(), StatusCode::OK);
    let snap = response_json(snapshot).await;
    assert_eq!(snap["media"]["thumbnail_path"], "thumb.webp");
    assert_eq!(snap["media"]["mime_type"], "image/webp");
    assert_eq!(snap["media"]["width"], serde_json::json!(800));
    assert_eq!(snap["media"]["height"], serde_json::json!(600));

    // Pre-check replay short-circuits BEFORE bans/rate/Miya/media: the stored
    // copy stays untouched and no second processing run happens.
    let replay = send(&app, bearer(create(), TG_TOKEN)).await;
    assert_eq!(replay.status(), StatusCode::OK);
    assert_eq!(
        response_json(replay).await["thread_id"],
        serde_json::json!(thread_id)
    );
    assert_eq!(media.uploads().len(), 1);
    assert!(media.deleted_image_ids().is_empty());

    // Approved-board gate sits above media for every key state: unknown board
    // on a NEW key rejects before any processing, so nothing uploads/deletes.
    let orphan = send(
        &app,
        bearer(
            post_multipart(
                "/internal/telegram/boards/ghost-board/threads",
                &[
                    ("principal", "media-principal"),
                    ("idempotency_key", "media-key-2"),
                    ("title", "Orphan media"),
                    ("body", "body"),
                ],
                Some((
                    "pixel.png",
                    "image/png",
                    b"\x89PNG\r\n\x1a\npayload".as_slice(),
                )),
            ),
            TG_TOKEN,
        ),
    )
    .await;
    assert_telegram_error(orphan, StatusCode::NOT_FOUND, "board not found").await;
    assert_eq!(media.uploads().len(), 1);
    assert!(media.deleted_image_ids().is_empty());
    let stored_media =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM post_media WHERE thread_id = ?")
            .bind(thread_id as i64)
            .fetch_one(&pool)
            .await
            .expect("stored media count");
    assert_eq!(stored_media, 1);

    // Without a processor the machine path degrades to 503 and persists nothing.
    let bare_app = tg_router(pool.clone());
    let unavailable = send(
        &bare_app,
        bearer(
            post_multipart(
                uri,
                &[
                    ("principal", "media-principal"),
                    ("idempotency_key", "media-key-2"),
                    ("title", "No processor"),
                    ("body", "body"),
                ],
                Some((
                    "pixel.png",
                    "image/png",
                    b"\x89PNG\r\n\x1a\npayload".as_slice(),
                )),
            ),
            TG_TOKEN,
        ),
    )
    .await;
    assert_telegram_error(
        unavailable,
        StatusCode::SERVICE_UNAVAILABLE,
        "Image uploads are temporarily unavailable.",
    )
    .await;
    assert_eq!(
        count(
            &pool,
            "SELECT COUNT(*) FROM threads WHERE title = 'No processor'"
        )
        .await,
        0
    );
}

// --- Turnstile difference: machine path never consults CAPTCHA ---

#[sqlx::test(migrator = "MIGRATOR")]
async fn machine_path_skips_turnstile_entirely(pool: SqlitePool) {
    // An empty scripted queue makes any verifier call panic the test.
    let captcha = Arc::new(ScriptedCaptcha::new(Vec::<CaptchaOutcome>::new()));
    let dependencies = test_dependencies(
        pool.clone(),
        HashSet::new(),
        Some(captcha as Arc<dyn CaptchaVerifier>),
        None,
    )
    .with_telegram_service_token(Some(TG_TOKEN.to_owned()));
    let app = telegram::telegram_router(Arc::new(dependencies));

    let created = send(
        &app,
        bearer(
            json_request(
                "/internal/telegram/boards/engineering/threads",
                thread_create_body("captcha-free", "cf-1", "No Turnstile", "body"),
            ),
            TG_TOKEN,
        ),
    )
    .await;
    assert_eq!(
        created.status(),
        StatusCode::CREATED,
        "machine posts must bypass CAPTCHA entirely"
    );
}

// --- concurrent duplicates ---

#[sqlx::test(migrator = "MIGRATOR")]
async fn concurrent_duplicate_creates_exactly_one_thread_and_event(pool: SqlitePool) {
    let app = Arc::new(tg_router(pool.clone()));
    let uri = "/internal/telegram/boards/engineering/threads";
    let payload = thread_create_body("concurrent-p", "concurrent-k", "Concurrent", "body");

    let tasks: Vec<_> = (0..8)
        .map(|_| {
            let app = app.clone();
            let payload = payload.clone();
            tokio::spawn(async move {
                let response = send(&app, bearer(json_request(uri, payload), TG_TOKEN)).await;
                let status = response.status();
                let body = response_json(response).await;
                let thread_id = body["thread_id"]
                    .as_u64()
                    .unwrap_or_else(|| panic!("status {status} without thread id: {body}"));
                (status, thread_id)
            })
        })
        .collect();

    let mut created_count = 0;
    let mut replayed_count = 0;
    let mut returned_ids = Vec::new();
    for task in tasks {
        let (status, thread_id) = task.await.expect("concurrent request completes");
        returned_ids.push(thread_id);
        match status {
            StatusCode::CREATED => created_count += 1,
            StatusCode::OK => replayed_count += 1,
            other => panic!(
                "unexpected concurrent status {other}: contract allows only one 201 and seven 200 replays"
            ),
        }
    }
    assert_eq!(created_count, 1, "exactly one winner creates the thread");
    assert_eq!(
        replayed_count, 7,
        "every loser observes the committed replay"
    );
    let winner_id = returned_ids
        .iter()
        .find(|id| **id != 0)
        .copied()
        .expect("winner id");
    assert!(
        returned_ids.iter().all(|id| *id == winner_id),
        "every response returns the winner's thread id: {returned_ids:?}"
    );
    assert_eq!(
        count(
            &pool,
            "SELECT COUNT(*) FROM threads WHERE title = 'Concurrent'"
        )
        .await,
        1
    );
    let events = outbox_rows(&pool).await;
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].0, "thread_created");
}

// --- Miya configured but unavailable: fail-open publish with auto-report ---

#[sqlx::test(migrator = "MIGRATOR")]
async fn miya_unavailable_publishes_thread_and_auto_reports_unchecked(pool: SqlitePool) {
    let dead_url = unreachable_webhook_url().await;
    let miya = miya::Miya::new(dead_url).expect("scripted Miya URL is valid");
    let dependencies =
        test_dependencies_with_miya(pool.clone(), HashSet::new(), Some(Arc::new(miya)))
            .with_telegram_service_token(Some(TG_TOKEN.to_owned()));
    let app = telegram::telegram_router(Arc::new(dependencies));

    let published = send(
        &app,
        bearer(
            json_request(
                "/internal/telegram/boards/engineering/threads",
                thread_create_body("miya-down", "mu-1", "Unchecked publish", "body"),
            ),
            TG_TOKEN,
        ),
    )
    .await;
    assert_eq!(
        published.status(),
        StatusCode::CREATED,
        "Miya outage must not hard-block publication"
    );
    let thread_id = response_json(published).await["thread_id"]
        .as_u64()
        .expect("thread id");

    let report = sqlx::query_as::<_, (String, Option<String>, String)>(
        "SELECT reason, details, status FROM reports WHERE thread_id = ?",
    )
    .bind(thread_id as i64)
    .fetch_one(&pool)
    .await
    .expect("fail-open auto report row");
    assert_eq!(report.0, "other");
    assert_eq!(
        report.1.as_deref(),
        Some("Miya unavailable — content was not checked.")
    );
    assert_eq!(report.2, "pending");
    let report_id = sqlx::query_scalar::<_, i64>("SELECT id FROM reports WHERE thread_id = ?")
        .bind(thread_id as i64)
        .fetch_one(&pool)
        .await
        .expect("auto report id") as u64;

    let events = outbox_rows(&pool).await;
    assert_eq!(
        events,
        vec![
            ("thread_created".into(), Some(thread_id as i64), None, None),
            (
                "report_created".into(),
                Some(thread_id as i64),
                None,
                Some(report_id as i64)
            ),
        ]
    );
}

// --- media processed then persistence fails: image is compensated away ---

#[sqlx::test(migrator = "MIGRATOR")]
async fn failed_persistence_after_media_processing_cleans_up_image(pool: SqlitePool) {
    let media = ScriptedMedia::new([ScriptedMediaOutcome::success(
        "img-doomed",
        "thumb-doomed.webp",
        "display-doomed.webp",
        800,
        600,
    )]);
    let dependencies = test_dependencies(
        pool.clone(),
        HashSet::new(),
        None,
        Some(media.clone() as Arc<dyn MediaProcessor>),
    )
    .with_telegram_service_token(Some(TG_TOKEN.to_owned()));
    let app = telegram::telegram_router(Arc::new(dependencies));

    // Break only the persistence layer inside this isolated test pool so the
    // idempotent persist transaction errors after processing succeeded.
    sqlx::query("DROP TABLE post_origins")
        .execute(&pool)
        .await
        .expect("drop post_origins fixture");

    let doomed = send(
        &app,
        bearer(
            post_multipart(
                "/internal/telegram/boards/engineering/threads",
                &[
                    ("principal", "doomed-p"),
                    ("idempotency_key", "doomed-k-1"),
                    ("title", "Doomed media"),
                    ("body", "body"),
                ],
                Some((
                    "pixel.png",
                    "image/png",
                    b"\x89PNG\r\n\x1a\npayload".as_slice(),
                )),
            ),
            TG_TOKEN,
        ),
    )
    .await;
    assert_telegram_error(doomed, StatusCode::INTERNAL_SERVER_ERROR, "database error").await;

    // Processing ran exactly once and its product was compensated on failure.
    assert_eq!(media.uploads().len(), 1);
    assert_eq!(media.deleted_image_ids(), vec!["img-doomed".to_owned()]);
    assert_eq!(
        count(
            &pool,
            "SELECT COUNT(*) FROM threads WHERE title = 'Doomed media'"
        )
        .await,
        0,
        "failed persistence leaves no partial thread"
    );
    assert_eq!(
        count(&pool, "SELECT COUNT(*) FROM projection_outbox").await,
        0,
        "failed persistence emits no outbox event"
    );
    assert_eq!(
        count(&pool, "SELECT COUNT(*) FROM machine_idempotency").await,
        0,
        "claim rolls back with the failed transaction"
    );
}

// --- snapshots: visibility, paging, state, media ---

#[sqlx::test(migrator = "MIGRATOR")]
async fn snapshot_covers_visibility_paging_state_and_media(pool: SqlitePool) {
    let app = tg_router(pool.clone());
    let visible_id = fixture_thread(
        &pool,
        "engineering",
        "Snapshot visible",
        "visible",
        false,
        100,
        false,
    )
    .await;
    let locked_id = fixture_thread(
        &pool,
        "engineering",
        "Snapshot locked",
        "locked",
        false,
        90,
        false,
    )
    .await;
    let archived_id = fixture_thread(
        &pool,
        "engineering",
        "Snapshot archived",
        "visible",
        false,
        80,
        true,
    )
    .await;
    let hidden_id = fixture_thread(
        &pool,
        "engineering",
        "Snapshot hidden",
        "hidden",
        false,
        70,
        false,
    )
    .await;

    sqlx::query(
        "INSERT INTO post_media (thread_id, thumbnail_path, display_path, mime_type, width, height) \
         VALUES (?, 't-snap.webp', 'd-snap.webp', 'image/webp', 640, 480)",
    )
    .bind(visible_id as i64)
    .execute(&pool)
    .await
    .expect("thread media fixture");

    let reply_a = fixture_reply(&pool, visible_id, "alpha", "visible", 40).await;
    let reply_b = fixture_reply(&pool, visible_id, "beta", "visible", 30).await;
    let reply_c = fixture_reply(&pool, visible_id, "gamma", "visible", 20).await;
    let reply_d = fixture_reply(&pool, visible_id, "delta", "visible", 10).await;
    let _shadowed = fixture_reply(&pool, visible_id, "shadow", "hidden", 15).await;

    sqlx::query(
        "INSERT INTO post_media (reply_id, thumbnail_path, display_path, mime_type, width, height) \
         VALUES (?, 't-gamma.webp', 'd-gamma.webp', 'image/webp', 320, 240)",
    )
    .bind(reply_c as i64)
    .execute(&pool)
    .await
    .expect("reply media fixture");

    let missing = send(
        &app,
        bearer(get_request("/internal/telegram/threads/987654"), TG_TOKEN),
    )
    .await;
    assert_telegram_error(missing, StatusCode::NOT_FOUND, "thread not found").await;

    let hidden = send(
        &app,
        bearer(
            get_request(&format!("/internal/telegram/threads/{hidden_id}")),
            TG_TOKEN,
        ),
    )
    .await;
    assert_telegram_error(hidden, StatusCode::NOT_FOUND, "thread not found").await;

    let full = send(
        &app,
        bearer(
            get_request(&format!("/internal/telegram/threads/{visible_id}")),
            TG_TOKEN,
        ),
    )
    .await;
    assert_eq!(full.status(), StatusCode::OK);
    let snap = response_json(full).await;
    assert_eq!(snap["id"], serde_json::json!(visible_id));
    assert_eq!(snap["board_slug"], "engineering");
    assert_eq!(snap["title"], "Snapshot visible");
    assert_eq!(snap["is_pinned"], serde_json::json!(false));
    assert_eq!(snap["is_locked"], serde_json::json!(false));
    assert_eq!(snap["is_archived"], serde_json::json!(false));
    assert_eq!(
        snap["reply_count"],
        serde_json::json!(4),
        "hidden replies stay excluded"
    );
    assert_eq!(snap["media"]["display_path"], "d-snap.webp");
    let reply_ids = collect_ids(&snap["replies"]);
    assert_eq!(reply_ids, vec![reply_a, reply_b, reply_c, reply_d]);
    assert_eq!(snap["has_next_replies"], serde_json::json!(false));
    let gamma = &snap["replies"][2];
    assert_eq!(gamma["media"]["thumbnail_path"], "t-gamma.webp");

    // Paging window ordered by (created_at, id).
    let page_one = send(
        &app,
        bearer(
            get_request(&format!(
                "/internal/telegram/threads/{visible_id}?reply_limit=2"
            )),
            TG_TOKEN,
        ),
    )
    .await;
    let page_one = response_json(page_one).await;
    assert_eq!(collect_ids(&page_one["replies"]), vec![reply_a, reply_b]);
    assert_eq!(page_one["has_next_replies"], serde_json::json!(true));
    assert_eq!(page_one["reply_count"], serde_json::json!(4));

    let page_two = send(
        &app,
        bearer(
            get_request(&format!(
                "/internal/telegram/threads/{visible_id}?reply_limit=2&reply_offset=2"
            )),
            TG_TOKEN,
        ),
    )
    .await;
    let page_two = response_json(page_two).await;
    assert_eq!(collect_ids(&page_two["replies"]), vec![reply_c, reply_d]);
    assert_eq!(page_two["has_next_replies"], serde_json::json!(false));

    // Alias parameters behave identically.
    let aliased = send(
        &app,
        bearer(
            get_request(&format!(
                "/internal/telegram/threads/{visible_id}?limit=2&offset=2"
            )),
            TG_TOKEN,
        ),
    )
    .await;
    assert_eq!(
        collect_ids(&response_json(aliased).await["replies"]),
        vec![reply_c, reply_d]
    );

    // Bounds with exact messages.
    for (query, message) in [
        ("reply_limit=0", "reply_limit must be between 1 and 100"),
        ("reply_limit=101", "reply_limit must be between 1 and 100"),
        ("reply_offset=-1", "reply_offset out of bounds"),
        ("reply_offset=100001", "reply_offset out of bounds"),
    ] {
        let bad = send(
            &app,
            bearer(
                get_request(&format!("/internal/telegram/threads/{visible_id}?{query}")),
                TG_TOKEN,
            ),
        )
        .await;
        assert_telegram_error(bad, StatusCode::BAD_REQUEST, message).await;
    }

    // Locked and archived threads remain readable with state flags set.
    let locked = send(
        &app,
        bearer(
            get_request(&format!("/internal/telegram/threads/{locked_id}")),
            TG_TOKEN,
        ),
    )
    .await;
    let locked = response_json(locked).await;
    assert_eq!(locked["is_locked"], serde_json::json!(true));
    assert_eq!(locked["is_archived"], serde_json::json!(false));

    let archived = send(
        &app,
        bearer(
            get_request(&format!("/internal/telegram/threads/{archived_id}")),
            TG_TOKEN,
        ),
    )
    .await;
    let archived = response_json(archived).await;
    assert_eq!(archived["is_archived"], serde_json::json!(true));
    assert_eq!(archived["is_locked"], serde_json::json!(false));

    // State gates on replies.
    let locked_reply = send(
        &app,
        bearer(
            json_request(
                &format!("/internal/telegram/threads/{locked_id}/replies"),
                reply_create_body("state-p", "state-k-1", "reply"),
            ),
            TG_TOKEN,
        ),
    )
    .await;
    assert_telegram_error(locked_reply, StatusCode::CONFLICT, "This thread is locked").await;

    let archived_reply = send(
        &app,
        bearer(
            json_request(
                &format!("/internal/telegram/threads/{archived_id}/replies"),
                reply_create_body("state-p", "state-k-2", "reply"),
            ),
            TG_TOKEN,
        ),
    )
    .await;
    assert_telegram_error(
        archived_reply,
        StatusCode::CONFLICT,
        "This thread is archived and read-only",
    )
    .await;

    let hidden_reply = send(
        &app,
        bearer(
            json_request(
                &format!("/internal/telegram/threads/{hidden_id}/replies"),
                reply_create_body("state-p", "state-k-3", "reply"),
            ),
            TG_TOKEN,
        ),
    )
    .await;
    assert_telegram_error(hidden_reply, StatusCode::NOT_FOUND, "thread not found").await;
}

// --- backfill: PASUM active order and limit bounds ---

#[sqlx::test(migrator = "MIGRATOR")]
async fn pasum_backfill_orders_active_threads_and_bounds_limit(pool: SqlitePool) {
    let app = tg_router(pool.clone());
    let uri = "/internal/telegram/boards/pasum/threads";

    let pinned = fixture_thread(&pool, "pasum", "Pinned old", "visible", true, 60, false).await;
    let newest = fixture_thread(&pool, "pasum", "Newest", "visible", false, 10, false).await;
    let middle = fixture_thread(&pool, "pasum", "Middle", "visible", false, 30, false).await;
    let locked_mid = fixture_thread(&pool, "pasum", "Locked mid", "locked", false, 40, false).await;
    let oldest = fixture_thread(&pool, "pasum", "Oldest", "visible", false, 50, false).await;
    let _archived_out =
        fixture_thread(&pool, "pasum", "Archived out", "visible", false, 5, true).await;
    let _hidden_out = fixture_thread(&pool, "pasum", "Hidden out", "hidden", false, 2, false).await;

    let limited = send(
        &app,
        bearer(get_request(&format!("{uri}?limit=2")), TG_TOKEN),
    )
    .await;
    assert_eq!(limited.status(), StatusCode::OK);
    assert_eq!(
        response_json(limited).await["thread_ids"],
        serde_json::json!([pinned, newest])
    );

    let default = send(&app, bearer(get_request(uri), TG_TOKEN)).await;
    assert_eq!(default.status(), StatusCode::OK);
    // Default limit 20 covers all fixtures while preserving active public order:
    // pinned DESC, then bumped_at DESC, then id DESC.
    assert_eq!(
        response_json(default).await["thread_ids"],
        serde_json::json!([pinned, newest, middle, locked_mid, oldest])
    );

    for raw in ["0", "101"] {
        let bad = send(
            &app,
            bearer(get_request(&format!("{uri}?limit={raw}")), TG_TOKEN),
        )
        .await;
        assert_telegram_error(
            bad,
            StatusCode::BAD_REQUEST,
            "limit must be between 1 and 100",
        )
        .await;
    }
    let malformed = send(
        &app,
        bearer(get_request(&format!("{uri}?limit=abc")), TG_TOKEN),
    )
    .await;
    assert_telegram_error(
        malformed,
        StatusCode::BAD_REQUEST,
        "limit must be integer 1..100",
    )
    .await;

    let unknown = send(
        &app,
        bearer(
            get_request("/internal/telegram/boards/ghost/threads"),
            TG_TOKEN,
        ),
    )
    .await;
    assert_telegram_error(unknown, StatusCode::NOT_FOUND, "board not found").await;
}

// --- outbox: lease, ACK, purge, moderation-driven events ---

#[sqlx::test(migrator = "MIGRATOR")]
async fn outbox_lease_ack_purge_and_moderation_events_flow(pool: SqlitePool) {
    let machine = tg_router(pool.clone());
    let moderation_app = moderator_router(pool.clone());

    // Machine mutations generate the three base kinds.
    let thread_response = send(
        &machine,
        bearer(
            json_request(
                "/internal/telegram/boards/engineering/threads",
                thread_create_body("outbox-p", "ob-k-1", "Outbox thread", "body"),
            ),
            TG_TOKEN,
        ),
    )
    .await;
    assert_eq!(thread_response.status(), StatusCode::CREATED);
    let thread_id = response_json(thread_response).await["thread_id"]
        .as_u64()
        .expect("thread id");

    let reply_response = send(
        &machine,
        bearer(
            json_request(
                &format!("/internal/telegram/threads/{thread_id}/replies"),
                reply_create_body("outbox-p", "ob-k-2", "outbox reply"),
            ),
            TG_TOKEN,
        ),
    )
    .await;
    assert_eq!(reply_response.status(), StatusCode::CREATED);

    let report_response = send(
        &machine,
        bearer(
            json_request(
                &format!("/internal/telegram/threads/{thread_id}/reports"),
                report_body("outbox-p", "ob-k-3", "spam", None),
            ),
            TG_TOKEN,
        ),
    )
    .await;
    assert_eq!(report_response.status(), StatusCode::CREATED);

    // Moderation actions append their own events: pin dirties, hide removes.
    let pin = send(
        &moderation_app,
        with_header(
            post_form(&format!("/mod/threads/{thread_id}/pin"), ""),
            "cf-access-authenticated-user-email",
            MODERATOR_EMAIL,
        ),
    )
    .await;
    assert_eq!(pin.status(), StatusCode::SEE_OTHER);

    let hide = send(
        &moderation_app,
        with_header(
            post_form(
                &format!("/mod/threads/{thread_id}/hide"),
                "reason=harassment&note=outbox+fixture",
            ),
            "cf-access-authenticated-user-email",
            MODERATOR_EMAIL,
        ),
    )
    .await;
    assert_eq!(hide.status(), StatusCode::SEE_OTHER);

    let all = outbox_rows(&pool).await;
    let report_event_report_id = all[2].3.expect("report event carries report id");
    assert_eq!(
        all,
        vec![
            (
                "thread_created".to_owned(),
                Some(thread_id as i64),
                None,
                None
            ),
            (
                "thread_dirty".to_owned(),
                Some(thread_id as i64),
                None,
                None
            ),
            (
                "report_created".to_owned(),
                Some(thread_id as i64),
                None,
                Some(report_event_report_id)
            ),
            (
                "thread_dirty".to_owned(),
                Some(thread_id as i64),
                None,
                None
            ),
            (
                "thread_removed".to_owned(),
                Some(thread_id as i64),
                None,
                None
            ),
        ],
        "expected created/dirty/report/dirty(pin)/removed(hide) in insertion order"
    );
    let event_ids: Vec<i64> = sqlx::query_scalar("SELECT id FROM projection_outbox ORDER BY id")
        .fetch_all(&pool)
        .await
        .expect("event ids");
    assert_eq!(event_ids.len(), 5);

    // Lease honors the requested batch size and stamps UUID tokens.
    let lease_one = send(
        &machine,
        bearer(
            json_request(
                "/internal/telegram/outbox/lease",
                serde_json::json!({"limit": 3}),
            ),
            TG_TOKEN,
        ),
    )
    .await;
    assert_eq!(lease_one.status(), StatusCode::OK);
    let batch_one = response_json(lease_one).await["events"]
        .as_array()
        .expect("events array")
        .to_vec();
    assert_eq!(batch_one.len(), 3);
    let leased_tokens: Vec<String> = batch_one
        .iter()
        .map(|event| {
            event["lease_token"]
                .as_str()
                .expect("lease token")
                .to_owned()
        })
        .collect();
    assert!(leased_tokens.iter().all(|token| token.len() == 36));
    assert_eq!(batch_one[0]["kind"], "thread_created");
    assert_eq!(batch_one[1]["kind"], "thread_dirty");
    assert_eq!(batch_one[2]["kind"], "report_created");
    assert!(batch_one[0]["lease_expires_at"].as_str().is_some());

    let ack_uri = "/internal/telegram/outbox/ack";
    let event_two = batch_one[1]["id"].as_u64().expect("event id");

    // Validation failures.
    let missing_event = send(
        &machine,
        bearer(
            json_request(ack_uri, serde_json::json!({"lease_token": "t"})),
            TG_TOKEN,
        ),
    )
    .await;
    assert_telegram_error(missing_event, StatusCode::BAD_REQUEST, "missing event_id").await;

    let missing_token = send(
        &machine,
        bearer(
            json_request(ack_uri, serde_json::json!({"event_id": event_two})),
            TG_TOKEN,
        ),
    )
    .await;
    assert_telegram_error(
        missing_token,
        StatusCode::BAD_REQUEST,
        "missing field `lease_token`",
    )
    .await;

    let unknown_event = send(
        &machine,
        bearer(
            json_request(
                ack_uri,
                serde_json::json!({"event_id": 999_999, "lease_token": "t"}),
            ),
            TG_TOKEN,
        ),
    )
    .await;
    assert_telegram_error(unknown_event, StatusCode::NOT_FOUND, "event not found").await;

    // Wrong token conflicts before acknowledgement.
    let wrong_token_ack = send(
        &machine,
        bearer(
            json_request(
                ack_uri,
                serde_json::json!({"event_id": event_two, "lease_token": "wrong-token"}),
            ),
            TG_TOKEN,
        ),
    )
    .await;
    assert_telegram_error(
        wrong_token_ack,
        StatusCode::CONFLICT,
        "lease token mismatch",
    )
    .await;

    // Matching token acknowledges; repeated matching ACK succeeds idempotently.
    let token_two = leased_tokens[1].clone();
    let ack_payload = serde_json::json!({"event_id": event_two, "lease_token": token_two});
    let ack = send(
        &machine,
        bearer(json_request(ack_uri, ack_payload.clone()), TG_TOKEN),
    )
    .await;
    assert_eq!(ack.status(), StatusCode::OK);
    assert_eq!(response_json(ack).await["status"], "acknowledged");
    let repeat_ack = send(
        &machine,
        bearer(json_request(ack_uri, ack_payload), TG_TOKEN),
    )
    .await;
    assert_eq!(
        repeat_ack.status(),
        StatusCode::OK,
        "repeated matching ACK succeeds"
    );

    // Expired leases are reclaimable with fresh tokens; ACK'd rows stay excluded.
    sqlx::query(
        "UPDATE projection_outbox SET lease_expires_at = datetime('now', '-1 minute') \
         WHERE id IN (?, ?)",
    )
    .bind(event_ids[0])
    .bind(event_ids[2])
    .execute(&pool)
    .await
    .expect("expire leases");

    let lease_two = send(
        &machine,
        bearer(
            raw_json_request("/internal/telegram/outbox/lease", String::new()),
            TG_TOKEN,
        ),
    )
    .await;
    assert_eq!(
        lease_two.status(),
        StatusCode::OK,
        "empty body leases with defaults"
    );
    let batch_two = response_json(lease_two).await["events"]
        .as_array()
        .expect("events array")
        .to_vec();
    let batch_two_ids: Vec<u64> = batch_two
        .iter()
        .map(|event| event["id"].as_u64().expect("event id"))
        .collect();
    assert_eq!(
        batch_two_ids,
        vec![
            event_ids[0] as u64,
            event_ids[2] as u64,
            event_ids[3] as u64,
            event_ids[4] as u64,
        ]
    );
    let reclaimed_token = batch_two[0]["lease_token"]
        .as_str()
        .expect("fresh token")
        .to_owned();
    assert_ne!(
        reclaimed_token, leased_tokens[0],
        "reclaimed events carry new tokens"
    );

    // ACK the remaining leased events with their fresh tokens.
    for event in &batch_two[2..4] {
        let ack = send(
            &machine,
            bearer(
                json_request(
                    ack_uri,
                    serde_json::json!({
                        "event_id": event["id"].as_u64().expect("event id"),
                        "lease_token": event["lease_token"].as_str().expect("token"),
                    }),
                ),
                TG_TOKEN,
            ),
        )
        .await;
        assert_eq!(ack.status(), StatusCode::OK);
    }

    // Purge removes only acknowledged rows older than retention.
    let recent = forum::purge_acknowledged_projection_outbox(&pool, 7 * 24 * 60 * 60)
        .await
        .expect("retention purge runs");
    assert_eq!(recent, 0, "recent acknowledgements survive retention");
    sqlx::query(
        "UPDATE projection_outbox SET acknowledged_at = datetime('now', '-8 days') \
         WHERE acknowledged_at IS NOT NULL",
    )
    .execute(&pool)
    .await
    .expect("backdate acknowledgements");
    let purged = forum::purge_acknowledged_projection_outbox(&pool, 7 * 24 * 60 * 60)
        .await
        .expect("purge runs");
    assert_eq!(
        purged, 3,
        "only acknowledged events past retention are purged"
    );

    let remaining: Vec<i64> = sqlx::query_scalar("SELECT id FROM projection_outbox ORDER BY id")
        .fetch_all(&pool)
        .await
        .expect("remaining events");
    assert_eq!(
        remaining,
        vec![event_ids[0], event_ids[2]],
        "leased-but-unacknowledged events survive purge"
    );

    // Nonpositive lease limits are rejected.
    let zero_limit = send(
        &machine,
        bearer(
            json_request(
                "/internal/telegram/outbox/lease",
                serde_json::json!({"limit": 0}),
            ),
            TG_TOKEN,
        ),
    )
    .await;
    assert_telegram_error(
        zero_limit,
        StatusCode::BAD_REQUEST,
        "limit must be positive",
    )
    .await;
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn telegram_idempotency_hashes_principal_and_frames_inputs_replay_first(pool: SqlitePool) {
    let app = tg_router(pool.clone());
    let thread_uri = "/internal/telegram/boards/engineering/threads";
    let threads_before_principal = count(&pool, "SELECT COUNT(*) FROM threads").await;
    let events_before_principal = count(&pool, "SELECT COUNT(*) FROM projection_outbox").await;

    // Principal is part of idempotency identity, but never persisted raw.
    let principal_first = send(
        &app,
        bearer(
            json_request(
                thread_uri,
                thread_create_body("principal-one", "principal-key", "Title", "Body"),
            ),
            TG_TOKEN,
        ),
    )
    .await;
    assert_eq!(principal_first.status(), StatusCode::CREATED);
    let principal_thread = response_json(principal_first).await["thread_id"]
        .as_u64()
        .expect("principal thread id");
    let principal_conflict = send(
        &app,
        bearer(
            json_request(
                thread_uri,
                thread_create_body("principal-two", "principal-key", "Title", "Body"),
            ),
            TG_TOKEN,
        ),
    )
    .await;
    assert_telegram_error(
        principal_conflict,
        StatusCode::CONFLICT,
        "idempotency conflict",
    )
    .await;
    assert_eq!(
        count(&pool, "SELECT COUNT(*) FROM threads").await,
        threads_before_principal + 1,
        "principal conflict creates no second thread"
    );
    assert_eq!(
        count(&pool, "SELECT COUNT(*) FROM projection_outbox").await,
        events_before_principal + 1,
        "principal conflict creates no second event"
    );

    let stored_poster: String = sqlx::query_scalar("SELECT poster_id FROM threads WHERE id = ?")
        .bind(principal_thread as i64)
        .fetch_one(&pool)
        .await
        .expect("poster id");
    assert!(!stored_poster.contains("principal-one"));

    // NUL bytes cannot shift field boundaries under length-prefixed framing.
    let threads_before_nul = count(&pool, "SELECT COUNT(*) FROM threads").await;
    let nul_first = send(
        &app,
        bearer(
            json_request(
                thread_uri,
                thread_create_body("nul-principal", "nul-key", "a\0b", "c"),
            ),
            TG_TOKEN,
        ),
    )
    .await;
    assert_eq!(nul_first.status(), StatusCode::CREATED);
    let nul_conflict = send(
        &app,
        bearer(
            json_request(
                thread_uri,
                thread_create_body("nul-principal", "nul-key", "a", "b\0c"),
            ),
            TG_TOKEN,
        ),
    )
    .await;
    assert_telegram_error(nul_conflict, StatusCode::CONFLICT, "idempotency conflict").await;
    assert_eq!(
        count(&pool, "SELECT COUNT(*) FROM threads").await,
        threads_before_nul + 1,
        "NUL boundary-shift conflict creates no second thread"
    );

    let null_target = fixture_thread(
        &pool,
        "engineering",
        "Null report target",
        "visible",
        false,
        5,
        false,
    )
    .await;
    let null_report = send(
        &app,
        bearer(
            json_request(
                &format!("/internal/telegram/threads/{null_target}/reports"),
                report_body("null-principal", "null-key", "spam", None),
            ),
            TG_TOKEN,
        ),
    )
    .await;
    assert_eq!(null_report.status(), StatusCode::CREATED);
    let reports_before_literal = count(&pool, "SELECT COUNT(*) FROM reports").await;
    let events_before_literal = count(&pool, "SELECT COUNT(*) FROM projection_outbox").await;
    let literal_conflict = send(
        &app,
        bearer(
            json_request(
                &format!("/internal/telegram/threads/{null_target}/reports"),
                report_body("null-principal", "null-key", "spam", Some("no_details")),
            ),
            TG_TOKEN,
        ),
    )
    .await;
    assert_telegram_error(
        literal_conflict,
        StatusCode::CONFLICT,
        "idempotency conflict",
    )
    .await;
    assert_eq!(
        count(&pool, "SELECT COUNT(*) FROM reports").await,
        reports_before_literal,
        "details sentinel collision creates no report"
    );
    assert_eq!(
        count(&pool, "SELECT COUNT(*) FROM projection_outbox").await,
        events_before_literal,
        "details sentinel collision creates no event"
    );

    // Reply replay survives a mutable thread lock.
    let lock_thread = fixture_thread(
        &pool,
        "engineering",
        "Lock target",
        "visible",
        false,
        4,
        false,
    )
    .await;
    let lock_reply_uri = format!("/internal/telegram/threads/{lock_thread}/replies");
    let lock_payload = reply_create_body("lock-principal", "lock-key", "reply body");
    let lock_created = send(
        &app,
        bearer(
            json_request(&lock_reply_uri, lock_payload.clone()),
            TG_TOKEN,
        ),
    )
    .await;
    assert_eq!(lock_created.status(), StatusCode::CREATED);
    let replies_before_lock_replay = count(&pool, "SELECT COUNT(*) FROM replies").await;
    let events_before_lock_replay = count(&pool, "SELECT COUNT(*) FROM projection_outbox").await;
    sqlx::query("UPDATE threads SET status = 'locked' WHERE id = ?")
        .bind(lock_thread as i64)
        .execute(&pool)
        .await
        .expect("lock thread");
    let lock_replay = send(
        &app,
        bearer(json_request(&lock_reply_uri, lock_payload), TG_TOKEN),
    )
    .await;
    assert_eq!(lock_replay.status(), StatusCode::OK);
    assert_eq!(
        count(&pool, "SELECT COUNT(*) FROM replies").await,
        replies_before_lock_replay
    );
    assert_eq!(
        count(&pool, "SELECT COUNT(*) FROM projection_outbox").await,
        events_before_lock_replay
    );

    // Thread-report replay survives target hide.
    let hidden_thread = fixture_thread(
        &pool,
        "engineering",
        "Hide target",
        "visible",
        false,
        3,
        false,
    )
    .await;
    let hidden_report_uri = format!("/internal/telegram/threads/{hidden_thread}/reports");
    let hidden_report_payload = report_body("hide-principal", "hide-key", "harassment", None);
    let hidden_report = send(
        &app,
        bearer(
            json_request(&hidden_report_uri, hidden_report_payload.clone()),
            TG_TOKEN,
        ),
    )
    .await;
    assert_eq!(hidden_report.status(), StatusCode::CREATED);
    let reports_before_hidden_replay = count(&pool, "SELECT COUNT(*) FROM reports").await;
    let events_before_hidden_replay = count(&pool, "SELECT COUNT(*) FROM projection_outbox").await;
    sqlx::query("UPDATE threads SET status = 'hidden' WHERE id = ?")
        .bind(hidden_thread as i64)
        .execute(&pool)
        .await
        .expect("hide thread");
    let hidden_replay = send(
        &app,
        bearer(
            json_request(&hidden_report_uri, hidden_report_payload),
            TG_TOKEN,
        ),
    )
    .await;
    assert_eq!(hidden_replay.status(), StatusCode::OK);
    assert_eq!(
        count(&pool, "SELECT COUNT(*) FROM reports").await,
        reports_before_hidden_replay
    );
    assert_eq!(
        count(&pool, "SELECT COUNT(*) FROM projection_outbox").await,
        events_before_hidden_replay
    );

    // Reply-report replay survives reply hide.
    let reply_target = fixture_thread(
        &pool,
        "engineering",
        "Reply report target",
        "visible",
        false,
        2,
        false,
    )
    .await;
    let hidden_reply = fixture_reply(&pool, reply_target, "hide", "visible", 1).await;
    let hidden_reply_uri = format!("/internal/telegram/replies/{hidden_reply}/reports");
    let hidden_reply_payload = report_body("reply-hide-principal", "reply-hide-key", "other", None);
    let hidden_reply_report = send(
        &app,
        bearer(
            json_request(&hidden_reply_uri, hidden_reply_payload.clone()),
            TG_TOKEN,
        ),
    )
    .await;
    assert_eq!(hidden_reply_report.status(), StatusCode::CREATED);
    let reports_before_reply_hidden_replay = count(&pool, "SELECT COUNT(*) FROM reports").await;
    let events_before_reply_hidden_replay =
        count(&pool, "SELECT COUNT(*) FROM projection_outbox").await;
    sqlx::query("UPDATE replies SET status = 'hidden' WHERE id = ?")
        .bind(hidden_reply as i64)
        .execute(&pool)
        .await
        .expect("hide reply");
    let hidden_reply_replay = send(
        &app,
        bearer(
            json_request(&hidden_reply_uri, hidden_reply_payload),
            TG_TOKEN,
        ),
    )
    .await;
    assert_eq!(hidden_reply_replay.status(), StatusCode::OK);
    assert_eq!(
        count(&pool, "SELECT COUNT(*) FROM reports").await,
        reports_before_reply_hidden_replay
    );
    assert_eq!(
        count(&pool, "SELECT COUNT(*) FROM projection_outbox").await,
        events_before_reply_hidden_replay
    );

    // Thread-create replay survives board archive.
    let archive_payload =
        thread_create_body("archive-principal", "archive-key", "Archive retry", "body");
    let archive_created = send(
        &app,
        bearer(json_request(thread_uri, archive_payload.clone()), TG_TOKEN),
    )
    .await;
    assert_eq!(archive_created.status(), StatusCode::CREATED);
    let archive_thread_id = response_json(archive_created).await["thread_id"]
        .as_u64()
        .expect("archive thread id");
    let threads_before_archive_replay = count(&pool, "SELECT COUNT(*) FROM threads").await;
    let events_before_archive_replay = count(&pool, "SELECT COUNT(*) FROM projection_outbox").await;
    sqlx::query("UPDATE boards SET status = 'archived' WHERE slug = 'engineering'")
        .execute(&pool)
        .await
        .expect("archive board");
    let archive_replay = send(
        &app,
        bearer(json_request(thread_uri, archive_payload), TG_TOKEN),
    )
    .await;
    assert_eq!(archive_replay.status(), StatusCode::OK);
    assert_eq!(
        response_json(archive_replay).await["thread_id"],
        serde_json::json!(archive_thread_id)
    );
    assert_eq!(
        count(&pool, "SELECT COUNT(*) FROM threads").await,
        threads_before_archive_replay
    );
    assert_eq!(
        count(&pool, "SELECT COUNT(*) FROM projection_outbox").await,
        events_before_archive_replay
    );
}
