use crate::error::{ApiError, ErrorBody};
use crate::extract::{ApiJson, DeploymentId, EnvironmentKey, ProjectAccess, ReleaseId, TokenId};
use crate::routes::people::{emails, PersonRef};
use crate::routes::scenarios::TestSummaryResponse;
use crate::AppState;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::Json;
use chrono::{DateTime, Utc};
use donka_release::{
    Bump, Deployment, DeploymentReason, DeploymentStatus, Environment, EnvironmentState, Release,
    ReleaseSummary, ReleasedDecision, RuntimeToken, SemVer, MAX_NOTES_CHARS, MAX_TOKEN_NAME_CHARS,
};
use donka_shared::page::PageRequest;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use utoipa::openapi::schema::{ObjectBuilder, Type};
use utoipa::openapi::{RefOr, Schema};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

/// One of the two environments every project has.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum EnvironmentName {
    Staging,
    Production,
}

impl From<Environment> for EnvironmentName {
    fn from(environment: Environment) -> Self {
        match environment {
            Environment::Staging => EnvironmentName::Staging,
            Environment::Production => EnvironmentName::Production,
        }
    }
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReleasedDecisionResponse {
    pub decision_id: Uuid,
    pub key: String,
    /// The version of the decision the release froze.
    pub version: i32,
    /// How the project's scenarios went on that version.
    pub tests: TestSummaryResponse,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseSummaryResponse {
    pub id: Uuid,
    /// Semantic version, e.g. `1.4.0`.
    pub version: String,
    pub notes: String,
    pub created_at: DateTime<Utc>,
    pub created_by: PersonRef,
    /// How many decisions it froze.
    pub decisions: i64,
    /// The scenarios' results on the frozen versions, added up.
    pub tests: TestSummaryResponse,
    /// The environments this release is live in.
    pub live_in: Vec<EnvironmentName>,
}

#[derive(Serialize, ToSchema)]
pub struct ReleaseResponse {
    #[serde(flatten)]
    pub release: ReleaseSummaryResponse,
    pub decisions: Vec<ReleasedDecisionResponse>,
}

#[derive(Serialize, ToSchema)]
pub struct ReleaseListResponse {
    pub items: Vec<ReleaseSummaryResponse>,
    /// Releases across all pages.
    pub total: i64,
}

/// The version each bump would give the next release.
#[derive(Serialize, ToSchema)]
pub struct NextVersionsResponse {
    pub major: String,
    pub minor: String,
    pub patch: String,
}

/// What a release made now would hold.
#[derive(Serialize, ToSchema)]
pub struct ReleasePreviewResponse {
    /// The latest release's version; none before the first release.
    pub latest: Option<String>,
    pub next: NextVersionsResponse,
    pub decisions: Vec<ReleasedDecisionResponse>,
    /// Decisions without a version: a release cannot be made while any remain.
    pub unversioned: Vec<String>,
}

/// Which part of the version the release raises.
#[derive(Debug, Clone, Copy, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum BumpRequest {
    Major,
    Minor,
    Patch,
}

#[derive(Deserialize, ToSchema)]
pub struct CreateReleaseRequest {
    pub bump: BumpRequest,
    #[schema(schema_with = notes_schema)]
    pub notes: String,
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct PageQuery {
    /// Page size, 1 to 100 (default 50).
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Debug, Clone, Copy, Serialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum DeploymentStatusResponse {
    /// Waiting for the publisher.
    Pending,
    /// A write failed; it is tried again at `nextAttemptAt`.
    Retrying,
    /// Gave up after too many failed writes; `POST …/retry` tries again.
    Failed,
    Published,
    /// A newer deployment of the environment replaced it before it was published.
    Superseded,
}

/// Why a deployment exists.
#[derive(Debug, Clone, Copy, Serialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum DeploymentReasonResponse {
    /// Someone deployed the release.
    Deploy,
    /// The environment's tokens changed: its release was published again.
    Tokens,
    /// An owner put a release once approved for production back (see `rollbackReason`).
    Rollback,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DeploymentResponse {
    pub id: Uuid,
    pub environment: EnvironmentName,
    pub release_id: Uuid,
    pub release_version: String,
    pub reason: DeploymentReasonResponse,
    /// Why production was rolled back, for a rollback.
    pub rollback_reason: Option<String>,
    pub requested_at: DateTime<Utc>,
    pub requested_by: PersonRef,
    pub status: DeploymentStatusResponse,
    /// Writes tried so far.
    pub attempts: i32,
    /// Why the last write failed.
    pub last_error: Option<String>,
    /// When the next write is tried, while it waits.
    pub next_attempt_at: Option<DateTime<Utc>>,
    pub published_at: Option<DateTime<Utc>>,
}

#[derive(Serialize, ToSchema)]
pub struct DeploymentListResponse {
    pub items: Vec<DeploymentResponse>,
    pub total: i64,
}

#[derive(Serialize, ToSchema)]
pub struct EnvironmentResponse {
    pub environment: EnvironmentName,
    /// The deployment live now (the release customer systems get).
    pub live: Option<DeploymentResponse>,
    /// The latest deployment asked for, which may still be on its way.
    pub latest: Option<DeploymentResponse>,
    /// Live Runtime tokens.
    pub tokens: i64,
}

#[derive(Serialize, ToSchema)]
pub struct EnvironmentListResponse {
    pub items: Vec<EnvironmentResponse>,
}

#[derive(Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DeployRequest {
    pub release_id: Uuid,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TokenResponse {
    pub id: Uuid,
    pub name: String,
    /// The token's last characters, to tell tokens apart.
    pub hint: String,
    pub created_at: DateTime<Utc>,
    pub created_by: PersonRef,
    pub revoked_at: Option<DateTime<Utc>>,
    pub revoked_by: Option<PersonRef>,
}

#[derive(Serialize, ToSchema)]
pub struct TokenListResponse {
    pub items: Vec<TokenResponse>,
}

#[derive(Serialize, ToSchema)]
pub struct IssuedTokenResponse {
    /// The token. Shown this once: Studio keeps only its hash.
    pub token: String,
    #[serde(flatten)]
    pub details: TokenResponse,
}

#[derive(Deserialize, ToSchema)]
pub struct IssueTokenRequest {
    #[schema(schema_with = token_name_schema)]
    pub name: String,
}

fn notes_schema() -> impl Into<RefOr<Schema>> {
    ObjectBuilder::new()
        .schema_type(Type::String)
        .min_length(Some(1))
        .max_length(Some(MAX_NOTES_CHARS))
        .description(Some(
            "What the release changes, for approvers and the history.",
        ))
}

fn token_name_schema() -> impl Into<RefOr<Schema>> {
    ObjectBuilder::new()
        .schema_type(Type::String)
        .min_length(Some(1))
        .max_length(Some(MAX_TOKEN_NAME_CHARS))
        .description(Some(
            "Who uses the token, e.g. the loan origination system.",
        ))
}

/// The project's releases, newest first (any member).
#[utoipa::path(
    get,
    path = "/projects/{project_id}/releases",
    tag = "releases",
    params(("project_id" = Uuid, Path), PageQuery),
    responses(
        (status = 200, body = ReleaseListResponse),
        (status = 404, description = "PROJECT_NOT_FOUND", body = ErrorBody),
    )
)]
pub async fn list(
    State(state): State<AppState>,
    ProjectAccess(access): ProjectAccess,
    Query(query): Query<PageQuery>,
) -> Result<Json<ReleaseListResponse>, ApiError> {
    let page = state
        .releases
        .list(&access, PageRequest::new(query.limit, query.offset))
        .await?;
    let emails = emails(&state, page.items.iter().map(|r| Some(r.created_by))).await?;
    Ok(Json(ReleaseListResponse {
        items: page
            .items
            .into_iter()
            .map(|r| summary(&emails, r))
            .collect(),
        total: page.total,
    }))
}

/// What a release made now would hold, and the version each bump would give (any member).
#[utoipa::path(
    get,
    path = "/projects/{project_id}/releases/preview",
    tag = "releases",
    params(("project_id" = Uuid, Path)),
    responses(
        (status = 200, body = ReleasePreviewResponse),
        (status = 404, description = "PROJECT_NOT_FOUND", body = ErrorBody),
    )
)]
pub async fn preview(
    State(state): State<AppState>,
    ProjectAccess(access): ProjectAccess,
) -> Result<Json<ReleasePreviewResponse>, ApiError> {
    let preview = state.releases.preview(&access).await?;
    let next = |bump| SemVer::next(preview.latest, bump).to_string();
    Ok(Json(ReleasePreviewResponse {
        latest: preview.latest.map(|v| v.to_string()),
        next: NextVersionsResponse {
            major: next(Bump::Major),
            minor: next(Bump::Minor),
            patch: next(Bump::Patch),
        },
        decisions: preview.decisions.into_iter().map(released).collect(),
        unversioned: preview.unversioned,
    }))
}

