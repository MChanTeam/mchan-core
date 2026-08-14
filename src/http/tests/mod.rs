use super::*;
use crate::captcha::{CaptchaVerifier, VerificationUnavailable};
use crate::media::{MediaError, MediaProcessor, MediaUpload, ProcessedMedia};
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{HeaderName, HeaderValue, Method, Request, Response, header::CONTENT_TYPE},
};
use std::{
    collections::{HashSet, VecDeque},
    future::Future,
    path::PathBuf,
    pin::Pin,
    sync::{Arc, Mutex},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};
use tower::ServiceExt;

async fn scripted_miya(status: u16, body: &str) -> (Arc<miya::Miya>, tokio::task::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind scripted Miya listener");
    let address = listener.local_addr().expect("scripted Miya address");
    let body = body.to_owned();
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener
            .accept()
            .await
            .expect("accept scripted Miya request");
        let mut request = Vec::new();
        loop {
            let mut chunk = [0; 4096];
            let read = stream
                .read(&mut chunk)
                .await
                .expect("read scripted Miya request");
            if read == 0 {
                break;
            }
            request.extend_from_slice(&chunk[..read]);
            if let Some(header_end) = request.windows(4).position(|window| window == b"\r\n\r\n") {
                let headers = String::from_utf8_lossy(&request[..header_end]);
                let content_length = headers
                    .lines()
                    .find_map(|line| {
                        let (name, value) = line.split_once(':')?;
                        name.eq_ignore_ascii_case("content-length").then_some(value)
                    })
                    .and_then(|length| length.trim().parse().ok())
                    .unwrap_or(0);
                if request.len() >= header_end + 4 + content_length {
                    break;
                }
            }
        }
        let response = format!(
            "HTTP/1.1 {status} Test\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        stream
            .write_all(response.as_bytes())
            .await
            .expect("write scripted Miya response");
        stream
            .shutdown()
            .await
            .expect("close scripted Miya response");
    });
    let miya = miya::Miya::new(format!("http://{address}")).expect("scripted Miya URL is valid");
    (Arc::new(miya), server)
}
async fn scripted_webhook(
    status: u16,
    expected_requests: usize,
) -> (String, tokio::task::JoinHandle<Vec<serde_json::Value>>) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind scripted webhook listener");
    let address = listener.local_addr().expect("scripted webhook address");
    let server = tokio::spawn(async move {
        let mut requests = Vec::with_capacity(expected_requests);
        for _ in 0..expected_requests {
            let (mut stream, _) = listener
                .accept()
                .await
                .expect("accept scripted webhook request");
            let mut request = Vec::new();
            loop {
                let mut chunk = [0; 4096];
                let read = stream
                    .read(&mut chunk)
                    .await
                    .expect("read scripted webhook request");
                if read == 0 {
                    break;
                }
                request.extend_from_slice(&chunk[..read]);
                let Some(header_end) = request.windows(4).position(|window| window == b"\r\n\r\n")
                else {
                    continue;
                };
                let headers = String::from_utf8_lossy(&request[..header_end]);
                let content_length = headers
                    .lines()
                    .find_map(|line| {
                        let (name, value) = line.split_once(':')?;
                        name.eq_ignore_ascii_case("content-length").then_some(value)
                    })
                    .and_then(|length| length.trim().parse::<usize>().ok())
                    .unwrap_or(0);
                if request.len() >= header_end + 4 + content_length {
                    let body_start = header_end + 4;
                    requests.push(
                        serde_json::from_slice(&request[body_start..body_start + content_length])
                            .expect("scripted webhook JSON"),
                    );
                    break;
                }
            }
            let response =
                format!("HTTP/1.1 {status} Test\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
            stream
                .write_all(response.as_bytes())
                .await
                .expect("write scripted webhook response");
            stream
                .shutdown()
                .await
                .expect("close scripted webhook response");
        }
        requests
    });
    (format!("http://{address}"), server)
}

async fn unreachable_webhook_url() -> String {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind unreachable webhook address");
    let address = listener.local_addr().expect("unreachable webhook address");
    drop(listener);
    format!("http://{address}")
}

mod moderation;
mod operations;
mod posting;
mod public;

const TEST_ABUSE_KEY: &str = "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f";
const TEST_CLIENT_IP: &str = "198.51.100.42";
const MODERATOR_EMAIL: &str = "moderator@example.com";

#[derive(Clone, Copy)]
enum CaptchaOutcome {
    Allow,
    Reject,
    Unavailable,
}

struct ScriptedCaptcha {
    outcomes: Mutex<VecDeque<CaptchaOutcome>>,
}

impl ScriptedCaptcha {
    fn new(outcomes: impl IntoIterator<Item = CaptchaOutcome>) -> Self {
        Self {
            outcomes: Mutex::new(outcomes.into_iter().collect()),
        }
    }
}

impl CaptchaVerifier for ScriptedCaptcha {
    fn site_key(&self) -> &str {
        "test-site-key"
    }

