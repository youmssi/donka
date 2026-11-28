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

// ----- Population stability (DNK-57) ---------------------------------------

/// Which decision to measure, over which two periods, and what to take the indices on.
#[derive(Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotBody {
    pub decision_key: String,
    pub reference_from: DateTime<Utc>,
    pub reference_until: DateTime<Utc>,
    pub recent_from: DateTime<Utc>,
    pub recent_until: DateTime<Utc>,
    /// The numeric output the shift index is taken on, usually the score, and its bin edges.
    /// Omitted measures the contract inputs alone.
    #[serde(default)]
    pub output_field: Option<String>,
    #[serde(default)]
    pub output_bins: Vec<f64>,
    /// Dotted paths of the contract inputs to measure.
    #[serde(default)]
    pub input_fields: Vec<String>,
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
#[serde(rename_all = "camelCase")]
pub struct StabilityQuery {
    /// Only this decision, by key.
    pub decision_key: String,
}

/// Takes a population stability snapshot: the decision mix by week, and the shift of the chosen
/// output and each input between a reference period and a recent one.
#[utoipa::path(
    post,
    path = "/projects/{project_id}/stability",
    tag = "analysis",
    params(("project_id" = Uuid, Path)),
    request_body = SnapshotBody,
    responses(
        (status = 201, body = donka_analysis::snapshot::Snapshot),
        (status = 403, description = "Only project owners may take a snapshot", body = ErrorBody),
        (status = 422, description = "A period ends before it starts", body = ErrorBody),
        (status = 503, description = "Analysis is disabled", body = ErrorBody),
    )
)]
pub async fn take_snapshot(
    State(state): State<AppState>,
    ProjectAccess(access): ProjectAccess,
    ApiJson(body): ApiJson<SnapshotBody>,
) -> Result<(StatusCode, Json<donka_analysis::snapshot::Snapshot>), ApiError> {
    state.licensing.require(Module::Analysis)?;
    let request = donka_analysis::snapshot::SnapshotRequest {
        decision_key: body.decision_key,
        reference_from: body.reference_from,
        reference_until: body.reference_until,
        recent_from: body.recent_from,
        recent_until: body.recent_until,
        output_field: body
            .output_field
            .map(|field| (field, body.output_bins.clone())),
        input_fields: body.input_fields,
    };

    // Owners are read, and their addresses with them, only when alerts are configured on: a
    // snapshot taken with alerts off has no reason to read anyone's address.
    let owners = if state.stability.alerts {
        stability_owners(&state, &access).await?
    } else {
        Vec::new()
    };
    let project = state.projects.get(&access).await?;
    let context = (state.stability.alerts && !owners.is_empty()).then(|| {
        donka_analysis::snapshot::AlertContext {
            project: donka_analysis::alerts::AlertProject {
                key: &project.key,
                name: &project.name,
            },
            public_url: &state.public_url,
            owners: &owners,
        }
    });

    let (snapshot, _alerted) = state
        .analysis
        .snapshot(
            &access,
            &request,
            state.stability.thresholds,
            state.stability.minimum_bin,
            context.as_ref(),
        )
        .await?;
    Ok((StatusCode::CREATED, Json(snapshot)))
}

/// The snapshots taken for one decision, newest first.
#[utoipa::path(
    get,
    path = "/projects/{project_id}/stability",
    tag = "analysis",
    params(("project_id" = Uuid, Path), StabilityQuery),
    responses(
        (status = 200, body = Vec<donka_analysis::snapshot::Snapshot>),
        (status = 403, description = "Only project owners may read snapshots", body = ErrorBody),
        (status = 503, description = "Analysis is disabled", body = ErrorBody),
    )
)]
pub async fn snapshots(
    State(state): State<AppState>,
    ProjectAccess(access): ProjectAccess,
    axum::extract::Query(query): axum::extract::Query<StabilityQuery>,
) -> Result<Json<Vec<donka_analysis::snapshot::Snapshot>>, ApiError> {
    state.licensing.require(Module::Analysis)?;
    Ok(Json(
        state
            .analysis
            .snapshots(&access, &query.decision_key, 50)
            .await?,
    ))
}

/// The project's owners, with the language each reads, as the release module's approvers are
/// assembled: the analysis module queries no other module's tables.
async fn stability_owners(
    state: &AppState,
    access: &donka_project::Access,
) -> Result<Vec<donka_analysis::alerts::Recipient>, ApiError> {
    let owners: Vec<Uuid> = state
        .projects
        .members(access)
        .await?
        .into_iter()
        .filter(|member| member.role == donka_project::Role::Owner)
        .map(|member| member.user_id)
        .collect();
    Ok(state
        .identity
        .users_by_ids(&owners)
        .await?
        .into_iter()
        .map(|user| donka_analysis::alerts::Recipient {
            email: user.email,
            language: match user.locale {
                donka_identity::Locale::En => donka_analysis::alerts::Language::En,
                donka_identity::Locale::Fr => donka_analysis::alerts::Language::Fr,
            },
        })
        .collect())
}
