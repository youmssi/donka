//! Rules sync (DNK-20): CI pipelines resolve a project target and download
//! its artifact with a project's CI token; members manage those tokens.

use crate::error::{ApiError, ErrorBody};
use crate::extract::{ApiJson, ProjectAccess, TokenId};
use crate::routes::people::{emails, known_person, person, PersonRef};
use crate::routes::releases::{token_name_schema, EnvironmentName};
use crate::AppState;
use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use chrono::{DateTime, Utc};
use donka_release::{
    ArtifactPath, CiAccess, CiToken, Resolved, SyncOutcome, SyncProject, SyncRequest,
    MAX_SYNC_DEPLOYMENTS,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use utoipa::ToSchema;
use uuid::Uuid;

// ----- Sync (CI token) -----------------------------------------------------

#[derive(Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SyncBody {
    /// At most 50; each is answered on its own.
    pub deployments: Vec<SyncDeployment>,
}

#[derive(Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SyncDeployment {
    /// The project's key or id.
    pub project: String,
    /// `main` (the newest release), `commit:<release id>`, `release:<version>`
    /// or `env:<staging|production>`. Default `main`.
    #[serde(default)]
    pub target: Option<String>,
    /// Echoed back, to tell answers apart.
    #[serde(default)]
    pub alias: Option<String>,
    /// What the pipeline already holds; a match answers `no_change`.
    #[serde(default)]
    pub current: Option<SyncCurrent>,
}

#[derive(Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SyncCurrent {
    pub commit_id: Option<String>,
    pub release_id: Option<String>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SyncResponse {
    /// Always `null`: Studio does not ask pipelines to poll.
    pub next_poll_at: Option<DateTime<Utc>>,
    pub deployments: Vec<SyncResult>,
}

/// What became of one deployment.
#[derive(Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum SyncAction {
    /// Download `artifact`.
    Load,
    /// The pipeline already holds it.
    NoChange,
    /// No release yet, or nothing live in the environment.
    NoRelease,
    /// The token cannot reach this project (or it does not exist).
    NoAccess,
    /// The target could not be read or resolved; see `code`.
    Error,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SyncResult {
    pub project: Option<SyncProjectRef>,
    pub target: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alias: Option<String>,
    pub action: SyncAction,
    /// The snapshot held after a load: the release, or for `env:` the deployment.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commit: Option<SyncCommit>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub release: Option<SyncRelease>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub environment: Option<SyncEnvironment>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artifact: Option<SyncArtifact>,
    /// With `error`: `INVALID_TARGET`, `UNSUPPORTED_TARGET` or `RELEASE_NOT_FOUND`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<&'static str>,
}

#[derive(Serialize, ToSchema)]
pub struct SyncProjectRef {
    pub id: Uuid,
    pub key: String,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SyncCommit {
    /// Send it back as `current.commitId` to be told `no_change`.
    pub id: Uuid,
    /// Always `null`: Donka has no branches.
    pub branch_id: Option<String>,
    pub branch_name: Option<String>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SyncRelease {
    pub id: Uuid,
    /// The release notes.
    pub name: String,
    pub version: String,
    pub semantic_version: String,
}

#[derive(Serialize, ToSchema)]
pub struct SyncEnvironment {
    pub id: String,
    pub key: EnvironmentName,
    pub name: String,
}

#[derive(Serialize, ToSchema)]
pub struct SyncArtifact {
    /// Relative to the API base path; download it with the same token.
    pub url: String,
    /// Lowercase hex SHA-256 of the download.
    pub sha256: String,
}

fn bearer(headers: &HeaderMap) -> Result<&str, ApiError> {
    headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .map(str::trim)
        .ok_or(ApiError::InvalidCiToken)
}

async fn ci_access(state: &AppState, headers: &HeaderMap) -> Result<CiAccess, ApiError> {
    Ok(state.releases.authenticate_ci(bearer(headers)?).await?)
}

/// Resolves the targets a pipeline asks for (`Authorization: Bearer <CI token>`).
/// A token reaches only its own project; any other answers `no_access`.
#[utoipa::path(
    post,
    path = "/rules-sync",
    tag = "rules-sync",
    request_body = SyncBody,
    responses(
        (status = 200, body = SyncResponse),
        (status = 400, description = "Not a request, or more than 50 deployments (INVALID_REQUEST)", body = ErrorBody),
        (status = 401, description = "Missing, unknown or revoked CI token (INVALID_TOKEN)", body = ErrorBody),
    )
)]
pub async fn sync(
    State(state): State<AppState>,
    headers: HeaderMap,
    ApiJson(body): ApiJson<SyncBody>,
) -> Result<Json<SyncResponse>, ApiError> {
    let ci = ci_access(&state, &headers).await?;
    if body.deployments.len() > MAX_SYNC_DEPLOYMENTS {
        return Err(ApiError::InvalidRequest(format!(
            "ask about at most {MAX_SYNC_DEPLOYMENTS} deployments at once"
        )));
    }
    let mut results = Vec::with_capacity(body.deployments.len());
    for deployment in body.deployments {
        let target = deployment.target.unwrap_or_else(|| "main".into());
        let current = deployment
            .current
            .map(|c| c.commit_id.into_iter().chain(c.release_id).collect())
            .unwrap_or_default();
        let outcome = state
            .releases
            .sync(
                ci,
                &SyncRequest {
                    project: deployment.project,
                    target: target.clone(),
                    current,
                },
            )
            .await?;
        results.push(result(outcome, target, deployment.alias));
    }
    Ok(Json(SyncResponse {
        next_poll_at: None,
        deployments: results,
    }))
}

