use crate::error::{ApiError, ErrorBody};
use crate::extract::ApiJson;
use crate::AppState;
use axum::extract::State;
use axum::Json;
use donka_engine::{Bundle, EvaluateOptions};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use utoipa::ToSchema;

/// Design-time evaluation from the editor. The editor sends the whole draft
/// project so sub-decision references resolve exactly as they will in a release.
#[derive(Debug, Deserialize, ToSchema)]
pub struct SimulateRequest {
    /// Every decision of the project (JDM documents), keyed as graphs reference them.
    #[schema(value_type = Object)]
    pub decisions: BTreeMap<String, Value>,
    /// The decision to evaluate.
    pub key: String,
    /// Input passed to the decision.
    #[serde(default)]
    #[schema(value_type = Object)]
    pub context: Value,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SimulateResponse {
    #[schema(value_type = Object)]
    pub result: Value,
    /// Per-node trace: inputs, outputs and timing of each node.
    #[schema(value_type = Object)]
    pub trace: Value,
    /// Engine evaluation time, e.g. `412.5µs`.
    pub performance: String,
}

#[utoipa::path(
    post,
    path = "/simulate",
    tag = "simulator",
    request_body = SimulateRequest,
    responses(
        (status = 200, description = "Result and per-node trace", body = SimulateResponse),
        (status = 400, description = "Malformed request body (INVALID_REQUEST)", body = ErrorBody),
        (status = 404, description = "No decision with that key (DECISION_NOT_FOUND)", body = ErrorBody),
        (status = 422, description = "Invalid decision model (INVALID_DECISION) or evaluation error (EVALUATION_FAILED)", body = ErrorBody),
    )
)]
pub async fn simulate(
    State(state): State<AppState>,
    ApiJson(req): ApiJson<SimulateRequest>,
) -> Result<Json<SimulateResponse>, ApiError> {
    let bundle = Bundle::from_json(req.decisions)?;
    let evaluation = state
        .runtime
        .evaluate(
            &bundle,
            &req.key,
            req.context,
            EvaluateOptions {
                trace: true,
                ..Default::default()
            },
        )
        .await?;
    Ok(Json(SimulateResponse {
        result: evaluation.result,
        trace: evaluation.trace.unwrap_or(Value::Null),
        performance: evaluation.performance,
    }))
}