/// Freezes the latest version of every decision as a new release (editors and owners).
#[utoipa::path(
    post,
    path = "/projects/{project_id}/releases",
    tag = "releases",
    params(("project_id" = Uuid, Path)),
    request_body = CreateReleaseRequest,
    responses(
        (status = 201, body = ReleaseResponse),
        (status = 400, description = "Missing or too long notes (INVALID_REQUEST)", body = ErrorBody),
        (status = 403, description = "Viewers cannot release (FORBIDDEN)", body = ErrorBody),
        (status = 404, description = "PROJECT_NOT_FOUND", body = ErrorBody),
        (status = 409, description = "RELEASE_CONFLICT (someone released at the same moment) or PROJECT_ARCHIVED", body = ErrorBody),
        (status = 422, description = "UNVERSIONED_DECISIONS (details: keys) or NOTHING_TO_RELEASE", body = ErrorBody),
    )
)]
pub async fn create(
    State(state): State<AppState>,
    ProjectAccess(access): ProjectAccess,
    ApiJson(req): ApiJson<CreateReleaseRequest>,
) -> Result<(StatusCode, Json<ReleaseResponse>), ApiError> {
    let bump = match req.bump {
        BumpRequest::Major => Bump::Major,
        BumpRequest::Minor => Bump::Minor,
        BumpRequest::Patch => Bump::Patch,
    };
    let release = state.releases.create(&access, bump, &req.notes).await?;
    Ok((StatusCode::CREATED, Json(full(&state, release).await?)))
}

