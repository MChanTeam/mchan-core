use chacha20poly1305::{
    ChaCha20Poly1305, Key, Nonce,
    aead::{Aead, KeyInit, Payload},
};
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};
use std::{error::Error, fmt};

const AAD: &[u8] = b"mchan-post-origin-v1";
const ENCRYPTION_DOMAIN: &[u8] = b"mchan-abuse-encryption-v1";
const FINGERPRINT_DOMAIN: &[u8] = b"mchan-abuse-fingerprint-v1";

#[derive(Debug)]
pub(crate) enum AbuseCryptoError {
    InvalidKey,
    Random(getrandom::Error),
    Encryption,
    Decryption,
}

impl fmt::Display for AbuseCryptoError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidKey => formatter
                .write_str("MCHAN_ABUSE_KEY must contain exactly 64 hexadecimal characters"),
            Self::Random(error) => {
                write!(formatter, "could not generate an encryption nonce: {error}")
            }
            Self::Encryption => formatter.write_str("could not encrypt the abuse record"),
            Self::Decryption => formatter.write_str("could not decrypt the abuse record"),
        }
    }
}

impl Error for AbuseCryptoError {}

pub(crate) struct ProtectedClient {
    pub(crate) fingerprint: [u8; 32],
    pub(crate) nonce: [u8; 12],
    pub(crate) ciphertext: Vec<u8>,
}

pub(crate) struct AbuseCipher {
    encryption_key: [u8; 32],
    fingerprint_key: [u8; 32],
}

impl AbuseCipher {
    pub(crate) fn from_hex(encoded_key: &str) -> Result<Self, AbuseCryptoError> {
        let master_key = decode_key(encoded_key)?;

        Ok(Self {
            encryption_key: derive_key(&master_key, ENCRYPTION_DOMAIN),
            fingerprint_key: derive_key(&master_key, FINGERPRINT_DOMAIN),
        })
    }

    pub(crate) fn protect(&self, client_key: &str) -> Result<ProtectedClient, AbuseCryptoError> {
        let mut nonce = [0_u8; 12];
        getrandom::fill(&mut nonce).map_err(AbuseCryptoError::Random)?;

        let cipher = ChaCha20Poly1305::new(Key::from_slice(&self.encryption_key));
        let ciphertext = cipher
            .encrypt(
                Nonce::from_slice(&nonce),
                Payload {
                    msg: client_key.as_bytes(),
                    aad: AAD,
                },
            )
            .map_err(|_| AbuseCryptoError::Encryption)?;

        Ok(ProtectedClient {
            fingerprint: self.fingerprint(client_key),
            nonce,
            ciphertext,
        })
    }

    pub(crate) fn fingerprint(&self, client_key: &str) -> [u8; 32] {
        let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(&self.fingerprint_key)
            .expect("HMAC accepts keys of any size");
        mac.update(client_key.as_bytes());
        mac.finalize().into_bytes().into()
    }

    pub(crate) fn decrypt(
        &self,
        nonce: &[u8],
        ciphertext: &[u8],
    ) -> Result<String, AbuseCryptoError> {
        let nonce: &[u8; 12] = nonce.try_into().map_err(|_| AbuseCryptoError::Decryption)?;
        let cipher = ChaCha20Poly1305::new(Key::from_slice(&self.encryption_key));
        let plaintext = cipher
            .decrypt(
                Nonce::from_slice(nonce),
                Payload {
                    msg: ciphertext,
                    aad: AAD,
                },
            )
            .map_err(|_| AbuseCryptoError::Decryption)?;

        String::from_utf8(plaintext).map_err(|_| AbuseCryptoError::Decryption)
    }
}

fn derive_key(master_key: &[u8; 32], domain: &[u8]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update(master_key);
    digest.finalize().into()
}

fn decode_key(encoded: &str) -> Result<[u8; 32], AbuseCryptoError> {
    let encoded = encoded.as_bytes();
    if encoded.len() != 64 {
        return Err(AbuseCryptoError::InvalidKey);
    }

    let mut key = [0_u8; 32];
    for (index, byte) in key.iter_mut().enumerate() {
        let offset = index * 2;
        let high = hex_digit(encoded[offset]).ok_or(AbuseCryptoError::InvalidKey)?;
        let low = hex_digit(encoded[offset + 1]).ok_or(AbuseCryptoError::InvalidKey)?;
        *byte = (high << 4) | low;
    }

    Ok(key)
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

    #[test]
    fn protects_and_recovers_client_keys() {
        let cipher = AbuseCipher::from_hex(KEY).unwrap();
        let protected = cipher.protect("203.0.113.4").unwrap();

        assert_eq!(
            cipher
                .decrypt(&protected.nonce, &protected.ciphertext)
                .unwrap(),
            "203.0.113.4"
        );
        assert_eq!(protected.fingerprint, cipher.fingerprint("203.0.113.4"));
        assert_ne!(protected.ciphertext, b"203.0.113.4");
    }

    #[test]
    fn rejects_invalid_keys_and_tampered_records() {
        assert!(AbuseCipher::from_hex("not-a-key").is_err());
        assert!(AbuseCipher::from_hex(&format!(" {KEY}")).is_err());
        assert!(AbuseCipher::from_hex(&format!("{KEY} ")).is_err());
        assert!(AbuseCipher::from_hex(&KEY[..63]).is_err());
        assert!(AbuseCipher::from_hex(&format!("{KEY}0")).is_err());

        let cipher = AbuseCipher::from_hex(KEY).unwrap();
        let mut protected = cipher.protect("203.0.113.4").unwrap();
        protected.ciphertext[0] ^= 1;

        assert!(
            cipher
                .decrypt(&protected.nonce, &protected.ciphertext)
                .is_err()
        );
    }
}
