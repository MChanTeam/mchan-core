use crate::forum;
use reqwest::{Client, StatusCode, Url, multipart};
use serde::Deserialize;
use std::{error::Error, fmt, future::Future, pin::Pin, time::Duration};

pub(crate) const MAX_UPLOAD_BYTES: usize = 20 * 1024 * 1024;
const IMAGE_SERVICE_URL_ENV: &str = "MCHAN_IMAGE_SERVICE_URL";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct MediaUpload {
    pub(crate) filename: Option<String>,
    pub(crate) content_type: Option<String>,
    pub(crate) bytes: Vec<u8>,
}

pub(crate) struct ProcessedMedia {
    pub(crate) image_id: String,
    pub(crate) media: forum::Media,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum MediaError {
    TooLarge,
    UnsupportedType,
    InvalidImage,
    Timeout,
    Unavailable,
    UpstreamProtocolError,
    MalformedResponse,
    CleanupFailed,
    InvalidImageId,
    #[doc(hidden)]
    UnexpectedStatus(u16),
}

impl fmt::Display for MediaError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::TooLarge => "image upload is too large",
            Self::UnsupportedType => "image type is not supported",
            Self::InvalidImage => "image is invalid",
            Self::Timeout => "image processing timed out",
            Self::Unavailable | Self::UnexpectedStatus(_) => "image processing is unavailable",
            Self::UpstreamProtocolError => "image processing rejected the request",
            Self::MalformedResponse => "image processing returned an invalid response",
            Self::CleanupFailed => "processed image cleanup failed",
            Self::InvalidImageId => "processed image identifier is invalid",
        };
        formatter.write_str(message)
    }
}

impl Error for MediaError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum MediaConfigError {
    InvalidEnvironmentEncoding,
    EmptyUrl,
    InvalidUrl,
}

impl fmt::Display for MediaConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidEnvironmentEncoding => "MCHAN_IMAGE_SERVICE_URL contains invalid UTF-8",
            Self::EmptyUrl => "MCHAN_IMAGE_SERVICE_URL must not be empty",
            Self::InvalidUrl => {
                "MCHAN_IMAGE_SERVICE_URL must be a valid HTTP(S) URL without credentials, query, or fragment"
            }
        })
    }
}

impl Error for MediaConfigError {}

pub(crate) trait MediaProcessor: Send + Sync {
    fn process<'a>(
        &'a self,
        upload: MediaUpload,
    ) -> Pin<Box<dyn Future<Output = Result<ProcessedMedia, MediaError>> + Send + 'a>>;

    fn delete<'a>(
        &'a self,
        image_id: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<(), MediaError>> + Send + 'a>>;
}

#[derive(Clone)]
pub(crate) struct HttpMediaProcessor {
    client: Client,
    base_url: Url,
}

impl HttpMediaProcessor {
    pub(crate) fn from_env() -> Result<Option<Self>, MediaConfigError> {
        match std::env::var(IMAGE_SERVICE_URL_ENV) {
            Ok(value) => Self::new(value).map(Some),
            Err(std::env::VarError::NotPresent) => Ok(None),
            Err(std::env::VarError::NotUnicode(_)) => {
                Err(MediaConfigError::InvalidEnvironmentEncoding)
            }
        }
    }

    pub(crate) fn new(base_url: impl AsRef<str>) -> Result<Self, MediaConfigError> {
        let base_url = base_url.as_ref();
        if base_url.trim().is_empty() {
            return Err(MediaConfigError::EmptyUrl);
        }

        let mut base_url = Url::parse(base_url).map_err(|_| MediaConfigError::InvalidUrl)?;
        if !matches!(base_url.scheme(), "http" | "https")
            || base_url.host_str().is_none()
            || !base_url.username().is_empty()
            || base_url.password().is_some()
            || base_url.query().is_some()
            || base_url.fragment().is_some()
        {
            return Err(MediaConfigError::InvalidUrl);
        }

        let path = base_url.path().trim_end_matches('/');
        base_url.set_path(&format!("{path}/"));

        Ok(Self {
            client: Client::builder()
                .timeout(REQUEST_TIMEOUT)
                .build()
                .map_err(|_| MediaConfigError::InvalidUrl)?,
            base_url,
        })
    }

    fn endpoint(&self, segments: &[&str]) -> Url {
        let mut url = self.base_url.clone();
        {
            let mut path = url
                .path_segments_mut()
                .expect("HTTP(S) URLs always support path segments");
            path.pop_if_empty();
            for segment in segments {
                path.push(segment);
            }
        }
        url
    }

