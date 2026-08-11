use super::*;
use crate::captcha::{CaptchaVerifier, VerificationUnavailable};
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{
        HeaderName, HeaderValue, Method, Request, Response,
        header::{CONTENT_TYPE, COOKIE},
    },
};
use std::{
    collections::{HashSet, VecDeque},
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
};
use tower::ServiceExt;

mod moderation;
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

fn test_router(pool: SqlitePool) -> Router {
    router(test_dependencies(pool, HashSet::new(), None))
}

fn moderator_router(pool: SqlitePool) -> Router {
    router(test_dependencies(
        pool,
        HashSet::from([String::from(MODERATOR_EMAIL)]),
        None,
    ))
}

fn captcha_router(pool: SqlitePool, outcomes: impl IntoIterator<Item = CaptchaOutcome>) -> Router {
    router(test_dependencies(
        pool,
        HashSet::new(),
        Some(Arc::new(ScriptedCaptcha::new(outcomes))),
    ))
}

fn test_dependencies(
    pool: SqlitePool,
    moderator_emails: HashSet<String>,
    captcha: Option<Arc<dyn CaptchaVerifier>>,
) -> HttpDependencies {
    HttpDependencies::new(
        pool,
        moderator_emails,
        abuse::AbuseCipher::from_hex(TEST_ABUSE_KEY).expect("valid test abuse key"),
        captcha,
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

fn with_cookie(request: Request<Body>, value: &'static str) -> Request<Body> {
    with_header(request, COOKIE.as_str(), value)
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
