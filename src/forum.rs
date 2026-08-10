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
    pub(crate) is_locked: bool,
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
    pub(crate) can_ban: bool,
}

#[derive(sqlx::FromRow)]
pub(crate) struct ActiveBan {
    pub(crate) scope: String,
    pub(crate) expires_at: String,
}

#[derive(sqlx::FromRow)]
pub(crate) struct EncryptedAbuseLog {
    pub(crate) target_kind: String,
    pub(crate) target_id: u64,
    pub(crate) nonce: Vec<u8>,
    pub(crate) ciphertext: Vec<u8>,
    pub(crate) created_at: String,
    pub(crate) retain_until: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ModerationAction {
    Dismiss,
    Resolve,
    Hide,
    Remove,
    Quarantine,
    Lock,
}

impl ModerationAction {
    pub(crate) fn parse(value: &str) -> Option<Self> {
        match value {
            "dismiss" => Some(Self::Dismiss),
            "resolve" => Some(Self::Resolve),
            "hide" => Some(Self::Hide),
            "remove" => Some(Self::Remove),
            "quarantine" => Some(Self::Quarantine),
            "lock" => Some(Self::Lock),
            _ => None,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Dismiss => "dismiss",
            Self::Resolve => "resolve",
            Self::Hide => "hide",
            Self::Remove => "remove",
            Self::Quarantine => "quarantine",
            Self::Lock => "lock",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BanScope {
    Board,
    Site,
}

impl BanScope {
    fn as_str(self) -> &'static str {
        match self {
            Self::Board => "board",
            Self::Site => "site",
        }
    }

    fn audit_action(self) -> &'static str {
        match self {
            Self::Board => "ban_board",
            Self::Site => "ban_site",
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum ModerationResult {
    Applied,
    NotFound,
    AlreadyHandled,
    InvalidTarget,
    MissingOrigin,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum CreateReplyResult {
    Created,
    NotFound,
    Locked,
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
    can_ban: bool,
}

#[derive(sqlx::FromRow)]
struct ReportStatusRow {
    thread_id: Option<u64>,
    reply_id: Option<u64>,
    status: String,
}

#[derive(sqlx::FromRow)]
struct BanReportRow {
    thread_id: Option<u64>,
    reply_id: Option<u64>,
    status: String,
    reason: String,
    board_id: u64,
    client_fingerprint: Option<Vec<u8>>,
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
    status: String,
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
    thread_status: String,
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
        SELECT t.id, t.title, t.body, t.poster_id, t.status
        FROM threads t
        JOIN boards b ON b.id = t.board_id
        WHERE b.slug = ?
        AND b.status = 'approved'
        AND t.status IN ('visible', 'locked')
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
        is_locked: thread.status == "locked",
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
    origin: &crate::abuse::ProtectedClient,
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

    sqlx::query(
        r#"
        INSERT INTO post_origins (
            thread_id,
            client_fingerprint,
            nonce,
            ciphertext
        )
        VALUES (?, ?, ?, ?)
        "#,
    )
    .bind(thread_id as i64)
    .bind(origin.fingerprint.as_slice())
    .bind(origin.nonce.as_slice())
    .bind(&origin.ciphertext)
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
    origin: &crate::abuse::ProtectedClient,
) -> Result<CreateReplyResult, sqlx::Error> {
    let mut transaction = pool.begin().await?;
    let Some(status) = sqlx::query_scalar::<_, String>(
        r#"
        SELECT status
        FROM threads
        WHERE id = ?
        "#,
    )
    .bind(thread_id as i64)
    .fetch_optional(&mut *transaction)
    .await?
    else {
        return Ok(CreateReplyResult::NotFound);
    };

    if status == "locked" {
        return Ok(CreateReplyResult::Locked);
    }

    if status != "visible" {
        return Ok(CreateReplyResult::NotFound);
    }

    let result = sqlx::query(
        r#"
        INSERT INTO replies (thread_id, body, poster_id, status)
        VALUES (?, ?, ?, 'visible')
        "#,
    )
    .bind(thread_id as i64)
    .bind(body)
    .bind(poster_id)
    .execute(&mut *transaction)
    .await?;

    let reply_id = result.last_insert_rowid();

    sqlx::query(
        r#"
        INSERT INTO post_origins (
            reply_id,
            client_fingerprint,
            nonce,
            ciphertext
        )
        VALUES (?, ?, ?, ?)
        "#,
    )
    .bind(reply_id)
    .bind(origin.fingerprint.as_slice())
    .bind(origin.nonce.as_slice())
    .bind(&origin.ciphertext)
    .execute(&mut *transaction)
    .await?;

    transaction.commit().await?;

    Ok(CreateReplyResult::Created)
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
            r.created_at,
            EXISTS(
                SELECT 1
                FROM post_origins po
                WHERE po.retain_until > CURRENT_TIMESTAMP
                  AND (po.thread_id = r.thread_id
                       OR po.reply_id = r.reply_id)
            ) AS can_ban
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
            can_ban: row.can_ban,
        })
        .collect())
}

pub(crate) async fn apply_moderation_action(
    pool: &sqlx::SqlitePool,
    report_id: u64,
    moderator_email: &str,
    action: ModerationAction,
) -> Result<ModerationResult, sqlx::Error> {
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
        return Ok(ModerationResult::NotFound);
    };

    if report.status != "pending" {
        return Ok(ModerationResult::AlreadyHandled);
    }

    let (target_kind, target_id) = if let Some(thread_id) = report.thread_id {
        ("thread", thread_id)
    } else if let Some(reply_id) = report.reply_id {
        ("reply", reply_id)
    } else {
        return Ok(ModerationResult::NotFound);
    };

    if action == ModerationAction::Lock && target_kind != "thread" {
        return Ok(ModerationResult::InvalidTarget);
    }

    let target_updated = match action {
        ModerationAction::Hide | ModerationAction::Remove | ModerationAction::Quarantine => {
            let result = if target_kind == "thread" {
                sqlx::query(
                    "UPDATE threads SET status = 'hidden' WHERE id = ? AND status IN ('visible', 'locked')",
                )
                .bind(target_id as i64)
                .execute(&mut *transaction)
                .await?
            } else {
                sqlx::query(
                    "UPDATE replies SET status = 'hidden' WHERE id = ? AND status = 'visible'",
                )
                .bind(target_id as i64)
                .execute(&mut *transaction)
                .await?
            };
            Some(result.rows_affected())
        }
        ModerationAction::Lock => {
            let result = sqlx::query(
                "UPDATE threads SET status = 'locked' WHERE id = ? AND status = 'visible'",
            )
            .bind(target_id as i64)
            .execute(&mut *transaction)
            .await?;
            Some(result.rows_affected())
        }
        ModerationAction::Dismiss | ModerationAction::Resolve => None,
    };

    if target_updated != Some(1) && target_updated.is_some() {
        let result = if action == ModerationAction::Lock {
            ModerationResult::InvalidTarget
        } else {
            ModerationResult::NotFound
        };
        return Ok(result);
    }

    let report_status = if action == ModerationAction::Dismiss {
        "dismissed"
    } else {
        "resolved"
    };
    let update = sqlx::query(
        r#"
        UPDATE reports
        SET status = ?
        WHERE id = ?
          AND status = 'pending'
        "#,
    )
    .bind(report_status)
    .bind(report_id as i64)
    .execute(&mut *transaction)
    .await?;

    if update.rows_affected() != 1 {
        return Ok(ModerationResult::AlreadyHandled);
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
        VALUES (?, ?, ?, ?, ?)
        "#,
    )
    .bind(report_id as i64)
    .bind(moderator_email)
    .bind(action.as_str())
    .bind(target_kind)
    .bind(target_id as i64)
    .execute(&mut *transaction)
    .await?;

    transaction.commit().await?;

    Ok(ModerationResult::Applied)
}

pub(crate) async fn apply_ban(
    pool: &sqlx::SqlitePool,
    report_id: u64,
    moderator_email: &str,
    scope: BanScope,
    days: u32,
) -> Result<ModerationResult, sqlx::Error> {
    let mut transaction = pool.begin().await?;

    let Some(report) = sqlx::query_as::<_, BanReportRow>(
        r#"
        SELECT
            r.thread_id,
            r.reply_id,
            r.status,
            r.reason,
            context_thread.board_id,
            po.client_fingerprint
        FROM reports r
        LEFT JOIN replies reported_reply
            ON reported_reply.id = r.reply_id
        JOIN threads context_thread
            ON context_thread.id = COALESCE(
                r.thread_id,
                reported_reply.thread_id
            )
        LEFT JOIN post_origins po
            ON po.retain_until > CURRENT_TIMESTAMP
           AND (po.thread_id = r.thread_id
                OR po.reply_id = r.reply_id)
        WHERE r.id = ?
        "#,
    )
    .bind(report_id as i64)
    .fetch_optional(&mut *transaction)
    .await?
    else {
        return Ok(ModerationResult::NotFound);
    };

    if report.status != "pending" {
        return Ok(ModerationResult::AlreadyHandled);
    }

    let Some(client_fingerprint) = report.client_fingerprint else {
        return Ok(ModerationResult::MissingOrigin);
    };

    let (target_kind, target_id) = if let Some(thread_id) = report.thread_id {
        ("thread", thread_id)
    } else if let Some(reply_id) = report.reply_id {
        ("reply", reply_id)
    } else {
        return Ok(ModerationResult::NotFound);
    };

    let board_id = match scope {
        BanScope::Board => Some(report.board_id as i64),
        BanScope::Site => None,
    };
    let max_days = match scope {
        BanScope::Board => 30,
        BanScope::Site => 365,
    };
    let duration = format!("+{} days", days.min(max_days));

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
        VALUES (?, ?, ?, ?, ?, ?, datetime('now', ?))
        "#,
    )
    .bind(&client_fingerprint)
    .bind(scope.as_str())
    .bind(board_id)
    .bind(report_id as i64)
    .bind(moderator_email)
    .bind(&report.reason)
    .bind(duration)
    .execute(&mut *transaction)
    .await?;

