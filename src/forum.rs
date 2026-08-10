pub(crate) struct Board {
    pub(crate) slug: String,
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) threads: Vec<Thread>,
}

pub(crate) struct Thread {
    pub(crate) id: u64,
    pub(crate) title: String,
    pub(crate) body: String,
    pub(crate) poster_id: String,
    pub(crate) replies: Vec<Reply>,
}

pub(crate) struct Reply {
    pub(crate) id: u64,
    pub(crate) body: String,
    pub(crate) poster_id: String,
}

pub(crate) struct ModerationReport {
    pub(crate) id: u64,
    pub(crate) target_id: u64,
    pub(crate) target_kind: String,
    pub(crate) thread_id: u64,
    pub(crate) board_slug: String,
    pub(crate) thread_title: String,
    pub(crate) body: String,
    pub(crate) reason: String,
    pub(crate) created_at: String,
}

#[derive(sqlx::FromRow)]
struct ModerationReportRow {
    id: u64,
    target_id: u64,
    target_kind: String,
    thread_id: u64,
    board_slug: String,
    thread_title: String,
    body: String,
    reason: String,
    created_at: String,
}

pub(crate) enum DismissReportResult {
    Applied,
    NotFound,
    AlreadyHandled,
}

#[derive(sqlx::FromRow)]
struct ReportStatusRow {
    thread_id: Option<u64>,
    reply_id: Option<u64>,
    status: String,
}

#[derive(sqlx::FromRow)]
struct BoardRow {
    slug: String,
    name: String,
    description: String,
}

#[derive(sqlx::FromRow)]
struct ThreadRow {
    id: u64,
    poster_id: String,
    title: String,
    body: String,
}

#[derive(sqlx::FromRow)]
struct ThreadPageRow {
    board_slug: String,
    board_name: String,
    board_description: String,
    thread_id: u64,
    poster_id: String,
    thread_title: String,
    thread_body: String,
}

#[derive(sqlx::FromRow)]
struct ReplyRow {
    id: u64,
    poster_id: String,
    body: String,
}

pub(crate) async fn load_approved_boards(
    pool: &sqlx::SqlitePool,
) -> Result<Vec<Board>, sqlx::Error> {
    let rows = sqlx::query_as::<_, BoardRow>(
        r#"
        SELECT slug, name, description
        FROM boards 
        WHERE status = 'approved' 
        ORDER BY name
        "#,
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| Board {
            slug: row.slug,
            name: row.name,
            description: row.description,
            threads: Vec::new(),
        })
        .collect())
}

pub(crate) async fn load_board(
    pool: &sqlx::SqlitePool,
    slug: &str,
) -> Result<Option<Board>, sqlx::Error> {
    let Some(board_row) = sqlx::query_as::<_, BoardRow>(
        r#"
        SELECT slug, name, description
        FROM boards
        WHERE slug = ? AND status = 'approved'
        "#,
    )
    .bind(slug)
    .fetch_optional(pool)
    .await?
    else {
        return Ok(None);
    };

    let threads = sqlx::query_as::<_, ThreadRow>(
        r#"
        SELECT t.id, t.title, t.body, t.poster_id
        FROM threads t
        JOIN boards b ON b.id = t.board_id
        WHERE b.slug = ?
        AND b.status = 'approved'
        AND t.status = 'visible'
        ORDER BY t.created_at DESC, t.id DESC
        "#,
    )
    .bind(slug)
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|thread| Thread {
        id: thread.id,
        title: thread.title,
        body: thread.body,
        poster_id: thread.poster_id,
        replies: Vec::new(),
    })
    .collect();

    Ok(Some(Board {
        slug: board_row.slug,
        name: board_row.name,
        description: board_row.description,
        threads,
    }))
}

pub(crate) async fn create_thread(
    pool: &sqlx::SqlitePool,
    board_slug: &str,
    title: &str,
    body: &str,
    anonymous_token: &str,
) -> Result<Option<u64>, sqlx::Error> {
    let mut transaction = pool.begin().await?;
    let Some(board_id) = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT id
        FROM boards
        WHERE slug = ? AND status = 'approved'
        "#,
    )
    .bind(board_slug)
    .fetch_optional(&mut *transaction)
    .await?
    else {
        return Ok(None);
    };

    let result = sqlx::query(
        r#"
        INSERT INTO threads (board_id, title, body, status)
        VALUES (?, ?, ?, 'visible')
        "#,
    )
    .bind(board_id)
    .bind(title)
    .bind(body)
    .execute(&mut *transaction)
    .await?;

    let thread_id = result.last_insert_rowid() as u64;
    let poster_id = crate::thread_poster_id(anonymous_token, thread_id);

    sqlx::query(
        r#"
        UPDATE threads
        SET poster_id = ?
        WHERE id = ?
        "#,
    )
    .bind(&poster_id)
    .bind(thread_id as i64)
    .execute(&mut *transaction)
    .await?;

    transaction.commit().await?;

    Ok(Some(thread_id))
}

