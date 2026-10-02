//! Decision log (DNK-18): the feed Runtimes send records to, the tokens they
//! send them with, and the project's records (search, open, replay).

use crate::auth::CurrentUser;
use crate::error::{ApiError, ErrorBody};
use crate::extract::{ApiJson, ProjectAccess, RecordId, TokenId};
use crate::routes::people::{emails, known_person, person, PersonRef};
use crate::routes::releases::EnvironmentName;
use crate::AppState;
use axum::extract::{Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::{Extension, Json};
use chrono::{DateTime, Utc};
use donka_decision_log::{Filter, LogToken, Payload, RecordSummary, Status, MAX_TEXT_CHARS};
use donka_release::Environment;
use donka_shared::page::PageRequest;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

// ----- Feed ---------------------------------------------------------------

/// A batch of records, as a Runtime sends it (docs/decision-log-feed.md).
#[derive(Deserialize, ToSchema)]
pub struct FeedBatch {
    /// At most 1000 records; each is checked on its own.
    pub records: Vec<Value>,
}

#[derive(Serialize, ToSchema)]
pub struct FeedReceipt {
    /// Records stored, now or before (a record sent twice is stored once).
    pub accepted: usize,
    /// Records not stored, with why: `invalid`, `wrong_environment` or `unknown_release`.
    pub rejected: Vec<FeedRejection>,
}

#[derive(Serialize, ToSchema)]
pub struct FeedRejection {
    pub id: Uuid,
    pub code: &'static str,
}

/// Receives the records a Runtime sends (`Authorization: Bearer <decision-log token>`).
#[utoipa::path(
    post,
    path = "/decision-log/records",
    tag = "decision-log",
    request_body = FeedBatch,
    responses(
        (status = 200, body = FeedReceipt),
        (status = 400, description = "Not a batch, or more than 1000 records (INVALID_REQUEST, TOO_MANY_RECORDS)", body = ErrorBody),
        (status = 401, description = "Missing, unknown or revoked token (INVALID_TOKEN)", body = ErrorBody),
    )
)]
pub async fn receive(
    State(state): State<AppState>,
    headers: HeaderMap,
    ApiJson(batch): ApiJson<FeedBatch>,
) -> Result<Json<FeedReceipt>, ApiError> {
    let bearer = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .ok_or(ApiError::InvalidLogToken)?;
    let token = state.decision_log.authenticate(bearer.trim()).await?;
    let receipt = state.decision_log.receive(token, batch.records).await?;
    Ok(Json(FeedReceipt {
        accepted: receipt.accepted,
        rejected: receipt
            .rejected
            .into_iter()
            .map(|(id, rejection)| FeedRejection {
                id,
                code: rejection.code(),
            })
            .collect(),
    }))
}

// ----- Tokens (administrators) ----------------------------------------------

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct LogTokenResponse {
    pub id: Uuid,
    /// The environment whose Runtimes send records with it.
    pub environment: EnvironmentName,
    pub name: String,
    /// The token's last characters, to tell tokens apart.
    pub hint: String,
    pub created_by: PersonRef,
    pub created_at: DateTime<Utc>,
    pub revoked_by: Option<PersonRef>,
    pub revoked_at: Option<DateTime<Utc>>,
}

#[derive(Serialize, ToSchema)]
pub struct LogTokenListResponse {
    pub items: Vec<LogTokenResponse>,
}

#[derive(Deserialize, ToSchema)]
pub struct IssueLogTokenRequest {
    pub environment: EnvironmentName,
    /// 1 to 100 characters, e.g. the Runtime's host.
    #[schema(max_length = 100)]
    pub name: String,
}

#[derive(Serialize, ToSchema)]
pub struct IssuedLogTokenResponse {
    /// Shown this once: set it as the Runtime's `DECISION_LOG__TOKEN`.
    pub token: String,
    #[serde(flatten)]
    pub details: LogTokenResponse,
}

