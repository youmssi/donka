use crate::error::{ApiError, ErrorBody};
use crate::extract::{ApiJson, ApprovalId, ProjectAccess};
use crate::AppState;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use chrono::{DateTime, Utc};
use donka_licence::Module;
use serde::Deserialize;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

#[derive(Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct StartAnalysisBody {
    pub decision_key: String,
    pub from: DateTime<Utc>,
    pub until: DateTime<Utc>,
}

#[derive(Deserialize, IntoParams)]
pub struct AnalysisJobPath {
    pub job_id: Uuid,
}

/// Queues a bounded comparison of production records against a pending approval's release.
#[utoipa::path(
    post,
    path = "/projects/{project_id}/approvals/{approval_id}/analysis",
    tag = "analysis",
    params(("project_id" = Uuid, Path), ("approval_id" = Uuid, Path)),
    request_body = StartAnalysisBody,
    responses(
        (status = 202, body = donka_analysis::Job),
        (status = 403, description = "Only project owners can start analyses", body = ErrorBody),
        (status = 404, description = "Approval or decision not found", body = ErrorBody),
        (status = 422, description = "Configure an outcome field before starting analysis", body = ErrorBody),
        (status = 503, description = "Analysis is disabled", body = ErrorBody),
    )
)]
pub async fn start(
    State(state): State<AppState>,
    ProjectAccess(access): ProjectAccess,
    ApprovalId(approval_id): ApprovalId,
    ApiJson(body): ApiJson<StartAnalysisBody>,
) -> Result<(StatusCode, Json<donka_analysis::Job>), ApiError> {
    state.licensing.require(Module::Analysis)?;
    let job = state
        .analysis
        .start(
            &access,
            approval_id,
            body.decision_key,
            body.from,
            body.until,
        )
        .await?;
    Ok((StatusCode::ACCEPTED, Json(job)))
}

/// Returns the most recent run for an approval, if one exists.
#[utoipa::path(
    get,
    path = "/projects/{project_id}/approvals/{approval_id}/analysis",
    tag = "analysis",
    params(("project_id" = Uuid, Path), ("approval_id" = Uuid, Path)),
    responses(
        (status = 200, body = Option<donka_analysis::Job>),
        (status = 403, description = "Only project owners may view results", body = ErrorBody),
    )
)]
pub async fn latest_for_approval(
    State(state): State<AppState>,
    ProjectAccess(access): ProjectAccess,
    ApprovalId(approval_id): ApprovalId,
) -> Result<Json<Option<donka_analysis::Job>>, ApiError> {
    state.licensing.require(Module::Analysis)?;
    Ok(Json(
        state
            .analysis
            .latest_for_approval(&access, approval_id)
            .await?,
    ))
}

/// Reads progress and aggregates for an analysis in this project.
#[utoipa::path(
    get,
    path = "/projects/{project_id}/analysis/{job_id}",
    tag = "analysis",
    params(("project_id" = Uuid, Path), AnalysisJobPath),
    responses(
        (status = 200, body = donka_analysis::Job),
        (status = 403, description = "Only project owners may view results", body = ErrorBody),
        (status = 404, body = ErrorBody),
    )
)]
pub async fn get(
    State(state): State<AppState>,
    ProjectAccess(access): ProjectAccess,
    Path(AnalysisJobPath { job_id }): Path<AnalysisJobPath>,
) -> Result<Json<donka_analysis::Job>, ApiError> {
    state.licensing.require(Module::Analysis)?;
    Ok(Json(state.analysis.get(&access, job_id).await?))
}

/// Requests cancellation of a queued or running analysis.
#[utoipa::path(
    post,
    path = "/projects/{project_id}/analysis/{job_id}/cancel",
    tag = "analysis",
    params(("project_id" = Uuid, Path), AnalysisJobPath),
    responses(
        (status = 200, body = donka_analysis::Job),
        (status = 403, description = "Only project owners may cancel analyses", body = ErrorBody),
        (status = 404, body = ErrorBody),
    )
)]
pub async fn cancel(
    State(state): State<AppState>,
    ProjectAccess(access): ProjectAccess,
    Path(AnalysisJobPath { job_id }): Path<AnalysisJobPath>,
) -> Result<Json<donka_analysis::Job>, ApiError> {
    state.licensing.require(Module::Analysis)?;
    Ok(Json(state.analysis.cancel(&access, job_id).await?))
}
