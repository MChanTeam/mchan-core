use super::*;

static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!();

fn origin(seed: u8) -> crate::abuse::ProtectedClient {
    crate::abuse::ProtectedClient {
        fingerprint: [seed; 32],
        nonce: [seed; 12],
        ciphertext: vec![seed; 24],
    }
}

fn request_hash(seed: u8) -> [u8; 32] {
    [seed; 32]
}

async fn fixture_thread(
    pool: &sqlx::SqlitePool,
    board_slug: &str,
    title: &str,
    status: &str,
    created_at: &str,
    bumped_at: &str,
    is_pinned: bool,
) -> u64 {
    sqlx::query(
        "INSERT INTO threads (board_id, title, body, status, created_at, bumped_at, is_pinned) VALUES ((SELECT id FROM boards WHERE slug = ?), ?, ?, ?, ?, ?, ?)",
    )
    .bind(board_slug)
    .bind(title)
    .bind(format!("body for {title}"))
    .bind(status)
    .bind(created_at)
    .bind(bumped_at)
    .bind(is_pinned)
    .execute(pool)
    .await
    .unwrap()
    .last_insert_rowid() as u64
}

async fn fixture_reply(
    pool: &sqlx::SqlitePool,
    thread_id: u64,
    body: &str,
    status: &str,
    created_at: &str,
) -> u64 {
    sqlx::query(
        "INSERT INTO replies (thread_id, body, poster_id, status, created_at) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(thread_id as i64)
    .bind(body)
    .bind("fixture-poster")
    .bind(status)
    .bind(created_at)
    .execute(pool)
    .await
    .unwrap()
    .last_insert_rowid() as u64
}

async fn clear_outbox(pool: &sqlx::SqlitePool) {
    sqlx::query("DELETE FROM projection_outbox")
        .execute(pool)
        .await
        .unwrap();
}

async fn event_rows(
    pool: &sqlx::SqlitePool,
) -> Vec<(String, Option<i64>, Option<i64>, Option<i64>)> {
    sqlx::query_as("SELECT kind, thread_id, reply_id, report_id FROM projection_outbox ORDER BY id")
        .fetch_all(pool)
        .await
        .unwrap()
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn idempotent_thread_reply_report_replay_preserves_ids_content_and_events(
    pool: sqlx::SqlitePool,
) {
    let thread_hash = request_hash(1);
    let thread_key = (
        "telegram",
        "thread.create",
        "thread-1",
        thread_hash.as_slice(),
    );
    let thread_origin = origin(1);
    let created = create_thread_idempotent(
        &pool,
        "engineering",
        "telegram idempotent thread",
        "one canonical body",
        &thread_origin,
        None,
        thread_key,
    )
    .await
    .unwrap();
    let thread_id = match created {
        IdempotentMutation::Created(id) => id,
        other => panic!("unexpected first thread result: {other:?}"),
    };
    assert_eq!(
        create_thread_idempotent(
            &pool,
            "engineering",
            "telegram idempotent thread",
            "one canonical body",
            &thread_origin,
            None,
            thread_key,
        )
        .await
        .unwrap(),
        IdempotentMutation::Replayed(thread_id)
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM threads WHERE id = ? AND title = ? AND body = ?",
        )
        .bind(thread_id as i64)
        .bind("telegram idempotent thread")
        .bind("one canonical body")
        .fetch_one(&pool)
        .await
        .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM projection_outbox WHERE kind = 'thread_created' AND thread_id = ?",
        )
        .bind(thread_id as i64)
        .fetch_one(&pool)
        .await
        .unwrap(),
        1
    );

    let reply_hash = request_hash(2);
    let reply_key = ("telegram", "reply.create", "reply-1", reply_hash.as_slice());
    let reply_origin = origin(2);
    let reply_id = match create_reply_idempotent(
        &pool,
        thread_id,
        "one canonical reply",
        &reply_origin,
        None,
        reply_key,
    )
    .await
    .unwrap()
    {
        IdempotentMutation::Created(id) => id,
        other => panic!("unexpected first reply result: {other:?}"),
    };
    assert_eq!(
        create_reply_idempotent(
            &pool,
            thread_id,
            "one canonical reply",
            &reply_origin,
            None,
            reply_key,
        )
        .await
        .unwrap(),
        IdempotentMutation::Replayed(reply_id)
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM replies WHERE id = ? AND body = ?",)
            .bind(reply_id as i64)
            .bind("one canonical reply")
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM projection_outbox WHERE kind = 'thread_dirty' AND thread_id = ?",
        )
        .bind(thread_id as i64)
        .fetch_one(&pool)
        .await
        .unwrap(),
        1
    );

    let report_hash = request_hash(3);
    let report_key = (
        "telegram",
        "thread.report",
        "report-1",
        report_hash.as_slice(),
    );
    let report = report_thread_idempotent(
        &pool,
        thread_id,
        "spam",
        Some("duplicate report payload"),
        report_key,
    )
    .await
    .unwrap();
    let (report_id, reported_thread_id) = match report {
        IdempotentMutation::Created(pair) => pair,
        other => panic!("unexpected first report result: {other:?}"),
    };
    assert_eq!(reported_thread_id, thread_id);
    assert_eq!(
        report_thread_idempotent(
            &pool,
            thread_id,
            "spam",
            Some("duplicate report payload"),
            report_key,
        )
        .await
        .unwrap(),
        IdempotentMutation::Replayed((report_id, thread_id))
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM reports WHERE id = ? AND reason = ? AND details = ?",
        )
        .bind(report_id as i64)
        .bind("spam")
        .bind("duplicate report payload")
        .fetch_one(&pool)
        .await
        .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM projection_outbox WHERE kind = 'report_created' AND report_id = ?",
        )
        .bind(report_id as i64)
        .fetch_one(&pool)
        .await
        .unwrap(),
        1
    );
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn concurrent_duplicate_idempotency_creates_one_thread_and_one_event(pool: sqlx::SqlitePool) {
    let hash = request_hash(7);
    let key = ("telegram", "thread.create", "concurrent-1", hash.as_slice());
    let request = async {
        create_thread_idempotent(
            &pool,
            "engineering",
            "concurrent duplicate",
            "single body",
            &origin(7),
            None,
            key,
        )
        .await
        .unwrap()
    };
    let (left, right) = tokio::join!(request, async {
        create_thread_idempotent(
            &pool,
            "engineering",
            "concurrent duplicate",
            "single body",
            &origin(7),
            None,
            key,
        )
        .await
        .unwrap()
    });
    let one_created = matches!(&left, IdempotentMutation::Created(_));
    let two_created = matches!(&right, IdempotentMutation::Created(_));
    let ids = [left, right]
        .into_iter()
        .map(|result| match result {
            IdempotentMutation::Created(id) | IdempotentMutation::Replayed(id) => id,
            IdempotentMutation::Conflict => panic!("concurrent request conflicted"),
        })
        .collect::<Vec<_>>();
    assert_eq!(ids[0], ids[1]);
    assert!(one_created ^ two_created);
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM threads WHERE title = 'concurrent duplicate'",
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM projection_outbox WHERE kind = 'thread_created' AND thread_id = ?",
        )
        .bind(ids[0] as i64)
        .fetch_one(&pool)
        .await
        .unwrap(),
        1
    );
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn idempotency_hash_mismatch_is_conflict_without_new_object_or_event(pool: sqlx::SqlitePool) {
    let first_hash = request_hash(9);
    let first_key = (
        "telegram",
        "thread.create",
        "hash-conflict",
        first_hash.as_slice(),
    );
    let first_id = match create_thread_idempotent(
        &pool,
        "engineering",
        "hash conflict thread",
        "original body",
        &origin(9),
        None,
        first_key,
    )
    .await
    .unwrap()
    {
        IdempotentMutation::Created(id) => id,
        other => panic!("unexpected first result: {other:?}"),
    };
    let second_hash = request_hash(10);
    let second_key = (
        "telegram",
        "thread.create",
        "hash-conflict",
        second_hash.as_slice(),
    );
    assert_eq!(
        create_thread_idempotent(
            &pool,
            "engineering",
            "hash conflict thread",
            "changed body",
            &origin(10),
            None,
            second_key,
        )
        .await
        .unwrap(),
        IdempotentMutation::Conflict
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM threads WHERE title = 'hash conflict thread'",
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT body FROM threads WHERE id = ?")
            .bind(first_id as i64)
            .fetch_one(&pool)
            .await
            .unwrap(),
        "original body"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM projection_outbox WHERE kind = 'thread_created' AND thread_id = ?",
        )
        .bind(first_id as i64)
        .fetch_one(&pool)
        .await
        .unwrap(),
        1
    );
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn failed_idempotent_mutation_rolls_back_object_idempotency_and_outbox(
    pool: sqlx::SqlitePool,
) {
    let hash = request_hash(11);
    let key = ("telegram", "thread.create", "rollback-1", hash.as_slice());
    let invalid_media = Media {
        thumbnail_path: "/thumb/invalid.webp".to_owned(),
        display_path: "/media/invalid.webp".to_owned(),
        mime_type: "image/webp".to_owned(),
        width: 0,
        height: 1,
    };
    assert!(
        create_thread_idempotent(
            &pool,
            "engineering",
            "rolled back thread",
            "must not persist",
            &origin(11),
            Some(&invalid_media),
            key,
        )
        .await
        .is_err()
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM threads WHERE title = 'rolled back thread'",
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM machine_idempotency WHERE opaque_key = 'rollback-1'"
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM projection_outbox")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn public_snapshot_hides_hidden_content_and_returns_locked_paged_media_fields(
    pool: sqlx::SqlitePool,
) {
    let hidden_id = fixture_thread(
        &pool,
        "engineering",
        "hidden snapshot thread",
        "hidden",
        "2031-01-01 00:00:00",
        "2031-01-01 00:00:00",
        false,
    )
    .await;
    assert!(
        load_public_thread_snapshot(&pool, hidden_id, 2, 0)
            .await
            .unwrap()
            .is_none()
    );

    let thread_id = fixture_thread(
        &pool,
        "engineering",
        "locked snapshot thread",
        "locked",
        "2031-01-02 00:00:00",
        "2031-01-02 00:00:00",
        true,
    )
    .await;
    sqlx::query(
        "INSERT INTO post_media (thread_id, thumbnail_path, display_path, mime_type, width, height) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(thread_id as i64)
    .bind("/thumb/thread.webp")
    .bind("/media/thread.webp")
    .bind("image/webp")
    .bind(640_i64)
    .bind(480_i64)
    .execute(&pool)
    .await
    .unwrap();
    let reply_one = fixture_reply(
        &pool,
        thread_id,
        "visible one",
        "visible",
        "2031-01-02 00:00:01",
    )
    .await;
    let reply_two = fixture_reply(
        &pool,
        thread_id,
        "visible two",
        "visible",
        "2031-01-02 00:00:02",
    )
    .await;
    let reply_three = fixture_reply(
        &pool,
        thread_id,
        "visible three",
        "visible",
        "2031-01-02 00:00:03",
    )
    .await;
    let _hidden_reply =
        fixture_reply(&pool, thread_id, "secret", "hidden", "2031-01-02 00:00:04").await;
    sqlx::query(
        "INSERT INTO post_media (reply_id, thumbnail_path, display_path, mime_type, width, height) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(reply_two as i64)
    .bind("/thumb/reply.webp")
    .bind("/media/reply.webp")
    .bind("image/webp")
    .bind(320_i64)
    .bind(240_i64)
    .execute(&pool)
    .await
    .unwrap();

    let first_page = load_public_thread_snapshot(&pool, thread_id, 2, 0)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(first_page.id, thread_id);
    assert_eq!(first_page.title, "locked snapshot thread");
    assert_eq!(first_page.created_at, "2031-01-02 00:00:00");
    assert!(first_page.is_pinned);
    assert!(first_page.is_locked);
    assert_eq!(first_page.reply_count, 3);
    assert!(first_page.has_next_replies);
    let thread_media = first_page.media.as_ref().unwrap();
    assert_eq!(thread_media.thumbnail_path, "/thumb/thread.webp");
    assert_eq!(thread_media.display_path, "/media/thread.webp");
    assert_eq!(thread_media.mime_type, "image/webp");
    assert_eq!((thread_media.width, thread_media.height), (640, 480));
    assert_eq!(
        first_page
            .replies
            .iter()
            .map(|reply| reply.id)
            .collect::<Vec<_>>(),
        vec![reply_one, reply_two]
    );
    assert!(first_page.replies[1].media.is_some());

    let second_page = load_public_thread_snapshot(&pool, thread_id, 2, 2)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        second_page
            .replies
            .iter()
            .map(|reply| reply.id)
            .collect::<Vec<_>>(),
        vec![reply_three]
    );
    assert!(!second_page.has_next_replies);
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn pasum_backfill_uses_active_pin_bump_order_and_bounds(pool: sqlx::SqlitePool) {
    let pinned_old = fixture_thread(
        &pool,
        "pasum",
        "pasum pinned old",
        "visible",
        "2032-01-01 00:00:00",
        "2032-01-01 00:00:01",
        true,
    )
    .await;
    let unpinned_new = fixture_thread(
        &pool,
        "pasum",
        "pasum unpinned new",
        "visible",
        "2032-01-01 00:00:02",
        "2032-01-01 00:00:04",
        false,
    )
    .await;
    let unpinned_old = fixture_thread(
        &pool,
        "pasum",
        "pasum unpinned old",
        "visible",
        "2032-01-01 00:00:03",
        "2032-01-01 00:00:03",
        false,
    )
    .await;
    let locked = fixture_thread(
        &pool,
        "pasum",
        "pasum locked",
        "locked",
        "2032-01-01 00:00:04",
        "2032-01-01 00:00:02",
        false,
    )
    .await;
    let hidden = fixture_thread(
        &pool,
        "pasum",
        "pasum hidden",
        "hidden",
        "2032-01-01 00:00:05",
        "2032-01-01 00:00:05",
        true,
    )
    .await;
    let archived = fixture_thread(
        &pool,
        "pasum",
        "pasum archived",
        "visible",
        "2032-01-01 00:00:06",
        "2032-01-01 00:00:06",
        false,
    )
    .await;
    sqlx::query("UPDATE threads SET archived_at = '2032-01-02 00:00:00' WHERE id = ?")
        .bind(archived as i64)
        .execute(&pool)
        .await
        .unwrap();

    assert_eq!(
        load_active_board_thread_ids(&pool, "pasum", 100)
            .await
            .unwrap(),
        vec![pinned_old, unpinned_new, unpinned_old, locked]
    );
    assert_eq!(
        load_active_board_thread_ids(&pool, "pasum", 1)
            .await
            .unwrap(),
        vec![pinned_old]
    );
    assert!(
        !load_active_board_thread_ids(&pool, "pasum", 100)
            .await
            .unwrap()
            .contains(&hidden)
    );
    assert!(
        load_active_board_thread_ids(&pool, "pasum", 0)
            .await
            .is_err()
    );
    assert!(
        load_active_board_thread_ids(&pool, "pasum", 101)
            .await
            .is_err()
    );
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn mutation_outbox_covers_report_reply_hide_and_thread_removal_without_body_data(
    pool: sqlx::SqlitePool,
) {
    let thread_id = fixture_thread(
        &pool,
        "engineering",
        "outbox mutation thread",
        "visible",
        "2033-01-01 00:00:00",
        "2033-01-01 00:00:00",
        false,
    )
    .await;
    let reply_id = fixture_reply(
        &pool,
        thread_id,
        "outbox mutation reply",
        "visible",
        "2033-01-01 00:00:01",
    )
    .await;
    clear_outbox(&pool).await;

    assert!(
        report_thread(&pool, thread_id, "spam", Some("report details"))
            .await
            .unwrap()
    );
    let report_id = sqlx::query_scalar::<_, u64>(
        "SELECT id FROM reports WHERE thread_id = ? ORDER BY id DESC LIMIT 1",
    )
    .bind(thread_id as i64)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        apply_direct_hide(
            &pool,
            "reply",
            reply_id,
            "moderator@example.com",
            DirectHideReason::Spam,
            Some("hide reply"),
        )
        .await
        .unwrap(),
        DirectHideResult::Applied
    );
    assert_eq!(
        apply_direct_hide(
            &pool,
            "thread",
            thread_id,
            "moderator@example.com",
            DirectHideReason::BoardRule,
            Some("remove thread"),
        )
        .await
        .unwrap(),
        DirectHideResult::Applied
    );
    assert_eq!(
        event_rows(&pool).await,
        vec![
            (
                "report_created".to_owned(),
                Some(thread_id as i64),
                None,
                Some(report_id as i64)
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
        ]
    );
    let columns = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM pragma_table_info('projection_outbox') WHERE name IN ('body', 'principal')",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(columns, 0);
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn outbox_lease_ack_reclaim_and_purge_are_bounded_and_token_checked(pool: sqlx::SqlitePool) {
    let thread_id = fixture_thread(
        &pool,
        "engineering",
        "lease fixture",
        "visible",
        "2034-01-01 00:00:00",
        "2034-01-01 00:00:00",
        false,
    )
    .await;
    clear_outbox(&pool).await;
    for _ in 0..3 {
        sqlx::query("INSERT INTO projection_outbox (kind, thread_id) VALUES ('thread_dirty', ?)")
            .bind(thread_id as i64)
            .execute(&pool)
            .await
            .unwrap();
    }
    let first = lease_projection_outbox(&pool, 2, 60).await.unwrap();
    assert_eq!(first.len(), 2);
    assert!(!first[0].lease_token.is_empty());
    assert_eq!(first[0].lease_token, first[1].lease_token);
    assert_eq!(
        acknowledge_projection_outbox(&pool, first[0].id, "wrong-token")
            .await
            .unwrap(),
        OutboxAck::LeaseMismatch
    );
    assert_eq!(
        acknowledge_projection_outbox(&pool, first[0].id, &first[0].lease_token)
            .await
            .unwrap(),
        OutboxAck::Acknowledged
    );
    assert_eq!(
        acknowledge_projection_outbox(&pool, first[0].id, &first[0].lease_token)
            .await
            .unwrap(),
        OutboxAck::Acknowledged
    );

    sqlx::query(
        "UPDATE projection_outbox SET lease_expires_at = datetime('now', '-1 second') WHERE id = ?",
    )
    .bind(first[1].id as i64)
    .execute(&pool)
    .await
    .unwrap();
    let reclaimed = lease_projection_outbox(&pool, 2, 30).await.unwrap();
    assert_eq!(reclaimed.len(), 2);
    let reclaimed_second = reclaimed
        .iter()
        .find(|event| event.id == first[1].id)
        .unwrap();
    assert_ne!(reclaimed_second.lease_token, first[0].lease_token);
    assert_eq!(
        acknowledge_projection_outbox(&pool, reclaimed_second.id, &reclaimed_second.lease_token)
            .await
            .unwrap(),
        OutboxAck::Acknowledged
    );
    sqlx::query("UPDATE projection_outbox SET acknowledged_at = datetime('now', '-2 seconds') WHERE acknowledged_at IS NOT NULL")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        purge_acknowledged_projection_outbox(&pool, 1)
            .await
            .unwrap(),
        2
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM projection_outbox")
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn outbox_lease_limit_caps_at_one_hundred_and_rejects_nonpositive_limit(
    pool: sqlx::SqlitePool,
) {
    let thread_id = fixture_thread(
        &pool,
        "engineering",
        "lease bound fixture",
        "visible",
        "2035-01-01 00:00:00",
        "2035-01-01 00:00:00",
        false,
    )
    .await;
    clear_outbox(&pool).await;
    for _ in 0..101 {
        sqlx::query("INSERT INTO projection_outbox (kind, thread_id) VALUES ('thread_dirty', ?)")
            .bind(thread_id as i64)
            .execute(&pool)
            .await
            .unwrap();
    }
    assert!(lease_projection_outbox(&pool, 0, 60).await.is_err());
    let leased = lease_projection_outbox(&pool, 200, 86_401).await.unwrap();
    assert_eq!(leased.len(), 100);
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM projection_outbox WHERE lease_token IS NULL",
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        1
    );
}
