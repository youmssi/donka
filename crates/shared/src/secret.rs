//! Bearer tokens Studio issues to other systems (Runtime access tokens,
//! decision-log tokens): random, shown once, kept only as a SHA-256.

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use sha2::{Digest, Sha256};

/// Random bytes in a token: 256 bits, so an unsalted hash is enough to keep it.
const TOKEN_BYTES: usize = 32;
/// Characters of a token kept in clear so people can tell tokens apart.
const HINT_CHARS: usize = 4;

/// A token as issued: its value (shown once), the hash to store and look up,
/// and its last characters.
#[derive(Debug, Clone)]
pub struct IssuedSecret {
    pub token: String,
    pub hash: String,
    pub hint: String,
}

#[derive(Debug, thiserror::Error)]
#[error("the system random number generator failed")]
pub struct RandomFailed;

/// A fresh token starting with `prefix` (`dnk_`), so a leaked one is easy to recognise.
pub fn issue(prefix: &str) -> Result<IssuedSecret, RandomFailed> {
    let mut bytes = [0u8; TOKEN_BYTES];
    getrandom::fill(&mut bytes).map_err(|_| RandomFailed)?;
    let token = format!("{prefix}{}", URL_SAFE_NO_PAD.encode(bytes));
    let hint = token[token.len() - HINT_CHARS..].to_owned();
    Ok(IssuedSecret {
        hash: hash(&token),
        hint,
        token,
    })
}

/// Lowercase hex SHA-256 of the token's UTF-8 bytes.
pub fn hash(token: &str) -> String {
    Sha256::digest(token.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_are_random_prefixed_and_hashed() {
        let a = issue("dnk_log_").unwrap();
        let b = issue("dnk_log_").unwrap();
        assert!(a.token.starts_with("dnk_log_"));
        assert_ne!(a.token, b.token);
        assert_eq!(a.hash, hash(&a.token));
        assert!(a.token.ends_with(&a.hint));
        assert_eq!(a.hint.len(), HINT_CHARS);
    }

    #[test]
    fn the_hash_matches_the_artifact_format_test_vector() {
        assert_eq!(
            hash("dnk_test_token"),
            "d4d813b79f07c455e68458c955824329d902dd5f8e7b0c250fb3f05b3f68c840"
        );
    }
}
