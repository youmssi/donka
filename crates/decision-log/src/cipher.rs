//! Encryption at rest of what a decision read and answered.
//!
//! AES-256-GCM with a fresh random nonce per record, and the record id as
//! associated data: a payload copied onto another record does not decrypt.
//! The key comes from `DONKA_DECISION_LOG_KEY`; each record names the key it
//! was sealed with (`key_id`, the start of the key's SHA-256), so a later key
//! rotation can tell old records from new ones. Losing the key makes the
//! records unreadable: it belongs in the installation's secret store.

use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use sha2::{Digest, Sha256};
use uuid::Uuid;

const KEY_BYTES: usize = 32;
const NONCE_BYTES: usize = 12;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum CipherError {
    #[error("must be {KEY_BYTES} random bytes in base64 (e.g. `openssl rand -base64 32`)")]
    InvalidKey,
    #[error("the system random number generator failed")]
    Random,
    #[error("the record was sealed with key {0}, which is not configured")]
    UnknownKey(String),
    #[error("the record could not be decrypted")]
    Corrupt,
}

/// A record's payload, sealed.
#[derive(Debug, Clone)]
pub struct Sealed {
    pub key_id: String,
    pub nonce: Vec<u8>,
    pub ciphertext: Vec<u8>,
}

pub struct Cipher {
    aead: Aes256Gcm,
    key_id: String,
}

// Neither the key nor anything derived from it beyond its public id is shown.
impl std::fmt::Debug for Cipher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Cipher")
            .field("key_id", &self.key_id)
            .finish()
    }
}

impl Cipher {
    /// From the base64 value of `DONKA_DECISION_LOG_KEY`.
    pub fn from_base64(key: &str) -> Result<Self, CipherError> {
        let bytes = STANDARD
            .decode(key.trim())
            .map_err(|_| CipherError::InvalidKey)?;
        if bytes.len() != KEY_BYTES {
            return Err(CipherError::InvalidKey);
        }
        let key_id = Sha256::digest(&bytes)[..8]
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        Ok(Self {
            aead: Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&bytes)),
            key_id,
        })
    }

    pub fn key_id(&self) -> &str {
        &self.key_id
    }

    pub fn seal(&self, record: Uuid, plaintext: &[u8]) -> Result<Sealed, CipherError> {
        let mut nonce = [0u8; NONCE_BYTES];
        getrandom::fill(&mut nonce).map_err(|_| CipherError::Random)?;
        let ciphertext = self
            .aead
            .encrypt(
                Nonce::from_slice(&nonce),
                Payload {
                    msg: plaintext,
                    aad: record.as_bytes(),
                },
            )
            .map_err(|_| CipherError::Corrupt)?;
        Ok(Sealed {
            key_id: self.key_id.clone(),
            nonce: nonce.to_vec(),
            ciphertext,
        })
    }

    pub fn open(&self, record: Uuid, sealed: &Sealed) -> Result<Vec<u8>, CipherError> {
        if sealed.key_id != self.key_id {
            return Err(CipherError::UnknownKey(sealed.key_id.clone()));
        }
        if sealed.nonce.len() != NONCE_BYTES {
            return Err(CipherError::Corrupt);
        }
        self.aead
            .decrypt(
                Nonce::from_slice(&sealed.nonce),
                Payload {
                    msg: &sealed.ciphertext,
                    aad: record.as_bytes(),
                },
            )
            .map_err(|_| CipherError::Corrupt)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: &str = "AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8=";

    #[test]
    fn a_sealed_payload_opens_only_for_its_record_and_key() {
        let cipher = Cipher::from_base64(KEY).unwrap();
        let record = Uuid::new_v4();
        let sealed = cipher.seal(record, b"{\"income\":1200}").unwrap();

        assert_eq!(sealed.key_id, cipher.key_id());
        assert!(!sealed.ciphertext.windows(6).any(|w| w == b"income"));
        assert_eq!(cipher.open(record, &sealed).unwrap(), b"{\"income\":1200}");
        assert_eq!(
            cipher.open(Uuid::new_v4(), &sealed),
            Err(CipherError::Corrupt),
            "moved to another record"
        );

        let other = Cipher::from_base64(&STANDARD.encode([7u8; 32])).unwrap();
        assert!(matches!(
            other.open(record, &sealed),
            Err(CipherError::UnknownKey(_))
        ));
    }

    #[test]
    fn two_seals_of_the_same_payload_differ() {
        let cipher = Cipher::from_base64(KEY).unwrap();
        let record = Uuid::new_v4();
        let a = cipher.seal(record, b"same").unwrap();
        let b = cipher.seal(record, b"same").unwrap();
        assert_ne!(a.nonce, b.nonce);
        assert_ne!(a.ciphertext, b.ciphertext);
    }

    #[test]
    fn keys_must_be_32_bytes_of_base64() {
        for bad in ["", "not base64!", &STANDARD.encode([1u8; 16])] {
            assert_eq!(
                Cipher::from_base64(bad).unwrap_err(),
                CipherError::InvalidKey
            );
        }
        assert!(!format!("{:?}", Cipher::from_base64(KEY).unwrap()).contains(KEY));
    }
}
