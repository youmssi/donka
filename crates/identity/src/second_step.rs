//! Second sign-in step with an authenticator app (DNK-59).
//!
//! Time-based one-time codes (RFC 6238: SHA-1, six digits, 30-second steps, one
//! step of clock tolerance). A code is accepted once: the newest accepted step is
//! stored and older or equal steps are refused. The secret is sealed with
//! `DONKA_SECOND_STEP_KEY` (the DNK-40 keyring, the user id as associated data)
//! and re-sealed with the current key whenever a code is accepted. Ten recovery
//! codes, shown once, are stored as SHA-256 of the user id and the code.

use crate::emails::{Kind, Notice};
use crate::{queue_email, secret, AccountRow, Identity, IdentityError, IssuedToken, User};
use chrono::{DateTime, Duration, Utc};
use donka_audit::{Action, Event};
use donka_shared::cipher::Sealed;
use serde_json::json;
use sha2::{Digest, Sha256};
use totp_rs::{Algorithm, Builder, Secret, Totp};
use uuid::Uuid;

/// Shown as the account's provider in the authenticator app.
const ISSUER: &str = "Donka";
/// 160 bits, the size RFC 4226 recommends for SHA-1.
const SECRET_BYTES: usize = 20;
const STEP_SECONDS: u64 = 30;
pub const RECOVERY_CODE_COUNT: usize = 10;
/// Base32 without padding: 12 characters, shown as `xxxx-xxxx-xxxx` (60 bits).
const RECOVERY_CODE_CHARS: usize = 12;
const RECOVERY_ALPHABET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
/// How long the second step may follow a correct password.
const PENDING_LIFETIME_MINUTES: i64 = 5;

/// Why a sign-in waits for a second step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Purpose {
    /// The account has a second step: ask for its code.
    Code,
    /// Policy requires a second step the account does not have yet.
    Enrol,
}

impl Purpose {
    fn as_str(self) -> &'static str {
        match self {
            Self::Code => "code",
            Self::Enrol => "enrol",
        }
    }
}

/// What a correct password leads to.
#[derive(Debug)]
pub enum SignIn {
    /// No second step: the session is open.
    Session(IssuedToken, User),
    /// The account's code comes next; the token proves the password step.
    Code(IssuedToken),
    /// Policy requires a second step the account has not set up: enrol first.
    Enrol(IssuedToken),
}

/// What an authenticator app needs: the key as text and as an `otpauth://` URL
/// (the QR code). Returned once, when an enrolment starts.
#[derive(Debug)]
pub struct Enrolment {
    pub key: String,
    pub otpauth_url: String,
}

/// Recovery codes in clear, shown once.
#[derive(Debug)]
pub struct RecoveryCodes(pub Vec<String>);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SecondStepStatus {
    pub enabled: bool,
    pub recovery_codes_left: i64,
}

#[derive(sqlx::FromRow)]
struct SecretRow {
    key_id: String,
    nonce: Vec<u8>,
    ciphertext: Vec<u8>,
    last_used_step: Option<i64>,
}

impl Identity {
    /// Whether the user has a second step, and how many recovery codes are unused.
    pub async fn second_step_status(&self, user: Uuid) -> Result<SecondStepStatus, IdentityError> {
        let (enabled, left): (bool, i64) = sqlx::query_as(
            "SELECT EXISTS (SELECT 1 FROM second_steps WHERE user_id = $1 AND confirmed_at IS NOT NULL), \
                    (SELECT count(*) FROM recovery_codes WHERE user_id = $1 AND used_at IS NULL)",
        )
        .bind(user)
        .fetch_one(&self.pool)
        .await?;
        Ok(SecondStepStatus {
            enabled,
            recovery_codes_left: if enabled { left } else { 0 },
        })
    }

    /// Starts (or restarts) setting up a second step for a signed-in person.
    pub async fn begin_enrolment(&self, user: &User) -> Result<Enrolment, IdentityError> {
        let mut tx = self.pool.begin().await?;
        let enrolment = self.store_new_secret(&mut tx, user).await?;
        tx.commit().await?;
        Ok(enrolment)
    }

