use crate::error::{ApiError, ErrorBody};
use crate::extract::{ApiJson, DecisionId, ProjectAccess, ScenarioId, VersionNumber};
use crate::routes::people::{emails, PersonRef};
use crate::AppState;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::Json;
use chrono::{DateTime, Utc};
use donka_decision::{
    Match, Mismatch, Scenario, ScenarioFields, TestResult, TestStatus, TestSummary,
    MAX_SCENARIO_NAME_CHARS,
};
use donka_shared::page::PageRequest;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use utoipa::openapi::schema::{AdditionalProperties, ObjectBuilder, Type};
use utoipa::openapi::{RefOr, Schema};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

/// How much of the output a scenario pins down.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum MatchMode {
    /// The output is exactly the expected document.
    Exact,
    /// Every expected field has its value; other fields are free.
    Partial,
}

impl From<MatchMode> for Match {
    fn from(mode: MatchMode) -> Self {
        match mode {
            MatchMode::Exact => Match::Exact,
            MatchMode::Partial => Match::Partial,
        }
    }
}

impl From<Match> for MatchMode {
    fn from(mode: Match) -> Self {
        match mode {
            Match::Exact => MatchMode::Exact,
            Match::Partial => MatchMode::Partial,
        }
    }
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ScenarioResponse {
    pub id: Uuid,
    pub decision_id: Uuid,
    pub decision_key: String,
    pub name: String,
    /// The input the decision is evaluated with.
    #[schema(schema_with = object_schema)]
    pub input: Value,
    /// The output it should give.
    #[schema(schema_with = object_schema)]
    pub expected: Value,
    #[serde(rename = "match")]
    pub match_mode: MatchMode,
    pub created_at: DateTime<Utc>,
    pub created_by: PersonRef,
    pub updated_at: DateTime<Utc>,
    pub updated_by: PersonRef,
}

#[derive(Serialize, ToSchema)]
pub struct ScenarioListResponse {
    pub items: Vec<ScenarioResponse>,
    /// Scenarios across all pages.
    pub total: i64,
}

#[derive(Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateScenarioRequest {
    /// The decision the scenario evaluates.
    pub decision_id: Uuid,
    #[schema(schema_with = name_schema)]
    pub name: String,
    #[schema(schema_with = object_schema)]
    pub input: Value,
    #[schema(schema_with = object_schema)]
    pub expected: Value,
    #[serde(rename = "match")]
    pub match_mode: MatchMode,
}

#[derive(Deserialize, ToSchema)]
pub struct UpdateScenarioRequest {
    #[schema(schema_with = name_schema)]
    pub name: String,
    #[schema(schema_with = object_schema)]
    pub input: Value,
    #[schema(schema_with = object_schema)]
    pub expected: Value,
    #[serde(rename = "match")]
    pub match_mode: MatchMode,
}