    let update = sqlx::query(
        r#"
        UPDATE reports
        SET status = 'resolved'
        WHERE id = ?
          AND status = 'pending'
        "#,
    )
    .bind(report_id as i64)
    .execute(&mut *transaction)
    .await?;

    if update.rows_affected() != 1 {
        return Ok(ModerationResult::AlreadyHandled);
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
        VALUES (?, ?, ?, ?, ?)
        "#,
    )
    .bind(report_id as i64)
    .bind(moderator_email)
    .bind(scope.audit_action())
    .bind(target_kind)
    .bind(target_id as i64)
    .execute(&mut *transaction)
    .await?;

    transaction.commit().await?;

    Ok(ModerationResult::Applied)
}

async fn load_active_ban(
    pool: &sqlx::SqlitePool,
    client_fingerprint: &[u8],
    board_id: i64,
) -> Result<Option<ActiveBan>, sqlx::Error> {
    sqlx::query_as::<_, ActiveBan>(
        r#"
        SELECT scope, expires_at
        FROM bans
        WHERE client_fingerprint = ?
          AND revoked_at IS NULL
          AND expires_at > CURRENT_TIMESTAMP
          AND (
            scope = 'site'
            OR (scope = 'board' AND board_id = ?)
          )
        ORDER BY
          CASE WHEN scope = 'site' THEN 0 ELSE 1 END,
          expires_at DESC
        LIMIT 1
        "#,
    )
    .bind(client_fingerprint)
    .bind(board_id)
    .fetch_optional(pool)
    .await
}

pub(crate) async fn load_active_ban_for_board(
    pool: &sqlx::SqlitePool,
    client_fingerprint: &[u8],
    board_slug: &str,
) -> Result<Option<ActiveBan>, sqlx::Error> {
    let Some(board_id) = sqlx::query_scalar::<_, i64>(
        "SELECT id FROM boards WHERE slug = ? AND status = 'approved'",
    )
    .bind(board_slug)
    .fetch_optional(pool)
    .await?
    else {
        return Ok(None);
    };

    load_active_ban(pool, client_fingerprint, board_id).await
}

pub(crate) async fn load_active_ban_for_thread(
    pool: &sqlx::SqlitePool,
    client_fingerprint: &[u8],
    thread_id: u64,
) -> Result<Option<ActiveBan>, sqlx::Error> {
    let Some(board_id) = sqlx::query_scalar::<_, i64>("SELECT board_id FROM threads WHERE id = ?")
        .bind(thread_id as i64)
        .fetch_optional(pool)
        .await?
    else {
        return Ok(None);
    };

    load_active_ban(pool, client_fingerprint, board_id).await
}

pub(crate) async fn load_abuse_logs(
    pool: &sqlx::SqlitePool,
) -> Result<Vec<EncryptedAbuseLog>, sqlx::Error> {
    sqlx::query_as::<_, EncryptedAbuseLog>(
        r#"
        SELECT
            CASE
                WHEN thread_id IS NOT NULL THEN 'thread'
                ELSE 'reply'
            END AS target_kind,
            CASE
                WHEN thread_id IS NOT NULL THEN thread_id
                ELSE reply_id
            END AS target_id,
            nonce,
            ciphertext,
            created_at,
            retain_until
        FROM post_origins
        WHERE retain_until > CURRENT_TIMESTAMP
        ORDER BY created_at DESC, id DESC
        LIMIT 100
        "#,
    )
    .fetch_all(pool)
    .await
}

pub(crate) async fn record_abuse_log_access(
    pool: &sqlx::SqlitePool,
    moderator_email: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query("INSERT INTO abuse_log_accesses (moderator_email) VALUES (?)")
        .bind(moderator_email)
        .execute(pool)
        .await?;

    Ok(())
}

pub(crate) async fn purge_expired_abuse_logs(pool: &sqlx::SqlitePool) -> Result<u64, sqlx::Error> {
    let result = sqlx::query("DELETE FROM post_origins WHERE retain_until <= CURRENT_TIMESTAMP")
        .execute(pool)
        .await?;

    Ok(result.rows_affected())
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
            t.poster_id AS poster_id,
            t.status AS thread_status
        FROM threads t 
        JOIN boards b ON b.id = t.board_id
        WHERE t.id = ?
            AND t.status IN ('visible', 'locked')
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
            is_locked: thread_row.thread_status == "locked",
            replies,
        },
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!();

