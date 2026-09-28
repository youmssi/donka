use crate::auth::CurrentUser;
use crate::error::{ApiError, ErrorBody};
use crate::extract::ApiJson;
use crate::routes::auth::UserResponse;
use crate::AppState;
use axum::extract::State;
use axum::http::StatusCode;
use axum::{Extension, Json};
use donka_identity::Locale;
use serde::Deserialize;
use utoipa::ToSchema;

#[derive(Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct InvitationRequest {
    pub email: String,
    /// Language of the invitation email and of the account. Defaults to the inviter's.
    pub locale: Option<Locale>,
    #[serde(default)]
    pub is_admin: bool,
}

/// Invites a person by email (administrators only). Inviting someone who has not
/// accepted yet sends a new link and revokes the previous one.
#[utoipa::path(
    post,
    path = "/users/invitations",
    tag = "identity",
    request_body = InvitationRequest,
    responses(
        (status = 201, description = "Account created; the invitation email is on its way", body = UserResponse),
        (status = 400, description = "Invalid email (INVALID_REQUEST)", body = ErrorBody),
        (status = 401, description = "Not signed in (UNAUTHENTICATED)", body = ErrorBody),
        (status = 403, description = "Not an administrator (FORBIDDEN)", body = ErrorBody),
        (status = 409, description = "The person already has an account (EMAIL_TAKEN)", body = ErrorBody),
    )
)]
pub async fn invite(
    State(state): State<AppState>,
    Extension(current): Extension<CurrentUser>,
    ApiJson(req): ApiJson<InvitationRequest>,
) -> Result<(StatusCode, Json<UserResponse>), ApiError> {
    let locale = req.locale.unwrap_or(current.user.locale);
    let user = state
        .identity
        .invite(&current.user, &req.email, locale, req.is_admin)
        .await?;
    Ok((StatusCode::CREATED, Json(user.into())))
}
