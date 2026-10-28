//! The rights of the people decisions are about (DNK-75): an owner finds every record about a
//! person by an identifier, gives them a report of it, anonymises their records when the law
//! allows, and records a contest of a decision with its review outcome.
//!
//! A record is found through keyed hashes of the identifiers it holds (the input fields its
//! decision's contract marks `identifier`), so nothing is decrypted to search and the index
//! holds no identifier in clear.

use crate::{
    redact, sql, stored_settings, DecisionLog, DecisionLogError, Payload, Record, RecordSummary,
    SUMMARY_COLUMNS,
};
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use chrono::{DateTime, NaiveDate, Utc};
use donka_audit::{Action, Event};
use donka_engine::contract;
use donka_project::{Access, Role};
use donka_shared::cipher::{CipherError, Sealed};
use hmac::{Hmac, Mac};
use serde_json::{json, Value};
use sha2::Sha256;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use uuid::Uuid;

/// Longest identifier searched for.
pub const MAX_IDENTIFIER_CHARS: usize = 200;
pub const MAX_RULE_DESCRIPTION_CHARS: usize = 500;
pub const MAX_LEGAL_REFERENCE_CHARS: usize = 200;
pub const MAX_CONTEST_TEXT_CHARS: usize = 2000;
pub const MAX_OUTCOME_CHARS: usize = 200;

/// The key identifiers are hashed with (`DONKA_PERSON_INDEX_KEY`). Without it, a hash cannot
/// be computed from a guessed national ID or phone number.
#[derive(Clone)]
pub struct PersonKey(Hmac<Sha256>);

// Never shown.
impl std::fmt::Debug for PersonKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("PersonKey")
    }
}

impl PersonKey {
    /// From 32 random bytes in base64.
    pub fn from_base64(key: &str) -> Result<Self, CipherError> {
        let bytes = STANDARD
            .decode(key.trim())
            .map_err(|_| CipherError::InvalidKey)?;
        if bytes.len() != 32 {
            return Err(CipherError::InvalidKey);
        }
        Hmac::new_from_slice(&bytes)
            .map(Self)
            .map_err(|_| CipherError::InvalidKey)
    }

    /// The hash of an identifier as written anywhere: case, spaces, dashes and dots do not
    /// matter. `None` when nothing is left of it.
    pub fn hash(&self, identifier: &str) -> Option<Vec<u8>> {
        let normalised: String = identifier
            .chars()
            .filter(|c| !c.is_whitespace() && !matches!(c, '-' | '.'))
            .flat_map(char::to_lowercase)
            .collect();
        if normalised.is_empty() {
            return None;
        }
        let mut mac = self.0.clone();
        mac.update(normalised.as_bytes());
        Some(mac.finalize().into_bytes().to_vec())
    }

    /// The hashes of the identifiers `input` holds at `paths` (through lists).
    fn hashes(&self, input: &Value, paths: &[String]) -> BTreeSet<Vec<u8>> {
        let mut values = Vec::new();
        for path in paths {
            let parts: Vec<&str> = path.split('.').collect();
            values_at(input, &parts, &mut values);
        }
        values
            .into_iter()
            .filter_map(|value| match value {
                Value::String(text) => self.hash(text),
                Value::Number(number) => self.hash(&number.to_string()),
                _ => None,
            })
            .collect()
    }
}

fn values_at<'a>(value: &'a Value, path: &[&str], out: &mut Vec<&'a Value>) {
    match (value, path) {
        (Value::Array(items), _) => {
            for item in items {
                values_at(item, path, out);
            }
        }
        (_, []) => out.push(value),
        (Value::Object(object), [first, rest @ ..]) => {
            if let Some(child) = object.get(*first) {
                values_at(child, rest, out);
            }
        }
        _ => {}
    }
}

/// A record about the person, as the search lists it.
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct PersonRecord {
    #[sqlx(flatten)]
    pub summary: RecordSummary,
    /// When the person's fields were removed from it.
    pub anonymised_at: Option<DateTime<Utc>>,
}

/// Every record about a person, opened for their access report.
#[derive(Debug, Clone)]
pub struct PersonReport {
    pub generated_at: DateTime<Utc>,
    pub entries: Vec<PersonReportEntry>,
}

/// A record about the person, opened for their report.
#[derive(Debug, Clone)]
pub struct PersonReportEntry {
    pub record: Record,
    pub contests: Vec<Contest>,
    /// What may be sent to be explained: the record without its personal and redacted fields.
    pub explainable: Payload,
}

