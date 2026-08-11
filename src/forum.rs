use sha2::{Digest, Sha256};
use std::{collections::HashMap, fmt};

pub(crate) struct Board {
    pub(crate) slug: String,
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) threads: Vec<Thread>,
}

pub(crate) struct Media {
    pub(crate) thumbnail_path: String,
    pub(crate) display_path: String,
    pub(crate) mime_type: String,
    pub(crate) width: u64,
    pub(crate) height: u64,
}

pub(crate) struct Thread {
    pub(crate) id: u64,
    pub(crate) title: String,
    pub(crate) body: String,
    pub(crate) poster_id: String,
    pub(crate) is_locked: bool,
    pub(crate) reply_count: u64,
    pub(crate) recent_replies: Vec<Reply>,
    pub(crate) replies: Vec<Reply>,
    pub(crate) media: Option<Media>,
    pub(crate) is_archived: bool,
}

#[derive(Debug)]
pub(crate) enum BoardPolicyError {
    EmptyConfiguration,
    UnknownBoardSlug(String),
    Database(sqlx::Error),
}

impl fmt::Display for BoardPolicyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyConfiguration => {
                formatter.write_str("enabled board slug configuration must not be empty")
            }
            Self::UnknownBoardSlug(slug) => {
                write!(formatter, "enabled board slug does not exist: {slug}")
            }
            Self::Database(error) => write!(formatter, "could not apply board policy: {error}"),
        }
    }
}

impl std::error::Error for BoardPolicyError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Database(error) => Some(error),
            Self::EmptyConfiguration | Self::UnknownBoardSlug(_) => None,
        }
    }
}

impl From<sqlx::Error> for BoardPolicyError {
    fn from(error: sqlx::Error) -> Self {
        Self::Database(error)
    }
}

pub(crate) struct Reply {
    pub(crate) id: u64,
    pub(crate) body: String,
    pub(crate) poster_id: String,
    pub(crate) media: Option<Media>,
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
    pub(crate) details: Option<String>,
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
    Archived,
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
    details: Option<String>,
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
    archived_at: Option<String>,
    reply_count: i64,
    media_thumbnail_path: Option<String>,
    media_display_path: Option<String>,
    media_mime_type: Option<String>,
    media_width: Option<u64>,
    media_height: Option<u64>,
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
    thread_archived_at: Option<String>,
    thread_media_thumbnail_path: Option<String>,
    thread_media_display_path: Option<String>,
    thread_media_mime_type: Option<String>,
    thread_media_width: Option<u64>,
    thread_media_height: Option<u64>,
}

#[derive(sqlx::FromRow)]
struct ReplyRow {
    id: u64,
    thread_id: u64,
    poster_id: String,
    body: String,
    media_thumbnail_path: Option<String>,
    media_display_path: Option<String>,
    media_mime_type: Option<String>,
    media_width: Option<u64>,
    media_height: Option<u64>,
}

fn thread_poster_id(token: &str, thread_id: u64) -> String {
    let mut digest = Sha256::new();
    digest.update(token.as_bytes());
    digest.update(thread_id.to_be_bytes());

    let hash = digest.finalize();

    format!(
        "Anonymous ##{:02x}{:02x}{:02x}{:02x}",
        hash[0], hash[1], hash[2], hash[3]
    )
}

fn media_from_parts(
    thumbnail_path: Option<String>,
    display_path: Option<String>,
    mime_type: Option<String>,
    width: Option<u64>,
    height: Option<u64>,
) -> Option<Media> {
    Some(Media {
        thumbnail_path: thumbnail_path?,
        display_path: display_path?,
        mime_type: mime_type?,
        width: width?,
        height: height?,
    })
}

fn thread_from_row(row: ThreadRow) -> Thread {
    Thread {
        id: row.id,
        title: row.title,
        body: row.body,
        poster_id: row.poster_id,
        is_locked: row.status == "locked",
        reply_count: row.reply_count as u64,
        recent_replies: Vec::new(),
        replies: Vec::new(),
        media: media_from_parts(
            row.media_thumbnail_path,
            row.media_display_path,
            row.media_mime_type,
            row.media_width,
            row.media_height,
        ),
        is_archived: row.archived_at.is_some(),
    }
}

