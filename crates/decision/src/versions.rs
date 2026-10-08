//! Immutable versions of a decision.
//!
//! "Save version" snapshots the draft, numbered 1, 2, 3… per decision, with its
//! author, time and message. Nothing ever changes a version: restoring an old
//! one adds a new version with its content and puts that content in the draft.
//! Each new version runs the project's test scenarios ([`crate::testing`]).

use chrono::{DateTime, Utc};
use donka_audit::{Action, Event};
use donka_engine::contract;
use donka_project::{authorize_change, Access, Role};
use donka_shared::page::{Page, PageRequest};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::testing::{latest_siblings, TestSummary, SUMMARY_JOIN};
use crate::{
    fetch, lock_at, sql, write_draft, Decision, DecisionError, DecisionSummary, Decisions, Tx,
};

pub const MAX_MESSAGE_CHARS: usize = 500;

/// A version as the history lists it, without its content.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct VersionSummary {
    pub number: i32,
    pub message: String,
    pub created_at: DateTime<Utc>,
    pub created_by: Uuid,
    /// The older version this one restores, when it does.
    pub restored_from: Option<i32>,
    /// How the project's test scenarios went on this version.
    #[sqlx(flatten)]
    pub tests: TestSummary,
}

/// A version's columns with its test counts (`v` is the version).
const VERSION_COLUMNS: &str = "v.number, v.message, v.created_at, v.created_by, v.restored_from, \
     coalesce(t.passed, 0) AS passed, coalesce(t.failed, 0) AS failed, coalesce(t.errors, 0) AS errors";

/// A version just saved, with what its author should know about its input contract.
#[derive(Debug, Clone)]
pub struct SavedVersion {
    pub summary: VersionSummary,
    /// Fields rules read but the contract does not declare, and required fields no rule reads.
    pub warnings: Vec<contract::Warning>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Version {
    #[sqlx(flatten)]
    pub summary: VersionSummary,
    /// The JDM graph as it was saved.
    pub content: Value,
}

impl Decisions {
    /// Saves the draft, as it is at `base_revision`, as the next version (editors).
    /// The editor sends the revision it shows, so the version is what the person saw.
    /// A version's input contract must be a usable JSON Schema: the engine would ignore a
    /// broken one, and every request would then go through unchecked.
    pub async fn save_version(
        &self,
        access: &Access,
        id: Uuid,
        base_revision: i32,
        message: &str,
    ) -> Result<SavedVersion, DecisionError> {
        let message = check_message(message)?;
        let mut tx = self.pool.begin().await?;
        authorize_change(&mut tx, access, Role::Editor).await?;
        let current = lock_at(&mut tx, access, id, base_revision).await?;
        if let (Some(latest), false) = (current.latest_version, current.changed_since_version) {
            return Err(DecisionError::Unchanged(latest));
        }
        let (content,): (Value,) = sqlx::query_as("SELECT content FROM decisions WHERE id = $1")
            .bind(id)
            .fetch_one(&mut *tx)
            .await?;
        let schema = contract::input_schema(&content)
            .map_err(|err| DecisionError::InvalidContract(err.to_string()))?;
        let mut version = self
            .insert_version(&mut tx, access, &current, &content, &message, None)
            .await?;
        version.tests = self
            .run_tests(&mut tx, access, &current, version.number, &content)
            .await?;
        donka_audit::record(
            &mut tx,
            Event::new(
                version.created_at,
                Some(access.user_id()),
                Action::DecisionVersionSaved,
            )
            .in_project(access.project_id())
            .with_details(
                json!({ "key": current.key, "version": version.number, "message": message }),
            ),
        )
        .await?;
        let warnings = match &schema {
            Some(schema) => {
                let mut contents = latest_siblings(&mut tx, access, current.id).await?;
                contents.insert(current.key.clone(), content);
                contract::check(&contents, &current.key, schema)
            }
            None => Vec::new(),
        };
        tx.commit().await?;
        Ok(SavedVersion {
            summary: version,
            warnings,
        })
    }

    /// The decision's versions, newest first (any member).
    pub async fn versions(
        &self,
        access: &Access,
        id: Uuid,
        page: PageRequest,
    ) -> Result<Page<VersionSummary>, DecisionError> {
        let mut conn = self.pool.acquire().await?;
        // The decision must exist in this project; its history is read with it.
        fetch(&mut conn, access, id).await?;
        let items = sqlx::query_as(sql(format!(
            "SELECT {VERSION_COLUMNS} FROM decision_versions v {SUMMARY_JOIN} \
             WHERE v.decision_id = $1 ORDER BY v.number DESC LIMIT $2 OFFSET $3"
        )))
        .bind(id)
        .bind(page.limit)
        .bind(page.offset)
        .fetch_all(&mut *conn)
        .await?;
        let (total,): (i64,) =
            sqlx::query_as("SELECT count(*) FROM decision_versions WHERE decision_id = $1")
                .bind(id)
                .fetch_one(&mut *conn)
                .await?;
        Ok(Page { items, total })
    }

    /// One version with its content (any member).
    pub async fn version(
        &self,
        access: &Access,
        id: Uuid,
        number: i32,
    ) -> Result<Version, DecisionError> {
        let mut conn = self.pool.acquire().await?;
        fetch(&mut conn, access, id).await?;
        sqlx::query_as(sql(format!(
            "SELECT {VERSION_COLUMNS}, v.content FROM decision_versions v {SUMMARY_JOIN} \
             WHERE v.decision_id = $1 AND v.number = $2"
        )))
        .bind(id)
        .bind(number)
        .fetch_optional(&mut *conn)
        .await?
        .ok_or(DecisionError::VersionNotFound)
    }

