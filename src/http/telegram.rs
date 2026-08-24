use super::*;
use axum::{
    Json,
    body::to_bytes,
    extract::{Path, Query, Request, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::Arc;

const JSON_LIMIT: usize = 64 * 1024;
const SERVICE: &str = "telegram";
const OP_THREAD_CREATE: &str = "thread.create";
const OP_REPLY_CREATE: &str = "reply.create";
const OP_THREAD_REPORT: &str = "thread.report";
const OP_REPLY_REPORT: &str = "reply.report";

fn hash_frame(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_be_bytes());
    hasher.update(bytes);
}

fn hash_optional_frame(hasher: &mut Sha256, value: Option<&[u8]>) {
    match value {
        Some(bytes) => {
            hasher.update([1]);
            hash_frame(hasher, bytes);
        }
        None => hasher.update([0]),
    }
}

fn hash_operation(operation: &str, principal_digest: &[u8; 32]) -> Sha256 {
    let mut hasher = Sha256::new();
    hash_frame(&mut hasher, operation.as_bytes());
    hash_frame(&mut hasher, principal_digest);
    hasher
}

fn hash_u64_frame(hasher: &mut Sha256, value: u64) {
    hash_frame(hasher, &value.to_be_bytes());
}

fn telegram_origin_key(principal: &str) -> String {
    format!("\0mchan-client:telegram:\0{}", principal)
}

fn validate_principal(principal: &str) -> Result<(), Response> {
    let len = principal.as_bytes().len();
    if !(1..=256).contains(&len) {
        return Err(telegram_error(
            StatusCode::BAD_REQUEST,
            "principal must be 1..256 bytes UTF-8",
        ));
    }
    Ok(())
}

fn validate_idempotency_key(key: &str) -> Result<(), Response> {
    let len = key.as_bytes().len();
    if !(1..=512).contains(&len) {
        return Err(telegram_error(
            StatusCode::BAD_REQUEST,
            "idempotency_key must be 1..512 bytes",
        ));
    }
    Ok(())
}

const CONCURRENT_COMMIT_POLLS: usize = 100;
const CONCURRENT_COMMIT_POLL_INTERVAL: std::time::Duration = std::time::Duration::from_millis(50);

enum ConcurrentCommit {
    Committed(u64),
    Mismatched,
    Unresolved,
}

async fn await_concurrent_idempotent_commit(
    state: &HttpDependencies,
    key: forum::IdempotencyKey<'_>,
) -> ConcurrentCommit {
    for _ in 0..CONCURRENT_COMMIT_POLLS {
        tokio::time::sleep(CONCURRENT_COMMIT_POLL_INTERVAL).await;
        match forum::check_machine_idempotency(&state.pool, key).await {
            Ok(forum::IdempotencyCheck::Replayed(id)) => return ConcurrentCommit::Committed(id),
            Ok(forum::IdempotencyCheck::Conflict) => return ConcurrentCommit::Mismatched,
            Ok(forum::IdempotencyCheck::New) => {}
            Err(_) => {}
        }
    }
    ConcurrentCommit::Unresolved
}

fn telegram_error(status: StatusCode, message: &str) -> Response {
    let body = serde_json::json!({ "status": "error", "error": message });
    (status, no_store_headers(), Json(body)).into_response()
}

fn telegram_json<T: Serialize>(status: StatusCode, value: T) -> Response {
    (status, no_store_headers(), Json(value)).into_response()
}

fn is_json(headers: &HeaderMap) -> bool {
    is_content_type(headers, "application/json")
}

fn is_multipart(headers: &HeaderMap) -> bool {
    is_content_type(headers, "multipart/form-data")
}

async fn read_json_limited<T: for<'de> Deserialize<'de>>(request: Request) -> Result<T, Response> {
    if !is_json(request.headers()) {
        return Err(telegram_error(
            StatusCode::BAD_REQUEST,
            "Content-Type must be application/json",
        ));
    }
    let headers = request.headers().clone();
    // Check content-length early if present
    if let Some(len) = headers
        .get(axum::http::header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<usize>().ok())
    {
        if len > JSON_LIMIT {
            return Err(telegram_error(
                StatusCode::PAYLOAD_TOO_LARGE,
                "JSON body too large",
            ));
        }
    }
    let body = request.into_body();
    let bytes = to_bytes(body, JSON_LIMIT)
        .await
        .map_err(|_| telegram_error(StatusCode::PAYLOAD_TOO_LARGE, "JSON body too large"))?;
    if bytes.len() > JSON_LIMIT {
        return Err(telegram_error(
            StatusCode::PAYLOAD_TOO_LARGE,
            "JSON body too large",
        ));
    }
    serde_json::from_slice::<T>(&bytes)
        .map_err(|e| telegram_error(StatusCode::BAD_REQUEST, &format!("invalid JSON: {e}")))
}

fn check_telegram_auth(headers: &HeaderMap, state: &HttpDependencies) -> Result<(), Response> {
    let Some(expected) = state.telegram_service_token.as_deref() else {
        return Err(telegram_error(StatusCode::NOT_FOUND, "endpoint disabled"));
    };
    if !bearer_matches(headers, expected) {
        return Err(telegram_error(StatusCode::UNAUTHORIZED, "unauthorized"));
    }
    Ok(())
}

fn canonical_media_hash(file: Option<&media::MediaUpload>, hasher: &mut Sha256) {
    let Some(file) = file else {
        hasher.update([0]);
        return;
    };
    hasher.update([1]);
    hash_optional_frame(hasher, file.filename.as_deref().map(str::as_bytes));
    hash_optional_frame(hasher, file.content_type.as_deref().map(str::as_bytes));
    hash_frame(hasher, &file.bytes);
}

fn thread_request_hash(
    principal_digest: &[u8; 32],
    board_slug: &str,
    title: &str,
    body: &str,
    file: Option<&media::MediaUpload>,
) -> [u8; 32] {
    let mut h = hash_operation(OP_THREAD_CREATE, principal_digest);
    hash_frame(&mut h, board_slug.as_bytes());
    hash_frame(&mut h, title.as_bytes());
    hash_frame(&mut h, body.as_bytes());
    canonical_media_hash(file, &mut h);
    h.finalize().into()
}

fn reply_request_hash(
    principal_digest: &[u8; 32],
    thread_id: u64,
    body: &str,
    file: Option<&media::MediaUpload>,
) -> [u8; 32] {
    let mut h = hash_operation(OP_REPLY_CREATE, principal_digest);
    hash_u64_frame(&mut h, thread_id);
    hash_frame(&mut h, body.as_bytes());
    canonical_media_hash(file, &mut h);
    h.finalize().into()
}

fn report_thread_hash(
    principal_digest: &[u8; 32],
    thread_id: u64,
    reason: &str,
    details: Option<&str>,
) -> [u8; 32] {
    let mut h = hash_operation(OP_THREAD_REPORT, principal_digest);
    hash_u64_frame(&mut h, thread_id);
    hash_frame(&mut h, reason.as_bytes());
    hash_optional_frame(&mut h, details.map(str::as_bytes));
    h.finalize().into()
}

fn report_reply_hash(
    principal_digest: &[u8; 32],
    reply_id: u64,
    reason: &str,
    details: Option<&str>,
) -> [u8; 32] {
    let mut h = hash_operation(OP_REPLY_REPORT, principal_digest);
    hash_u64_frame(&mut h, reply_id);
    hash_frame(&mut h, reason.as_bytes());
    hash_optional_frame(&mut h, details.map(str::as_bytes));
    h.finalize().into()
}

#[derive(Serialize)]
struct ThreadIdsResponse {
    thread_ids: Vec<u64>,
}

#[derive(Serialize)]
struct MediaView {
    thumbnail_path: String,
    display_path: String,
    mime_type: String,
    width: u64,
    height: u64,
}

#[derive(Serialize)]
struct ReplyView {
    id: u64,
    body: String,
    poster_id: String,
    created_at: String,
    media: Option<MediaView>,
}

#[derive(Serialize)]
struct SnapshotResponse {
    id: u64,
    board_slug: String,
    title: String,
    body: String,
    poster_id: String,
    created_at: String,
    is_pinned: bool,
    is_locked: bool,
    is_archived: bool,
    reply_count: u64,
    media: Option<MediaView>,
    replies: Vec<ReplyView>,
    has_next_replies: bool,
}

fn media_view(m: &forum::Media) -> MediaView {
    MediaView {
        thumbnail_path: m.thumbnail_path.clone(),
        display_path: m.display_path.clone(),
        mime_type: m.mime_type.clone(),
        width: m.width,
        height: m.height,
    }
}

#[derive(Deserialize)]
struct BackfillParams {
    limit: Option<String>,
}

async fn backfill_handler(
    State(state): State<Arc<HttpDependencies>>,
    headers: HeaderMap,
    Path(slug): Path<String>,
    Query(params): Query<BackfillParams>,
) -> Response {
    if let Err(resp) = check_telegram_auth(&headers, &state) {
        return resp;
    }
    let limit: i64 = match params.limit.as_deref() {
        Some(raw) => match raw.parse::<i64>() {
            Ok(parsed) => parsed,
            Err(_) => {
                return telegram_error(StatusCode::BAD_REQUEST, "limit must be integer 1..100");
            }
        },
        None => 20,
    };
    if !(1..=100).contains(&limit) {
        return telegram_error(StatusCode::BAD_REQUEST, "limit must be between 1 and 100");
    }
    match forum::approved_board_exists(&state.pool, &slug).await {
        Ok(true) => {}
        Ok(false) => return telegram_error(StatusCode::NOT_FOUND, "board not found"),
        Err(_) => return telegram_error(StatusCode::INTERNAL_SERVER_ERROR, "database error"),
    }
    match forum::load_active_board_thread_ids(&state.pool, &slug, limit).await {
        Ok(ids) => telegram_json(StatusCode::OK, ThreadIdsResponse { thread_ids: ids }),
        Err(_) => telegram_error(StatusCode::INTERNAL_SERVER_ERROR, "database error"),
    }
}

#[derive(Deserialize)]
struct SnapshotQuery {
    reply_limit: Option<String>,
    reply_offset: Option<String>,
    limit: Option<String>,
    offset: Option<String>,
}

async fn snapshot_handler(
    State(state): State<Arc<HttpDependencies>>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(params): Query<SnapshotQuery>,
) -> Response {
    if let Err(resp) = check_telegram_auth(&headers, &state) {
        return resp;
    }
    let thread_id: u64 = match id.parse() {
        Ok(v) => v,
        Err(_) => return telegram_error(StatusCode::NOT_FOUND, "thread not found"),
    };
    let reply_limit: i64 = params
        .reply_limit
        .or(params.limit)
        .map(|s| s.parse::<i64>().unwrap_or(-1))
        .unwrap_or(20);
    let reply_offset: i64 = params
        .reply_offset
        .or(params.offset)
        .map(|s| s.parse::<i64>().unwrap_or(-1))
        .unwrap_or(0);
    if !(1..=100).contains(&reply_limit) {
        return telegram_error(
            StatusCode::BAD_REQUEST,
            "reply_limit must be between 1 and 100",
        );
    }
    if !(0..=10_000).contains(&reply_offset) {
        return telegram_error(StatusCode::BAD_REQUEST, "reply_offset out of bounds");
    }
    match forum::load_public_thread_snapshot(&state.pool, thread_id, reply_limit, reply_offset)
        .await
    {
        Ok(Some(snap)) => {
            let resp = SnapshotResponse {
                id: snap.id,
                board_slug: snap.board_slug,
                title: snap.title,
                body: snap.body,
                poster_id: snap.poster_id,
                created_at: snap.created_at,
                is_pinned: snap.is_pinned,
                is_locked: snap.is_locked,
                is_archived: snap.is_archived,
                reply_count: snap.reply_count,
                media: snap.media.as_ref().map(media_view),
                replies: snap
                    .replies
                    .into_iter()
                    .map(|r| ReplyView {
                        id: r.id,
                        body: r.body,
                        poster_id: r.poster_id,
                        created_at: r.created_at,
                        media: r.media.as_ref().map(media_view),
                    })
                    .collect(),
                has_next_replies: snap.has_next_replies,
            };
            telegram_json(StatusCode::OK, resp)
        }
        Ok(None) => telegram_error(StatusCode::NOT_FOUND, "thread not found"),
        Err(_) => telegram_error(StatusCode::INTERNAL_SERVER_ERROR, "database error"),
    }
}

// --- helpers for parsing telegram create requests ---

#[derive(Deserialize)]
struct CreateThreadJson {
    principal: String,
    #[serde(rename = "idempotency_key")]
    idempotency_key: String,
    title: String,
    body: String,
}

#[derive(Deserialize)]
struct CreateReplyJson {
    principal: String,
    #[serde(rename = "idempotency_key")]
    idempotency_key: String,
    body: String,
}

#[derive(Deserialize)]
struct ReportJson {
    principal: String,
    #[serde(rename = "idempotency_key")]
    idempotency_key: String,
    reason: String,
    details: Option<String>,
}

struct ParsedThreadCreate {
    principal: String,
    idempotency_key: String,
    title: String,
    body: String,
    file: Option<media::MediaUpload>,
}

struct ParsedReplyCreate {
    principal: String,
    idempotency_key: String,
    body: String,
    file: Option<media::MediaUpload>,
}

const PRINCIPAL_FIELD_MAX_BYTES: usize = 256;
const IDEMPOTENCY_KEY_FIELD_MAX_BYTES: usize = 512;
const TITLE_FIELD_MAX_BYTES: usize = 4 * 1024;
const BODY_FIELD_MAX_BYTES: usize = 64 * 1024;

async fn capped_field_text(
    field: &mut axum::extract::multipart::Field<'_>,
    max_bytes: usize,
    invalid_message: &'static str,
) -> Result<String, Response> {
    let mut buffer: Vec<u8> = Vec::new();
    while let Some(chunk) = field
        .chunk()
        .await
        .map_err(|_| telegram_error(StatusCode::BAD_REQUEST, invalid_message))?
    {
        if buffer.len().saturating_add(chunk.len()) > max_bytes {
            return Err(telegram_error(
                StatusCode::BAD_REQUEST,
                "multipart field too large",
            ));
        }
        buffer.extend_from_slice(&chunk);
    }
    String::from_utf8(buffer).map_err(|_| telegram_error(StatusCode::BAD_REQUEST, invalid_message))
}

async fn parse_thread_create(request: Request) -> Result<ParsedThreadCreate, Response> {
    let headers = request.headers().clone();
    if is_json(&headers) {
        let json: CreateThreadJson = read_json_limited(request).await?;
        Ok(ParsedThreadCreate {
            principal: json.principal,
            idempotency_key: json.idempotency_key,
            title: json.title,
            body: json.body,
            file: None,
        })
    } else if is_multipart(&headers) {
        let mut multipart = Multipart::from_request(request, &())
            .await
            .map_err(|_| telegram_error(StatusCode::BAD_REQUEST, "malformed multipart"))?;
        let mut principal = None;
        let mut idempotency_key = None;
        let mut title = None;
        let mut body = None;
        let mut file: Option<media::MediaUpload> = None;
        let mut file_seen = false;
        while let Some(mut field) = multipart.next_field().await.map_err(|e| {
            if e.status() == StatusCode::PAYLOAD_TOO_LARGE {
                telegram_error(StatusCode::PAYLOAD_TOO_LARGE, "Uploaded file is too large")
            } else {
                telegram_error(StatusCode::BAD_REQUEST, "malformed multipart")
            }
        })? {
            let name = field
                .name()
                .ok_or_else(|| telegram_error(StatusCode::BAD_REQUEST, "missing field name"))?
                .to_owned();
            match name.as_str() {
                "principal" => {
                    if principal.is_some() {
                        return Err(telegram_error(
                            StatusCode::BAD_REQUEST,
                            "duplicate principal",
                        ));
                    }
                    principal = Some(
                        capped_field_text(
                            &mut field,
                            PRINCIPAL_FIELD_MAX_BYTES,
                            "invalid principal",
                        )
                        .await?,
                    );
                }
                "idempotency_key" => {
                    if idempotency_key.is_some() {
                        return Err(telegram_error(
                            StatusCode::BAD_REQUEST,
                            "duplicate idempotency_key",
                        ));
                    }
                    idempotency_key = Some(
                        capped_field_text(
                            &mut field,
                            IDEMPOTENCY_KEY_FIELD_MAX_BYTES,
                            "invalid idempotency_key",
                        )
                        .await?,
                    );
                }
                "title" => {
                    if title.is_some() {
                        return Err(telegram_error(StatusCode::BAD_REQUEST, "duplicate title"));
                    }
                    title = Some(
                        capped_field_text(&mut field, TITLE_FIELD_MAX_BYTES, "invalid title")
                            .await?,
                    );
                }
                "body" => {
                    if body.is_some() {
                        return Err(telegram_error(StatusCode::BAD_REQUEST, "duplicate body"));
                    }
                    body = Some(
                        capped_field_text(&mut field, BODY_FIELD_MAX_BYTES, "invalid body").await?,
                    );
                }
                "file" => {
                    if file_seen {
                        return Err(telegram_error(StatusCode::BAD_REQUEST, "duplicate file"));
                    }
                    file_seen = true;
                    let filename = field.file_name().map(str::to_owned);
                    let content_type = field.content_type().map(str::to_owned);
                    let mut bytes = Vec::new();
                    while let Some(chunk) = field.chunk().await.map_err(|e| {
                        if e.status() == StatusCode::PAYLOAD_TOO_LARGE {
                            telegram_error(
                                StatusCode::PAYLOAD_TOO_LARGE,
                                "Uploaded file is too large",
                            )
                        } else {
                            telegram_error(StatusCode::BAD_REQUEST, "malformed multipart")
                        }
                    })? {
                        if bytes.len().saturating_add(chunk.len()) > media::MAX_UPLOAD_BYTES {
                            return Err(telegram_error(
                                StatusCode::PAYLOAD_TOO_LARGE,
                                "Uploaded file is too large",
                            ));
                        }
                        bytes.extend_from_slice(&chunk);
                    }
                    if !bytes.is_empty() {
                        file = Some(media::MediaUpload {
                            filename,
                            content_type,
                            bytes,
                        });
                    }
                }
                _ => return Err(telegram_error(StatusCode::BAD_REQUEST, "unexpected field")),
            }
        }
        Ok(ParsedThreadCreate {
            principal: principal
                .ok_or_else(|| telegram_error(StatusCode::BAD_REQUEST, "missing principal"))?,
            idempotency_key: idempotency_key.ok_or_else(|| {
                telegram_error(StatusCode::BAD_REQUEST, "missing idempotency_key")
            })?,
            title: title.unwrap_or_default(),
            body: body.unwrap_or_default(),
            file,
        })
    } else {
        Err(telegram_error(
            StatusCode::BAD_REQUEST,
            "unsupported Content-Type",
        ))
    }
}

async fn parse_reply_create(request: Request) -> Result<ParsedReplyCreate, Response> {
    let headers = request.headers().clone();
    if is_json(&headers) {
        let json: CreateReplyJson = read_json_limited(request).await?;
        Ok(ParsedReplyCreate {
            principal: json.principal,
            idempotency_key: json.idempotency_key,
            body: json.body,
            file: None,
        })
    } else if is_multipart(&headers) {
        let mut multipart = Multipart::from_request(request, &())
            .await
            .map_err(|_| telegram_error(StatusCode::BAD_REQUEST, "malformed multipart"))?;
        let mut principal = None;
        let mut idempotency_key = None;
        let mut body = None;
        let mut file: Option<media::MediaUpload> = None;
        let mut file_seen = false;
        while let Some(mut field) = multipart.next_field().await.map_err(|e| {
            if e.status() == StatusCode::PAYLOAD_TOO_LARGE {
                telegram_error(StatusCode::PAYLOAD_TOO_LARGE, "Uploaded file is too large")
            } else {
                telegram_error(StatusCode::BAD_REQUEST, "malformed multipart")
            }
        })? {
            let name = field
                .name()
                .ok_or_else(|| telegram_error(StatusCode::BAD_REQUEST, "missing field name"))?
                .to_owned();
            match name.as_str() {
                "principal" => {
                    if principal.is_some() {
                        return Err(telegram_error(
                            StatusCode::BAD_REQUEST,
                            "duplicate principal",
                        ));
                    }
                    principal = Some(
                        capped_field_text(
                            &mut field,
                            PRINCIPAL_FIELD_MAX_BYTES,
                            "invalid principal",
                        )
                        .await?,
                    );
                }
                "idempotency_key" => {
                    if idempotency_key.is_some() {
                        return Err(telegram_error(
                            StatusCode::BAD_REQUEST,
                            "duplicate idempotency_key",
                        ));
                    }
                    idempotency_key = Some(
                        capped_field_text(
                            &mut field,
                            IDEMPOTENCY_KEY_FIELD_MAX_BYTES,
                            "invalid idempotency_key",
                        )
                        .await?,
                    );
                }
                "body" => {
                    if body.is_some() {
                        return Err(telegram_error(StatusCode::BAD_REQUEST, "duplicate body"));
                    }
                    body = Some(
                        capped_field_text(&mut field, BODY_FIELD_MAX_BYTES, "invalid body").await?,
                    );
                }
                "file" => {
                    if file_seen {
                        return Err(telegram_error(StatusCode::BAD_REQUEST, "duplicate file"));
                    }
                    file_seen = true;
                    let filename = field.file_name().map(str::to_owned);
                    let content_type = field.content_type().map(str::to_owned);
                    let mut bytes = Vec::new();
                    while let Some(chunk) = field.chunk().await.map_err(|e| {
                        if e.status() == StatusCode::PAYLOAD_TOO_LARGE {
                            telegram_error(
                                StatusCode::PAYLOAD_TOO_LARGE,
                                "Uploaded file is too large",
                            )
                        } else {
                            telegram_error(StatusCode::BAD_REQUEST, "malformed multipart")
                        }
                    })? {
                        if bytes.len().saturating_add(chunk.len()) > media::MAX_UPLOAD_BYTES {
                            return Err(telegram_error(
                                StatusCode::PAYLOAD_TOO_LARGE,
                                "Uploaded file is too large",
                            ));
                        }
                        bytes.extend_from_slice(&chunk);
                    }
                    if !bytes.is_empty() {
                        file = Some(media::MediaUpload {
                            filename,
                            content_type,
                            bytes,
                        });
                    }
                }
                _ => return Err(telegram_error(StatusCode::BAD_REQUEST, "unexpected field")),
            }
        }
        Ok(ParsedReplyCreate {
            principal: principal
                .ok_or_else(|| telegram_error(StatusCode::BAD_REQUEST, "missing principal"))?,
            idempotency_key: idempotency_key.ok_or_else(|| {
                telegram_error(StatusCode::BAD_REQUEST, "missing idempotency_key")
            })?,
            body: body.unwrap_or_default(),
            file,
        })
    } else {
        Err(telegram_error(
            StatusCode::BAD_REQUEST,
            "unsupported Content-Type",
        ))
    }
}

async fn handle_thread_create(
    State(state): State<Arc<HttpDependencies>>,
    headers: HeaderMap,
    Path(slug): Path<String>,
    request: Request,
) -> Response {
    if let Err(resp) = check_telegram_auth(&headers, &state) {
        return resp;
    }
    let parsed = match parse_thread_create(request).await {
        Ok(v) => v,
        Err(resp) => return resp,
    };
    if let Err(resp) = validate_principal(&parsed.principal) {
        return resp;
    }
    if let Err(resp) = validate_idempotency_key(&parsed.idempotency_key) {
        return resp;
    }
    let (title, body) =
        match crate::http::posting::validate_thread_inputs(&parsed.title, &parsed.body) {
            Ok(v) => v,
            Err(e) => {
                let (status, msg) = match e {
                    crate::http::posting::CanonicalError::Validation(s, m) => (s, m),
                    _ => (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "validation error".to_string(),
                    ),
                };
                return telegram_error(status, &msg);
            }
        };
    let origin_key = telegram_origin_key(&parsed.principal);
    let fingerprint = state.abuse_cipher.fingerprint(&origin_key);
    let hash_bytes = thread_request_hash(&fingerprint, &slug, &title, &body, parsed.file.as_ref());
    let key: forum::IdempotencyKey = (
        SERVICE,
        OP_THREAD_CREATE,
        &parsed.idempotency_key,
        &hash_bytes,
    );
    match forum::check_machine_idempotency(&state.pool, key).await {
        Ok(forum::IdempotencyCheck::Replayed(id)) => {
            return telegram_json(
                StatusCode::OK,
                serde_json::json!({ "id": id, "thread_id": id }),
            );
        }
        Ok(forum::IdempotencyCheck::Conflict) => {
            return telegram_error(StatusCode::CONFLICT, "idempotency conflict");
        }
        Ok(forum::IdempotencyCheck::New) => {}
        Err(_) => return telegram_error(StatusCode::INTERNAL_SERVER_ERROR, "database error"),
    }
    match forum::approved_board_exists(&state.pool, &slug).await {
        Ok(true) => {}
        Ok(false) => return telegram_error(StatusCode::NOT_FOUND, "board not found"),
        Err(_) => return telegram_error(StatusCode::INTERNAL_SERVER_ERROR, "database error"),
    }
    if let Err(e) =
        crate::http::posting::ensure_not_banned_for_board(&state, &fingerprint, &slug).await
    {
        return match e {
            crate::http::posting::CanonicalError::Ban { scope, expires_at } => telegram_error(
                StatusCode::FORBIDDEN,
                &format!("Posting is blocked by an active {scope} ban until {expires_at}."),
            ),
            crate::http::posting::CanonicalError::Database => {
                telegram_error(StatusCode::INTERNAL_SERVER_ERROR, "database error")
            }
            _ => telegram_error(StatusCode::INTERNAL_SERVER_ERROR, "ban check failed"),
        };
    }
    if let Err(e) = crate::http::posting::ensure_thread_rate(&state, &fingerprint) {
        return match e {
            crate::http::posting::CanonicalError::RateLimited(s, m) => {
                match await_concurrent_idempotent_commit(&state, key).await {
                    ConcurrentCommit::Committed(id) => telegram_json(
                        StatusCode::OK,
                        serde_json::json!({ "id": id, "thread_id": id }),
                    ),
                    ConcurrentCommit::Mismatched => {
                        telegram_error(StatusCode::CONFLICT, "idempotency conflict")
                    }
                    ConcurrentCommit::Unresolved => telegram_error(s, &m),
                }
            }
            _ => telegram_error(StatusCode::INTERNAL_SERVER_ERROR, "rate check failed"),
        };
    }
    let origin = match crate::http::posting::protect_origin(&state, &origin_key) {
        Ok(o) => o,
        Err(_) => {
            return telegram_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Could not protect operational data",
            );
        }
    };
    let report_details =
        match crate::http::posting::evaluate_miya(&state, &format!("Title: {title}\n\n{body}"))
            .await
        {
            Ok(v) => v,
            Err(crate::http::posting::CanonicalError::MiyaBlocked(details)) => {
                return telegram_error(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    &format!("Your post was blocked by moderation: {details}"),
                );
            }
            Err(_) => return telegram_error(StatusCode::INTERNAL_SERVER_ERROR, "miya error"),
        };
    let processed = match crate::http::posting::canonical_process_media(&state, parsed.file).await {
        Ok(v) => v,
        Err(crate::http::posting::CanonicalError::Media(s, m)) => return telegram_error(s, &m),
        Err(_) => return telegram_error(StatusCode::INTERNAL_SERVER_ERROR, "media error"),
    };
    match forum::create_thread_idempotent(
        &state.pool,
        &slug,
        &title,
        &body,
        &origin,
        processed.as_ref().map(|p| &p.media),
        key,
    )
    .await
    {
        Ok(forum::IdempotentMutation::Created(id)) => {
            if let Some(details) = report_details.as_deref() {
                match forum::report_thread(&state.pool, id, "other", Some(details)).await {
                    Ok(true) => {
                        state
                            .notify_report("thread", id, id, "other", Some(details))
                            .await;
                    }
                    Ok(false) => {}
                    Err(error) => eprintln!("Miya report insertion failed: {error}"),
                }
            }
            telegram_json(
                StatusCode::CREATED,
                serde_json::json!({ "id": id, "thread_id": id }),
            )
        }
        Ok(forum::IdempotentMutation::Replayed(id)) => {
            if let Some(p) = processed.as_ref() {
                crate::http::posting::cleanup_media(&state, &p.image_id).await;
            }
            telegram_json(
                StatusCode::OK,
                serde_json::json!({ "id": id, "thread_id": id }),
            )
        }
        Ok(forum::IdempotentMutation::Conflict) => {
            if let Some(p) = processed.as_ref() {
                crate::http::posting::cleanup_media(&state, &p.image_id).await;
            }
            telegram_error(StatusCode::CONFLICT, "idempotency conflict")
        }
        Err(_) => {
            if let Some(p) = processed.as_ref() {
                crate::http::posting::cleanup_media(&state, &p.image_id).await;
            }
            telegram_error(StatusCode::INTERNAL_SERVER_ERROR, "database error")
        }
    }
}