    /// Starts setting up a second step during a sign-in that policy holds until
    /// the person has one.
    pub async fn begin_enrolment_at_sign_in(
        &self,
        pending: &str,
    ) -> Result<Enrolment, IdentityError> {
        let now = self.clock.now();
        let mut tx = self.pool.begin().await?;
        let user = pending_user(&mut tx, pending, Purpose::Enrol, now).await?;
        let enrolment = self.store_new_secret(&mut tx, &user).await?;
        tx.commit().await?;
        Ok(enrolment)
    }

    /// Turns the second step on once the person typed a valid code, and returns
    /// their recovery codes.
    pub async fn confirm_enrolment(
        &self,
        user: &User,
        code: &str,
    ) -> Result<RecoveryCodes, IdentityError> {
        let now = self.clock.now();
        let mut tx = self.pool.begin().await?;
        let codes = self.confirm(&mut tx, user, code, now).await?;
        tx.commit().await?;
        self.outbox.notify_one();
        Ok(codes)
    }

    /// Confirms the enrolment a sign-in was held for, then opens the session.
    pub async fn confirm_enrolment_at_sign_in(
        &self,
        pending: &str,
        code: &str,
    ) -> Result<(IssuedToken, User, RecoveryCodes), IdentityError> {
        let now = self.clock.now();
        let mut tx = self.pool.begin().await?;
        let user = pending_user(&mut tx, pending, Purpose::Enrol, now).await?;
        let codes = self.confirm(&mut tx, &user, code, now).await?;
        consume_pending(&mut tx, pending, now).await?;
        let session = self
            .open_session(&mut tx, user.id, now, json!({ "secondStep": "enrolled" }))
            .await?;
        tx.commit().await?;
        self.outbox.notify_one();
        Ok((session, user, codes))
    }

    /// Finishes a sign-in with an authenticator code or a recovery code. Wrong
    /// codes count towards the account lockout and get the same answer as a
    /// wrong password.
    pub async fn complete_sign_in(
        &self,
        pending: &str,
        code: &str,
    ) -> Result<(IssuedToken, User), IdentityError> {
        let now = self.clock.now();
        let mut tx = self.pool.begin().await?;
        let user = pending_user(&mut tx, pending, Purpose::Code, now).await?;
        let account: AccountRow = sqlx::query_as(
            "SELECT id, email, is_admin, locale, password_hash, failed_sign_ins, \
                    failed_window_started_at, locked_until \
             FROM users WHERE id = $1 FOR UPDATE",
        )
        .bind(user.id)
        .fetch_one(&mut *tx)
        .await?;
        if account.locked_until.is_some_and(|until| until > now) {
            tracing::warn!(user_id = %user.id, "second step refused: account is locked");
            return Err(IdentityError::InvalidCredentials);
        }

        let method = if self.accept_code(&mut tx, user.id, code, now).await? {
            "code"
        } else if let Some(remaining) = use_recovery_code(&mut tx, user.id, code, now).await? {
            donka_audit::record(
                &mut tx,
                Event::new(now, Some(user.id), Action::UserRecoveryCodeUsed)
                    .about_user(user.id)
                    .with_details(json!({ "remaining": remaining })),
            )
            .await?;
            queue_email(
                &mut tx,
                user.id,
                Kind::Notice(Notice::RecoveryCodeUsed),
                now,
            )
            .await?;
            "recovery_code"
        } else {
            self.record_failure(&mut tx, &account, now).await?;
            tx.commit().await?;
            return Err(IdentityError::InvalidCredentials);
        };

        consume_pending(&mut tx, pending, now).await?;
        let session = self
            .open_session(&mut tx, user.id, now, json!({ "secondStep": method }))
            .await?;
        tx.commit().await?;
        self.outbox.notify_one();
        Ok((session, user))
    }

