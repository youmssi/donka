use crate::error::{ApiError, ErrorBody};
use crate::extract::{ApiJson, ApprovalId, ProjectAccess};
use crate::routes::people::{emails, PersonRef};
use crate::routes::releases::{
    one_deployment, summary, DeploymentResponse, PageQuery, ReleaseSummaryResponse,
};
use crate::routes::scenarios::TestSummaryResponse;
use crate::AppState;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::Json;
use chrono::{DateTime, Utc};
use donka_identity::Locale;
use donka_project::Role;
use donka_release::{
    Approval, ApprovalStatus, Approver, Change, DecisionChange, Language, MAX_REASON_CHARS,
};
use donka_shared::page::PageRequest;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use utoipa::openapi::schema::{ObjectBuilder, Type};
use utoipa::openapi::{RefOr, Schema};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Serialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum ApprovalStatusResponse {
    /// Waiting for an owner.
    Pending,
    /// Approved: the release was queued for production.
    Approved,
    Rejected,
    /// Withdrawn by the person who asked.
    Withdrawn,
}

impl From<ApprovalStatus> for ApprovalStatusResponse {
    fn from(status: ApprovalStatus) -> Self {
        match status {
            ApprovalStatus::Pending => Self::Pending,
            ApprovalStatus::Approved => Self::Approved,
            ApprovalStatus::Rejected => Self::Rejected,
            ApprovalStatus::Withdrawn => Self::Withdrawn,
        }
    }
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ApprovalResponse {
    pub id: Uuid,
    pub release_id: Uuid,
    pub release_version: String,
    pub release_notes: String,
    /// Who made the release: they cannot approve it.
    pub release_created_by: PersonRef,
    /// Who asked for production: they cannot approve it either.
    pub requested_by: PersonRef,
    pub requested_at: DateTime<Utc>,
    pub status: ApprovalStatusResponse,
    pub decided_by: Option<PersonRef>,
    pub decided_at: Option<DateTime<Utc>>,
    /// Why it was rejected.
    pub reason: Option<String>,
    /// The production deployment the approval queued.
    pub deployment_id: Option<Uuid>,
}

#[derive(Serialize, ToSchema)]
pub struct ApprovalListResponse {
    pub items: Vec<ApprovalResponse>,
    pub total: i64,
}

#[derive(Debug, Clone, Copy, Serialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum ChangeResponse {
    Added,
    Changed,
    Removed,
    Unchanged,
}

impl From<Change> for ChangeResponse {
    fn from(change: Change) -> Self {
        match change {
            Change::Added => Self::Added,
            Change::Changed => Self::Changed,
            Change::Removed => Self::Removed,
            Change::Unchanged => Self::Unchanged,
        }
    }
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DecisionChangeResponse {
    pub key: String,
    pub decision_id: Uuid,
    pub change: ChangeResponse,
    /// The version production runs, if any.
    pub from_version: Option<i32>,
    /// The version the release brings, if any.
    pub to_version: Option<i32>,
    /// The scenarios' results on the version the release brings.
    pub tests: Option<TestSummaryResponse>,
}

/// What an approver reviews: the request, production now, what changes and the tests.
#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ApprovalReviewResponse {
    #[serde(flatten)]
    pub approval: ApprovalResponse,
    /// The release production runs (or ran when the request was decided).
    pub production_version: Option<String>,
    pub changes: Vec<DecisionChangeResponse>,
    /// The scenarios' results on the release, added up.
    pub tests: TestSummaryResponse,
    /// Whether the person reading may approve or reject it now.
    pub can_decide: bool,
}

#[derive(Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RequestApprovalRequest {
    /// The release live on staging.
    pub release_id: Uuid,
}

#[derive(Deserialize, ToSchema)]
pub struct RejectRequest {
    #[schema(schema_with = reason_schema)]
    pub reason: String,
}

fn reason_schema() -> impl Into<RefOr<Schema>> {
    ObjectBuilder::new()
        .schema_type(Type::String)
        .min_length(Some(1))
        .max_length(Some(MAX_REASON_CHARS))
        .description(Some("Why the release may not go to production."))
}

