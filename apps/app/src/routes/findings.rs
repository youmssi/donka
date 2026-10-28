//! What the table checks find in a decision, and acknowledging a finding as intended (DNK-76).

use crate::error::{ApiError, ErrorBody};
use crate::extract::{ApiJson, DecisionId, ProjectAccess};
use crate::routes::people::{emails, PersonRef};
use crate::AppState;
use axum::extract::{Path, State};
use axum::Json;
use chrono::{DateTime, Utc};
use donka_decision::Finding;
use donka_engine::tables::{self, FindingKind};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use utoipa::openapi::schema::{AdditionalProperties, ObjectBuilder, SchemaType, Type};
use utoipa::openapi::{RefOr, Schema};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub enum TableFindingKind {
    /// Inputs no row matches: the table gives no answer for them.
    Gap,
    /// A row that never fires: earlier rows already match all it matches.
    Overlap,
    /// A condition no input satisfies: its row never fires.
    NeverMatches,
    /// The table is too large to check completely.
    Incomplete,
}

#[derive(Serialize, ToSchema)]
pub struct TableColumnResponse {
    pub id: String,
    /// The name the editor shows: the column's name, else its field.
    pub name: String,
}

#[derive(Serialize, ToSchema)]
pub struct GapConditionResponse {
    pub column: TableColumnResponse,
    /// The condition, in the table's expression language (`> 60`).
    pub condition: String,
}

#[derive(Serialize, ToSchema)]
pub struct GapCaseResponse {
    pub conditions: Vec<GapConditionResponse>,
    /// An input that falls in the gap.
    #[schema(schema_with = example_schema)]
    pub example: Option<Value>,
}

/// A request context, as a caller would send it.
fn example_schema() -> impl Into<RefOr<Schema>> {
    ObjectBuilder::new()
        .schema_type(SchemaType::from_iter([Type::Object, Type::Null]))
        .additional_properties(Some(AdditionalProperties::FreeForm(true)))
        .description(Some("An input, as a request's context would carry it."))
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AcknowledgementResponse {
    pub reason: String,
    pub acknowledged_by: PersonRef,
    pub acknowledged_at: DateTime<Utc>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TableFindingResponse {
    /// Stays the same while the finding does; acknowledge the finding with it.
    pub fingerprint: String,
    pub kind: TableFindingKind,
    /// The decision table node.
    pub table_id: String,
    pub table_name: String,
    /// The row at fault, numbered from 1 as in the editor (overlap, never matches).
    pub row: Option<u32>,
    pub row_id: Option<String>,
    /// The column at fault (never matches).
    pub column: Option<TableColumnResponse>,
    /// Overlap: the earlier rows that already match all the row matches.
    pub covered_by: Vec<u32>,
    /// Overlap: other rows further down that never fire for the same reason.
    pub also: Vec<u32>,
    /// Overlap: the covering row gives the same result, so removing the row changes nothing.
    pub same_result: bool,
    /// Gap: the first uncovered inputs, out of `caseCount`.
    pub cases: Vec<GapCaseResponse>,
    pub case_count: Option<u32>,
    /// Overlap: an input the row matches but never gets to answer.
    #[schema(schema_with = example_schema)]
    pub example: Option<Value>,
    /// Present when an owner acknowledged the finding as intended.
    pub acknowledgement: Option<AcknowledgementResponse>,
}

#[derive(Serialize, ToSchema)]
pub struct TableFindingListResponse {
    pub items: Vec<TableFindingResponse>,
}

#[derive(Deserialize, ToSchema)]
pub struct AcknowledgeRequest {
    /// Why the finding is intended, 1 to 1000 characters.
    #[schema(min_length = 1, max_length = 1000)]
    pub reason: String,
}

#[derive(Deserialize)]
pub struct FindingPath {
    fingerprint: String,
}

/// The findings in the API's shape, with the emails of the people who acknowledged them.
pub async fn responses(
    state: &AppState,
    findings: Vec<Finding>,
) -> Result<Vec<TableFindingResponse>, ApiError> {
    let people = emails(
        state,
        findings
            .iter()
            .map(|f| f.acknowledgement.as_ref().map(|a| a.acknowledged_by)),
    )
    .await?;
    Ok(findings
        .into_iter()
        .map(|finding| response(&people, finding))
        .collect())
}

fn response(people: &HashMap<Uuid, String>, finding: Finding) -> TableFindingResponse {
    let column = |column: tables::Column| TableColumnResponse {
        id: column.id,
        name: column.name,
    };
    let Finding {
        finding,
        acknowledgement,
    } = finding;
    TableFindingResponse {
        fingerprint: finding.fingerprint,
        kind: match finding.kind {
            FindingKind::Gap => TableFindingKind::Gap,
            FindingKind::Overlap => TableFindingKind::Overlap,
            FindingKind::NeverMatches => TableFindingKind::NeverMatches,
            FindingKind::Incomplete => TableFindingKind::Incomplete,
        },
        table_id: finding.table_id,
        table_name: finding.table_name,
        row: finding.row,
        row_id: finding.row_id,
        column: finding.column.map(column),
        covered_by: finding.covered_by,
        also: finding.also,
        same_result: finding.same_result,
        cases: finding
            .cases
            .into_iter()
            .map(|case| GapCaseResponse {
                conditions: case
                    .conditions
                    .into_iter()
                    .map(|c| GapConditionResponse {
                        column: column(c.column),
                        condition: c.condition,
                    })
                    .collect(),
                example: case.example,
            })
            .collect(),
        case_count: finding.case_count,
        example: finding.example,
        acknowledgement: acknowledgement.map(|a| AcknowledgementResponse {
            reason: a.reason,
            acknowledged_by: PersonRef {
                id: a.acknowledged_by,
                email: people.get(&a.acknowledged_by).cloned().unwrap_or_default(),
            },
            acknowledged_at: a.acknowledged_at,
        }),
    }
}

/// What the table checks find in the decision's draft (any member).
#[utoipa::path(
    get,
    path = "/projects/{project_id}/decisions/{decision_id}/findings",
    tag = "decisions",
    params(("project_id" = Uuid, Path), ("decision_id" = Uuid, Path)),
    responses(
        (status = 200, body = TableFindingListResponse),
        (status = 404, description = "PROJECT_NOT_FOUND or DECISION_NOT_FOUND", body = ErrorBody),
    )
)]
pub async fn list(
    State(state): State<AppState>,
    ProjectAccess(access): ProjectAccess,
    DecisionId(id): DecisionId,
) -> Result<Json<TableFindingListResponse>, ApiError> {
    let findings = state.decisions.draft_findings(&access, id).await?;
    Ok(Json(TableFindingListResponse {
        items: responses(&state, findings).await?,
    }))
}

