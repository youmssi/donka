//! Identity module: Studio users, sign-in with lockout, sessions, invitations,
//! password reset and the one-time links they rely on.
//!
//! Other modules use it only through [`Identity`] and the types exported here.

mod emails;
mod secret;

use chrono::{DateTime, Duration, Utc};
use donka_audit::{Action, Event};
use donka_db::PgPool;
use donka_mail::{Email, Mailer};
use donka_shared::clock::Clock;
use donka_shared::page::{Page, PageRequest};
use emails::Kind;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::Acquire;
use std::sync::Arc;
use tokio::sync::Notify;
use uuid::Uuid;

pub use secret::{MAX_PASSWORD_CHARS, MIN_PASSWORD_CHARS};

/// Avoid a database write on every request: `last_seen_at` moves at most this often.
const TOUCH_INTERVAL_SECONDS: i64 = 60;
/// Emails handled per call to [`Identity::deliver_due_emails`].
const EMAIL_BATCH: usize = 20;
/// Retry delays double from this value after each failed send...
const EMAIL_RETRY_BASE_SECONDS: i64 = 30;
/// ...up to this ceiling.
const EMAIL_RETRY_MAX_SECONDS: i64 = 3600;
/// Page of the web app (under the locale segment) that reads the token and
/// asks for the new password.
const SETUP_PAGE: &str = "setup-password";

#[derive(Debug, Clone)]
pub struct Policy {
    /// A session ends after this long without activity.
    pub session_idle_timeout: Duration,
    /// Failed sign-ins allowed within `failure_window` before the account locks.
    pub max_failed_sign_ins: u32,
    pub failure_window: Duration,
    pub lock_duration: Duration,
    /// How long an invitation link (and the first administrator's link) works.
    pub invitation_link_lifetime: Duration,
    /// How long a password-reset link works.
    pub reset_link_lifetime: Duration,
    /// Failed sends after which an account email is abandoned.
    pub email_max_attempts: u32,
}

impl Default for Policy {
    fn default() -> Self {
        Self {
            session_idle_timeout: Duration::hours(8),
            max_failed_sign_ins: 5,
            failure_window: Duration::minutes(15),
            lock_duration: Duration::minutes(15),
            invitation_link_lifetime: Duration::hours(72),
            reset_link_lifetime: Duration::minutes(30),
            email_max_attempts: 10,
        }
    }
}

/// Language of a user's emails and screens.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[serde(rename_all = "lowercase")]
#[sqlx(type_name = "text", rename_all = "lowercase")]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub enum Locale {
    En,
    Fr,
}

impl Locale {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::En => "en",
            Self::Fr => "fr",
        }
    }
}

impl std::str::FromStr for Locale {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "en" => Ok(Self::En),
            "fr" => Ok(Self::Fr),
            other => Err(format!(
                "unsupported language '{other}' (expected en or fr)"
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, sqlx::FromRow)]
pub struct User {
    pub id: Uuid,
    pub email: String,
    pub is_admin: bool,
    pub locale: Locale,
}

/// A Studio account as administrators see it.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct Account {
    pub id: Uuid,
    pub email: String,
    pub is_admin: bool,
    pub locale: Locale,
    /// False until the person has chosen a password (an invitation is pending).
    pub active: bool,
    pub created_at: DateTime<Utc>,
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
    #[error("only administrators can do this")]
    Forbidden,
    #[error("a user with this email already exists")]
    EmailTaken,
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
    /// Wakes the email worker when a transaction has queued an email.
    outbox: Arc<Notify>,
}

impl Identity {
    pub fn new(pool: PgPool, clock: Arc<dyn Clock>, policy: Policy) -> Self {
        Self {
            pool,
            clock,
            policy,
            outbox: Arc::new(Notify::new()),
        }
    }

    /// Resolves when an email has been queued since the last call; the email
    /// worker waits on it between polls.
    pub async fn email_queued(&self) {
        self.outbox.notified().await;
    }