    /// Restores version `number` (editors): its content becomes the draft and a
    /// new version, so the history only grows. The draft must still be at
    /// `base_revision`, so nobody's unsaved work is replaced unseen. Returns the
    /// decision with its new draft, and the version the restore added.
    pub async fn restore(
        &self,
        access: &Access,
        id: Uuid,
        number: i32,
        base_revision: i32,
    ) -> Result<(Decision, VersionSummary), DecisionError> {
        let mut tx = self.pool.begin().await?;
        authorize_change(&mut tx, access, Role::Editor).await?;
        lock_at(&mut tx, access, id, base_revision).await?;
        let old: Option<(Value,)> = sqlx::query_as(
            "SELECT content FROM decision_versions WHERE decision_id = $1 AND number = $2",
        )
        .bind(id)
        .bind(number)
        .fetch_optional(&mut *tx)
        .await?;
        let (content,) = old.ok_or(DecisionError::VersionNotFound)?;

        let now = self.clock.now();
        write_draft(&mut tx, access, id, &content, now).await?;
        let draft = fetch(&mut tx, access, id).await?.summary;
        let message = format!("Restored version {number}");
        let mut version = self
            .insert_version(&mut tx, access, &draft, &content, &message, Some(number))
            .await?;
        version.tests = self
            .run_tests(&mut tx, access, &draft, version.number, &content)
            .await?;
        donka_audit::record(
            &mut tx,
            Event::new(now, Some(access.user_id()), Action::DecisionVersionRestored)
                .in_project(access.project_id())
                .with_details(
                    json!({ "key": draft.key, "version": version.number, "from": number }),
                ),
        )
        .await?;
        let decision = fetch(&mut tx, access, id).await?;
        tx.commit().await?;
        Ok((decision, version))
    }

    /// Adds the next version of `decision` (the caller holds its lock).
    async fn insert_version(
        &self,
        tx: &mut Tx<'_>,
        access: &Access,
        decision: &DecisionSummary,
        content: &Value,
        message: &str,
        restored_from: Option<i32>,
    ) -> Result<VersionSummary, DecisionError> {
        let number = decision.latest_version.unwrap_or(0) + 1;
        let created_at = self.clock.now();
        sqlx::query(
            "INSERT INTO decision_versions \
             (decision_id, number, content, message, draft_revision, restored_from, created_by, created_at) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
        )
        .bind(decision.id)
        .bind(number)
        .bind(content)
        .bind(message)
        .bind(decision.revision)
        .bind(restored_from)
        .bind(access.user_id())
        .bind(created_at)
        .execute(&mut **tx)
        .await?;
        Ok(VersionSummary {
            number,
            message: message.to_owned(),
            created_at,
            created_by: access.user_id(),
            restored_from,
            tests: TestSummary::default(),
        })
    }
}

fn check_message(message: &str) -> Result<String, DecisionError> {
    let message = message.trim();
    if message.is_empty() || message.chars().count() > MAX_MESSAGE_CHARS {
        Err(DecisionError::InvalidMessage)
    } else {
        Ok(message.to_owned())
    }
}

/// A decision's latest version as a release freezes it.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct FrozenVersion {
    pub decision_id: Uuid,
    pub key: String,
    pub number: i32,
    pub content: Value,
    /// How the project's scenarios went on this version.
    #[sqlx(flatten)]
    pub tests: TestSummary,
}

/// What a release would freeze now: every decision's latest version, and the
/// decisions that have none yet (which block a release).
#[derive(Debug, Clone, Default)]
pub struct ReleaseSnapshot {
    pub versions: Vec<FrozenVersion>,
    pub unversioned: Vec<String>,
}

#[derive(sqlx::FromRow)]
struct SnapshotRow {
    decision_id: Uuid,
    key: String,
    number: Option<i32>,
    content: Option<Value>,
    #[sqlx(flatten)]
    tests: TestSummary,
}

impl Decisions {
    /// The latest version of every decision of the project, by key (any member).
    pub async fn release_snapshot(
        &self,
        access: &Access,
    ) -> Result<ReleaseSnapshot, DecisionError> {
        let rows: Vec<SnapshotRow> = sqlx::query_as(sql(format!(
            "SELECT d.id AS decision_id, d.key, v.number, v.content, \
             coalesce(t.passed, 0) AS passed, coalesce(t.failed, 0) AS failed, \
             coalesce(t.errors, 0) AS errors \
             FROM decisions d LEFT JOIN LATERAL (\
             SELECT decision_id, number, content FROM decision_versions WHERE decision_id = d.id \
             ORDER BY number DESC LIMIT 1) v ON true {SUMMARY_JOIN} \
             WHERE d.project_id = $1 AND d.deleted_at IS NULL ORDER BY d.key"
        )))
        .bind(access.project_id())
        .fetch_all(&self.pool)
        .await?;
        let mut snapshot = ReleaseSnapshot::default();
        for row in rows {
            match (row.number, row.content) {
                (Some(number), Some(content)) => snapshot.versions.push(FrozenVersion {
                    decision_id: row.decision_id,
                    key: row.key,
                    number,
                    content,
                    tests: row.tests,
                }),
                _ => snapshot.unversioned.push(row.key),
            }
        }
        Ok(snapshot)
    }
}
