//! Test scenarios: an input to one decision and the output it should give.
//!
//! Scenarios belong to the project; each targets one decision. Every version
//! saved runs all of them ([`crate::testing`]).

use chrono::{DateTime, Utc};
use donka_audit::{Action, Event};
use donka_project::{authorize_change, Access, Role};
use donka_shared::page::{Page, PageRequest};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::compare::Match;
use crate::{fetch, DecisionError, Decisions, Tx};

pub const MAX_SCENARIO_NAME_CHARS: usize = 200;

#[derive(Debug, Clone)]
pub struct Scenario {
    pub id: Uuid,
    pub decision_id: Uuid,
    pub decision_key: String,
    pub name: String,
    /// The context the decision is evaluated with.
    pub input: Value,
    /// The output it should give, exactly or in part ([`Match`]).
    pub expected: Value,
    pub match_mode: Match,
    pub created_at: DateTime<Utc>,
    pub created_by: Uuid,
    pub updated_at: DateTime<Utc>,
    pub updated_by: Uuid,
}

/// What a person writes in a scenario.
#[derive(Debug, Clone)]
pub struct ScenarioFields {
    pub name: String,
    pub input: Value,
    pub expected: Value,
    pub match_mode: Match,
}

#[derive(sqlx::FromRow)]
struct Row {
    id: Uuid,
    decision_id: Uuid,
    decision_key: String,
    name: String,
    input: Value,
    expected: Value,
    #[sqlx(rename = "match")]
    match_mode: String,
    created_at: DateTime<Utc>,
    created_by: Uuid,
    updated_at: DateTime<Utc>,
    updated_by: Uuid,
}

impl From<Row> for Scenario {
    fn from(row: Row) -> Self {
        Self {
            id: row.id,
            decision_id: row.decision_id,
            decision_key: row.decision_key,
            name: row.name,
            input: row.input,
            expected: row.expected,
            // The table only holds the two modes (CHECK constraint).
            match_mode: Match::parse(&row.match_mode).unwrap_or(Match::Exact),
            created_at: row.created_at,
            created_by: row.created_by,
            updated_at: row.updated_at,
            updated_by: row.updated_by,
        }
    }
}

/// The scenarios of decisions that still exist, with their decision's key.
const SELECT: &str = "SELECT s.id, s.decision_id, d.key AS decision_key, s.name, s.input, \
     s.expected, s.match, s.created_at, s.created_by, s.updated_at, s.updated_by \
     FROM test_scenarios s JOIN decisions d ON d.id = s.decision_id \
     WHERE s.project_id = $1 AND d.deleted_at IS NULL";

impl Decisions {
    /// The project's scenarios by decision and name, or one decision's (any member).
    pub async fn scenarios(
        &self,
        access: &Access,
        decision: Option<Uuid>,
        page: PageRequest,
    ) -> Result<Page<Scenario>, DecisionError> {
        let mut conn = self.pool.acquire().await?;
        let rows: Vec<Row> = sqlx::query_as(crate::sql(format!(
            "{SELECT} AND ($2::uuid IS NULL OR s.decision_id = $2) \
             ORDER BY d.key, lower(s.name) LIMIT $3 OFFSET $4"
        )))
        .bind(access.project_id())
        .bind(decision)
        .bind(page.limit)
        .bind(page.offset)
        .fetch_all(&mut *conn)
        .await?;
        let (total,): (i64,) = sqlx::query_as(
            "SELECT count(*) FROM test_scenarios s JOIN decisions d ON d.id = s.decision_id \
             WHERE s.project_id = $1 AND d.deleted_at IS NULL \
             AND ($2::uuid IS NULL OR s.decision_id = $2)",
        )
        .bind(access.project_id())
        .bind(decision)
        .fetch_one(&mut *conn)
        .await?;
        Ok(Page {
            items: rows.into_iter().map(Scenario::from).collect(),
            total,
        })
    }

    /// One scenario (any member).
    pub async fn scenario(&self, access: &Access, id: Uuid) -> Result<Scenario, DecisionError> {
        find(&mut *self.pool.acquire().await?, access, id).await
    }

