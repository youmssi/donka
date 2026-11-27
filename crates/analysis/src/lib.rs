//! Bounded, privacy-preserving summaries for recorded-decision analyses.
//!
//! This crate deliberately has no access to a database or network. Callers load encrypted
//! records through the decision-log API, evaluate them through `DecisionRuntime`, then retain
//! only these aggregates and record identifiers.

pub mod alerts;
pub mod snapshot;
pub mod stability;

use chrono::{DateTime, Utc};
use donka_audit::{Action, Event};
use donka_db::PgPool;
use donka_decision_log::{DecisionLog, Filter};
use donka_engine::{contract, Bundle, DecisionRuntime, EvaluateOptions};
use donka_project::{Access, ProjectError, Role};
use donka_release::{Environment, ReleaseError, Releases};
use donka_shared::page::PageRequest;
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::sync::Arc;
use thiserror::Error;
use tokio::task;
use uuid::Uuid;

/// Outcome transition between production and a proposed release.
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, serde::Deserialize, utoipa::ToSchema,
)]
pub struct Transition {
    pub before: String,
    pub after: String,
}

/// Why a production record could not be compared.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Serialize,
    serde::Deserialize,
    utoipa::ToSchema,
)]
#[serde(rename_all = "camelCase")]
pub enum UncomparableReason {
    ConnectorNotPreviouslyCalled,
    ContractRejected,
    EvaluationError,
    TimeDependentExpression,
}

/// One record's aggregate contribution. Inputs and outputs are intentionally absent.
#[derive(Debug, Clone)]
pub enum Comparison {
    Compared {
        record_id: Uuid,
        before: String,
        after: String,
    },
    Uncomparable {
        reason: UncomparableReason,
    },
}

/// Persistable result. Record identifiers are the only record-level data retained.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ResultSummary {
    pub records_read: usize,
    pub records_available: usize,
    pub truncated: bool,
    pub records_compared: usize,
    pub before: BTreeMap<String, usize>,
    pub after: BTreeMap<String, usize>,
    pub transitions: Vec<TransitionCell>,
    pub uncomparable: BTreeMap<UncomparableReason, usize>,
}

/// One table cell and the records that moved through it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TransitionCell {
    pub before: String,
    pub after: String,
    pub count: usize,
    pub records: Vec<Uuid>,
}

impl ResultSummary {
    pub fn from_comparisons(comparisons: impl IntoIterator<Item = Comparison>) -> Self {
        let mut result = Self::default();
        let mut transitions = BTreeMap::<Transition, (usize, Vec<Uuid>)>::new();
        for comparison in comparisons {
            result.records_read += 1;
            match comparison {
                Comparison::Compared {
                    record_id,
                    before,
                    after,
                } => {
                    result.records_compared += 1;
                    *result.before.entry(before.clone()).or_default() += 1;
                    *result.after.entry(after.clone()).or_default() += 1;
                    let changed = before != after;
                    let (count, records) = transitions
                        .entry(Transition { before, after })
                        .or_insert_with(|| (0, Vec::new()));
                    *count += 1;
                    if changed {
                        records.push(record_id);
                    }
                }
                Comparison::Uncomparable { reason } => {
                    *result.uncomparable.entry(reason).or_default() += 1;
                }
            }
        }
        result.transitions = transitions
            .into_iter()
            .map(|(transition, (count, records))| TransitionCell {
                before: transition.before,
                after: transition.after,
                count,
                records,
            })
            .collect();
        result
    }
}

