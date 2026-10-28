//! The rights of the people decisions are about (DNK-75): owners find a person's records by an
//! identifier, export their access report, anonymise their records, document the retention
//! rules that forbid it, and record contests of decisions with their review outcome.
//!
//! Identifiers travel in request bodies, never in a URL, so they stay out of access logs.

use crate::error::{ApiError, ErrorBody};
use crate::extract::{ApiJson, ContestId, ProjectAccess, RecordId, RuleId};
use crate::routes::decision_log::{summary, ExplainLanguage, RecordSummaryResponse};
use crate::AppState;
use axum::extract::State;
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use chrono::{DateTime, NaiveDate, Utc};
use donka_decision_log::report::{self, AccessReport, ReportContest, ReportRecord};
use donka_decision_log::{
    Contest, ContestOutcome, ContestStatus, NewRetentionRule, Payload, RetentionRule, Status,
};
use donka_explain::{Language, Question};
use donka_identity::Locale;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

// ----- A person's records ---------------------------------------------------

#[derive(Deserialize, ToSchema)]
pub struct PersonRequest {
    /// A value of a field the contract marks as an identifier (national ID, phone…). Case,
    /// spaces, dashes and dots do not matter. Never stored.
    pub identifier: String,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PersonRecordResponse {
    #[serde(flatten)]
    pub summary: RecordSummaryResponse,
    /// When the person's fields were removed from it.
    pub anonymised_at: Option<DateTime<Utc>>,
}

#[derive(Serialize, ToSchema)]
pub struct PersonRecordsResponse {
    pub items: Vec<PersonRecordResponse>,
}

/// The project's records about a person, found by an identifier, newest first (owners).
/// Nothing is decrypted to search. Audited (`person.records_searched`) without the identifier.
#[utoipa::path(
    post,
    path = "/projects/{project_id}/people/search",
    tag = "people-rights",
    params(("project_id" = Uuid, Path)),
    request_body = PersonRequest,
    responses(
        (status = 200, body = PersonRecordsResponse),
        (status = 400, description = "Not an identifier (INVALID_REQUEST)", body = ErrorBody),
        (status = 403, description = "Owners only (FORBIDDEN)", body = ErrorBody),
        (status = 404, description = "PROJECT_NOT_FOUND", body = ErrorBody),
    )
)]
pub async fn search(
    State(state): State<AppState>,
    ProjectAccess(access): ProjectAccess,
    ApiJson(req): ApiJson<PersonRequest>,
) -> Result<Json<PersonRecordsResponse>, ApiError> {
    let records = state
        .decision_log
        .find_person(&access, &req.identifier)
        .await?;
    Ok(Json(PersonRecordsResponse {
        items: records
            .into_iter()
            .map(|record| PersonRecordResponse {
                summary: summary(record.summary),
                anonymised_at: record.anonymised_at,
            })
            .collect(),
    }))
}

#[derive(Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReportRequest {
    pub identifier: String,
    /// The person's language.
    pub language: ExplainLanguage,
    /// Adds a plain-language explanation of each decision, when the installation explains
    /// decisions (DNK-19). Personal and redacted fields are removed before anything is sent.
    #[serde(default)]
    pub explain: bool,
}

