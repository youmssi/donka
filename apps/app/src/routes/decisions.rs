use crate::error::{ApiError, ErrorBody};
use crate::extract::{ApiJson, DecisionId, ProjectAccess};
use crate::routes::people::{emails, person, PersonRef};
use crate::routes::simulate::SimulateResponse;
use crate::AppState;
use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use chrono::{DateTime, Utc};
use donka_decision::{Decision, DecisionError, DecisionSummary, KEY_PATTERN, MAX_KEY_CHARS};
use donka_engine::EvaluateOptions;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use utoipa::openapi::schema::{AdditionalProperties, ObjectBuilder, Type};
use utoipa::openapi::{RefOr, Schema};
use utoipa::ToSchema;

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DecisionSummaryResponse {
    pub id: uuid::Uuid,
    /// How graphs call this decision, e.g. `bureau/normalize`.
    pub key: String,
    /// Grows by one on every save; send it back when saving.
    pub revision: i32,
    pub updated_at: DateTime<Utc>,
    pub updated_by: PersonRef,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DecisionResponse {
    #[serde(flatten)]
    pub summary: DecisionSummaryResponse,
    /// The draft: a JDM graph.
    #[schema(schema_with = graph_schema)]
    pub content: Value,
}

#[derive(Serialize, ToSchema)]
pub struct DecisionListResponse {
    pub items: Vec<DecisionSummaryResponse>,
}

/// A JDM graph: an object with any fields (`nodes`, `edges`, `contentType`…).
fn graph_schema() -> impl Into<RefOr<Schema>> {
    ObjectBuilder::new()
        .schema_type(Type::Object)
        .additional_properties(Some(AdditionalProperties::FreeForm(true)))
        .description(Some("A JDM decision graph."))
}

fn key_schema() -> impl Into<RefOr<Schema>> {
    ObjectBuilder::new()
        .schema_type(Type::String)
        .min_length(Some(1))
        .max_length(Some(MAX_KEY_CHARS))
        .pattern(Some(KEY_PATTERN))
        .description(Some(
            "Lowercase words with single hyphens; folders separated by `/`.",
        ))
}

#[derive(Deserialize, ToSchema)]
pub struct CreateDecisionRequest {
    #[schema(schema_with = key_schema)]
    pub key: String,
    /// A JDM graph to start from; empty when absent.
    #[schema(schema_with = graph_schema, required = false)]
    pub content: Option<Value>,
}

#[derive(Deserialize, ToSchema)]
pub struct SaveDecisionRequest {
    /// The whole draft: a JDM graph.
    #[schema(schema_with = graph_schema)]
    pub content: Value,
    /// The revision this draft was loaded or last saved at.
    pub revision: i32,
}

#[derive(Deserialize, ToSchema)]
pub struct SimulateDecisionRequest {
    /// Input passed to the decision.
    #[serde(default)]
    #[schema(value_type = Object)]
    pub context: Value,
    /// The editor's content of this decision, saved or not; the saved draft when absent.
    /// Every other decision of the project is taken from its saved draft.
    #[schema(schema_with = graph_schema, required = false)]
    pub content: Option<Value>,
}

/// The project's decisions, by key (any member).
#[utoipa::path(
    get,
    path = "/projects/{project_id}/decisions",
    tag = "decisions",
    params(("project_id" = uuid::Uuid, Path)),
    responses(
        (status = 200, body = DecisionListResponse),
        (status = 404, description = "PROJECT_NOT_FOUND", body = ErrorBody),
    )
)]
pub async fn list(
    State(state): State<AppState>,
    ProjectAccess(access): ProjectAccess,
) -> Result<Json<DecisionListResponse>, ApiError> {
    let decisions = state.decisions.list(&access).await?;
    let emails = emails(&state, decisions.iter().map(|d| Some(d.updated_by))).await?;
    Ok(Json(DecisionListResponse {
        items: decisions.into_iter().map(|d| summary(&emails, d)).collect(),
    }))
}

/// A new decision, empty unless content is given (editors and owners).
#[utoipa::path(
    post,
    path = "/projects/{project_id}/decisions",
    tag = "decisions",
    params(("project_id" = uuid::Uuid, Path)),
    request_body = CreateDecisionRequest,
    responses(
        (status = 201, body = DecisionResponse),
        (status = 400, description = "Invalid key (INVALID_REQUEST)", body = ErrorBody),
        (status = 403, description = "Viewers cannot create decisions (FORBIDDEN)", body = ErrorBody),
        (status = 404, description = "PROJECT_NOT_FOUND", body = ErrorBody),
        (status = 409, description = "DECISION_KEY_TAKEN or PROJECT_ARCHIVED", body = ErrorBody),
        (status = 422, description = "The content is not a decision model (INVALID_DECISION)", body = ErrorBody),
    )
)]
pub async fn create(
    State(state): State<AppState>,
    ProjectAccess(access): ProjectAccess,
    ApiJson(req): ApiJson<CreateDecisionRequest>,
) -> Result<(StatusCode, Json<DecisionResponse>), ApiError> {
    let decision = state
        .decisions
        .create(&access, &req.key, req.content)
        .await?;
    Ok((StatusCode::CREATED, Json(full(&state, decision).await?)))
}

/// One decision with its draft (any member).
#[utoipa::path(
    get,
    path = "/projects/{project_id}/decisions/{decision_id}",
    tag = "decisions",
    params(("project_id" = uuid::Uuid, Path), ("decision_id" = uuid::Uuid, Path)),
    responses(
        (status = 200, body = DecisionResponse),
        (status = 404, description = "PROJECT_NOT_FOUND or DECISION_NOT_FOUND", body = ErrorBody),
    )
)]
pub async fn get(
    State(state): State<AppState>,
    ProjectAccess(access): ProjectAccess,
    DecisionId(id): DecisionId,
) -> Result<Json<DecisionResponse>, ApiError> {
    let decision = state.decisions.get(&access, id).await?;
    Ok(Json(full(&state, decision).await?))
}

