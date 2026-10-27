use crate::auth::{pending_token, CurrentUser};
use crate::config::MfaRequired;
use crate::error::{ApiError, ErrorBody};
use crate::extract::ApiJson;
use crate::rate_limit::ClientAddr;
use crate::AppState;
use axum::extract::State;
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{AppendHeaders, IntoResponse, Response};
use axum::{Extension, Json};
use donka_identity::{Locale, SignIn, User, MAX_PASSWORD_CHARS, MIN_PASSWORD_CHARS};
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
    #[schema(schema_with = password_schema)]
    pub password: String,
}

/// The password rules, published so the web app validates with the same limits.
fn password_schema() -> utoipa::openapi::Object {
    utoipa::openapi::ObjectBuilder::new()
        .schema_type(utoipa::openapi::schema::Type::String)
        .min_length(Some(MIN_PASSWORD_CHARS))
        .max_length(Some(MAX_PASSWORD_CHARS))
        .build()
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

/// The step that follows a correct password.
#[derive(Serialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum NextStep {
    /// Send the authenticator or recovery code to `/auth/sign-in/code`.
    Code,
    /// Policy requires a second step: set one up with `/auth/sign-in/enrolment`.
    Enrol,
}

#[derive(Serialize, ToSchema)]
pub struct SecondStepResponse {
    pub next: NextStep,
}

#[derive(Deserialize, ToSchema)]
pub struct CodeRequest {
    /// Six digits from the authenticator app, or a recovery code.
    pub code: String,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct EnrolmentResponse {
    /// The key to type into the authenticator app (base32).
    pub key: String,
    /// The same key as an `otpauth://` URL, to show as a QR code.
    pub otpauth_url: String,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct EnrolledResponse {
    pub user: UserResponse,
    /// Shown once; each works once instead of a code.
    pub recovery_codes: Vec<String>,
}

/// Whether policy requires a second step of this person (DNK-59).
pub async fn second_step_required(state: &AppState, user: &User) -> Result<bool, ApiError> {
    Ok(match state.mfa_required {
        MfaRequired::None => false,
        MfaRequired::All => true,
        MfaRequired::Privileged => user.is_admin || state.projects.owns_any(user.id).await?,
    })
}

/// Checks the password. Without a second step the session opens (200); with one,
/// or when policy requires one the account lacks, it answers 202 with the next
/// step and a short-lived cookie that holds the password step.
#[utoipa::path(
    post,
    path = "/auth/sign-in",
    tag = "identity",
    request_body = SignInRequest,
    responses(
        (status = 200, description = "Signed in; the session cookie is set", body = UserResponse),
        (status = 202, description = "Password accepted; the second step comes next (the pending cookie is set)", body = SecondStepResponse),
        (status = 401, description = "Email or password is incorrect, or the account is locked (INVALID_CREDENTIALS)", body = ErrorBody),
        (status = 403, description = "Missing CSRF header (CSRF_REQUIRED)", body = ErrorBody),
        (status = 429, description = "Too many sign-in attempts from this address (RATE_LIMITED); `Retry-After` says when to try again", body = ErrorBody),
    )
)]
pub async fn sign_in(
    State(state): State<AppState>,
    client: ClientAddr,
    ApiJson(req): ApiJson<SignInRequest>,
) -> Result<Response, ApiError> {
    client.check(&state.auth_limits.sign_in)?;
    // Known before the password is checked, from the account the email names; an
    // unknown or malformed email takes the same path as a wrong password.
    let required = match state.identity.find_by_email(&req.email).await {
        Ok(Some(user)) => second_step_required(&state, &user).await?,
        _ => false,
    };
    let outcome = state
        .identity
        .sign_in(&req.email, &req.password, required)
        .await?;
    Ok(match outcome {
        SignIn::Session(token, user) => (
            [(header::SET_COOKIE, state.cookies.session(token.expose()))],
            Json(UserResponse::from(user)),
        )
            .into_response(),
        SignIn::Code(pending) => second_step(&state, &pending, NextStep::Code),
        SignIn::Enrol(pending) => second_step(&state, &pending, NextStep::Enrol),
    })
}

fn second_step(
    state: &AppState,
    pending: &donka_identity::IssuedToken,
    next: NextStep,
) -> Response {
    (
        StatusCode::ACCEPTED,
        [(header::SET_COOKIE, state.cookies.pending(pending.expose()))],
        Json(SecondStepResponse { next }),
    )
        .into_response()
}