#[derive(Deserialize, IntoParams)]
#[serde(rename_all = "camelCase")]
#[into_params(parameter_in = Query)]
pub struct ScenariosQuery {
    /// Only the scenarios of this decision.
    pub decision_id: Option<Uuid>,
    /// Page size, 1 to 100 (default 50).
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

/// How the project's scenarios went on a version.
#[derive(Serialize, ToSchema)]
pub struct TestSummaryResponse {
    pub passed: i64,
    pub failed: i64,
    /// Scenarios that could not run (a decision without a version, an evaluation error).
    pub errors: i64,
}

impl From<TestSummary> for TestSummaryResponse {
    fn from(summary: TestSummary) -> Self {
        Self {
            passed: summary.passed,
            failed: summary.failed,
            errors: summary.errors,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum TestStatusResponse {
    Passed,
    /// The decision gave another output.
    Failed,
    /// The decision could not run.
    Error,
}

/// One field where the output differs: `path` is dotted (`decision.limit`),
/// empty for the whole output; a side is absent when the field is missing there.
#[derive(Serialize, ToSchema)]
pub struct MismatchResponse {
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actual: Option<Value>,
}

/// A scenario as it ran on the version, and what happened.
#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TestResultResponse {
    pub scenario_id: Uuid,
    pub name: String,
    pub decision_key: String,
    #[schema(schema_with = object_schema)]
    pub input: Value,
    #[schema(schema_with = object_schema)]
    pub expected: Value,
    #[serde(rename = "match")]
    pub match_mode: MatchMode,
    pub status: TestStatusResponse,
    /// What the decision returned; absent when it could not run.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actual: Option<Value>,
    /// Empty when it passed.
    pub mismatches: Vec<MismatchResponse>,
    /// It could not run because this decision has no saved version.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub missing_decision: Option<String>,
    /// It could not run: the engine's message.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub struct TestResultListResponse {
    pub summary: TestSummaryResponse,
    pub items: Vec<TestResultResponse>,
}

fn object_schema() -> impl Into<RefOr<Schema>> {
    ObjectBuilder::new()
        .schema_type(Type::Object)
        .additional_properties(Some(AdditionalProperties::FreeForm(true)))
        .description(Some("A JSON object."))
}

fn name_schema() -> impl Into<RefOr<Schema>> {
    ObjectBuilder::new()
        .schema_type(Type::String)
        .min_length(Some(1))
        .max_length(Some(MAX_SCENARIO_NAME_CHARS))
        .description(Some("Unique among the decision's scenarios (any case)."))
}

/// The project's scenarios, by decision and name (any member).
#[utoipa::path(
    get,
    path = "/projects/{project_id}/test-scenarios",
    tag = "scenarios",
    params(("project_id" = Uuid, Path), ScenariosQuery),
    responses(
        (status = 200, body = ScenarioListResponse),
        (status = 404, description = "PROJECT_NOT_FOUND", body = ErrorBody),
    )
)]
pub async fn list(
    State(state): State<AppState>,
    ProjectAccess(access): ProjectAccess,
    Query(query): Query<ScenariosQuery>,
) -> Result<Json<ScenarioListResponse>, ApiError> {
    let page = state
        .decisions
        .scenarios(
            &access,
            query.decision_id,
            PageRequest::new(query.limit, query.offset),
        )
        .await?;
    let emails = emails(
        &state,
        page.items
            .iter()
            .flat_map(|s| [Some(s.created_by), Some(s.updated_by)]),
    )
    .await?;
    Ok(Json(ScenarioListResponse {
        items: page
            .items
            .into_iter()
            .map(|s| response(&emails, s))
            .collect(),
        total: page.total,
    }))
}

/// A new scenario for one of the project's decisions (editors and owners).
#[utoipa::path(
    post,
    path = "/projects/{project_id}/test-scenarios",
    tag = "scenarios",
    params(("project_id" = Uuid, Path)),
    request_body = CreateScenarioRequest,
    responses(
        (status = 201, body = ScenarioResponse),
        (status = 400, description = "Invalid name, input or expected output (INVALID_REQUEST, fields)", body = ErrorBody),
        (status = 403, description = "Viewers cannot change scenarios (FORBIDDEN)", body = ErrorBody),
        (status = 404, description = "PROJECT_NOT_FOUND or DECISION_NOT_FOUND", body = ErrorBody),
        (status = 409, description = "SCENARIO_NAME_TAKEN or PROJECT_ARCHIVED", body = ErrorBody),
    )
)]
pub async fn create(
    State(state): State<AppState>,
    ProjectAccess(access): ProjectAccess,
    ApiJson(req): ApiJson<CreateScenarioRequest>,
) -> Result<(StatusCode, Json<ScenarioResponse>), ApiError> {
    let fields = ScenarioFields {
        name: req.name,
        input: req.input,
        expected: req.expected,
        match_mode: req.match_mode.into(),
    };
    let created = state
        .decisions
        .create_scenario(&access, req.decision_id, fields)
        .await?;
    Ok((StatusCode::CREATED, Json(one(&state, created).await?)))
}

/// One scenario (any member).
#[utoipa::path(
    get,
    path = "/projects/{project_id}/test-scenarios/{scenario_id}",
    tag = "scenarios",
    params(("project_id" = Uuid, Path), ("scenario_id" = Uuid, Path)),
    responses(
        (status = 200, body = ScenarioResponse),
        (status = 404, description = "PROJECT_NOT_FOUND or SCENARIO_NOT_FOUND", body = ErrorBody),
    )
)]
pub async fn get(
    State(state): State<AppState>,
    ProjectAccess(access): ProjectAccess,
    ScenarioId(id): ScenarioId,
) -> Result<Json<ScenarioResponse>, ApiError> {
    let scenario = state.decisions.scenario(&access, id).await?;
    Ok(Json(one(&state, scenario).await?))
}

