use crate::error::{ApiError, ErrorBody};
use crate::extract::{ApiJson, DecisionId, ProjectAccess, VersionNumber};
use crate::routes::decisions::{conflict_or, full, graph_schema, DecisionResponse};
use crate::routes::people::{emails, PersonRef};
use crate::routes::scenarios::TestSummaryResponse;
use crate::AppState;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::Json;
use chrono::{DateTime, Utc};
use donka_decision::{VersionSummary, MAX_MESSAGE_CHARS};
use donka_shared::page::PageRequest;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use utoipa::openapi::schema::{ObjectBuilder, Type};
use utoipa::openapi::{RefOr, Schema};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DecisionVersionResponse {
    /// 1, 2, 3… per decision.
    pub number: i32,
    pub message: String,
    pub created_at: DateTime<Utc>,
    pub created_by: PersonRef,
    /// The older version this one restores, when it does.
    pub restored_from: Option<i32>,
    /// How the project's test scenarios went when this version was saved.
    pub tests: TestSummaryResponse,
}

#[derive(Serialize, ToSchema)]
pub struct DecisionVersionDetailResponse {
    #[serde(flatten)]
    pub version: DecisionVersionResponse,
    /// The JDM graph as it was saved.
    #[schema(schema_with = graph_schema)]
    pub content: Value,
}

#[derive(Serialize, ToSchema)]
pub struct DecisionVersionListResponse {
    pub items: Vec<DecisionVersionResponse>,
    /// Versions of the decision, across all pages.
    pub total: i64,
}

#[derive(Serialize, ToSchema)]
pub struct RestoreResponse {
    /// The decision, its draft now the restored content.
    pub decision: DecisionResponse,
    /// The new version the restore created.
    pub version: DecisionVersionResponse,
}

fn message_schema() -> impl Into<RefOr<Schema>> {
    ObjectBuilder::new()
        .schema_type(Type::String)
        .min_length(Some(1))
        .max_length(Some(MAX_MESSAGE_CHARS))
        .description(Some("What changed and why, for the history."))
}

#[derive(Deserialize, ToSchema)]
pub struct SaveVersionRequest {
    #[schema(schema_with = message_schema)]
    pub message: String,
    /// The draft revision the editor shows: the version is exactly that draft.
    pub revision: i32,
}

#[derive(Deserialize, ToSchema)]
pub struct RestoreRequest {
    /// The draft revision the editor shows; `409 DECISION_CONFLICT` when it moved on.
    pub revision: i32,
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct VersionsQuery {
    /// Page size, 1 to 100 (default 50).
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

/// The decision's versions, newest first (any member).
#[utoipa::path(
    get,
    path = "/projects/{project_id}/decisions/{decision_id}/versions",
    tag = "versions",
    params(("project_id" = Uuid, Path), ("decision_id" = Uuid, Path), VersionsQuery),
    responses(
        (status = 200, body = DecisionVersionListResponse),
        (status = 404, description = "PROJECT_NOT_FOUND or DECISION_NOT_FOUND", body = ErrorBody),
    )
)]
pub async fn list(
    State(state): State<AppState>,
    ProjectAccess(access): ProjectAccess,
    DecisionId(id): DecisionId,
    Query(query): Query<VersionsQuery>,
) -> Result<Json<DecisionVersionListResponse>, ApiError> {
    let page = state
        .decisions
        .versions(&access, id, PageRequest::new(query.limit, query.offset))
        .await?;
    let emails = emails(&state, page.items.iter().map(|v| Some(v.created_by))).await?;
    Ok(Json(DecisionVersionListResponse {
        items: page
            .items
            .into_iter()
            .map(|v| response(&emails, v))
            .collect(),
        total: page.total,
    }))
}