async fn handle_reply_create(
    State(state): State<Arc<HttpDependencies>>,
    headers: HeaderMap,
    Path(thread_id_raw): Path<String>,
    request: Request,
) -> Response {
    if let Err(resp) = check_telegram_auth(&headers, &state) {
        return resp;
    }
    let thread_id: u64 = match thread_id_raw.parse() {
        Ok(v) => v,
        Err(_) => return telegram_error(StatusCode::NOT_FOUND, "thread not found"),
    };
    let parsed = match parse_reply_create(request).await {
        Ok(v) => v,
        Err(resp) => return resp,
    };
    if let Err(resp) = validate_principal(&parsed.principal) {
        return resp;
    }
    if let Err(resp) = validate_idempotency_key(&parsed.idempotency_key) {
        return resp;
    }
    let body = match crate::http::posting::validate_reply_body(&parsed.body) {
        Ok(v) => v,
        Err(e) => {
            let (s, m) = match e {
                crate::http::posting::CanonicalError::Validation(s, m) => (s, m),
                _ => (StatusCode::BAD_REQUEST, "validation error".to_string()),
            };
            return telegram_error(s, &m);
        }
    };
    let origin_key = telegram_origin_key(&parsed.principal);
    let fingerprint = state.abuse_cipher.fingerprint(&origin_key);
    let hash_bytes = reply_request_hash(&fingerprint, thread_id, &body, parsed.file.as_ref());
    let key: forum::IdempotencyKey = (
        SERVICE,
        OP_REPLY_CREATE,
        &parsed.idempotency_key,
        &hash_bytes,
    );
    match forum::check_machine_idempotency(&state.pool, key).await {
        Ok(forum::IdempotencyCheck::Replayed(id)) => {
            return telegram_json(
                StatusCode::OK,
                serde_json::json!({ "id": id, "reply_id": id, "thread_id": thread_id }),
            );
        }
        Ok(forum::IdempotencyCheck::Conflict) => {
            return telegram_error(StatusCode::CONFLICT, "idempotency conflict");
        }
        Ok(forum::IdempotencyCheck::New) => {}
        Err(_) => return telegram_error(StatusCode::INTERNAL_SERVER_ERROR, "database error"),
    }
    match forum::classify_reply_target(&state.pool, thread_id).await {
        Ok(forum::ReplyTargetState::Visible) => {}
        Ok(forum::ReplyTargetState::Locked) => {
            return telegram_error(StatusCode::CONFLICT, "This thread is locked");
        }
        Ok(forum::ReplyTargetState::Archived) => {
            return telegram_error(
                StatusCode::CONFLICT,
                "This thread is archived and read-only",
            );
        }
        Ok(forum::ReplyTargetState::Missing) => {
            return telegram_error(StatusCode::NOT_FOUND, "thread not found");
        }
        Err(_) => return telegram_error(StatusCode::INTERNAL_SERVER_ERROR, "database error"),
    }
    if let Err(e) =
        crate::http::posting::ensure_not_banned_for_thread(&state, &fingerprint, thread_id).await
    {
        return match e {
            crate::http::posting::CanonicalError::Ban { scope, expires_at } => telegram_error(
                StatusCode::FORBIDDEN,
                &format!("Posting is blocked by an active {scope} ban until {expires_at}."),
            ),
            _ => telegram_error(StatusCode::INTERNAL_SERVER_ERROR, "database error"),
        };
    }
    if let Err(e) = crate::http::posting::ensure_reply_rate(&state, &fingerprint) {
        return match e {
            crate::http::posting::CanonicalError::RateLimited(s, m) => {
                match await_concurrent_idempotent_commit(&state, key).await {
                    ConcurrentCommit::Committed(id) => telegram_json(
                        StatusCode::OK,
                        serde_json::json!({ "id": id, "reply_id": id, "thread_id": thread_id }),
                    ),
                    ConcurrentCommit::Mismatched => {
                        telegram_error(StatusCode::CONFLICT, "idempotency conflict")
                    }
                    ConcurrentCommit::Unresolved => telegram_error(s, &m),
                }
            }
            _ => telegram_error(StatusCode::INTERNAL_SERVER_ERROR, "rate"),
        };
    }
    let origin = match crate::http::posting::protect_origin(&state, &origin_key) {
        Ok(o) => o,
        Err(_) => {
            return telegram_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Could not protect operational data",
            );
        }
    };
    let report_details = match crate::http::posting::evaluate_miya(&state, &body).await {
        Ok(v) => v,
        Err(crate::http::posting::CanonicalError::MiyaBlocked(d)) => {
            return telegram_error(
                StatusCode::UNPROCESSABLE_ENTITY,
                &format!("Your post was blocked by moderation: {d}"),
            );
        }
        Err(_) => return telegram_error(StatusCode::INTERNAL_SERVER_ERROR, "miya"),
    };
    let processed = match crate::http::posting::canonical_process_media(&state, parsed.file).await {
        Ok(v) => v,
        Err(crate::http::posting::CanonicalError::Media(s, m)) => return telegram_error(s, &m),
        Err(_) => return telegram_error(StatusCode::INTERNAL_SERVER_ERROR, "media"),
    };
    match forum::create_reply_idempotent(
        &state.pool,
        thread_id,
        &body,
        &origin,
        processed.as_ref().map(|p| &p.media),
        key,
    )
    .await
    {
        Ok(forum::IdempotentMutation::Created(id)) => {
            if let Some(details) = report_details.as_deref() {
                match forum::report_reply(&state.pool, id, "other", Some(details)).await {
                    Ok(Some(notified_thread_id)) => {
                        state
                            .notify_report("reply", id, notified_thread_id, "other", Some(details))
                            .await;
                    }
                    Ok(None) => {}
                    Err(error) => eprintln!("Miya report insertion failed: {error}"),
                }
            }
            telegram_json(
                StatusCode::CREATED,
                serde_json::json!({ "id": id, "reply_id": id, "thread_id": thread_id }),
            )
        }
        Ok(forum::IdempotentMutation::Replayed(id)) => {
            if let Some(p) = processed.as_ref() {
                crate::http::posting::cleanup_media(&state, &p.image_id).await;
            }
            telegram_json(
                StatusCode::OK,
                serde_json::json!({ "id": id, "reply_id": id, "thread_id": thread_id }),
            )
        }
        Ok(forum::IdempotentMutation::Conflict) => {
            if let Some(p) = processed.as_ref() {
                crate::http::posting::cleanup_media(&state, &p.image_id).await;
            }
            telegram_error(StatusCode::CONFLICT, "idempotency conflict")
        }
        Err(_) => {
            if let Some(p) = processed.as_ref() {
                crate::http::posting::cleanup_media(&state, &p.image_id).await;
            }
            telegram_error(StatusCode::INTERNAL_SERVER_ERROR, "database error")
        }
    }
}