/// Every decision-log token, live ones first; never their value (administrators).
#[utoipa::path(
    get,
    path = "/decision-log/tokens",
    tag = "decision-log",
    responses(
        (status = 200, body = LogTokenListResponse),
        (status = 403, description = "Not an administrator (FORBIDDEN)", body = ErrorBody),
    )
)]
pub async fn tokens(
    State(state): State<AppState>,
    Extension(current): Extension<CurrentUser>,
) -> Result<Json<LogTokenListResponse>, ApiError> {
    let tokens = state.decision_log.tokens(&current.user).await?;
    let emails = emails(
        &state,
        tokens
            .iter()
            .flat_map(|t| [Some(t.created_by), t.revoked_by]),
    )
    .await?;
    Ok(Json(LogTokenListResponse {
        items: tokens.into_iter().map(|t| log_token(&emails, t)).collect(),
    }))
}

/// Issues a token Runtimes of one environment send their records with
/// (administrators). The token is in this response only.
#[utoipa::path(
    post,
    path = "/decision-log/tokens",
    tag = "decision-log",
    request_body = IssueLogTokenRequest,
    responses(
        (status = 201, body = IssuedLogTokenResponse),
        (status = 400, description = "Missing or too long name (INVALID_REQUEST)", body = ErrorBody),
        (status = 403, description = "Not an administrator (FORBIDDEN)", body = ErrorBody),
    )
)]
pub async fn issue_token(
    State(state): State<AppState>,
    Extension(current): Extension<CurrentUser>,
    ApiJson(req): ApiJson<IssueLogTokenRequest>,
) -> Result<(StatusCode, Json<IssuedLogTokenResponse>), ApiError> {
    let issued = state
        .decision_log
        .issue_token(&current.user, environment(req.environment), &req.name)
        .await?;
    let emails = emails(&state, [Some(issued.details.created_by)]).await?;
    Ok((
        StatusCode::CREATED,
        Json(IssuedLogTokenResponse {
            token: issued.token,
            details: log_token(&emails, issued.details),
        }),
    ))
}

/// Revokes a decision-log token: batches sent with it are refused from now on (administrators).
#[utoipa::path(
    delete,
    path = "/decision-log/tokens/{token_id}",
    tag = "decision-log",
    params(("token_id" = Uuid, Path)),
    responses(
        (status = 204, description = "Revoked"),
        (status = 403, description = "Not an administrator (FORBIDDEN)", body = ErrorBody),
        (status = 404, description = "Unknown or already revoked (TOKEN_NOT_FOUND)", body = ErrorBody),
    )
)]
pub async fn revoke_token(
    State(state): State<AppState>,
    Extension(current): Extension<CurrentUser>,
    TokenId(id): TokenId,
) -> Result<StatusCode, ApiError> {
    state.decision_log.revoke_token(&current.user, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

fn log_token(emails: &HashMap<Uuid, String>, token: LogToken) -> LogTokenResponse {
    LogTokenResponse {
        id: token.id,
        environment: token.environment.0.into(),
        name: token.name,
        hint: token.hint,
        created_by: known_person(emails, token.created_by),
        created_at: token.created_at,
        revoked_by: person(emails, token.revoked_by),
        revoked_at: token.revoked_at,
    }
}

fn environment(name: EnvironmentName) -> Environment {
    match name {
        EnvironmentName::Staging => Environment::Staging,
        EnvironmentName::Production => Environment::Production,
    }
}

// ----- Settings -------------------------------------------------------------

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DecisionLogSettingsBody {
    /// The output field whose value is a record's outcome, as a dotted path
    /// (`decision`, `result.band`); `null` for none. Read when a record arrives.
    #[schema(max_length = 200)]
    pub outcome_field: Option<String>,
}

/// The project's decision-log settings (any member).
#[utoipa::path(
    get,
    path = "/projects/{project_id}/decision-log/settings",
    tag = "decision-log",
    params(("project_id" = Uuid, Path)),
    responses(
        (status = 200, body = DecisionLogSettingsBody),
        (status = 404, description = "PROJECT_NOT_FOUND", body = ErrorBody),
    )
)]
pub async fn settings(
    State(state): State<AppState>,
    ProjectAccess(access): ProjectAccess,
) -> Result<Json<DecisionLogSettingsBody>, ApiError> {
    let settings = state.decision_log.settings(&access).await?;
    Ok(Json(DecisionLogSettingsBody {
        outcome_field: settings.outcome_field,
    }))
}

