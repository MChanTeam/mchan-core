use axum::http::{HeaderMap, header::HOST};

const DESCRIPTION_LIMIT: usize = 200;
const FORWARDED_PROTO: &str = "x-forwarded-proto";

fn host_name(host: &str) -> &str {
    if let Some(rest) = host.strip_prefix('[') {
        return rest.split(']').next().unwrap_or(rest);
    }

    host.split(':').next().unwrap_or(host)
}

fn is_local(host: &str) -> bool {
    matches!(host_name(host), "localhost" | "127.0.0.1" | "::1")
}

fn forwarded_scheme(headers: &HeaderMap) -> Option<&'static str> {
    let value = headers.get(FORWARDED_PROTO)?.to_str().ok()?;
    match value.split(',').next()?.trim() {
        "http" => Some("http"),
        "https" => Some("https"),
        _ => None,
    }
}

pub(super) fn request_origin(headers: &HeaderMap) -> Option<String> {
    let host = headers.get(HOST)?.to_str().ok()?.trim();
    if host.is_empty() || host.contains('/') || host.split_whitespace().count() != 1 {
        return None;
    }

    let scheme = forwarded_scheme(headers).unwrap_or(if is_local(host) { "http" } else { "https" });

    Some(format!("{scheme}://{host}"))
}

pub(super) fn absolute(origin: Option<&String>, path: &str) -> Option<String> {
    let origin = origin?;
    if path.starts_with('/') {
        Some(format!("{origin}{path}"))
    } else {
        Some(format!("{origin}/{path}"))
    }
}

pub(super) fn summarize(body: &str) -> String {
    let collapsed = body.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.chars().count() <= DESCRIPTION_LIMIT {
        return collapsed;
    }

    let kept: String = collapsed.chars().take(DESCRIPTION_LIMIT).collect();
    format!("{}...", kept.trim_end())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn headers(pairs: &[(&str, &str)]) -> HeaderMap {
        let mut headers = HeaderMap::new();
        for (name, value) in pairs {
            headers.insert(
                axum::http::HeaderName::from_bytes(name.as_bytes()).unwrap(),
                value.parse().unwrap(),
            );
        }
        headers
    }

    #[test]
    fn public_host_defaults_to_https() {
        let headers = headers(&[("host", "mchan.example")]);
        assert_eq!(
            request_origin(&headers),
            Some(String::from("https://mchan.example"))
        );
    }

    #[test]
    fn loopback_host_defaults_to_http() {
        let headers = headers(&[("host", "localhost:3000")]);
        assert_eq!(
            request_origin(&headers),
            Some(String::from("http://localhost:3000"))
        );
    }

    #[test]
    fn forwarded_proto_wins_over_the_default() {
        let headers = headers(&[("host", "mchan.example"), ("x-forwarded-proto", "http")]);
        assert_eq!(
            request_origin(&headers),
            Some(String::from("http://mchan.example"))
        );
    }

    #[test]
    fn first_forwarded_proto_is_used() {
        let headers = headers(&[
            ("host", "mchan.example"),
            ("x-forwarded-proto", "https, http"),
        ]);
        assert_eq!(
            request_origin(&headers),
            Some(String::from("https://mchan.example"))
        );
    }

    #[test]
    fn malformed_host_is_rejected() {
        assert_eq!(request_origin(&headers(&[("host", "evil/path")])), None);
        assert_eq!(request_origin(&headers(&[("host", "two hosts")])), None);
        assert_eq!(request_origin(&HeaderMap::new()), None);
    }

    #[test]
    fn absolute_joins_paths() {
        let origin = String::from("https://mchan.example");
        assert_eq!(
            absolute(Some(&origin), "/images/a/display.webp"),
            Some(String::from("https://mchan.example/images/a/display.webp"))
        );
        assert_eq!(absolute(None, "/images/a/display.webp"), None);
    }

    #[test]
    fn summarize_collapses_whitespace() {
        assert_eq!(summarize("one\n\ntwo   three"), "one two three");
    }

    #[test]
    fn summarize_truncates_long_bodies() {
        let body = "ab".repeat(400);
        let summary = summarize(&body);
        assert!(summary.ends_with("..."));
        assert_eq!(summary.chars().count(), DESCRIPTION_LIMIT + 3);
    }

    #[test]
    fn summarize_keeps_multibyte_characters_intact() {
        let body = "あ".repeat(400);
        let summary = summarize(&body);
        assert_eq!(summary.chars().count(), DESCRIPTION_LIMIT + 3);
    }
}
