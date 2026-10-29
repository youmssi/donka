//! Studio licence files (DNK-77).
//!
//! Donka signs each customer's licence with an Ed25519 key that never leaves Donka; Studio
//! verifies it offline with the public key built into the binary ([`PublicKey::built_in`]).
//! A licence names the customer, the days it is valid and the modules it includes.
//!
//! Decisions never depend on it: the Runtime does not read licences. Without a valid licence
//! Studio stays readable and exportable but refuses new versions and releases; without any
//! licence file it runs in evaluation, where nothing reaches production.
//!
//! The file is JSON. The signed payload is kept as the exact bytes that were signed (base64),
//! so verification never depends on how JSON is re-serialised:
//!
//! ```json
//! { "format": "donka-licence/1", "keyId": "…", "payload": "…", "signature": "…" }
//! ```

use base64::engine::general_purpose::URL_SAFE_NO_PAD as B64;
use base64::Engine as _;
use chrono::{DateTime, NaiveDate, Utc};
use ed25519_dalek::{Signature, Signer as _, SigningKey, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fmt;
use uuid::Uuid;

/// The only format this version reads and writes.
pub const FORMAT: &str = "donka-licence/1";

/// Administrators are told this many days before the last valid day.
pub const NOTICE_DAYS: i64 = 30;

/// Longest customer name a licence carries.
pub const MAX_CUSTOMER_CHARS: usize = 200;

/// The public key built into Studio. Release builds trust Donka's key; builds with the
/// `development-key` feature (local development and CI) trust the development key instead, so
/// a test licence never verifies in a delivered Studio.
#[cfg(not(feature = "development-key"))]
const BUILT_IN_KEY: &str = include_str!("../keys/licence.pub");
#[cfg(feature = "development-key")]
const BUILT_IN_KEY: &str = include_str!("../keys/development.pub");

/// This build trusts the development key: local development and CI only.
pub const DEVELOPMENT_BUILD: bool = cfg!(feature = "development-key");

/// The key committed in `keys/licence.pub` until Donka's own replaces it. Nobody holds its
/// signing half, so it verifies no licence; CI on `main` refuses to release with it
/// (`scripts/check-licence-key.sh`).
pub const PLACEHOLDER_KEY_ID: &str = "f134b43fb8ce25c6";

#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
pub enum LicenceError {
    /// Not a licence file of a format this Studio reads; why, in words.
    #[error("not a readable licence file: {0}")]
    Unreadable(String),
    /// Signed with another key than the one built into this Studio.
    #[error("signed by another key (key {found}, this Studio trusts {expected})")]
    Foreign { found: String, expected: String },
    /// Its content changed after it was signed.
    #[error("its signature does not match its content: the file was changed after signing")]
    Tampered,
    /// A key file that is not an Ed25519 key in the expected form.
    #[error("invalid key: {0}")]
    InvalidKey(String),
    /// What a licence cannot say (no customer, dates in the wrong order…).
    #[error("invalid licence: {0}")]
    Invalid(String),
}

/// An optional part of Studio a licence includes or not. The core (editor, tests, releases,
/// approvals, the decision log and the rights of the people decisions are about) is always
/// included.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Module {
    /// Plain-language explanations of logged decisions (DNK-19).
    Explain,
    /// Connector nodes that call the customer's services (DNK-21, DNK-63).
    Connectors,
    /// Analysis on recorded decisions (DNK-56, DNK-57, DNK-58).
    Analysis,
}

impl Module {
    pub const ALL: [Self; 3] = [Self::Explain, Self::Connectors, Self::Analysis];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Explain => "explain",
            Self::Connectors => "connectors",
            Self::Analysis => "analysis",
        }
    }

    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|module| module.as_str() == name)
    }
}

impl fmt::Display for Module {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// What a licence says, once its signature is verified.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Licence {
    pub id: Uuid,
    pub customer: String,
    pub issued_at: DateTime<Utc>,
    /// First valid day (UTC), included.
    pub valid_from: NaiveDate,
    /// Last valid day (UTC), included.
    pub valid_until: NaiveDate,
    /// Modules this Studio knows; names it does not know are ignored, so a newer licence
    /// still works on an older Studio.
    pub modules: BTreeSet<Module>,
}