    /// New recovery codes replace the old ones; needs a current authenticator code.
    pub async fn regenerate_recovery_codes(
        &self,
        user: &User,
        code: &str,
    ) -> Result<RecoveryCodes, IdentityError> {
        let now = self.clock.now();
        let mut tx = self.pool.begin().await?;
        require_enabled(&mut tx, user.id).await?;
        if !self.accept_code(&mut tx, user.id, code, now).await? {
            return Err(IdentityError::InvalidCode);
        }
        let codes = replace_recovery_codes(&mut tx, user.id, now).await?;
        donka_audit::record(
            &mut tx,
            Event::new(now, Some(user.id), Action::UserRecoveryCodesRegenerated)
                .about_user(user.id),
        )
        .await?;
        tx.commit().await?;
        Ok(codes)
    }

    /// Turns the person's own second step off. Needs an authenticator or recovery
    /// code, and is refused when policy requires a second step for them.
    pub async fn remove_second_step(
        &self,
        user: &User,
        code: &str,
        required_by_policy: bool,
    ) -> Result<(), IdentityError> {
        if required_by_policy {
            return Err(IdentityError::SecondStepRequired);
        }
        let now = self.clock.now();
        let mut tx = self.pool.begin().await?;
        require_enabled(&mut tx, user.id).await?;
        let valid = self.accept_code(&mut tx, user.id, code, now).await?
            || use_recovery_code(&mut tx, user.id, code, now)
                .await?
                .is_some();
        if !valid {
            return Err(IdentityError::InvalidCode);
        }
        delete_second_step(&mut tx, user.id).await?;
        donka_audit::record(
            &mut tx,
            Event::new(now, Some(user.id), Action::UserSecondStepRemoved).about_user(user.id),
        )
        .await?;
        queue_email(
            &mut tx,
            user.id,
            Kind::Notice(Notice::SecondStepRemoved),
            now,
        )
        .await?;
        tx.commit().await?;
        self.outbox.notify_one();
        Ok(())
    }

    /// An administrator removes another person's second step (lost device) and
    /// signs them out everywhere. They set it up again at their next sign-in
    /// when policy requires it.
    pub async fn reset_second_step(&self, admin: &User, user: Uuid) -> Result<(), IdentityError> {
        if !admin.is_admin || admin.id == user {
            return Err(IdentityError::Forbidden);
        }
        let now = self.clock.now();
        let mut tx = self.pool.begin().await?;
        require_enabled(&mut tx, user).await?;
        delete_second_step(&mut tx, user).await?;
        sqlx::query("DELETE FROM sessions WHERE user_id = $1")
            .bind(user)
            .execute(&mut *tx)
            .await?;
        donka_audit::record(
            &mut tx,
            Event::new(now, Some(admin.id), Action::UserSecondStepReset).about_user(user),
        )
        .await?;
        queue_email(&mut tx, user, Kind::Notice(Notice::SecondStepReset), now).await?;
        tx.commit().await?;
        self.outbox.notify_one();
        tracing::info!(admin = %admin.id, user_id = %user, "second step reset by an administrator");
        Ok(())
    }