/// The project's production requests, newest first (any member).
#[utoipa::path(
    get,
    path = "/projects/{project_id}/approvals",
    tag = "approvals",
    params(("project_id" = Uuid, Path), PageQuery),
    responses(
        (status = 200, body = ApprovalListResponse),
        (status = 404, description = "PROJECT_NOT_FOUND", body = ErrorBody),
    )
)]
pub async fn list(
    State(state): State<AppState>,
    ProjectAccess(access): ProjectAccess,
    Query(query): Query<PageQuery>,
) -> Result<Json<ApprovalListResponse>, ApiError> {
    let page = state
        .releases
        .approvals(&access, PageRequest::new(query.limit, query.offset))
        .await?;
    let emails = emails(&state, page.items.iter().flat_map(people)).await?;
    Ok(Json(ApprovalListResponse {
        items: page
            .items
            .into_iter()
            .map(|a| approval(&emails, a))
            .collect(),
        total: page.total,
    }))
}

/// Asks for the release live on staging to go to production; the owners who
/// may decide are emailed (editors and owners).
#[utoipa::path(
    post,
    path = "/projects/{project_id}/approvals",
    tag = "approvals",
    params(("project_id" = Uuid, Path)),
    request_body = RequestApprovalRequest,
    responses(
        (status = 201, body = ApprovalResponse),
        (status = 403, description = "Viewers cannot ask (FORBIDDEN)", body = ErrorBody),
        (status = 404, description = "PROJECT_NOT_FOUND or RELEASE_NOT_FOUND", body = ErrorBody),
        (status = 409, description = "APPROVAL_PENDING (another request waits) or PROJECT_ARCHIVED", body = ErrorBody),
        (status = 422, description = "NOT_ON_STAGING or NO_APPROVER (no other owner)", body = ErrorBody),
    )
)]
pub async fn request(
    State(state): State<AppState>,
    ProjectAccess(access): ProjectAccess,
    ApiJson(req): ApiJson<RequestApprovalRequest>,
) -> Result<(StatusCode, Json<ApprovalResponse>), ApiError> {
    let owners: Vec<Uuid> = state
        .projects
        .members(&access)
        .await?
        .into_iter()
        .filter(|member| member.role == Role::Owner)
        .map(|member| member.user_id)
        .collect();
    let owners = state
        .identity
        .users_by_ids(&owners)
        .await?
        .into_iter()
        .map(|user| Approver {
            user_id: user.id,
            email: user.email,
            language: match user.locale {
                Locale::En => Language::En,
                Locale::Fr => Language::Fr,
            },
        })
        .collect();
    let requested = state
        .releases
        .request_approval(&access, req.release_id, owners)
        .await?;
    Ok((StatusCode::CREATED, Json(one(&state, requested).await?)))
}

/// One request with what it would change in production and its test results (any member).
#[utoipa::path(
    get,
    path = "/projects/{project_id}/approvals/{approval_id}",
    tag = "approvals",
    params(("project_id" = Uuid, Path), ("approval_id" = Uuid, Path)),
    responses(
        (status = 200, body = ApprovalReviewResponse),
        (status = 404, description = "PROJECT_NOT_FOUND or APPROVAL_NOT_FOUND", body = ErrorBody),
    )
)]
pub async fn get(
    State(state): State<AppState>,
    ProjectAccess(access): ProjectAccess,
    ApprovalId(id): ApprovalId,
) -> Result<Json<ApprovalReviewResponse>, ApiError> {
    let review = state.releases.approval(&access, id).await?;
    let emails = emails(&state, people(&review.approval)).await?;
    let project = state.projects.get(&access).await?;
    let can_decide = review.approval.status == ApprovalStatus::Pending
        && access.role() == Role::Owner
        && project.archived_at.is_none()
        && !review.approval.is_author(access.user_id());
    Ok(Json(ApprovalReviewResponse {
        approval: approval(&emails, review.approval),
        production_version: review.production.map(|v| v.to_string()),
        changes: review.changes.into_iter().map(change).collect(),
        tests: review.tests.into(),
        can_decide,
    }))
}