async fn handle_report_thread(
    State(state): State<Arc<HttpDependencies>>,
    headers: HeaderMap,
    Path(thread_id_raw): Path<String>,
    request: Request,
) -> Response {
    if let Err(resp) = check_telegram_auth(&headers, &state) {
        return resp;
    }
    let thread_id: u64 = match thread_id_raw.parse() {
        Ok(v) => v,
        Err(_) => return telegram_error(StatusCode::NOT_FOUND, "thread not found"),
    };
    let json: ReportJson = match read_json_limited(request).await {
        Ok(v) => v,
        Err(e) => return e,
    };
    if let Err(resp) = validate_principal(&json.principal) {
        return resp;
    }
    if let Err(resp) = validate_idempotency_key(&json.idempotency_key) {
        return resp;
    }
    let (reason, details) =
        match crate::http::posting::validate_report_inputs(&json.reason, json.details.as_deref()) {
            Ok(v) => v,
            Err(e) => {
                let (s, m) = match e {
                    crate::http::posting::CanonicalError::Validation(s, m) => (s, m),
                    _ => (StatusCode::BAD_REQUEST, "validation".to_string()),
                };
                return telegram_error(s, &m);
            }
        };
    let origin_key = telegram_origin_key(&json.principal);
    let fingerprint = state.abuse_cipher.fingerprint(&origin_key);
    let hash_bytes = report_thread_hash(&fingerprint, thread_id, &reason, details.as_deref());
    let key: forum::IdempotencyKey = (
        SERVICE,
        OP_THREAD_REPORT,
        &json.idempotency_key,
        &hash_bytes,
    );
    match forum::check_machine_idempotency(&state.pool, key).await {
        Ok(forum::IdempotencyCheck::Replayed(report_id)) => {
            return telegram_json(
                StatusCode::OK,
                serde_json::json!({ "id": report_id, "report_id": report_id, "thread_id": thread_id }),
            );
        }
        Ok(forum::IdempotencyCheck::Conflict) => {
            return telegram_error(StatusCode::CONFLICT, "idempotency conflict");
        }
        Ok(forum::IdempotencyCheck::New) => {}
        Err(_) => return telegram_error(StatusCode::INTERNAL_SERVER_ERROR, "database error"),
    }
    match forum::thread_report_target_exists(&state.pool, thread_id).await {
        Ok(true) => {}
        Ok(false) => return telegram_error(StatusCode::NOT_FOUND, "thread not found"),
        Err(_) => return telegram_error(StatusCode::INTERNAL_SERVER_ERROR, "database error"),
    }

    if let Err(e) = crate::http::posting::ensure_report_rate(&state, &fingerprint) {
        return match e {
            crate::http::posting::CanonicalError::RateLimited(s, m) => {
                match await_concurrent_idempotent_commit(&state, key).await {
                    ConcurrentCommit::Committed(report_id) => telegram_json(
                        StatusCode::OK,
                        serde_json::json!({ "id": report_id, "report_id": report_id, "thread_id": thread_id }),
                    ),
                    ConcurrentCommit::Mismatched => {
                        telegram_error(StatusCode::CONFLICT, "idempotency conflict")
                    }
                    ConcurrentCommit::Unresolved => telegram_error(s, &m),
                }
            }
            _ => telegram_error(StatusCode::INTERNAL_SERVER_ERROR, "rate"),
        };
    }
    match forum::report_thread_idempotent(&state.pool, thread_id, &reason, details.as_deref(), key)
        .await
    {
        Ok(forum::IdempotentMutation::Created((report_id, tid))) => {
            state
                .notify_report("thread", thread_id, tid, &reason, details.as_deref())
                .await;
            telegram_json(
                StatusCode::CREATED,
                serde_json::json!({ "id": report_id, "report_id": report_id, "thread_id": tid }),
            )
        }
        Ok(forum::IdempotentMutation::Replayed((report_id, tid))) => telegram_json(
            StatusCode::OK,
            serde_json::json!({ "id": report_id, "report_id": report_id, "thread_id": tid }),
        ),
        Ok(forum::IdempotentMutation::Conflict) => {
            telegram_error(StatusCode::CONFLICT, "idempotency conflict")
        }
        Err(_) => telegram_error(StatusCode::INTERNAL_SERVER_ERROR, "database error"),
    }
}