fn result(outcome: SyncOutcome, target: String, alias: Option<String>) -> SyncResult {
    let empty = |project: Option<SyncProject>, action: SyncAction, code| SyncResult {
        project: project.map(project_ref),
        target: target.clone(),
        alias: alias.clone(),
        action,
        commit: None,
        release: None,
        environment: None,
        artifact: None,
        code,
    };
    match outcome {
        SyncOutcome::NoAccess => empty(None, SyncAction::NoAccess, None),
        SyncOutcome::NoRelease { project } => empty(Some(project), SyncAction::NoRelease, None),
        SyncOutcome::Error { project, code } => empty(project, SyncAction::Error, Some(code)),
        SyncOutcome::NoChange(resolved) => {
            resolved_result(resolved, SyncAction::NoChange, None, target, alias)
        }
        SyncOutcome::Load {
            resolved,
            path,
            sha256,
        } => resolved_result(
            resolved,
            SyncAction::Load,
            Some(SyncArtifact {
                url: path.url(),
                sha256,
            }),
            target,
            alias,
        ),
    }
}

fn resolved_result(
    resolved: Resolved,
    action: SyncAction,
    artifact: Option<SyncArtifact>,
    target: String,
    alias: Option<String>,
) -> SyncResult {
    let version = resolved.version.to_string();
    SyncResult {
        commit: Some(SyncCommit {
            id: resolved.snapshot_id(),
            branch_id: None,
            branch_name: None,
        }),
        environment: resolved.deployment.map(|(environment, _)| SyncEnvironment {
            id: format!("{}/{}", resolved.project.id, environment.as_str()),
            key: environment.into(),
            name: match environment {
                donka_release::Environment::Staging => "Staging".into(),
                donka_release::Environment::Production => "Production".into(),
            },
        }),
        release: Some(SyncRelease {
            id: resolved.release_id,
            name: resolved.notes,
            version: version.clone(),
            semantic_version: version,
        }),
        project: Some(project_ref(resolved.project)),
        target,
        alias,
        action,
        artifact,
        code: None,
    }
}

fn project_ref(project: SyncProject) -> SyncProjectRef {
    SyncProjectRef {
        id: project.id,
        key: project.key,
    }
}

/// A release's artifact (no environment, no token), with the CI token of its project.
#[utoipa::path(
    get,
    path = "/rules-sync/artifacts/{project_id}/releases/{release_id}",
    tag = "rules-sync",
    params(("project_id" = Uuid, Path), ("release_id" = Uuid, Path)),
    responses(
        (status = 200, description = "The artifact (a zip)", content_type = "application/zip"),
        (status = 401, description = "INVALID_TOKEN", body = ErrorBody),
        (status = 404, description = "Not this token's project, or no such release (RELEASE_NOT_FOUND)", body = ErrorBody),
    )
)]
pub async fn release_artifact(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((project, release)): Path<(String, String)>,
) -> Result<Response, ApiError> {
    let ci = ci_access(&state, &headers).await?;
    let path = ArtifactPath::Release {
        project: uuid(&project)?,
        release: uuid(&release)?,
    };
    download(&state, ci, path).await
}

/// An environment's artifact as its Runtime reads it, tokens included, with
/// the CI token of its project.
#[utoipa::path(
    get,
    path = "/rules-sync/artifacts/{project_id}/deployments/{deployment_id}",
    tag = "rules-sync",
    params(("project_id" = Uuid, Path), ("deployment_id" = Uuid, Path)),
    responses(
        (status = 200, description = "The artifact (a zip)", content_type = "application/zip"),
        (status = 401, description = "INVALID_TOKEN", body = ErrorBody),
        (status = 404, description = "Not this token's project, or no such published deployment (RELEASE_NOT_FOUND)", body = ErrorBody),
    )
)]
pub async fn deployment_artifact(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((project, deployment)): Path<(String, String)>,
) -> Result<Response, ApiError> {
    let ci = ci_access(&state, &headers).await?;
    let path = ArtifactPath::Deployment {
        project: uuid(&project)?,
        deployment: uuid(&deployment)?,
    };
    download(&state, ci, path).await
}

fn uuid(text: &str) -> Result<Uuid, ApiError> {
    Uuid::parse_str(text).map_err(|_| ApiError::ReleaseNotFound)
}

