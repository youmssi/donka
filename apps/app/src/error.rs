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
