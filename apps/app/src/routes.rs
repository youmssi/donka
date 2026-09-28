use crate::AppState;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use donka_engine::{Bundle, EvaluateOptions, Evaluation, RuntimeError};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::BTreeMap;

pub async fn health() -> &'static str {
    "ok"
}

/// Design-time evaluation from the editor. The editor sends the whole draft
/// project so sub-decision references resolve exactly as they will in a release.
#[derive(Debug, Deserialize)]
pub struct SimulateRequest {
    pub decisions: BTreeMap<String, Value>,
    pub key: String,
    #[serde(default)]
    pub context: Value,
}

pub async fn simulate(
    State(state): State<AppState>,
    Json(req): Json<SimulateRequest>,
) -> Result<Json<Evaluation>, ApiError> {
    let bundle = Bundle::from_json(req.decisions)?;
    let evaluation = state
        .runtime
        .evaluate(
            &bundle,
            &req.key,
            req.context,
            EvaluateOptions { trace: true },
        )
        .await?;
    Ok(Json(evaluation))
}

pub struct ApiError(RuntimeError);

impl From<RuntimeError> for ApiError {
    fn from(err: RuntimeError) -> Self {
        Self(err)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, body) = match &self.0 {
            RuntimeError::InvalidContent { key, message } => (
                StatusCode::UNPROCESSABLE_ENTITY,
                json!({ "code": "invalid_decision", "key": key, "message": message }),
            ),
            RuntimeError::NotFound(key) => (
                StatusCode::NOT_FOUND,
                json!({ "code": "decision_not_found", "key": key }),
            ),
            RuntimeError::Evaluation { details } => (
                StatusCode::UNPROCESSABLE_ENTITY,
                json!({ "code": "evaluation_failed", "details": details }),
            ),
            RuntimeError::Internal(message) => {
                tracing::error!(%message, "engine worker failed");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    json!({ "code": "internal_error" }),
                )
            }
        };
        (status, Json(body)).into_response()
    }
}
