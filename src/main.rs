mod abuse;
mod captcha;
mod forum;
mod http;
mod media;
mod miya;

use sqlx::{SqlitePool, sqlite::SqliteConnectOptions};
use std::{collections::HashSet, fmt, path::PathBuf, str::FromStr, sync::Arc, time::Duration};

#[derive(Debug, PartialEq, Eq)]
enum BoardSlugConfigError {
    EmptyEntry,
    NotUnicode,
}

impl fmt::Display for BoardSlugConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyEntry => {
                formatter.write_str("MCHAN_ENABLED_BOARD_SLUGS must contain non-empty slugs")
            }
            Self::NotUnicode => {
                formatter.write_str("MCHAN_ENABLED_BOARD_SLUGS must be valid UTF-8")
            }
        }
    }
}

impl std::error::Error for BoardSlugConfigError {}

fn parse_enabled_board_slugs(
    value: Option<&str>,
) -> Result<Option<Vec<String>>, BoardSlugConfigError> {
    let Some(value) = value else {
        return Ok(None);
    };

    let mut slugs = Vec::new();
    for raw_slug in value.split(',') {
        let slug = raw_slug.trim();
        if slug.is_empty() {
            return Err(BoardSlugConfigError::EmptyEntry);
        }
        if !slugs.iter().any(|existing| existing == slug) {
            slugs.push(slug.to_owned());
        }
    }
    Ok(Some(slugs))
}

fn enabled_board_slugs_from_env() -> Result<Option<Vec<String>>, BoardSlugConfigError> {
    match std::env::var("MCHAN_ENABLED_BOARD_SLUGS") {
        Ok(value) => parse_enabled_board_slugs(Some(&value)),
        Err(std::env::VarError::NotPresent) => Ok(None),
        Err(std::env::VarError::NotUnicode(_)) => Err(BoardSlugConfigError::NotUnicode),
    }
}

const DEFAULT_MEDIA_STORAGE_ROOT: &str = "/data";

fn media_storage_root_from_env() -> PathBuf {
    std::env::var_os("MCHAN_MEDIA_STORAGE_ROOT")
        .map(PathBuf::from)
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| PathBuf::from(DEFAULT_MEDIA_STORAGE_ROOT))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let enabled_board_slugs = enabled_board_slugs_from_env()?;

    let moderator_emails = std::env::var("MCHAN_MODERATOR_EMAILS")
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|email| !email.is_empty())
        .map(|email| email.to_ascii_lowercase())
        .collect::<HashSet<_>>();

    let discord_moderation_token = std::env::var("MCHAN_DISCORD_MODERATION_TOKEN")
        .ok()
        .map(|token| token.trim().to_owned())
        .filter(|token| !token.is_empty());

    let media_processor = media::HttpMediaProcessor::from_env()?;
    let miya = miya::Miya::from_env()?;
    let media_storage_root = media_storage_root_from_env();
    let captcha = captcha::Captcha::from_env()?;

    let abuse_key = std::env::var("MCHAN_ABUSE_KEY").map_err(|_| {
        std::io::Error::other(
            "MCHAN_ABUSE_KEY is required; generate one with `openssl rand -hex 32`",
        )
    })?;
    let abuse_cipher = abuse::AbuseCipher::from_hex(&abuse_key)?;
    let database_url =
        std::env::var("DATABASE_URL").unwrap_or_else(|_| String::from("sqlite://mchan.db"));

    let options = SqliteConnectOptions::from_str(&database_url)?.create_if_missing(true);
    let pool = SqlitePool::connect_with(options).await?;
    sqlx::migrate!().run(&pool).await?;
    if let Some(enabled_board_slugs) = enabled_board_slugs.as_deref() {
        forum::apply_board_policy(&pool, enabled_board_slugs).await?;
    }
    forum::purge_expired_abuse_logs(&pool).await?;

    let cleanup_pool = pool.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(60 * 60));
        interval.tick().await;
        loop {
            interval.tick().await;
            if let Err(error) = forum::purge_expired_abuse_logs(&cleanup_pool).await {
                eprintln!("Could not purge expired abuse logs: {error}");
            }
        }
    });

    let app = http::router(http::HttpDependencies::new(
        pool,
        moderator_emails,
        abuse_cipher,
        captcha.map(|captcha| Arc::new(captcha) as Arc<dyn captcha::CaptchaVerifier>),
        media_processor.map(|processor| Arc::new(processor) as Arc<dyn media::MediaProcessor>),
        media_storage_root,
        miya.map(Arc::new),
        discord_moderation_token,
    ));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();

    axum::serve(listener, app).await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::parse_enabled_board_slugs;

    #[test]
    fn enabled_board_slugs_are_unset_by_default() {
        assert_eq!(parse_enabled_board_slugs(None).unwrap(), None);
    }

    #[test]
    fn enabled_board_slugs_trim_and_deduplicate_values() {
        assert_eq!(
            parse_enabled_board_slugs(Some(" b, pasum, b ")).unwrap(),
            Some(vec![String::from("b"), String::from("pasum")])
        );
    }

    #[test]
    fn enabled_board_slugs_reject_empty_configuration() {
        assert!(parse_enabled_board_slugs(Some("")).is_err());
        assert!(parse_enabled_board_slugs(Some("b, ,pasum")).is_err());
    }
}