/// The signed payload as written. Field names are part of the format.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Payload {
    id: Uuid,
    customer: String,
    issued_at: DateTime<Utc>,
    valid_from: NaiveDate,
    valid_until: NaiveDate,
    modules: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Envelope {
    format: String,
    key_id: String,
    payload: String,
    signature: String,
}

/// An Ed25519 public key that licences are verified with.
#[derive(Clone)]
pub struct PublicKey {
    key: VerifyingKey,
    id: String,
}

impl fmt::Debug for PublicKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PublicKey").field("id", &self.id).finish()
    }
}

impl PublicKey {
    /// The key built into this Studio.
    pub fn built_in() -> Result<Self, LicenceError> {
        Self::from_text(BUILT_IN_KEY)
    }

    /// Reads a public key file: the 32 key bytes in base64 (URL-safe, no padding), on one line.
    pub fn from_text(text: &str) -> Result<Self, LicenceError> {
        let bytes = decode_key(text.trim())?;
        let key = VerifyingKey::from_bytes(&bytes)
            .map_err(|_| LicenceError::InvalidKey("not an Ed25519 public key".into()))?;
        Ok(Self::new(key))
    }

    fn new(key: VerifyingKey) -> Self {
        let id = key_id(key.as_bytes());
        Self { key, id }
    }

    /// A short fingerprint, written in each licence, that names the key without revealing
    /// anything: the first 16 hexadecimal digits of the SHA-256 of the key.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// The placeholder Donka's own key has not replaced yet: no licence verifies with it.
    pub fn is_placeholder(&self) -> bool {
        self.id == PLACEHOLDER_KEY_ID
    }

    /// The key file's content.
    pub fn to_text(&self) -> String {
        format!("{}\n", B64.encode(self.key.as_bytes()))
    }

    /// Verifies a licence file and reads what it says.
    pub fn verify(&self, text: &str) -> Result<Licence, LicenceError> {
        let envelope: Envelope =
            serde_json::from_str(text).map_err(|err| LicenceError::Unreadable(err.to_string()))?;
        if envelope.format != FORMAT {
            return Err(LicenceError::Unreadable(format!(
                "format `{}`, expected `{FORMAT}`",
                envelope.format
            )));
        }
        if envelope.key_id != self.id {
            return Err(LicenceError::Foreign {
                found: envelope.key_id,
                expected: self.id.clone(),
            });
        }
        let payload = B64
            .decode(envelope.payload.as_bytes())
            .map_err(|_| LicenceError::Unreadable("the payload is not base64".into()))?;
        let signature = B64
            .decode(envelope.signature.as_bytes())
            .ok()
            .and_then(|bytes| Signature::from_slice(&bytes).ok())
            .ok_or_else(|| {
                LicenceError::Unreadable("the signature is not an Ed25519 one".into())
            })?;
        self.key
            .verify_strict(&payload, &signature)
            .map_err(|_| LicenceError::Tampered)?;
        // Signed by Donka: from here on, a problem is a bug in the issuing tool.
        let payload: Payload = serde_json::from_slice(&payload)
            .map_err(|err| LicenceError::Invalid(err.to_string()))?;
        let licence = Licence {
            id: payload.id,
            customer: payload.customer,
            issued_at: payload.issued_at,
            valid_from: payload.valid_from,
            valid_until: payload.valid_until,
            modules: payload
                .modules
                .iter()
                .filter_map(|name| Module::parse(name))
                .collect(),
        };
        check(&licence)?;
        Ok(licence)
    }
}

/// Donka's signing key. Used only by the issuing tool, kept outside customer installations.
pub struct SigningKeyPair {
    key: SigningKey,
}

impl SigningKeyPair {
    /// A new key from 32 random bytes the caller drew from the system's generator.
    pub fn from_seed(seed: [u8; 32]) -> Self {
        Self {
            key: SigningKey::from_bytes(&seed),
        }
    }

    /// Reads a signing key file: the 32-byte seed in base64 (URL-safe, no padding).
    pub fn from_text(text: &str) -> Result<Self, LicenceError> {
        Ok(Self::from_seed(decode_key(text.trim())?))
    }

    /// The signing key file's content. Keep it offline.
    pub fn to_text(&self) -> String {
        format!("{}\n", B64.encode(self.key.to_bytes()))
    }

    pub fn public_key(&self) -> PublicKey {
        PublicKey::new(self.key.verifying_key())
    }