/// Saves the draft as the next version (editors and owners).
#[utoipa::path(
    post,
    path = "/projects/{project_id}/decisions/{decision_id}/versions",
    tag = "versions",
    params(("project_id" = Uuid, Path), ("decision_id" = Uuid, Path)),
    request_body = SaveVersionRequest,
    responses(
        (status = 201, body = DecisionVersionResponse),
        (status = 400, description = "Missing or too long message (INVALID_REQUEST)", body = ErrorBody),
        (status = 403, description = "Viewers cannot save versions (FORBIDDEN)", body = ErrorBody),
        (status = 404, description = "PROJECT_NOT_FOUND or DECISION_NOT_FOUND", body = ErrorBody),
        (status = 409, description = "DECISION_CONFLICT (the draft moved on) or PROJECT_ARCHIVED", body = ErrorBody),
        (status = 422, description = "Nothing changed since the latest version (VERSION_UNCHANGED, details: version)", body = ErrorBody),
    )
)]
pub async fn save(
    State(state): State<AppState>,
    ProjectAccess(access): ProjectAccess,
    DecisionId(id): DecisionId,
    ApiJson(req): ApiJson<SaveVersionRequest>,
) -> Result<(StatusCode, Json<DecisionVersionResponse>), ApiError> {
    let saved = state
        .decisions
        .save_version(&access, id, req.revision, &req.message)
        .await;
    let version = conflict_or(&state, saved).await?;
    let emails = emails(&state, [Some(version.created_by)]).await?;
    Ok((StatusCode::CREATED, Json(response(&emails, version))))
}

/// One version with its content, to compare or restore (any member).
#[utoipa::path(
    get,
    path = "/projects/{project_id}/decisions/{decision_id}/versions/{number}",
    tag = "versions",
    params(("project_id" = Uuid, Path), ("decision_id" = Uuid, Path), ("number" = i32, Path)),
    responses(
        (status = 200, body = DecisionVersionDetailResponse),
        (status = 404, description = "PROJECT_NOT_FOUND, DECISION_NOT_FOUND or VERSION_NOT_FOUND", body = ErrorBody),
    )
)]
pub async fn get(
    State(state): State<AppState>,
    ProjectAccess(access): ProjectAccess,
    DecisionId(id): DecisionId,
    VersionNumber(number): VersionNumber,
) -> Result<Json<DecisionVersionDetailResponse>, ApiError> {
    let version = state.decisions.version(&access, id, number).await?;
    let emails = emails(&state, [Some(version.summary.created_by)]).await?;
    Ok(Json(DecisionVersionDetailResponse {
        version: response(&emails, version.summary),
        content: version.content,
    }))
}

/// Restores a version (editors and owners): its content becomes the draft and a
/// new version; the history only grows.
#[utoipa::path(
    post,
    path = "/projects/{project_id}/decisions/{decision_id}/versions/{number}/restore",
    tag = "versions",
    params(("project_id" = Uuid, Path), ("decision_id" = Uuid, Path), ("number" = i32, Path)),
    request_body = RestoreRequest,
    responses(
        (status = 201, body = RestoreResponse),
        (status = 403, description = "Viewers cannot restore (FORBIDDEN)", body = ErrorBody),
        (status = 404, description = "PROJECT_NOT_FOUND, DECISION_NOT_FOUND or VERSION_NOT_FOUND", body = ErrorBody),
        (status = 409, description = "DECISION_CONFLICT (the draft moved on) or PROJECT_ARCHIVED", body = ErrorBody),
    )
)]
pub async fn restore(
    State(state): State<AppState>,
    ProjectAccess(access): ProjectAccess,
    DecisionId(id): DecisionId,
    VersionNumber(number): VersionNumber,
    ApiJson(req): ApiJson<RestoreRequest>,
) -> Result<(StatusCode, Json<RestoreResponse>), ApiError> {
    let restored = state
        .decisions
        .restore(&access, id, number, req.revision)
        .await;
    let (decision, version) = conflict_or(&state, restored).await?;
    let emails = emails(&state, [Some(version.created_by)]).await?;
    Ok((
        StatusCode::CREATED,
        Json(RestoreResponse {
            decision: full(&state, decision).await?,
            version: response(&emails, version),
        }),
    ))
}

fn response(emails: &HashMap<Uuid, String>, version: VersionSummary) -> DecisionVersionResponse {
    DecisionVersionResponse {
        number: version.number,
        message: version.message,
        created_at: version.created_at,
        created_by: PersonRef {
            id: version.created_by,
            email: emails.get(&version.created_by).cloned().unwrap_or_default(),
        },
        restored_from: version.restored_from,
        tests: version.tests.into(),
    }
}
