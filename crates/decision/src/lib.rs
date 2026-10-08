//! Decision module: the decisions of a project (JDM graphs) and their drafts.
//!
//! A decision is addressed by a key that graphs use to call each other
//! (`person-score`, `bureau/normalize`). Its `content` is the working draft the
//! editor autosaves; every save names the revision it started from, so two
//! people editing the same draft get [`DecisionError::Conflict`] instead of one
//! silently overwriting the other.
//!
//! "Save version" snapshots the draft as an immutable, numbered version
//! ([`versions`]); restoring an old version adds a new one. Deleting a decision
//! keeps its history.
//!
//! Test scenarios ([`scenarios`]) pin what a decision should answer; every
//! version saved runs all of the project's and keeps the results ([`testing`]).

use chrono::{DateTime, Utc};
use donka_audit::{Action, Event};
use donka_db::PgPool;
use donka_engine::{Bundle, DecisionRuntime, RuntimeError};
use donka_project::{authorize_change, Access, ProjectError, Role};
use donka_shared::clock::Clock;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::sync::Arc;
use uuid::Uuid;

mod compare;
mod scenarios;
mod testing;
mod versions;
pub use compare::{Match, Mismatch};
pub use scenarios::{Scenario, ScenarioFields, MAX_SCENARIO_NAME_CHARS};
pub use testing::{TestResult, TestStatus, TestSummary};
pub use versions::{
    FrozenVersion, ReleaseSnapshot, SavedVersion, Version, VersionSummary, MAX_MESSAGE_CHARS,
};

pub const MAX_KEY_CHARS: usize = 120;
/// The key rule as a regular expression, for API clients: lowercase words with
/// single hyphens, folders separated by `/` (the check below is the same rule).
pub const KEY_PATTERN: &str = "^[a-z][a-z0-9]*(-[a-z0-9]+)*(/[a-z][a-z0-9]*(-[a-z0-9]+)*)*$";

/// A decision as lists show it, without its content.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct DecisionSummary {
    pub id: Uuid,
    pub key: String,
    pub revision: i32,
    pub updated_at: DateTime<Utc>,
    pub updated_by: Uuid,
    /// The number of its latest version; none before the first "Save version".
    pub latest_version: Option<i32>,
    /// Whether the draft has changed since that version (always, before the first).
    pub changed_since_version: bool,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Decision {
    #[sqlx(flatten)]
    pub summary: DecisionSummary,
    /// The JDM graph.
    pub content: Value,
}

/// A decision's columns with where its draft stands against its latest version.
/// The contents are compared, not revisions: a save that changes nothing (the
/// same graph, keys in another order) leaves the draft unchanged.
const SUMMARY: &str = "d.id, d.key, d.revision, d.updated_at, d.updated_by, \
     v.number AS latest_version, v.content IS DISTINCT FROM d.content AS changed_since_version";
/// The decisions that exist (not deleted), each with its latest version.
const FROM: &str = "FROM decisions d LEFT JOIN LATERAL (\
     SELECT number, content FROM decision_versions WHERE decision_id = d.id \
     ORDER BY number DESC LIMIT 1) v ON true \
     WHERE d.deleted_at IS NULL AND d.project_id = $1";