async fn handle_report_reply(
    State(state): State<Arc<HttpDependencies>>,
    headers: HeaderMap,
    Path(reply_id_raw): Path<String>,
    request: Request,
) -> Response {
    if let Err(resp) = check_telegram_auth(&headers, &state) {
        return resp;
    }
    let reply_id: u64 = match reply_id_raw.parse() {
        Ok(v) => v,
        Err(_) => return telegram_error(StatusCode::NOT_FOUND, "reply not found"),
    };
    let json: ReportJson = match read_json_limited(request).await {
        Ok(v) => v,
        Err(e) => return e,
    };
    if let Err(resp) = validate_principal(&json.principal) {
        return resp;
    }
    if let Err(resp) = validate_idempotency_key(&json.idempotency_key) {
        return resp;
    }
    let (reason, details) =
        match crate::http::posting::validate_report_inputs(&json.reason, json.details.as_deref()) {
            Ok(v) => v,
            Err(e) => {
                let (s, m) = match e {
                    crate::http::posting::CanonicalError::Validation(s, m) => (s, m),
                    _ => (StatusCode::BAD_REQUEST, "validation".to_string()),
                };
                return telegram_error(s, &m);
            }
        };
    let origin_key = telegram_origin_key(&json.principal);
    let fingerprint = state.abuse_cipher.fingerprint(&origin_key);
    let hash_bytes = report_reply_hash(&fingerprint, reply_id, &reason, details.as_deref());
    let key: forum::IdempotencyKey = (SERVICE, OP_REPLY_REPORT, &json.idempotency_key, &hash_bytes);
    match forum::check_machine_idempotency(&state.pool, key).await {
        Ok(forum::IdempotencyCheck::Replayed(report_id)) => {
            let thread_id = match forum::reply_owner_thread_id(&state.pool, reply_id).await {
                Ok(Some(thread_id)) => thread_id,
                Ok(None) => return telegram_error(StatusCode::NOT_FOUND, "reply not found"),
                Err(_) => {
                    return telegram_error(StatusCode::INTERNAL_SERVER_ERROR, "database error");
                }
            };
            return telegram_json(
                StatusCode::OK,
                serde_json::json!({ "id": report_id, "report_id": report_id, "thread_id": thread_id, "reply_id": reply_id }),
            );
        }
        Ok(forum::IdempotencyCheck::Conflict) => {
            return telegram_error(StatusCode::CONFLICT, "idempotency conflict");
        }
        Ok(forum::IdempotencyCheck::New) => {}
        Err(_) => return telegram_error(StatusCode::INTERNAL_SERVER_ERROR, "database error"),
    }
    match forum::reply_report_target_exists(&state.pool, reply_id).await {
        Ok(true) => {}
        Ok(false) => return telegram_error(StatusCode::NOT_FOUND, "reply not found"),
        Err(_) => return telegram_error(StatusCode::INTERNAL_SERVER_ERROR, "database error"),
    }

    if let Err(e) = crate::http::posting::ensure_report_rate(&state, &fingerprint) {
        return match e {
            crate::http::posting::CanonicalError::RateLimited(s, m) => {
                match await_concurrent_idempotent_commit(&state, key).await {
                    ConcurrentCommit::Committed(report_id) => {
                        match forum::reply_owner_thread_id(&state.pool, reply_id).await {
                            Ok(Some(thread_id)) => telegram_json(
                                StatusCode::OK,
                                serde_json::json!({ "id": report_id, "report_id": report_id, "thread_id": thread_id, "reply_id": reply_id }),
                            ),
                            Ok(None) => telegram_error(StatusCode::NOT_FOUND, "reply not found"),
                            Err(_) => {
                                telegram_error(StatusCode::INTERNAL_SERVER_ERROR, "database error")
                            }
                        }
                    }
                    ConcurrentCommit::Mismatched => {
                        telegram_error(StatusCode::CONFLICT, "idempotency conflict")
                    }
                    ConcurrentCommit::Unresolved => telegram_error(s, &m),
                }
            }
            _ => telegram_error(StatusCode::INTERNAL_SERVER_ERROR, "rate"),
        };
    }
    match forum::report_reply_idempotent(&state.pool, reply_id, &reason, details.as_deref(), key)
        .await
    {
        Ok(forum::IdempotentMutation::Created((report_id, thread_id))) => {
            state
                .notify_report("reply", reply_id, thread_id, &reason, details.as_deref())
                .await;
            telegram_json(
                StatusCode::CREATED,
                serde_json::json!({ "id": report_id, "report_id": report_id, "thread_id": thread_id, "reply_id": reply_id }),
            )
        }
        Ok(forum::IdempotentMutation::Replayed((report_id, thread_id))) => telegram_json(
            StatusCode::OK,
            serde_json::json!({ "id": report_id, "report_id": report_id, "thread_id": thread_id, "reply_id": reply_id }),
        ),
        Ok(forum::IdempotentMutation::Conflict) => {
            telegram_error(StatusCode::CONFLICT, "idempotency conflict")
        }
        Err(_) => telegram_error(StatusCode::INTERNAL_SERVER_ERROR, "database error"),
    }
}