/// Approves a request: the release is queued for production (owners who
/// neither made the release nor asked).
#[utoipa::path(
    post,
    path = "/projects/{project_id}/approvals/{approval_id}/approve",
    tag = "approvals",
    params(("project_id" = Uuid, Path), ("approval_id" = Uuid, Path)),
    responses(
        (status = 200, body = ApprovalResponse),
        (status = 403, description = "Only owners decide (FORBIDDEN)", body = ErrorBody),
        (status = 404, description = "PROJECT_NOT_FOUND or APPROVAL_NOT_FOUND", body = ErrorBody),
        (status = 409, description = "APPROVAL_DECIDED (someone decided first) or PROJECT_ARCHIVED", body = ErrorBody),
        (status = 422, description = "SELF_APPROVAL (you made the release or asked for it)", body = ErrorBody),
    )
)]
pub async fn approve(
    State(state): State<AppState>,
    ProjectAccess(access): ProjectAccess,
    ApprovalId(id): ApprovalId,
) -> Result<Json<ApprovalResponse>, ApiError> {
    let approved = state.releases.approve(&access, id).await?;
    Ok(Json(one(&state, approved).await?))
}

/// Rejects a request with a reason (owners who neither made the release nor asked).
#[utoipa::path(
    post,
    path = "/projects/{project_id}/approvals/{approval_id}/reject",
    tag = "approvals",
    params(("project_id" = Uuid, Path), ("approval_id" = Uuid, Path)),
    request_body = RejectRequest,
    responses(
        (status = 200, body = ApprovalResponse),
        (status = 400, description = "Missing or too long reason (INVALID_REQUEST)", body = ErrorBody),
        (status = 403, description = "Only owners decide (FORBIDDEN)", body = ErrorBody),
        (status = 404, description = "PROJECT_NOT_FOUND or APPROVAL_NOT_FOUND", body = ErrorBody),
        (status = 409, description = "APPROVAL_DECIDED (someone decided first) or PROJECT_ARCHIVED", body = ErrorBody),
        (status = 422, description = "SELF_APPROVAL (you made the release or asked for it)", body = ErrorBody),
    )
)]
pub async fn reject(
    State(state): State<AppState>,
    ProjectAccess(access): ProjectAccess,
    ApprovalId(id): ApprovalId,
    ApiJson(req): ApiJson<RejectRequest>,
) -> Result<Json<ApprovalResponse>, ApiError> {
    let rejected = state.releases.reject(&access, id, &req.reason).await?;
    Ok(Json(one(&state, rejected).await?))
}

/// Withdraws a request (the person who asked).
#[utoipa::path(
    post,
    path = "/projects/{project_id}/approvals/{approval_id}/withdraw",
    tag = "approvals",
    params(("project_id" = Uuid, Path), ("approval_id" = Uuid, Path)),
    responses(
        (status = 200, body = ApprovalResponse),
        (status = 403, description = "NOT_REQUESTER, or a viewer (FORBIDDEN)", body = ErrorBody),
        (status = 404, description = "PROJECT_NOT_FOUND or APPROVAL_NOT_FOUND", body = ErrorBody),
        (status = 409, description = "APPROVAL_DECIDED or PROJECT_ARCHIVED", body = ErrorBody),
    )
)]
pub async fn withdraw(
    State(state): State<AppState>,
    ProjectAccess(access): ProjectAccess,
    ApprovalId(id): ApprovalId,
) -> Result<Json<ApprovalResponse>, ApiError> {
    let withdrawn = state.releases.withdraw(&access, id).await?;
    Ok(Json(one(&state, withdrawn).await?))
}

fn people(a: &Approval) -> [Option<Uuid>; 3] {
    [
        Some(a.release_created_by),
        Some(a.requested_by),
        a.decided_by,
    ]
}

fn person(emails: &HashMap<Uuid, String>, id: Uuid) -> PersonRef {
    PersonRef {
        id,
        email: emails.get(&id).cloned().unwrap_or_default(),
    }
}

