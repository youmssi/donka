//! Tokens handed to browsers and password hashes. Nothing here is ever logged.

use argon2::{Argon2, PasswordHasher, PasswordVerifier};
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use sha2::{Digest, Sha256};

/// 256 bits: long enough that guessing a live session is not a concern.
const TOKEN_BYTES: usize = 32;

/// Verifying against this hash when the account is unknown, locked or has no
/// password keeps every failed sign-in equally slow, so response time does not
/// reveal whether an email exists. The password behind it is irrelevant.
const DUMMY_HASH: &str =
    "$argon2id$v=19$m=19456,t=2,p=1$ykT+Br1ZKcc/HchZdMeDhw$au++p+p9WJnqy+DxmsGBZ4vmHrwD+0I0ro5NEyKzzcE";

pub const MIN_PASSWORD_CHARS: usize = 12;
/// Upper bound so a huge password cannot be used to burn CPU in argon2.
pub const MAX_PASSWORD_CHARS: usize = 128;

#[derive(Debug, thiserror::Error)]
pub enum SecretError {
    #[error("the system random number generator failed")]
    Random,
    #[error("password hashing failed")]
    Hashing,
}

/// A fresh random token (URL-safe, no padding) and the hash to store for it.
pub fn new_token() -> Result<(String, Vec<u8>), SecretError> {
    let mut bytes = [0u8; TOKEN_BYTES];
    getrandom::fill(&mut bytes).map_err(|_| SecretError::Random)?;
    let token = URL_SAFE_NO_PAD.encode(bytes);
    let hash = hash_token(&token);
    Ok((token, hash))
}

/// What the database stores and looks up instead of the token itself.
pub fn hash_token(token: &str) -> Vec<u8> {
    Sha256::digest(token.as_bytes()).to_vec()
}

/// argon2id with the library defaults (m=19 MiB, t=2, p=1, OWASP's recommendation).
/// CPU-heavy: call it from a blocking thread.
pub fn hash_password(password: &str) -> Result<String, SecretError> {
    Argon2::default()
        .hash_password(password.as_bytes())
        .map(|hash| hash.to_string())
        .map_err(|_| SecretError::Hashing)
}

/// Checks `password` against `stored`, or against a dummy hash when there is
/// no usable stored hash, so the time taken is the same either way.
pub fn verify_password(password: &str, stored: Option<&str>) -> bool {
    let matches = Argon2::default()
        .verify_password(password.as_bytes(), stored.unwrap_or(DUMMY_HASH))
        .is_ok();
    matches && stored.is_some()
}

#[cfg(test)]
#[path = "secret/tests.rs"]
mod tests;
