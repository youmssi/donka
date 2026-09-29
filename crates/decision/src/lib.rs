//! Decision module: the decisions of a project (JDM graphs) and their drafts.
//!
//! A decision is addressed by a key that graphs use to call each other
//! (`person-score`, `bureau/normalize`). Its `content` is the working draft the
//! editor autosaves; every save names the revision it started from, so two
//! people editing the same draft get [`DecisionError::Conflict`] instead of one
//! silently overwriting the other. Immutable versions come with DNK-9.

use chrono::{DateTime, Utc};
use donka_audit::{Action, Event};
use donka_db::PgPool;
use donka_engine::{Bundle, RuntimeError};
use donka_project::{authorize_change, Access, ProjectError, Role};
use donka_shared::clock::Clock;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::sync::Arc;
use uuid::Uuid;

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
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Decision {
    pub id: Uuid,
    pub key: String,
    /// The JDM graph.
    pub content: Value,
    pub revision: i32,
    pub updated_at: DateTime<Utc>,
    pub updated_by: Uuid,
}

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
    #[error(transparent)]
    Project(#[from] ProjectError),
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

#[derive(Clone)]
pub struct Decisions {
    pool: PgPool,
    clock: Arc<dyn Clock>,
}

impl Decisions {
    pub fn new(pool: PgPool, clock: Arc<dyn Clock>) -> Self {
        Self { pool, clock }
    }

    /// Every decision of the project, by key (any member).
    pub async fn list(&self, access: &Access) -> Result<Vec<DecisionSummary>, DecisionError> {
        Ok(sqlx::query_as(
            "SELECT id, key, revision, updated_at, updated_by FROM decisions \
             WHERE project_id = $1 ORDER BY key",
        )
        .bind(access.project_id())
        .fetch_all(&self.pool)
        .await?)
    }

    /// One decision with its draft (any member).
    pub async fn get(&self, access: &Access, id: Uuid) -> Result<Decision, DecisionError> {
        sqlx::query_as(
            "SELECT id, key, content, revision, updated_at, updated_by FROM decisions \
             WHERE project_id = $1 AND id = $2",
        )
        .bind(access.project_id())
        .bind(id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(DecisionError::NotFound)
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
             VALUES ($1, $2, $3, $4, $5, $6, $5, $6) ON CONFLICT (project_id, key) DO NOTHING",
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
            id,
            key,
            content,
            revision: 1,
            updated_at: now,
            updated_by: access.user_id(),
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
    ) -> Result<Decision, DecisionError> {
        let mut tx = self.pool.begin().await?;
        authorize_change(&mut tx, access, Role::Editor).await?;
        let current: DecisionSummary = sqlx::query_as(
            "SELECT id, key, revision, updated_at, updated_by FROM decisions \
             WHERE project_id = $1 AND id = $2 FOR UPDATE",
        )
        .bind(access.project_id())
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(DecisionError::NotFound)?;
        if current.revision != base_revision {
            return Err(DecisionError::Conflict {
                current: Box::new(current),
            });
        }
        check_content(&current.key, &content)?;

        let now = self.clock.now();
        let revision = current.revision + 1;
        sqlx::query(
            "UPDATE decisions SET content = $1, revision = $2, updated_by = $3, updated_at = $4 \
             WHERE id = $5",
        )
        .bind(&content)
        .bind(revision)
        .bind(access.user_id())
        .bind(now)
        .bind(id)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;

        Ok(Decision {
            id,
            key: current.key,
            content,
            revision,
            updated_at: now,
            updated_by: access.user_id(),
        })
    }

    /// Deletes a decision and its draft (editors).
    pub async fn delete(&self, access: &Access, id: Uuid) -> Result<(), DecisionError> {
        let mut tx = self.pool.begin().await?;
        authorize_change(&mut tx, access, Role::Editor).await?;
        let deleted: Option<(String,)> =
            sqlx::query_as("DELETE FROM decisions WHERE project_id = $1 AND id = $2 RETURNING key")
                .bind(access.project_id())
                .bind(id)
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
        let rows: Vec<(Uuid, String, Value)> =
            sqlx::query_as("SELECT id, key, content FROM decisions WHERE project_id = $1")
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