/// Changes a scenario (editors and owners). Results already recorded keep it as it ran.
#[utoipa::path(
    put,
    path = "/projects/{project_id}/test-scenarios/{scenario_id}",
    tag = "scenarios",
    params(("project_id" = Uuid, Path), ("scenario_id" = Uuid, Path)),
    request_body = UpdateScenarioRequest,
    responses(
        (status = 200, body = ScenarioResponse),
        (status = 400, description = "Invalid name, input or expected output (INVALID_REQUEST, fields)", body = ErrorBody),
        (status = 403, description = "Viewers cannot change scenarios (FORBIDDEN)", body = ErrorBody),
        (status = 404, description = "PROJECT_NOT_FOUND or SCENARIO_NOT_FOUND", body = ErrorBody),
        (status = 409, description = "SCENARIO_NAME_TAKEN or PROJECT_ARCHIVED", body = ErrorBody),
    )
)]
pub async fn update(
    State(state): State<AppState>,
    ProjectAccess(access): ProjectAccess,
    ScenarioId(id): ScenarioId,
    ApiJson(req): ApiJson<UpdateScenarioRequest>,
) -> Result<Json<ScenarioResponse>, ApiError> {
    let fields = ScenarioFields {
        name: req.name,
        input: req.input,
        expected: req.expected,
        match_mode: req.match_mode.into(),
    };
    let updated = state.decisions.update_scenario(&access, id, fields).await?;
    Ok(Json(one(&state, updated).await?))
}

/// Deletes a scenario (editors and owners). Results already recorded stay.
#[utoipa::path(
    delete,
    path = "/projects/{project_id}/test-scenarios/{scenario_id}",
    tag = "scenarios",
    params(("project_id" = Uuid, Path), ("scenario_id" = Uuid, Path)),
    responses(
        (status = 204, description = "Deleted"),
        (status = 403, description = "Viewers cannot change scenarios (FORBIDDEN)", body = ErrorBody),
        (status = 404, description = "PROJECT_NOT_FOUND or SCENARIO_NOT_FOUND", body = ErrorBody),
        (status = 409, description = "PROJECT_ARCHIVED", body = ErrorBody),
    )
)]
pub async fn delete(
    State(state): State<AppState>,
    ProjectAccess(access): ProjectAccess,
    ScenarioId(id): ScenarioId,
) -> Result<StatusCode, ApiError> {
    state.decisions.delete_scenario(&access, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// How every scenario of the project went when this version was saved (any member).
#[utoipa::path(
    get,
    path = "/projects/{project_id}/decisions/{decision_id}/versions/{number}/test-results",
    tag = "scenarios",
    params(("project_id" = Uuid, Path), ("decision_id" = Uuid, Path), ("number" = i32, Path)),
    responses(
        (status = 200, body = TestResultListResponse),
        (status = 404, description = "PROJECT_NOT_FOUND, DECISION_NOT_FOUND or VERSION_NOT_FOUND", body = ErrorBody),
    )
)]
pub async fn test_results(
    State(state): State<AppState>,
    ProjectAccess(access): ProjectAccess,
    DecisionId(id): DecisionId,
    VersionNumber(number): VersionNumber,
) -> Result<Json<TestResultListResponse>, ApiError> {
    let results = state.decisions.test_results(&access, id, number).await?;
    let mut summary = TestSummary::default();
    for result in &results {
        match result.status {
            TestStatus::Passed => summary.passed += 1,
            TestStatus::Failed => summary.failed += 1,
            TestStatus::Error => summary.errors += 1,
        }
    }
    Ok(Json(TestResultListResponse {
        summary: summary.into(),
        items: results.into_iter().map(result).collect(),
    }))
}

async fn one(state: &AppState, scenario: Scenario) -> Result<ScenarioResponse, ApiError> {
    let emails = emails(
        state,
        [Some(scenario.created_by), Some(scenario.updated_by)],
    )
    .await?;
    Ok(response(&emails, scenario))
}

fn response(emails: &HashMap<Uuid, String>, scenario: Scenario) -> ScenarioResponse {
    let person = |id: Uuid| PersonRef {
        id,
        email: emails.get(&id).cloned().unwrap_or_default(),
    };
    ScenarioResponse {
        id: scenario.id,
        decision_id: scenario.decision_id,
        decision_key: scenario.decision_key,
        name: scenario.name,
        input: scenario.input,
        expected: scenario.expected,
        match_mode: scenario.match_mode.into(),
        created_at: scenario.created_at,
        created_by: person(scenario.created_by),
        updated_at: scenario.updated_at,
        updated_by: person(scenario.updated_by),
    }
}

fn result(result: TestResult) -> TestResultResponse {
    TestResultResponse {
        scenario_id: result.scenario_id,
        name: result.name,
        decision_key: result.decision_key,
        input: result.input,
        expected: result.expected,
        match_mode: result.match_mode.into(),
        status: match result.status {
            TestStatus::Passed => TestStatusResponse::Passed,
            TestStatus::Failed => TestStatusResponse::Failed,
            TestStatus::Error => TestStatusResponse::Error,
        },
        actual: result.actual,
        mismatches: result
            .mismatches
            .into_iter()
            .map(
                |Mismatch {
                     path,
                     expected,
                     actual,
                 }| MismatchResponse {
                    path,
                    expected,
                    actual,
                },
            )
            .collect(),
        missing_decision: result.missing_decision,
        error: result.error,
    }
}