/// The access report of a person: every record about them with what it read, what it
/// answered, its explanation and contests, as a PDF in English or French (owners). Audited
/// once (`person.report_exported`).
#[utoipa::path(
    post,
    path = "/projects/{project_id}/people/report",
    tag = "people-rights",
    params(("project_id" = Uuid, Path)),
    request_body = ReportRequest,
    responses(
        (status = 200, description = "The report, `access-report.pdf`", content_type = "application/pdf"),
        (status = 400, description = "Not an identifier (INVALID_REQUEST)", body = ErrorBody),
        (status = 403, description = "Owners only (FORBIDDEN)", body = ErrorBody),
        (status = 404, description = "PROJECT_NOT_FOUND", body = ErrorBody),
    )
)]
pub async fn report(
    State(state): State<AppState>,
    ProjectAccess(access): ProjectAccess,
    ApiJson(req): ApiJson<ReportRequest>,
) -> Result<Response, ApiError> {
    let explainer = state.explainer.as_ref().filter(|_| req.explain);
    let report = state
        .decision_log
        .person_report(&access, &req.identifier, explainer.is_some())
        .await?;
    let project = state.projects.get(&access).await?;
    let (locale, language) = match req.language {
        ExplainLanguage::En => (Locale::En, Language::English),
        ExplainLanguage::Fr => (Locale::Fr, Language::French),
    };
    let mut records = Vec::with_capacity(report.entries.len());
    for entry in report.entries {
        let record = entry.record;
        let succeeded = record.summary.status == Status::Succeeded;
        let explanation = match explainer {
            Some(explainer) => {
                let Payload {
                    input,
                    output,
                    error,
                    trace,
                } = entry.explainable;
                let question = Question {
                    language,
                    decision_key: record.summary.decision_key.clone(),
                    release_version: record.release_version.clone(),
                    succeeded,
                    input,
                    output,
                    error,
                    trace,
                };
                // A record the service cannot explain is still reported, without explanation.
                match explainer.explain(&question).await {
                    Ok(explanation) => Some(explanation.text),
                    Err(err) => {
                        tracing::warn!(%err, record = %record.summary.id, "a record of an access report was not explained");
                        None
                    }
                }
            }
            None => None,
        };
        records.push(ReportRecord {
            decision_key: record.summary.decision_key,
            evaluated_at: record.summary.evaluated_at,
            release: record.release_version,
            outcome: record.summary.outcome,
            succeeded,
            input: record.payload.input,
            output: record.payload.output,
            explanation,
            anonymised: record.anonymised_at.is_some(),
            contests: entry
                .contests
                .into_iter()
                .map(|contest| ReportContest {
                    opened_at: contest.opened_at,
                    status: contest.status.as_str().to_owned(),
                    reason: contest.reason,
                    review: contest.review,
                })
                .collect(),
        });
    }
    let pdf = report::render(&AccessReport {
        project: project.name,
        generated_at: report.generated_at,
        locale,
        records,
    })
    .map_err(|err| ApiError::Internal(err.to_string()))?;
    Ok((
        [
            (header::CONTENT_TYPE, "application/pdf".to_owned()),
            (
                header::CONTENT_DISPOSITION,
                "attachment; filename=\"access-report.pdf\"".to_owned(),
            ),
        ],
        pdf,
    )
        .into_response())
}

#[derive(Serialize, ToSchema)]
pub struct AnonymisedResponse {
    /// Records anonymised by this request; those anonymised before are not counted.
    pub records: u64,
}

/// Removes the person's fields from every record holding the identifier (owners): the
/// fields the contract marks as personal data and the project's redacted fields go from what
/// each record read and answered; outcome, release and time stay. Cannot be undone. Refused
/// while a retention rule runs (`409 RETENTION_RULE_RUNS`, the rule in details). Audited
/// (`person.records_anonymised`).
#[utoipa::path(
    post,
    path = "/projects/{project_id}/people/anonymise",
    tag = "people-rights",
    params(("project_id" = Uuid, Path)),
    request_body = PersonRequest,
    responses(
        (status = 200, body = AnonymisedResponse),
        (status = 400, description = "Not an identifier (INVALID_REQUEST)", body = ErrorBody),
        (status = 403, description = "Owners only (FORBIDDEN)", body = ErrorBody),
        (status = 404, description = "PROJECT_NOT_FOUND", body = ErrorBody),
        (status = 409, description = "A retention rule runs (RETENTION_RULE_RUNS)", body = ErrorBody),
    )
)]
pub async fn anonymise(
    State(state): State<AppState>,
    ProjectAccess(access): ProjectAccess,
    ApiJson(req): ApiJson<PersonRequest>,
) -> Result<Json<AnonymisedResponse>, ApiError> {
    let records = state
        .decision_log
        .anonymise_person(&access, &req.identifier)
        .await?;
    Ok(Json(AnonymisedResponse { records }))
}

// ----- Retention rules ------------------------------------------------------

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RetentionRuleResponse {
    pub id: Uuid,
    /// What requires keeping the records whole.
    pub description: String,
    pub legal_reference: Option<String>,
    /// The last day it runs.
    pub until: NaiveDate,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub ended_by: Option<Uuid>,
    pub ended_at: Option<DateTime<Utc>>,
    /// It blocks anonymisation: not ended, and its last day has not passed.
    pub running: bool,
}

#[derive(Serialize, ToSchema)]
pub struct RetentionRuleListResponse {
    pub items: Vec<RetentionRuleResponse>,
}

#[derive(Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct NewRetentionRuleRequest {
    /// 1 to 500 characters.
    pub description: String,
    /// At most 200 characters.
    #[serde(default)]
    pub legal_reference: Option<String>,
    /// The last day it runs: today or later.
    pub until: NaiveDate,
}