/// How far `donka-app decision-log index-people` got.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IndexProgress {
    /// Records read by this batch.
    pub records: u64,
    pub finished: bool,
}

/// A documented reason to keep a project's records whole (a dispute, an anti-money-laundering
/// file): while it runs, they cannot be anonymised.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct RetentionRule {
    pub id: Uuid,
    pub description: String,
    pub legal_reference: Option<String>,
    /// The last day it runs.
    pub until: NaiveDate,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub ended_by: Option<Uuid>,
    pub ended_at: Option<DateTime<Utc>>,
    /// Not ended, and `until` is today or later.
    pub running: bool,
}

#[derive(Debug, Clone)]
pub struct NewRetentionRule {
    pub description: String,
    pub legal_reference: Option<String>,
    pub until: NaiveDate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type)]
#[sqlx(type_name = "text", rename_all = "lowercase")]
pub enum ContestStatus {
    /// Recorded, waiting for the review.
    Open,
    /// The review kept the decision.
    Upheld,
    /// The review changed the decision: the new outcome is recorded.
    Revised,
}

impl ContestStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            ContestStatus::Open => "open",
            ContestStatus::Upheld => "upheld",
            ContestStatus::Revised => "revised",
        }
    }
}

/// A person contesting a decision, and the outcome of the review.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct Contest {
    pub id: Uuid,
    pub record_id: Uuid,
    pub status: ContestStatus,
    pub reason: String,
    pub opened_by: Uuid,
    pub opened_at: DateTime<Utc>,
    pub review: Option<String>,
    pub revised_outcome: Option<String>,
    pub reviewed_by: Option<Uuid>,
    pub reviewed_at: Option<DateTime<Utc>>,
}

/// What a review concluded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContestOutcome {
    Upheld,
    Revised { outcome: String },
}

const RULE_COLUMNS: &str = "id, description, legal_reference, until, created_by, created_at, \
     ended_by, ended_at, (ended_at IS NULL AND until >= $2) AS running";
const CONTEST_COLUMNS: &str = "id, record_id, status, reason, opened_by, opened_at, review, \
     revised_outcome, reviewed_by, reviewed_at";

/// The records of `project` holding the identifier hashed as `$2`.
const PERSON_RECORDS: &str = "project_id = $1 AND (id, evaluated_at) IN \
     (SELECT record_id, evaluated_at FROM decision_record_people WHERE project_id = $1 AND hash = $2)";

/// Fields read from a release's decision contract, cached for one batch.
pub(crate) type ContractFields = HashMap<(Uuid, String), Vec<String>>;

impl DecisionLog {
    /// The paths `pick` reads from the contract of `key` in `release`, cached in `cache`.
    pub(crate) async fn contract_fields(
        &self,
        cache: &mut ContractFields,
        project: Uuid,
        release: Uuid,
        key: &str,
        pick: fn(&Value) -> Vec<String>,
    ) -> Result<Vec<String>, DecisionLogError> {
        if let Some(fields) = cache.get(&(release, key.to_owned())) {
            return Ok(fields.clone());
        }
        let fields = self
            .releases
            .decision_content(project, release, key)
            .await?
            .and_then(|content| contract::input_schema(&content).ok().flatten())
            .map(|schema| pick(&schema))
            .unwrap_or_default();
        cache.insert((release, key.to_owned()), fields.clone());
        Ok(fields)
    }

    /// Adds a record's identifier hashes to the index, in the record's transaction.
    pub(crate) async fn index_person(
        &self,
        conn: &mut sqlx::PgConnection,
        project: Uuid,
        record: (Uuid, DateTime<Utc>),
        input: &Value,
        paths: &[String],
    ) -> Result<usize, sqlx::Error> {
        let hashes: Vec<Vec<u8>> = self.person_key.hashes(input, paths).into_iter().collect();
        if hashes.is_empty() {
            return Ok(0);
        }
        sqlx::query(
            "INSERT INTO decision_record_people (project_id, hash, record_id, evaluated_at) \
             SELECT $1, hash, $3, $4 FROM unnest($2::bytea[]) AS hash ON CONFLICT DO NOTHING",
        )
        .bind(project)
        .bind(&hashes)
        .bind(record.0)
        .bind(record.1)
        .execute(conn)
        .await?;
        Ok(hashes.len())
    }

    // ----- Indexing records stored before DNK-75 ----------------------------