#[derive(Deserialize)]
struct LeaseRequest {
    limit: Option<i64>,
    lease_seconds: Option<i64>,
}

async fn handle_lease(
    State(state): State<Arc<HttpDependencies>>,
    headers: HeaderMap,
    request: Request,
) -> Response {
    if let Err(resp) = check_telegram_auth(&headers, &state) {
        return resp;
    }
    let bytes = match to_bytes(request.into_body(), JSON_LIMIT).await {
        Ok(b) => b,
        Err(_) => return telegram_error(StatusCode::PAYLOAD_TOO_LARGE, "JSON body too large"),
    };
    let req: LeaseRequest = if bytes.is_empty() {
        LeaseRequest {
            limit: None,
            lease_seconds: None,
        }
    } else {
        match serde_json::from_slice(&bytes) {
            Ok(v) => v,
            Err(e) => {
                return telegram_error(StatusCode::BAD_REQUEST, &format!("invalid JSON: {e}"));
            }
        }
    };
    let limit = req.limit.unwrap_or(100);
    let lease_seconds = req.lease_seconds.unwrap_or(60);
    if limit < 1 {
        return telegram_error(StatusCode::BAD_REQUEST, "limit must be positive");
    }
    match forum::lease_projection_outbox(&state.pool, limit, lease_seconds).await {
        Ok(events) => {
            let out: Vec<serde_json::Value> = events
                .into_iter()
                .map(|e| {
                    serde_json::json!({
                        "id": e.id,
                        "kind": e.kind,
                        "thread_id": e.thread_id,
                        "reply_id": e.reply_id,
                        "report_id": e.report_id,
                        "created_at": e.created_at,
                        "lease_token": e.lease_token,
                        "lease_expires_at": e.lease_expires_at
                    })
                })
                .collect();
            telegram_json(StatusCode::OK, serde_json::json!({ "events": out }))
        }
        Err(_) => telegram_error(StatusCode::INTERNAL_SERVER_ERROR, "database error"),
    }
}

