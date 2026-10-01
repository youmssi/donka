//! Running every scenario of the project on a version, and the results kept with it.
//!
//! A version is tested as a release would hold it: this decision at the new
//! version, every other decision at its latest version. A decision never saved
//! as a version is not part of that, so a scenario that needs it cannot run.

use donka_engine::{Bundle, EvaluateOptions, RuntimeError};
use donka_project::Access;
use serde_json::Value;
use std::collections::BTreeMap;
use uuid::Uuid;

use crate::compare::{compare, Match, Mismatch};
use crate::{fetch, DecisionError, DecisionSummary, Decisions, Tx};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TestStatus {
    Passed,
    /// The decision ran and gave another output.
    Failed,
    /// The decision could not run (a decision missing, an evaluation error).
    Error,
}

impl TestStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            TestStatus::Passed => "passed",
            TestStatus::Failed => "failed",
            TestStatus::Error => "error",
        }
    }

    fn parse(value: &str) -> Self {
        match value {
            "passed" => TestStatus::Passed,
            "failed" => TestStatus::Failed,
            _ => TestStatus::Error,
        }
    }
}

/// How the scenarios went on one version.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, sqlx::FromRow)]
pub struct TestSummary {
    pub passed: i64,
    pub failed: i64,
    pub errors: i64,
}

/// One scenario as it ran on a version: the scenario as it was then, and what happened.
#[derive(Debug, Clone)]
pub struct TestResult {
    pub scenario_id: Uuid,
    pub name: String,
    pub decision_key: String,
    pub input: Value,
    pub expected: Value,
    pub match_mode: Match,
    pub status: TestStatus,
    pub actual: Option<Value>,
    pub mismatches: Vec<Mismatch>,
    /// It could not run because this decision has no saved version.
    pub missing_decision: Option<String>,
    /// It could not run: the engine's message.
    pub error: Option<String>,
}

#[derive(sqlx::FromRow)]
struct ResultRow {
    scenario_id: Uuid,
    name: String,
    decision_key: String,
    input: Value,
    expected: Value,
    #[sqlx(rename = "match")]
    match_mode: String,
    status: String,
    actual: Option<Value>,
    mismatches: Value,
    missing_decision: Option<String>,
    error: Option<String>,
}

#[derive(sqlx::FromRow)]
struct ScenarioRow {
    id: Uuid,
    name: String,
    decision_key: String,
    input: Value,
    expected: Value,
    #[sqlx(rename = "match")]
    match_mode: String,
}

/// The counts of a version's results, for queries that list versions (`v` is the version).
pub(crate) const SUMMARY_JOIN: &str = "LEFT JOIN LATERAL (SELECT \
     count(*) FILTER (WHERE status = 'passed') AS passed, \
     count(*) FILTER (WHERE status = 'failed') AS failed, \
     count(*) FILTER (WHERE status = 'error') AS errors \
     FROM test_results r WHERE r.decision_id = v.decision_id AND r.version_number = v.number) t ON true";