    /// Ends the password step of a sign-in: the person must use the code (or
    /// enrol) within a few minutes, once.
    pub(crate) async fn hold_for_second_step(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        user: Uuid,
        enrolled: bool,
        now: DateTime<Utc>,
    ) -> Result<SignIn, IdentityError> {
        let purpose = if enrolled {
            Purpose::Code
        } else {
            Purpose::Enrol
        };
        // One pending sign-in per person: a new password step replaces the previous one.
        sqlx::query("DELETE FROM pending_sign_ins WHERE user_id = $1")
            .bind(user)
            .execute(&mut **tx)
            .await?;
        let (token, token_hash) = secret::new_token()?;
        sqlx::query(
            "INSERT INTO pending_sign_ins (token_hash, user_id, purpose, created_at, expires_at) \
             VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(token_hash)
        .bind(user)
        .bind(purpose.as_str())
        .bind(now)
        .bind(now + Duration::minutes(PENDING_LIFETIME_MINUTES))
        .execute(&mut **tx)
        .await?;
        let token = IssuedToken(token);
        Ok(match purpose {
            Purpose::Code => SignIn::Code(token),
            Purpose::Enrol => SignIn::Enrol(token),
        })
    }

    async fn store_new_secret(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        user: &User,
    ) -> Result<Enrolment, IdentityError> {
        let confirmed: Option<(Option<DateTime<Utc>>,)> =
            sqlx::query_as("SELECT confirmed_at FROM second_steps WHERE user_id = $1 FOR UPDATE")
                .bind(user.id)
                .fetch_optional(&mut **tx)
                .await?;
        if confirmed.is_some_and(|(at,)| at.is_some()) {
            return Err(IdentityError::SecondStepAlreadyOn);
        }
        let mut bytes = [0u8; SECRET_BYTES];
        getrandom::fill(&mut bytes)
            .map_err(|_| IdentityError::Internal("the random number generator failed".into()))?;
        let totp = totp(bytes.to_vec(), &user.email)?;
        let sealed = self.second_step_keys.seal(user.id, &bytes)?;
        sqlx::query(
            "INSERT INTO second_steps (user_id, key_id, nonce, ciphertext, created_at) \
             VALUES ($1, $2, $3, $4, $5) \
             ON CONFLICT (user_id) DO UPDATE SET key_id = $2, nonce = $3, ciphertext = $4, \
             created_at = $5, confirmed_at = NULL, last_used_step = NULL",
        )
        .bind(user.id)
        .bind(&sealed.key_id)
        .bind(&sealed.nonce)
        .bind(&sealed.ciphertext)
        .bind(self.clock.now())
        .execute(&mut **tx)
        .await?;
        Ok(Enrolment {
            key: Secret::from(bytes.to_vec()).to_base32(),
            otpauth_url: totp
                .to_url()
                .map_err(|err| IdentityError::Internal(err.to_string()))?,
        })
    }

    async fn confirm(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        user: &User,
        code: &str,
        now: DateTime<Utc>,
    ) -> Result<RecoveryCodes, IdentityError> {
        let pending: Option<(Option<DateTime<Utc>>,)> =
            sqlx::query_as("SELECT confirmed_at FROM second_steps WHERE user_id = $1 FOR UPDATE")
                .bind(user.id)
                .fetch_optional(&mut **tx)
                .await?;
        match pending {
            None => return Err(IdentityError::SecondStepOff),
            Some((Some(_),)) => return Err(IdentityError::SecondStepAlreadyOn),
            Some((None,)) => {}
        }
        if !self.accept_code(tx, user.id, code, now).await? {
            return Err(IdentityError::InvalidCode);
        }
        sqlx::query("UPDATE second_steps SET confirmed_at = $2 WHERE user_id = $1")
            .bind(user.id)
            .bind(now)
            .execute(&mut **tx)
            .await?;
        let codes = replace_recovery_codes(tx, user.id, now).await?;
        donka_audit::record(
            tx,
            Event::new(now, Some(user.id), Action::UserSecondStepEnabled).about_user(user.id),
        )
        .await?;
        queue_email(tx, user.id, Kind::Notice(Notice::SecondStepEnabled), now).await?;
        Ok(codes)
    }

    /// Checks an authenticator code against the user's secret (confirmed or being
    /// set up). A code for a step already accepted is refused. On success the
    /// step is recorded and the secret is re-sealed with the current key.
    async fn accept_code(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        user: Uuid,
        code: &str,
        now: DateTime<Utc>,
    ) -> Result<bool, IdentityError> {
        let code: String = code.chars().filter(|c| !c.is_whitespace()).collect();
        if code.len() != 6 || !code.chars().all(|c| c.is_ascii_digit()) {
            return Ok(false);
        }
        let row: Option<SecretRow> = sqlx::query_as(
            "SELECT key_id, nonce, ciphertext, last_used_step FROM second_steps \
             WHERE user_id = $1 FOR UPDATE",
        )
        .bind(user)
        .fetch_optional(&mut **tx)
        .await?;
        let Some(row) = row else { return Ok(false) };
        let bytes = self.second_step_keys.open(
            user,
            &Sealed {
                key_id: row.key_id.clone(),
                nonce: row.nonce,
                ciphertext: row.ciphertext,
            },
        )?;
        let time = u64::try_from(now.timestamp()).unwrap_or(0);
        let Some(step) = totp(bytes.clone(), "account")?.check(&code, time) else {
            return Ok(false);
        };
        let step = i64::try_from(step).unwrap_or(i64::MAX);
        if row.last_used_step.is_some_and(|last| step <= last) {
            return Ok(false);
        }
        let resealed = if row.key_id == self.second_step_keys.key_id() {
            None
        } else {
            Some(self.second_step_keys.seal(user, &bytes)?)
        };
        sqlx::query(
            "UPDATE second_steps SET last_used_step = $2, \
             key_id = COALESCE($3, key_id), nonce = COALESCE($4, nonce), \
             ciphertext = COALESCE($5, ciphertext) WHERE user_id = $1",
        )
        .bind(user)
        .bind(step)
        .bind(resealed.as_ref().map(|s| s.key_id.clone()))
        .bind(resealed.as_ref().map(|s| s.nonce.clone()))
        .bind(resealed.as_ref().map(|s| s.ciphertext.clone()))
        .execute(&mut **tx)
        .await?;
        Ok(true)
    }
}

fn totp(secret: Vec<u8>, account: &str) -> Result<Totp, IdentityError> {
    Builder::new()
        .with_algorithm(Algorithm::SHA1)
        .with_digits(6)
        .with_skew(1)
        .with_step_duration(STEP_SECONDS)
        .with_secret(secret)
        .with_issuer(Some(ISSUER))
        // A colon would break the otpauth label; emails never contain one.
        .with_account_name(account.replace(':', ""))
        .build()
        .map_err(|err| IdentityError::Internal(err.to_string()))
}

/// The user a pending sign-in belongs to, if it is still valid for `purpose`.
async fn pending_user(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    pending: &str,
    purpose: Purpose,
    now: DateTime<Utc>,
) -> Result<User, IdentityError> {
    sqlx::query_as(
        "SELECT u.id, u.email, u.is_admin, u.locale \
         FROM pending_sign_ins p JOIN users u ON u.id = p.user_id \
         WHERE p.token_hash = $1 AND p.purpose = $2 AND p.used_at IS NULL AND p.expires_at > $3 \
         FOR UPDATE OF p",
    )
    .bind(secret::hash_token(pending))
    .bind(purpose.as_str())
    .bind(now)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(IdentityError::SignInExpired)
}

async fn consume_pending(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    pending: &str,
    now: DateTime<Utc>,
) -> Result<(), IdentityError> {
    sqlx::query("UPDATE pending_sign_ins SET used_at = $2 WHERE token_hash = $1")
        .bind(secret::hash_token(pending))
        .bind(now)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

async fn require_enabled(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user: Uuid,
) -> Result<(), IdentityError> {
    let enabled: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM second_steps \
         WHERE user_id = $1 AND confirmed_at IS NOT NULL)",
    )
    .bind(user)
    .fetch_one(&mut **tx)
    .await?;
    if enabled {
        Ok(())
    } else {
        Err(IdentityError::SecondStepOff)
    }
}