pub(crate) async fn create_reply(
    pool: &sqlx::SqlitePool,
    thread_id: u64,
    body: &str,
    poster_id: &str,
) -> Result<bool, sqlx::Error> {
    let Some(_) = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT id 
        FROM threads 
        WHERE id = ? AND status = 'visible'
        "#,
    )
    .bind(thread_id as i64)
    .fetch_optional(pool)
    .await?
    else {
        return Ok(false);
    };

    sqlx::query(
        r#"
        INSERT INTO replies (thread_id, body, poster_id, status)
        VALUES (?, ?, ?, 'visible')
        "#,
    )
    .bind(thread_id as i64)
    .bind(body)
    .bind(poster_id)
    .execute(pool)
    .await?;

    Ok(true)
}

pub(crate) async fn report_thread(
    pool: &sqlx::SqlitePool,
    thread_id: u64,
    reason: &str,
) -> Result<bool, sqlx::Error> {
    let Some(_) = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT id
        FROM threads
        WHERE id = ?
          AND status IN ('visible', 'locked')
        "#,
    )
    .bind(thread_id as i64)
    .fetch_optional(pool)
    .await?
    else {
        return Ok(false);
    };

    sqlx::query(
        r#"
        INSERT INTO reports (thread_id, reason, status)
        VALUES (?, ?, 'pending')
        "#,
    )
    .bind(thread_id as i64)
    .bind(reason)
    .execute(pool)
    .await?;

    Ok(true)
}

pub(crate) async fn report_reply(
    pool: &sqlx::SqlitePool,
    reply_id: u64,
    reason: &str,
) -> Result<Option<u64>, sqlx::Error> {
    let Some(thread_id) = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT thread_id
        FROM replies
        WHERE id = ? AND status = 'visible'
        "#,
    )
    .bind(reply_id as i64)
    .fetch_optional(pool)
    .await?
    else {
        return Ok(None);
    };

    sqlx::query(
        r#"
        INSERT INTO reports (reply_id, reason, status)
        VALUES (?, ?, 'pending')
        "#,
    )
    .bind(reply_id as i64)
    .bind(reason)
    .execute(pool)
    .await?;

    Ok(Some(thread_id as u64))
}