/// The project's retention rules, running ones first (any member).
#[utoipa::path(
    get,
    path = "/projects/{project_id}/retention-rules",
    tag = "people-rights",
    params(("project_id" = Uuid, Path)),
    responses(
        (status = 200, body = RetentionRuleListResponse),
        (status = 404, description = "PROJECT_NOT_FOUND", body = ErrorBody),
    )
)]
pub async fn rules(
    State(state): State<AppState>,
    ProjectAccess(access): ProjectAccess,
) -> Result<Json<RetentionRuleListResponse>, ApiError> {
    let rules = state.decision_log.retention_rules(&access).await?;
    Ok(Json(RetentionRuleListResponse {
        items: rules.into_iter().map(rule_response).collect(),
    }))
}

/// Documents a rule that requires keeping the project's records whole (owners): while it
/// runs, they cannot be anonymised. Audited (`retention_rule.created`).
#[utoipa::path(
    post,
    path = "/projects/{project_id}/retention-rules",
    tag = "people-rights",
    params(("project_id" = Uuid, Path)),
    request_body = NewRetentionRuleRequest,
    responses(
        (status = 201, body = RetentionRuleResponse),
        (status = 400, description = "A field is not valid (INVALID_REQUEST)", body = ErrorBody),
        (status = 403, description = "Owners only (FORBIDDEN)", body = ErrorBody),
        (status = 404, description = "PROJECT_NOT_FOUND", body = ErrorBody),
    )
)]
pub async fn create_rule(
    State(state): State<AppState>,
    ProjectAccess(access): ProjectAccess,
    ApiJson(req): ApiJson<NewRetentionRuleRequest>,
) -> Result<(StatusCode, Json<RetentionRuleResponse>), ApiError> {
    let rule = state
        .decision_log
        .create_retention_rule(
            &access,
            NewRetentionRule {
                description: req.description,
                legal_reference: req.legal_reference,
                until: req.until,
            },
        )
        .await?;
    Ok((StatusCode::CREATED, Json(rule_response(rule))))
}

/// Ends a retention rule before its last day (owners). Audited (`retention_rule.ended`).
#[utoipa::path(
    post,
    path = "/projects/{project_id}/retention-rules/{rule_id}/end",
    tag = "people-rights",
    params(("project_id" = Uuid, Path), ("rule_id" = Uuid, Path)),
    responses(
        (status = 200, body = RetentionRuleResponse),
        (status = 403, description = "Owners only (FORBIDDEN)", body = ErrorBody),
        (status = 404, description = "PROJECT_NOT_FOUND or RETENTION_RULE_NOT_FOUND", body = ErrorBody),
    )
)]
pub async fn end_rule(
    State(state): State<AppState>,
    ProjectAccess(access): ProjectAccess,
    RuleId(id): RuleId,
) -> Result<Json<RetentionRuleResponse>, ApiError> {
    let rule = state.decision_log.end_retention_rule(&access, id).await?;
    Ok(Json(rule_response(rule)))
}

fn rule_response(rule: RetentionRule) -> RetentionRuleResponse {
    RetentionRuleResponse {
        id: rule.id,
        description: rule.description,
        legal_reference: rule.legal_reference,
        until: rule.until,
        created_by: rule.created_by,
        created_at: rule.created_at,
        ended_by: rule.ended_by,
        ended_at: rule.ended_at,
        running: rule.running,
    }
}

// ----- Contests -------------------------------------------------------------

#[derive(Debug, Clone, Copy, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum ContestStatusName {
    Open,
    Upheld,
    Revised,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ContestResponse {
    pub id: Uuid,
    pub record_id: Uuid,
    pub status: ContestStatusName,
    /// Why the person contests the decision.
    pub reason: String,
    pub opened_by: Uuid,
    pub opened_at: DateTime<Utc>,
    /// What the review found.
    pub review: Option<String>,
    /// The decision's new outcome, when the review revised it.
    pub revised_outcome: Option<String>,
    pub reviewed_by: Option<Uuid>,
    pub reviewed_at: Option<DateTime<Utc>>,
}

#[derive(Serialize, ToSchema)]
pub struct ContestListResponse {
    pub items: Vec<ContestResponse>,
}

#[derive(Deserialize, ToSchema)]
pub struct NewContestRequest {
    /// Why the person contests the decision: 1 to 2000 characters.
    pub reason: String,
}

#[derive(Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReviewContestRequest {
    /// `upheld` or `revised`.
    pub status: ContestStatusName,
    /// What the review found: 1 to 2000 characters.
    pub review: String,
    /// The new outcome, required when revised (1 to 200 characters).
    #[serde(default)]
    pub revised_outcome: Option<String>,
}