/// One release with the version of each decision it froze (any member).
#[utoipa::path(
    get,
    path = "/projects/{project_id}/releases/{release_id}",
    tag = "releases",
    params(("project_id" = Uuid, Path), ("release_id" = Uuid, Path)),
    responses(
        (status = 200, body = ReleaseResponse),
        (status = 404, description = "PROJECT_NOT_FOUND or RELEASE_NOT_FOUND", body = ErrorBody),
    )
)]
pub async fn get(
    State(state): State<AppState>,
    ProjectAccess(access): ProjectAccess,
    ReleaseId(id): ReleaseId,
) -> Result<Json<ReleaseResponse>, ApiError> {
    let release = state.releases.get(&access, id).await?;
    Ok(Json(full(&state, release).await?))
}

/// Both environments: what is live, what was last asked for, how many tokens (any member).
#[utoipa::path(
    get,
    path = "/projects/{project_id}/environments",
    tag = "environments",
    params(("project_id" = Uuid, Path)),
    responses(
        (status = 200, body = EnvironmentListResponse),
        (status = 404, description = "PROJECT_NOT_FOUND", body = ErrorBody),
    )
)]
pub async fn environments(
    State(state): State<AppState>,
    ProjectAccess(access): ProjectAccess,
) -> Result<Json<EnvironmentListResponse>, ApiError> {
    let states = state.releases.environments(&access).await?;
    let mut people = Vec::new();
    for s in &states {
        people.extend(s.live.iter().chain(&s.latest).map(|d| Some(d.requested_by)));
    }
    let emails = emails(&state, people).await?;
    Ok(Json(EnvironmentListResponse {
        items: states
            .into_iter()
            .map(|s| environment(&emails, s))
            .collect(),
    }))
}