/// Saves the draft (editors and owners). Send the revision the draft was loaded
/// at: when someone saved since, the answer is `409 DECISION_CONFLICT` with who
/// and when in `details`, and nothing is overwritten.
#[utoipa::path(
    put,
    path = "/projects/{project_id}/decisions/{decision_id}",
    tag = "decisions",
    params(("project_id" = uuid::Uuid, Path), ("decision_id" = uuid::Uuid, Path)),
    request_body = SaveDecisionRequest,
    responses(
        (status = 200, description = "Saved; the new revision", body = DecisionSummaryResponse),
        (status = 403, description = "Viewers cannot save (FORBIDDEN)", body = ErrorBody),
        (status = 404, description = "PROJECT_NOT_FOUND or DECISION_NOT_FOUND", body = ErrorBody),
        (status = 409, description = "DECISION_CONFLICT (details: revision, updatedAt, updatedBy) or PROJECT_ARCHIVED", body = ErrorBody),
        (status = 422, description = "The content is not a decision model (INVALID_DECISION)", body = ErrorBody),
    )
)]
pub async fn save(
    State(state): State<AppState>,
    ProjectAccess(access): ProjectAccess,
    DecisionId(id): DecisionId,
    ApiJson(req): ApiJson<SaveDecisionRequest>,
) -> Result<Json<DecisionSummaryResponse>, ApiError> {
    match state
        .decisions
        .save_draft(&access, id, req.content, req.revision)
        .await
    {
        Ok(saved) => {
            let emails = emails(&state, [Some(saved.updated_by)]).await?;
            Ok(Json(summary(&emails, summary_of(saved))))
        }
        Err(DecisionError::Conflict { current }) => {
            let emails = emails(&state, [Some(current.updated_by)]).await?;
            Err(ApiError::DecisionConflict(json!({
                "revision": current.revision,
                "updatedAt": current.updated_at,
                "updatedBy": person(&emails, Some(current.updated_by)),
            })))
        }
        Err(err) => Err(err.into()),
    }
}

/// Deletes a decision and its draft (editors and owners).
#[utoipa::path(
    delete,
    path = "/projects/{project_id}/decisions/{decision_id}",
    tag = "decisions",
    params(("project_id" = uuid::Uuid, Path), ("decision_id" = uuid::Uuid, Path)),
    responses(
        (status = 204, description = "Deleted"),
        (status = 403, description = "Viewers cannot delete (FORBIDDEN)", body = ErrorBody),
        (status = 404, description = "PROJECT_NOT_FOUND or DECISION_NOT_FOUND", body = ErrorBody),
        (status = 409, description = "PROJECT_ARCHIVED", body = ErrorBody),
    )
)]
pub async fn delete(
    State(state): State<AppState>,
    ProjectAccess(access): ProjectAccess,
    DecisionId(id): DecisionId,
) -> Result<StatusCode, ApiError> {
    state.decisions.delete(&access, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Evaluates this decision with every other decision of the project, as a
/// release would, and returns the result with a per-node trace (any member:
/// viewers simulate too). Nothing is saved.
#[utoipa::path(
    post,
    path = "/projects/{project_id}/decisions/{decision_id}/simulate",
    tag = "decisions",
    params(("project_id" = uuid::Uuid, Path), ("decision_id" = uuid::Uuid, Path)),
    request_body = SimulateDecisionRequest,
    responses(
        (status = 200, description = "Result and per-node trace", body = SimulateResponse),
        (status = 404, description = "PROJECT_NOT_FOUND or DECISION_NOT_FOUND", body = ErrorBody),
        (status = 422, description = "A decision of the project is not a valid model (INVALID_DECISION), or the evaluation failed (EVALUATION_FAILED, trace in details)", body = ErrorBody),
    )
)]
pub async fn simulate(
    State(state): State<AppState>,
    ProjectAccess(access): ProjectAccess,
    DecisionId(id): DecisionId,
    ApiJson(req): ApiJson<SimulateDecisionRequest>,
) -> Result<Json<SimulateResponse>, ApiError> {
    let (bundle, key) = state.decisions.bundle(&access, id, req.content).await?;
    let evaluation = state
        .runtime
        .evaluate(&bundle, &key, req.context, EvaluateOptions { trace: true })
        .await?;
    Ok(Json(SimulateResponse {
        result: evaluation.result,
        trace: evaluation.trace.unwrap_or(Value::Null),
        performance: evaluation.performance,
    }))
}

fn summary_of(decision: Decision) -> DecisionSummary {
    DecisionSummary {
        id: decision.id,
        key: decision.key,
        revision: decision.revision,
        updated_at: decision.updated_at,
        updated_by: decision.updated_by,
    }
}

fn summary(
    emails: &std::collections::HashMap<uuid::Uuid, String>,
    decision: DecisionSummary,
) -> DecisionSummaryResponse {
    DecisionSummaryResponse {
        id: decision.id,
        key: decision.key,
        revision: decision.revision,
        updated_at: decision.updated_at,
        updated_by: PersonRef {
            id: decision.updated_by,
            email: emails
                .get(&decision.updated_by)
                .cloned()
                .unwrap_or_default(),
        },
    }
}

async fn full(state: &AppState, mut decision: Decision) -> Result<DecisionResponse, ApiError> {
    let emails = emails(state, [Some(decision.updated_by)]).await?;
    let content = std::mem::take(&mut decision.content);
    Ok(DecisionResponse {
        summary: summary(&emails, summary_of(decision)),
        content,
    })
}