    /// Whether every record stored before the people index existed is indexed.
    pub async fn people_index_finished(&self) -> Result<bool, DecisionLogError> {
        let finished: Option<DateTime<Utc>> =
            sqlx::query_scalar("SELECT finished_at FROM decision_record_people_backfill")
                .fetch_one(&self.pool)
                .await?;
        Ok(finished.is_some())
    }

    /// Indexes up to `limit` of the records stored before the people index existed, oldest
    /// first, in one transaction that records where it stopped and, per project, how many
    /// records it read. Stopping between batches loses nothing.
    pub async fn index_people_batch(&self, limit: i64) -> Result<IndexProgress, DecisionLogError> {
        let mut tx = self.pool.begin().await?;
        #[derive(sqlx::FromRow)]
        struct Backfill {
            received_before: DateTime<Utc>,
            after_evaluated_at: Option<DateTime<Utc>>,
            after_id: Option<Uuid>,
            finished_at: Option<DateTime<Utc>>,
        }
        let Backfill {
            received_before,
            after_evaluated_at,
            after_id,
            finished_at,
        } = sqlx::query_as(
            "SELECT received_before, after_evaluated_at, after_id, finished_at \
             FROM decision_record_people_backfill FOR UPDATE",
        )
        .fetch_one(&mut *tx)
        .await?;
        if finished_at.is_some() {
            return Ok(IndexProgress {
                records: 0,
                finished: true,
            });
        }
        #[derive(sqlx::FromRow)]
        struct Row {
            id: Uuid,
            evaluated_at: DateTime<Utc>,
            project_id: Uuid,
            release_id: Uuid,
            decision_key: String,
            key_id: String,
            nonce: Vec<u8>,
            payload: Vec<u8>,
        }
        let rows: Vec<Row> = sqlx::query_as(
            "SELECT id, evaluated_at, project_id, release_id, decision_key, key_id, nonce, payload \
             FROM decision_records \
             WHERE received_at < $1 AND anonymised_at IS NULL \
               AND ($2::timestamptz IS NULL OR (evaluated_at, id) > ($2, $3)) \
             ORDER BY evaluated_at, id LIMIT $4",
        )
        .bind(received_before)
        .bind(after_evaluated_at)
        .bind(after_id)
        .bind(limit)
        .fetch_all(&mut *tx)
        .await?;

        let mut cache = ContractFields::new();
        let mut projects: BTreeMap<Uuid, (u64, u64)> = BTreeMap::new();
        let last = rows.last().map(|row| (row.evaluated_at, row.id));
        let records = rows.len() as u64;
        for row in rows {
            let paths = self
                .contract_fields(
                    &mut cache,
                    row.project_id,
                    row.release_id,
                    &row.decision_key,
                    contract::identifier_fields,
                )
                .await?;
            let entry = projects.entry(row.project_id).or_default();
            entry.0 += 1;
            if paths.is_empty() {
                continue;
            }
            let payload = self.unseal(
                row.id,
                Sealed {
                    key_id: row.key_id,
                    nonce: row.nonce,
                    ciphertext: row.payload,
                },
            )?;
            let indexed = self
                .index_person(
                    &mut tx,
                    row.project_id,
                    (row.id, row.evaluated_at),
                    &payload.input,
                    &paths,
                )
                .await?;
            entry.1 += u64::from(indexed > 0);
        }
        let finished = records < u64::try_from(limit).unwrap_or(0);
        let now = self.clock.now();
        sqlx::query(
            "UPDATE decision_record_people_backfill SET \
             after_evaluated_at = COALESCE($1, after_evaluated_at), \
             after_id = COALESCE($2, after_id), finished_at = $3",
        )
        .bind(last.map(|(at, _)| at))
        .bind(last.map(|(_, id)| id))
        .bind(finished.then_some(now))
        .execute(&mut *tx)
        .await?;
        for (project, (read, indexed)) in projects {
            donka_audit::record(
                &mut tx,
                Event::new(now, None, Action::DecisionLogPeopleIndexed)
                    .in_project(project)
                    .with_details(json!({ "records": read, "withIdentifiers": indexed })),
            )
            .await?;
        }
        tx.commit().await?;
        Ok(IndexProgress { records, finished })
    }

    // ----- A person's records (owners) --------------------------------------