#[derive(Deserialize)]
struct AckRequest {
    event_id: Option<u64>,
    id: Option<u64>,
    lease_token: String,
}

async fn handle_ack(
    State(state): State<Arc<HttpDependencies>>,
    headers: HeaderMap,
    request: Request,
) -> Response {
    if let Err(resp) = check_telegram_auth(&headers, &state) {
        return resp;
    }
    let json: AckRequest = match read_json_limited(request).await {
        Ok(v) => v,
        Err(e) => return e,
    };
    let event_id = json.event_id.or(json.id).unwrap_or(0);
    if event_id == 0 {
        return telegram_error(StatusCode::BAD_REQUEST, "missing event_id");
    }
    if json.lease_token.is_empty() {
        return telegram_error(StatusCode::BAD_REQUEST, "missing lease_token");
    }
    match forum::acknowledge_projection_outbox(&state.pool, event_id, &json.lease_token).await {
        Ok(forum::OutboxAck::Acknowledged) => telegram_json(
            StatusCode::OK,
            serde_json::json!({ "status": "acknowledged", "event_id": event_id }),
        ),
        Ok(forum::OutboxAck::NotFound) => telegram_error(StatusCode::NOT_FOUND, "event not found"),
        Ok(forum::OutboxAck::LeaseMismatch) => {
            telegram_error(StatusCode::CONFLICT, "lease token mismatch")
        }
        Err(_) => telegram_error(StatusCode::INTERNAL_SERVER_ERROR, "database error"),
    }
}