    /// Creates the first administrator, without a password, when no user exists
    /// yet, and returns a one-time password-setup token. Returns `None` when
    /// users already exist, so it is safe to call on every start.
    pub async fn bootstrap_admin(
        &self,
        email: &str,
        locale: Locale,
    ) -> Result<Option<IssuedToken>, IdentityError> {
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
            "INSERT INTO users (id, email, is_admin, locale, created_at) \
             VALUES ($1, $2, true, $3, $4)",
        )
        .bind(user_id)
        .bind(&email)
        .bind(locale)
        .bind(now)
        .execute(&mut *tx)
        .await?;
        let token = self
            .issue_setup_token(&mut tx, user_id, now, self.policy.invitation_link_lifetime)
            .await?;
        donka_audit::record(
            &mut tx,
            Event::new(now, None, Action::UserInvited)
                .about_user(user_id)
                .with_details(json!({ "isAdmin": true, "firstAdministrator": true })),
        )
        .await?;
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
        // A new password ends every existing session of that user...
        sqlx::query("DELETE FROM sessions WHERE user_id = $1")
            .bind(user_id)
            .execute(&mut *tx)
            .await?;
        // ...and every other link still waiting in their inbox.
        revoke_setup_tokens(&mut tx, user_id, now).await?;
        donka_audit::record(
            &mut tx,
            Event::new(now, Some(user_id), Action::UserPasswordSet),
        )
        .await?;
        tx.commit().await?;
        Ok(())
    }

    /// Invites a person: creates their account without a password and queues
    /// an email with a one-time link to choose it. Inviting someone who has
    /// not accepted yet sends a fresh link and revokes the previous one.
    pub async fn invite(
        &self,
        inviter: &User,
        email: &str,
        locale: Locale,
        is_admin: bool,
    ) -> Result<User, IdentityError> {
        if !inviter.is_admin {
            return Err(IdentityError::Forbidden);
        }
        let email = normalize_email(email)?;
        let now = self.clock.now();
        let mut tx = self.pool.begin().await?;
        let existing: Option<(Uuid, bool)> = sqlx::query_as(
            "SELECT id, password_hash IS NOT NULL FROM users WHERE email = $1 FOR UPDATE",
        )
        .bind(&email)
        .fetch_optional(&mut *tx)
        .await?;
        let user_id = match existing {
            Some((_, true)) => return Err(IdentityError::EmailTaken),
            Some((id, false)) => {
                sqlx::query("UPDATE users SET is_admin = $2, locale = $3 WHERE id = $1")
                    .bind(id)
                    .bind(is_admin)
                    .bind(locale)
                    .execute(&mut *tx)
                    .await?;
                revoke_setup_tokens(&mut tx, id, now).await?;
                id
            }
            None => {
                let id = Uuid::new_v4();
                sqlx::query(
                    "INSERT INTO users (id, email, is_admin, locale, created_at) \
                     VALUES ($1, $2, $3, $4, $5)",
                )
                .bind(id)
                .bind(&email)
                .bind(is_admin)
                .bind(locale)
                .bind(now)
                .execute(&mut *tx)
                .await
                .map_err(|err| match err {
                    // Invited by someone else at the same moment.
                    sqlx::Error::Database(db) if db.is_unique_violation() => {
                        IdentityError::EmailTaken
                    }
                    other => other.into(),
                })?;
                id
            }
        };
        queue_email(&mut tx, user_id, Kind::Invitation, now).await?;
        donka_audit::record(
            &mut tx,
            Event::new(now, Some(inviter.id), Action::UserInvited)
                .about_user(user_id)
                .with_details(
                    json!({ "isAdmin": is_admin, "locale": locale, "resent": existing.is_some() }),
                ),
        )
        .await?;
        tx.commit().await?;
        self.outbox.notify_one();
        tracing::info!(inviter = %inviter.id, user_id = %user_id, "user invited");
        Ok(User {
            id: user_id,
            email,
            is_admin,
            locale,
        })
    }

    /// Queues a password-reset email when the address belongs to a user.
    /// Known and unknown addresses take the same single statement and give the
    /// same answer, so the caller cannot tell which accounts exist.
    pub async fn request_password_reset(&self, email: &str) -> Result<(), IdentityError> {
        let email = normalize_email(email)?;
        let now = self.clock.now();
        // ON CONFLICT: a reset already waiting to be sent is not queued twice.
        sqlx::query(
            "INSERT INTO account_emails (id, user_id, kind, created_at, next_attempt_at) \
             SELECT $1, id, $2, $3, $3 FROM users WHERE email = $4 \
             ON CONFLICT DO NOTHING",
        )
        .bind(Uuid::new_v4())
        .bind(Kind::PasswordReset.as_str())
        .bind(now)
        .bind(&email)
        .execute(&self.pool)
        .await?;
        self.outbox.notify_one();
        Ok(())
    }

    /// Sends the account emails that are due, each in its own transaction, and
    /// returns how many were sent. Safe to run from several instances at once.
    ///
    /// The one-time link is created at send time, inside a savepoint that is
    /// rolled back if the send fails: a working link never exists without an
    /// email that carries it, and no link is ever stored in clear.
    pub async fn deliver_due_emails(
        &self,
        mailer: &dyn Mailer,
        public_url: &str,
    ) -> Result<usize, IdentityError> {
        let mut sent = 0;
        for _ in 0..EMAIL_BATCH {
            let now = self.clock.now();
            let mut tx = self.pool.begin().await?;
            let due: Option<DueEmail> = sqlx::query_as(
                "SELECT e.id, e.kind, e.attempts, e.user_id, e.created_at, u.email, u.locale \
                 FROM account_emails e JOIN users u ON u.id = e.user_id \
                 WHERE e.sent_at IS NULL AND e.abandoned_at IS NULL AND e.next_attempt_at <= $1 \
                 ORDER BY e.next_attempt_at LIMIT 1 \
                 FOR UPDATE OF e SKIP LOCKED",
            )
            .bind(now)
            .fetch_optional(&mut *tx)
            .await?;
            let Some(due) = due else { break };
            let kind = Kind::parse(&due.kind).ok_or_else(|| {
                IdentityError::Internal(format!("unknown email kind {}", due.kind))
            })?;
            let lifetime = match kind {
                Kind::Invitation => self.policy.invitation_link_lifetime,
                Kind::PasswordReset => self.policy.reset_link_lifetime,
            };

            let mut attempt = tx.begin().await?;
            let token = self
                .issue_setup_token(&mut attempt, due.user_id, now, lifetime)
                .await?;
            let link = setup_link(public_url, due.locale, &token);
            let content = emails::render(kind, due.locale, &link, lifetime);
            let email = Email {
                to: due.email,
                subject: content.subject,
                text: content.text,
            };
            let attempts = due.attempts.saturating_add(1);
            match mailer.send(&email).await {
                Ok(()) => {
                    attempt.commit().await?;
                    sqlx::query(
                        "UPDATE account_emails SET sent_at = $2, attempts = $3, last_error = NULL \
                         WHERE id = $1",
                    )
                    .bind(due.id)
                    .bind(now)
                    .bind(attempts)
                    .execute(&mut *tx)
                    .await?;
                    if kind == Kind::PasswordReset {
                        record_reset_request(&mut tx, due.user_id, due.created_at, true).await?;
                    }
                    sent += 1;
                }
                Err(err) => {
                    attempt.rollback().await?;
                    let max = i32::try_from(self.policy.email_max_attempts).unwrap_or(i32::MAX);
                    let abandoned_at = (err.is_permanent() || attempts >= max).then_some(now);
                    if abandoned_at.is_some() {
                        tracing::error!(email_id = %due.id, kind = kind.as_str(), %err, attempts, "account email abandoned");
                    } else {
                        tracing::warn!(email_id = %due.id, kind = kind.as_str(), %err, attempts, "account email not sent; will retry");
                    }
                    sqlx::query(
                        "UPDATE account_emails SET attempts = $2, last_error = $3, \
                         next_attempt_at = $4, abandoned_at = $5 WHERE id = $1",
                    )
                    .bind(due.id)
                    .bind(attempts)
                    .bind(err.to_string())
                    .bind(now + retry_delay(attempts))
                    .bind(abandoned_at)
                    .execute(&mut *tx)
                    .await?;
                    if kind == Kind::PasswordReset && abandoned_at.is_some() {
                        record_reset_request(&mut tx, due.user_id, due.created_at, false).await?;
                    }
                }
            }
            tx.commit().await?;
        }
        Ok(sent)
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
            "SELECT id, email, is_admin, locale, password_hash, failed_sign_ins, \
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
        donka_audit::record(
            &mut tx,
            Event::new(now, Some(account.id), Action::UserSignedIn),
        )
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
            "SELECT u.id, u.email, u.is_admin, u.locale, s.last_seen_at \
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
            locale: session.locale,
        })
    }

    /// Every Studio account, by email. Administrators only.
    pub async fn list_accounts(
        &self,
        reader: &User,
        page: PageRequest,
    ) -> Result<Page<Account>, IdentityError> {
        if !reader.is_admin {
            return Err(IdentityError::Forbidden);
        }
        let items = sqlx::query_as(
            "SELECT id, email, is_admin, locale, password_hash IS NOT NULL AS active, created_at \
             FROM users ORDER BY email LIMIT $1 OFFSET $2",
        )
        .bind(page.limit)
        .bind(page.offset)
        .fetch_all(&self.pool)
        .await?;
        let (total,): (i64,) = sqlx::query_as("SELECT count(*) FROM users")
            .fetch_one(&self.pool)
            .await?;
        Ok(Page { items, total })
    }

    /// The user with this email, if any. For callers that are already
    /// authorized to know (e.g. a project owner adding a member).
    pub async fn find_by_email(&self, email: &str) -> Result<Option<User>, IdentityError> {
        let email = normalize_email(email)?;
        Ok(
            sqlx::query_as("SELECT id, email, is_admin, locale FROM users WHERE email = $1")
                .bind(email)
                .fetch_optional(&self.pool)
                .await?,
        )
    }

    /// The users with these ids, in no particular order; unknown ids are skipped.
    pub async fn users_by_ids(&self, ids: &[Uuid]) -> Result<Vec<User>, IdentityError> {
        Ok(
            sqlx::query_as("SELECT id, email, is_admin, locale FROM users WHERE id = ANY($1)")
                .bind(ids)
                .fetch_all(&self.pool)
                .await?,
        )
    }

    /// Ends the session server-side. Unknown tokens are ignored.
    pub async fn sign_out(&self, token: &str) -> Result<(), IdentityError> {
        let mut tx = self.pool.begin().await?;
        let ended: Option<(Uuid,)> =
            sqlx::query_as("DELETE FROM sessions WHERE token_hash = $1 RETURNING user_id")
                .bind(secret::hash_token(token))
                .fetch_optional(&mut *tx)
                .await?;
        if let Some((user_id,)) = ended {
            donka_audit::record(
                &mut tx,
                Event::new(self.clock.now(), Some(user_id), Action::UserSignedOut),
            )
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    async fn issue_setup_token(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        user_id: Uuid,
        now: DateTime<Utc>,
        lifetime: Duration,
    ) -> Result<IssuedToken, IdentityError> {
        let (token, token_hash) = secret::new_token()?;
        sqlx::query(
            "INSERT INTO password_setup_tokens (token_hash, user_id, created_at, expires_at) \
             VALUES ($1, $2, $3, $4)",
        )
        .bind(token_hash)
        .bind(user_id)
        .bind(now)
        .bind(now + lifetime)
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
        let locks = failures >= max;
        // Nobody is signed in yet: the event is about the account, with no actor.
        donka_audit::record(
            tx,
            Event::new(now, None, Action::UserSignInFailed)
                .about_user(account.id)
                .with_details(json!({ "locked": locks })),
        )
        .await?;
        if locks {
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
    locale: Locale,
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
            locale: self.locale,
        }
    }
}

#[derive(sqlx::FromRow)]
struct SessionRow {
    id: Uuid,
    email: String,
    is_admin: bool,
    locale: Locale,
    last_seen_at: DateTime<Utc>,
}

#[derive(sqlx::FromRow)]
struct DueEmail {
    id: Uuid,
    kind: String,
    attempts: i32,
    user_id: Uuid,
    created_at: DateTime<Utc>,
    email: String,
    locale: Locale,
}

/// A reset request is recorded when its email reaches a final state (sent or
/// abandoned), not when it is asked for: the request itself must take the same
/// path for known and unknown addresses (see `request_password_reset`).
async fn record_reset_request(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user_id: Uuid,
    requested_at: DateTime<Utc>,
    delivered: bool,
) -> Result<(), IdentityError> {
    donka_audit::record(
        tx,
        Event::new(requested_at, None, Action::UserPasswordResetRequested)
            .about_user(user_id)
            .with_details(json!({ "delivered": delivered })),
    )
    .await?;
    Ok(())
}

async fn queue_email(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user_id: Uuid,
    kind: Kind,
    now: DateTime<Utc>,
) -> Result<(), IdentityError> {
    // ON CONFLICT: an email of this kind already waiting covers this request.
    sqlx::query(
        "INSERT INTO account_emails (id, user_id, kind, created_at, next_attempt_at) \
         VALUES ($1, $2, $3, $4, $4) ON CONFLICT DO NOTHING",
    )
    .bind(Uuid::new_v4())
    .bind(user_id)
    .bind(kind.as_str())
    .bind(now)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// Marks the user's unused links as spent, so only the newest one works.
async fn revoke_setup_tokens(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user_id: Uuid,
    now: DateTime<Utc>,
) -> Result<(), IdentityError> {
    sqlx::query(
        "UPDATE password_setup_tokens SET used_at = $2 WHERE user_id = $1 AND used_at IS NULL",
    )
    .bind(user_id)
    .bind(now)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// The link a person opens to choose their password, in their language.
pub fn setup_link(public_url: &str, locale: Locale, token: &IssuedToken) -> String {
    format!(
        "{}/{}/{SETUP_PAGE}/?token={}",
        public_url.trim_end_matches('/'),
        locale.as_str(),
        token.expose()
    )
}

/// 30 s, 1 min, 2 min… capped at one hour.
fn retry_delay(attempts: i32) -> Duration {
    let doublings = u32::try_from(attempts.saturating_sub(1))
        .unwrap_or(0)
        .min(16);
    Duration::seconds((EMAIL_RETRY_BASE_SECONDS << doublings).min(EMAIL_RETRY_MAX_SECONDS))
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
    fn retries_back_off_exponentially_up_to_an_hour() {
        assert_eq!(retry_delay(1), Duration::seconds(30));
        assert_eq!(retry_delay(2), Duration::seconds(60));
        assert_eq!(retry_delay(5), Duration::seconds(480));
        assert_eq!(retry_delay(9), Duration::hours(1));
        assert_eq!(retry_delay(i32::MAX), Duration::hours(1));
    }

    #[test]
    fn locales_parse_from_their_codes() {
        assert_eq!("fr".parse::<Locale>().unwrap(), Locale::Fr);
        assert_eq!(Locale::En.as_str(), "en");
        assert!("de".parse::<Locale>().is_err());
    }

    #[test]
    fn issued_tokens_do_not_print_their_value() {
        let token = IssuedToken("secret-value".into());
        assert_eq!(format!("{token:?}"), "IssuedToken(***)");
    }
}