    fn person_hash(&self, identifier: &str) -> Result<Vec<u8>, DecisionLogError> {
        if identifier.chars().count() > MAX_IDENTIFIER_CHARS {
            return Err(DecisionLogError::InvalidIdentifier);
        }
        self.person_key
            .hash(identifier)
            .ok_or(DecisionLogError::InvalidIdentifier)
    }

    /// The project's records holding `identifier` in a field its contract marks as one,
    /// newest first (owners). Audited, without the identifier.
    pub async fn find_person(
        &self,
        access: &Access,
        identifier: &str,
    ) -> Result<Vec<PersonRecord>, DecisionLogError> {
        access.require(Role::Owner)?;
        let hash = self.person_hash(identifier)?;
        let mut tx = self.pool.begin().await?;
        let records: Vec<PersonRecord> = sqlx::query_as(sql(format!(
            "SELECT {SUMMARY_COLUMNS}, anonymised_at FROM decision_records \
             WHERE {PERSON_RECORDS} ORDER BY evaluated_at DESC, id DESC"
        )))
        .bind(access.project_id())
        .bind(&hash)
        .fetch_all(&mut *tx)
        .await?;
        donka_audit::record(
            &mut tx,
            Event::new(
                self.clock.now(),
                Some(access.user_id()),
                Action::PersonRecordsSearched,
            )
            .in_project(access.project_id())
            .with_details(json!({ "records": records.len() })),
        )
        .await?;
        tx.commit().await?;
        Ok(records)
    }

    /// Every record about the person, opened for their access report, oldest first (owners).
    /// Audited once, with how many records it holds and whether they are explained.
    pub async fn person_report(
        &self,
        access: &Access,
        identifier: &str,
        explained: bool,
    ) -> Result<PersonReport, DecisionLogError> {
        access.require(Role::Owner)?;
        let hash = self.person_hash(identifier)?;
        #[derive(sqlx::FromRow)]
        struct Row {
            #[sqlx(flatten)]
            summary: RecordSummary,
            received_at: DateTime<Utc>,
            anonymised_at: Option<DateTime<Utc>>,
            key_id: String,
            nonce: Vec<u8>,
            payload: Vec<u8>,
        }
        let generated_at = self.clock.now();
        let mut tx = self.pool.begin().await?;
        let rows: Vec<Row> = sqlx::query_as(sql(format!(
            "SELECT {SUMMARY_COLUMNS}, received_at, anonymised_at, key_id, nonce, payload \
             FROM decision_records WHERE {PERSON_RECORDS} ORDER BY evaluated_at, id"
        )))
        .bind(access.project_id())
        .bind(&hash)
        .fetch_all(&mut *tx)
        .await?;
        donka_audit::record(
            &mut tx,
            Event::new(
                generated_at,
                Some(access.user_id()),
                Action::PersonReportExported,
            )
            .in_project(access.project_id())
            .with_details(json!({ "records": rows.len(), "explained": explained })),
        )
        .await?;
        tx.commit().await?;

        let ids: Vec<Uuid> = rows.iter().map(|row| row.summary.id).collect();
        let mut contests = self.contests_of(access.project_id(), &ids).await?;
        let redacted = {
            let mut conn = self.pool.acquire().await?;
            stored_settings(&mut conn, access.project_id(), false)
                .await?
                .redacted_fields
        };
        let mut personal = ContractFields::new();
        let mut versions: HashMap<Uuid, String> = HashMap::new();
        let mut entries = Vec::with_capacity(rows.len());
        for row in rows {
            let summary = row.summary;
            let payload = self.unseal(
                summary.id,
                Sealed {
                    key_id: row.key_id,
                    nonce: row.nonce,
                    ciphertext: row.payload,
                },
            )?;
            let release_version = match versions.get(&summary.release_id) {
                Some(version) => version.clone(),
                None => {
                    let release = self.releases.get(access, summary.release_id).await?;
                    let version = release.summary.version.to_string();
                    versions.insert(summary.release_id, version.clone());
                    version
                }
            };
            let fields = self
                .removed_fields(&mut personal, access.project_id(), &summary, &redacted)
                .await?;
            entries.push(PersonReportEntry {
                explainable: redact(payload.clone(), &fields),
                contests: contests.remove(&summary.id).unwrap_or_default(),
                record: Record {
                    release_version,
                    received_at: row.received_at,
                    anonymised_at: row.anonymised_at,
                    summary,
                    payload,
                },
            });
        }
        Ok(PersonReport {
            generated_at,
            entries,
        })
    }