async fn add_telegram_no_store_headers(mut response: Response) -> Response {
    let headers = response.headers_mut();
    headers.insert(CACHE_CONTROL, HeaderValue::from_static("no-store, private"));
    headers.insert(PRAGMA, HeaderValue::from_static("no-cache"));
    response
}

pub(crate) fn telegram_router(state: Arc<HttpDependencies>) -> Router {
    Router::new()
        .route(
            "/internal/telegram/boards/{slug}/threads",
            get(backfill_handler).post(handle_thread_create),
        )
        .route("/internal/telegram/threads/{id}", get(snapshot_handler))
        .route(
            "/internal/telegram/threads/{id}/replies",
            post(handle_reply_create),
        )
        .route(
            "/internal/telegram/threads/{id}/reports",
            post(handle_report_thread),
        )
        .route(
            "/internal/telegram/replies/{id}/reports",
            post(handle_report_reply),
        )
        .route("/internal/telegram/outbox/lease", post(handle_lease))
        .route("/internal/telegram/outbox/ack", post(handle_ack))
        .with_state(state)
        .layer(axum::extract::DefaultBodyLimit::max(
            media::MAX_UPLOAD_BYTES + 64 * 1024,
        ))
        .layer(axum::middleware::map_response(
            add_telegram_no_store_headers,
        ))
}