/// An environment's deployments, newest first (any member).
#[utoipa::path(
    get,
    path = "/projects/{project_id}/environments/{environment}/deployments",
    tag = "environments",
    params(("project_id" = Uuid, Path), ("environment" = EnvironmentName, Path), PageQuery),
    responses(
        (status = 200, body = DeploymentListResponse),
        (status = 404, description = "PROJECT_NOT_FOUND or ENVIRONMENT_NOT_FOUND", body = ErrorBody),
    )
)]
pub async fn deployments(
    State(state): State<AppState>,
    ProjectAccess(access): ProjectAccess,
    EnvironmentKey(env): EnvironmentKey,
    Query(query): Query<PageQuery>,
) -> Result<Json<DeploymentListResponse>, ApiError> {
    let page = state
        .releases
        .deployments(&access, env, PageRequest::new(query.limit, query.offset))
        .await?;
    let emails = emails(&state, page.items.iter().map(|d| Some(d.requested_by))).await?;
    Ok(Json(DeploymentListResponse {
        items: page
            .items
            .into_iter()
            .map(|d| deployment(&emails, d))
            .collect(),
        total: page.total,
    }))
}

/// Deploys a release to staging (editors and owners). The artifact is
/// written after the request, retried on failure; follow it on the
/// environment. Production is published through an approval.
#[utoipa::path(
    post,
    path = "/projects/{project_id}/environments/{environment}/deployments",
    tag = "environments",
    params(("project_id" = Uuid, Path), ("environment" = EnvironmentName, Path)),
    request_body = DeployRequest,
    responses(
        (status = 202, description = "Queued; published shortly", body = DeploymentResponse),
        (status = 403, description = "Viewers cannot deploy (FORBIDDEN)", body = ErrorBody),
        (status = 404, description = "PROJECT_NOT_FOUND, ENVIRONMENT_NOT_FOUND or RELEASE_NOT_FOUND", body = ErrorBody),
        (status = 409, description = "PROJECT_ARCHIVED", body = ErrorBody),
        (status = 422, description = "APPROVAL_REQUIRED (production)", body = ErrorBody),
    )
)]
pub async fn deploy(
    State(state): State<AppState>,
    ProjectAccess(access): ProjectAccess,
    EnvironmentKey(env): EnvironmentKey,
    ApiJson(req): ApiJson<DeployRequest>,
) -> Result<(StatusCode, Json<DeploymentResponse>), ApiError> {
    let queued = state.releases.deploy(&access, env, req.release_id).await?;
    Ok((
        StatusCode::ACCEPTED,
        Json(one_deployment(&state, queued).await?),
    ))
}

/// Tries a deployment that gave up again now (editors and owners).
#[utoipa::path(
    post,
    path = "/projects/{project_id}/environments/{environment}/deployments/{deployment_id}/retry",
    tag = "environments",
    params(("project_id" = Uuid, Path), ("environment" = EnvironmentName, Path), ("deployment_id" = Uuid, Path)),
    responses(
        (status = 202, description = "Queued again", body = DeploymentResponse),
        (status = 403, description = "Viewers cannot retry (FORBIDDEN)", body = ErrorBody),
        (status = 404, description = "PROJECT_NOT_FOUND, ENVIRONMENT_NOT_FOUND or DEPLOYMENT_NOT_FOUND", body = ErrorBody),
        (status = 409, description = "NOT_RETRYABLE (it did not give up) or PROJECT_ARCHIVED", body = ErrorBody),
    )
)]
pub async fn retry(
    State(state): State<AppState>,
    ProjectAccess(access): ProjectAccess,
    EnvironmentKey(env): EnvironmentKey,
    DeploymentId(id): DeploymentId,
) -> Result<(StatusCode, Json<DeploymentResponse>), ApiError> {
    let queued = state.releases.retry(&access, env, id).await?;
    Ok((
        StatusCode::ACCEPTED,
        Json(one_deployment(&state, queued).await?),
    ))
}

