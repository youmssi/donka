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
use donka_engine::{contract, Bundle, DecisionRuntime, EvaluateOptions, RuntimeError};
use donka_identity::User;
use donka_project::{Access, ProjectError, Role};
use donka_release::{Environment, ReleaseError, Releases};
use donka_shared::clock::Clock;
use donka_shared::page::{Page, PageRequest};
use donka_shared::secret::{self, IssuedSecret};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet, HashMap};
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
/// Fields a project may list as never leaving Studio in an explanation.
pub const MAX_REDACTED_FIELDS: usize = 50;
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
    #[error(
        "redacted fields are at most {MAX_REDACTED_FIELDS} dotted paths, e.g. applicant.nationalId"
    )]
    InvalidRedactedFields,
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

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Settings {
    /// The output field whose value is a record's outcome.
    pub outcome_field: Option<String>,
    /// Fields removed from a record before it is sent to be explained
    /// (dotted paths, e.g. `applicant.nationalId`), in the order given.
    pub redacted_fields: Vec<String>,
}

/// A change to the settings; a field left `None` keeps its value.
#[derive(Debug, Clone, Default)]
pub struct SettingsChange {
    /// `Some(None)` removes the outcome field.
    pub outcome_field: Option<Option<String>>,
    pub redacted_fields: Option<Vec<String>>,
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
                    let field = stored_settings(&mut tx, record.project_id, false)
                        .await?
                        .outcome_field;
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
        Ok(stored_settings(&mut conn, access.project_id(), false).await?)
    }

    /// Changes the outcome field read from each new record and the fields
    /// removed before a record is explained (owners). Records already stored
    /// keep the outcome they arrived with.
    pub async fn update_settings(
        &self,
        access: &Access,
        change: SettingsChange,
    ) -> Result<Settings, DecisionLogError> {
        let outcome_field = change
            .outcome_field
            .map(|field| field.map(|f| f.trim().to_owned()).filter(|f| !f.is_empty()));
        if let Some(Some(field)) = &outcome_field {
            if !valid_field_path(field) {
                return Err(DecisionLogError::InvalidOutcomeField);
            }
        }
        let redacted_fields = change.redacted_fields.map(normalize_fields).transpose()?;

        let now = self.clock.now();
        let mut tx = self.pool.begin().await?;
        donka_project::authorize_change(&mut tx, access, Role::Owner).await?;
        let before = stored_settings(&mut tx, access.project_id(), true).await?;
        let after = Settings {
            outcome_field: outcome_field.unwrap_or_else(|| before.outcome_field.clone()),
            redacted_fields: redacted_fields.unwrap_or_else(|| before.redacted_fields.clone()),
        };
        if after == before {
            return Ok(before);
        }
        sqlx::query(
            "INSERT INTO decision_log_settings \
             (project_id, outcome_field, redacted_fields, updated_by, updated_at) \
             VALUES ($1, $2, $3, $4, $5) ON CONFLICT (project_id) DO UPDATE \
             SET outcome_field = $2, redacted_fields = $3, updated_by = $4, updated_at = $5",
        )
        .bind(access.project_id())
        .bind(&after.outcome_field)
        .bind(&after.redacted_fields)
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
                "from": settings_details(&before),
                "to": settings_details(&after),
            })),
        )
        .await?;
        tx.commit().await?;
        Ok(after)
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

    /// Opens a record to send it to be explained (any member), with the
    /// project's redacted fields removed, and the fields its decision's input contract marks
    /// as personal data (`pii`, DNK-37) without listing them again. Audited before anything
    /// leaves Studio.
    pub async fn explain_source(
        &self,
        access: &Access,
        id: Uuid,
    ) -> Result<Record, DecisionLogError> {
        let (record, payload) = self
            .open(access, id, Action::DecisionRecordExplained)
            .await?;
        let mut conn = self.pool.acquire().await?;
        let settings = stored_settings(&mut conn, access.project_id(), false).await?;
        drop(conn);
        let release = self.releases.get(access, record.summary.release_id).await?;
        let contents = self
            .releases
            .contents(access, record.summary.release_id)
            .await?;
        let mut fields = settings.redacted_fields;
        if let Some(schema) = contents
            .get(&record.summary.decision_key)
            .and_then(|content| contract::input_schema(content).ok().flatten())
        {
            for field in contract::pii_fields(&schema) {
                if !fields.contains(&field) {
                    fields.push(field);
                }
            }
        }
        Ok(Record {
            release_version: release.summary.version.to_string(),
            received_at: record.received_at,
            summary: record.summary,
            payload: redact(payload, &fields),
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

// ----- Key rotation (DNK-40) -----------------------------------------------

/// Records still sealed with a key other than the current one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyUsage {
    pub key_id: String,
    pub records: i64,
    /// Whether `DONKA_DECISION_LOG_PREVIOUS_KEYS` holds this key.
    pub configured: bool,
}

/// What re-sealing reads of a record.
#[derive(sqlx::FromRow)]
struct SealedRow {
    id: Uuid,
    project_id: Uuid,
    key_id: String,
    nonce: Vec<u8>,
    payload: Vec<u8>,
}

impl DecisionLog {
    /// The id of the key new records are sealed with.
    pub fn key_id(&self) -> &str {
        self.cipher.key_id()
    }

    /// For each key other than the current one, configured or still sealing
    /// records, how many records it seals.
    pub async fn previous_key_usage(&self) -> Result<Vec<KeyUsage>, DecisionLogError> {
        // The keys in use, one index probe per key (a skip scan): the table can
        // hold years of records and this runs at every start.
        let keys: Vec<String> = sqlx::query_scalar(
            "WITH RECURSIVE used(key_id) AS ( \
                 SELECT min(key_id) FROM decision_records \
                 UNION ALL \
                 SELECT (SELECT min(key_id) FROM decision_records WHERE key_id > used.key_id) \
                 FROM used WHERE used.key_id IS NOT NULL \
             ) SELECT key_id FROM used WHERE key_id IS NOT NULL",
        )
        .fetch_all(&self.pool)
        .await?;
        let mut rows = Vec::new();
        for key_id in keys.into_iter().filter(|key| key != self.cipher.key_id()) {
            let records: i64 =
                sqlx::query_scalar("SELECT count(*) FROM decision_records WHERE key_id = $1")
                    .bind(&key_id)
                    .fetch_one(&self.pool)
                    .await?;
            rows.push((key_id, records));
        }
        let configured = self.cipher.previous_key_ids();
        let mut usage: Vec<KeyUsage> = rows
            .into_iter()
            .map(|(key_id, records)| KeyUsage {
                configured: configured.contains(&key_id),
                key_id,
                records,
            })
            .collect();
        // A previous key no record uses any more can be removed.
        for key_id in configured {
            if !usage.iter().any(|key| key.key_id == key_id) {
                usage.push(KeyUsage {
                    key_id,
                    records: 0,
                    configured: true,
                });
            }
        }
        usage.sort_by(|a, b| a.key_id.cmp(&b.key_id));
        Ok(usage)
    }

    /// Re-seals up to `limit` records sealed with one of the previous keys with the
    /// current one, in one transaction that also records, per project, how many were
    /// re-sealed. Returns how many; zero once none is left. Stopping between
    /// batches loses nothing: the next call starts with the records left.
    pub async fn reseal_batch(&self, limit: i64) -> Result<u64, DecisionLogError> {
        let mut tx = self.pool.begin().await?;
        // The guard on decision_records lets this transaction, and only it,
        // replace a record's key, nonce and payload.
        sqlx::query("SELECT set_config('donka.reseal', 'on', true)")
            .execute(&mut *tx)
            .await?;
        let rows: Vec<SealedRow> = sqlx::query_as(
            // Through the (key_id, id) index: each batch reads only records
            // still to re-seal, however many came before.
            "SELECT id, project_id, key_id, nonce, payload FROM decision_records \
             WHERE key_id = ANY($1) ORDER BY key_id, id LIMIT $2 FOR UPDATE SKIP LOCKED",
        )
        .bind(self.cipher.previous_key_ids())
        .bind(limit)
        .fetch_all(&mut *tx)
        .await?;

        let mut projects: BTreeMap<Uuid, (u64, BTreeSet<String>)> = BTreeMap::new();
        for row in rows {
            let (id, key_id) = (row.id, row.key_id);
            let plaintext = self.cipher.open(
                id,
                &cipher::Sealed {
                    key_id: key_id.clone(),
                    nonce: row.nonce,
                    ciphertext: row.payload,
                },
            )?;
            let sealed = self.cipher.seal(id, &plaintext)?;
            sqlx::query(
                "UPDATE decision_records SET key_id = $2, nonce = $3, payload = $4 WHERE id = $1",
            )
            .bind(id)
            .bind(&sealed.key_id)
            .bind(&sealed.nonce)
            .bind(&sealed.ciphertext)
            .execute(&mut *tx)
            .await?;
            let entry = projects.entry(row.project_id).or_default();
            entry.0 += 1;
            entry.1.insert(key_id);
        }

        let mut resealed = 0;
        for (project, (records, from)) in projects {
            donka_audit::record(
                &mut tx,
                Event::new(self.clock.now(), None, Action::DecisionLogResealed)
                    .in_project(project)
                    .with_details(json!({
                        "records": records,
                        "fromKeys": from,
                        "toKey": self.cipher.key_id(),
                    })),
            )
            .await?;
            resealed += records;
        }
        tx.commit().await?;
        Ok(resealed)
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

/// The project's settings; `lock` holds them for the transaction's update.
async fn stored_settings(
    conn: &mut sqlx::PgConnection,
    project: Uuid,
    lock: bool,
) -> Result<Settings, sqlx::Error> {
    let row: Option<(Option<String>, Vec<String>)> = sqlx::query_as(sql(format!(
        "SELECT outcome_field, redacted_fields FROM decision_log_settings WHERE project_id = $1{}",
        if lock { " FOR UPDATE" } else { "" }
    )))
    .bind(project)
    .fetch_optional(conn)
    .await?;
    Ok(row
        .map(|(outcome_field, redacted_fields)| Settings {
            outcome_field,
            redacted_fields,
        })
        .unwrap_or_default())
}

fn settings_details(settings: &Settings) -> Value {
    json!({
        "outcomeField": settings.outcome_field,
        "redactedFields": settings.redacted_fields,
    })
}

/// Trims, checks and removes duplicates, keeping the order given.
fn normalize_fields(fields: Vec<String>) -> Result<Vec<String>, DecisionLogError> {
    let mut kept: Vec<String> = Vec::new();
    for field in fields {
        let field = field.trim().to_owned();
        if field.is_empty() || !valid_field_path(&field) {
            return Err(DecisionLogError::InvalidRedactedFields);
        }
        if !kept.contains(&field) {
            kept.push(field);
        }
    }
    if kept.len() > MAX_REDACTED_FIELDS {
        return Err(DecisionLogError::InvalidRedactedFields);
    }
    Ok(kept)
}

/// The payload without the `fields` (dotted paths): removed from what the
/// decision read and answered, and from what each node of the trace received
/// and returned. Values the engine traced under an expression naming a field
/// (a decision table's `reference_map`) go too.
pub fn redact(payload: Payload, fields: &[String]) -> Payload {
    if fields.is_empty() {
        return payload;
    }
    let paths: Vec<Vec<&str>> = fields.iter().map(|f| f.split('.').collect()).collect();
    let strip = |mut value: Value| {
        for path in &paths {
            remove_path(&mut value, path);
        }
        value
    };
    let trace = payload.trace.map(|mut trace| {
        if let Some(nodes) = trace.as_object_mut() {
            for node in nodes.values_mut() {
                for key in ["input", "output", "traceData"] {
                    if let Some(part) = node.get_mut(key) {
                        *part = strip(part.take());
                    }
                }
                if let Some(map) = node
                    .pointer_mut("/traceData/reference_map")
                    .and_then(Value::as_object_mut)
                {
                    map.retain(|expression, _| !names_field(expression, fields));
                }
            }
        }
        trace
    });
    Payload {
        input: strip(payload.input),
        output: payload.output.map(strip),
        error: payload.error,
        trace,
    }
}

fn remove_path(value: &mut Value, path: &[&str]) {
    match value {
        // A path through a list applies to each of its items.
        Value::Array(items) => {
            for item in items {
                remove_path(item, path);
            }
        }
        Value::Object(object) => match path {
            [] => {}
            [last] => {
                object.remove(*last);
            }
            [first, rest @ ..] => {
                if let Some(child) = object.get_mut(*first) {
                    remove_path(child, rest);
                }
            }
        },
        _ => {}
    }
}

/// Whether an expression is a redacted field or something inside it.
fn names_field(expression: &str, fields: &[String]) -> bool {
    let expression = expression.trim();
    fields.iter().any(|field| {
        expression == field
            || expression
                .strip_prefix(field.as_str())
                .is_some_and(|rest| rest.starts_with(['.', '[']))
    })
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

    fn payload() -> Payload {
        Payload {
            input: json!({
                "applicant": { "nationalId": "CM-1", "income": 150000, "name": "Ada" },
                "loans": [{ "iban": "CM21", "amount": 10 }, { "iban": "CM22", "amount": 20 }]
            }),
            output: Some(json!({ "applicant": { "nationalId": "CM-1" }, "decision": "approve" })),
            error: None,
            trace: Some(json!({
                "table": {
                    "name": "limits",
                    "input": { "applicant": { "nationalId": "CM-1", "income": 150000 } },
                    "output": { "decision": "approve" },
                    "traceData": { "reference_map": {
                        "applicant.nationalId": "CM-1",
                        "applicant.nationalId[0]": "C",
                        "applicant.nationalIdType": "passport",
                        "applicant.income": 150000
                    } }
                }
            })),
        }
    }

    #[test]
    fn redaction_removes_fields_wherever_the_record_holds_them() {
        let fields = vec!["applicant.nationalId".to_owned(), "loans.iban".to_owned()];
        let redacted = redact(payload(), &fields);
        assert_eq!(
            redacted.input,
            json!({
                "applicant": { "income": 150000, "name": "Ada" },
                "loans": [{ "amount": 10 }, { "amount": 20 }]
            })
        );
        assert_eq!(
            redacted.output,
            Some(json!({ "applicant": {}, "decision": "approve" }))
        );
        let trace = redacted.trace.unwrap();
        assert_eq!(
            trace["table"]["input"],
            json!({ "applicant": { "income": 150000 } })
        );
        assert_eq!(
            trace["table"]["traceData"]["reference_map"],
            json!({ "applicant.nationalIdType": "passport", "applicant.income": 150000 })
        );
        assert!(!serde_json::to_string(&trace).unwrap().contains("CM-1"));
    }

    #[test]
    fn nothing_listed_changes_nothing() {
        let unchanged = redact(payload(), &[]);
        assert_eq!(unchanged.input, payload().input);
        assert_eq!(unchanged.trace, payload().trace);
    }

    #[test]
    fn redacted_fields_are_checked_and_deduplicated() {
        let fields = |list: &[&str]| list.iter().map(|f| (*f).to_owned()).collect::<Vec<_>>();
        assert_eq!(
            normalize_fields(fields(&[" a.b ", "c", "a.b"])).unwrap(),
            fields(&["a.b", "c"])
        );
        assert!(normalize_fields(fields(&["a-b"])).is_err());
        assert!(normalize_fields(fields(&[""])).is_err());
        let many: Vec<String> = (0..=MAX_REDACTED_FIELDS).map(|i| format!("f{i}")).collect();
        assert!(normalize_fields(many).is_err());
    }
}
