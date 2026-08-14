use crate::abuse::AbuseCipher;
use std::{
    collections::HashSet,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

pub(crate) const COOKIE_NAME: &str = "__Host-mchan-moderator";

const TOKEN_VERSION: &str = "v1";
const SESSION_TTL: Duration = Duration::from_secs(8 * 60 * 60);
const SESSION_PAYLOAD_DOMAIN: &str = "mchan-moderator-session-payload-v1";

pub(crate) fn issue(cipher: &AbuseCipher, email: &str, now: SystemTime) -> String {
    let issued_at = unix_seconds(now);
    let normalized_email = normalize_email(email);
    let payload = session_payload(issued_at, &normalized_email);
    let signature = cipher.sign_moderator_session(payload.as_bytes());

    format!("{TOKEN_VERSION}.{issued_at}.{}", encode_mac(&signature))
}

pub(crate) fn verify(
    cipher: &AbuseCipher,
    allowed_emails: &HashSet<String>,
    token: &str,
    now: SystemTime,
) -> Option<String> {
    let mut parts = token.split('.');
    let version = parts.next()?;
    let encoded_issued_at = parts.next()?;
    let issued_at = encoded_issued_at.parse::<u64>().ok()?;
    if encoded_issued_at.is_empty()
        || !encoded_issued_at.bytes().all(|byte| byte.is_ascii_digit())
        || (encoded_issued_at.len() > 1 && encoded_issued_at.starts_with('0'))
    {
        return None;
    }
    let encoded_mac = parts.next()?;
    if parts.next().is_some() || version != TOKEN_VERSION {
        return None;
    }

    let mac = decode_mac(encoded_mac)?;
    let now = unix_seconds(now);
    if issued_at > now || now.saturating_sub(issued_at) >= SESSION_TTL.as_secs() {
        return None;
    }

    let mut matching_email = None;
    for allowed_email in allowed_emails {
        let normalized_email = normalize_email(allowed_email);
        let payload = session_payload(issued_at, &normalized_email);
        if cipher.verify_moderator_session(payload.as_bytes(), &mac) {
            matching_email = Some(allowed_email.clone());
        }
    }
    matching_email
}

fn normalize_email(email: &str) -> String {
    email.trim().to_ascii_lowercase()
}

fn session_payload(issued_at: u64, normalized_email: &str) -> String {
    format!("{SESSION_PAYLOAD_DOMAIN}\0{TOKEN_VERSION}\0{issued_at}\0{normalized_email}")
}

fn unix_seconds(now: SystemTime) -> u64 {
    now.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs()
}

fn encode_mac(mac: &[u8; 32]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(mac.len() * 2);
    for &byte in mac {
        encoded.push(HEX[(byte >> 4) as usize] as char);
        encoded.push(HEX[(byte & 0x0f) as usize] as char);
    }
    encoded
}

fn decode_mac(encoded: &str) -> Option<[u8; 32]> {
    if encoded.len() != 64 {
        return None;
    }

    let mut decoded = [0_u8; 32];
    for (index, byte) in decoded.iter_mut().enumerate() {
        let offset = index * 2;
        let high = hex_digit(encoded.as_bytes()[offset])?;
        let low = hex_digit(encoded.as_bytes()[offset + 1])?;
        *byte = (high << 4) | low;
    }
    Some(decoded)
}

fn hex_digit(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: &str = "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f";
    const NOW_SECONDS: u64 = 1_000_000;

    fn cipher() -> AbuseCipher {
        AbuseCipher::from_hex(KEY).unwrap()
    }

    fn now() -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(NOW_SECONDS)
    }

    #[test]
    fn issues_and_verifies_an_opaque_current_moderator_session() {
        let cipher = cipher();
        let allowed_emails = HashSet::from([String::from("mod@example.com")]);
        let token = issue(&cipher, "MOD@example.com", now());
        let parts: Vec<_> = token.split('.').collect();

        assert_eq!(parts.len(), 3);
        assert_eq!(parts[0], TOKEN_VERSION);
        assert_eq!(parts[1], NOW_SECONDS.to_string());
        assert_eq!(parts[2].len(), 64);
        assert!(!token.contains("MOD@example.com"));
        assert!(!token.contains("mod@example.com"));
        assert!(!token.contains("6d6f64406578616d706c652e636f6d"));
        assert_eq!(
            verify(&cipher, &allowed_emails, &token, now()),
            Some(String::from("mod@example.com"))
        );
    }

    #[test]
    fn rejects_a_tampered_session() {
        let cipher = cipher();
        let allowed_emails = HashSet::from([String::from("mod@example.com")]);
        let mut token = issue(&cipher, "mod@example.com", now());
        let index = token.len() - 1;
        token.replace_range(index..=index, "0");

        assert_eq!(verify(&cipher, &allowed_emails, &token, now()), None);
    }

    #[test]
    fn rejects_expired_sessions() {
        let cipher = cipher();
        let allowed_emails = HashSet::from([String::from("mod@example.com")]);
        let token = issue(&cipher, "mod@example.com", now());
        let expired = now() + SESSION_TTL;

        assert_eq!(verify(&cipher, &allowed_emails, &token, expired), None);
    }

    #[test]
    fn rejects_sessions_for_removed_moderators() {
        let cipher = cipher();
        let token = issue(&cipher, "mod@example.com", now());

        assert_eq!(verify(&cipher, &HashSet::new(), &token, now()), None);
    }

    #[test]
    fn rejects_sessions_from_the_future() {
        let cipher = cipher();
        let allowed_emails = HashSet::from([String::from("mod@example.com")]);
        let token = issue(&cipher, "mod@example.com", now() + Duration::from_secs(1));

        assert_eq!(verify(&cipher, &allowed_emails, &token, now()), None);
    }
}