/// Names the output field read as each new record's outcome (owners).
/// Records already stored keep the outcome they arrived with.
#[utoipa::path(
    put,
    path = "/projects/{project_id}/decision-log/settings",
    tag = "decision-log",
    params(("project_id" = Uuid, Path)),
    request_body = DecisionLogSettingsBody,
    responses(
        (status = 200, body = DecisionLogSettingsBody),
        (status = 400, description = "Not a dotted path of field names (INVALID_REQUEST)", body = ErrorBody),
        (status = 403, description = "Only owners change settings (FORBIDDEN)", body = ErrorBody),
        (status = 404, description = "PROJECT_NOT_FOUND", body = ErrorBody),
        (status = 409, description = "PROJECT_ARCHIVED", body = ErrorBody),
    )
)]
pub async fn update_settings(
    State(state): State<AppState>,
    ProjectAccess(access): ProjectAccess,
    ApiJson(req): ApiJson<DecisionLogSettingsBody>,
) -> Result<Json<DecisionLogSettingsBody>, ApiError> {
    let settings = state
        .decision_log
        .update_settings(&access, req.outcome_field.as_deref())
        .await?;
    Ok(Json(DecisionLogSettingsBody {
        outcome_field: settings.outcome_field,
    }))
}

// ----- Records ----------------------------------------------------------------

/// Whether the Runtime answered or the evaluation failed.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum RecordStatus {
    Succeeded,
    Failed,
}

impl From<Status> for RecordStatus {
    fn from(status: Status) -> Self {
        match status {
            Status::Succeeded => RecordStatus::Succeeded,
            Status::Failed => RecordStatus::Failed,
        }
    }
}

