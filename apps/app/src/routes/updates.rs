//! Whether a newer Donka release is published (DNK-64), for administrators.

use crate::auth::CurrentUser;
use crate::error::{ApiError, ErrorBody};
use crate::AppState;
use axum::extract::State;
use axum::{Extension, Json};
use chrono::{DateTime, Utc};
use serde::Serialize;
use utoipa::ToSchema;

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdatesResponse {
    /// This Studio's version.
    pub current: String,
    /// Whether Studio checks for new releases; off with `DONKA_UPDATE_CHECK=false`.
    pub checking: bool,
    /// The latest published release, when it is newer than this Studio.
    pub newer: Option<PublishedRelease>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PublishedRelease {
    #[schema(example = "1.6.0")]
    pub version: String,
    /// Its release notes.
    pub url: String,
    pub published_at: Option<DateTime<Utc>>,
}

/// Whether a newer Donka release is published (administrators only).
#[utoipa::path(
    get,
    path = "/updates",
    tag = "operations",
    responses(
        (status = 200, body = UpdatesResponse),
        (status = 401, description = "Not signed in (UNAUTHENTICATED)", body = ErrorBody),
        (status = 403, description = "Not an administrator (FORBIDDEN)", body = ErrorBody),
    )
)]
pub async fn get(
    State(state): State<AppState>,
    Extension(current): Extension<CurrentUser>,
) -> Result<Json<UpdatesResponse>, ApiError> {
    if !current.user.is_admin {
        return Err(ApiError::Forbidden);
    }
    let updates = &state.updates;
    Ok(Json(UpdatesResponse {
        current: updates.current().to_owned(),
        checking: updates.enabled(),
        newer: updates.newer().map(|release| PublishedRelease {
            version: release.version,
            url: release.url,
            published_at: release.published_at,
        }),
    }))
}