    #[test]
    fn parses_all_moderation_actions() {
        for action in ["dismiss", "resolve", "hide", "remove", "quarantine", "lock"] {
            assert!(ModerationAction::parse(action).is_some());
        }
        assert!(ModerationAction::parse("ban-board").is_none());
        assert!(ModerationAction::parse("unknown").is_none());
    }

    #[sqlx::test(migrator = "MIGRATOR")]
    async fn moderation_action_updates_status_and_audits_once(pool: sqlx::SqlitePool) {
        sqlx::query(
            r#"
            INSERT INTO reports (thread_id, reason, status)
            VALUES (1, 'spam', 'pending')
            "#,
        )
        .execute(&pool)
        .await
        .unwrap();

        let result =
            apply_moderation_action(&pool, 1, "mod@example.com", ModerationAction::Dismiss)
                .await
                .unwrap();
        assert_eq!(result, ModerationResult::Applied);

        let status = sqlx::query_scalar::<_, String>("SELECT status FROM reports WHERE id = 1")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(status, "dismissed");

        let action = sqlx::query_scalar::<_, String>(
            "SELECT action FROM moderation_actions WHERE report_id = 1",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(action, "dismiss");

        let second_result =
            apply_moderation_action(&pool, 1, "mod@example.com", ModerationAction::Resolve)
                .await
                .unwrap();
        assert_eq!(second_result, ModerationResult::AlreadyHandled);

        let audit_count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM moderation_actions WHERE report_id = 1",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(audit_count, 1);
    }

    #[sqlx::test(migrator = "MIGRATOR")]
    async fn lock_is_only_valid_for_thread_reports(pool: sqlx::SqlitePool) {
        sqlx::query(
            r#"
            INSERT INTO reports (reply_id, reason, status)
            VALUES (1, 'spam', 'pending')
            "#,
        )
        .execute(&pool)
        .await
        .unwrap();

        let result = apply_moderation_action(&pool, 1, "mod@example.com", ModerationAction::Lock)
            .await
            .unwrap();
        assert_eq!(result, ModerationResult::InvalidTarget);

        let status = sqlx::query_scalar::<_, String>("SELECT status FROM reports WHERE id = 1")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(status, "pending");
    }

    #[sqlx::test(migrator = "MIGRATOR")]
    async fn locked_threads_can_be_hidden_but_hidden_threads_cannot_be_locked(
        pool: sqlx::SqlitePool,
    ) {
        let thread_id = sqlx::query(
            r#"
            INSERT INTO threads (board_id, title, body, status)
            VALUES (
                (SELECT id FROM boards WHERE slug = 'engineering'),
                'Locked moderation fixture',
                'A locked target reserved for moderation transition coverage.',
                'locked'
            )
            "#,
        )
        .execute(&pool)
        .await
        .unwrap()
        .last_insert_rowid() as u64;

        let hide_report_id = sqlx::query(
            r#"
            INSERT INTO reports (thread_id, reason, status)
            VALUES (?, 'spam', 'pending')
            "#,
        )
        .bind(thread_id as i64)
        .execute(&pool)
        .await
        .unwrap()
        .last_insert_rowid() as u64;

        assert_eq!(
            apply_moderation_action(
                &pool,
                hide_report_id,
                "mod@example.com",
                ModerationAction::Hide
            )
            .await
            .unwrap(),
            ModerationResult::Applied
        );

        let thread_status =
            sqlx::query_scalar::<_, String>("SELECT status FROM threads WHERE id = ?")
                .bind(thread_id as i64)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(thread_status, "hidden");
        assert!(load_thread(&pool, thread_id).await.unwrap().is_none());

        let hide_report_status =
            sqlx::query_scalar::<_, String>("SELECT status FROM reports WHERE id = ?")
                .bind(hide_report_id as i64)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(hide_report_status, "resolved");

        let hide_audit_action = sqlx::query_scalar::<_, String>(
            "SELECT action FROM moderation_actions WHERE report_id = ?",
        )
        .bind(hide_report_id as i64)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(hide_audit_action, "hide");

        let lock_report_id = sqlx::query(
            "INSERT INTO reports (thread_id, reason, status) VALUES (?, 'spam', 'pending')",
        )
        .bind(thread_id as i64)
        .execute(&pool)
        .await
        .unwrap()
        .last_insert_rowid() as u64;

        assert_eq!(
            apply_moderation_action(
                &pool,
                lock_report_id,
                "mod@example.com",
                ModerationAction::Lock
            )
            .await
            .unwrap(),
            ModerationResult::InvalidTarget
        );

        let lock_report_status =
            sqlx::query_scalar::<_, String>("SELECT status FROM reports WHERE id = ?")
                .bind(lock_report_id as i64)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(lock_report_status, "pending");

        let lock_audit_count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM moderation_actions WHERE report_id = ?",
        )
        .bind(lock_report_id as i64)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(lock_audit_count, 0);

        let final_thread_status =
            sqlx::query_scalar::<_, String>("SELECT status FROM threads WHERE id = ?")
                .bind(thread_id as i64)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(final_thread_status, "hidden");
    }

