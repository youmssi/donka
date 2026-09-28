//! Identity module: Studio users, sign-in with lockout, sessions and one-time
//! password-setup links.
//!
//! Other modules use it only through [`Identity`] and the types exported here.

pub mod clock;
mod secret;

use chrono::{DateTime, Duration, Utc};
use clock::Clock;
use donka_db::PgPool;
use serde::Serialize;
use std::sync::Arc;
use uuid::Uuid;

pub use secret::{MAX_PASSWORD_CHARS, MIN_PASSWORD_CHARS};

/// Avoid a database write on every request: `last_seen_at` moves at most this often.
const TOUCH_INTERVAL_SECONDS: i64 = 60;

#[derive(Debug, Clone)]
pub struct Policy {
    /// A session ends after this long without activity.
    pub session_idle_timeout: Duration,
    /// Failed sign-ins allowed within `failure_window` before the account locks.
    pub max_failed_sign_ins: u32,
    pub failure_window: Duration,
    pub lock_duration: Duration,
    pub setup_link_lifetime: Duration,
}

impl Default for Policy {
    fn default() -> Self {
        Self {
            session_idle_timeout: Duration::hours(8),
            max_failed_sign_ins: 5,
            failure_window: Duration::minutes(15),
            lock_duration: Duration::minutes(15),
            setup_link_lifetime: Duration::hours(24),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct User {
    pub id: Uuid,
    pub email: String,
    pub is_admin: bool,
}

/// A secret handed to the browser (session cookie) or to a person (setup link).
/// Only its hash is stored. Debug output never shows the value.
pub struct IssuedToken(String);

impl IssuedToken {
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for IssuedToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("IssuedToken(***)")
    }
}

#[derive(Debug, thiserror::Error)]
pub enum IdentityError {
    /// Wrong email or password, unknown account, account without a password,
    /// or locked account: deliberately indistinguishable to the caller.
    #[error("invalid credentials")]
    InvalidCredentials,
    #[error("no valid session")]
    Unauthenticated,
    #[error("the password-setup link is invalid, used or expired")]
    InvalidSetupLink,
    #[error("password must be between {MIN_PASSWORD_CHARS} and {MAX_PASSWORD_CHARS} characters")]
    WeakPassword,
    #[error("not a valid email address")]
    InvalidEmail,
    #[error(transparent)]
    Database(#[from] sqlx::Error),
    #[error("internal error: {0}")]
    Internal(String),
}

impl From<secret::SecretError> for IdentityError {
    fn from(err: secret::SecretError) -> Self {
        Self::Internal(err.to_string())
    }
}

#[derive(Clone)]
pub struct Identity {
    pool: PgPool,
    clock: Arc<dyn Clock>,
    policy: Policy,
}

impl Identity {
    pub fn new(pool: PgPool, clock: Arc<dyn Clock>, policy: Policy) -> Self {
        Self {
            pool,
            clock,
            policy,
        }
    }

    /// Creates the first administrator, without a password, when no user exists
    /// yet, and returns a one-time password-setup token. Returns `None` when
    /// users already exist, so it is safe to call on every start.
    pub async fn bootstrap_admin(&self, email: &str) -> Result<Option<IssuedToken>, IdentityError> {
        let email = normalize_email(email)?;
        let now = self.clock.now();
        let mut tx = self.pool.begin().await?;
        // Serializes concurrent starts of several instances.
        sqlx::query("LOCK TABLE users IN EXCLUSIVE MODE")
            .execute(&mut *tx)
            .await?;
        let (existing,): (i64,) = sqlx::query_as("SELECT count(*) FROM users")
            .fetch_one(&mut *tx)
            .await?;
        if existing > 0 {
            return Ok(None);
        }
        let user_id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO users (id, email, is_admin, created_at) VALUES ($1, $2, true, $3)",
        )
        .bind(user_id)
        .bind(&email)
        .bind(now)
        .execute(&mut *tx)
        .await?;
        let token = self.issue_setup_token(&mut tx, user_id, now).await?;
        tx.commit().await?;
        Ok(Some(token))
    }

    /// Sets the password of the user the link belongs to. The link works once.
    pub async fn complete_password_setup(
        &self,
        token: &str,
        password: &str,
    ) -> Result<(), IdentityError> {
        check_password_rules(password)?;
        let now = self.clock.now();
        let hash = hash_off_thread(password.to_owned()).await?;
        let mut tx = self.pool.begin().await?;
        let user_id: Option<(Uuid,)> = sqlx::query_as(
            "UPDATE password_setup_tokens SET used_at = $2 \
             WHERE token_hash = $1 AND used_at IS NULL AND expires_at > $2 \
             RETURNING user_id",
        )
        .bind(secret::hash_token(token))
        .bind(now)
        .fetch_optional(&mut *tx)
        .await?;
        let (user_id,) = user_id.ok_or(IdentityError::InvalidSetupLink)?;
        sqlx::query(
            "UPDATE users SET password_hash = $2, failed_sign_ins = 0, \
             failed_window_started_at = NULL, locked_until = NULL WHERE id = $1",
        )
        .bind(user_id)
        .bind(hash)
        .execute(&mut *tx)
        .await?;
        // A new password ends every existing session of that user.
        sqlx::query("DELETE FROM sessions WHERE user_id = $1")
            .bind(user_id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }

    /// Checks the credentials and opens a session. Every failure, including a
    /// locked or unknown account, is reported as `InvalidCredentials` after the
    /// same amount of work.
    pub async fn sign_in(
        &self,
        email: &str,
        password: &str,
    ) -> Result<(IssuedToken, User), IdentityError> {
        let Ok(email) = normalize_email(email) else {
            verify_off_thread(password.to_owned(), None).await?;
            return Err(IdentityError::InvalidCredentials);
        };
        let now = self.clock.now();
        let mut tx = self.pool.begin().await?;
        // FOR UPDATE: concurrent failures for one account are counted one after another.
        let row: Option<AccountRow> = sqlx::query_as(
            "SELECT id, email, is_admin, password_hash, failed_sign_ins, \
                    failed_window_started_at, locked_until \
             FROM users WHERE email = $1 FOR UPDATE",
        )
        .bind(&email)
        .fetch_optional(&mut *tx)
        .await?;

        let Some(account) = row else {
            verify_off_thread(password.to_owned(), None).await?;
            return Err(IdentityError::InvalidCredentials);
        };
        if account.locked_until.is_some_and(|until| until > now) {
            verify_off_thread(password.to_owned(), None).await?;
            tracing::warn!(user_id = %account.id, "sign-in refused: account is locked");
            return Err(IdentityError::InvalidCredentials);
        }

        let matches = verify_off_thread(password.to_owned(), account.password_hash.clone()).await?;
        if !matches {
            self.record_failure(&mut tx, &account, now).await?;
            tx.commit().await?;
            return Err(IdentityError::InvalidCredentials);
        }

        sqlx::query(
            "UPDATE users SET failed_sign_ins = 0, failed_window_started_at = NULL, \
             locked_until = NULL WHERE id = $1",
        )
        .bind(account.id)
        .execute(&mut *tx)
        .await?;
        let (token, token_hash) = secret::new_token()?;
        sqlx::query(
            "INSERT INTO sessions (token_hash, user_id, created_at, last_seen_at) \
             VALUES ($1, $2, $3, $3)",
        )
        .bind(token_hash)
        .bind(account.id)
        .bind(now)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok((IssuedToken(token), account.into_user()))
    }

    /// Resolves a session token to its user, ending sessions idle for longer
    /// than the policy allows.
    pub async fn authenticate(&self, token: &str) -> Result<User, IdentityError> {
        let now = self.clock.now();
        let token_hash = secret::hash_token(token);
        let row: Option<SessionRow> = sqlx::query_as(
            "SELECT u.id, u.email, u.is_admin, s.last_seen_at \
             FROM sessions s JOIN users u ON u.id = s.user_id WHERE s.token_hash = $1",
        )
        .bind(&token_hash)
        .fetch_optional(&self.pool)
        .await?;
        let Some(session) = row else {
            return Err(IdentityError::Unauthenticated);
        };
        let idle = now - session.last_seen_at;
        if idle > self.policy.session_idle_timeout {
            sqlx::query("DELETE FROM sessions WHERE token_hash = $1")
                .bind(&token_hash)
                .execute(&self.pool)
                .await?;
            return Err(IdentityError::Unauthenticated);
        }
        if idle > Duration::seconds(TOUCH_INTERVAL_SECONDS) {
            sqlx::query("UPDATE sessions SET last_seen_at = $2 WHERE token_hash = $1")
                .bind(&token_hash)
                .bind(now)
                .execute(&self.pool)
                .await?;
        }
        Ok(User {
            id: session.id,
            email: session.email,
            is_admin: session.is_admin,
        })
    }

    /// Ends the session server-side. Unknown tokens are ignored.
    pub async fn sign_out(&self, token: &str) -> Result<(), IdentityError> {
        sqlx::query("DELETE FROM sessions WHERE token_hash = $1")
            .bind(secret::hash_token(token))
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn issue_setup_token(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        user_id: Uuid,
        now: DateTime<Utc>,
    ) -> Result<IssuedToken, IdentityError> {
        let (token, token_hash) = secret::new_token()?;
        sqlx::query(
            "INSERT INTO password_setup_tokens (token_hash, user_id, created_at, expires_at) \
             VALUES ($1, $2, $3, $4)",
        )
        .bind(token_hash)
        .bind(user_id)
        .bind(now)
        .bind(now + self.policy.setup_link_lifetime)
        .execute(&mut **tx)
        .await?;
        Ok(IssuedToken(token))
    }

    async fn record_failure(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        account: &AccountRow,
        now: DateTime<Utc>,
    ) -> Result<(), IdentityError> {
        let window_open = account
            .failed_window_started_at
            .is_some_and(|start| now - start <= self.policy.failure_window);
        let (failures, window_start) = if window_open {
            (
                account.failed_sign_ins + 1,
                account.failed_window_started_at,
            )
        } else {
            (1, Some(now))
        };
        let max = i32::try_from(self.policy.max_failed_sign_ins).unwrap_or(i32::MAX);
        if failures >= max {
            tracing::warn!(user_id = %account.id, "account locked after repeated failed sign-ins");
            sqlx::query(
                "UPDATE users SET failed_sign_ins = 0, failed_window_started_at = NULL, \
                 locked_until = $2 WHERE id = $1",
            )
            .bind(account.id)
            .bind(now + self.policy.lock_duration)
            .execute(&mut **tx)
            .await?;
        } else {
            sqlx::query(
                "UPDATE users SET failed_sign_ins = $2, failed_window_started_at = $3 WHERE id = $1",
            )
            .bind(account.id)
            .bind(failures)
            .bind(window_start)
            .execute(&mut **tx)
            .await?;
        }
        Ok(())
    }
}

#[derive(sqlx::FromRow)]
struct AccountRow {
    id: Uuid,
    email: String,
    is_admin: bool,
    password_hash: Option<String>,
    failed_sign_ins: i32,
    failed_window_started_at: Option<DateTime<Utc>>,
    locked_until: Option<DateTime<Utc>>,
}

impl AccountRow {
    fn into_user(self) -> User {
        User {
            id: self.id,
            email: self.email,
            is_admin: self.is_admin,
        }
    }
}

#[derive(sqlx::FromRow)]
struct SessionRow {
    id: Uuid,
    email: String,
    is_admin: bool,
    last_seen_at: DateTime<Utc>,
}

/// Emails are compared case-insensitively and stored lower-cased.
fn normalize_email(email: &str) -> Result<String, IdentityError> {
    let email = email.trim().to_lowercase();
    let well_formed = email.len() <= 254
        && !email.chars().any(char::is_whitespace)
        && email
            .split_once('@')
            .is_some_and(|(local, domain)| !local.is_empty() && domain.contains('.'));
    if well_formed {
        Ok(email)
    } else {
        Err(IdentityError::InvalidEmail)
    }
}

fn check_password_rules(password: &str) -> Result<(), IdentityError> {
    let chars = password.chars().count();
    if (MIN_PASSWORD_CHARS..=MAX_PASSWORD_CHARS).contains(&chars) {
        Ok(())
    } else {
        Err(IdentityError::WeakPassword)
    }
}

async fn hash_off_thread(password: String) -> Result<String, IdentityError> {
    tokio::task::spawn_blocking(move || secret::hash_password(&password))
        .await
        .map_err(|err| IdentityError::Internal(err.to_string()))?
        .map_err(Into::into)
}

async fn verify_off_thread(
    password: String,
    stored: Option<String>,
) -> Result<bool, IdentityError> {
    // Very long input is refused before argon2 runs, so it cannot be used to burn CPU.
    if password.chars().count() > MAX_PASSWORD_CHARS {
        return Ok(false);
    }
    tokio::task::spawn_blocking(move || secret::verify_password(&password, stored.as_deref()))
        .await
        .map_err(|err| IdentityError::Internal(err.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emails_are_normalized_and_validated() {
        assert_eq!(
            normalize_email("  Ada@Bank.EXAMPLE ").unwrap(),
            "ada@bank.example"
        );
        for bad in [
            "",
            "ada",
            "ada@",
            "@bank.example",
            "ada@bank",
            "a da@bank.example",
        ] {
            assert!(normalize_email(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn password_length_rules() {
        assert!(check_password_rules("short").is_err());
        assert!(check_password_rules(&"x".repeat(12)).is_ok());
        assert!(check_password_rules(&"x".repeat(128)).is_ok());
        assert!(check_password_rules(&"x".repeat(129)).is_err());
    }

    #[test]
    fn issued_tokens_do_not_print_their_value() {
        let token = IssuedToken("secret-value".into());
        assert_eq!(format!("{token:?}"), "IssuedToken(***)");
    }
}