#[derive(Debug, thiserror::Error)]
pub enum DecisionError {
    /// No such decision in this project: also the answer for another project's decision.
    #[error("decision not found")]
    NotFound,
    #[error("keys are lowercase words with single hyphens, folders separated by '/', at most {MAX_KEY_CHARS} characters")]
    InvalidKey,
    #[error("a decision with this key already exists in the project")]
    KeyTaken,
    #[error("decision '{key}' is not a valid decision model: {message}")]
    InvalidContent { key: String, message: String },
    /// Someone saved the draft since this person loaded it; `current` is what they saved.
    #[error("the draft changed since it was loaded")]
    Conflict { current: Box<DecisionSummary> },
    #[error("no such version of this decision")]
    VersionNotFound,
    #[error("messages are 1 to {MAX_MESSAGE_CHARS} characters")]
    InvalidMessage,
    /// The draft is the same as the latest version: there is nothing to save.
    #[error("nothing changed since version {0}")]
    Unchanged(i32),
    /// No such scenario in this project.
    #[error("scenario not found")]
    ScenarioNotFound,
    #[error("another scenario of this decision has this name")]
    ScenarioNameTaken,
    /// The named field of a scenario is invalid: a name of 1 to
    /// {MAX_SCENARIO_NAME_CHARS} characters, an input and an expected output that are objects.
    #[error("invalid scenario {0}")]
    InvalidScenario(&'static str),
    /// The input contract (the input node's JSON Schema) cannot be used; why, in words.
    #[error("{0}")]
    InvalidContract(String),
    #[error(transparent)]
    Project(#[from] ProjectError),
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

#[derive(Clone)]
pub struct Decisions {
    pool: PgPool,
    clock: Arc<dyn Clock>,
    /// Runs the test scenarios when a version is saved.
    runtime: Arc<dyn DecisionRuntime>,
}

impl Decisions {
    pub fn new(pool: PgPool, clock: Arc<dyn Clock>, runtime: Arc<dyn DecisionRuntime>) -> Self {
        Self {
            pool,
            clock,
            runtime,
        }
    }

    /// Every decision of the project, by key (any member).
    pub async fn list(&self, access: &Access) -> Result<Vec<DecisionSummary>, DecisionError> {
        Ok(
            sqlx::query_as(sql(format!("SELECT {SUMMARY} {FROM} ORDER BY d.key")))
                .bind(access.project_id())
                .fetch_all(&self.pool)
                .await?,
        )
    }

    /// One decision with its draft (any member).
    pub async fn get(&self, access: &Access, id: Uuid) -> Result<Decision, DecisionError> {
        fetch(&mut *self.pool.acquire().await?, access, id).await
    }

    /// A new decision; empty when no content is given (editors).
    pub async fn create(
        &self,
        access: &Access,
        key: &str,
        content: Option<Value>,
    ) -> Result<Decision, DecisionError> {
        let key = check_key(key)?;
        let content = content.unwrap_or_else(empty_graph);
        check_content(&key, &content)?;
        let now = self.clock.now();
        let id = Uuid::new_v4();

        let mut tx = self.pool.begin().await?;
        authorize_change(&mut tx, access, Role::Editor).await?;
        let inserted = sqlx::query(
            "INSERT INTO decisions (id, project_id, key, content, created_by, created_at, updated_by, updated_at) \
             VALUES ($1, $2, $3, $4, $5, $6, $5, $6) \
             ON CONFLICT (project_id, key) WHERE deleted_at IS NULL DO NOTHING",
        )
        .bind(id)
        .bind(access.project_id())
        .bind(&key)
        .bind(&content)
        .bind(access.user_id())
        .bind(now)
        .execute(&mut *tx)
        .await?;
        if inserted.rows_affected() == 0 {
            return Err(DecisionError::KeyTaken);
        }
        donka_audit::record(
            &mut tx,
            Event::new(now, Some(access.user_id()), Action::DecisionCreated)
                .in_project(access.project_id())
                .with_details(json!({ "key": key })),
        )
        .await?;
        tx.commit().await?;

        Ok(Decision {
            summary: DecisionSummary {
                id,
                key,
                revision: 1,
                updated_at: now,
                updated_by: access.user_id(),
                latest_version: None,
                changed_since_version: true,
            },
            content,
        })
    }

    /// Saves the draft if nobody saved it since `base_revision` (editors).
    ///
    /// Autosaves of a draft are not audit events: the draft is a working copy,
    /// and what leaves it (a version, a release) is recorded when it does.
    pub async fn save_draft(
        &self,
        access: &Access,
        id: Uuid,
        content: Value,
        base_revision: i32,
    ) -> Result<DecisionSummary, DecisionError> {
        let mut tx = self.pool.begin().await?;
        authorize_change(&mut tx, access, Role::Editor).await?;
        let current = lock_at(&mut tx, access, id, base_revision).await?;
        check_content(&current.key, &content)?;
        write_draft(&mut tx, access, id, &content, self.clock.now()).await?;
        let saved = fetch(&mut tx, access, id).await?;
        tx.commit().await?;
        Ok(saved.summary)
    }

    /// Deletes a decision (editors). It disappears from the project, its key is
    /// free again, and its versions stay in the history.
    pub async fn delete(&self, access: &Access, id: Uuid) -> Result<(), DecisionError> {
        let mut tx = self.pool.begin().await?;
        authorize_change(&mut tx, access, Role::Editor).await?;
        // Kept, marked deleted: its versions stay in the history.
        let deleted: Option<(String,)> = sqlx::query_as(
            "UPDATE decisions SET deleted_at = $3, deleted_by = $4 \
             WHERE project_id = $1 AND id = $2 AND deleted_at IS NULL RETURNING key",
        )
        .bind(access.project_id())
        .bind(id)
        .bind(self.clock.now())
        .bind(access.user_id())
        .fetch_optional(&mut *tx)
        .await?;
        let (key,) = deleted.ok_or(DecisionError::NotFound)?;
        donka_audit::record(
            &mut tx,
            Event::new(
                self.clock.now(),
                Some(access.user_id()),
                Action::DecisionDeleted,
            )
            .in_project(access.project_id())
            .with_details(json!({ "key": key })),
        )
        .await?;
        tx.commit().await?;
        Ok(())
    }

    /// Every draft of the project, keyed as graphs reference each other, with
    /// `edited` (the editor's unsaved content of decision `id`) in place of its
    /// saved draft, and that decision's key: what the simulator evaluates (any member).
    pub async fn bundle(
        &self,
        access: &Access,
        id: Uuid,
        edited: Option<Value>,
    ) -> Result<(Bundle, String), DecisionError> {
        let rows: Vec<(Uuid, String, Value)> = sqlx::query_as(
            "SELECT id, key, content FROM decisions WHERE project_id = $1 AND deleted_at IS NULL",
        )
        .bind(access.project_id())
        .fetch_all(&self.pool)
        .await?;
        let mut target = None;
        let mut contents = BTreeMap::new();
        let mut edited = edited;
        for (row_id, key, content) in rows {
            let content = if row_id == id {
                target = Some(key.clone());
                edited.take().unwrap_or(content)
            } else {
                content
            };
            contents.insert(key, content);
        }
        let key = target.ok_or(DecisionError::NotFound)?;
        let bundle = Bundle::from_json(contents).map_err(invalid_content)?;
        Ok((bundle, key))
    }
}

type Tx<'a> = sqlx::Transaction<'a, sqlx::Postgres>;

/// A query assembled from this module's constant fragments ([`SUMMARY`], [`FROM`]):
/// no value from outside ever reaches the text, only bound parameters.
fn sql(text: String) -> sqlx::AssertSqlSafe<String> {
    sqlx::AssertSqlSafe(text)
}

/// One decision with its draft and where the draft stands.
async fn fetch(
    conn: &mut sqlx::PgConnection,
    access: &Access,
    id: Uuid,
) -> Result<Decision, DecisionError> {
    sqlx::query_as(sql(format!(
        "SELECT {SUMMARY}, d.content {FROM} AND d.id = $2"
    )))
    .bind(access.project_id())
    .bind(id)
    .fetch_optional(conn)
    .await?
    .ok_or(DecisionError::NotFound)
}

/// Locks the decision for a change of its draft, which must still be at
/// `base_revision`: otherwise someone saved since, and this is a conflict.
async fn lock_at(
    tx: &mut Tx<'_>,
    access: &Access,
    id: Uuid,
    base_revision: i32,
) -> Result<DecisionSummary, DecisionError> {
    let current: DecisionSummary = sqlx::query_as(sql(format!(
        "SELECT {SUMMARY} {FROM} AND d.id = $2 FOR UPDATE OF d"
    )))
    .bind(access.project_id())
    .bind(id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(DecisionError::NotFound)?;
    if current.revision != base_revision {
        return Err(DecisionError::Conflict {
            current: Box::new(current),
        });
    }
    Ok(current)
}

/// Replaces the draft (the caller holds the lock), one revision up.
async fn write_draft(
    tx: &mut Tx<'_>,
    access: &Access,
    id: Uuid,
    content: &Value,
    now: DateTime<Utc>,
) -> Result<(), DecisionError> {
    sqlx::query(
        "UPDATE decisions SET content = $1, revision = revision + 1, updated_by = $2, updated_at = $3 \
         WHERE id = $4",
    )
    .bind(content)
    .bind(access.user_id())
    .bind(now)
    .bind(id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// A decision with no node yet: what the editor opens on.
fn empty_graph() -> Value {
    json!({ "nodes": [], "edges": [] })
}

fn check_key(key: &str) -> Result<String, DecisionError> {
    let key = key.trim();
    let valid = !key.is_empty()
        && key.chars().count() <= MAX_KEY_CHARS
        && key.split('/').all(|segment| {
            let mut chars = segment.chars();
            chars.next().is_some_and(|first| first.is_ascii_lowercase())
                && segment
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
                && !segment.ends_with('-')
                && !segment.contains("--")
        });
    if valid {
        Ok(key.to_owned())
    } else {
        Err(DecisionError::InvalidKey)
    }
}

/// The content must be a decision model the engine accepts, so a broken graph
/// is refused when it is saved, not when an applicant is scored.
fn check_content(key: &str, content: &Value) -> Result<(), DecisionError> {
    Bundle::from_json(BTreeMap::from([(key.to_owned(), content.clone())]))
        .map(drop)
        .map_err(invalid_content)
}

fn invalid_content(err: RuntimeError) -> DecisionError {
    match err {
        RuntimeError::InvalidContent { key, message } => {
            DecisionError::InvalidContent { key, message }
        }
        other => DecisionError::InvalidContent {
            key: String::new(),
            message: other.to_string(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_are_words_and_folders() {
        for good in [
            "person-score",
            "bureau/normalize",
            "a",
            "v2/limits/card-2026",
        ] {
            assert_eq!(check_key(good).unwrap(), good, "{good}");
        }
        for bad in [
            "", "Person", "2fa", "a--b", "a-", "/a", "a/", "a//b", "a b", "a_b", "a/-b",
        ] {
            assert!(check_key(bad).is_err(), "{bad}");
        }
        assert!(check_key(&"a".repeat(MAX_KEY_CHARS + 1)).is_err());
    }

    #[test]
    fn an_empty_graph_is_a_valid_decision() {
        check_content("new", &empty_graph()).unwrap();
        assert!(matches!(
            check_content("broken", &json!({ "nodes": "no" })),
            Err(DecisionError::InvalidContent { .. })
        ));
    }
}
