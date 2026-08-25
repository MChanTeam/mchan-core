use sha2::{Digest, Sha256};
use std::{collections::HashMap, fmt};
use uuid::Uuid;

const THREAD_BUMP_LIMIT: i64 = 300;
const MAX_SNAPSHOT_REPLY_OFFSET: i64 = 10_000;

pub(crate) struct Board {
    pub(crate) slug: String,
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) threads: Vec<Thread>,
    pub(crate) is_archived: bool,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct ManagedBoard {
    pub(crate) slug: String,
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) status: String,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum BoardManagementResult {
    Created,
    DuplicateSlug,
    Archived,
    Restored,
    NotFound,
    InvalidTransition,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DirectHideReason {
    Spam,
    Harassment,
    Doxxing,
    Threat,
    SexualContent,
    IllegalContent,
    OffTopic,
    BoardRule,
    Other,
}

impl DirectHideReason {
    pub(crate) fn parse(value: &str) -> Option<Self> {
        match value {
            "spam" => Some(Self::Spam),
            "harassment" => Some(Self::Harassment),
            "doxxing" => Some(Self::Doxxing),
            "threat" => Some(Self::Threat),
            "sexual-content" => Some(Self::SexualContent),
            "illegal-content" => Some(Self::IllegalContent),
            "off-topic" => Some(Self::OffTopic),
            "board-rule" => Some(Self::BoardRule),
            "other" => Some(Self::Other),
            _ => None,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Spam => "spam",
            Self::Harassment => "harassment",
            Self::Doxxing => "doxxing",
            Self::Threat => "threat",
            Self::SexualContent => "sexual-content",
            Self::IllegalContent => "illegal-content",
            Self::OffTopic => "off-topic",
            Self::BoardRule => "board-rule",
            Self::Other => "other",
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum DirectHideResult {
    Applied,
    NotFound,
    InvalidTarget,
}

pub(crate) struct BoardPage {
    pub(crate) board: Board,
    pub(crate) has_next: bool,
}

pub(crate) struct Media {
    pub(crate) thumbnail_path: String,
    pub(crate) display_path: String,
    pub(crate) mime_type: String,
    pub(crate) width: u64,
    pub(crate) height: u64,
}
pub(crate) struct PublicReplySnapshot {
    pub(crate) id: u64,
    pub(crate) body: String,
    pub(crate) poster_id: String,
    pub(crate) created_at: String,
    pub(crate) media: Option<Media>,
}

pub(crate) struct PublicThreadSnapshot {
    pub(crate) id: u64,
    pub(crate) board_slug: String,
    pub(crate) title: String,
    pub(crate) body: String,
    pub(crate) poster_id: String,
    pub(crate) created_at: String,
    pub(crate) is_pinned: bool,
    pub(crate) is_locked: bool,
    pub(crate) is_archived: bool,
    pub(crate) reply_count: u64,
    pub(crate) media: Option<Media>,
    pub(crate) replies: Vec<PublicReplySnapshot>,
    pub(crate) has_next_replies: bool,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum IdempotentMutation<T> {
    Created(T),
    Replayed(T),
    Conflict,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct ProjectionOutboxEvent {
    pub(crate) id: u64,
    pub(crate) kind: String,
    pub(crate) thread_id: Option<u64>,
    pub(crate) reply_id: Option<u64>,
    pub(crate) report_id: Option<u64>,
    pub(crate) created_at: String,
    pub(crate) lease_token: String,
    pub(crate) lease_expires_at: String,
}

#[derive(sqlx::FromRow)]
struct ProjectionOutboxRow {
    id: u64,
    kind: String,
    thread_id: Option<u64>,
    reply_id: Option<u64>,
    report_id: Option<u64>,
    created_at: String,
    lease_token: String,
    lease_expires_at: String,
}

pub(crate) struct Thread {
    pub(crate) id: u64,
    pub(crate) title: String,
    pub(crate) body: String,
    pub(crate) poster_id: String,
    pub(crate) created_at: String,
    pub(crate) is_pinned: bool,
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
    pub(crate) created_at: String,
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

#[derive(sqlx::FromRow)]
pub(crate) struct OperationalMetrics {
    pub(crate) boards: i64,
    pub(crate) threads: i64,
    pub(crate) replies: i64,
    pub(crate) pending_reports: i64,
    pub(crate) active_board_bans: i64,
    pub(crate) active_site_bans: i64,
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
    Created(u64),
    NotFound,
    Locked,
    Archived,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum ThreadPinResult {
    Applied,
    NotFound,
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
    status: String,
}

#[derive(sqlx::FromRow)]
struct ThreadRow {
    id: u64,
    poster_id: String,
    created_at: String,
    title: String,
    body: String,
    status: String,
    archived_at: Option<String>,
    reply_count: i64,
    media_thumbnail_path: Option<String>,
    media_display_path: Option<String>,
    media_mime_type: Option<String>,
    media_width: Option<u64>,
    is_pinned: bool,
    media_height: Option<u64>,
}

#[derive(sqlx::FromRow)]
struct ThreadPageRow {
    board_slug: String,
    board_name: String,
    board_description: String,
    board_status: String,
    thread_id: u64,
    poster_id: String,
    thread_created_at: String,
    thread_title: String,
    thread_body: String,
    thread_status: String,
    thread_archived_at: Option<String>,
    thread_media_thumbnail_path: Option<String>,
    thread_media_display_path: Option<String>,
    thread_media_mime_type: Option<String>,
    thread_media_width: Option<u64>,
    thread_is_pinned: bool,
    thread_media_height: Option<u64>,
}

#[derive(sqlx::FromRow)]
struct ReplyRow {
    id: u64,
    thread_id: u64,
    poster_id: String,
    created_at: String,
    body: String,
    media_thumbnail_path: Option<String>,
    media_display_path: Option<String>,
    media_mime_type: Option<String>,
    media_width: Option<u64>,
    media_height: Option<u64>,
}

pub(crate) type IdempotencyKey<'a> = (&'a str, &'a str, &'a str, &'a [u8]);

enum IdempotencyClaim {
    New,
    Replay(u64),
    Conflict,
}

async fn claim_idempotency(
    transaction: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    key: IdempotencyKey<'_>,
) -> Result<IdempotencyClaim, sqlx::Error> {
    if key.3.len() != 32 {
        return Err(sqlx::Error::Protocol(
            "idempotency request hash must be 32 bytes".to_owned(),
        ));
    }
    sqlx::query(
        "INSERT OR IGNORE INTO machine_idempotency (service, operation, opaque_key, request_hash) VALUES (?, ?, ?, ?)",
    )
    .bind(key.0)
    .bind(key.1)
    .bind(key.2)
    .bind(key.3)
    .execute(&mut **transaction)
    .await?;
    let row = sqlx::query_as::<_, (Vec<u8>, Option<i64>)>(
        "SELECT request_hash, result_id FROM machine_idempotency WHERE service = ? AND operation = ? AND opaque_key = ?",
    )
    .bind(key.0)
    .bind(key.1)
    .bind(key.2)
    .fetch_one(&mut **transaction)
    .await?;
    if row.0.as_slice() != key.3 {
        return Ok(IdempotencyClaim::Conflict);
    }
    Ok(row
        .1
        .map(|id| IdempotencyClaim::Replay(id as u64))
        .unwrap_or(IdempotencyClaim::New))
}

async fn finish_idempotency(
    transaction: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    key: IdempotencyKey<'_>,
    result_id: u64,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE machine_idempotency SET result_id = ?, updated_at = CURRENT_TIMESTAMP WHERE service = ? AND operation = ? AND opaque_key = ?",
    )
    .bind(result_id as i64)
    .bind(key.0)
    .bind(key.1)
    .bind(key.2)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum IdempotencyCheck<T> {
    New,
    Replayed(T),
    Conflict,
}

pub(crate) async fn check_machine_idempotency(
    pool: &sqlx::SqlitePool,
    key: IdempotencyKey<'_>,
) -> Result<IdempotencyCheck<u64>, sqlx::Error> {
    if key.3.len() != 32 {
        return Err(sqlx::Error::Protocol(
            "idempotency request hash must be 32 bytes".to_owned(),
        ));
    }
    if key.2.as_bytes().len() < 1 || key.2.as_bytes().len() > 512 {
        return Err(sqlx::Error::Protocol(
            "opaque_key must be 1..512 bytes".to_owned(),
        ));
    }
    let row = sqlx::query_as::<_, (Vec<u8>, Option<i64>)>(
        "SELECT request_hash, result_id FROM machine_idempotency WHERE service = ? AND operation = ? AND opaque_key = ?",
    )
    .bind(key.0)
    .bind(key.1)
    .bind(key.2)
    .fetch_optional(pool)
    .await?;
    let Some((stored_hash, result_id)) = row else {
        return Ok(IdempotencyCheck::New);
    };
    if stored_hash.as_slice() != key.3 {
        return Ok(IdempotencyCheck::Conflict);
    }
    match result_id {
        Some(id) => Ok(IdempotencyCheck::Replayed(id as u64)),
        None => Ok(IdempotencyCheck::New),
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum ReplyTargetState {
    Visible,
    Locked,
    Archived,
    Missing,
}

pub(crate) async fn classify_reply_target(
    pool: &sqlx::SqlitePool,
    thread_id: u64,
) -> Result<ReplyTargetState, sqlx::Error> {
    let row = sqlx::query_as::<_, (String, Option<String>)>(
        "SELECT t.status, t.archived_at FROM threads t JOIN boards b ON b.id = t.board_id WHERE t.id = ? AND b.status = 'approved'",
    )
    .bind(thread_id as i64)
    .fetch_optional(pool)
    .await?;
    Ok(match row {
        None => ReplyTargetState::Missing,
        Some((_, Some(_))) => ReplyTargetState::Archived,
        Some((status, None)) => match status.as_str() {
            "visible" => ReplyTargetState::Visible,
            "locked" => ReplyTargetState::Locked,
            _ => ReplyTargetState::Missing,
        },
    })
}

pub(crate) async fn approved_board_exists(
    pool: &sqlx::SqlitePool,
    slug: &str,
) -> Result<bool, sqlx::Error> {
    let row =
        sqlx::query_scalar::<_, i64>("SELECT 1 FROM boards WHERE slug = ? AND status = 'approved'")
            .bind(slug)
            .fetch_optional(pool)
            .await?;
    Ok(row.is_some())
}

pub(crate) async fn thread_report_target_exists(
    pool: &sqlx::SqlitePool,
    thread_id: u64,
) -> Result<bool, sqlx::Error> {
    let row = sqlx::query_scalar::<_, i64>(
        "SELECT t.id FROM threads t JOIN boards b ON b.id = t.board_id WHERE t.id = ? AND b.status = 'approved' AND t.status IN ('visible', 'locked')",
    )
    .bind(thread_id as i64)
    .fetch_optional(pool)
    .await?;
    Ok(row.is_some())
}

pub(crate) async fn reply_report_target_exists(
    pool: &sqlx::SqlitePool,
    reply_id: u64,
) -> Result<bool, sqlx::Error> {
    let row = sqlx::query_scalar::<_, i64>(
        "SELECT r.id FROM replies r JOIN threads t ON t.id = r.thread_id JOIN boards b ON b.id = t.board_id WHERE r.id = ? AND r.status = 'visible' AND b.status = 'approved'",
    )
    .bind(reply_id as i64)
    .fetch_optional(pool)
    .await?;
    Ok(row.is_some())
}

pub(crate) async fn reply_owner_thread_id(
    pool: &sqlx::SqlitePool,
    reply_id: u64,
) -> Result<Option<u64>, sqlx::Error> {
    sqlx::query_scalar::<_, u64>("SELECT thread_id FROM replies WHERE id = ?")
        .bind(reply_id as i64)
        .fetch_optional(pool)
        .await
}

async fn insert_projection_outbox(
    transaction: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    kind: &str,
    thread_id: Option<u64>,
    reply_id: Option<u64>,
    report_id: Option<u64>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO projection_outbox (kind, thread_id, reply_id, report_id) VALUES (?, ?, ?, ?)",
    )
    .bind(kind)
    .bind(thread_id.map(|id| id as i64))
    .bind(reply_id.map(|id| id as i64))
    .bind(report_id.map(|id| id as i64))
    .execute(&mut **transaction)
    .await?;
    Ok(())
}
fn thread_poster_id(fingerprint: &[u8; 32], thread_id: u64) -> String {
    let mut digest = Sha256::new();
    digest.update(fingerprint);
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
        is_pinned: row.is_pinned,
        body: row.body,
        created_at: row.created_at,
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
        created_at: row.created_at,
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

pub(crate) async fn load_operational_metrics(
    pool: &sqlx::SqlitePool,
) -> Result<OperationalMetrics, sqlx::Error> {
    sqlx::query_as::<_, OperationalMetrics>(
        r#"
        SELECT
            (SELECT COUNT(*) FROM boards WHERE status = 'approved') AS boards,
            (
                SELECT COUNT(*)
                FROM threads
                WHERE status IN ('visible', 'locked') AND archived_at IS NULL
            ) AS threads,
            (SELECT COUNT(*) FROM replies WHERE status = 'visible') AS replies,
            (SELECT COUNT(*) FROM reports WHERE status = 'pending') AS pending_reports,
            (
                SELECT COUNT(*)
                FROM bans
                WHERE scope = 'board'
                    AND revoked_at IS NULL
                    AND datetime(expires_at) > CURRENT_TIMESTAMP
            ) AS active_board_bans,
            (
                SELECT COUNT(*)
                FROM bans
                WHERE scope = 'site'
                    AND revoked_at IS NULL
                    AND datetime(expires_at) > CURRENT_TIMESTAMP
            ) AS active_site_bans
        "#,
    )
    .fetch_one(pool)
    .await
}

pub(crate) async fn load_approved_boards(
    pool: &sqlx::SqlitePool,
) -> Result<Vec<Board>, sqlx::Error> {
    let rows = sqlx::query_as::<_, BoardRow>(
        r#"
        SELECT slug, name, description, status
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
            is_archived: false,
        })
        .collect())
}

pub(crate) async fn load_managed_boards(
    pool: &sqlx::SqlitePool,
) -> Result<Vec<ManagedBoard>, sqlx::Error> {
    #[derive(sqlx::FromRow)]
    struct ManagedBoardRow {
        slug: String,
        name: String,
        description: String,
        status: String,
    }

    let rows = sqlx::query_as::<_, ManagedBoardRow>(
        r#"
        SELECT slug, name, description, status
        FROM boards
        ORDER BY name, slug
        "#,
    )
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| ManagedBoard {
            slug: row.slug,
            name: row.name,
            description: row.description,
            status: row.status,
        })
        .collect())
}

pub(crate) async fn create_board(
    pool: &sqlx::SqlitePool,
    slug: &str,
    name: &str,
    description: &str,
) -> Result<BoardManagementResult, sqlx::Error> {
    let mut transaction = pool.begin().await?;
    if sqlx::query_scalar::<_, i64>("SELECT id FROM boards WHERE slug = ?")
        .bind(slug)
        .fetch_optional(&mut *transaction)
        .await?
        .is_some()
    {
        return Ok(BoardManagementResult::DuplicateSlug);
    }
    sqlx::query(
        r#"
        INSERT INTO boards (slug, name, description, status)
        VALUES (?, ?, ?, 'approved')
        "#,
    )
    .bind(slug)
    .bind(name)
    .bind(description)
    .execute(&mut *transaction)
    .await?;
    transaction.commit().await?;
    Ok(BoardManagementResult::Created)
}

pub(crate) async fn archive_board(
    pool: &sqlx::SqlitePool,
    slug: &str,
) -> Result<BoardManagementResult, sqlx::Error> {
    let mut transaction = pool.begin().await?;
    let Some(status) = sqlx::query_scalar::<_, String>("SELECT status FROM boards WHERE slug = ?")
        .bind(slug)
        .fetch_optional(&mut *transaction)
        .await?
    else {
        return Ok(BoardManagementResult::NotFound);
    };
    if status != "approved" {
        return Ok(BoardManagementResult::InvalidTransition);
    }
    sqlx::query("UPDATE boards SET status = 'archived' WHERE slug = ? AND status = 'approved'")
        .bind(slug)
        .execute(&mut *transaction)
        .await?;
    transaction.commit().await?;
    Ok(BoardManagementResult::Archived)
}

pub(crate) async fn restore_board(
    pool: &sqlx::SqlitePool,
    slug: &str,
) -> Result<BoardManagementResult, sqlx::Error> {
    let mut transaction = pool.begin().await?;
    let Some(status) = sqlx::query_scalar::<_, String>("SELECT status FROM boards WHERE slug = ?")
        .bind(slug)
        .fetch_optional(&mut *transaction)
        .await?
    else {
        return Ok(BoardManagementResult::NotFound);
    };
    if status != "archived" {
        return Ok(BoardManagementResult::InvalidTransition);
    }
    sqlx::query("UPDATE boards SET status = 'approved' WHERE slug = ? AND status = 'archived'")
        .bind(slug)
        .execute(&mut *transaction)
        .await?;
    transaction.commit().await?;
    Ok(BoardManagementResult::Restored)
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
    Ok(load_board_page(pool, slug, 20, 0)
        .await?
        .map(|page| page.board))
}

pub(crate) async fn load_archive(
    pool: &sqlx::SqlitePool,
    slug: &str,
) -> Result<Option<Board>, sqlx::Error> {
    Ok(load_archive_page(pool, slug, 50, 0)
        .await?
        .map(|page| page.board))
}

pub(crate) async fn load_board_page(
    pool: &sqlx::SqlitePool,
    slug: &str,
    limit: i64,
    offset: i64,
) -> Result<Option<BoardPage>, sqlx::Error> {
    load_board_variant_page(pool, slug, false, limit, offset).await
}

pub(crate) async fn load_archive_page(
    pool: &sqlx::SqlitePool,
    slug: &str,
    limit: i64,
    offset: i64,
) -> Result<Option<BoardPage>, sqlx::Error> {
    load_board_variant_page(pool, slug, true, limit, offset).await
}

async fn load_board_variant_page(
    pool: &sqlx::SqlitePool,
    slug: &str,
    archived: bool,
    limit: i64,
    offset: i64,
) -> Result<Option<BoardPage>, sqlx::Error> {
    if limit < 0 || offset < 0 {
        return Err(sqlx::Error::Protocol(
            "board page limit and offset must be non-negative".to_owned(),
        ));
    }
    let query_limit = limit
        .checked_add(1)
        .ok_or_else(|| sqlx::Error::Protocol("board page limit is too large".to_owned()))?;

    let Some(board_row) = sqlx::query_as::<_, BoardRow>(
        r#"
        SELECT slug, name, description, status
        FROM boards
        WHERE slug = ? AND status IN ('approved', 'archived')
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
    let order_by = if archived {
        "t.created_at DESC, t.id DESC"
    } else {
        "COALESCE(t.is_pinned, 0) DESC, t.bumped_at DESC, t.id DESC"
    };
    let thread_query = format!(
        r#"
        SELECT
            t.id,
            t.title,
            t.body,
            t.poster_id,
            t.created_at,
            t.status,
            t.archived_at,
            t.is_pinned AS is_pinned,
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
            AND b.status IN ('approved', 'archived')
            AND t.status IN ('visible', 'locked')
            AND {archive_filter}
        GROUP BY t.id
        ORDER BY {order_by}
        LIMIT ? OFFSET ?
        "#
    );
    let mut thread_rows = sqlx::query_as::<_, ThreadRow>(&thread_query)
        .bind(slug)
        .bind(query_limit)
        .bind(offset)
        .fetch_all(pool)
        .await?;
    let has_next = thread_rows.len() > limit as usize;
    if has_next {
        thread_rows.pop();
    }
    let mut threads = thread_rows
        .into_iter()
        .map(thread_from_row)
        .collect::<Vec<_>>();
    let thread_indexes = threads
        .iter()
        .enumerate()
        .map(|(index, thread)| (thread.id, index))
        .collect::<HashMap<_, _>>();

    if !threads.is_empty() {
        let placeholders = std::iter::repeat_n("?", threads.len())
            .collect::<Vec<_>>()
            .join(", ");
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
                WHERE r.status = 'visible'
                    AND r.thread_id IN ({placeholders})
            )
            SELECT
                rr.id,
                rr.thread_id,
                rr.poster_id,
                rr.created_at AS created_at,
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
        let mut reply_query = sqlx::query_as::<_, ReplyRow>(&reply_query);
        for thread in &threads {
            let thread_id = i64::try_from(thread.id).map_err(|_| {
                sqlx::Error::Protocol(
                    "board page thread ID exceeds SQLite integer range".to_owned(),
                )
            })?;
            reply_query = reply_query.bind(thread_id);
        }
        for reply in reply_query.fetch_all(pool).await? {
            if let Some(&index) = thread_indexes.get(&reply.thread_id) {
                threads[index].recent_replies.push(reply_from_row(reply));
            }
        }
    }

    Ok(Some(BoardPage {
        board: Board {
            slug: board_row.slug,
            name: board_row.name,
            description: board_row.description,
            threads,
            is_archived: board_row.status == "archived",
        },
        has_next,
    }))
}

pub(crate) async fn create_thread(
    pool: &sqlx::SqlitePool,
    board_slug: &str,
    title: &str,
    body: &str,
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
        INSERT INTO threads (board_id, title, body, status, created_at, bumped_at)
        VALUES (?, ?, ?, 'visible', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
        "#,
    )
    .bind(board_id)
    .bind(title)
    .bind(body)
    .execute(&mut *transaction)
    .await?;

    let thread_id = result.last_insert_rowid() as u64;
    let poster_id = thread_poster_id(&origin.fingerprint, thread_id);

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
    insert_projection_outbox(
        &mut transaction,
        "thread_created",
        Some(thread_id),
        None,
        None,
    )
    .await?;

    transaction.commit().await?;

    Ok(Some(thread_id))
}
pub(crate) async fn create_thread_idempotent(
    pool: &sqlx::SqlitePool,
    board_slug: &str,
    title: &str,
    body: &str,
    origin: &crate::abuse::ProtectedClient,
    media: Option<&Media>,
    key: IdempotencyKey<'_>,
) -> Result<IdempotentMutation<u64>, sqlx::Error> {
    let mut transaction = pool.begin().await?;
    match claim_idempotency(&mut transaction, key).await? {
        IdempotencyClaim::Conflict => return Ok(IdempotentMutation::Conflict),
        IdempotencyClaim::Replay(id) => return Ok(IdempotentMutation::Replayed(id)),
        IdempotencyClaim::New => {}
    }
    let Some(board_id) = sqlx::query_scalar::<_, i64>(
        "SELECT id FROM boards WHERE slug = ? AND status = 'approved'",
    )
    .bind(board_slug)
    .fetch_optional(&mut *transaction)
    .await?
    else {
        return Ok(IdempotentMutation::Conflict);
    };
    let result = sqlx::query(
        "INSERT INTO threads (board_id, title, body, status, created_at, bumped_at) VALUES (?, ?, ?, 'visible', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
    )
    .bind(board_id)
    .bind(title)
    .bind(body)
    .execute(&mut *transaction)
    .await?;
    let thread_id = result.last_insert_rowid() as u64;
    sqlx::query("UPDATE threads SET poster_id = ? WHERE id = ?")
        .bind(thread_poster_id(&origin.fingerprint, thread_id))
        .bind(thread_id as i64)
        .execute(&mut *transaction)
        .await?;
    sqlx::query("INSERT INTO post_origins (thread_id, client_fingerprint, nonce, ciphertext) VALUES (?, ?, ?, ?)")
        .bind(thread_id as i64)
        .bind(origin.fingerprint.as_slice())
        .bind(origin.nonce.as_slice())
        .bind(&origin.ciphertext)
        .execute(&mut *transaction)
        .await?;
    if let Some(media) = media {
        sqlx::query("INSERT INTO post_media (thread_id, thumbnail_path, display_path, mime_type, width, height) VALUES (?, ?, ?, ?, ?, ?)")
            .bind(thread_id as i64)
            .bind(&media.thumbnail_path)
            .bind(&media.display_path)
            .bind(&media.mime_type)
            .bind(media.width as i64)
            .bind(media.height as i64)
            .execute(&mut *transaction)
            .await?;
    }
    insert_projection_outbox(
        &mut transaction,
        "thread_created",
        Some(thread_id),
        None,
        None,
    )
    .await?;
    finish_idempotency(&mut transaction, key, thread_id).await?;
    transaction.commit().await?;
    Ok(IdempotentMutation::Created(thread_id))
}

pub(crate) async fn create_reply(
    pool: &sqlx::SqlitePool,
    thread_id: u64,
    body: &str,
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
    let poster_id = thread_poster_id(&origin.fingerprint, thread_id);

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
    sqlx::query(
        r#"
        UPDATE threads
        SET bumped_at = (
            SELECT created_at FROM replies WHERE id = ?
        )
        WHERE id = ?
          AND (
              SELECT COUNT(*)
              FROM replies
              WHERE thread_id = ? AND status = 'visible'
          ) <= ?
        "#,
    )
    .bind(reply_id)
    .bind(thread_id as i64)
    .bind(thread_id as i64)
    .bind(THREAD_BUMP_LIMIT)
    .execute(&mut *transaction)
    .await?;
    insert_projection_outbox(
        &mut transaction,
        "thread_dirty",
        Some(thread_id),
        None,
        None,
    )
    .await?;

    transaction.commit().await?;

    Ok(CreateReplyResult::Created(reply_id as u64))
}
pub(crate) async fn create_reply_idempotent(
    pool: &sqlx::SqlitePool,
    thread_id: u64,
    body: &str,
    origin: &crate::abuse::ProtectedClient,
    media: Option<&Media>,
    key: IdempotencyKey<'_>,
) -> Result<IdempotentMutation<u64>, sqlx::Error> {
    let mut transaction = pool.begin().await?;
    match claim_idempotency(&mut transaction, key).await? {
        IdempotencyClaim::Conflict => return Ok(IdempotentMutation::Conflict),
        IdempotencyClaim::Replay(id) => return Ok(IdempotentMutation::Replayed(id)),
        IdempotencyClaim::New => {}
    }
    let Some((status, archived_at)) = sqlx::query_as::<_, (String, Option<String>)>(
        "SELECT t.status, t.archived_at FROM threads t JOIN boards b ON b.id=t.board_id WHERE t.id=? AND b.status='approved'",
    )
    .bind(thread_id as i64)
    .fetch_optional(&mut *transaction)
    .await?
    else {
        return Ok(IdempotentMutation::Conflict);
    };
    if archived_at.is_some() || status != "visible" {
        return Ok(IdempotentMutation::Conflict);
    }
    let result = sqlx::query(
        "INSERT INTO replies (thread_id, body, poster_id, status) VALUES (?, ?, ?, 'visible')",
    )
    .bind(thread_id as i64)
    .bind(body)
    .bind(thread_poster_id(&origin.fingerprint, thread_id))
    .execute(&mut *transaction)
    .await?;
    let reply_id = result.last_insert_rowid() as u64;
    sqlx::query("INSERT INTO post_origins (reply_id, client_fingerprint, nonce, ciphertext) VALUES (?, ?, ?, ?)")
        .bind(reply_id as i64)
        .bind(origin.fingerprint.as_slice())
        .bind(origin.nonce.as_slice())
        .bind(&origin.ciphertext)
        .execute(&mut *transaction)
        .await?;
    if let Some(media) = media {
        sqlx::query("INSERT INTO post_media (reply_id, thumbnail_path, display_path, mime_type, width, height) VALUES (?, ?, ?, ?, ?, ?)")
            .bind(reply_id as i64)
            .bind(&media.thumbnail_path)
            .bind(&media.display_path)
            .bind(&media.mime_type)
            .bind(media.width as i64)
            .bind(media.height as i64)
            .execute(&mut *transaction)
            .await?;
    }
    sqlx::query(
        r#"
        UPDATE threads
        SET bumped_at = (
            SELECT created_at FROM replies WHERE id = ?
        )
        WHERE id = ?
          AND (
              SELECT COUNT(*)
              FROM replies
              WHERE thread_id = ? AND status = 'visible'
          ) <= ?
        "#,
    )
    .bind(reply_id as i64)
    .bind(thread_id as i64)
    .bind(thread_id as i64)
    .bind(THREAD_BUMP_LIMIT)
    .execute(&mut *transaction)
    .await?;
    insert_projection_outbox(
        &mut transaction,
        "thread_dirty",
        Some(thread_id),
        None,
        None,
    )
    .await?;
    finish_idempotency(&mut transaction, key, reply_id).await?;
    transaction.commit().await?;
    Ok(IdempotentMutation::Created(reply_id))
}

pub(crate) async fn report_thread(
    pool: &sqlx::SqlitePool,
    thread_id: u64,
    reason: &str,
    details: Option<&str>,
) -> Result<bool, sqlx::Error> {
    let mut transaction = pool.begin().await?;
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
    .fetch_optional(&mut *transaction)
    .await?
    else {
        return Ok(false);
    };

    let result = sqlx::query(
        "INSERT INTO reports (thread_id, reason, details, status) VALUES (?, ?, ?, 'pending')",
    )
    .bind(thread_id as i64)
    .bind(reason)
    .bind(details)
    .execute(&mut *transaction)
    .await?;
    let report_id = result.last_insert_rowid() as u64;
    insert_projection_outbox(
        &mut transaction,
        "report_created",
        Some(thread_id),
        None,
        Some(report_id),
    )
    .await?;
    transaction.commit().await?;
    Ok(true)
}
pub(crate) async fn report_thread_idempotent(
    pool: &sqlx::SqlitePool,
    thread_id: u64,
    reason: &str,
    details: Option<&str>,
    key: IdempotencyKey<'_>,
) -> Result<IdempotentMutation<(u64, u64)>, sqlx::Error> {
    let mut transaction = pool.begin().await?;
    match claim_idempotency(&mut transaction, key).await? {
        IdempotencyClaim::Conflict => return Ok(IdempotentMutation::Conflict),
        IdempotencyClaim::Replay(id) => return Ok(IdempotentMutation::Replayed((id, thread_id))),
        IdempotencyClaim::New => {}
    }
    let Some(_) = sqlx::query_scalar::<_, i64>(
        "SELECT t.id FROM threads t JOIN boards b ON b.id=t.board_id WHERE t.id=? AND b.status='approved' AND t.status IN ('visible','locked')",
    )
    .bind(thread_id as i64)
    .fetch_optional(&mut *transaction)
    .await?
    else {
        return Ok(IdempotentMutation::Conflict);
    };
    let result = sqlx::query(
        "INSERT INTO reports (thread_id, reason, details, status) VALUES (?, ?, ?, 'pending')",
    )
    .bind(thread_id as i64)
    .bind(reason)
    .bind(details)
    .execute(&mut *transaction)
    .await?;
    let report_id = result.last_insert_rowid() as u64;
    insert_projection_outbox(
        &mut transaction,
        "report_created",
        Some(thread_id),
        None,
        Some(report_id),
    )
    .await?;
    finish_idempotency(&mut transaction, key, report_id).await?;
    transaction.commit().await?;
    Ok(IdempotentMutation::Created((report_id, thread_id)))
}

pub(crate) async fn report_reply(
    pool: &sqlx::SqlitePool,
    reply_id: u64,
    reason: &str,
    details: Option<&str>,
) -> Result<Option<u64>, sqlx::Error> {
    let mut transaction = pool.begin().await?;
    let Some(thread_id) = sqlx::query_scalar::<_, u64>(
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
    .fetch_optional(&mut *transaction)
    .await?
    else {
        return Ok(None);
    };
    let result = sqlx::query(
        "INSERT INTO reports (reply_id, reason, details, status) VALUES (?, ?, ?, 'pending')",
    )
    .bind(reply_id as i64)
    .bind(reason)
    .bind(details)
    .execute(&mut *transaction)
    .await?;
    let report_id = result.last_insert_rowid() as u64;
    insert_projection_outbox(
        &mut transaction,
        "report_created",
        Some(thread_id),
        Some(reply_id),
        Some(report_id),
    )
    .await?;
    transaction.commit().await?;
    Ok(Some(thread_id))
}
pub(crate) async fn report_reply_idempotent(
    pool: &sqlx::SqlitePool,
    reply_id: u64,
    reason: &str,
    details: Option<&str>,
    key: IdempotencyKey<'_>,
) -> Result<IdempotentMutation<(u64, u64)>, sqlx::Error> {
    let mut transaction = pool.begin().await?;
    let claim = claim_idempotency(&mut transaction, key).await?;
    if let IdempotencyClaim::Conflict = claim {
        return Ok(IdempotentMutation::Conflict);
    }
    if let IdempotencyClaim::Replay(id) = claim {
        let thread_id = sqlx::query_scalar::<_, u64>(
            "SELECT COALESCE((SELECT r.thread_id FROM replies r WHERE r.id = reports.reply_id), reports.thread_id) FROM reports WHERE reports.id = ?",
        )
        .bind(id as i64)
        .fetch_one(&mut *transaction)
        .await?;
        return Ok(IdempotentMutation::Replayed((id, thread_id)));
    }
    let Some(thread_id) = sqlx::query_scalar::<_, u64>(
        "SELECT r.thread_id FROM replies r JOIN threads t ON t.id=r.thread_id JOIN boards b ON b.id=t.board_id WHERE r.id=? AND r.status='visible' AND b.status='approved'",
    )
    .bind(reply_id as i64)
    .fetch_optional(&mut *transaction)
    .await?
    else {
        return Ok(IdempotentMutation::Conflict);
    };
    let result = sqlx::query(
        "INSERT INTO reports (reply_id, reason, details, status) VALUES (?, ?, ?, 'pending')",
    )
    .bind(reply_id as i64)
    .bind(reason)
    .bind(details)
    .execute(&mut *transaction)
    .await?;
    let report_id = result.last_insert_rowid() as u64;
    insert_projection_outbox(
        &mut transaction,
        "report_created",
        Some(thread_id),
        Some(reply_id),
        Some(report_id),
    )
    .await?;
    finish_idempotency(&mut transaction, key, report_id).await?;
    transaction.commit().await?;
    Ok(IdempotentMutation::Created((report_id, thread_id)))
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

pub(crate) async fn load_pending_reports_for_boards(
    pool: &sqlx::SqlitePool,
    board_slugs: &[String],
) -> Result<Vec<ModerationReport>, sqlx::Error> {
    if board_slugs.is_empty() {
        return Ok(Vec::new());
    }
    let placeholders = std::iter::repeat_n("?", board_slugs.len())
        .collect::<Vec<_>>()
        .join(", ");
    let query = format!(
        r#"
        SELECT
            r.id,
            CASE WHEN r.thread_id IS NOT NULL THEN r.thread_id ELSE r.reply_id END AS target_id,
            CASE WHEN r.thread_id IS NOT NULL THEN 'thread' ELSE 'reply' END AS target_kind,
            context_thread.id AS thread_id,
            b.slug AS board_slug,
            context_thread.title AS thread_title,
            CASE WHEN r.thread_id IS NOT NULL THEN reported_thread.body ELSE reported_reply.body END AS body,
            r.reason,
            r.details,
            r.created_at,
            EXISTS(
                SELECT 1 FROM post_origins po
                WHERE po.retain_until > CURRENT_TIMESTAMP
                  AND (po.thread_id = r.thread_id OR po.reply_id = r.reply_id)
            ) AS can_ban
        FROM reports r
        LEFT JOIN threads reported_thread ON reported_thread.id = r.thread_id
        LEFT JOIN replies reported_reply ON reported_reply.id = r.reply_id
        JOIN threads context_thread
            ON context_thread.id = COALESCE(r.thread_id, reported_reply.thread_id)
        JOIN boards b ON b.id = context_thread.board_id
        WHERE r.status = 'pending' AND b.slug IN ({placeholders})
        ORDER BY r.created_at ASC, r.id ASC
        "#
    );
    let mut statement = sqlx::query_as::<_, ModerationReportRow>(&query);
    for slug in board_slugs {
        statement = statement.bind(slug);
    }
    Ok(statement
        .fetch_all(pool)
        .await?
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

pub(crate) async fn load_thread_board_slug(
    pool: &sqlx::SqlitePool,
    thread_id: u64,
) -> Result<Option<String>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT b.slug FROM threads t JOIN boards b ON b.id = t.board_id WHERE t.id = ?",
    )
    .bind(thread_id as i64)
    .fetch_optional(pool)
    .await
}

pub(crate) async fn load_reply_board_slug(
    pool: &sqlx::SqlitePool,
    reply_id: u64,
) -> Result<Option<String>, sqlx::Error> {
    sqlx::query_scalar(
        r#"
        SELECT b.slug
        FROM replies r
        JOIN threads t ON t.id = r.thread_id
        JOIN boards b ON b.id = t.board_id
        WHERE r.id = ?
        "#,
    )
    .bind(reply_id as i64)
    .fetch_optional(pool)
    .await
}

pub(crate) async fn load_report_board_slug(
    pool: &sqlx::SqlitePool,
    report_id: u64,
) -> Result<Option<String>, sqlx::Error> {
    sqlx::query_scalar(
        r#"
        SELECT b.slug
        FROM reports r
        JOIN threads t ON t.id = COALESCE(r.thread_id, (SELECT thread_id FROM replies WHERE id = r.reply_id))
        JOIN boards b ON b.id = t.board_id
        WHERE r.id = ?
        "#,
    )
    .bind(report_id as i64)
    .fetch_optional(pool)
    .await
}

pub(crate) async fn set_thread_pinned(
    pool: &sqlx::SqlitePool,
    thread_id: u64,
    is_pinned: bool,
) -> Result<ThreadPinResult, sqlx::Error> {
    let mut transaction = pool.begin().await?;
    let result = sqlx::query("UPDATE threads SET is_pinned = ? WHERE id = ?")
        .bind(is_pinned)
        .bind(thread_id as i64)
        .execute(&mut *transaction)
        .await?;
    if result.rows_affected() == 0 {
        return Ok(ThreadPinResult::NotFound);
    }
    insert_projection_outbox(
        &mut transaction,
        "thread_dirty",
        Some(thread_id),
        None,
        None,
    )
    .await?;
    transaction.commit().await?;
    Ok(ThreadPinResult::Applied)
}

pub(crate) async fn pin_thread(
    pool: &sqlx::SqlitePool,
    thread_id: u64,
) -> Result<ThreadPinResult, sqlx::Error> {
    set_thread_pinned(pool, thread_id, true).await
}

pub(crate) async fn unpin_thread(
    pool: &sqlx::SqlitePool,
    thread_id: u64,
) -> Result<ThreadPinResult, sqlx::Error> {
    set_thread_pinned(pool, thread_id, false).await
}

pub(crate) async fn load_board_moderators(
    pool: &sqlx::SqlitePool,
    board_slug: &str,
) -> Result<Vec<String>, sqlx::Error> {
    sqlx::query_scalar(
        r#"
        SELECT bm.email
        FROM board_moderators bm
        JOIN boards b ON b.slug = bm.board_slug
        WHERE bm.board_slug = ?
        ORDER BY bm.email
        "#,
    )
    .bind(board_slug)
    .fetch_all(pool)
    .await
}

pub(crate) async fn add_board_moderator(
    pool: &sqlx::SqlitePool,
    board_slug: &str,
    email: &str,
) -> Result<bool, sqlx::Error> {
    let email = email.trim().to_ascii_lowercase();
    let result = sqlx::query(
        r#"
        INSERT OR IGNORE INTO board_moderators (board_slug, email)
        SELECT ?, ? WHERE EXISTS (SELECT 1 FROM boards WHERE slug = ?)
        "#,
    )
    .bind(board_slug)
    .bind(email)
    .bind(board_slug)
    .execute(pool)
    .await?;
    Ok(result.rows_affected() != 0)
}

pub(crate) async fn remove_board_moderator(
    pool: &sqlx::SqlitePool,
    board_slug: &str,
    email: &str,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query("DELETE FROM board_moderators WHERE board_slug = ? AND email = ?")
        .bind(board_slug)
        .bind(email.trim().to_ascii_lowercase())
        .execute(pool)
        .await?;
    Ok(result.rows_affected() != 0)
}

pub(crate) async fn is_board_moderator(
    pool: &sqlx::SqlitePool,
    board_slug: &str,
    email: &str,
) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar::<_, i64>(
        "SELECT EXISTS(SELECT 1 FROM board_moderators WHERE board_slug = ? AND email = ?)",
    )
    .bind(board_slug)
    .bind(email.trim().to_ascii_lowercase())
    .fetch_one(pool)
    .await
    .map(|value| value != 0)
}

async fn update_target_status(
    transaction: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    target_kind: &str,
    target_id: u64,
    action: ModerationAction,
) -> Result<Option<u64>, sqlx::Error> {
    let result = match action {
        ModerationAction::Hide | ModerationAction::Remove | ModerationAction::Quarantine => {
            if target_kind == "thread" {
                sqlx::query("UPDATE threads SET status = 'hidden' WHERE id = ? AND status IN ('visible', 'locked')")
                    .bind(target_id as i64).execute(&mut **transaction).await?
            } else {
                sqlx::query(
                    "UPDATE replies SET status = 'hidden' WHERE id = ? AND status = 'visible'",
                )
                .bind(target_id as i64)
                .execute(&mut **transaction)
                .await?
            }
        }
        ModerationAction::Lock => {
            sqlx::query("UPDATE threads SET status = 'locked' WHERE id = ? AND status = 'visible'")
                .bind(target_id as i64)
                .execute(&mut **transaction)
                .await?
        }
        ModerationAction::Dismiss | ModerationAction::Resolve => return Ok(None),
    };
    Ok(Some(result.rows_affected()))
}

pub(crate) async fn apply_direct_hide(
    pool: &sqlx::SqlitePool,
    target_kind: &str,
    target_id: u64,
    moderator_email: &str,
    reason: DirectHideReason,
    note: Option<&str>,
) -> Result<DirectHideResult, sqlx::Error> {
    if target_kind != "thread" && target_kind != "reply" {
        return Ok(DirectHideResult::InvalidTarget);
    }
    let mut transaction = pool.begin().await?;
    let visible = if target_kind == "thread" {
        sqlx::query_scalar::<_, i64>("SELECT t.id FROM threads t JOIN boards b ON b.id=t.board_id WHERE t.id=? AND t.status IN ('visible','locked') AND b.status IN ('approved','archived')")
            .bind(target_id as i64).fetch_optional(&mut *transaction).await?.is_some()
    } else {
        sqlx::query_scalar::<_, i64>("SELECT r.id FROM replies r JOIN threads t ON t.id=r.thread_id JOIN boards b ON b.id=t.board_id WHERE r.id=? AND r.status='visible' AND b.status IN ('approved','archived')")
            .bind(target_id as i64).fetch_optional(&mut *transaction).await?.is_some()
    };
    if !visible {
        return Ok(DirectHideResult::NotFound);
    }
    if update_target_status(
        &mut transaction,
        target_kind,
        target_id,
        ModerationAction::Hide,
    )
    .await?
        != Some(1)
    {
        return Ok(DirectHideResult::NotFound);
    }
    sqlx::query("INSERT INTO direct_moderation_actions (moderator_email,target_kind,target_id,reason,note) VALUES (?,?,?,?,?)")
        .bind(moderator_email).bind(target_kind).bind(target_id as i64).bind(reason.as_str()).bind(note)
        .execute(&mut *transaction).await?;
    if target_kind == "thread" {
        sqlx::query(
            "UPDATE reports SET status = 'resolved' WHERE thread_id = ? AND status = 'pending'",
        )
        .bind(target_id as i64)
        .execute(&mut *transaction)
        .await?;
    } else {
        sqlx::query(
            "UPDATE reports SET status = 'resolved' WHERE reply_id = ? AND status = 'pending'",
        )
        .bind(target_id as i64)
        .execute(&mut *transaction)
        .await?;
    }

    let event_kind = if target_kind == "thread" {
        "thread_removed"
    } else {
        "thread_dirty"
    };
    let event_thread_id = if target_kind == "thread" {
        Some(target_id)
    } else {
        Some(
            sqlx::query_scalar::<_, u64>("SELECT thread_id FROM replies WHERE id = ?")
                .bind(target_id as i64)
                .fetch_one(&mut *transaction)
                .await?,
        )
    };
    insert_projection_outbox(&mut transaction, event_kind, event_thread_id, None, None).await?;
    transaction.commit().await?;
    Ok(DirectHideResult::Applied)
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

    let target_updated =
        update_target_status(&mut transaction, target_kind, target_id, action).await?;

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
    if matches!(
        action,
        ModerationAction::Hide
            | ModerationAction::Remove
            | ModerationAction::Quarantine
            | ModerationAction::Lock
    ) {
        let event_kind = if action == ModerationAction::Lock || target_kind == "reply" {
            "thread_dirty"
        } else {
            "thread_removed"
        };
        let event_thread_id = if target_kind == "thread" {
            target_id
        } else {
            sqlx::query_scalar::<_, u64>("SELECT thread_id FROM replies WHERE id = ?")
                .bind(target_id as i64)
                .fetch_one(&mut *transaction)
                .await?
        };
        insert_projection_outbox(
            &mut transaction,
            event_kind,
            Some(event_thread_id),
            None,
            None,
        )
        .await?;
    }
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

pub(crate) async fn load_public_thread_snapshot(
    pool: &sqlx::SqlitePool,
    thread_id: u64,
    reply_limit: i64,
    reply_offset: i64,
) -> Result<Option<PublicThreadSnapshot>, sqlx::Error> {
    if !(1..=100).contains(&reply_limit) || !(0..=MAX_SNAPSHOT_REPLY_OFFSET).contains(&reply_offset)
    {
        return Err(sqlx::Error::Protocol(
            "snapshot reply limit must be between 1 and 100 and offset within safe bound"
                .to_owned(),
        ));
    }

    let mut transaction = pool.begin().await?;
    let Some(row) = sqlx::query_as::<
        _,
        (
            String,
            u64,
            String,
            String,
            String,
            String,
            bool,
            bool,
            bool,
            i64,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<u64>,
            Option<u64>,
        ),
    >(
        r#"
        SELECT b.slug, t.id, t.title, t.body, t.poster_id, t.created_at,
               t.is_pinned, (t.status = 'locked'), (t.archived_at IS NOT NULL),
               COUNT(r.id),
               pm.thumbnail_path, pm.display_path, pm.mime_type, pm.width, pm.height
        FROM threads t
        JOIN boards b ON b.id = t.board_id
        LEFT JOIN replies r ON r.thread_id = t.id AND r.status = 'visible'
        LEFT JOIN post_media pm ON pm.thread_id = t.id
        WHERE t.id = ? AND t.status IN ('visible', 'locked')
          AND b.status IN ('approved', 'archived')
        GROUP BY t.id
        "#,
    )
    .bind(thread_id as i64)
    .fetch_optional(&mut *transaction)
    .await?
    else {
        transaction.commit().await?;
        return Ok(None);
    };

    let query_limit = reply_limit + 1;
    let replies = sqlx::query_as::<
        _,
        (
            u64,
            String,
            String,
            String,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<u64>,
            Option<u64>,
        ),
    >(
        r#"
        SELECT r.id, r.body, r.poster_id, r.created_at,
               pm.thumbnail_path, pm.display_path, pm.mime_type, pm.width, pm.height
        FROM replies r
        LEFT JOIN post_media pm ON pm.reply_id = r.id
        WHERE r.thread_id = ? AND r.status = 'visible'
        ORDER BY r.created_at, r.id
        LIMIT ? OFFSET ?
        "#,
    )
    .bind(thread_id as i64)
    .bind(query_limit)
    .bind(reply_offset)
    .fetch_all(&mut *transaction)
    .await?;
    let has_next_replies = replies.len() > reply_limit as usize;
    let replies = replies
        .into_iter()
        .take(reply_limit as usize)
        .map(
            |(id, body, poster_id, created_at, thumb, display, mime, width, height)| {
                PublicReplySnapshot {
                    id,
                    body,
                    poster_id,
                    created_at,
                    media: media_from_parts(thumb, display, mime, width, height),
                }
            },
        )
        .collect();
    let snapshot = PublicThreadSnapshot {
        id: row.1,
        board_slug: row.0,
        title: row.2,
        body: row.3,
        poster_id: row.4,
        created_at: row.5,
        is_pinned: row.6,
        is_locked: row.7,
        is_archived: row.8,
        reply_count: row.9 as u64,
        media: media_from_parts(row.10, row.11, row.12, row.13, row.14),
        replies,
        has_next_replies,
    };
    transaction.commit().await?;
    Ok(Some(snapshot))
}

pub(crate) async fn load_active_board_thread_ids(
    pool: &sqlx::SqlitePool,
    board_slug: &str,
    limit: i64,
) -> Result<Vec<u64>, sqlx::Error> {
    if !(1..=100).contains(&limit) {
        return Err(sqlx::Error::Protocol(
            "active board thread limit must be between 1 and 100".to_owned(),
        ));
    }
    sqlx::query_scalar(
        r#"
        SELECT t.id
        FROM threads t
        JOIN boards b ON b.id = t.board_id
        WHERE b.slug = ? AND b.status = 'approved'
          AND t.status IN ('visible', 'locked')
          AND t.archived_at IS NULL
        ORDER BY COALESCE(t.is_pinned, 0) DESC, t.bumped_at DESC, t.id DESC
        LIMIT ?
        "#,
    )
    .bind(board_slug)
    .bind(limit)
    .fetch_all(pool)
    .await
}

pub(crate) async fn lease_projection_outbox(
    pool: &sqlx::SqlitePool,
    limit: i64,
    lease_seconds: i64,
) -> Result<Vec<ProjectionOutboxEvent>, sqlx::Error> {
    if limit < 1 {
        return Err(sqlx::Error::Protocol(
            "outbox lease limit must be positive".to_owned(),
        ));
    }
    let limit = limit.min(100);
    let lease_seconds = lease_seconds.clamp(1, 86_400);
    let token = Uuid::new_v4().to_string();
    let modifier = format!("+{lease_seconds} seconds");
    let mut transaction = pool.begin().await?;
    sqlx::query(
        r#"
        UPDATE projection_outbox
        SET lease_token = ?, lease_expires_at = datetime('now', ?)
        WHERE acknowledged_at IS NULL
          AND (lease_expires_at IS NULL OR lease_expires_at <= CURRENT_TIMESTAMP)
          AND id IN (
            SELECT id FROM projection_outbox
            WHERE acknowledged_at IS NULL
              AND (lease_expires_at IS NULL OR lease_expires_at <= CURRENT_TIMESTAMP)
            ORDER BY id LIMIT ?
          )
        "#,
    )
    .bind(&token)
    .bind(&modifier)
    .bind(limit)
    .execute(&mut *transaction)
    .await?;
    let rows = sqlx::query_as::<_, ProjectionOutboxRow>(
        r#"
        SELECT id, kind, thread_id, reply_id, report_id, created_at,
               lease_token, lease_expires_at
        FROM projection_outbox
        WHERE lease_token = ? AND acknowledged_at IS NULL
        ORDER BY id
        "#,
    )
    .bind(&token)
    .fetch_all(&mut *transaction)
    .await?;
    transaction.commit().await?;
    Ok(rows
        .into_iter()
        .map(|row| ProjectionOutboxEvent {
            id: row.id,
            kind: row.kind,
            thread_id: row.thread_id,
            reply_id: row.reply_id,
            report_id: row.report_id,
            created_at: row.created_at,
            lease_token: row.lease_token,
            lease_expires_at: row.lease_expires_at,
        })
        .collect())
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum OutboxAck {
    Acknowledged,
    NotFound,
    LeaseMismatch,
}

pub(crate) async fn acknowledge_projection_outbox(
    pool: &sqlx::SqlitePool,
    event_id: u64,
    lease_token: &str,
) -> Result<OutboxAck, sqlx::Error> {
    let mut transaction = pool.begin().await?;
    let Some((current_token, acknowledged_at)) =
        sqlx::query_as::<_, (Option<String>, Option<String>)>(
            "SELECT lease_token, acknowledged_at FROM projection_outbox WHERE id = ?",
        )
        .bind(event_id as i64)
        .fetch_optional(&mut *transaction)
        .await?
    else {
        return Ok(OutboxAck::NotFound);
    };
    if current_token.as_deref() != Some(lease_token) {
        return Ok(OutboxAck::LeaseMismatch);
    }
    if acknowledged_at.is_none() {
        sqlx::query(
            "UPDATE projection_outbox SET acknowledged_at = CURRENT_TIMESTAMP WHERE id = ? AND lease_token = ? AND acknowledged_at IS NULL",
        )
        .bind(event_id as i64)
        .bind(lease_token)
        .execute(&mut *transaction)
        .await?;
    }
    transaction.commit().await?;
    Ok(OutboxAck::Acknowledged)
}

pub(crate) async fn purge_acknowledged_projection_outbox(
    pool: &sqlx::SqlitePool,
    retention_seconds: i64,
) -> Result<u64, sqlx::Error> {
    let retention_seconds = retention_seconds.max(0);
    let modifier = format!("-{retention_seconds} seconds");
    let result = sqlx::query(
        "DELETE FROM projection_outbox WHERE acknowledged_at IS NOT NULL AND acknowledged_at <= datetime('now', ?)",
    )
    .bind(modifier)
    .execute(pool)
    .await?;
    Ok(result.rows_affected())
}
pub(crate) async fn load_abuse_logs(
    pool: &sqlx::SqlitePool,
) -> Result<Vec<EncryptedAbuseLog>, sqlx::Error> {
    sqlx::query_as::<_, EncryptedAbuseLog>(
        r#"
        SELECT
            CASE WHEN thread_id IS NOT NULL THEN 'thread' ELSE 'reply' END AS target_kind,
            CASE WHEN thread_id IS NOT NULL THEN thread_id ELSE reply_id END AS target_id,
            nonce, ciphertext, created_at, retain_until
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
            t.created_at AS thread_created_at,
            t.is_pinned AS thread_is_pinned,
            t.status AS thread_status,
            t.archived_at AS thread_archived_at,
            b.status AS board_status,
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
            AND b.status IN ('approved', 'archived')
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
            r.created_at,
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
            is_archived: thread_row.board_status == "archived",
        },
        Thread {
            id: thread_row.thread_id,
            poster_id: thread_row.poster_id,
            title: thread_row.thread_title,
            created_at: thread_row.thread_created_at,
            is_pinned: thread_row.thread_is_pinned,
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
mod telegram_tests;

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
            create_reply(&pool, 1, "must not be inserted", &origin, None)
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
            create_reply(&pool, 2, "should not be inserted", &origin, None)
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

        assert!(matches!(
            create_reply(
                &pool,
                1,
                "Reply media transaction body",
                &origin,
                Some(&media),
            )
            .await
            .unwrap(),
            CreateReplyResult::Created(_)
        ));

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
            create_reply(&pool, 1, "Reply media rollback body", &origin, Some(&media),)
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
    #[sqlx::test(migrator = "MIGRATOR")]
    async fn new_threads_initialize_bump_timestamp(pool: sqlx::SqlitePool) {
        let origin = crate::abuse::ProtectedClient {
            fingerprint: [51; 32],
            nonce: [52; 12],
            ciphertext: vec![53, 54, 55],
        };

        let thread_id = create_thread(
            &pool,
            "engineering",
            "Bump initialization",
            "A thread should begin with its creation timestamp as its bump.",
            &origin,
            None,
        )
        .await
        .unwrap()
        .unwrap();

        let timestamps = sqlx::query_as::<_, (String, String)>(
            "SELECT created_at, bumped_at FROM threads WHERE id = ?",
        )
        .bind(thread_id as i64)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(timestamps.0, timestamps.1);
    }

    #[sqlx::test(migrator = "MIGRATOR")]
    async fn successful_reply_replaces_thread_bump_timestamp(pool: sqlx::SqlitePool) {
        sqlx::query("UPDATE threads SET bumped_at = '2000-01-01 00:00:00' WHERE id = 1")
            .execute(&pool)
            .await
            .unwrap();
        let origin = crate::abuse::ProtectedClient {
            fingerprint: [61; 32],
            nonce: [62; 12],
            ciphertext: vec![63, 64, 65],
        };

        let reply_id = match create_reply(&pool, 1, "Bump the thread", &origin, None)
            .await
            .unwrap()
        {
            CreateReplyResult::Created(reply_id) => reply_id,
            other => panic!("expected reply creation, got {other:?}"),
        };

        let timestamps = sqlx::query_as::<_, (String, String)>(
            r#"
            SELECT t.bumped_at, r.created_at
            FROM threads t
            JOIN replies r ON r.thread_id = t.id
            WHERE t.id = ? AND r.id = ?
            "#,
        )
        .bind(1_i64)
        .bind(reply_id as i64)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_ne!(timestamps.0, "2000-01-01 00:00:00");
        assert_eq!(timestamps.0, timestamps.1);
    }

    #[sqlx::test(migrator = "MIGRATOR")]
    async fn failed_reply_rolls_back_bump_update(pool: sqlx::SqlitePool) {
        sqlx::query("UPDATE threads SET bumped_at = '2000-01-01 00:00:00' WHERE id = 1")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query(
            r#"
            CREATE TRIGGER reject_reply_bump_media
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
            fingerprint: [71; 32],
            nonce: [72; 12],
            ciphertext: vec![73, 74, 75],
        };
        let media = Media {
            thumbnail_path: String::from("/thumb/rejected-bump.webp"),
            display_path: String::from("/media/rejected-bump.webp"),
            mime_type: String::from("image/webp"),
            width: 640,
            height: 480,
        };

        assert!(
            create_reply(&pool, 1, "This reply must roll back", &origin, Some(&media),)
                .await
                .is_err()
        );

        let bump = sqlx::query_scalar::<_, String>("SELECT bumped_at FROM threads WHERE id = 1")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(bump, "2000-01-01 00:00:00");
        let reply_count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM replies WHERE body = 'This reply must roll back'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(reply_count, 0);
    }

    #[sqlx::test(migrator = "MIGRATOR")]
    async fn visible_reply_bump_limit_and_active_listing_order_are_deterministic(
        pool: sqlx::SqlitePool,
    ) {
        sqlx::query(
            r#"
            INSERT INTO boards (slug, name, description, status)
            VALUES ('bump-limit-fixture', 'Bump limit fixture', 'Bump limit fixture', 'approved')
            "#,
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            r#"
            INSERT INTO threads (
                id, board_id, title, body, status, created_at, bumped_at, is_pinned, archived_at
            )
            VALUES
                (
                    3001,
                    (SELECT id FROM boards WHERE slug = 'bump-limit-fixture'),
                    'Capped old thread',
                    'Capped old thread body',
                    'visible',
                    '2000-01-01 00:00:00',
                    '2000-01-01 00:00:00',
                    0,
                    NULL
                ),
                (
                    3002,
                    (SELECT id FROM boards WHERE slug = 'bump-limit-fixture'),
                    'Newer thread',
                    'Newer thread body',
                    'visible',
                    '2000-01-02 00:00:00',
                    '2000-01-02 00:00:00',
                    NULL,
                    NULL
                )
            "#,
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            r#"
            WITH RECURSIVE numbers(n) AS (
                SELECT 1
                UNION ALL
                SELECT n + 1 FROM numbers WHERE n < 298
            )
            INSERT INTO replies (thread_id, body, poster_id, status, created_at)
            SELECT 3001, 'visible-' || n, 'poster-' || n, 'visible',
                printf('2000-01-01 00:%02d:%02d', n / 60, n % 60)
            FROM numbers
            UNION ALL
            SELECT 3001, 'hidden', 'hidden-poster', 'hidden', '2000-01-02 00:00:00'
            "#,
        )
        .execute(&pool)
        .await
        .unwrap();

        let counts = sqlx::query_as::<_, (i64, i64)>(
            r#"
            SELECT
                COUNT(*) FILTER (WHERE status = 'visible'),
                COUNT(*) FILTER (WHERE status = 'hidden')
            FROM replies
            WHERE thread_id = 3001
            "#,
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(counts, (298, 1));

        let origin = crate::abuse::ProtectedClient {
            fingerprint: [94; 32],
            nonce: [95; 12],
            ciphertext: vec![96],
        };
        let old_bump = "2000-01-01 00:00:00";
        assert!(matches!(
            create_reply(&pool, 3001, "Reply 299", &origin, None)
                .await
                .unwrap(),
            CreateReplyResult::Created(_)
        ));
        let bump_after_299 =
            sqlx::query_scalar::<_, String>("SELECT bumped_at FROM threads WHERE id = 3001")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_ne!(bump_after_299, old_bump);

        sqlx::query("UPDATE threads SET bumped_at = ? WHERE id = 3001")
            .bind(old_bump)
            .execute(&pool)
            .await
            .unwrap();
        assert!(matches!(
            create_reply(&pool, 3001, "Reply 300", &origin, None)
                .await
                .unwrap(),
            CreateReplyResult::Created(_)
        ));
        let bump_after_300 =
            sqlx::query_scalar::<_, String>("SELECT bumped_at FROM threads WHERE id = 3001")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_ne!(bump_after_300, old_bump);

        sqlx::query("UPDATE threads SET bumped_at = ? WHERE id = 3001")
            .bind(old_bump)
            .execute(&pool)
            .await
            .unwrap();
        assert!(matches!(
            create_reply(&pool, 3001, "Reply 301", &origin, None)
                .await
                .unwrap(),
            CreateReplyResult::Created(_)
        ));
        let bump_after_301 =
            sqlx::query_scalar::<_, String>("SELECT bumped_at FROM threads WHERE id = 3001")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(bump_after_301, old_bump);
        let counts = sqlx::query_as::<_, (i64, i64)>(
            "SELECT COUNT(*) FILTER (WHERE status = 'visible'), \
                    COUNT(*) FILTER (WHERE status = 'hidden') \
             FROM replies WHERE thread_id = 3001",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(counts, (301, 1));

        let page = load_board_page(&pool, "bump-limit-fixture", 2, 0)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            page.board
                .threads
                .into_iter()
                .map(|thread| thread.id)
                .collect::<Vec<_>>(),
            vec![3002, 3001]
        );
    }

    #[sqlx::test(migrator = "MIGRATOR")]
    async fn hidden_later_reply_does_not_replace_visible_bump(pool: sqlx::SqlitePool) {
        sqlx::query("UPDATE threads SET bumped_at = '2030-01-01 00:00:00' WHERE id = 1")
            .execute(&pool)
            .await
            .unwrap();
        let origin = crate::abuse::ProtectedClient {
            fingerprint: [81; 32],
            nonce: [82; 12],
            ciphertext: vec![83, 84, 85],
        };
        let reply_id = match create_reply(&pool, 1, "A reply that will be hidden", &origin, None)
            .await
            .unwrap()
        {
            CreateReplyResult::Created(reply_id) => reply_id,
            other => panic!("expected reply creation, got {other:?}"),
        };
        sqlx::query("UPDATE replies SET created_at = '2030-01-02 00:00:00' WHERE id = ?")
            .bind(reply_id as i64)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("UPDATE threads SET bumped_at = '2030-01-02 00:00:00' WHERE id = 1")
            .execute(&pool)
            .await
            .unwrap();
        let matching_report_id = sqlx::query(
            "INSERT INTO reports (reply_id, reason, status) VALUES (?, 'spam', 'pending')",
        )
        .bind(reply_id as i64)
        .execute(&pool)
        .await
        .unwrap()
        .last_insert_rowid();
        let unrelated_report_id = sqlx::query(
            "INSERT INTO reports (thread_id, reason, status) VALUES (1, 'spam', 'pending')",
        )
        .execute(&pool)
        .await
        .unwrap()
        .last_insert_rowid();

        assert_eq!(
            apply_direct_hide(
                &pool,
                "reply",
                reply_id,
                "moderator@example.com",
                DirectHideReason::Spam,
                None,
            )
            .await
            .unwrap(),
            DirectHideResult::Applied
        );
        let matching_report_status =
            sqlx::query_scalar::<_, String>("SELECT status FROM reports WHERE id = ?")
                .bind(matching_report_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(matching_report_status, "resolved");
        let unrelated_report_status =
            sqlx::query_scalar::<_, String>("SELECT status FROM reports WHERE id = ?")
                .bind(unrelated_report_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(unrelated_report_status, "pending");

        let bump = sqlx::query_scalar::<_, String>("SELECT bumped_at FROM threads WHERE id = 1")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(bump, "2030-01-02 00:00:00");
        let thread = load_board(&pool, "engineering")
            .await
            .unwrap()
            .unwrap()
            .threads
            .into_iter()
            .find(|thread| thread.id == 1)
            .unwrap();
        assert_eq!(thread.reply_count, 2);
    }

    #[sqlx::test(migrator = "MIGRATOR")]
    async fn pin_and_bump_ordering_spans_pages_and_unpin_restores_normal_order(
        pool: sqlx::SqlitePool,
    ) {
        sqlx::query(
            r#"
            INSERT INTO boards (slug, name, description, status)
            VALUES ('ordering-fixture', 'Ordering fixture', 'Thread ordering fixture', 'approved')
            "#,
        )
        .execute(&pool)
        .await
        .unwrap();
        let fixtures = [
            ("unpinned newest", "2030-01-05 00:00:00", None),
            ("pinned older", "2030-01-01 00:00:00", Some(true)),
            ("pinned newer", "2030-01-03 00:00:00", Some(true)),
            ("unpinned oldest", "2030-01-04 00:00:00", Some(false)),
            ("unpinned tie", "2030-01-05 00:00:00", Some(false)),
        ];
        let mut ids = Vec::new();
        for (title, bumped_at, is_pinned) in fixtures {
            let id = sqlx::query(
                r#"
                INSERT INTO threads (
                    board_id, title, body, status, created_at, bumped_at, is_pinned
                )
                VALUES (
                    (SELECT id FROM boards WHERE slug = 'ordering-fixture'),
                    ?, 'Ordering body', 'visible', '2030-01-01 00:00:00', ?, ?
                )
                "#,
            )
            .bind(title)
            .bind(bumped_at)
            .bind(is_pinned)
            .execute(&pool)
            .await
            .unwrap()
            .last_insert_rowid() as u64;
            ids.push(id);
        }

        let page_ids = |page: BoardPage| {
            page.board
                .threads
                .into_iter()
                .map(|thread| thread.id)
                .collect::<Vec<_>>()
        };
        let first_page = load_board_page(&pool, "ordering-fixture", 2, 0)
            .await
            .unwrap()
            .unwrap();
        assert!(first_page.has_next);
        assert_eq!(page_ids(first_page), vec![ids[2], ids[1]]);
        let second_page = load_board_page(&pool, "ordering-fixture", 2, 2)
            .await
            .unwrap()
            .unwrap();
        assert!(second_page.has_next);
        assert_eq!(page_ids(second_page), vec![ids[4], ids[0]]);
        let third_page = load_board_page(&pool, "ordering-fixture", 2, 4)
            .await
            .unwrap()
            .unwrap();
        assert!(!third_page.has_next);
        assert_eq!(page_ids(third_page), vec![ids[3]]);

        assert_eq!(
            pin_thread(&pool, ids[3]).await.unwrap(),
            ThreadPinResult::Applied
        );
        let pinned_page = load_board_page(&pool, "ordering-fixture", 5, 0)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            page_ids(pinned_page),
            vec![ids[3], ids[2], ids[1], ids[4], ids[0]]
        );

        assert_eq!(
            unpin_thread(&pool, ids[3]).await.unwrap(),
            ThreadPinResult::Applied
        );
        let unpinned_page = load_board_page(&pool, "ordering-fixture", 5, 0)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            page_ids(unpinned_page),
            vec![ids[2], ids[1], ids[4], ids[0], ids[3]]
        );
    }

    #[sqlx::test(migrator = "MIGRATOR")]
    async fn board_moderator_assignments_list_normalize_deduplicate_and_remove(
        pool: sqlx::SqlitePool,
    ) {
        assert!(
            load_board_moderators(&pool, "engineering")
                .await
                .unwrap()
                .is_empty()
        );

        assert!(
            add_board_moderator(&pool, "engineering", "  Moderator@Example.COM ")
                .await
                .unwrap()
        );
        assert!(
            !add_board_moderator(&pool, "engineering", "MODERATOR@example.com")
                .await
                .unwrap()
        );
        assert!(
            add_board_moderator(&pool, "engineering", "Second@Example.com")
                .await
                .unwrap()
        );
        assert_eq!(
            load_board_moderators(&pool, "engineering").await.unwrap(),
            vec![
                String::from("moderator@example.com"),
                String::from("second@example.com")
            ]
        );
        assert!(
            is_board_moderator(&pool, "engineering", " MODERATOR@EXAMPLE.COM ")
                .await
                .unwrap()
        );
        assert!(
            !is_board_moderator(&pool, "engineering", "missing@example.com")
                .await
                .unwrap()
        );

        assert!(
            remove_board_moderator(&pool, "engineering", " MODERATOR@EXAMPLE.COM ")
                .await
                .unwrap()
        );
        assert!(
            !remove_board_moderator(&pool, "engineering", "moderator@example.com")
                .await
                .unwrap()
        );
        assert_eq!(
            load_board_moderators(&pool, "engineering").await.unwrap(),
            vec![String::from("second@example.com")]
        );
        assert!(
            !add_board_moderator(&pool, "missing-board", "new@example.com")
                .await
                .unwrap()
        );
    }
}