fn approval(emails: &HashMap<Uuid, String>, a: Approval) -> ApprovalResponse {
    ApprovalResponse {
        id: a.id,
        release_id: a.release_id,
        release_version: a.release_version.to_string(),
        release_notes: a.release_notes,
        release_created_by: person(emails, a.release_created_by),
        requested_by: person(emails, a.requested_by),
        requested_at: a.requested_at,
        status: a.status.into(),
        decided_by: a.decided_by.map(|id| person(emails, id)),
        decided_at: a.decided_at,
        reason: a.reason,
        deployment_id: a.deployment_id,
    }
}

async fn one(state: &AppState, a: Approval) -> Result<ApprovalResponse, ApiError> {
    let emails = emails(state, people(&a)).await?;
    Ok(approval(&emails, a))
}

fn change(c: DecisionChange) -> DecisionChangeResponse {
    DecisionChangeResponse {
        key: c.key,
        decision_id: c.decision_id,
        change: c.change.into(),
        from_version: c.from_version,
        to_version: c.to_version,
        tests: c.tests.map(Into::into),
    }
}

#[derive(Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RollbackRequest {
    /// A release once approved for production, not the one live now.
    pub release_id: Uuid,
    #[schema(schema_with = rollback_reason_schema)]
    pub reason: String,
}

fn rollback_reason_schema() -> impl Into<RefOr<Schema>> {
    ObjectBuilder::new()
        .schema_type(Type::String)
        .min_length(Some(1))
        .max_length(Some(MAX_REASON_CHARS))
        .description(Some(
            "Why production goes back, for the audit log and the team.",
        ))
}

#[derive(Serialize, ToSchema)]
pub struct RollbackTargetsResponse {
    /// Releases once approved for production, except the live one, newest first.
    pub items: Vec<ReleaseSummaryResponse>,
}

/// The releases production can be rolled back to (any member).
#[utoipa::path(
    get,
    path = "/projects/{project_id}/rollback-targets",
    tag = "approvals",
    params(("project_id" = Uuid, Path)),
    responses(
        (status = 200, body = RollbackTargetsResponse),
        (status = 404, description = "PROJECT_NOT_FOUND", body = ErrorBody),
    )
)]
pub async fn rollback_targets(
    State(state): State<AppState>,
    ProjectAccess(access): ProjectAccess,
) -> Result<Json<RollbackTargetsResponse>, ApiError> {
    let targets = state.releases.rollback_targets(&access).await?;
    let emails = emails(&state, targets.iter().map(|r| Some(r.created_by))).await?;
    Ok(Json(RollbackTargetsResponse {
        items: targets.into_iter().map(|r| summary(&emails, r)).collect(),
    }))
}

/// Puts a release once approved for production back in production, with a
/// reason and without a new approval (owners).
#[utoipa::path(
    post,
    path = "/projects/{project_id}/rollbacks",
    tag = "approvals",
    params(("project_id" = Uuid, Path)),
    request_body = RollbackRequest,
    responses(
        (status = 202, description = "Queued for production", body = DeploymentResponse),
        (status = 400, description = "Missing or too long reason (INVALID_REQUEST)", body = ErrorBody),
        (status = 403, description = "Only owners roll back (FORBIDDEN)", body = ErrorBody),
        (status = 404, description = "PROJECT_NOT_FOUND or RELEASE_NOT_FOUND", body = ErrorBody),
        (status = 409, description = "PROJECT_ARCHIVED", body = ErrorBody),
        (status = 422, description = "NEVER_APPROVED or ALREADY_LIVE", body = ErrorBody),
    )
)]
pub async fn rollback(
    State(state): State<AppState>,
    ProjectAccess(access): ProjectAccess,
    ApiJson(req): ApiJson<RollbackRequest>,
) -> Result<(StatusCode, Json<DeploymentResponse>), ApiError> {
    let queued = state
        .releases
        .rollback(&access, req.release_id, &req.reason)
        .await?;
    Ok((
        StatusCode::ACCEPTED,
        Json(one_deployment(&state, queued).await?),
    ))
}
