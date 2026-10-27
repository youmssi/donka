//! A person's own second sign-in step, and an administrator's reset (DNK-59).

use crate::auth::CurrentUser;
use crate::error::{ApiError, ErrorBody};
use crate::extract::ApiJson;
use crate::routes::auth::{second_step_required, CodeRequest, EnrolmentResponse};
use crate::AppState;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::{Extension, Json};
use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SecondStepStatusResponse {
    pub enabled: bool,
    /// Policy requires a second step of this person: it cannot be turned off.
    pub required: bool,
    pub recovery_codes_left: i64,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RecoveryCodesResponse {
    /// Shown once; each works once instead of a code.
    pub recovery_codes: Vec<String>,
}

/// Whether the signed-in person has a second step, and whether policy requires one.
#[utoipa::path(
    get,
    path = "/account/second-step",
    tag = "identity",
    responses(
        (status = 200, body = SecondStepStatusResponse),
        (status = 401, description = "Not signed in (UNAUTHENTICATED)", body = ErrorBody),
    )
)]
pub async fn status(
    State(state): State<AppState>,
    Extension(current): Extension<CurrentUser>,
) -> Result<Json<SecondStepStatusResponse>, ApiError> {
    let status = state.identity.second_step_status(current.user.id).await?;
    Ok(Json(SecondStepStatusResponse {
        enabled: status.enabled,
        required: second_step_required(&state, &current.user).await?,
        recovery_codes_left: status.recovery_codes_left,
    }))
}

/// Starts setting up a second step. Starting again replaces the key not yet confirmed.
#[utoipa::path(
    post,
    path = "/account/second-step/enrolment",
    tag = "identity",
    responses(
        (status = 200, body = EnrolmentResponse),
        (status = 401, description = "Not signed in (UNAUTHENTICATED)", body = ErrorBody),
        (status = 409, description = "Already on (SECOND_STEP_ALREADY_ON)", body = ErrorBody),
    )
)]
pub async fn begin_enrolment(
    State(state): State<AppState>,
    Extension(current): Extension<CurrentUser>,
) -> Result<Json<EnrolmentResponse>, ApiError> {
    let enrolment = state.identity.begin_enrolment(&current.user).await?;
    Ok(Json(EnrolmentResponse {
        key: enrolment.key,
        otpauth_url: enrolment.otpauth_url,
    }))
}

/// Turns the second step on with a first valid code; returns the recovery codes.
#[utoipa::path(
    post,
    path = "/account/second-step/enrolment/confirm",
    tag = "identity",
    request_body = CodeRequest,
    responses(
        (status = 200, body = RecoveryCodesResponse),
        (status = 400, description = "Wrong code (INVALID_CODE)", body = ErrorBody),
        (status = 401, description = "Not signed in (UNAUTHENTICATED)", body = ErrorBody),
        (status = 409, description = "Already on (SECOND_STEP_ALREADY_ON) or no enrolment started (SECOND_STEP_OFF)", body = ErrorBody),
    )
)]
pub async fn confirm_enrolment(
    State(state): State<AppState>,
    Extension(current): Extension<CurrentUser>,
    ApiJson(req): ApiJson<CodeRequest>,
) -> Result<Json<RecoveryCodesResponse>, ApiError> {
    let codes = state
        .identity
        .confirm_enrolment(&current.user, &req.code)
        .await?;
    Ok(Json(RecoveryCodesResponse {
        recovery_codes: codes.0,
    }))
}

/// Replaces the recovery codes; needs a current authenticator code.
#[utoipa::path(
    post,
    path = "/account/second-step/recovery-codes",
    tag = "identity",
    request_body = CodeRequest,
    responses(
        (status = 200, body = RecoveryCodesResponse),
        (status = 400, description = "Wrong code (INVALID_CODE)", body = ErrorBody),
        (status = 401, description = "Not signed in (UNAUTHENTICATED)", body = ErrorBody),
        (status = 409, description = "Two-step sign-in is off (SECOND_STEP_OFF)", body = ErrorBody),
    )
)]
pub async fn regenerate_recovery_codes(
    State(state): State<AppState>,
    Extension(current): Extension<CurrentUser>,
    ApiJson(req): ApiJson<CodeRequest>,
) -> Result<Json<RecoveryCodesResponse>, ApiError> {
    let codes = state
        .identity
        .regenerate_recovery_codes(&current.user, &req.code)
        .await?;
    Ok(Json(RecoveryCodesResponse {
        recovery_codes: codes.0,
    }))
}

/// Turns the person's second step off with an authenticator or recovery code.
#[utoipa::path(
    post,
    path = "/account/second-step/disable",
    tag = "identity",
    request_body = CodeRequest,
    responses(
        (status = 204, description = "Turned off"),
        (status = 400, description = "Wrong code (INVALID_CODE)", body = ErrorBody),
        (status = 401, description = "Not signed in (UNAUTHENTICATED)", body = ErrorBody),
        (status = 409, description = "Already off (SECOND_STEP_OFF) or required by policy (SECOND_STEP_REQUIRED)", body = ErrorBody),
    )
)]
pub async fn disable(
    State(state): State<AppState>,
    Extension(current): Extension<CurrentUser>,
    ApiJson(req): ApiJson<CodeRequest>,
) -> Result<StatusCode, ApiError> {
    let required = second_step_required(&state, &current.user).await?;
    state
        .identity
        .remove_second_step(&current.user, &req.code, required)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

/// An administrator removes another person's second step (lost device) and
/// signs them out. They are told by email.
#[utoipa::path(
    post,
    path = "/users/{user_id}/second-step/reset",
    tag = "identity",
    params(("user_id" = Uuid, Path, description = "The person whose second step to reset")),
    responses(
        (status = 204, description = "Reset; the person is signed out everywhere"),
        (status = 401, description = "Not signed in (UNAUTHENTICATED)", body = ErrorBody),
        (status = 403, description = "Not an administrator, or one's own account (FORBIDDEN)", body = ErrorBody),
        (status = 409, description = "The person has no second step (SECOND_STEP_OFF)", body = ErrorBody),
    )
)]
pub async fn reset(
    State(state): State<AppState>,
    Extension(current): Extension<CurrentUser>,
    Path(user_id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    state
        .identity
        .reset_second_step(&current.user, user_id)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