pub(crate) async fn load_pending_reports(
    pool: &sqlx::SqlitePool,
) -> Result<Vec<ModerationReport>, sqlx::Error> {
    let rows = sqlx::query_as::<_, ModerationReportRow>(
        r#"
        SELECT
            r.id,
            CASE
                WHEN r.thread_id IS NOT NULL THEN r.thread_id
                ELSE r.reply_id
            END AS target_id,
            CASE
                WHEN r.thread_id IS NOT NULL THEN 'thread'
                ELSE 'reply'
            END AS target_kind,
            context_thread.id AS thread_id,
            b.slug AS board_slug,
            context_thread.title AS thread_title,
            CASE
                WHEN r.thread_id IS NOT NULL THEN reported_thread.body
                ELSE reported_reply.body
            END AS body,
            r.reason,
            r.created_at
        FROM reports r
        LEFT JOIN threads reported_thread
            ON reported_thread.id = r.thread_id
        LEFT JOIN replies reported_reply
            ON reported_reply.id = r.reply_id
        JOIN threads context_thread
            ON context_thread.id = COALESCE(
                r.thread_id,
                reported_reply.thread_id
            )
        JOIN boards b
            ON b.id = context_thread.board_id
        WHERE r.status = 'pending'
        ORDER BY r.created_at ASC, r.id ASC
        "#,
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| ModerationReport {
            id: row.id,
            target_id: row.target_id,
            target_kind: row.target_kind,
            thread_id: row.thread_id,
            board_slug: row.board_slug,
            thread_title: row.thread_title,
            body: row.body,
            reason: row.reason,
            created_at: row.created_at,
        })
        .collect())
}
pub(crate) async fn dismiss_report(
    pool: &sqlx::SqlitePool,
    report_id: u64,
    moderator_email: &str,
) -> Result<DismissReportResult, sqlx::Error> {
    let mut transaction = pool.begin().await?;

    let Some(report) = sqlx::query_as::<_, ReportStatusRow>(
        r#"
        SELECT thread_id, reply_id, status
        FROM reports
        WHERE id = ?
        "#,
    )
    .bind(report_id as i64)
    .fetch_optional(&mut *transaction)
    .await?
    else {
        return Ok(DismissReportResult::NotFound);
    };

    if report.status != "pending" {
        return Ok(DismissReportResult::AlreadyHandled);
    }

    let (target_kind, target_id) = if let Some(thread_id) = report.thread_id {
        ("thread", thread_id)
    } else if let Some(reply_id) = report.reply_id {
        ("reply", reply_id)
    } else {
        return Ok(DismissReportResult::NotFound);
    };

    let update = sqlx::query(
        r#"
        UPDATE reports
        SET status = 'dismissed'
        WHERE id = ?
          AND status = 'pending'
        "#,
    )
    .bind(report_id as i64)
    .execute(&mut *transaction)
    .await?;

    if update.rows_affected() != 1 {
        return Ok(DismissReportResult::AlreadyHandled);
    }

    sqlx::query(
        r#"
        INSERT INTO moderation_actions (
            report_id,
            moderator_email,
            action,
            target_kind,
            target_id
        )
        VALUES (?, ?, 'dismiss', ?, ?)
        "#,
    )
    .bind(report_id as i64)
    .bind(moderator_email)
    .bind(target_kind)
    .bind(target_id as i64)
    .execute(&mut *transaction)
    .await?;

    transaction.commit().await?;

    Ok(DismissReportResult::Applied)
}
pub(crate) async fn load_thread(
    pool: &sqlx::SqlitePool,
    id: u64,
) -> Result<Option<(Board, Thread)>, sqlx::Error> {
    let Some(thread_row) = sqlx::query_as::<_, ThreadPageRow>(
        r#"
        SELECT
            b.slug AS board_slug,
            b.name AS board_name,
            b.description AS board_description,
            t.id AS thread_id,
            t.title AS thread_title,
            t.body AS thread_body,
            t.poster_id AS poster_id
        FROM threads t 
        JOIN boards b ON b.id = t.board_id
        WHERE t.id = ?
            AND t.status IN ('visible', "locked")
            AND b.status = 'approved'
        "#,
    )
    .bind(id as i64)
    .fetch_optional(pool)
    .await?
    else {
        return Ok(None);
    };

    let replies = sqlx::query_as::<_, ReplyRow>(
        r#"
        SELECT id, body, poster_id
        FROM replies 
        WHERE thread_id = ?
            AND status = 'visible'
        ORDER BY created_at, id
        "#,
    )
    .bind(id as i64)
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|reply| Reply {
        id: reply.id,
        body: reply.body,
        poster_id: reply.poster_id,
    })
    .collect();

    Ok(Some((
        Board {
            slug: thread_row.board_slug,
            name: thread_row.board_name,
            description: thread_row.board_description,
            threads: Vec::new(),
        },
        Thread {
            id: thread_row.thread_id,
            poster_id: thread_row.poster_id,
            title: thread_row.thread_title,
            body: thread_row.thread_body,
            replies,
        },
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[sqlx::test(migrations = "src/../migrations")]
    async fn dismiss_report_updates_status_and_audit(pool: sqlx::SqlitePool) {
        sqlx::query(
            r#"
            INSERT INTO reports (thread_id, reason, status)
            VALUES (1, 'spam', 'pending')
            "#,
        )
        .execute(&pool)
        .await
        .unwrap();

        let result = dismiss_report(&pool, 1, "mod@example.com").await.unwrap();
        assert!(matches!(result, DismissReportResult::Applied));

        let status = sqlx::query_scalar::<_, String>("SELECT status FROM reports WHERE id = 1")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(status, "dismissed");

        let audit = sqlx::query_as::<_, (String, String, String, i64)>(
            r#"
            SELECT moderator_email, action, target_kind, target_id
            FROM moderation_actions
            WHERE report_id = 1
            "#,
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(
            audit,
            (
                String::from("mod@example.com"),
                String::from("dismiss"),
                String::from("thread"),
                1,
            )
        );

        let second_result = dismiss_report(&pool, 1, "mod@example.com").await.unwrap();
        assert!(matches!(second_result, DismissReportResult::AlreadyHandled));

        let audit_count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM moderation_actions WHERE report_id = 1",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(audit_count, 1);
    }
}