impl From<RecordStatus> for Status {
    fn from(status: RecordStatus) -> Self {
        match status {
            RecordStatus::Succeeded => Status::Succeeded,
            RecordStatus::Failed => Status::Failed,
        }
    }
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
#[serde(rename_all = "camelCase")]
pub struct RecordQuery {
    /// The caller's reference (`X-Donka-Reference`), exactly.
    pub reference: Option<String>,
    /// Only this decision, by key.
    pub decision_key: Option<String>,
    /// Only this outcome, exactly (`error` for failed evaluations).
    pub outcome: Option<String>,
    pub environment: Option<EnvironmentName>,
    pub status: Option<RecordStatus>,
    /// Evaluated from this instant, inclusive (ISO-8601).
    pub from: Option<DateTime<Utc>>,
    /// Evaluated until this instant, exclusive (ISO-8601).
    pub until: Option<DateTime<Utc>>,
    /// Page size, 1 to 100 (default 50).
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RecordSummaryResponse {
    pub id: Uuid,
    pub decision_key: String,
    pub reference: Option<String>,
    pub environment: EnvironmentName,
    pub status: RecordStatus,
    /// The project's outcome field when the record arrived; `error` for failures.
    pub outcome: Option<String>,
    pub release_id: Uuid,
    pub evaluated_at: DateTime<Utc>,
    /// How long the Runtime took, in microseconds.
    pub duration_us: i64,
}

#[derive(Serialize, ToSchema)]
pub struct RecordListResponse {
    pub items: Vec<RecordSummaryResponse>,
    /// Records matching across all pages.
    pub total: i64,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RecordResponse {
    #[serde(flatten)]
    pub summary: RecordSummaryResponse,
    /// The release that answered, e.g. `1.4.0`.
    pub release_version: String,
    pub received_at: DateTime<Utc>,
    /// What the decision was asked.
    pub input: Value,
    /// What it answered, when it succeeded.
    pub output: Option<Value>,
    /// The error the caller received, when it failed.
    pub error: Option<Value>,
    /// The engine's per-node trace, when the Runtime had one.
    pub trace: Option<Value>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReplayResponse {
    /// Both succeeded with the same output, or both failed.
    pub identical: bool,
    pub status: RecordStatus,
    pub output: Option<Value>,
    /// The engine's error, when the replay failed.
    pub error: Option<Value>,
}

/// The project's decision records, newest first (any member). Lists hold
/// nothing the decisions read or answered.
#[utoipa::path(
    get,
    path = "/projects/{project_id}/decision-log",
    tag = "decision-log",
    params(("project_id" = Uuid, Path), RecordQuery),
    responses(
        (status = 200, body = RecordListResponse),
        (status = 400, description = "A filter is not valid (INVALID_REQUEST)", body = ErrorBody),
        (status = 404, description = "PROJECT_NOT_FOUND", body = ErrorBody),
    )
)]
pub async fn search(
    State(state): State<AppState>,
    ProjectAccess(access): ProjectAccess,
    Query(query): Query<RecordQuery>,
) -> Result<Json<RecordListResponse>, ApiError> {
    let text = |value: Option<String>, field: &'static str| -> Result<Option<String>, ApiError> {
        match value.map(|v| v.trim().to_owned()).filter(|v| !v.is_empty()) {
            Some(v) if v.chars().count() > MAX_TEXT_CHARS => Err(ApiError::InvalidField {
                field,
                message: format!("at most {MAX_TEXT_CHARS} characters"),
            }),
            other => Ok(other),
        }
    };
    let filter = Filter {
        reference: text(query.reference, "reference")?,
        decision_key: text(query.decision_key, "decisionKey")?,
        outcome: text(query.outcome, "outcome")?,
        environment: query.environment.map(environment),
        status: query.status.map(Status::from),
        from: query.from,
        to: query.until,
    };
    let page = state
        .decision_log
        .search(
            &access,
            &filter,
            PageRequest::new(query.limit, query.offset),
        )
        .await?;
    Ok(Json(RecordListResponse {
        items: page.items.into_iter().map(summary).collect(),
        total: page.total,
    }))
}

/// Opens a decision record (any member). Each opening is audited
/// (`decision_record.viewed`): records hold applicants' personal data.
#[utoipa::path(
    get,
    path = "/projects/{project_id}/decision-log/{record_id}",
    tag = "decision-log",
    params(("project_id" = Uuid, Path), ("record_id" = Uuid, Path)),
    responses(
        (status = 200, body = RecordResponse),
        (status = 404, description = "PROJECT_NOT_FOUND or RECORD_NOT_FOUND", body = ErrorBody),
    )
)]
pub async fn get(
    State(state): State<AppState>,
    ProjectAccess(access): ProjectAccess,
    RecordId(id): RecordId,
) -> Result<Json<RecordResponse>, ApiError> {
    let record = state.decision_log.view(&access, id).await?;
    let Payload {
        input,
        output,
        error,
        trace,
    } = record.payload;
    Ok(Json(RecordResponse {
        summary: summary(record.summary),
        release_version: record.release_version,
        received_at: record.received_at,
        input,
        output,
        error,
        trace,
    }))
}

/// Evaluates a record again with the release that answered it (any member);
/// connector nodes answer with what they answered then, so nothing is called.
/// Audited (`decision_record.replayed`).
#[utoipa::path(
    post,
    path = "/projects/{project_id}/decision-log/{record_id}/replay",
    tag = "decision-log",
    params(("project_id" = Uuid, Path), ("record_id" = Uuid, Path)),
    responses(
        (status = 200, body = ReplayResponse),
        (status = 404, description = "PROJECT_NOT_FOUND or RECORD_NOT_FOUND", body = ErrorBody),
    )
)]
pub async fn replay(
    State(state): State<AppState>,
    ProjectAccess(access): ProjectAccess,
    RecordId(id): RecordId,
) -> Result<Json<ReplayResponse>, ApiError> {
    let replay = state.decision_log.replay(&access, id).await?;
    Ok(Json(ReplayResponse {
        identical: replay.identical,
        status: replay.status.into(),
        output: replay.output,
        error: replay.error,
    }))
}

fn summary(record: RecordSummary) -> RecordSummaryResponse {
    RecordSummaryResponse {
        id: record.id,
        decision_key: record.decision_key,
        reference: record.reference,
        environment: record.environment.0.into(),
        status: record.status.into(),
        outcome: record.outcome,
        release_id: record.release_id,
        evaluated_at: record.evaluated_at,
        duration_us: record.duration_us,
    }
}