    #[sqlx::test(migrator = "MIGRATOR")]
    async fn expired_origins_cannot_authorize_bans(pool: sqlx::SqlitePool) {
        let fingerprint = vec![7_u8; 32];
        let nonce = vec![8_u8; 12];
        let ciphertext = vec![9_u8; 24];

        sqlx::query(
            r#"
            INSERT INTO reports (thread_id, reason, status)
            VALUES (1, 'spam', 'pending')
            "#,
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            r#"
            INSERT INTO post_origins (
                thread_id, client_fingerprint, nonce, ciphertext, retain_until
            )
            VALUES (1, ?, ?, ?, datetime('now', '-1 day'))
            "#,
        )
        .bind(&fingerprint)
        .bind(&nonce)
        .bind(&ciphertext)
        .execute(&pool)
        .await
        .unwrap();

        assert_eq!(
            apply_ban(&pool, 1, "mod@example.com", BanScope::Board, 7)
                .await
                .unwrap(),
            ModerationResult::MissingOrigin
        );

        let report_status =
            sqlx::query_scalar::<_, String>("SELECT status FROM reports WHERE id = 1")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(report_status, "pending");

        let ban_count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM bans")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(ban_count, 0);

        let audit_count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM moderation_actions WHERE report_id = 1",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(audit_count, 0);
    }