    fn verify<'a>(
        &'a self,
        token: &'a str,
        remote_ip: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<bool, VerificationUnavailable>> + Send + 'a>> {
        assert_eq!(token, "scripted-token");
        assert_eq!(remote_ip, TEST_CLIENT_IP);

        let outcome = self
            .outcomes
            .lock()
            .expect("scripted CAPTCHA mutex poisoned")
            .pop_front()
            .expect("scripted CAPTCHA outcome missing");
        Box::pin(async move {
            match outcome {
                CaptchaOutcome::Allow => Ok(true),
                CaptchaOutcome::Reject => Ok(false),
                CaptchaOutcome::Unavailable => Err(VerificationUnavailable),
            }
        })
    }
}

fn test_dependencies_with_miya(
    pool: SqlitePool,
    admin_emails: HashSet<String>,
    miya: Option<Arc<miya::Miya>>,
) -> HttpDependencies {
    HttpDependencies::new(
        pool,
        admin_emails,
        abuse::AbuseCipher::from_hex(TEST_ABUSE_KEY).expect("valid test abuse key"),
        None,
        None,
        PathBuf::from("/data"),
        miya,
        None,
        None,
    )
}
enum ScriptedMediaOutcome {
    Success {
        image_id: String,
        thumbnail_path: String,
        display_path: String,
        width: u64,
        height: u64,
    },
    Error(MediaError),
}

impl ScriptedMediaOutcome {
    fn success(
        image_id: &str,
        thumbnail_path: &str,
        display_path: &str,
        width: u64,
        height: u64,
    ) -> Self {
        Self::Success {
            image_id: image_id.to_owned(),
            thumbnail_path: thumbnail_path.to_owned(),
            display_path: display_path.to_owned(),
            width,
            height,
        }
    }
}

struct ScriptedMedia {
    outcomes: Mutex<VecDeque<ScriptedMediaOutcome>>,
    uploads: Mutex<Vec<MediaUpload>>,
    deleted_image_ids: Mutex<Vec<String>>,
}

impl ScriptedMedia {
    fn new(outcomes: impl IntoIterator<Item = ScriptedMediaOutcome>) -> Arc<Self> {
        Arc::new(Self {
            outcomes: Mutex::new(outcomes.into_iter().collect()),
            uploads: Mutex::new(Vec::new()),
            deleted_image_ids: Mutex::new(Vec::new()),
        })
    }

    fn uploads(&self) -> Vec<MediaUpload> {
        self.uploads
            .lock()
            .expect("scripted media uploads mutex poisoned")
            .clone()
    }

    fn deleted_image_ids(&self) -> Vec<String> {
        self.deleted_image_ids
            .lock()
            .expect("scripted media deletes mutex poisoned")
            .clone()
    }
}

impl MediaProcessor for ScriptedMedia {
    fn process<'a>(
        &'a self,
        upload: MediaUpload,
    ) -> Pin<Box<dyn Future<Output = Result<ProcessedMedia, MediaError>> + Send + 'a>> {
        self.uploads
            .lock()
            .expect("scripted media uploads mutex poisoned")
            .push(upload);
        let outcome = self
            .outcomes
            .lock()
            .expect("scripted media outcomes mutex poisoned")
            .pop_front()
            .expect("scripted media outcome missing");
        Box::pin(async move {
            match outcome {
                ScriptedMediaOutcome::Success {
                    image_id,
                    thumbnail_path,
                    display_path,
                    width,
                    height,
                } => Ok(ProcessedMedia {
                    image_id,
                    media: crate::forum::Media {
                        thumbnail_path,
                        display_path,
                        mime_type: "image/webp".to_owned(),
                        width,
                        height,
                    },
                }),
                ScriptedMediaOutcome::Error(error) => Err(error),
            }
        })
    }

    fn delete<'a>(
        &'a self,
        image_id: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<(), MediaError>> + Send + 'a>> {
        self.deleted_image_ids
            .lock()
            .expect("scripted media deletes mutex poisoned")
            .push(image_id.to_owned());
        Box::pin(async { Ok(()) })
    }
}

fn media_router(
    pool: SqlitePool,
    outcomes: impl IntoIterator<Item = ScriptedMediaOutcome>,
) -> (Router, Arc<ScriptedMedia>) {
    let media = ScriptedMedia::new(outcomes);
    let app = router(test_dependencies(
        pool,
        HashSet::new(),
        None,
        Some(media.clone() as Arc<dyn MediaProcessor>),
    ));
    (app, media)
}

fn test_router(pool: SqlitePool) -> Router {
    router(test_dependencies(pool, HashSet::new(), None, None))
}

fn report_webhook_router(pool: SqlitePool, webhook_url: Option<String>) -> Router {
    router(test_dependencies(pool, HashSet::new(), None, None).with_report_webhook_url(webhook_url))
}

