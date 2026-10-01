use crate::auth::CurrentUser;
use crate::error::ApiError;
use crate::AppState;
use axum::extract::{FromRequest, FromRequestParts, Path};
use axum::http::request::Parts;
use donka_project::Access;
use std::collections::HashMap;
use uuid::Uuid;

/// `axum::Json` whose rejections use the API error shape (`400 INVALID_REQUEST`).
#[derive(FromRequest)]
#[from_request(via(axum::Json), rejection(ApiError))]
pub struct ApiJson<T>(pub T);

/// The signed-in user's access to the project in the `{project_id}` path segment.
///
/// Every project route takes this extractor, so membership is checked in one
/// place: a non-member (or a malformed id) gets `404 PROJECT_NOT_FOUND`. Role
/// checks happen in the project module, which is the only code that can build
/// an [`Access`].
pub struct ProjectAccess(pub Access);

impl FromRequestParts<AppState> for ProjectAccess {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, ApiError> {
        let user = parts
            .extensions
            .get::<CurrentUser>()
            .ok_or(ApiError::Unauthenticated)?
            .user
            .id;
        let project_id = path_id(parts, state, "project_id")
            .await
            .ok_or(ApiError::ProjectNotFound)?;
        Ok(Self(state.projects.access(user, project_id).await?))
    }
}

/// The member in the `{user_id}` path segment; a malformed id is `404 MEMBER_NOT_FOUND`.
pub struct MemberId(pub Uuid);

impl FromRequestParts<AppState> for MemberId {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, ApiError> {
        path_id(parts, state, "user_id")
            .await
            .map(Self)
            .ok_or(ApiError::MemberNotFound)
    }
}

/// The decision in the `{decision_id}` path segment; a malformed id is `404 DECISION_NOT_FOUND`.
pub struct DecisionId(pub Uuid);

impl FromRequestParts<AppState> for DecisionId {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, ApiError> {
        path_id(parts, state, "decision_id")
            .await
            .map(Self)
            .ok_or(ApiError::DecisionMissing)
    }
}

/// The scenario in the `{scenario_id}` path segment; a malformed id is `404 SCENARIO_NOT_FOUND`.
pub struct ScenarioId(pub Uuid);

impl FromRequestParts<AppState> for ScenarioId {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, ApiError> {
        path_id(parts, state, "scenario_id")
            .await
            .map(Self)
            .ok_or(ApiError::ScenarioNotFound)
    }
}

/// The version in the `{number}` path segment; anything but a number is `404 VERSION_NOT_FOUND`.
pub struct VersionNumber(pub i32);

impl FromRequestParts<AppState> for VersionNumber {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, ApiError> {
        let Path(params) = Path::<HashMap<String, String>>::from_request_parts(parts, state)
            .await
            .map_err(|_| ApiError::VersionNotFound)?;
        params
            .get("number")
            .and_then(|number| number.parse().ok())
            .map(Self)
            .ok_or(ApiError::VersionNotFound)
    }
}

async fn path_id(parts: &mut Parts, state: &AppState, name: &str) -> Option<Uuid> {
    let Path(params) = Path::<HashMap<String, String>>::from_request_parts(parts, state)
        .await
        .ok()?;
    params.get(name).and_then(|id| Uuid::parse_str(id).ok())
}