    /// A new scenario for decision `decision` (editors).
    pub async fn create_scenario(
        &self,
        access: &Access,
        decision: Uuid,
        fields: ScenarioFields,
    ) -> Result<Scenario, DecisionError> {
        let fields = check(fields)?;
        let now = self.clock.now();
        let id = Uuid::new_v4();
        let mut tx = self.pool.begin().await?;
        authorize_change(&mut tx, access, Role::Editor).await?;
        let target = fetch(&mut tx, access, decision).await?.summary;
        sqlx::query(
            "INSERT INTO test_scenarios \
             (id, project_id, decision_id, name, input, expected, match, created_by, created_at, updated_by, updated_at) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $8, $9)",
        )
        .bind(id)
        .bind(access.project_id())
        .bind(decision)
        .bind(&fields.name)
        .bind(&fields.input)
        .bind(&fields.expected)
        .bind(fields.match_mode.as_str())
        .bind(access.user_id())
        .bind(now)
        .execute(&mut *tx)
        .await
        .map_err(name_taken)?;
        audit(
            &mut tx,
            access,
            now,
            Action::ScenarioCreated,
            &fields.name,
            &target.key,
        )
        .await?;
        let created = find(&mut tx, access, id).await?;
        tx.commit().await?;
        Ok(created)
    }

    /// Changes a scenario's name, input, expected output or match (editors).
    pub async fn update_scenario(
        &self,
        access: &Access,
        id: Uuid,
        fields: ScenarioFields,
    ) -> Result<Scenario, DecisionError> {
        let fields = check(fields)?;
        let now = self.clock.now();
        let mut tx = self.pool.begin().await?;
        authorize_change(&mut tx, access, Role::Editor).await?;
        let current = find(&mut tx, access, id).await?;
        sqlx::query(
            "UPDATE test_scenarios SET name = $1, input = $2, expected = $3, match = $4, \
             updated_by = $5, updated_at = $6 WHERE id = $7",
        )
        .bind(&fields.name)
        .bind(&fields.input)
        .bind(&fields.expected)
        .bind(fields.match_mode.as_str())
        .bind(access.user_id())
        .bind(now)
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(name_taken)?;
        audit(
            &mut tx,
            access,
            now,
            Action::ScenarioUpdated,
            &fields.name,
            &current.decision_key,
        )
        .await?;
        let updated = find(&mut tx, access, id).await?;
        tx.commit().await?;
        Ok(updated)
    }

    /// Deletes a scenario (editors). Results already recorded keep it as it ran.
    pub async fn delete_scenario(&self, access: &Access, id: Uuid) -> Result<(), DecisionError> {
        let now = self.clock.now();
        let mut tx = self.pool.begin().await?;
        authorize_change(&mut tx, access, Role::Editor).await?;
        let current = find(&mut tx, access, id).await?;
        sqlx::query("DELETE FROM test_scenarios WHERE id = $1")
            .bind(id)
            .execute(&mut *tx)
            .await?;
        audit(
            &mut tx,
            access,
            now,
            Action::ScenarioDeleted,
            &current.name,
            &current.decision_key,
        )
        .await?;
        tx.commit().await?;
        Ok(())
    }
}

async fn find(
    conn: &mut sqlx::PgConnection,
    access: &Access,
    id: Uuid,
) -> Result<Scenario, DecisionError> {
    let row: Option<Row> = sqlx::query_as(crate::sql(format!("{SELECT} AND s.id = $2")))
        .bind(access.project_id())
        .bind(id)
        .fetch_optional(conn)
        .await?;
    row.map(Scenario::from)
        .ok_or(DecisionError::ScenarioNotFound)
}

async fn audit(
    tx: &mut Tx<'_>,
    access: &Access,
    now: DateTime<Utc>,
    action: Action,
    name: &str,
    key: &str,
) -> Result<(), DecisionError> {
    donka_audit::record(
        tx,
        Event::new(now, Some(access.user_id()), action)
            .in_project(access.project_id())
            .with_details(json!({ "name": name, "key": key })),
    )
    .await?;
    Ok(())
}

fn check(fields: ScenarioFields) -> Result<ScenarioFields, DecisionError> {
    let name = fields.name.trim().to_owned();
    if name.is_empty() || name.chars().count() > MAX_SCENARIO_NAME_CHARS {
        return Err(DecisionError::InvalidScenario("name"));
    }
    if !fields.input.is_object() {
        return Err(DecisionError::InvalidScenario("input"));
    }
    if !fields.expected.is_object() {
        return Err(DecisionError::InvalidScenario("expected"));
    }
    Ok(ScenarioFields { name, ..fields })
}

/// Two scenarios of a decision cannot share a name (any case).
fn name_taken(err: sqlx::Error) -> DecisionError {
    match err.as_database_error().and_then(|db| db.code()) {
        Some(code) if code == "23505" => DecisionError::ScenarioNameTaken,
        _ => err.into(),
    }
}