fn moderator_router(pool: SqlitePool) -> Router {
    router(test_dependencies(
        pool,
        HashSet::from([String::from(MODERATOR_EMAIL)]),
        None,
        None,
    ))
}
async fn board_moderator_router(pool: SqlitePool, board_slug: &str, email: &str) -> Router {
    sqlx::query("INSERT INTO board_moderators (board_slug, email) VALUES (?, lower(?))")
        .bind(board_slug)
        .bind(email)
        .execute(&pool)
        .await
        .expect("board moderator fixture inserts");
    router(test_dependencies(pool, HashSet::new(), None, None))
}

fn captcha_router(pool: SqlitePool, outcomes: impl IntoIterator<Item = CaptchaOutcome>) -> Router {
    router(test_dependencies(
        pool,
        HashSet::new(),
        Some(Arc::new(ScriptedCaptcha::new(outcomes))),
        None,
    ))
}

fn miya_router(pool: SqlitePool, miya: Arc<miya::Miya>) -> Router {
    router(HttpDependencies::new(
        pool,
        HashSet::new(),
        abuse::AbuseCipher::from_hex(TEST_ABUSE_KEY).expect("valid test abuse key"),
        None,
        None,
        PathBuf::from("/data"),
        Some(miya),
        None,
        None,
    ))
}
fn test_dependencies(
    pool: SqlitePool,
    admin_emails: HashSet<String>,
    captcha: Option<Arc<dyn CaptchaVerifier>>,
    media_processor: Option<Arc<dyn MediaProcessor>>,
) -> HttpDependencies {
    test_dependencies_with_media_storage_root(
        pool,
        admin_emails,
        captcha,
        media_processor,
        PathBuf::from("/data"),
    )
}

fn test_dependencies_with_media_storage_root(
    pool: SqlitePool,
    admin_emails: HashSet<String>,
    captcha: Option<Arc<dyn CaptchaVerifier>>,
    media_processor: Option<Arc<dyn MediaProcessor>>,
    media_storage_root: PathBuf,
) -> HttpDependencies {
    HttpDependencies::new(
        pool,
        admin_emails,
        abuse::AbuseCipher::from_hex(TEST_ABUSE_KEY).expect("valid test abuse key"),
        captcha,
        media_processor,
        media_storage_root,
        None,
        None,
        None,
    )
}
fn test_dependencies_with_discord_token(
    pool: SqlitePool,
    discord_token: Option<&str>,
) -> HttpDependencies {
    HttpDependencies::new(
        pool,
        HashSet::new(),
        abuse::AbuseCipher::from_hex(TEST_ABUSE_KEY).expect("valid test abuse key"),
        None,
        None,
        PathBuf::from("/data"),
        None,
        discord_token.map(str::to_owned),
        None,
    )
}
fn test_dependencies_with_ops_token(pool: SqlitePool, ops_token: Option<&str>) -> HttpDependencies {
    HttpDependencies::new(
        pool,
        HashSet::new(),
        abuse::AbuseCipher::from_hex(TEST_ABUSE_KEY).expect("valid test abuse key"),
        None,
        None,
        PathBuf::from("/data"),
        None,
        None,
        ops_token.map(str::to_owned),
    )
}

fn get_request(uri: &str) -> Request<Body> {
    Request::builder()
        .uri(uri)
        .body(Body::empty())
        .expect("valid GET request")
}

fn post_form(uri: &str, body: &str) -> Request<Body> {
    Request::builder()
        .method(Method::POST)
        .uri(uri)
        .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(body.to_owned()))
        .expect("valid form request")
}

fn post_multipart(
    uri: &str,
    fields: &[(&str, &str)],
    file: Option<(&str, &str, &[u8])>,
) -> Request<Body> {
    const BOUNDARY: &str = "mchan-test-boundary";
    let mut body = Vec::new();
    for (name, value) in fields {
        body.extend_from_slice(
            format!(
                "--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"{name}\"\r\n\r\n{value}\r\n"
            )
            .as_bytes(),
        );
    }
    if let Some((filename, content_type, bytes)) = file {
        body.extend_from_slice(format!(
            "--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{filename}\"\r\nContent-Type: {content_type}\r\n\r\n"
        ).as_bytes());
        body.extend_from_slice(bytes);
        body.extend_from_slice(b"\r\n");
    }
    body.extend_from_slice(format!("--{BOUNDARY}--\r\n").as_bytes());

    Request::builder()
        .method(Method::POST)
        .uri(uri)
        .header(
            CONTENT_TYPE,
            format!("multipart/form-data; boundary={BOUNDARY}"),
        )
        .body(Body::from(body))
        .expect("valid multipart request")
}

fn with_header(
    mut request: Request<Body>,
    name: &'static str,
    value: &'static str,
) -> Request<Body> {
    request.headers_mut().insert(
        HeaderName::from_static(name),
        HeaderValue::from_static(value),
    );
    request
}

async fn send(app: &Router, request: Request<Body>) -> Response<Body> {
    app.clone()
        .oneshot(request)
        .await
        .expect("Router request succeeds")
}

async fn response_text(response: Response<Body>) -> String {
    let bytes = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("response body is readable");
    String::from_utf8(bytes.to_vec()).expect("response body is UTF-8")
}
