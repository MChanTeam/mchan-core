use reqwest::{Client, StatusCode, Url};
use serde::{Deserialize, Serialize};
use std::{error::Error, fmt, time::Duration};

const MIYA_URL_ENV: &str = "MCHAN_MIYA_URL";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(1);
const FALLBACK_CATEGORY: &str = "unspecified";
const FALLBACK_REVIEW_REASON: &str = "content requires review";
const FALLBACK_BLOCK_REASON: &str = "content blocked by moderation";

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum MiyaDecision {
    Allow,
    Review { category: String, reason: String },
    Block { category: String, reason: String },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum MiyaConfigError {
    InvalidEnvironmentEncoding,
    InvalidUrl,
}

impl fmt::Display for MiyaConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidEnvironmentEncoding => "MCHAN_MIYA_URL contains invalid UTF-8",
            Self::InvalidUrl => {
                "MCHAN_MIYA_URL must be a valid HTTP(S) URL without credentials, query, or fragment"
            }
        })
    }
}

impl Error for MiyaConfigError {}

#[derive(Debug)]
pub(crate) enum MiyaError {
    Request(reqwest::Error),
    UnexpectedStatus(StatusCode),
    MalformedResponse(reqwest::Error),
    UnknownAction,
}

impl fmt::Display for MiyaError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Request(_) => "Miya moderation is unavailable",
            Self::UnexpectedStatus(_) => "Miya moderation returned an unexpected response",
            Self::MalformedResponse(_) | Self::UnknownAction => {
                "Miya moderation returned an invalid response"
            }
        })
    }
}

impl Error for MiyaError {}

#[derive(Serialize)]
struct ModerateRequest<'a> {
    content: &'a str,
}

#[derive(Deserialize)]
struct ModerateResponse {
    action: String,
    #[serde(default)]
    categories: Vec<CategoryResponse>,
}

#[derive(Deserialize)]
struct CategoryResponse {
    category: String,
    score: f64,
    reason: String,
}

#[derive(Clone)]
pub(crate) struct Miya {
    client: Client,
    endpoint: Url,
}

impl Miya {
    pub(crate) fn from_env() -> Result<Option<Self>, MiyaConfigError> {
        match std::env::var(MIYA_URL_ENV) {
            Ok(value) if value.trim().is_empty() => Ok(None),
            Ok(value) => Self::new(value).map(Some),
            Err(std::env::VarError::NotPresent) => Ok(None),
            Err(std::env::VarError::NotUnicode(_)) => {
                Err(MiyaConfigError::InvalidEnvironmentEncoding)
            }
        }
    }

    pub(crate) fn new(base_url: impl AsRef<str>) -> Result<Self, MiyaConfigError> {
        let mut base_url =
            Url::parse(base_url.as_ref()).map_err(|_| MiyaConfigError::InvalidUrl)?;
        if !matches!(base_url.scheme(), "http" | "https")
            || base_url.host_str().is_none()
            || !base_url.username().is_empty()
            || base_url.password().is_some()
            || base_url.query().is_some()
            || base_url.fragment().is_some()
        {
            return Err(MiyaConfigError::InvalidUrl);
        }

        let path = base_url.path().trim_end_matches('/');
        base_url.set_path(&format!("{path}/v1/moderate/text"));

        let client = Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .build()
            .map_err(|_| MiyaConfigError::InvalidUrl)?;
        Ok(Self {
            client,
            endpoint: base_url,
        })
    }

    pub(crate) async fn moderate(&self, content: &str) -> Result<MiyaDecision, MiyaError> {
        let response = self
            .client
            .post(self.endpoint.clone())
            .json(&ModerateRequest { content })
            .send()
            .await
            .map_err(MiyaError::Request)?;
        let status = response.status();
        if !status.is_success() {
            return Err(MiyaError::UnexpectedStatus(status));
        }
        let moderation = response
            .json::<ModerateResponse>()
            .await
            .map_err(MiyaError::MalformedResponse)?;
        let category = moderation
            .categories
            .iter()
            .enumerate()
            .max_by(|(left_index, left), (right_index, right)| {
                left.score
                    .total_cmp(&right.score)
                    .then_with(|| right_index.cmp(left_index))
            })
            .map(|(_, category)| category);

        match moderation.action.as_str() {
            "allow" => Ok(MiyaDecision::Allow),
            "review" => Ok(MiyaDecision::Review {
                category: category
                    .map(|category| category.category.clone())
                    .unwrap_or_else(|| FALLBACK_CATEGORY.to_owned()),
                reason: category
                    .map(|category| category.reason.clone())
                    .unwrap_or_else(|| FALLBACK_REVIEW_REASON.to_owned()),
            }),
            "block" => Ok(MiyaDecision::Block {
                category: category
                    .map(|category| category.category.clone())
                    .unwrap_or_else(|| FALLBACK_CATEGORY.to_owned()),
                reason: category
                    .map(|category| category.reason.clone())
                    .unwrap_or_else(|| FALLBACK_BLOCK_REASON.to_owned()),
            }),
            _ => Err(MiyaError::UnknownAction),
        }
    }
}
