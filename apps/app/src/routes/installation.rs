//! The installation itself (DNK-74): whether its first-run assistant is done, and the name of
//! the organisation it serves.

use crate::auth::CurrentUser;
use crate::error::{ApiError, ErrorBody};
use crate::extract::ApiJson;
use crate::AppState;
use axum::extract::State;
use axum::{Extension, Json};
use donka_installation::{Settings, MAX_ORGANISATION_CHARS};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct InstallationResponse {
    /// The first-run assistant is still to be finished by an administrator.
    pub first_run: bool,
    /// The organisation the installation serves; absent until an administrator names it.
    pub organisation: Option<String>,
}

impl From<Settings> for InstallationResponse {
    fn from(settings: Settings) -> Self {
        Self {
            first_run: settings.first_run(),
            organisation: settings.organisation,
        }
    }
}

#[derive(Deserialize, ToSchema)]
pub struct OrganisationRequest {
    /// One line, trimmed.
    #[schema(schema_with = organisation_schema)]
    pub organisation: String,
}

fn organisation_schema() -> utoipa::openapi::Object {
    utoipa::openapi::ObjectBuilder::new()
        .schema_type(utoipa::openapi::schema::Type::String)
        .min_length(Some(1))
        .max_length(Some(MAX_ORGANISATION_CHARS))
        .build()
}

/// Whether the installation is set up, and its organisation (anyone, signed in or not: the
/// sign-in page names the organisation).
#[utoipa::path(
    get,
    path = "/installation",
    tag = "installation",
    responses((status = 200, body = InstallationResponse))
)]
pub async fn get(State(state): State<AppState>) -> Result<Json<InstallationResponse>, ApiError> {
    Ok(Json(state.installation.settings().await?.into()))
}

/// Finishes the first-run assistant with the organisation's name (an administrator with a
/// second sign-in step). The packs were imported before, one by one.
#[utoipa::path(
    post,
    path = "/installation/first-run",
    tag = "installation",
    request_body = OrganisationRequest,
    responses(
        (status = 200, body = InstallationResponse),
        (status = 400, description = "Not a name of 1 to 100 characters on one line (INVALID_REQUEST)", body = ErrorBody),
        (status = 401, description = "Not signed in (UNAUTHENTICATED)", body = ErrorBody),
        (status = 403, description = "Not an administrator (FORBIDDEN)", body = ErrorBody),
        (status = 409, description = "Already set up (ALREADY_SET_UP); no second sign-in step yet (SECOND_STEP_REQUIRED)", body = ErrorBody),
    )
)]
pub async fn finish_first_run(
    State(state): State<AppState>,
    Extension(current): Extension<CurrentUser>,
    ApiJson(req): ApiJson<OrganisationRequest>,
) -> Result<Json<InstallationResponse>, ApiError> {
    if !current.user.is_admin {
        return Err(ApiError::Forbidden);
    }
    if !state
        .identity
        .second_step_status(current.user.id)
        .await?
        .enabled
    {
        return Err(ApiError::SetUpNeedsSecondStep);
    }
    let settings = state
        .installation
        .finish_first_run(current.user.id, &req.organisation)
        .await?;
    Ok(Json(settings.into()))
}

/// Renames the organisation (administrators), once the installation is set up.
#[utoipa::path(
    put,
    path = "/installation/organisation",
    tag = "installation",
    request_body = OrganisationRequest,
    responses(
        (status = 200, body = InstallationResponse),
        (status = 400, description = "Not a name of 1 to 100 characters on one line (INVALID_REQUEST)", body = ErrorBody),
        (status = 401, description = "Not signed in (UNAUTHENTICATED)", body = ErrorBody),
        (status = 403, description = "Not an administrator (FORBIDDEN)", body = ErrorBody),
        (status = 409, description = "The first-run assistant is not finished (NOT_SET_UP)", body = ErrorBody),
    )
)]
pub async fn rename(
    State(state): State<AppState>,
    Extension(current): Extension<CurrentUser>,
    ApiJson(req): ApiJson<OrganisationRequest>,
) -> Result<Json<InstallationResponse>, ApiError> {
    if !current.user.is_admin {
        return Err(ApiError::Forbidden);
    }
    let settings = state
        .installation
        .rename(current.user.id, &req.organisation)
        .await?;
    Ok(Json(settings.into()))
}