async fn download(
    state: &AppState,
    ci: CiAccess,
    path: ArtifactPath,
) -> Result<Response, ApiError> {
    let bytes = state.releases.ci_artifact(ci, path).await?;
    Ok((
        [
            (header::CONTENT_TYPE, "application/zip"),
            (header::CACHE_CONTROL, "no-store"),
        ],
        Body::from(bytes),
    )
        .into_response())
}

// ----- CI tokens (members) ---------------------------------------------------

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CiTokenResponse {
    pub id: Uuid,
    pub name: String,
    /// The token's last characters, to tell tokens apart.
    pub hint: String,
    pub created_at: DateTime<Utc>,
    pub created_by: PersonRef,
    pub revoked_at: Option<DateTime<Utc>>,
    pub revoked_by: Option<PersonRef>,
    /// When a pipeline last used it.
    pub last_used_at: Option<DateTime<Utc>>,
}

#[derive(Serialize, ToSchema)]
pub struct CiTokenListResponse {
    pub items: Vec<CiTokenResponse>,
}

#[derive(Serialize, ToSchema)]
pub struct IssuedCiTokenResponse {
    /// The token. Shown this once: Studio keeps only its hash.
    pub token: String,
    #[serde(flatten)]
    pub details: CiTokenResponse,
}

#[derive(Deserialize, ToSchema)]
pub struct IssueCiTokenRequest {
    #[schema(schema_with = token_name_schema)]
    pub name: String,
}

/// The project's CI tokens, live ones first; never their value (any member).
#[utoipa::path(
    get,
    path = "/projects/{project_id}/ci-tokens",
    tag = "rules-sync",
    params(("project_id" = Uuid, Path)),
    responses(
        (status = 200, body = CiTokenListResponse),
        (status = 404, description = "PROJECT_NOT_FOUND", body = ErrorBody),
    )
)]
pub async fn ci_tokens(
    State(state): State<AppState>,
    ProjectAccess(access): ProjectAccess,
) -> Result<Json<CiTokenListResponse>, ApiError> {
    let tokens = state.releases.ci_tokens(&access).await?;
    let emails = emails(
        &state,
        tokens
            .iter()
            .flat_map(|t| [Some(t.created_by), t.revoked_by]),
    )
    .await?;
    Ok(Json(CiTokenListResponse {
        items: tokens.into_iter().map(|t| ci_token(&emails, t)).collect(),
    }))
}

/// Issues a read-only CI token for the project (owners). The token is in this response only.
#[utoipa::path(
    post,
    path = "/projects/{project_id}/ci-tokens",
    tag = "rules-sync",
    params(("project_id" = Uuid, Path)),
    request_body = IssueCiTokenRequest,
    responses(
        (status = 201, body = IssuedCiTokenResponse),
        (status = 400, description = "INVALID_REQUEST", body = ErrorBody),
        (status = 403, description = "Only owners issue tokens (FORBIDDEN)", body = ErrorBody),
        (status = 404, description = "PROJECT_NOT_FOUND", body = ErrorBody),
        (status = 409, description = "PROJECT_ARCHIVED", body = ErrorBody),
    )
)]
pub async fn issue_ci_token(
    State(state): State<AppState>,
    ProjectAccess(access): ProjectAccess,
    ApiJson(req): ApiJson<IssueCiTokenRequest>,
) -> Result<(StatusCode, Json<IssuedCiTokenResponse>), ApiError> {
    let issued = state.releases.issue_ci_token(&access, &req.name).await?;
    let emails = emails(&state, [Some(issued.details.created_by)]).await?;
    Ok((
        StatusCode::CREATED,
        Json(IssuedCiTokenResponse {
            token: issued.token,
            details: ci_token(&emails, issued.details),
        }),
    ))
}

/// Revokes a CI token (owners); pipelines using it are refused from then on.
#[utoipa::path(
    delete,
    path = "/projects/{project_id}/ci-tokens/{token_id}",
    tag = "rules-sync",
    params(("project_id" = Uuid, Path), ("token_id" = Uuid, Path)),
    responses(
        (status = 204, description = "Revoked"),
        (status = 403, description = "FORBIDDEN", body = ErrorBody),
        (status = 404, description = "PROJECT_NOT_FOUND or TOKEN_NOT_FOUND", body = ErrorBody),
    )
)]
pub async fn revoke_ci_token(
    State(state): State<AppState>,
    ProjectAccess(access): ProjectAccess,
    TokenId(id): TokenId,
) -> Result<StatusCode, ApiError> {
    state.releases.revoke_ci_token(&access, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

fn ci_token(emails: &HashMap<Uuid, String>, token: CiToken) -> CiTokenResponse {
    CiTokenResponse {
        id: token.id,
        name: token.name,
        hint: token.hint,
        created_at: token.created_at,
        created_by: known_person(emails, token.created_by),
        revoked_at: token.revoked_at,
        revoked_by: person(emails, token.revoked_by),
        last_used_at: token.last_used_at,
    }
}
