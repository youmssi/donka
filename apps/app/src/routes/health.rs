use crate::error::{ApiError, ErrorBody};
use crate::AppState;
use axum::extract::State;
use axum::Json;
use serde::Serialize;
use std::time::Duration;

/// How long readiness waits for the database before reporting it unavailable.
const READY_TIMEOUT: Duration = Duration::from_secs(2);

/// Liveness: answers as long as the process serves requests. Does not touch the database.
#[utoipa::path(
    get,
    path = "/health",
    tag = "operations",
    responses((status = 200, description = "The service is running", body = String, example = "ok"))
)]
pub async fn health() -> &'static str {
    "ok"
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct VersionResponse {
    /// Release version baked in at build time, or `unknown` for a local build.
    #[schema(example = "1.4.0")]
    pub version: &'static str,
}

/// The version of Studio that is running. Needs no session, like health.
#[utoipa::path(
    get,
    path = "/version",
    tag = "operations",
    responses((status = 200, body = VersionResponse))
)]
pub async fn version() -> Json<VersionResponse> {
    Json(VersionResponse {
        version: crate::VERSION,
    })
}

/// Readiness: the service can handle requests that need the database.
#[utoipa::path(
    get,
    path = "/ready",
    tag = "operations",
    responses(
        (status = 200, description = "The database answers", body = String, example = "ready"),
        (status = 503, description = "The database is not reachable (DATABASE_UNAVAILABLE)", body = ErrorBody),
    )
)]
pub async fn ready(State(state): State<AppState>) -> Result<&'static str, ApiError> {
    if donka_db::is_ready(&state.db, READY_TIMEOUT).await {
        Ok("ready")
    } else {
        Err(ApiError::DatabaseUnavailable)
    }
}