    /// The fields removed before a record is explained or when it is anonymised: those its
    /// contract marks as personal data, and the project's redacted fields.
    pub(crate) async fn removed_fields(
        &self,
        cache: &mut ContractFields,
        project: Uuid,
        summary: &RecordSummary,
        redacted: &[String],
    ) -> Result<Vec<String>, DecisionLogError> {
        let mut fields = redacted.to_vec();
        for field in self
            .contract_fields(
                cache,
                project,
                summary.release_id,
                &summary.decision_key,
                contract::pii_fields,
            )
            .await?
        {
            if !fields.contains(&field) {
                fields.push(field);
            }
        }
        Ok(fields)
    }

    /// Removes the person's fields from every record holding `identifier` (owners): the
    /// fields the contract marks as personal data and the project's redacted fields go from
    /// the input, output and trace; outcome, release and time stay. It cannot be undone, and
    /// the records leave the people index. Refused while a retention rule runs, naming it.
    /// Returns how many records were anonymised.
    pub async fn anonymise_person(
        &self,
        access: &Access,
        identifier: &str,
    ) -> Result<u64, DecisionLogError> {
        access.require(Role::Owner)?;
        let hash = self.person_hash(identifier)?;
        let now = self.clock.now();
        let mut tx = self.pool.begin().await?;
        lock_rules(&mut tx, access.project_id()).await?;
        let running: Option<RetentionRule> = sqlx::query_as(sql(format!(
            "SELECT {RULE_COLUMNS} FROM retention_rules \
             WHERE project_id = $1 AND ended_at IS NULL AND until >= $2 \
             ORDER BY until DESC, created_at LIMIT 1"
        )))
        .bind(access.project_id())
        .bind(now.date_naive())
        .fetch_optional(&mut *tx)
        .await?;
        if let Some(rule) = running {
            return Err(DecisionLogError::RetentionRuleRuns(Box::new(rule)));
        }
        // The guard on decision_records lets this transaction, and only it, replace a
        // record's payload once, setting when it was anonymised.
        sqlx::query("SELECT set_config('donka.anonymise', 'on', true)")
            .execute(&mut *tx)
            .await?;
        #[derive(sqlx::FromRow)]
        struct Row {
            #[sqlx(flatten)]
            summary: RecordSummary,
            key_id: String,
            nonce: Vec<u8>,
            payload: Vec<u8>,
        }
        let rows: Vec<Row> = sqlx::query_as(sql(format!(
            "SELECT {SUMMARY_COLUMNS}, key_id, nonce, payload FROM decision_records \
             WHERE {PERSON_RECORDS} AND anonymised_at IS NULL FOR UPDATE"
        )))
        .bind(access.project_id())
        .bind(&hash)
        .fetch_all(&mut *tx)
        .await?;
        let redacted = stored_settings(&mut tx, access.project_id(), false)
            .await?
            .redacted_fields;
        let mut personal = ContractFields::new();
        let (mut ids, mut times) = (Vec::new(), Vec::new());
        for row in rows {
            let summary = row.summary;
            let payload = self.unseal(
                summary.id,
                Sealed {
                    key_id: row.key_id,
                    nonce: row.nonce,
                    ciphertext: row.payload,
                },
            )?;
            let fields = self
                .removed_fields(&mut personal, access.project_id(), &summary, &redacted)
                .await?;
            let plaintext =
                serde_json::to_vec(&redact(payload, &fields)).map_err(|_| CipherError::Corrupt)?;
            let sealed = self.cipher.seal(summary.id, &plaintext)?;
            sqlx::query(
                "UPDATE decision_records SET key_id = $3, nonce = $4, payload = $5, anonymised_at = $6 \
                 WHERE id = $1 AND evaluated_at = $2",
            )
            .bind(summary.id)
            .bind(summary.evaluated_at)
            .bind(&sealed.key_id)
            .bind(&sealed.nonce)
            .bind(&sealed.ciphertext)
            .bind(now)
            .execute(&mut *tx)
            .await?;
            ids.push(summary.id);
            times.push(summary.evaluated_at);
        }
        // Every identifier of those records leaves the index, not only the one searched.
        sqlx::query(
            "DELETE FROM decision_record_people WHERE project_id = $1 AND (record_id, evaluated_at) \
             IN (SELECT * FROM unnest($2::uuid[], $3::timestamptz[]))",
        )
        .bind(access.project_id())
        .bind(&ids)
        .bind(&times)
        .execute(&mut *tx)
        .await?;
        donka_audit::record(
            &mut tx,
            Event::new(now, Some(access.user_id()), Action::PersonRecordsAnonymised)
                .in_project(access.project_id())
                .with_details(json!({ "records": ids.len(), "recordIds": ids })),
        )
        .await?;
        tx.commit().await?;
        Ok(ids.len() as u64)
    }