async fn delete_second_step(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user: Uuid,
) -> Result<(), IdentityError> {
    sqlx::query("DELETE FROM second_steps WHERE user_id = $1")
        .bind(user)
        .execute(&mut **tx)
        .await?;
    sqlx::query("DELETE FROM recovery_codes WHERE user_id = $1")
        .bind(user)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

async fn replace_recovery_codes(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user: Uuid,
    now: DateTime<Utc>,
) -> Result<RecoveryCodes, IdentityError> {
    sqlx::query("DELETE FROM recovery_codes WHERE user_id = $1")
        .bind(user)
        .execute(&mut **tx)
        .await?;
    let mut codes = Vec::with_capacity(RECOVERY_CODE_COUNT);
    while codes.len() < RECOVERY_CODE_COUNT {
        let code = new_recovery_code()?;
        if codes.contains(&code) {
            continue;
        }
        sqlx::query(
            "INSERT INTO recovery_codes (user_id, code_hash, created_at) VALUES ($1, $2, $3)",
        )
        .bind(user)
        .bind(recovery_hash(user, &code))
        .bind(now)
        .execute(&mut **tx)
        .await?;
        codes.push(code);
    }
    Ok(RecoveryCodes(codes))
}

/// Spends a recovery code; returns how many are left, or `None` when the code
/// is unknown or already used.
async fn use_recovery_code(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user: Uuid,
    code: &str,
    now: DateTime<Utc>,
) -> Result<Option<i64>, IdentityError> {
    let Some(code) = normalize_recovery_code(code) else {
        return Ok(None);
    };
    let used = sqlx::query(
        "UPDATE recovery_codes SET used_at = $3 \
         WHERE user_id = $1 AND code_hash = $2 AND used_at IS NULL",
    )
    .bind(user)
    .bind(recovery_hash(user, &code))
    .bind(now)
    .execute(&mut **tx)
    .await?
    .rows_affected();
    if used == 0 {
        return Ok(None);
    }
    let left: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM recovery_codes WHERE user_id = $1 AND used_at IS NULL",
    )
    .bind(user)
    .fetch_one(&mut **tx)
    .await?;
    Ok(Some(left))
}