fn pending(headers: &HeaderMap) -> Result<&str, ApiError> {
    pending_token(headers).ok_or(ApiError::SignInExpired)
}

/// Finishes a sign-in with an authenticator code or a recovery code. A wrong
/// code counts towards the account lockout, like a wrong password.
#[utoipa::path(
    post,
    path = "/auth/sign-in/code",
    tag = "identity",
    request_body = CodeRequest,
    responses(
        (status = 200, description = "Signed in; the session cookie is set", body = UserResponse),
        (status = 401, description = "Wrong code or locked account (INVALID_CREDENTIALS); no password step or it expired (SIGN_IN_EXPIRED)", body = ErrorBody),
        (status = 429, description = "Too many attempts from this address (RATE_LIMITED)", body = ErrorBody),
    )
)]
pub async fn sign_in_code(
    State(state): State<AppState>,
    client: ClientAddr,
    headers: HeaderMap,
    ApiJson(req): ApiJson<CodeRequest>,
) -> Result<Response, ApiError> {
    client.check(&state.auth_limits.sign_in)?;
    let (token, user) = state
        .identity
        .complete_sign_in(pending(&headers)?, &req.code)
        .await?;
    Ok((
        AppendHeaders([
            (header::SET_COOKIE, state.cookies.session(token.expose())),
            (header::SET_COOKIE, state.cookies.clear_pending()),
        ]),
        Json(UserResponse::from(user)),
    )
        .into_response())
}

/// Starts setting up a second step when policy holds the sign-in until there is one.
#[utoipa::path(
    post,
    path = "/auth/sign-in/enrolment",
    tag = "identity",
    responses(
        (status = 200, body = EnrolmentResponse),
        (status = 401, description = "No password step, or it expired (SIGN_IN_EXPIRED)", body = ErrorBody),
    )
)]
pub async fn sign_in_enrolment(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<EnrolmentResponse>, ApiError> {
    let enrolment = state
        .identity
        .begin_enrolment_at_sign_in(pending(&headers)?)
        .await?;
    Ok(Json(EnrolmentResponse {
        key: enrolment.key,
        otpauth_url: enrolment.otpauth_url,
    }))
}

/// Turns the second step on with a first valid code and opens the session.
#[utoipa::path(
    post,
    path = "/auth/sign-in/enrolment/confirm",
    tag = "identity",
    request_body = CodeRequest,
    responses(
        (status = 200, description = "Signed in; the session cookie is set; recovery codes shown once", body = EnrolledResponse),
        (status = 400, description = "Wrong code (INVALID_CODE)", body = ErrorBody),
        (status = 401, description = "No password step, or it expired (SIGN_IN_EXPIRED)", body = ErrorBody),
        (status = 409, description = "No enrolment started (SECOND_STEP_OFF)", body = ErrorBody),
        (status = 429, description = "Too many attempts from this address (RATE_LIMITED)", body = ErrorBody),
    )
)]
pub async fn sign_in_enrolment_confirm(
    State(state): State<AppState>,
    client: ClientAddr,
    headers: HeaderMap,
    ApiJson(req): ApiJson<CodeRequest>,
) -> Result<Response, ApiError> {
    client.check(&state.auth_limits.sign_in)?;
    let (token, user, codes) = state
        .identity
        .confirm_enrolment_at_sign_in(pending(&headers)?, &req.code)
        .await?;
    Ok((
        AppendHeaders([
            (header::SET_COOKIE, state.cookies.session(token.expose())),
            (header::SET_COOKIE, state.cookies.clear_pending()),
        ]),
        Json(EnrolledResponse {
            user: user.into(),
            recovery_codes: codes.0,
        }),
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
        (status = 429, description = "Too many reset requests from this address (RATE_LIMITED); `Retry-After` says when to try again", body = ErrorBody),
    )
)]
pub async fn password_reset(
    State(state): State<AppState>,
    client: ClientAddr,
    ApiJson(req): ApiJson<PasswordResetRequest>,
) -> Result<StatusCode, ApiError> {
    client.check(&state.auth_limits.password_reset)?;
    state.identity.request_password_reset(&req.email).await?;
    Ok(StatusCode::ACCEPTED)
}
