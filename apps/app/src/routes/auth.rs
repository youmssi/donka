use crate::auth::CurrentUser;
use crate::error::{ApiError, ErrorBody};
use crate::extract::ApiJson;
use crate::AppState;
use axum::extract::State;
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use donka_identity::User;
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

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MeResponse {
    pub id: Uuid,
    pub email: String,
    pub is_admin: bool,
}

impl From<User> for MeResponse {
    fn from(user: User) -> Self {
        Self {
            id: user.id,
            email: user.email,
            is_admin: user.is_admin,
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
        (status = 200, description = "Signed in; the session cookie is set", body = MeResponse),
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
        Json(MeResponse::from(user)),
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
        (status = 200, body = MeResponse),
        (status = 401, description = "Not signed in (UNAUTHENTICATED)", body = ErrorBody),
    )
)]
pub async fn me(Extension(current): Extension<CurrentUser>) -> Json<MeResponse> {
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