/// An environment's Runtime tokens, live ones first; never their value (any member).
#[utoipa::path(
    get,
    path = "/projects/{project_id}/environments/{environment}/tokens",
    tag = "environments",
    params(("project_id" = Uuid, Path), ("environment" = EnvironmentName, Path)),
    responses(
        (status = 200, body = TokenListResponse),
        (status = 404, description = "PROJECT_NOT_FOUND or ENVIRONMENT_NOT_FOUND", body = ErrorBody),
    )
)]
pub async fn tokens(
    State(state): State<AppState>,
    ProjectAccess(access): ProjectAccess,
    EnvironmentKey(env): EnvironmentKey,
) -> Result<Json<TokenListResponse>, ApiError> {
    let tokens = state.releases.tokens(&access, env).await?;
    let emails = emails(
        &state,
        tokens
            .iter()
            .flat_map(|t| [Some(t.created_by), t.revoked_by]),
    )
    .await?;
    Ok(Json(TokenListResponse {
        items: tokens.into_iter().map(|t| token(&emails, t)).collect(),
    }))
}

/// Issues a Runtime token for the environment (owners). The token is in this
/// response only; the environment's release is published again so the
/// Runtime accepts it.
#[utoipa::path(
    post,
    path = "/projects/{project_id}/environments/{environment}/tokens",
    tag = "environments",
    params(("project_id" = Uuid, Path), ("environment" = EnvironmentName, Path)),
    request_body = IssueTokenRequest,
    responses(
        (status = 201, body = IssuedTokenResponse),
        (status = 400, description = "Missing or too long name (INVALID_REQUEST)", body = ErrorBody),
        (status = 403, description = "Only owners manage tokens (FORBIDDEN)", body = ErrorBody),
        (status = 404, description = "PROJECT_NOT_FOUND or ENVIRONMENT_NOT_FOUND", body = ErrorBody),
        (status = 409, description = "PROJECT_ARCHIVED", body = ErrorBody),
    )
)]
pub async fn issue_token(
    State(state): State<AppState>,
    ProjectAccess(access): ProjectAccess,
    EnvironmentKey(env): EnvironmentKey,
    ApiJson(req): ApiJson<IssueTokenRequest>,
) -> Result<(StatusCode, Json<IssuedTokenResponse>), ApiError> {
    let issued = state.releases.issue_token(&access, env, &req.name).await?;
    let emails = emails(&state, [Some(issued.details.created_by)]).await?;
    Ok((
        StatusCode::CREATED,
        Json(IssuedTokenResponse {
            token: issued.token,
            details: token(&emails, issued.details),
        }),
    ))
}