    /// Decrypts a stored payload.
    pub(crate) fn unseal(&self, id: Uuid, sealed: Sealed) -> Result<Payload, DecisionLogError> {
        let plaintext = self.cipher.open(id, &sealed)?;
        Ok(serde_json::from_slice(&plaintext).map_err(|_| CipherError::Corrupt)?)
    }

    // ----- Retention rules --------------------------------------------------

    /// The project's retention rules, running ones first (any member).
    pub async fn retention_rules(
        &self,
        access: &Access,
    ) -> Result<Vec<RetentionRule>, DecisionLogError> {
        Ok(sqlx::query_as(sql(format!(
            "SELECT {RULE_COLUMNS} FROM retention_rules WHERE project_id = $1 \
             ORDER BY running DESC, until DESC, created_at DESC"
        )))
        .bind(access.project_id())
        .bind(self.clock.now().date_naive())
        .fetch_all(&self.pool)
        .await?)
    }

    /// Documents a retention rule (owners). Audited.
    pub async fn create_retention_rule(
        &self,
        access: &Access,
        rule: NewRetentionRule,
    ) -> Result<RetentionRule, DecisionLogError> {
        access.require(Role::Owner)?;
        let now = self.clock.now();
        let description = rule.description.trim().to_owned();
        let legal_reference = rule
            .legal_reference
            .map(|text| text.trim().to_owned())
            .filter(|text| !text.is_empty());
        if description.is_empty()
            || description.chars().count() > MAX_RULE_DESCRIPTION_CHARS
            || legal_reference
                .as_ref()
                .is_some_and(|text| text.chars().count() > MAX_LEGAL_REFERENCE_CHARS)
            || rule.until < now.date_naive()
        {
            return Err(DecisionLogError::InvalidRetentionRule);
        }
        let mut tx = self.pool.begin().await?;
        lock_rules(&mut tx, access.project_id()).await?;
        let id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO retention_rules \
             (id, project_id, description, legal_reference, until, created_by, created_at) \
             VALUES ($1, $2, $3, $4, $5, $6, $7)",
        )
        .bind(id)
        .bind(access.project_id())
        .bind(&description)
        .bind(&legal_reference)
        .bind(rule.until)
        .bind(access.user_id())
        .bind(now)
        .execute(&mut *tx)
        .await?;
        donka_audit::record(
            &mut tx,
            Event::new(now, Some(access.user_id()), Action::RetentionRuleCreated)
                .in_project(access.project_id())
                .with_details(json!({
                    "ruleId": id,
                    "description": description,
                    "legalReference": legal_reference,
                    "until": rule.until,
                })),
        )
        .await?;
        let created = rule_by_id(&mut tx, access.project_id(), id, now.date_naive()).await?;
        tx.commit().await?;
        Ok(created)
    }

    /// Ends a retention rule before its date (owners). Audited.
    pub async fn end_retention_rule(
        &self,
        access: &Access,
        id: Uuid,
    ) -> Result<RetentionRule, DecisionLogError> {
        access.require(Role::Owner)?;
        let now = self.clock.now();
        let mut tx = self.pool.begin().await?;
        lock_rules(&mut tx, access.project_id()).await?;
        let rule = rule_by_id(&mut tx, access.project_id(), id, now.date_naive()).await?;
        if rule.ended_at.is_some() {
            return Ok(rule);
        }
        sqlx::query("UPDATE retention_rules SET ended_by = $2, ended_at = $3 WHERE id = $1")
            .bind(id)
            .bind(access.user_id())
            .bind(now)
            .execute(&mut *tx)
            .await?;
        donka_audit::record(
            &mut tx,
            Event::new(now, Some(access.user_id()), Action::RetentionRuleEnded)
                .in_project(access.project_id())
                .with_details(json!({
                    "ruleId": id,
                    "description": rule.description,
                    "until": rule.until,
                })),
        )
        .await?;
        let ended = rule_by_id(&mut tx, access.project_id(), id, now.date_naive()).await?;
        tx.commit().await?;
        Ok(ended)
    }

    // ----- Contests ---------------------------------------------------------

    /// The contests of a record, in the order they were recorded (any member: they are its history).
    pub async fn contests(
        &self,
        access: &Access,
        record: Uuid,
    ) -> Result<Vec<Contest>, DecisionLogError> {
        let exists: Option<Uuid> =
            sqlx::query_scalar("SELECT id FROM decision_records WHERE project_id = $1 AND id = $2")
                .bind(access.project_id())
                .bind(record)
                .fetch_optional(&self.pool)
                .await?;
        if exists.is_none() {
            return Err(DecisionLogError::RecordNotFound);
        }
        Ok(self
            .contests_of(access.project_id(), &[record])
            .await?
            .remove(&record)
            .unwrap_or_default())
    }

    async fn contests_of(
        &self,
        project: Uuid,
        records: &[Uuid],
    ) -> Result<HashMap<Uuid, Vec<Contest>>, DecisionLogError> {
        let contests: Vec<Contest> = sqlx::query_as(sql(format!(
            "SELECT {CONTEST_COLUMNS} FROM decision_contests \
             WHERE project_id = $1 AND record_id = ANY($2) ORDER BY seq"
        )))
        .bind(project)
        .bind(records)
        .fetch_all(&self.pool)
        .await?;
        let mut by_record: HashMap<Uuid, Vec<Contest>> = HashMap::new();
        for contest in contests {
            by_record
                .entry(contest.record_id)
                .or_default()
                .push(contest);
        }
        Ok(by_record)
    }

    /// Records that the person contests a decision, with their reason (owners). A record has
    /// one open contest at a time. Audited.
    pub async fn open_contest(
        &self,
        access: &Access,
        record: Uuid,
        reason: &str,
    ) -> Result<Contest, DecisionLogError> {
        access.require(Role::Owner)?;
        let reason = contest_text(reason)?;
        let now = self.clock.now();
        let mut tx = self.pool.begin().await?;
        let evaluated_at: DateTime<Utc> = sqlx::query_scalar(
            // Locks the record's contests: two owners cannot open one each.
            "SELECT evaluated_at FROM decision_records WHERE project_id = $1 AND id = $2 FOR SHARE",
        )
        .bind(access.project_id())
        .bind(record)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(DecisionLogError::RecordNotFound)?;
        sqlx::query("SELECT pg_advisory_xact_lock(hashtext('donka_contest:' || $1::text))")
            .bind(record)
            .execute(&mut *tx)
            .await?;
        let open: Option<Uuid> = sqlx::query_scalar(
            "SELECT id FROM decision_contests WHERE project_id = $1 AND record_id = $2 AND status = 'open'",
        )
        .bind(access.project_id())
        .bind(record)
        .fetch_optional(&mut *tx)
        .await?;
        if open.is_some() {
            return Err(DecisionLogError::ContestOpen);
        }
        let id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO decision_contests \
             (id, project_id, record_id, evaluated_at, status, reason, opened_by, opened_at) \
             VALUES ($1, $2, $3, $4, 'open', $5, $6, $7)",
        )
        .bind(id)
        .bind(access.project_id())
        .bind(record)
        .bind(evaluated_at)
        .bind(&reason)
        .bind(access.user_id())
        .bind(now)
        .execute(&mut *tx)
        .await?;
        donka_audit::record(
            &mut tx,
            Event::new(now, Some(access.user_id()), Action::DecisionContestOpened)
                .in_project(access.project_id())
                .with_details(json!({ "contestId": id, "recordId": record })),
        )
        .await?;
        let contest = contest_by_id(&mut tx, access.project_id(), id).await?;
        tx.commit().await?;
        Ok(contest)
    }

    /// Records the outcome of a contest's review, with what the review found (owners). A
    /// revised decision names its new outcome. Audited.
    pub async fn review_contest(
        &self,
        access: &Access,
        id: Uuid,
        outcome: ContestOutcome,
        review: &str,
    ) -> Result<Contest, DecisionLogError> {
        access.require(Role::Owner)?;
        let review = contest_text(review)?;
        let (status, revised_outcome) = match outcome {
            ContestOutcome::Upheld => (ContestStatus::Upheld, None),
            ContestOutcome::Revised { outcome } => {
                let outcome = outcome.trim().to_owned();
                if outcome.is_empty() || outcome.chars().count() > MAX_OUTCOME_CHARS {
                    return Err(DecisionLogError::InvalidContest);
                }
                (ContestStatus::Revised, Some(outcome))
            }
        };
        let now = self.clock.now();
        let mut tx = self.pool.begin().await?;
        let updated = sqlx::query(
            "UPDATE decision_contests SET status = $3, review = $4, revised_outcome = $5, \
             reviewed_by = $6, reviewed_at = $7 \
             WHERE project_id = $1 AND id = $2 AND status = 'open'",
        )
        .bind(access.project_id())
        .bind(id)
        .bind(status)
        .bind(&review)
        .bind(&revised_outcome)
        .bind(access.user_id())
        .bind(now)
        .execute(&mut *tx)
        .await?;
        let contest = contest_by_id(&mut tx, access.project_id(), id).await?;
        if updated.rows_affected() == 0 {
            return Err(DecisionLogError::ContestReviewed);
        }
        donka_audit::record(
            &mut tx,
            Event::new(now, Some(access.user_id()), Action::DecisionContestReviewed)
                .in_project(access.project_id())
                .with_details(json!({
                    "contestId": id,
                    "recordId": contest.record_id,
                    "status": status.as_str(),
                    "revisedOutcome": revised_outcome,
                })),
        )
        .await?;
        tx.commit().await?;
        Ok(contest)
    }
}