impl Decisions {
    /// The results of every scenario on version `number` of a decision (any member).
    pub async fn test_results(
        &self,
        access: &Access,
        id: Uuid,
        number: i32,
    ) -> Result<Vec<TestResult>, DecisionError> {
        let mut conn = self.pool.acquire().await?;
        fetch(&mut conn, access, id).await?;
        let exists: Option<(i32,)> = sqlx::query_as(
            "SELECT number FROM decision_versions WHERE decision_id = $1 AND number = $2",
        )
        .bind(id)
        .bind(number)
        .fetch_optional(&mut *conn)
        .await?;
        exists.ok_or(DecisionError::VersionNotFound)?;
        let rows: Vec<ResultRow> = sqlx::query_as(
            "SELECT scenario_id, name, decision_key, input, expected, match, status, actual, mismatches, \
             missing_decision, error \
             FROM test_results WHERE decision_id = $1 AND version_number = $2 \
             ORDER BY decision_key, lower(name)",
        )
        .bind(id)
        .bind(number)
        .fetch_all(&mut *conn)
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| TestResult {
                scenario_id: row.scenario_id,
                name: row.name,
                decision_key: row.decision_key,
                input: row.input,
                expected: row.expected,
                match_mode: Match::parse(&row.match_mode).unwrap_or(Match::Exact),
                status: TestStatus::parse(&row.status),
                actual: row.actual,
                // Written by `record` below from the same type.
                mismatches: serde_json::from_value(row.mismatches).unwrap_or_default(),
                missing_decision: row.missing_decision,
                error: row.error,
            })
            .collect())
    }

    /// Runs every scenario of the project on the version just added for
    /// `decision` (its graph is `content`) and keeps the results with it.
    pub(crate) async fn run_tests(
        &self,
        tx: &mut Tx<'_>,
        access: &Access,
        decision: &DecisionSummary,
        number: i32,
        content: &Value,
    ) -> Result<TestSummary, DecisionError> {
        let scenarios: Vec<ScenarioRow> = sqlx::query_as(
            "SELECT s.id, s.name, d.key AS decision_key, s.input, s.expected, s.match \
             FROM test_scenarios s JOIN decisions d ON d.id = s.decision_id \
             WHERE s.project_id = $1 AND d.deleted_at IS NULL ORDER BY d.key, lower(s.name)",
        )
        .bind(access.project_id())
        .fetch_all(&mut **tx)
        .await?;
        if scenarios.is_empty() {
            return Ok(TestSummary::default());
        }

        let siblings: Vec<(String, Value)> = sqlx::query_as(
            "SELECT d.key, v.content FROM decisions d JOIN LATERAL (\
             SELECT content FROM decision_versions WHERE decision_id = d.id \
             ORDER BY number DESC LIMIT 1) v ON true \
             WHERE d.project_id = $1 AND d.deleted_at IS NULL AND d.id <> $2",
        )
        .bind(access.project_id())
        .bind(decision.id)
        .fetch_all(&mut **tx)
        .await?;
        let mut contents: BTreeMap<String, Value> = siblings.into_iter().collect();
        contents.insert(decision.key.clone(), content.clone());
        // Every version was a valid draft, so this only fails if the engine changed its mind.
        let bundle = Bundle::from_json(contents).map_err(|err| err.to_string());

        let mut summary = TestSummary::default();
        for scenario in scenarios {
            let mode = Match::parse(&scenario.match_mode).unwrap_or(Match::Exact);
            let outcome = match &bundle {
                Ok(bundle) => self.run_one(bundle, &scenario, mode).await,
                Err(message) => Outcome::error(Some(message.clone())),
            };
            match outcome.status {
                TestStatus::Passed => summary.passed += 1,
                TestStatus::Failed => summary.failed += 1,
                TestStatus::Error => summary.errors += 1,
            }
            record(tx, decision.id, number, &scenario, &outcome).await?;
        }
        Ok(summary)
    }

    async fn run_one(&self, bundle: &Bundle, scenario: &ScenarioRow, mode: Match) -> Outcome {
        if !bundle.contains(&scenario.decision_key) {
            return Outcome {
                missing_decision: Some(scenario.decision_key.clone()),
                ..Outcome::error(None)
            };
        }
        let evaluation = self
            .runtime
            .evaluate(
                bundle,
                &scenario.decision_key,
                scenario.input.clone(),
                EvaluateOptions::default(),
            )
            .await;
        match evaluation {
            Ok(evaluation) => {
                let mismatches = compare(&scenario.expected, &evaluation.result, mode);
                Outcome {
                    status: if mismatches.is_empty() {
                        TestStatus::Passed
                    } else {
                        TestStatus::Failed
                    },
                    actual: Some(evaluation.result),
                    mismatches,
                    missing_decision: None,
                    error: None,
                }
            }
            Err(RuntimeError::Evaluation { details }) => Outcome::error(
                ["source", "message"]
                    .iter()
                    .find_map(|field| details.get(field).and_then(Value::as_str))
                    .map(str::to_owned),
            ),
            Err(other) => Outcome::error(Some(other.to_string())),
        }
    }
}

struct Outcome {
    status: TestStatus,
    actual: Option<Value>,
    mismatches: Vec<Mismatch>,
    missing_decision: Option<String>,
    error: Option<String>,
}

impl Outcome {
    /// The scenario could not run; `message` says why when there is something to say.
    fn error(message: Option<String>) -> Self {
        Self {
            status: TestStatus::Error,
            actual: None,
            mismatches: Vec::new(),
            missing_decision: None,
            error: message,
        }
    }
}

async fn record(
    tx: &mut Tx<'_>,
    decision_id: Uuid,
    number: i32,
    scenario: &ScenarioRow,
    outcome: &Outcome,
) -> Result<(), DecisionError> {
    sqlx::query(
        "INSERT INTO test_results \
         (decision_id, version_number, scenario_id, name, decision_key, input, expected, match, \
          status, actual, mismatches, missing_decision, error) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)",
    )
    .bind(decision_id)
    .bind(number)
    .bind(scenario.id)
    .bind(&scenario.name)
    .bind(&scenario.decision_key)
    .bind(&scenario.input)
    .bind(&scenario.expected)
    .bind(&scenario.match_mode)
    .bind(outcome.status.as_str())
    .bind(&outcome.actual)
    // A list of paths and JSON values always serializes.
    .bind(serde_json::to_value(&outcome.mismatches).unwrap_or_default())
    .bind(&outcome.missing_decision)
    .bind(&outcome.error)
    .execute(&mut **tx)
    .await?;
    Ok(())
}
