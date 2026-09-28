//! The one error shape of the API: `{ message, code, requestId, fields?, details? }`.
//!
//! Handlers return `ApiError`; this module is the only place that maps an error
//! to a status code, and it never exposes internal detail on a 500.

use crate::request_id;
use axum::extract::rejection::JsonRejection;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use donka_engine::RuntimeError;
use donka_identity::IdentityError;
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;
use utoipa::ToSchema;

/// Rejections from the JSON extractor can quote parts of the body; keep them short.
const MAX_REJECTION_LEN: usize = 300;

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ErrorBody {
    /// Safe to show to a person.
    pub message: String,
    /// Stable, machine-readable code, e.g. `DECISION_NOT_FOUND`.
    pub code: &'static str,
    /// Same value as the `x-request-id` response header; quote it when reporting a problem.
    pub request_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fields: Option<BTreeMap<String, String>>,
    /// Engine error document (failing node and reason) for evaluation failures.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<Value>,
}

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("invalid request: {0}")]
    InvalidRequest(String),
    #[error("decision '{0}' not found")]
    DecisionNotFound(String),
    #[error("decision '{key}' is invalid: {message}")]
    InvalidDecision { key: String, message: String },
    #[error("evaluation failed")]
    EvaluationFailed(Value),
    #[error("database unavailable")]
    DatabaseUnavailable,
    #[error("no such endpoint")]
    RouteNotFound,
    #[error("not signed in")]
    Unauthenticated,
    #[error("invalid credentials")]
    InvalidCredentials,
    #[error("missing CSRF header")]
    CsrfRequired,
    #[error("invalid password-setup link")]
    InvalidSetupLink,
    #[error("forbidden")]
    Forbidden,
    #[error("email already used")]
    EmailTaken,
    #[error("invalid field {field}: {message}")]
    InvalidField {
        field: &'static str,
        message: String,
    },
    #[error("internal error: {0}")]
    Internal(String),
}

impl From<RuntimeError> for ApiError {
    fn from(err: RuntimeError) -> Self {
        match err {
            RuntimeError::NotFound(key) => Self::DecisionNotFound(key),
            RuntimeError::InvalidContent { key, message } => Self::InvalidDecision { key, message },
            RuntimeError::Evaluation { details } => Self::EvaluationFailed(details),
            RuntimeError::Internal(message) => Self::Internal(message),
        }
    }
}

impl From<IdentityError> for ApiError {
    fn from(err: IdentityError) -> Self {
        match err {
            IdentityError::InvalidCredentials => Self::InvalidCredentials,
            IdentityError::Unauthenticated => Self::Unauthenticated,
            IdentityError::InvalidSetupLink => Self::InvalidSetupLink,
            IdentityError::Forbidden => Self::Forbidden,
            IdentityError::EmailTaken => Self::EmailTaken,
            IdentityError::WeakPassword => Self::InvalidField {
                field: "password",
                message: IdentityError::WeakPassword.to_string(),
            },
            IdentityError::InvalidEmail => Self::InvalidField {
                field: "email",
                message: IdentityError::InvalidEmail.to_string(),
            },
            IdentityError::Database(err) => Self::Internal(err.to_string()),
            IdentityError::Internal(message) => Self::Internal(message),
        }
    }
}

impl From<JsonRejection> for ApiError {
    fn from(rejection: JsonRejection) -> Self {
        let mut text = rejection.body_text();
        text.truncate(MAX_REJECTION_LEN);
        Self::InvalidRequest(text)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let request_id = request_id::current().unwrap_or_default();
        let (status, code, message, fields, details) = match self {
            Self::InvalidRequest(reason) => (
                StatusCode::BAD_REQUEST,
                "INVALID_REQUEST",
                format!("The request body is not valid: {reason}"),
                None,
                None,
            ),
            Self::DecisionNotFound(key) => (
                StatusCode::NOT_FOUND,
                "DECISION_NOT_FOUND",
                format!("No decision named '{key}' in this project."),
                Some(BTreeMap::from([("key".to_owned(), key)])),
                None,
            ),
            Self::InvalidDecision { key, message } => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "INVALID_DECISION",
                format!("Decision '{key}' is not a valid decision model: {message}"),
                Some(BTreeMap::from([("key".to_owned(), key)])),
                None,
            ),
            Self::EvaluationFailed(details) => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "EVALUATION_FAILED",
                "The decision could not be evaluated with this input.".to_owned(),
                None,
                Some(details),
            ),
            Self::DatabaseUnavailable => (
                StatusCode::SERVICE_UNAVAILABLE,
                "DATABASE_UNAVAILABLE",
                "The database is not reachable. Try again shortly.".to_owned(),
                None,
                None,
            ),
            Self::RouteNotFound => (
                StatusCode::NOT_FOUND,
                "NOT_FOUND",
                "There is no endpoint at this path.".to_owned(),
                None,
                None,
            ),
            Self::Unauthenticated => (
                StatusCode::UNAUTHORIZED,
                "UNAUTHENTICATED",
                "Sign in to continue.".to_owned(),
                None,
                None,
            ),
            // One message for every sign-in failure: it must not reveal whether
            // the email exists or the account is locked.
            Self::InvalidCredentials => (
                StatusCode::UNAUTHORIZED,
                "INVALID_CREDENTIALS",
                "Email or password is incorrect.".to_owned(),
                None,
                None,
            ),
            Self::CsrfRequired => (
                StatusCode::FORBIDDEN,
                "CSRF_REQUIRED",
                format!("Requests that change data must send the {} header.", crate::auth::CSRF_HEADER),
                None,
                None,
            ),
            Self::InvalidSetupLink => (
                StatusCode::BAD_REQUEST,
                "INVALID_SETUP_LINK",
                "This link is invalid, already used or expired. Ask an administrator for a new one."
                    .to_owned(),
                None,
                None,
            ),
            Self::Forbidden => (
                StatusCode::FORBIDDEN,
                "FORBIDDEN",
                "You do not have permission to do this.".to_owned(),
                None,
                None,
            ),
            Self::EmailTaken => (
                StatusCode::CONFLICT,
                "EMAIL_TAKEN",
                "Someone already has an account with this email.".to_owned(),
                Some(BTreeMap::from([(
                    "email".to_owned(),
                    "already used".to_owned(),
                )])),
                None,
            ),
            Self::InvalidField { field, message } => (
                StatusCode::BAD_REQUEST,
                "INVALID_REQUEST",
                format!("Check the {field} field."),
                Some(BTreeMap::from([(field.to_owned(), message)])),
                None,
            ),
            Self::Internal(reason) => {
                tracing::error!(%reason, "unexpected failure");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "INTERNAL_ERROR",
                    "Something went wrong on our side. Quote the request id if you report it."
                        .to_owned(),
                    None,
                    None,
                )
            }
        };
        let body = ErrorBody {
            message,
            code,
            request_id,
            fields,
            details,
        };
        (status, Json(body)).into_response()
    }
}