#[derive(Debug, Error)]
pub enum AnalysisError {
    #[error("analysis is disabled")]
    Disabled,
    #[error("the selected approval has no decision named '{0}'")]
    DecisionNotInRelease(String),
    #[error("analysis job not found")]
    NotFound,
    #[error("the end of the analysis period must be after its start")]
    InvalidPeriod,
    #[error("the decision log must have an outcome field before analysis can run")]
    OutcomeNotConfigured,
    #[error(transparent)]
    Project(#[from] ProjectError),
    #[error(transparent)]
    Release(#[from] ReleaseError),
    #[error(transparent)]
    Database(#[from] sqlx::Error),
    #[error(transparent)]
    DecisionLog(#[from] donka_decision_log::DecisionLogError),
    #[error("analysis result could not be stored: {0}")]
    ResultSerialization(String),
}

#[derive(Debug, Clone, Serialize, serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Job {
    pub id: Uuid,
    pub status: String,
    pub records_read: usize,
    pub result: Option<ResultSummary>,
}

/// Runs impact analysis over the decision log. Only encrypted inputs are loaded, and the job
/// table stores aggregates plus changed record IDs.
#[derive(Clone)]
pub struct Analysis {
    pool: PgPool,
    log: DecisionLog,
    releases: Releases,
    runtime: Arc<dyn DecisionRuntime>,
    enabled: bool,
    max_records: usize,
}

struct JobExecution {
    id: Uuid,
    access: Access,
    release_id: Uuid,
    production_id: Option<Uuid>,
    decision_key: String,
    from: DateTime<Utc>,
    until: DateTime<Utc>,
}

impl Analysis {
    pub fn new(
        pool: PgPool,
        log: DecisionLog,
        releases: Releases,
        runtime: Arc<dyn DecisionRuntime>,
        enabled: bool,
        max_records: usize,
    ) -> Self {
        Self {
            pool,
            log,
            releases,
            runtime,
            enabled,
            max_records,
        }
    }

    /// Queues a run for a pending approval. The owner is checked before any record is opened.
    pub async fn start(
        &self,
        access: &Access,
        approval_id: Uuid,
        decision_key: String,
        from: DateTime<Utc>,
        until: DateTime<Utc>,
    ) -> Result<Job, AnalysisError> {
        if !self.enabled {
            return Err(AnalysisError::Disabled);
        }
        access.require(Role::Owner)?;
        if from >= until {
            return Err(AnalysisError::InvalidPeriod);
        }
        if self.log.settings(access).await?.outcome_field.is_none() {
            return Err(AnalysisError::OutcomeNotConfigured);
        }

        let review = self.releases.approval(access, approval_id).await?;
        if review.approval.status != donka_release::ApprovalStatus::Pending {
            return Err(AnalysisError::Release(ReleaseError::ApprovalNotFound));
        }
        let proposed = self
            .releases
            .contents(access, review.approval.release_id)
            .await?;
        if !proposed.contains_key(&decision_key) {
            return Err(AnalysisError::DecisionNotInRelease(decision_key));
        }
        let production = self
            .releases
            .environments(access)
            .await?
            .into_iter()
            .find(|env| env.environment == Environment::Production)
            .and_then(|env| env.live.map(|deployment| deployment.release_id));

        let id = Uuid::new_v4();
        let now = Utc::now();
        let mut tx = self.pool.begin().await?;
        sqlx::query(
            "INSERT INTO analysis_jobs (id, project_id, approval_id, release_id, decision_key, started_by, from_at, until_at, status, created_at) \
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8,'queued',$9)"
        ).bind(id).bind(access.project_id()).bind(approval_id).bind(review.approval.release_id)
            .bind(&decision_key).bind(access.user_id()).bind(from).bind(until).bind(now)
            .execute(&mut *tx).await?;
        donka_audit::record(&mut tx, Event::new(now, Some(access.user_id()), Action::AnalysisRunStarted)
            .in_project(access.project_id()).with_details(json!({"jobId":id,"approvalId":approval_id,"decisionKey":decision_key,"from":from,"until":until,"maxRecords":self.max_records}))).await?;
        tx.commit().await?;

        let this = self.clone();
        let execution = JobExecution {
            id,
            access: *access,
            release_id: review.approval.release_id,
            production_id: production,
            decision_key,
            from,
            until,
        };
        task::spawn(async move {
            this.execute(execution).await;
        });
        Ok(Job {
            id,
            status: "queued".into(),
            records_read: 0,
            result: None,
        })
    }

    /// Reads a job only inside the requesting project; owners alone can inspect applicant IDs.
    pub async fn get(&self, access: &Access, id: Uuid) -> Result<Job, AnalysisError> {
        access.require(Role::Owner)?;
        let row: Option<(String, i32, Option<Value>)> = sqlx::query_as(
            "SELECT status, records_read, result FROM analysis_jobs WHERE project_id=$1 AND id=$2",
        )
        .bind(access.project_id())
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        let (status, records_read, result) = row.ok_or(AnalysisError::NotFound)?;
        let result = result
            .map(|value| {
                serde_json::from_value(value)
                    .map_err(|err| AnalysisError::ResultSerialization(err.to_string()))
            })
            .transpose()?;
        Ok(Job {
            id,
            status,
            records_read: records_read as usize,
            result,
        })
    }

    pub async fn latest_for_approval(
        &self,
        access: &Access,
        approval_id: Uuid,
    ) -> Result<Option<Job>, AnalysisError> {
        access.require(Role::Owner)?;
        let row: Option<(Uuid, String, i32, Option<Value>)> = sqlx::query_as(
            "SELECT id, status, records_read, result FROM analysis_jobs \
             WHERE project_id=$1 AND approval_id=$2 ORDER BY created_at DESC LIMIT 1",
        )
        .bind(access.project_id())
        .bind(approval_id)
        .fetch_optional(&self.pool)
        .await?;
        row.map(|(id, status, records_read, result)| {
            let result = result
                .map(|value| {
                    serde_json::from_value(value)
                        .map_err(|error| AnalysisError::ResultSerialization(error.to_string()))
                })
                .transpose()?;
            Ok(Job {
                id,
                status,
                records_read: records_read as usize,
                result,
            })
        })
        .transpose()
    }

    pub async fn cancel(&self, access: &Access, id: Uuid) -> Result<Job, AnalysisError> {
        access.require(Role::Owner)?;
        let updated = sqlx::query(
            "UPDATE analysis_jobs SET cancel_requested=true WHERE project_id=$1 AND id=$2 AND status IN ('queued','running')"
        ).bind(access.project_id()).bind(id).execute(&self.pool).await?;
        if updated.rows_affected() == 0 {
            return self.get(access, id).await;
        }
        self.get(access, id).await
    }

    async fn execute(&self, execution: JobExecution) {
        let result = self.execute_inner(&execution).await;
        if let Err(error) = result {
            tracing::error!(job_id=%execution.id, error=%error, "decision analysis job failed");
            if let Ok(mut tx) = self.pool.begin().await {
                let records_read = sqlx::query_scalar::<_, i32>(
                    "UPDATE analysis_jobs SET status='failed', finished_at=now() \
                     WHERE id=$1 AND status IN ('queued','running') RETURNING records_read",
                )
                .bind(execution.id)
                .fetch_optional(&mut *tx)
                .await;
                if let Ok(Some(records_read)) = records_read {
                    let _ = donka_audit::record(
                        &mut tx,
                        Event::new(
                            Utc::now(),
                            Some(execution.access.user_id()),
                            Action::AnalysisRunCompleted,
                        )
                        .in_project(execution.access.project_id())
                        .with_details(
                            json!({"jobId":execution.id,"recordsRead":records_read,"status":"failed"}),
                        ),
                    )
                    .await;
                }
                let _ = tx.commit().await;
            }
        }
    }

    async fn execute_inner(&self, execution: &JobExecution) -> Result<(), AnalysisError> {
        let id = execution.id;
        let access = &execution.access;
        loop {
            if is_cancelled(&self.pool, id).await? {
                let mut tx = self.pool.begin().await?;
                let updated = sqlx::query(
                    "UPDATE analysis_jobs SET status='cancelled', finished_at=now() \
                     WHERE id=$1 AND status='queued'",
                )
                .bind(id)
                .execute(&mut *tx)
                .await?;
                if updated.rows_affected() > 0 {
                    donka_audit::record(
                        &mut tx,
                        Event::new(
                            Utc::now(),
                            Some(access.user_id()),
                            Action::AnalysisRunCompleted,
                        )
                        .in_project(access.project_id())
                        .with_details(json!({"jobId":id,"recordsRead":0,"status":"cancelled"})),
                    )
                    .await?;
                }
                tx.commit().await?;
                return Ok(());
            }
            if self.try_claim_project_slot(id, access.project_id()).await? {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
        self.execute_claimed(execution).await
    }

    /// Claims the project's single analysis slot without holding a database connection while
    /// the run proceeds. A short transaction advisory lock makes the claim atomic across app
    /// instances; the running row keeps the slot occupied until the job reaches a terminal state.
    async fn try_claim_project_slot(
        &self,
        id: Uuid,
        project_id: Uuid,
    ) -> Result<bool, AnalysisError> {
        let mut tx = self.pool.begin().await?;
        let (locked,): (bool,) =
            sqlx::query_as("SELECT pg_try_advisory_xact_lock(hashtextextended($1, 0))")
                .bind(project_id.to_string())
                .fetch_one(&mut *tx)
                .await?;
        if !locked {
            tx.rollback().await?;
            return Ok(false);
        }
        let (already_running,): (bool,) = sqlx::query_as(
            "SELECT EXISTS(SELECT 1 FROM analysis_jobs WHERE project_id=$1 AND status='running')",
        )
        .bind(project_id)
        .fetch_one(&mut *tx)
        .await?;
        if already_running {
            tx.rollback().await?;
            return Ok(false);
        }
        let (older_queued,): (bool,) = sqlx::query_as(
            "SELECT EXISTS(SELECT 1 FROM analysis_jobs older \
             JOIN analysis_jobs current ON current.id=$1 \
             WHERE older.project_id=$2 AND older.status='queued' \
             AND (older.created_at, older.id) < (current.created_at, current.id))",
        )
        .bind(id)
        .bind(project_id)
        .fetch_one(&mut *tx)
        .await?;
        if older_queued {
            tx.rollback().await?;
            return Ok(false);
        }
        let claimed = sqlx::query(
            "UPDATE analysis_jobs SET status='running' WHERE id=$1 AND status='queued' AND cancel_requested=false",
        )
        .bind(id)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(claimed.rows_affected() > 0)
    }

    async fn execute_claimed(&self, execution: &JobExecution) -> Result<(), AnalysisError> {
        let id = execution.id;
        let access = &execution.access;
        let release_id = execution.release_id;
        let production_id = execution.production_id;
        let decision_key = execution.decision_key.as_str();
        let from = execution.from;
        let until = execution.until;
        let contents = self.releases.contents(access, release_id).await?;
        let candidate = contents
            .get(decision_key)
            .cloned()
            .ok_or_else(|| AnalysisError::DecisionNotInRelease(decision_key.to_owned()))?;
        let bundle = Bundle::from_json(contents)
            .map_err(|err| AnalysisError::ResultSerialization(err.to_string()))?;
        let settings = self.log.settings(access).await?;
        let mut comparisons = Vec::new();
        let mut referenced_records = Vec::new();
        let mut records_available = 0_usize;
        let mut offset = 0_i64;
        let mut stop = false;
        while !stop && comparisons.len() < self.max_records {
            if production_id.is_none() {
                break;
            }
            if is_cancelled(&self.pool, id).await? {
                break;
            }
            let page = self
                .log
                .search(
                    access,
                    &Filter {
                        decision_key: Some(decision_key.to_owned()),
                        environment: Some(Environment::Production),
                        from: Some(from),
                        to: Some(until),
                        ..Filter::default()
                    },
                    PageRequest { limit: 100, offset },
                )
                .await?;
            records_available = usize::try_from(page.total).unwrap_or(usize::MAX);
            if page.items.is_empty() {
                break;
            }
            offset += page.items.len() as i64;
            for summary in page.items {
                if comparisons.len() >= self.max_records || is_cancelled(&self.pool, id).await? {
                    stop = true;
                    break;
                }
                let id_record = summary.id;
                referenced_records.push((id_record, summary.evaluated_at));
                let old_outcome = summary.outcome.clone();
                let record = self.log.view(access, id_record).await?;
                let comparison = if time_dependent(&candidate) {
                    Comparison::Uncomparable {
                        reason: UncomparableReason::TimeDependentExpression,
                    }
                } else if !contract::accepts_input(&candidate, &record.payload.input)
                    .unwrap_or(false)
                {
                    Comparison::Uncomparable {
                        reason: UncomparableReason::ContractRejected,
                    }
                } else if connector_not_recorded(&candidate, record.payload.trace.as_ref()) {
                    Comparison::Uncomparable {
                        reason: UncomparableReason::ConnectorNotPreviouslyCalled,
                    }
                } else {
                    match self
                        .runtime
                        .evaluate(
                            &bundle,
                            decision_key,
                            record.payload.input.clone(),
                            EvaluateOptions {
                                trace: false,
                                recorded_trace: record.payload.trace.clone().map(Arc::new),
                            },
                        )
                        .await
                    {
                        Ok(evaluation) => {
                            match (
                                old_outcome,
                                outcome(&evaluation.result, settings.outcome_field.as_deref()),
                            ) {
                                (Some(before), Some(after)) => Comparison::Compared {
                                    record_id: id_record,
                                    before,
                                    after,
                                },
                                _ => Comparison::Uncomparable {
                                    reason: UncomparableReason::EvaluationError,
                                },
                            }
                        }
                        Err(_) => Comparison::Uncomparable {
                            reason: UncomparableReason::EvaluationError,
                        },
                    }
                };
                comparisons.push(comparison);
                sqlx::query(
                    "UPDATE analysis_jobs SET records_read=$2 WHERE id=$1 AND status='running'",
                )
                .bind(id)
                .bind(comparisons.len() as i32)
                .execute(&self.pool)
                .await?;
            }
        }
        let cancelled = is_cancelled(&self.pool, id).await?;
        let mut result = ResultSummary::from_comparisons(comparisons);
        result.records_available = records_available;
        result.truncated = result.records_read < records_available;
        let serialized = serde_json::to_value(&result)
            .map_err(|err| AnalysisError::ResultSerialization(err.to_string()))?;
        let status = if cancelled { "cancelled" } else { "succeeded" };
        let mut tx = self.pool.begin().await?;
        sqlx::query("UPDATE analysis_jobs SET status=$2, result=$3, finished_at=now() WHERE id=$1")
            .bind(id)
            .bind(status)
            .bind(serialized)
            .execute(&mut *tx)
            .await?;
        for (record_id, evaluated_at) in referenced_records {
            sqlx::query("INSERT INTO analysis_job_records (job_id,record_id,evaluated_at) VALUES ($1,$2,$3) ON CONFLICT DO NOTHING")
                .bind(id).bind(record_id).bind(evaluated_at).execute(&mut *tx).await?;
        }
        donka_audit::record(
            &mut tx,
            Event::new(
                Utc::now(),
                Some(access.user_id()),
                Action::AnalysisRunCompleted,
            )
            .in_project(access.project_id())
            .with_details(json!({"jobId":id,"recordsRead":result.records_read,"status":status})),
        )
        .await?;
        tx.commit().await?;
        Ok(())
    }
}

async fn is_cancelled(pool: &PgPool, id: Uuid) -> Result<bool, sqlx::Error> {
    let (cancelled,): (bool,) =
        sqlx::query_as("SELECT cancel_requested FROM analysis_jobs WHERE id=$1")
            .bind(id)
            .fetch_one(pool)
            .await?;
    Ok(cancelled)
}

fn outcome(output: &Value, path: Option<&str>) -> Option<String> {
    let mut value = output;
    for part in path?.split('.') {
        value = value.get(part)?;
    }
    match value {
        Value::String(value) => Some(
            value
                .chars()
                .take(donka_decision_log::MAX_TEXT_CHARS)
                .collect(),
        ),
        Value::Number(value) => Some(
            value
                .to_string()
                .chars()
                .take(donka_decision_log::MAX_TEXT_CHARS)
                .collect(),
        ),
        Value::Bool(value) => Some(value.to_string()),
        _ => None,
    }
}

fn connector_not_recorded(content: &Value, trace: Option<&Value>) -> bool {
    let Some(nodes) = content.get("nodes").and_then(Value::as_array) else {
        return false;
    };
    nodes
        .iter()
        .filter(|node| {
            node.get("type").and_then(Value::as_str) == Some("customNode")
                && node.pointer("/content/kind").and_then(Value::as_str) == Some("donka.connector")
        })
        .any(|node| {
            let Some(id) = node.get("id").and_then(Value::as_str) else {
                return true;
            };
            trace
                .and_then(Value::as_object)
                .is_none_or(|trace| !trace.contains_key(id))
        })
}

fn time_dependent(content: &Value) -> bool {
    match content {
        Value::String(text) => ["now(", "today(", "Date.now", "new Date("]
            .iter()
            .any(|needle| text.contains(needle)),
        Value::Array(items) => items.iter().any(time_dependent),
        Value::Object(items) => items.values().any(time_dependent),
        _ => false,
    }
}

#[cfg(test)]
mod tests;