fn new_recovery_code() -> Result<String, IdentityError> {
    let mut bytes = [0u8; RECOVERY_CODE_CHARS];
    getrandom::fill(&mut bytes)
        .map_err(|_| IdentityError::Internal("the random number generator failed".into()))?;
    // 256 is a multiple of the 32-letter alphabet, so every letter is equally likely.
    let letters: Vec<char> = bytes
        .iter()
        .map(|byte| char::from(RECOVERY_ALPHABET[usize::from(*byte) % RECOVERY_ALPHABET.len()]))
        .collect();
    Ok(letters
        .chunks(4)
        .map(|chunk| chunk.iter().collect::<String>())
        .collect::<Vec<_>>()
        .join("-"))
}

/// Upper-case with the dashes put back, so `abcd efgh-jkmn` matches `ABCD-EFGH-JKMN`.
fn normalize_recovery_code(code: &str) -> Option<String> {
    let letters: Vec<char> = code
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_uppercase())
        .collect();
    (letters.len() == RECOVERY_CODE_CHARS).then(|| {
        letters
            .chunks(4)
            .map(|chunk| chunk.iter().collect::<String>())
            .collect::<Vec<_>>()
            .join("-")
    })
}

fn recovery_hash(user: Uuid, code: &str) -> Vec<u8> {
    let mut hasher = Sha256::new();
    hasher.update(user.as_bytes());
    hasher.update(code.as_bytes());
    hasher.finalize().to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recovery_codes_are_twelve_letters_in_three_groups() {
        let code = new_recovery_code().unwrap();
        assert_eq!(code.len(), 14);
        assert_eq!(code.matches('-').count(), 2);
        assert!(code
            .chars()
            .filter(|c| *c != '-')
            .all(|c| RECOVERY_ALPHABET.contains(&(c as u8))));
        assert_ne!(code, new_recovery_code().unwrap());
    }

    #[test]
    fn recovery_codes_are_read_loosely() {
        assert_eq!(
            normalize_recovery_code(" abcd efgh-jkmn ").as_deref(),
            Some("ABCD-EFGH-JKMN")
        );
        assert_eq!(normalize_recovery_code("123456"), None);
    }

    #[test]
    fn a_recovery_hash_depends_on_the_user() {
        let code = "ABCD-EFGH-JKMN";
        assert_ne!(
            recovery_hash(Uuid::new_v4(), code),
            recovery_hash(Uuid::new_v4(), code)
        );
    }

    #[test]
    fn codes_follow_rfc_6238() {
        // RFC 6238 appendix B, SHA-1 secret, truncated to six digits.
        let totp = totp(b"12345678901234567890".to_vec(), "ada@bank.example").unwrap();
        assert_eq!(totp.generate(59).to_string(), "287082");
        assert_eq!(totp.generate(1_111_111_109).to_string(), "081804");
        assert_eq!(totp.check("287082", 59 + 30), Some(1));
        assert_eq!(totp.check("287082", 59 + 90), None);
        assert!(totp
            .to_url()
            .unwrap()
            .starts_with("otpauth://totp/Donka:ada%40bank.example?"));
    }
}