    /// Signs a licence and returns the licence file's content.
    pub fn issue(&self, licence: &Licence) -> Result<String, LicenceError> {
        check(licence)?;
        let payload = Payload {
            id: licence.id,
            customer: licence.customer.clone(),
            issued_at: licence.issued_at,
            valid_from: licence.valid_from,
            valid_until: licence.valid_until,
            modules: licence
                .modules
                .iter()
                .map(|module| module.as_str().to_owned())
                .collect(),
        };
        let payload =
            serde_json::to_vec(&payload).map_err(|err| LicenceError::Invalid(err.to_string()))?;
        let signature = self.key.sign(&payload);
        let envelope = Envelope {
            format: FORMAT.to_owned(),
            key_id: self.public_key().id().to_owned(),
            payload: B64.encode(&payload),
            signature: B64.encode(signature.to_bytes()),
        };
        let mut text = serde_json::to_string_pretty(&envelope)
            .map_err(|err| LicenceError::Invalid(err.to_string()))?;
        text.push('\n');
        Ok(text)
    }
}

/// Where an installation stands on a given day.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    /// No licence file configured: everything works except deploying to production.
    Evaluation,
    /// Valid today; `days_left` counts the valid days after today (0 on the last day).
    Active { licence: Licence, days_left: i64 },
    /// Signed by Donka but its first day has not come yet.
    NotYetValid { licence: Licence },
    /// Signed by Donka but its last day has passed.
    Expired { licence: Licence },
    /// A licence file is configured but cannot be trusted; why.
    Refused { reason: LicenceError },
}

impl Status {
    /// What the licence file read (or not) means on `today`.
    pub fn on(verified: Option<Result<Licence, LicenceError>>, today: NaiveDate) -> Self {
        match verified {
            None => Self::Evaluation,
            Some(Err(reason)) => Self::Refused { reason },
            Some(Ok(licence)) if today < licence.valid_from => Self::NotYetValid { licence },
            Some(Ok(licence)) if today > licence.valid_until => Self::Expired { licence },
            Some(Ok(licence)) => {
                let days_left = (licence.valid_until - today).num_days();
                Self::Active { licence, days_left }
            }
        }
    }

    /// New versions and releases are allowed.
    pub fn allows_changes(&self) -> bool {
        matches!(self, Self::Evaluation | Self::Active { .. })
    }

    /// Releases can reach production (anything but evaluation).
    pub fn allows_production(&self) -> bool {
        !matches!(self, Self::Evaluation)
    }

    /// The module is part of this installation. Evaluation tries every module.
    pub fn includes(&self, module: Module) -> bool {
        match self {
            Self::Evaluation => true,
            Self::Refused { .. } => false,
            Self::Active { licence, .. }
            | Self::NotYetValid { licence }
            | Self::Expired { licence } => licence.modules.contains(&module),
        }
    }

    /// Administrators should be told: the licence ends within [`NOTICE_DAYS`].
    pub fn expiring(&self) -> bool {
        matches!(self, Self::Active { days_left, .. } if *days_left <= NOTICE_DAYS)
    }

    pub fn licence(&self) -> Option<&Licence> {
        match self {
            Self::Active { licence, .. }
            | Self::NotYetValid { licence }
            | Self::Expired { licence } => Some(licence),
            Self::Evaluation | Self::Refused { .. } => None,
        }
    }
}

fn check(licence: &Licence) -> Result<(), LicenceError> {
    let customer = licence.customer.trim();
    if customer.is_empty() || customer.chars().count() > MAX_CUSTOMER_CHARS {
        return Err(LicenceError::Invalid(format!(
            "the customer needs 1 to {MAX_CUSTOMER_CHARS} characters"
        )));
    }
    if licence.valid_until < licence.valid_from {
        return Err(LicenceError::Invalid(
            "the last valid day is before the first".into(),
        ));
    }
    Ok(())
}

fn decode_key(text: &str) -> Result<[u8; 32], LicenceError> {
    let bytes = B64
        .decode(text.as_bytes())
        .map_err(|_| LicenceError::InvalidKey("not base64 (URL-safe, no padding)".into()))?;
    bytes
        .try_into()
        .map_err(|_| LicenceError::InvalidKey("not 32 bytes long".into()))
}

fn key_id(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .take(8)
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[cfg(test)]
mod tests;
