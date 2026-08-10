use reqwest::{Client, Url};
use serde::{Deserialize, Serialize};
use std::{env, error::Error, fmt};

const SITE_KEY_ENV: &str = "MCHAN_TURNSTILE_SITE_KEY";
const SECRET_KEY_ENV: &str = "MCHAN_TURNSTILE_SECRET_KEY";
const VERIFY_URL_ENV: &str = "MCHAN_TURNSTILE_VERIFY_URL";
const DEFAULT_VERIFY_URL: &str = "https://challenges.cloudflare.com/turnstile/v0/siteverify";

#[derive(Debug)]
pub(crate) enum CaptchaError {
    InvalidConfiguration(&'static str),
    InvalidEnvironmentEncoding(&'static str),
    InvalidVerifyUrl,
    EmptyToken,
    Transport(reqwest::Error),
    Response(reqwest::Error),
}

impl fmt::Display for CaptchaError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidConfiguration(message) => formatter.write_str(message),
            Self::InvalidEnvironmentEncoding(name) => {
                write!(formatter, "{name} contains invalid UTF-8")
            }
            Self::InvalidVerifyUrl => formatter.write_str(
                "Turnstile verification URL must be a valid HTTP(S) URL without credentials or fragments; HTTPS is required except for loopback HTTP",
            ),
            Self::EmptyToken => formatter.write_str("Turnstile response token is empty"),
            Self::Transport(error) => {
                write!(formatter, "Turnstile verification request failed: {error}")
            }
            Self::Response(error) => write!(
                formatter,
                "Turnstile verification response was invalid: {error}"
            ),
        }
    }
}

impl Error for CaptchaError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Transport(error) | Self::Response(error) => Some(error),
            _ => None,
        }
    }
}

pub(crate) struct Captcha {
    site_key: String,
    secret_key: String,
    verify_url: Url,
    client: Client,
}

impl Captcha {
    /// Read optional Turnstile configuration from the process environment.
    ///
    /// CAPTCHA is disabled when both keys are absent. Supplying only one key,
    /// or supplying an empty key, is a startup configuration error.
    pub(crate) fn from_env() -> Result<Option<Self>, CaptchaError> {
        let site_key = environment_value(SITE_KEY_ENV)?;
        let secret_key = environment_value(SECRET_KEY_ENV)?;

        match (site_key, secret_key) {
            (None, None) => Ok(None),
            (Some(_), None) | (None, Some(_)) => Err(CaptchaError::InvalidConfiguration(
                "MCHAN_TURNSTILE_SITE_KEY and MCHAN_TURNSTILE_SECRET_KEY must be set together",
            )),
            (Some(site_key), Some(secret_key)) => {
                let verify_url = environment_value(VERIFY_URL_ENV)?
                    .unwrap_or_else(|| DEFAULT_VERIFY_URL.to_owned());
                Self::with_verify_url(site_key, secret_key, verify_url).map(Some)
            }
        }
    }

    /// Build a client with an explicit siteverify URL, useful for deterministic
    /// local tests while retaining Cloudflare as the production default.
    pub(crate) fn with_verify_url(
        site_key: impl Into<String>,
        secret_key: impl Into<String>,
        verify_url: impl AsRef<str>,
    ) -> Result<Self, CaptchaError> {
        let site_key = site_key.into();
        let secret_key = secret_key.into();
        if site_key.trim().is_empty() {
            return Err(CaptchaError::InvalidConfiguration(
                "MCHAN_TURNSTILE_SITE_KEY must not be empty",
            ));
        }
        if secret_key.trim().is_empty() {
            return Err(CaptchaError::InvalidConfiguration(
                "MCHAN_TURNSTILE_SECRET_KEY must not be empty",
            ));
        }
        let verify_url = parse_verify_url(verify_url.as_ref())?;

        Ok(Self {
            site_key,
            secret_key,
            verify_url,
            client: Client::new(),
        })
    }

    pub(crate) fn site_key(&self) -> &str {
        &self.site_key
    }

    pub(crate) async fn verify(&self, token: &str, remote_ip: &str) -> Result<bool, CaptchaError> {
        if token.trim().is_empty() {
            return Err(CaptchaError::EmptyToken);
        }

        let request = VerifyRequest {
            secret: &self.secret_key,
            response: token,
            remoteip: remote_ip,
        };
        let response = self
            .client
            .post(self.verify_url.clone())
            .form(&request)
            .send()
            .await
            .map_err(CaptchaError::Transport)?
            .error_for_status()
            .map_err(CaptchaError::Transport)?;

        let result = response
            .json::<VerifyResponse>()
            .await
            .map_err(CaptchaError::Response)?;
        Ok(result.success)
    }
}

fn environment_value(name: &'static str) -> Result<Option<String>, CaptchaError> {
    match env::var(name) {
        Ok(value) => Ok(Some(value)),
        Err(env::VarError::NotPresent) => Ok(None),
        Err(env::VarError::NotUnicode(_)) => Err(CaptchaError::InvalidEnvironmentEncoding(name)),
    }
}

fn parse_verify_url(value: &str) -> Result<Url, CaptchaError> {
    let url = Url::parse(value).map_err(|_| CaptchaError::InvalidVerifyUrl)?;

    let authority_contains_userinfo = value
        .split_once("://")
        .and_then(|(_, remainder)| {
            remainder
                .split(|character| matches!(character, '/' | '?' | '#'))
                .next()
        })
        .map_or(false, |authority| authority.contains('@'));
    if authority_contains_userinfo
        || url.username() != ""
        || url.password().is_some()
        || url.fragment().is_some()
    {
        return Err(CaptchaError::InvalidVerifyUrl);
    }

    let host = url.host_str().ok_or(CaptchaError::InvalidVerifyUrl)?;
    let loopback_host = host
        .strip_prefix('[')
        .and_then(|host| host.strip_suffix(']'))
        .unwrap_or(host);
    let allowed = match url.scheme() {
        "https" => true,
        "http" => {
            matches!(loopback_host, "127.0.0.1" | "::1")
                || loopback_host.eq_ignore_ascii_case("localhost")
        }
        _ => false,
    };
    if !allowed {
        return Err(CaptchaError::InvalidVerifyUrl);
    }

    Ok(url)
}