/// Revokes a Runtime token (owners); the environment's release is published again without it.
#[utoipa::path(
    delete,
    path = "/projects/{project_id}/environments/{environment}/tokens/{token_id}",
    tag = "environments",
    params(("project_id" = Uuid, Path), ("environment" = EnvironmentName, Path), ("token_id" = Uuid, Path)),
    responses(
        (status = 204, description = "Revoked"),
        (status = 403, description = "Only owners manage tokens (FORBIDDEN)", body = ErrorBody),
        (status = 404, description = "PROJECT_NOT_FOUND, ENVIRONMENT_NOT_FOUND or TOKEN_NOT_FOUND", body = ErrorBody),
        (status = 409, description = "PROJECT_ARCHIVED", body = ErrorBody),
    )
)]
pub async fn revoke_token(
    State(state): State<AppState>,
    ProjectAccess(access): ProjectAccess,
    EnvironmentKey(env): EnvironmentKey,
    TokenId(id): TokenId,
) -> Result<StatusCode, ApiError> {
    state.releases.revoke_token(&access, env, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

fn person(emails: &HashMap<Uuid, String>, id: Uuid) -> PersonRef {
    PersonRef {
        id,
        email: emails.get(&id).cloned().unwrap_or_default(),
    }
}

fn released(decision: ReleasedDecision) -> ReleasedDecisionResponse {
    ReleasedDecisionResponse {
        decision_id: decision.decision_id,
        key: decision.key,
        version: decision.version,
        tests: decision.tests.into(),
    }
}

pub(crate) fn summary(
    emails: &HashMap<Uuid, String>,
    release: ReleaseSummary,
) -> ReleaseSummaryResponse {
    ReleaseSummaryResponse {
        id: release.id,
        version: release.version.to_string(),
        notes: release.notes,
        created_at: release.created_at,
        created_by: person(emails, release.created_by),
        decisions: release.decisions,
        tests: release.tests.into(),
        live_in: release.live_in.into_iter().map(Into::into).collect(),
    }
}

async fn full(state: &AppState, release: Release) -> Result<ReleaseResponse, ApiError> {
    let emails = emails(state, [Some(release.summary.created_by)]).await?;
    Ok(ReleaseResponse {
        release: summary(&emails, release.summary),
        decisions: release.decisions.into_iter().map(released).collect(),
    })
}

pub(crate) fn deployment(emails: &HashMap<Uuid, String>, d: Deployment) -> DeploymentResponse {
    DeploymentResponse {
        id: d.id,
        environment: d.environment.into(),
        release_id: d.release_id,
        release_version: d.release_version.to_string(),
        reason: match d.reason {
            DeploymentReason::Deploy => DeploymentReasonResponse::Deploy,
            DeploymentReason::Tokens => DeploymentReasonResponse::Tokens,
            DeploymentReason::Rollback => DeploymentReasonResponse::Rollback,
        },
        rollback_reason: d.rollback_reason,
        requested_at: d.requested_at,
        requested_by: person(emails, d.requested_by),
        status: match d.status {
            DeploymentStatus::Pending => DeploymentStatusResponse::Pending,
            DeploymentStatus::Retrying => DeploymentStatusResponse::Retrying,
            DeploymentStatus::Failed => DeploymentStatusResponse::Failed,
            DeploymentStatus::Published => DeploymentStatusResponse::Published,
            DeploymentStatus::Superseded => DeploymentStatusResponse::Superseded,
        },
        attempts: d.attempts,
        last_error: d.last_error,
        next_attempt_at: d.next_attempt_at,
        published_at: d.published_at,
    }
}

pub(crate) async fn one_deployment(
    state: &AppState,
    d: Deployment,
) -> Result<DeploymentResponse, ApiError> {
    let emails = emails(state, [Some(d.requested_by)]).await?;
    Ok(deployment(&emails, d))
}

fn environment(emails: &HashMap<Uuid, String>, s: EnvironmentState) -> EnvironmentResponse {
    EnvironmentResponse {
        environment: s.environment.into(),
        live: s.live.map(|d| deployment(emails, d)),
        latest: s.latest.map(|d| deployment(emails, d)),
        tokens: s.tokens,
    }
}

fn token(emails: &HashMap<Uuid, String>, t: RuntimeToken) -> TokenResponse {
    TokenResponse {
        id: t.id,
        name: t.name,
        hint: t.hint,
        created_at: t.created_at,
        created_by: person(emails, t.created_by),
        revoked_at: t.revoked_at,
        revoked_by: t.revoked_by.map(|id| person(emails, id)),
    }
}
