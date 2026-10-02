//! Decision-log module: every decision a Runtime makes, as the Runtime sends
//! it (DNK-18).
//!
//! A Runtime sends records with a decision-log token an administrator issued
//! for its environment ([`DecisionLog::receive`]). A record is stored only
//! when its release was published to that environment of its project. What
//! can be searched (decision, reference, outcome, time) is stored as is; what
//! the decision read and answered is encrypted ([`cipher`]). Members search
//! the log; opening or replaying a record is audited, since it holds
//! applicants' personal data. Replay re-evaluates a record with its release,
//! connector nodes answering with what they answered at the time. Records
//! older than the retention period are purged, and each purge is audited.

use chrono::{DateTime, Duration, Utc};
use donka_audit::{Action, Event};
use donka_db::PgPool;
use donka_engine::{Bundle, DecisionRuntime, EvaluateOptions, RuntimeError};
use donka_identity::User;
use donka_project::{Access, ProjectError, Role};
use donka_release::{Environment, ReleaseError, Releases};
use donka_shared::clock::Clock;
use donka_shared::page::{Page, PageRequest};
use donka_shared::secret::{self, IssuedSecret};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

pub mod cipher;
mod feed;

pub use cipher::{Cipher, CipherError};
use feed::FeedRecord;
pub use feed::{Payload, Rejection, Status, MAX_TEXT_CHARS};

/// Records one batch may hold; the Runtime sends 100 by default.
pub const MAX_BATCH_RECORDS: usize = 1000;
pub const MAX_TOKEN_NAME_CHARS: usize = 100;
/// Every decision-log token starts with this, so a leaked one is easy to recognise.
const TOKEN_PREFIX: &str = "dnk_log_";