/// Anonymising and changing a project's retention rules wait for each other, so a rule
/// created a moment ago always blocks.
async fn lock_rules(conn: &mut sqlx::PgConnection, project: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query("SELECT pg_advisory_xact_lock(hashtext('donka_retention_rules:' || $1::text))")
        .bind(project)
        .execute(conn)
        .await?;
    Ok(())
}

async fn rule_by_id(
    conn: &mut sqlx::PgConnection,
    project: Uuid,
    id: Uuid,
    today: NaiveDate,
) -> Result<RetentionRule, DecisionLogError> {
    sqlx::query_as(sql(format!(
        "SELECT {RULE_COLUMNS} FROM retention_rules WHERE project_id = $1 AND id = $3"
    )))
    .bind(project)
    .bind(today)
    .bind(id)
    .fetch_optional(conn)
    .await?
    .ok_or(DecisionLogError::RetentionRuleNotFound)
}

async fn contest_by_id(
    conn: &mut sqlx::PgConnection,
    project: Uuid,
    id: Uuid,
) -> Result<Contest, DecisionLogError> {
    sqlx::query_as(sql(format!(
        "SELECT {CONTEST_COLUMNS} FROM decision_contests WHERE project_id = $1 AND id = $2"
    )))
    .bind(project)
    .bind(id)
    .fetch_optional(conn)
    .await?
    .ok_or(DecisionLogError::ContestNotFound)
}