/// Acknowledges a finding of the draft as intended, with why (owners). The acknowledgement holds
/// on later versions while the same finding comes back; acknowledging again replaces the reason.
#[utoipa::path(
    put,
    path = "/projects/{project_id}/decisions/{decision_id}/findings/{fingerprint}/acknowledgement",
    tag = "decisions",
    params(
        ("project_id" = Uuid, Path),
        ("decision_id" = Uuid, Path),
        ("fingerprint" = String, Path, description = "The finding's fingerprint"),
    ),
    request_body = AcknowledgeRequest,
    responses(
        (status = 200, body = TableFindingResponse),
        (status = 400, description = "Missing or too long reason (INVALID_REQUEST)", body = ErrorBody),
        (status = 403, description = "Only owners acknowledge findings (FORBIDDEN)", body = ErrorBody),
        (status = 404, description = "PROJECT_NOT_FOUND, DECISION_NOT_FOUND, or FINDING_NOT_FOUND (the draft no longer has it)", body = ErrorBody),
        (status = 409, description = "PROJECT_ARCHIVED", body = ErrorBody),
    )
)]
pub async fn acknowledge(
    State(state): State<AppState>,
    ProjectAccess(access): ProjectAccess,
    DecisionId(id): DecisionId,
    Path(path): Path<FindingPath>,
    ApiJson(req): ApiJson<AcknowledgeRequest>,
) -> Result<Json<TableFindingResponse>, ApiError> {
    let finding = state
        .decisions
        .acknowledge_finding(&access, id, &path.fingerprint, &req.reason)
        .await?;
    let mut items = responses(&state, vec![finding]).await?;
    items.pop().map(Json).ok_or(ApiError::FindingMissing)
}
