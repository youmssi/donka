use crate::auth::CurrentUser;
use crate::error::{ApiError, ErrorBody};
use crate::extract::ApiJson;
use crate::AppState;
use axum::extract::State;
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use donka_identity::{Locale, User};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Deserialize, ToSchema)]
pub struct SignInRequest {
    pub email: String,
    pub password: String,
}

#[derive(Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PasswordSetupRequest {
    /// The token from the password-setup link.
    pub token: String,
    pub password: String,
}

#[derive(Deserialize, ToSchema)]
pub struct PasswordResetRequest {
    pub email: String,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UserResponse {
    pub id: Uuid,
    pub email: String,
    pub is_admin: bool,
    pub locale: Locale,
}

impl From<User> for UserResponse {
    fn from(user: User) -> Self {
        Self {
            id: user.id,
            email: user.email,
            is_admin: user.is_admin,
            locale: user.locale,
        }
    }
}

/// Signs in and sets the httpOnly session cookie.
#[utoipa::path(
    post,
    path = "/auth/sign-in",
    tag = "identity",
    request_body = SignInRequest,
    responses(
        (status = 200, description = "Signed in; the session cookie is set", body = UserResponse),
        (status = 401, description = "Email or password is incorrect, or the account is locked (INVALID_CREDENTIALS)", body = ErrorBody),
        (status = 403, description = "Missing CSRF header (CSRF_REQUIRED)", body = ErrorBody),
    )
)]
pub async fn sign_in(
    State(state): State<AppState>,
    ApiJson(req): ApiJson<SignInRequest>,
) -> Result<Response, ApiError> {
    let (token, user) = state.identity.sign_in(&req.email, &req.password).await?;
    Ok((
        [(header::SET_COOKIE, state.cookies.session(token.expose()))],
        Json(UserResponse::from(user)),
    )
        .into_response())
}

/// Ends the session on the server and clears the cookie.
#[utoipa::path(
    post,
    path = "/auth/sign-out",
    tag = "identity",
    responses(
        (status = 204, description = "Signed out"),
        (status = 401, description = "Not signed in (UNAUTHENTICATED)", body = ErrorBody),
    )
)]
pub async fn sign_out(
    State(state): State<AppState>,
    Extension(current): Extension<CurrentUser>,
) -> Result<Response, ApiError> {
    state.identity.sign_out(&current.token).await?;
    Ok((
        StatusCode::NO_CONTENT,
        [(header::SET_COOKIE, state.cookies.clear())],
    )
        .into_response())
}

/// The signed-in user.
#[utoipa::path(
    get,
    path = "/auth/me",
    tag = "identity",
    responses(
        (status = 200, body = UserResponse),
        (status = 401, description = "Not signed in (UNAUTHENTICATED)", body = ErrorBody),
    )
)]
pub async fn me(Extension(current): Extension<CurrentUser>) -> Json<UserResponse> {
    Json(current.user.into())
}

/// Sets a password from a one-time setup link. Ends the user's other sessions.
#[utoipa::path(
    post,
    path = "/auth/password-setup",
    tag = "identity",
    request_body = PasswordSetupRequest,
    responses(
        (status = 204, description = "Password set; sign in with it"),
        (status = 400, description = "Link invalid, used or expired (INVALID_SETUP_LINK), or password too short or too long (INVALID_REQUEST)", body = ErrorBody),
    )
)]
pub async fn password_setup(
    State(state): State<AppState>,
    ApiJson(req): ApiJson<PasswordSetupRequest>,
) -> Result<StatusCode, ApiError> {
    state
        .identity
        .complete_password_setup(&req.token, &req.password)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Emails a one-time link to choose a new password. Answers the same whether or
/// not the address has an account, so it cannot be used to find accounts.
#[utoipa::path(
    post,
    path = "/auth/password-reset",
    tag = "identity",
    request_body = PasswordResetRequest,
    responses(
        (status = 202, description = "If the address has an account, a reset link is on its way"),
        (status = 400, description = "Not an email address (INVALID_REQUEST)", body = ErrorBody),
    )
)]
pub async fn password_reset(
    State(state): State<AppState>,
    ApiJson(req): ApiJson<PasswordResetRequest>,
) -> Result<StatusCode, ApiError> {
    state.identity.request_password_reset(&req.email).await?;
    Ok(StatusCode::ACCEPTED)
}