    async fn process_upload(&self, upload: MediaUpload) -> Result<ProcessedMedia, MediaError> {
        if upload.bytes.len() > MAX_UPLOAD_BYTES {
            return Err(MediaError::TooLarge);
        }

        let mut file = multipart::Part::bytes(upload.bytes)
            .file_name(upload.filename.unwrap_or_else(|| "upload".to_owned()));
        if let Some(content_type) = upload.content_type {
            file = file
                .mime_str(&content_type)
                .map_err(|_| MediaError::UnsupportedType)?;
        }

        let form = multipart::Form::new()
            .part("file", file)
            .text("variants", "display,thumbnail");
        let response = self
            .client
            .post(self.endpoint(&["v1", "images"]))
            .multipart(form)
            .send()
            .await
            .map_err(classify_transport)?;

        let status = response.status();
        if !status.is_success() {
            return Err(match status {
                StatusCode::BAD_REQUEST => MediaError::UpstreamProtocolError,
                StatusCode::UNPROCESSABLE_ENTITY => MediaError::InvalidImage,
                StatusCode::PAYLOAD_TOO_LARGE => MediaError::TooLarge,
                StatusCode::UNSUPPORTED_MEDIA_TYPE => MediaError::UnsupportedType,
                StatusCode::SERVICE_UNAVAILABLE => MediaError::Unavailable,
                StatusCode::GATEWAY_TIMEOUT => MediaError::Timeout,
                _ => MediaError::UnexpectedStatus(status.as_u16()),
            });
        }

        let payload = response.json::<ImageResponse>().await.map_err(|error| {
            if error.is_timeout() {
                MediaError::Timeout
            } else if error.is_decode() {
                MediaError::MalformedResponse
            } else {
                MediaError::Unavailable
            }
        })?;
        processed_media(payload)
    }

    async fn delete_image(&self, image_id: &str) -> Result<(), MediaError> {
        if !valid_image_id(image_id) {
            return Err(MediaError::InvalidImageId);
        }

        let response = self
            .client
            .delete(self.endpoint(&["v1", "images", image_id]))
            .send()
            .await
            .map_err(classify_transport)?;
        if response.status().is_success() {
            Ok(())
        } else {
            Err(MediaError::CleanupFailed)
        }
    }
}

impl MediaProcessor for HttpMediaProcessor {
    fn process<'a>(
        &'a self,
        upload: MediaUpload,
    ) -> Pin<Box<dyn Future<Output = Result<ProcessedMedia, MediaError>> + Send + 'a>> {
        Box::pin(self.process_upload(upload))
    }

    fn delete<'a>(
        &'a self,
        image_id: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<(), MediaError>> + Send + 'a>> {
        Box::pin(self.delete_image(image_id))
    }
}

#[derive(Deserialize)]
struct ImageResponse {
    image_id: String,
    variants: Vec<ImageVariant>,
}

#[derive(Deserialize)]
struct ImageVariant {
    name: String,
    storage_key: String,
    content_type: String,
    width: u64,
    height: u64,
    size_bytes: u64,
}

fn processed_media(response: ImageResponse) -> Result<ProcessedMedia, MediaError> {
    if !valid_image_id(&response.image_id) || response.variants.len() != 2 {
        return Err(MediaError::MalformedResponse);
    }

    let image_id = response.image_id;
    let mut display = None;
    let mut thumbnail = None;
    for variant in response.variants {
        let (max_dimension, expected_filename) = match variant.name.as_str() {
            "display" if display.is_none() => (512_u64, "display.webp"),
            "thumbnail" if thumbnail.is_none() => (128_u64, "thumbnail.webp"),
            _ => return Err(MediaError::MalformedResponse),
        };
        if variant.content_type != "image/webp"
            || variant.width == 0
            || variant.height == 0
            || variant.width.max(variant.height) > max_dimension
            || variant.size_bytes == 0
            || public_path(&variant.storage_key, &image_id, expected_filename).is_none()
        {
            return Err(MediaError::MalformedResponse);
        }

        match variant.name.as_str() {
            "display" => display = Some(variant),
            "thumbnail" => thumbnail = Some(variant),
            _ => unreachable!("variant name validated above"),
        }
    }

    let display = display.ok_or(MediaError::MalformedResponse)?;
    let thumbnail = thumbnail.ok_or(MediaError::MalformedResponse)?;
    let display_path = public_path(&display.storage_key, &image_id, "display.webp")
        .ok_or(MediaError::MalformedResponse)?;
    let thumbnail_path = public_path(&thumbnail.storage_key, &image_id, "thumbnail.webp")
        .ok_or(MediaError::MalformedResponse)?;

    Ok(ProcessedMedia {
        image_id,
        media: forum::Media {
            thumbnail_path,
            display_path,
            mime_type: "image/webp".to_owned(),
            width: display.width,
            height: display.height,
        },
    })
}