fn reply_from_row(row: ReplyRow) -> Reply {
    Reply {
        id: row.id,
        body: row.body,
        poster_id: row.poster_id,
        media: media_from_parts(
            row.media_thumbnail_path,
            row.media_display_path,
            row.media_mime_type,
            row.media_width,
            row.media_height,
        ),
    }
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

pub(crate) async fn apply_board_policy(
    pool: &sqlx::SqlitePool,
    enabled_slugs: &[String],
) -> Result<(), BoardPolicyError> {
    if enabled_slugs.is_empty() {
        return Err(BoardPolicyError::EmptyConfiguration);
    }

    let mut transaction = pool.begin().await?;

    for slug in enabled_slugs {
        let exists = sqlx::query_scalar::<_, i64>("SELECT id FROM boards WHERE slug = ?")
            .bind(slug)
            .fetch_optional(&mut *transaction)
            .await?
            .is_some();
        if !exists {
            return Err(BoardPolicyError::UnknownBoardSlug(slug.clone()));
        }
    }

    let mut approve_query = String::from("UPDATE boards SET status = 'approved' WHERE slug IN (");
    for (index, _) in enabled_slugs.iter().enumerate() {
        if index > 0 {
            approve_query.push_str(", ");
        }
        approve_query.push('?');
    }
    approve_query.push(')');

    let mut approve = sqlx::query(&approve_query);
    for slug in enabled_slugs {
        approve = approve.bind(slug);
    }
    approve.execute(&mut *transaction).await?;

    let mut archive_query = String::from(
        "UPDATE boards SET status = 'archived' WHERE status = 'approved' AND slug NOT IN (",
    );
    for (index, _) in enabled_slugs.iter().enumerate() {
        if index > 0 {
            archive_query.push_str(", ");
        }
        archive_query.push('?');
    }
    archive_query.push(')');

    let mut archive = sqlx::query(&archive_query);
    for slug in enabled_slugs {
        archive = archive.bind(slug);
    }
    archive.execute(&mut *transaction).await?;

    transaction.commit().await?;
    Ok(())
}

pub(crate) async fn load_board(
    pool: &sqlx::SqlitePool,
    slug: &str,
) -> Result<Option<Board>, sqlx::Error> {
    load_board_variant(pool, slug, false).await
}

pub(crate) async fn load_archive(
    pool: &sqlx::SqlitePool,
    slug: &str,
) -> Result<Option<Board>, sqlx::Error> {
    load_board_variant(pool, slug, true).await
}

async fn load_board_variant(
    pool: &sqlx::SqlitePool,
    slug: &str,
    archived: bool,
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

    let archive_filter = if archived {
        "t.archived_at IS NOT NULL"
    } else {
        "t.archived_at IS NULL"
    };
    let thread_query = format!(
        r#"
        SELECT
            t.id,
            t.title,
            t.body,
            t.poster_id,
            t.status,
            t.archived_at,
            COUNT(r.id) AS reply_count,
            pm.thumbnail_path AS media_thumbnail_path,
            pm.display_path AS media_display_path,
            pm.mime_type AS media_mime_type,
            pm.width AS media_width,
            pm.height AS media_height
        FROM threads t
        JOIN boards b ON b.id = t.board_id
        LEFT JOIN replies r
            ON r.thread_id = t.id
            AND r.status = 'visible'
        LEFT JOIN post_media pm ON pm.thread_id = t.id
        WHERE b.slug = ?
            AND b.status = 'approved'
            AND t.status IN ('visible', 'locked')
            AND {archive_filter}
        GROUP BY t.id
        ORDER BY t.created_at DESC, t.id DESC
        "#
    );
    let thread_rows = sqlx::query_as::<_, ThreadRow>(&thread_query)
        .bind(slug)
        .fetch_all(pool)
        .await?;
    let mut threads = thread_rows
        .into_iter()
        .map(thread_from_row)
        .collect::<Vec<_>>();
    let thread_indexes = threads
        .iter()
        .enumerate()
        .map(|(index, thread)| (thread.id, index))
        .collect::<HashMap<_, _>>();

    let reply_query = format!(
        r#"
        WITH ranked_replies AS (
            SELECT
                r.id,
                r.thread_id,
                r.poster_id,
                r.body,
                r.created_at,
                ROW_NUMBER() OVER (
                    PARTITION BY r.thread_id
                    ORDER BY r.created_at DESC, r.id DESC
                ) AS recent_rank
            FROM replies r
            JOIN threads t ON t.id = r.thread_id
            JOIN boards b ON b.id = t.board_id
            WHERE b.slug = ?
                AND b.status = 'approved'
                AND t.status IN ('visible', 'locked')
                AND r.status = 'visible'
                AND {archive_filter}
        )
        SELECT
            rr.id,
            rr.thread_id,
            rr.poster_id,
            rr.body,
            pm.thumbnail_path AS media_thumbnail_path,
            pm.display_path AS media_display_path,
            pm.mime_type AS media_mime_type,
            pm.width AS media_width,
            pm.height AS media_height
        FROM ranked_replies rr
        LEFT JOIN post_media pm ON pm.reply_id = rr.id
        WHERE rr.recent_rank <= 3
        ORDER BY rr.thread_id, rr.created_at, rr.id
        "#
    );
    for reply in sqlx::query_as::<_, ReplyRow>(&reply_query)
        .bind(slug)
        .fetch_all(pool)
        .await?
    {
        if let Some(&index) = thread_indexes.get(&reply.thread_id) {
            threads[index].recent_replies.push(reply_from_row(reply));
        }
    }

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
    media: Option<&Media>,
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
    let poster_id = thread_poster_id(anonymous_token, thread_id);

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
    if let Some(media) = media {
        sqlx::query(
            r#"
            INSERT INTO post_media (
                thread_id,
                thumbnail_path,
                display_path,
                mime_type,
                width,
                height
            )
            VALUES (?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(thread_id as i64)
        .bind(&media.thumbnail_path)
        .bind(&media.display_path)
        .bind(&media.mime_type)
        .bind(media.width as i64)
        .bind(media.height as i64)
        .execute(&mut *transaction)
        .await?;
    }

    transaction.commit().await?;

    Ok(Some(thread_id))
}

pub(crate) async fn create_reply(
    pool: &sqlx::SqlitePool,
    thread_id: u64,
    body: &str,
    anonymous_token: &str,
    origin: &crate::abuse::ProtectedClient,
    media: Option<&Media>,
) -> Result<CreateReplyResult, sqlx::Error> {
    let mut transaction = pool.begin().await?;
    let Some((status, archived_at)) = sqlx::query_as::<_, (String, Option<String>)>(
        r#"
            SELECT t.status, t.archived_at
            FROM threads AS t
            JOIN boards AS b ON b.id = t.board_id
            WHERE t.id = ? AND b.status = 'approved'
            "#,
    )
    .bind(thread_id as i64)
    .fetch_optional(&mut *transaction)
    .await?
    else {
        return Ok(CreateReplyResult::NotFound);
    };

    if archived_at.is_some() {
        return Ok(CreateReplyResult::Archived);
    }

    if status == "locked" {
        return Ok(CreateReplyResult::Locked);
    }

    if status != "visible" {
        return Ok(CreateReplyResult::NotFound);
    }
    let poster_id = thread_poster_id(anonymous_token, thread_id);

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
    if let Some(media) = media {
        sqlx::query(
            r#"
            INSERT INTO post_media (
                reply_id,
                thumbnail_path,
                display_path,
                mime_type,
                width,
                height
            )
            VALUES (?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(reply_id)
        .bind(&media.thumbnail_path)
        .bind(&media.display_path)
        .bind(&media.mime_type)
        .bind(media.width as i64)
        .bind(media.height as i64)
        .execute(&mut *transaction)
        .await?;
    }

    transaction.commit().await?;

    Ok(CreateReplyResult::Created)
}

pub(crate) async fn report_thread(
    pool: &sqlx::SqlitePool,
    thread_id: u64,
    reason: &str,
    details: Option<&str>,
) -> Result<bool, sqlx::Error> {
    let Some(_) = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT t.id
        FROM threads AS t
        JOIN boards AS b ON b.id = t.board_id
        WHERE t.id = ?
          AND b.status = 'approved'
          AND t.status IN ('visible', 'locked')
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
        INSERT INTO reports (thread_id, reason, details, status)
        VALUES (?, ?, ?, 'pending')
        "#,
    )
    .bind(thread_id as i64)
    .bind(reason)
    .bind(details)
    .execute(pool)
    .await?;

    Ok(true)
}

pub(crate) async fn report_reply(
    pool: &sqlx::SqlitePool,
    reply_id: u64,
    reason: &str,
    details: Option<&str>,
) -> Result<Option<u64>, sqlx::Error> {
    let Some(thread_id) = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT r.thread_id
        FROM replies AS r
        JOIN threads AS t ON t.id = r.thread_id
        JOIN boards AS b ON b.id = t.board_id
        WHERE r.id = ?
          AND r.status = 'visible'
          AND b.status = 'approved'
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
        INSERT INTO reports (reply_id, reason, details, status)
        VALUES (?, ?, ?, 'pending')
        "#,
    )
    .bind(reply_id as i64)
    .bind(reason)
    .bind(details)
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
            r.details,
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
            details: row.details,
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
            t.status AS thread_status,
            t.archived_at AS thread_archived_at,
            pm.thumbnail_path AS thread_media_thumbnail_path,
            pm.display_path AS thread_media_display_path,
            pm.mime_type AS thread_media_mime_type,
            pm.width AS thread_media_width,
            pm.height AS thread_media_height
        FROM threads t
        JOIN boards b ON b.id = t.board_id
        LEFT JOIN post_media pm ON pm.thread_id = t.id
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
        SELECT
            r.id,
            r.thread_id,
            r.body,
            r.poster_id,
            pm.thumbnail_path AS media_thumbnail_path,
            pm.display_path AS media_display_path,
            pm.mime_type AS media_mime_type,
            pm.width AS media_width,
            pm.height AS media_height
        FROM replies r
        LEFT JOIN post_media pm ON pm.reply_id = r.id
        WHERE r.thread_id = ?
            AND r.status = 'visible'
        ORDER BY r.created_at, r.id
        "#,
    )
    .bind(id as i64)
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(reply_from_row)
    .collect::<Vec<_>>();
    let reply_count = replies.len() as u64;

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
            reply_count,
            recent_replies: Vec::new(),
            replies,
            media: media_from_parts(
                thread_row.thread_media_thumbnail_path,
                thread_row.thread_media_display_path,
                thread_row.thread_media_mime_type,
                thread_row.thread_media_width,
                thread_row.thread_media_height,
            ),
            is_archived: thread_row.thread_archived_at.is_some(),
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
    #[sqlx::test(migrator = "MIGRATOR")]
    async fn board_policy_approves_exactly_configured_slugs(pool: sqlx::SqlitePool) {
        let enabled_slugs = vec![String::from("b"), String::from("pasum")];

        apply_board_policy(&pool, &enabled_slugs).await.unwrap();

        let approved = sqlx::query_scalar::<_, String>(
            "SELECT slug FROM boards WHERE status = 'approved' ORDER BY slug",
        )
        .fetch_all(&pool)
        .await
        .unwrap();
        assert_eq!(approved, vec![String::from("b"), String::from("pasum")]);

        let engineering_status =
            sqlx::query_scalar::<_, String>("SELECT status FROM boards WHERE slug = 'engineering'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(engineering_status, "archived");
    }

    #[sqlx::test(migrator = "MIGRATOR")]
    async fn board_policy_rejects_unknown_slug_without_changes(pool: sqlx::SqlitePool) {
        let before =
            sqlx::query_as::<_, (String, String)>("SELECT slug, status FROM boards ORDER BY slug")
                .fetch_all(&pool)
                .await
                .unwrap();

        let enabled_slugs = vec![String::from("b"), String::from("unknown")];
        assert!(matches!(
            apply_board_policy(&pool, &enabled_slugs).await,
            Err(BoardPolicyError::UnknownBoardSlug(slug)) if slug == "unknown"
        ));

        let after =
            sqlx::query_as::<_, (String, String)>("SELECT slug, status FROM boards ORDER BY slug")
                .fetch_all(&pool)
                .await
                .unwrap();
        assert_eq!(after, before);
    }

    #[sqlx::test(migrator = "MIGRATOR")]
    async fn disabled_boards_reject_reply_and_report_writes(pool: sqlx::SqlitePool) {
        let enabled_slugs = vec![String::from("b"), String::from("pasum")];
        apply_board_policy(&pool, &enabled_slugs).await.unwrap();

        let origin = crate::abuse::ProtectedClient {
            fingerprint: [0; 32],
            nonce: [0; 12],
            ciphertext: Vec::new(),
        };
        assert_eq!(
            create_reply(
                &pool,
                1,
                "must not be inserted",
                "anonymous-token",
                &origin,
                None
            )
            .await
            .unwrap(),
            CreateReplyResult::NotFound
        );
        assert!(
            !report_thread(&pool, 1, "must not be reported", None)
                .await
                .unwrap()
        );
        assert_eq!(
            report_reply(&pool, 1, "must not be reported", None)
                .await
                .unwrap(),
            None
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM reports")
                .fetch_one(&pool)
                .await
                .unwrap(),
            0
        );
    }

    #[sqlx::test(migrator = "MIGRATOR")]
    async fn active_and_archive_loaders_separate_archived_threads(pool: sqlx::SqlitePool) {
        let active = load_board(&pool, "engineering").await.unwrap().unwrap();
        assert!(active.threads.iter().all(|thread| !thread.is_archived));
        assert!(active.threads.iter().all(|thread| thread.id != 2));

        let archive = load_archive(&pool, "engineering").await.unwrap().unwrap();
        assert_eq!(archive.threads.len(), 1);
        assert_eq!(archive.threads[0].id, 2);
        assert!(archive.threads[0].is_archived);
    }

    #[sqlx::test(migrator = "MIGRATOR")]
    async fn board_summary_counts_visible_replies_and_returns_newest_three_chronologically(
        pool: sqlx::SqlitePool,
    ) {
        let mut visible_ids = Vec::new();
        for (index, timestamp) in [
            "2030-01-01 00:00:01",
            "2030-01-01 00:00:02",
            "2030-01-01 00:00:03",
            "2030-01-01 00:00:04",
        ]
        .into_iter()
        .enumerate()
        {
            let id = sqlx::query(
                r#"
                INSERT INTO replies (thread_id, body, poster_id, status, created_at)
                VALUES (1, ?, ?, 'visible', ?)
                "#,
            )
            .bind(format!("visible-{index}"))
            .bind(format!("poster-{index}"))
            .bind(timestamp)
            .execute(&pool)
            .await
            .unwrap()
            .last_insert_rowid() as u64;
            visible_ids.push(id);
        }
        sqlx::query(
            r#"
            INSERT INTO replies (thread_id, body, poster_id, status, created_at)
            VALUES (1, 'hidden', 'hidden-poster', 'hidden', '2030-01-01 00:00:59')
            "#,
        )
        .execute(&pool)
        .await
        .unwrap();

        let board = load_board(&pool, "engineering").await.unwrap().unwrap();
        let thread = board.threads.iter().find(|thread| thread.id == 1).unwrap();
        assert_eq!(thread.reply_count, 6);
        let recent_ids = thread
            .recent_replies
            .iter()
            .map(|reply| reply.id)
            .collect::<Vec<_>>();
        assert_eq!(recent_ids, visible_ids[1..].to_vec());
    }

    #[sqlx::test(migrator = "MIGRATOR")]
    async fn archived_threads_reject_new_replies_before_inserting(pool: sqlx::SqlitePool) {
        let before =
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM replies WHERE thread_id = 2")
                .fetch_one(&pool)
                .await
                .unwrap();
        let origin = crate::abuse::ProtectedClient {
            fingerprint: [0; 32],
            nonce: [0; 12],
            ciphertext: Vec::new(),
        };
        assert_eq!(
            create_reply(
                &pool,
                2,
                "should not be inserted",
                "anonymous-token",
                &origin,
                None
            )
            .await
            .unwrap(),
            CreateReplyResult::Archived
        );
        let after =
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM replies WHERE thread_id = 2")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(after, before);
    }

    #[sqlx::test(migrator = "MIGRATOR")]
    async fn thread_and_reply_media_are_mapped_optionally(pool: sqlx::SqlitePool) {
        sqlx::query(
            r#"
            INSERT INTO post_media (
                thread_id, thumbnail_path, display_path, mime_type, width, height
            )
            VALUES (1, '/thumb/thread.webp', '/media/thread.webp', 'image/webp', 640, 480)
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
            VALUES (1, '/thumb/reply.webp', '/media/reply.webp', 'image/webp', 320, 240)
            "#,
        )
        .execute(&pool)
        .await
        .unwrap();

        let board = load_board(&pool, "engineering").await.unwrap().unwrap();
        let summary_thread = board.threads.iter().find(|thread| thread.id == 1).unwrap();
        let thread_media = summary_thread.media.as_ref().unwrap();
        assert_eq!(thread_media.thumbnail_path, "/thumb/thread.webp");
        assert_eq!(thread_media.width, 640);
        let summary_reply = summary_thread
            .recent_replies
            .iter()
            .find(|reply| reply.id == 1)
            .unwrap();
        let reply_media = summary_reply.media.as_ref().unwrap();
        assert_eq!(reply_media.display_path, "/media/reply.webp");
        assert_eq!(reply_media.height, 240);

        let (_, thread) = load_thread(&pool, 1).await.unwrap().unwrap();
        assert_eq!(thread.media.as_ref().unwrap().mime_type, "image/webp");
        assert_eq!(
            thread
                .replies
                .iter()
                .find(|reply| reply.id == 1)
                .unwrap()
                .media
                .as_ref()
                .unwrap()
                .width,
            320
        );
    }
    #[sqlx::test(migrator = "MIGRATOR")]
    async fn create_thread_with_media_persists_media_and_origin(pool: sqlx::SqlitePool) {
        let origin = crate::abuse::ProtectedClient {
            fingerprint: [11; 32],
            nonce: [12; 12],
            ciphertext: vec![13, 14, 15],
        };
        let media = Media {
            thumbnail_path: String::from("/thumb/created-thread.webp"),
            display_path: String::from("/media/created-thread.webp"),
            mime_type: String::from("image/webp"),
            width: 1600,
            height: 900,
        };

        let thread_id = create_thread(
            &pool,
            "engineering",
            "Thread media transaction",
            "Thread media transaction body",
            "thread-media-token",
            &origin,
            Some(&media),
        )
        .await
        .unwrap()
        .unwrap();

        let post_media = sqlx::query_as::<_, (i64, Option<i64>, String, String, String, i64, i64)>(
            r#"
            SELECT thread_id, reply_id, thumbnail_path, display_path, mime_type, width, height
            FROM post_media
            WHERE thread_id = ?
            "#,
        )
        .bind(thread_id as i64)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(
            post_media,
            (
                thread_id as i64,
                None,
                String::from("/thumb/created-thread.webp"),
                String::from("/media/created-thread.webp"),
                String::from("image/webp"),
                1600,
                900,
            )
        );

        let origin_count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM post_origins WHERE client_fingerprint = ?",
        )
        .bind(origin.fingerprint.as_slice())
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(origin_count, 1);
    }

    #[sqlx::test(migrator = "MIGRATOR")]
    async fn create_reply_with_media_persists_media_and_origin(pool: sqlx::SqlitePool) {
        let origin = crate::abuse::ProtectedClient {
            fingerprint: [21; 32],
            nonce: [22; 12],
            ciphertext: vec![23, 24, 25],
        };
        let media = Media {
            thumbnail_path: String::from("/thumb/created-reply.webp"),
            display_path: String::from("/media/created-reply.webp"),
            mime_type: String::from("image/webp"),
            width: 1280,
            height: 720,
        };

        assert_eq!(
            create_reply(
                &pool,
                1,
                "Reply media transaction body",
                "reply-media-token",
                &origin,
                Some(&media),
            )
            .await
            .unwrap(),
            CreateReplyResult::Created
        );

        let reply_id = sqlx::query_scalar::<_, i64>(
            "SELECT id FROM replies WHERE body = 'Reply media transaction body'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        let post_media = sqlx::query_as::<_, (Option<i64>, i64, String, String, String, i64, i64)>(
            r#"
            SELECT thread_id, reply_id, thumbnail_path, display_path, mime_type, width, height
            FROM post_media
            WHERE reply_id = ?
            "#,
        )
        .bind(reply_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(
            post_media,
            (
                None,
                reply_id,
                String::from("/thumb/created-reply.webp"),
                String::from("/media/created-reply.webp"),
                String::from("image/webp"),
                1280,
                720,
            )
        );

        let origin_count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM post_origins WHERE client_fingerprint = ?",
        )
        .bind(origin.fingerprint.as_slice())
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(origin_count, 1);
    }

    #[sqlx::test(migrator = "MIGRATOR")]
    async fn create_thread_rolls_back_origin_when_media_insert_fails(pool: sqlx::SqlitePool) {
        sqlx::query(
            r#"
            CREATE TRIGGER reject_thread_post_media
            BEFORE INSERT ON post_media
            WHEN NEW.thread_id IS NOT NULL
            BEGIN
                SELECT RAISE(ABORT, 'thread media rejected');
            END
            "#,
        )
        .execute(&pool)
        .await
        .unwrap();

        let origin = crate::abuse::ProtectedClient {
            fingerprint: [31; 32],
            nonce: [32; 12],
            ciphertext: vec![33, 34, 35],
        };
        let media = Media {
            thumbnail_path: String::from("/thumb/rejected-thread.webp"),
            display_path: String::from("/media/rejected-thread.webp"),
            mime_type: String::from("image/webp"),
            width: 800,
            height: 600,
        };

        assert!(
            create_thread(
                &pool,
                "engineering",
                "Thread media rollback",
                "Thread media rollback body",
                "thread-rollback-token",
                &origin,
                Some(&media),
            )
            .await
            .is_err()
        );

        let thread_count =
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM threads WHERE title = ?")
                .bind("Thread media rollback")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(thread_count, 0);
        let origin_count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM post_origins WHERE client_fingerprint = ?",
        )
        .bind(origin.fingerprint.as_slice())
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(origin_count, 0);
    }

    #[sqlx::test(migrator = "MIGRATOR")]
    async fn create_reply_rolls_back_origin_when_media_insert_fails(pool: sqlx::SqlitePool) {
        sqlx::query(
            r#"
            CREATE TRIGGER reject_reply_post_media
            BEFORE INSERT ON post_media
            WHEN NEW.reply_id IS NOT NULL
            BEGIN
                SELECT RAISE(ABORT, 'reply media rejected');
            END
            "#,
        )
        .execute(&pool)
        .await
        .unwrap();

        let origin = crate::abuse::ProtectedClient {
            fingerprint: [41; 32],
            nonce: [42; 12],
            ciphertext: vec![43, 44, 45],
        };
        let media = Media {
            thumbnail_path: String::from("/thumb/rejected-reply.webp"),
            display_path: String::from("/media/rejected-reply.webp"),
            mime_type: String::from("image/webp"),
            width: 640,
            height: 480,
        };

        assert!(
            create_reply(
                &pool,
                1,
                "Reply media rollback body",
                "reply-rollback-token",
                &origin,
                Some(&media),
            )
            .await
            .is_err()
        );

        let reply_count =
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM replies WHERE body = ?")
                .bind("Reply media rollback body")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(reply_count, 0);
        let origin_count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM post_origins WHERE client_fingerprint = ?",
        )
        .bind(origin.fingerprint.as_slice())
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(origin_count, 0);
    }
}