#[derive(Debug, thiserror::Error)]
pub enum DecisionLogError {
    #[error("only administrators can do this")]
    NotAdministrator,
    #[error("token names are 1 to {MAX_TOKEN_NAME_CHARS} characters")]
    InvalidTokenName,
    #[error("token not found")]
    TokenNotFound,
    /// The feed's bearer token is unknown or revoked.
    #[error("invalid decision-log token")]
    InvalidToken,
    #[error("a batch holds at most {MAX_BATCH_RECORDS} records")]
    BatchTooLarge,
    #[error("the outcome field is a dotted path of field names, e.g. decision or result.band")]
    InvalidOutcomeField,
    #[error("decision record not found")]
    RecordNotFound,
    #[error("the record cannot be read: {0}")]
    Unreadable(#[from] CipherError),
    #[error("the system random number generator failed")]
    Random,
    #[error(transparent)]
    Project(#[from] ProjectError),
    #[error(transparent)]
    Release(#[from] ReleaseError),
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

/// A decision-log token as administrators see it; never its value.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct LogToken {
    pub id: Uuid,
    #[sqlx(try_from = "String")]
    pub environment: EnvironmentName,
    pub name: String,
    pub hint: String,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub revoked_by: Option<Uuid>,
    pub revoked_at: Option<DateTime<Utc>>,
}

/// An environment read back from the database (always one of the two).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EnvironmentName(pub Environment);

#[derive(Debug, thiserror::Error)]
#[error("unknown environment {0}")]
pub struct UnknownEnvironment(String);

impl TryFrom<String> for EnvironmentName {
    type Error = UnknownEnvironment;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Environment::parse(&value)
            .map(Self)
            .ok_or(UnknownEnvironment(value))
    }
}

/// A token just issued: its value is shown this once.
#[derive(Debug, Clone)]
pub struct IssuedLogToken {
    pub token: String,
    pub details: LogToken,
}

/// The token a batch was sent with.
#[derive(Debug, Clone, Copy)]
pub struct FeedToken {
    id: Uuid,
    environment: Environment,
}

/// What became of a batch.
#[derive(Debug, Clone, Default)]
pub struct Receipt {
    /// Stored now or before (a record sent twice is stored once).
    pub accepted: usize,
    pub rejected: Vec<(Uuid, Rejection)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settings {
    /// The output field whose value is a record's outcome.
    pub outcome_field: Option<String>,
}

/// What a search narrows to; every field is optional.
#[derive(Debug, Clone, Default)]
pub struct Filter {
    pub reference: Option<String>,
    pub decision_key: Option<String>,
    pub outcome: Option<String>,
    pub environment: Option<Environment>,
    pub status: Option<Status>,
    pub from: Option<DateTime<Utc>>,
    pub to: Option<DateTime<Utc>>,
}

/// A record as searches list it: nothing the decision read or answered.
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct RecordSummary {
    pub id: Uuid,
    pub release_id: Uuid,
    #[sqlx(try_from = "String")]
    pub environment: EnvironmentName,
    pub decision_key: String,
    pub reference: Option<String>,
    pub status: Status,
    pub outcome: Option<String>,
    pub evaluated_at: DateTime<Utc>,
    pub duration_us: i64,
}

/// A record opened: its summary, release version and what was encrypted.
#[derive(Debug, Clone)]
pub struct Record {
    pub summary: RecordSummary,
    pub release_version: String,
    pub received_at: DateTime<Utc>,
    pub payload: Payload,
}

/// A record evaluated again with its release.
#[derive(Debug, Clone)]
pub struct Replay {
    /// Both succeeded with the same output, or both failed.
    pub identical: bool,
    pub status: Status,
    pub output: Option<Value>,
    pub error: Option<Value>,
}

#[derive(sqlx::FromRow)]
struct StoredRecord {
    #[sqlx(flatten)]
    summary: RecordSummary,
    received_at: DateTime<Utc>,
    key_id: String,
    nonce: Vec<u8>,
    payload: Vec<u8>,
}

const SUMMARY_COLUMNS: &str =
    "id, release_id, environment, decision_key, reference, status, outcome, evaluated_at, duration_us";
const TOKEN_COLUMNS: &str =
    "id, environment, name, hint, created_by, created_at, revoked_by, revoked_at";

#[derive(Clone)]
pub struct DecisionLog {
    pool: PgPool,
    clock: Arc<dyn Clock>,
    cipher: Arc<Cipher>,
    releases: Releases,
    runtime: Arc<dyn DecisionRuntime>,
    retention: Duration,
}

impl DecisionLog {
    pub fn new(
        pool: PgPool,
        clock: Arc<dyn Clock>,
        cipher: Arc<Cipher>,
        releases: Releases,
        runtime: Arc<dyn DecisionRuntime>,
        retention: Duration,
    ) -> Self {
        Self {
            pool,
            clock,
            cipher,
            releases,
            runtime,
            retention,
        }
    }

    // ----- Tokens (administrators) ------------------------------------------

    /// Every decision-log token, live ones first; never their value.
    pub async fn tokens(&self, actor: &User) -> Result<Vec<LogToken>, DecisionLogError> {
        require_admin(actor)?;
        Ok(sqlx::query_as(sql(format!(
            "SELECT {TOKEN_COLUMNS} FROM decision_log_tokens \
             ORDER BY revoked_at IS NOT NULL, created_at DESC"
        )))
        .fetch_all(&self.pool)
        .await?)
    }

    /// Issues a token a Runtime of `environment` sends its records with. Its
    /// value is returned once; only its hash is kept.
    pub async fn issue_token(
        &self,
        actor: &User,
        environment: Environment,
        name: &str,
    ) -> Result<IssuedLogToken, DecisionLogError> {
        require_admin(actor)?;
        let name = name.trim();
        if name.is_empty() || name.chars().count() > MAX_TOKEN_NAME_CHARS {
            return Err(DecisionLogError::InvalidTokenName);
        }
        let IssuedSecret { token, hash, hint } =
            secret::issue(TOKEN_PREFIX).map_err(|_| DecisionLogError::Random)?;
        let now = self.clock.now();
        let id = Uuid::new_v4();
        let mut tx = self.pool.begin().await?;
        sqlx::query(
            "INSERT INTO decision_log_tokens (id, environment, name, hash, hint, created_by, created_at) \
             VALUES ($1, $2, $3, $4, $5, $6, $7)",
        )
        .bind(id)
        .bind(environment.as_str())
        .bind(name)
        .bind(&hash)
        .bind(&hint)
        .bind(actor.id)
        .bind(now)
        .execute(&mut *tx)
        .await?;
        donka_audit::record(
            &mut tx,
            Event::new(now, Some(actor.id), Action::DecisionLogTokenIssued)
                .with_details(json!({ "environment": environment.as_str(), "name": name })),
        )
        .await?;
        tx.commit().await?;
        Ok(IssuedLogToken {
            token,
            details: LogToken {
                id,
                environment: EnvironmentName(environment),
                name: name.to_owned(),
                hint,
                created_by: actor.id,
                created_at: now,
                revoked_by: None,
                revoked_at: None,
            },
        })
    }

    /// Revokes a token: batches sent with it are refused from now on.
    pub async fn revoke_token(&self, actor: &User, id: Uuid) -> Result<(), DecisionLogError> {
        require_admin(actor)?;
        let now = self.clock.now();
        let mut tx = self.pool.begin().await?;
        let revoked: Option<(String, String)> = sqlx::query_as(
            "UPDATE decision_log_tokens SET revoked_at = $2, revoked_by = $3 \
             WHERE id = $1 AND revoked_at IS NULL RETURNING environment, name",
        )
        .bind(id)
        .bind(now)
        .bind(actor.id)
        .fetch_optional(&mut *tx)
        .await?;
        let (environment, name) = revoked.ok_or(DecisionLogError::TokenNotFound)?;
        donka_audit::record(
            &mut tx,
            Event::new(now, Some(actor.id), Action::DecisionLogTokenRevoked)
                .with_details(json!({ "environment": environment, "name": name })),
        )
        .await?;
        tx.commit().await?;
        Ok(())
    }

    // ----- Feed (Runtimes) --------------------------------------------------

    /// The live token a Runtime sent, from the `Authorization: Bearer` value.
    pub async fn authenticate(&self, bearer: &str) -> Result<FeedToken, DecisionLogError> {
        let row: Option<(Uuid, String)> = sqlx::query_as(
            "SELECT id, environment FROM decision_log_tokens WHERE hash = $1 AND revoked_at IS NULL",
        )
        .bind(secret::hash(bearer))
        .fetch_optional(&self.pool)
        .await?;
        let (id, environment) = row.ok_or(DecisionLogError::InvalidToken)?;
        let environment = Environment::parse(&environment).ok_or(DecisionLogError::InvalidToken)?;
        Ok(FeedToken { id, environment })
    }

    /// Stores a batch of records sent with `token`. Each record is checked on
    /// its own; those refused are listed with why.
    pub async fn receive(
        &self,
        token: FeedToken,
        records: Vec<Value>,
    ) -> Result<Receipt, DecisionLogError> {
        if records.len() > MAX_BATCH_RECORDS {
            return Err(DecisionLogError::BatchTooLarge);
        }
        let now = self.clock.now();
        let mut receipt = Receipt::default();
        let mut published: HashMap<(Uuid, Uuid), bool> = HashMap::new();
        let mut outcome_fields: HashMap<Uuid, Option<String>> = HashMap::new();
        let mut tx = self.pool.begin().await?;
        for value in records {
            let record = match FeedRecord::parse(value) {
                Ok(record) => record,
                Err(id) => {
                    match id {
                        Some(id) => receipt.rejected.push((id, Rejection::Invalid)),
                        None => tracing::warn!("a decision record without an id was refused"),
                    }
                    continue;
                }
            };
            if record.environment() != Some(token.environment) {
                receipt
                    .rejected
                    .push((record.id, Rejection::WrongEnvironment));
                continue;
            }
            let key = (record.project_id, record.release_id);
            let known = match published.get(&key) {
                Some(known) => *known,
                None => {
                    let known = self
                        .releases
                        .was_published(record.project_id, record.release_id, token.environment)
                        .await?;
                    published.insert(key, known);
                    known
                }
            };
            if !known {
                receipt
                    .rejected
                    .push((record.id, Rejection::UnknownRelease));
                continue;
            }
            let outcome_field = match outcome_fields.get(&record.project_id) {
                Some(field) => field.clone(),
                None => {
                    let field = outcome_field(&mut tx, record.project_id).await?;
                    outcome_fields.insert(record.project_id, field.clone());
                    field
                }
            };
            let outcome = record.outcome(outcome_field.as_deref());
            let (id, project_id, release_id, environment) = (
                record.id,
                record.project_id,
                record.release_id,
                token.environment,
            );
            let (decision_key, reference, status, evaluated_at, duration_us) = (
                record.decision_key.clone(),
                record.reference.clone(),
                record.status,
                record.evaluated_at,
                i64::try_from(record.duration_us).unwrap_or(i64::MAX),
            );
            let plaintext =
                serde_json::to_vec(&record.into_payload()).map_err(|_| CipherError::Corrupt)?;
            let sealed = self.cipher.seal(id, &plaintext)?;
            sqlx::query(
                "INSERT INTO decision_records \
                 (id, project_id, release_id, environment, decision_key, reference, status, outcome, \
                  evaluated_at, duration_us, received_at, token_id, key_id, nonce, payload) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15) \
                 ON CONFLICT (id) DO NOTHING",
            )
            .bind(id)
            .bind(project_id)
            .bind(release_id)
            .bind(environment.as_str())
            .bind(&decision_key)
            .bind(&reference)
            .bind(status)
            .bind(&outcome)
            .bind(evaluated_at)
            .bind(duration_us)
            .bind(now)
            .bind(token.id)
            .bind(&sealed.key_id)
            .bind(&sealed.nonce)
            .bind(&sealed.ciphertext)
            .execute(&mut *tx)
            .await?;
            receipt.accepted += 1;
        }
        tx.commit().await?;
        Ok(receipt)
    }

    // ----- Settings ---------------------------------------------------------

    /// The project's decision-log settings (any member).
    pub async fn settings(&self, access: &Access) -> Result<Settings, DecisionLogError> {
        let mut conn = self.pool.acquire().await?;
        Ok(Settings {
            outcome_field: outcome_field(&mut conn, access.project_id()).await?,
        })
    }

    /// Names the output field read as each new record's outcome (owners).
    /// Records already stored keep the outcome they arrived with.
    pub async fn update_settings(
        &self,
        access: &Access,
        outcome_field: Option<&str>,
    ) -> Result<Settings, DecisionLogError> {
        let field = outcome_field.map(str::trim).filter(|f| !f.is_empty());
        if let Some(field) = field {
            if !valid_field_path(field) {
                return Err(DecisionLogError::InvalidOutcomeField);
            }
        }
        let now = self.clock.now();
        let mut tx = self.pool.begin().await?;
        donka_project::authorize_change(&mut tx, access, Role::Owner).await?;
        let before = outcome_field_for_update(&mut tx, access.project_id()).await?;
        if before.as_deref() == field {
            return Ok(Settings {
                outcome_field: before,
            });
        }
        sqlx::query(
            "INSERT INTO decision_log_settings (project_id, outcome_field, updated_by, updated_at) \
             VALUES ($1, $2, $3, $4) ON CONFLICT (project_id) DO UPDATE \
             SET outcome_field = $2, updated_by = $3, updated_at = $4",
        )
        .bind(access.project_id())
        .bind(field)
        .bind(access.user_id())
        .bind(now)
        .execute(&mut *tx)
        .await?;
        donka_audit::record(
            &mut tx,
            Event::new(
                now,
                Some(access.user_id()),
                Action::DecisionLogSettingsUpdated,
            )
            .in_project(access.project_id())
            .with_details(json!({
                "from": { "outcomeField": before },
                "to": { "outcomeField": field },
            })),
        )
        .await?;
        tx.commit().await?;
        Ok(Settings {
            outcome_field: field.map(str::to_owned),
        })
    }

    // ----- Records (members) ------------------------------------------------

    /// The project's records matching `filter`, newest first (any member).
    /// Lists show nothing the decisions read or answered, so they are not audited.
    pub async fn search(
        &self,
        access: &Access,
        filter: &Filter,
        page: PageRequest,
    ) -> Result<Page<RecordSummary>, DecisionLogError> {
        let mut query = sqlx::QueryBuilder::new(format!(
            "SELECT {SUMMARY_COLUMNS} FROM decision_records WHERE project_id = "
        ));
        push_filter(&mut query, access, filter);
        query
            .push(" ORDER BY evaluated_at DESC, id DESC LIMIT ")
            .push_bind(page.limit)
            .push(" OFFSET ")
            .push_bind(page.offset);
        let items = query.build_query_as().fetch_all(&self.pool).await?;

        let mut count =
            sqlx::QueryBuilder::new("SELECT count(*) FROM decision_records WHERE project_id = ");
        push_filter(&mut count, access, filter);
        let (total,): (i64,) = count.build_query_as().fetch_one(&self.pool).await?;
        Ok(Page { items, total })
    }

    /// Opens a record (any member). It holds personal data, so each opening is audited.
    pub async fn view(&self, access: &Access, id: Uuid) -> Result<Record, DecisionLogError> {
        let (record, payload) = self.open(access, id, Action::DecisionRecordViewed).await?;
        let release = self.releases.get(access, record.summary.release_id).await?;
        Ok(Record {
            release_version: release.summary.version.to_string(),
            received_at: record.received_at,
            summary: record.summary,
            payload,
        })
    }

    /// Evaluates a record again with the release that answered it (any
    /// member), connector nodes answering with what they answered then. The
    /// result is compared with the recorded one. Audited like an opening.
    pub async fn replay(&self, access: &Access, id: Uuid) -> Result<Replay, DecisionLogError> {
        let (record, payload) = self
            .open(access, id, Action::DecisionRecordReplayed)
            .await?;
        let contents = self
            .releases
            .contents(access, record.summary.release_id)
            .await?;
        let evaluated = match Bundle::from_json(contents) {
            Ok(bundle) => {
                self.runtime
                    .evaluate(
                        &bundle,
                        &record.summary.decision_key,
                        payload.input.clone(),
                        EvaluateOptions {
                            trace: false,
                            recorded_trace: Some(Arc::new(
                                payload.trace.clone().unwrap_or(Value::Null),
                            )),
                        },
                    )
                    .await
            }
            Err(err) => Err(err),
        };
        let (status, output, error) = match evaluated {
            Ok(evaluation) => (Status::Succeeded, Some(evaluation.result), None),
            Err(err) => (Status::Failed, None, Some(runtime_error(err))),
        };
        let identical = match (record.summary.status, status) {
            (Status::Succeeded, Status::Succeeded) => output == payload.output,
            (Status::Failed, Status::Failed) => true,
            _ => false,
        };
        Ok(Replay {
            identical,
            status,
            output,
            error,
        })
    }

    /// Loads and decrypts a record, recording `action` for the reader.
    async fn open(
        &self,
        access: &Access,
        id: Uuid,
        action: Action,
    ) -> Result<(StoredRecord, Payload), DecisionLogError> {
        let mut tx = self.pool.begin().await?;
        let record: StoredRecord = sqlx::query_as(sql(format!(
            "SELECT {SUMMARY_COLUMNS}, received_at, key_id, nonce, payload \
             FROM decision_records WHERE project_id = $1 AND id = $2"
        )))
        .bind(access.project_id())
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(DecisionLogError::RecordNotFound)?;
        donka_audit::record(
            &mut tx,
            Event::new(self.clock.now(), Some(access.user_id()), action)
                .in_project(access.project_id())
                .with_details(json!({
                    "recordId": id,
                    "decisionKey": record.summary.decision_key,
                    "reference": record.summary.reference,
                })),
        )
        .await?;
        tx.commit().await?;
        let plaintext = self.cipher.open(
            id,
            &cipher::Sealed {
                key_id: record.key_id.clone(),
                nonce: record.nonce.clone(),
                ciphertext: record.payload.clone(),
            },
        )?;
        let payload = serde_json::from_slice(&plaintext).map_err(|_| CipherError::Corrupt)?;
        Ok((record, payload))
    }

    // ----- Retention --------------------------------------------------------

    /// Deletes the records evaluated before the retention period, project by
    /// project, each purge with its audit event. Returns how many went.
    pub async fn purge(&self) -> Result<u64, DecisionLogError> {
        let before = self.clock.now() - self.retention;
        let projects: Vec<(Uuid,)> = sqlx::query_as(
            "SELECT DISTINCT project_id FROM decision_records WHERE evaluated_at < $1",
        )
        .bind(before)
        .fetch_all(&self.pool)
        .await?;
        let mut purged = 0;
        for (project,) in projects {
            let mut tx = self.pool.begin().await?;
            // The guard on decision_records lets this transaction, and only it,
            // delete records evaluated before `before`.
            sqlx::query("SELECT set_config('donka.purge_before', $1, true)")
                .bind(before.to_rfc3339())
                .execute(&mut *tx)
                .await?;
            let deleted = sqlx::query(
                "DELETE FROM decision_records WHERE project_id = $1 AND evaluated_at < $2",
            )
            .bind(project)
            .bind(before)
            .execute(&mut *tx)
            .await?
            .rows_affected();
            if deleted > 0 {
                donka_audit::record(
                    &mut tx,
                    Event::new(self.clock.now(), None, Action::DecisionLogPurged)
                        .in_project(project)
                        .with_details(json!({
                            "before": before,
                            "records": deleted,
                            "retentionDays": self.retention.num_days(),
                        })),
                )
                .await?;
            }
            tx.commit().await?;
            purged += deleted;
        }
        Ok(purged)
    }
}

/// ` <project id> AND …` for each filter that is set.
fn push_filter(query: &mut sqlx::QueryBuilder<sqlx::Postgres>, access: &Access, filter: &Filter) {
    query.push_bind(access.project_id());
    if let Some(reference) = &filter.reference {
        query.push(" AND reference = ").push_bind(reference.clone());
    }
    if let Some(key) = &filter.decision_key {
        query.push(" AND decision_key = ").push_bind(key.clone());
    }
    if let Some(outcome) = &filter.outcome {
        query.push(" AND outcome = ").push_bind(outcome.clone());
    }
    if let Some(environment) = filter.environment {
        query
            .push(" AND environment = ")
            .push_bind(environment.as_str());
    }
    if let Some(status) = filter.status {
        query.push(" AND status = ").push_bind(status);
    }
    if let Some(from) = filter.from {
        query.push(" AND evaluated_at >= ").push_bind(from);
    }
    if let Some(to) = filter.to {
        query.push(" AND evaluated_at < ").push_bind(to);
    }
}

fn require_admin(actor: &User) -> Result<(), DecisionLogError> {
    if actor.is_admin {
        Ok(())
    } else {
        Err(DecisionLogError::NotAdministrator)
    }
}

/// `decision`, `result.band`: field names joined by dots.
fn valid_field_path(path: &str) -> bool {
    path.chars().count() <= MAX_TEXT_CHARS
        && path.split('.').all(|part| {
            part.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_')
                && part.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        })
}

async fn outcome_field(
    conn: &mut sqlx::PgConnection,
    project: Uuid,
) -> Result<Option<String>, sqlx::Error> {
    let row: Option<(Option<String>,)> =
        sqlx::query_as("SELECT outcome_field FROM decision_log_settings WHERE project_id = $1")
            .bind(project)
            .fetch_optional(conn)
            .await?;
    Ok(row.and_then(|(field,)| field))
}

async fn outcome_field_for_update(
    conn: &mut sqlx::PgConnection,
    project: Uuid,
) -> Result<Option<String>, sqlx::Error> {
    let row: Option<(Option<String>,)> = sqlx::query_as(
        "SELECT outcome_field FROM decision_log_settings WHERE project_id = $1 FOR UPDATE",
    )
    .bind(project)
    .fetch_optional(conn)
    .await?;
    Ok(row.and_then(|(field,)| field))
}

/// A replay's failure as the record shows one: the engine's own error document.
fn runtime_error(err: RuntimeError) -> Value {
    match err {
        RuntimeError::Evaluation { details } => details,
        other => json!({ "message": other.to_string() }),
    }
}

fn sql(text: String) -> sqlx::AssertSqlSafe<String> {
    sqlx::AssertSqlSafe(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outcome_fields_are_dotted_names() {
        for good in ["decision", "result.band", "_x.y_1"] {
            assert!(valid_field_path(good), "{good}");
        }
        for bad in ["", ".", "a.", "1st", "a-b", "a..b", "a b"] {
            assert!(!valid_field_path(bad), "{bad}");
        }
    }
}