    #[sqlx::test(migrator = "MIGRATOR")]
    async fn bans_use_stored_origins_and_retention_is_thirty_days(pool: sqlx::SqlitePool) {
        let fingerprint = vec![7_u8; 32];
        let nonce = vec![8_u8; 12];
        let ciphertext = vec![9_u8; 24];

        sqlx::query(
            r#"
            INSERT INTO reports (thread_id, reason, status)
            VALUES (1, 'spam', 'pending')
            "#,
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            r#"
            INSERT INTO post_origins (
                thread_id, client_fingerprint, nonce, ciphertext
            )
            VALUES (1, ?, ?, ?)
            "#,
        )
        .bind(&fingerprint)
        .bind(&nonce)
        .bind(&ciphertext)
        .execute(&pool)
        .await
        .unwrap();

        assert_eq!(
            apply_ban(&pool, 1, "mod@example.com", BanScope::Board, 31)
                .await
                .unwrap(),
            ModerationResult::Applied
        );
        let board_ban = sqlx::query_as::<_, (String, i64)>(
            "SELECT scope, CAST((julianday(expires_at) - julianday('now')) * 86400 AS INTEGER) FROM bans",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(board_ban.0, "board");
        assert!(board_ban.1 <= 30 * 86400);

        sqlx::query(
            r#"
            INSERT INTO reports (thread_id, reason, status)
            VALUES (2, 'spam', 'pending')
            "#,
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            r#"
            INSERT INTO post_origins (
                thread_id, client_fingerprint, nonce, ciphertext
            )
            VALUES (2, ?, ?, ?)
            "#,
        )
        .bind(&fingerprint)
        .bind(&nonce)
        .bind(&ciphertext)
        .execute(&pool)
        .await
        .unwrap();

        assert_eq!(
            apply_ban(&pool, 2, "mod@example.com", BanScope::Site, 366)
                .await
                .unwrap(),
            ModerationResult::Applied
        );
        let site_ban = sqlx::query_as::<_, (String, Option<i64>)>(
            "SELECT scope, board_id FROM bans WHERE scope = 'site'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(site_ban, (String::from("site"), None));

        sqlx::query(
            r#"
            INSERT INTO post_origins (
                reply_id, client_fingerprint, nonce, ciphertext, retain_until
            )
            VALUES (1, ?, ?, ?, datetime('now', '-1 day'))
            "#,
        )
        .bind(&fingerprint)
        .bind(&nonce)
        .bind(&ciphertext)
        .execute(&pool)
        .await
        .unwrap();
        assert_eq!(purge_expired_abuse_logs(&pool).await.unwrap(), 1);
    }
}