fn public_path(storage_key: &str, image_id: &str, expected_filename: &str) -> Option<String> {
    let mut segments = storage_key.split('/');
    if segments.next()? != "images"
        || segments.next()? != image_id
        || segments.next()? != expected_filename
        || segments.next().is_some()
    {
        return None;
    }
    if !storage_key.split('/').all(safe_path_segment) {
        return None;
    }
    Some(format!("/{storage_key}"))
}

fn safe_path_segment(segment: &str) -> bool {
    !segment.is_empty()
        && segment != "."
        && segment != ".."
        && segment
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~'))
}

fn valid_image_id(image_id: &str) -> bool {
    image_id.starts_with("img_") && image_id.len() > "img_".len() && safe_path_segment(image_id)
}

fn classify_transport(error: reqwest::Error) -> MediaError {
    if error.is_timeout() {
        MediaError::Timeout
    } else {
        MediaError::Unavailable
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::{env, sync::Mutex};
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
        task::JoinHandle,
    };

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn with_image_service_url<T>(value: Option<&str>, test: impl FnOnce() -> T) -> T {
        let _lock = ENV_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let previous = env::var_os(IMAGE_SERVICE_URL_ENV);
        unsafe {
            match value {
                Some(value) => env::set_var(IMAGE_SERVICE_URL_ENV, value),
                None => env::remove_var(IMAGE_SERVICE_URL_ENV),
            }
        }
        let result = test();
        unsafe {
            match previous {
                Some(value) => env::set_var(IMAGE_SERVICE_URL_ENV, value),
                None => env::remove_var(IMAGE_SERVICE_URL_ENV),
            }
        }
        result
    }

    async fn server_once(status: u16, body: String) -> (String, JoinHandle<Vec<u8>>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut request = Vec::new();
            loop {
                let mut chunk = [0; 4096];
                let read = stream.read(&mut chunk).await.unwrap();
                if read == 0 {
                    break;
                }
                request.extend_from_slice(&chunk[..read]);
                if let Some(header_end) =
                    request.windows(4).position(|window| window == b"\r\n\r\n")
                {
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
            stream.write_all(response.as_bytes()).await.unwrap();
            stream.shutdown().await.unwrap();
            request
        });
        (format!("http://{address}"), server)
    }

    fn variants_json(display_key: &str, thumbnail_key: &str) -> String {
        format!(
            r#"{{"image_id":"img_123","variants":[
                {{"name":"thumbnail","storage_key":"{thumbnail_key}","content_type":"image/webp","width":128,"height":96,"size_bytes":1024}},
                {{"name":"display","storage_key":"{display_key}","content_type":"image/webp","width":512,"height":384,"size_bytes":8192}}
            ]}}"#
        )
    }

    fn valid_image_response() -> ImageResponse {
        ImageResponse {
            image_id: "img_123".to_owned(),
            variants: vec![
                ImageVariant {
                    name: "thumbnail".to_owned(),
                    storage_key: "images/img_123/thumbnail.webp".to_owned(),
                    content_type: "image/webp".to_owned(),
                    width: 128,
                    height: 96,
                    size_bytes: 1024,
                },
                ImageVariant {
                    name: "display".to_owned(),
                    storage_key: "images/img_123/display.webp".to_owned(),
                    content_type: "image/webp".to_owned(),
                    width: 512,
                    height: 384,
                    size_bytes: 8192,
                },
            ],
        }
    }

    #[test]
    fn image_service_url_is_optional_and_validated_when_present() {
        with_image_service_url(None, || {
            assert!(HttpMediaProcessor::from_env().unwrap().is_none());
        });
        with_image_service_url(Some(""), || {
            assert!(matches!(
                HttpMediaProcessor::from_env(),
                Err(MediaConfigError::EmptyUrl)
            ));
        });
        with_image_service_url(Some("https://user:pass@example.test/images?x=1"), || {
            assert!(matches!(
                HttpMediaProcessor::from_env(),
                Err(MediaConfigError::InvalidUrl)
            ));
        });
        with_image_service_url(Some("http://127.0.0.1:8787/images///"), || {
            let processor = HttpMediaProcessor::from_env().unwrap().unwrap();
            assert_eq!(processor.base_url.as_str(), "http://127.0.0.1:8787/images/");
        });
    }

    #[test]
    fn process_posts_file_and_requests_named_variants() {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            let body = variants_json(
                "images/img_123/display.webp",
                "images/img_123/thumbnail.webp",
            );
            let (base_url, server) = server_once(200, body).await;
            let processor = HttpMediaProcessor::new(base_url).unwrap();
            let processed = processor
                .process(MediaUpload {
                    filename: Some("photo.png".to_owned()),
                    content_type: Some("image/png".to_owned()),
                    bytes: vec![0, 1, 2, 3],
                })
                .await
                .unwrap();
            let request = server.await.unwrap();
            let request_text = String::from_utf8_lossy(&request);
            assert!(request_text.starts_with("POST /v1/images HTTP/1.1\r\n"));
            assert!(request_text.contains("name=\"file\""));
            assert!(request_text.contains("filename=\"photo.png\""));
            assert!(request_text.contains("name=\"variants\""));
            assert!(request_text.contains("display,thumbnail"));
            assert!(
                request
                    .windows(4)
                    .any(|window| window == [0_u8, 1, 2, 3].as_slice())
            );
            assert_eq!(processed.image_id, "img_123");
            assert_eq!(processed.media.display_path, "/images/img_123/display.webp");
            assert_eq!(
                processed.media.thumbnail_path,
                "/images/img_123/thumbnail.webp"
            );
            assert_eq!(processed.media.mime_type, "image/webp");
            assert_eq!(processed.media.width, 512);
            assert_eq!(processed.media.height, 384);
        });
    }

    #[test]
    fn named_variants_must_include_display_and_thumbnail() {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            for variants in [
                r#"[{"name":"thumbnail","storage_key":"images/img_123/thumbnail.webp","content_type":"image/webp","width":128,"height":96,"size_bytes":1024},{"name":"other","storage_key":"images/img_123/other.webp","content_type":"image/webp","width":1,"height":1,"size_bytes":1}]"#,
                r#"[{"name":"display","storage_key":"images/img_123/display.webp","content_type":"image/webp","width":512,"height":384,"size_bytes":8192},{"name":"other","storage_key":"images/img_123/other.webp","content_type":"image/webp","width":1,"height":1,"size_bytes":1}]"#,
            ] {
                let body = format!(r#"{{"image_id":"img_123","variants":{variants}}}"#);
                let (base_url, server) = server_once(200, body).await;
                let processor = HttpMediaProcessor::new(base_url).unwrap();
                assert!(matches!(
                    processor
                        .process(MediaUpload {
                            filename: None,
                            content_type: None,
                            bytes: vec![1],
                        })
                        .await,
                    Err(MediaError::MalformedResponse)
                ));
                server.await.unwrap();
            }
        });
    }

    #[test]
    fn unsafe_storage_key_is_rejected() {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            let body = variants_json("../display.webp", "images/img_123/thumbnail.webp");
            let (base_url, server) = server_once(200, body).await;
            let processor = HttpMediaProcessor::new(base_url).unwrap();
            assert!(matches!(
                processor
                    .process(MediaUpload {
                        filename: None,
                        content_type: None,
                        bytes: vec![1],
                    })
                    .await,
                Err(MediaError::MalformedResponse)
            ));
            server.await.unwrap();
        });
    }

    #[test]
    fn invalid_success_metadata_is_rejected() {
        let mut response = valid_image_response();
        response.image_id = "image-123".to_owned();
        assert!(matches!(
            processed_media(response),
            Err(MediaError::MalformedResponse)
        ));

        let mut response = valid_image_response();
        response.variants[1].width = 513;
        assert!(matches!(
            processed_media(response),
            Err(MediaError::MalformedResponse)
        ));

        let mut response = valid_image_response();
        response.variants[0].height = 129;
        assert!(matches!(
            processed_media(response),
            Err(MediaError::MalformedResponse)
        ));

        let mut response = valid_image_response();
        response.variants[0].size_bytes = 0;
        assert!(matches!(
            processed_media(response),
            Err(MediaError::MalformedResponse)
        ));

        let mut response = valid_image_response();
        response.variants[1].storage_key = "images/img_other/display.webp".to_owned();
        assert!(matches!(
            processed_media(response),
            Err(MediaError::MalformedResponse)
        ));
    }

    #[test]
    fn process_statuses_map_to_stable_media_errors() {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            for (status, expected) in [
                (400, MediaError::UpstreamProtocolError),
                (413, MediaError::TooLarge),
                (415, MediaError::UnsupportedType),
                (422, MediaError::InvalidImage),
                (504, MediaError::Timeout),
            ] {
                let (base_url, server) = server_once(status, String::new()).await;
                let processor = HttpMediaProcessor::new(base_url).unwrap();
                let result = processor
                    .process(MediaUpload {
                        filename: None,
                        content_type: None,
                        bytes: vec![1],
                    })
                    .await;
                assert!(matches!(result, Err(error) if error == expected));
                server.await.unwrap();
            }
        });
    }

    #[test]
    fn delete_hits_image_cleanup_endpoint() {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            let (base_url, server) = server_once(204, String::new()).await;
            let processor = HttpMediaProcessor::new(base_url).unwrap();
            processor.delete("img_123").await.unwrap();
            let request = server.await.unwrap();
            let request_text = String::from_utf8_lossy(&request);
            assert!(request_text.starts_with("DELETE /v1/images/img_123 HTTP/1.1\r\n"));
        });
    }
}