/// The contests of a decision record, in the order they were recorded (any member): its history.
#[utoipa::path(
    get,
    path = "/projects/{project_id}/decision-log/{record_id}/contests",
    tag = "people-rights",
    params(("project_id" = Uuid, Path), ("record_id" = Uuid, Path)),
    responses(
        (status = 200, body = ContestListResponse),
        (status = 404, description = "PROJECT_NOT_FOUND or RECORD_NOT_FOUND", body = ErrorBody),
    )
)]
pub async fn contests(
    State(state): State<AppState>,
    ProjectAccess(access): ProjectAccess,
    RecordId(id): RecordId,
) -> Result<Json<ContestListResponse>, ApiError> {
    let contests = state.decision_log.contests(&access, id).await?;
    Ok(Json(ContestListResponse {
        items: contests.into_iter().map(contest_response).collect(),
    }))
}

/// Records that the person contests a decision (owners); the review happens in the lender's
/// own systems. A record has one open contest at a time. Audited (`decision_contest.opened`).
#[utoipa::path(
    post,
    path = "/projects/{project_id}/decision-log/{record_id}/contests",
    tag = "people-rights",
    params(("project_id" = Uuid, Path), ("record_id" = Uuid, Path)),
    request_body = NewContestRequest,
    responses(
        (status = 201, body = ContestResponse),
        (status = 400, description = "No reason (INVALID_REQUEST)", body = ErrorBody),
        (status = 403, description = "Owners only (FORBIDDEN)", body = ErrorBody),
        (status = 404, description = "PROJECT_NOT_FOUND or RECORD_NOT_FOUND", body = ErrorBody),
        (status = 409, description = "A contest is under review (CONTEST_OPEN)", body = ErrorBody),
    )
)]
pub async fn open_contest(
    State(state): State<AppState>,
    ProjectAccess(access): ProjectAccess,
    RecordId(id): RecordId,
    ApiJson(req): ApiJson<NewContestRequest>,
) -> Result<(StatusCode, Json<ContestResponse>), ApiError> {
    let contest = state
        .decision_log
        .open_contest(&access, id, &req.reason)
        .await?;
    Ok((StatusCode::CREATED, Json(contest_response(contest))))
}

/// Records the outcome of a contest's review (owners): upheld, or revised with the new
/// outcome. Audited (`decision_contest.reviewed`).
#[utoipa::path(
    post,
    path = "/projects/{project_id}/contests/{contest_id}/review",
    tag = "people-rights",
    params(("project_id" = Uuid, Path), ("contest_id" = Uuid, Path)),
    request_body = ReviewContestRequest,
    responses(
        (status = 200, body = ContestResponse),
        (status = 400, description = "No review, or a revision without its outcome (INVALID_REQUEST)", body = ErrorBody),
        (status = 403, description = "Owners only (FORBIDDEN)", body = ErrorBody),
        (status = 404, description = "PROJECT_NOT_FOUND or CONTEST_NOT_FOUND", body = ErrorBody),
        (status = 409, description = "Already reviewed (CONTEST_REVIEWED)", body = ErrorBody),
    )
)]
pub async fn review_contest(
    State(state): State<AppState>,
    ProjectAccess(access): ProjectAccess,
    ContestId(id): ContestId,
    ApiJson(req): ApiJson<ReviewContestRequest>,
) -> Result<Json<ContestResponse>, ApiError> {
    let outcome = match req.status {
        ContestStatusName::Upheld => ContestOutcome::Upheld,
        ContestStatusName::Revised => ContestOutcome::Revised {
            outcome: req.revised_outcome.unwrap_or_default(),
        },
        ContestStatusName::Open => {
            return Err(ApiError::InvalidField {
                field: "status",
                message: "a review concludes upheld or revised".to_owned(),
            })
        }
    };
    let contest = state
        .decision_log
        .review_contest(&access, id, outcome, &req.review)
        .await?;
    Ok(Json(contest_response(contest)))
}

fn contest_response(contest: Contest) -> ContestResponse {
    ContestResponse {
        id: contest.id,
        record_id: contest.record_id,
        status: match contest.status {
            ContestStatus::Open => ContestStatusName::Open,
            ContestStatus::Upheld => ContestStatusName::Upheld,
            ContestStatus::Revised => ContestStatusName::Revised,
        },
        reason: contest.reason,
        opened_by: contest.opened_by,
        opened_at: contest.opened_at,
        review: contest.review,
        revised_outcome: contest.revised_outcome,
        reviewed_by: contest.reviewed_by,
        reviewed_at: contest.reviewed_at,
    }
}