#[derive(Serialize)]
struct VerifyRequest<'a> {
    secret: &'a str,
    response: &'a str,
    remoteip: &'a str,
}

#[derive(Deserialize)]
struct VerifyResponse {
    success: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn set_env(name: &str, value: Option<&str>) {
        // Environment mutation is unsafe in Rust 2024 because other threads
        // may concurrently observe the process environment. Tests serialize
        // all mutations with ENV_LOCK.
        unsafe {
            match value {
                Some(value) => env::set_var(name, value),
                None => env::remove_var(name),
            }
        }
    }

    #[test]
    fn absent_keys_disable_captcha() {
        let _lock = ENV_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        set_env(SITE_KEY_ENV, None);
        set_env(SECRET_KEY_ENV, None);
        set_env(VERIFY_URL_ENV, Some("%%%"));

        assert!(Captcha::from_env().unwrap().is_none());
        set_env(VERIFY_URL_ENV, None);
    }

    #[test]
    fn partial_keys_are_rejected() {
        let _lock = ENV_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        set_env(SITE_KEY_ENV, Some("site"));
        set_env(SECRET_KEY_ENV, None);

        assert!(matches!(
            Captcha::from_env(),
            Err(CaptchaError::InvalidConfiguration(_))
        ));
        set_env(SITE_KEY_ENV, None);
    }

    #[test]
    fn complete_keys_produce_client_without_exposing_secret() {
        let _lock = ENV_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        set_env(SITE_KEY_ENV, Some("site-key"));
        set_env(SECRET_KEY_ENV, Some("secret-key"));
        let error = match Captcha::with_verify_url("site", "secret", "%%%") {
            Err(error) => error,
            Ok(_) => panic!("invalid verify URL unexpectedly accepted"),
        };
        assert_eq!(
            error.to_string(),
            "Turnstile verification URL must be a valid HTTP(S) URL without credentials or fragments; HTTPS is required except for loopback HTTP"
        );
        set_env(VERIFY_URL_ENV, None);
        let captcha = Captcha::from_env().unwrap().unwrap();
        assert_eq!(captcha.site_key(), "site-key");

        set_env(SITE_KEY_ENV, None);
        set_env(SECRET_KEY_ENV, None);
    }

    #[test]
    fn environment_verify_url_override_uses_same_validation() {
        let _lock = ENV_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        set_env(SITE_KEY_ENV, Some("site-key"));
        set_env(SECRET_KEY_ENV, Some("secret-key"));
        set_env(VERIFY_URL_ENV, Some("http://example.test/siteverify"));

        assert!(matches!(
            Captcha::from_env(),
            Err(CaptchaError::InvalidVerifyUrl)
        ));

        set_env(SITE_KEY_ENV, None);
        set_env(SECRET_KEY_ENV, None);
        set_env(VERIFY_URL_ENV, None);
    }

    #[test]
    fn official_https_verify_url_is_accepted() {
        let captcha = Captcha::with_verify_url("site", "secret", DEFAULT_VERIFY_URL).unwrap();
        assert_eq!(captcha.verify_url.as_str(), DEFAULT_VERIFY_URL);
    }

    #[test]
    fn arbitrary_http_verify_url_is_rejected() {
        assert!(matches!(
            Captcha::with_verify_url("site", "secret", "http://example.test/siteverify"),
            Err(CaptchaError::InvalidVerifyUrl)
        ));
    }

    #[test]
    fn loopback_http_verify_urls_are_accepted() {
        for url in [
            "http://127.0.0.1:8787/siteverify",
            "http://[::1]:8787/siteverify",
            "http://localhost:8787/siteverify",
        ] {
            assert!(
                Captcha::with_verify_url("site", "secret", url).is_ok(),
                "loopback URL should be accepted: {url}"
            );
        }
    }

    #[test]
    fn malformed_verify_url_is_rejected() {
        assert!(matches!(
            Captcha::with_verify_url("site", "secret", "%%%"),
            Err(CaptchaError::InvalidVerifyUrl)
        ));
    }

    #[test]
    fn verify_url_credentials_are_rejected() {
        assert!(matches!(
            Captcha::with_verify_url(
                "site",
                "secret",
                "https://user:password@example.test/siteverify"
            ),
            Err(CaptchaError::InvalidVerifyUrl)
        ));
    }

    #[test]
    fn verify_url_fragments_are_rejected() {
        assert!(matches!(
            Captcha::with_verify_url("site", "secret", "https://example.test/siteverify#fragment"),
            Err(CaptchaError::InvalidVerifyUrl)
        ));
    }

    #[test]
    fn empty_tokens_are_rejected_before_network() {
        let captcha = Captcha::with_verify_url("site", "secret", DEFAULT_VERIFY_URL).unwrap();
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let result = runtime.block_on(captcha.verify("   ", ""));
        assert!(matches!(result, Err(CaptchaError::EmptyToken)));
    }

    #[test]
    fn response_success_is_the_only_authorization_value() {
        assert!(VerifyResponse { success: true }.success);
        assert!(!VerifyResponse { success: false }.success);
    }
}