fn contest_text(text: &str) -> Result<String, DecisionLogError> {
    let text = text.trim();
    if text.is_empty() || text.chars().count() > MAX_CONTEST_TEXT_CHARS {
        return Err(DecisionLogError::InvalidContest);
    }
    Ok(text.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: &str = "AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8=";

    #[test]
    fn an_identifier_hashes_the_same_however_it_is_written() {
        let key = PersonKey::from_base64(KEY).unwrap();
        let hash = key.hash("CM-1234.567 ab").unwrap();
        assert_eq!(key.hash(" cm1234567AB ").unwrap(), hash);
        assert_ne!(key.hash("cm1234567ac").unwrap(), hash);
        assert_eq!(key.hash(" -. "), None);
        let other = PersonKey::from_base64("ICEiIyQlJicoKSorLC0uLzAxMjM0NTY3ODk6Ozw9Pj8=").unwrap();
        assert_ne!(other.hash("cm1234567ab").unwrap(), hash);
        assert!(PersonKey::from_base64("too-short").is_err());
    }

    #[test]
    fn identifiers_are_read_through_lists_and_numbers() {
        let key = PersonKey::from_base64(KEY).unwrap();
        let input = json!({
            "applicant": { "nationalId": "A1", "phone": 237_650_000_000_u64 },
            "guarantors": [{ "nationalId": "B2" }, { "nationalId": null }]
        });
        let paths = vec![
            "applicant.nationalId".to_owned(),
            "applicant.phone".to_owned(),
            "guarantors.nationalId".to_owned(),
            "missing.field".to_owned(),
        ];
        let hashes = key.hashes(&input, &paths);
        let expected: BTreeSet<Vec<u8>> = ["a1", "237650000000", "b2"]
            .iter()
            .map(|value| key.hash(value).unwrap())
            .collect();
        assert_eq!(hashes, expected);
    }
}
