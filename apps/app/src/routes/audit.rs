use crate::error::{ApiError, ErrorBody};
use crate::extract::ProjectAccess;
use crate::AppState;
use axum::extract::{Query, State};
use axum::http::header;
use axum::response::{IntoResponse, Response};
use axum::Json;
use chrono::{DateTime, Utc};
use donka_audit::{Action, Entry, Filter};
use donka_project::Role;
use donka_shared::page::PageRequest;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct AuditQuery {
    /// Only events by this person.
    pub actor: Option<Uuid>,
    /// Only this kind of change.
    pub action: Option<Action>,
    /// From this instant, inclusive (ISO-8601).
    pub from: Option<DateTime<Utc>>,
    /// Until this instant, exclusive (ISO-8601).
    pub until: Option<DateTime<Utc>>,
    /// Page size, 1 to 100 (default 50). Ignored by the export.
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

impl AuditQuery {
    fn filter(&self) -> Filter {
        Filter {
            actor: self.actor,
            action: self.action,
            from: self.from,
            until: self.until,
        }
    }
}

#[derive(Serialize, ToSchema)]
pub struct PersonRef {
    pub id: Uuid,
    pub email: String,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AuditEventResponse {
    pub id: i64,
    pub occurred_at: DateTime<Utc>,
    /// Who made the change; absent for changes Studio made on its own.
    pub actor: Option<PersonRef>,
    pub action: Action,
    /// The person the change is about (a member added…), when there is one.
    pub target: Option<PersonRef>,
    /// What changed, e.g. `{ "from": "viewer", "to": "editor" }`.
    pub details: Value,
}

#[derive(Serialize, ToSchema)]
pub struct AuditListResponse {
    pub items: Vec<AuditEventResponse>,
    /// Events matching the filter, across all pages.
    pub total: i64,
}

/// The project's audit log, newest first (owners).
#[utoipa::path(
    get,
    path = "/projects/{project_id}/audit",
    tag = "audit",
    params(("project_id" = Uuid, Path), AuditQuery),
    responses(
        (status = 200, body = AuditListResponse),
        (status = 400, description = "Invalid filter (INVALID_REQUEST)", body = ErrorBody),
        (status = 403, description = "Not an owner (FORBIDDEN)", body = ErrorBody),
        (status = 404, description = "PROJECT_NOT_FOUND", body = ErrorBody),
    )
)]
pub async fn list(
    State(state): State<AppState>,
    ProjectAccess(access): ProjectAccess,
    Query(query): Query<AuditQuery>,
) -> Result<Json<AuditListResponse>, ApiError> {
    access.require(Role::Owner)?;
    let page = state
        .audit
        .list(
            access.project_id(),
            &query.filter(),
            PageRequest::new(query.limit, query.offset),
        )
        .await
        .map_err(|err| ApiError::Internal(err.to_string()))?;
    let emails = emails(&state, &page.items).await?;
    Ok(Json(AuditListResponse {
        items: page
            .items
            .into_iter()
            .map(|entry| AuditEventResponse {
                id: entry.id,
                occurred_at: entry.occurred_at,
                actor: person(&emails, entry.actor_id),
                action: entry.action,
                target: person(&emails, entry.target_user_id),
                details: entry.details,
            })
            .collect(),
        total: page.total,
    }))
}

/// The project's audit log as CSV, with the same filters (owners). At most
/// 50,000 rows; narrow the dates for more.
#[utoipa::path(
    get,
    path = "/projects/{project_id}/audit/export",
    tag = "audit",
    params(("project_id" = Uuid, Path), AuditQuery),
    responses(
        (status = 200, description = "CSV: occurred_at, actor, action, target, details", content_type = "text/csv", body = String),
        (status = 403, description = "Not an owner (FORBIDDEN)", body = ErrorBody),
        (status = 404, description = "PROJECT_NOT_FOUND", body = ErrorBody),
    )
)]
pub async fn export(
    State(state): State<AppState>,
    ProjectAccess(access): ProjectAccess,
    Query(query): Query<AuditQuery>,
) -> Result<Response, ApiError> {
    access.require(Role::Owner)?;
    let project = state.projects.get(&access).await?;
    let entries = state
        .audit
        .export(access.project_id(), &query.filter())
        .await
        .map_err(|err| ApiError::Internal(err.to_string()))?;
    let emails = emails(&state, &entries).await?;
    let email = |id: Option<Uuid>| {
        id.and_then(|id| emails.get(&id).cloned())
            .unwrap_or_default()
    };

    let mut csv = String::from("occurred_at,actor,action,target,details\r\n");
    for entry in &entries {
        let row = [
            entry.occurred_at.to_rfc3339(),
            email(entry.actor_id),
            entry.action.as_str().to_owned(),
            email(entry.target_user_id),
            entry.details.to_string(),
        ];
        let cells: Vec<String> = row.iter().map(|cell| csv_cell(cell)).collect();
        csv.push_str(&cells.join(","));
        csv.push_str("\r\n");
    }
    let filename = format!("audit-{}.csv", project.key);
    Ok((
        [
            (header::CONTENT_TYPE, "text/csv; charset=utf-8".to_owned()),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{filename}\""),
            ),
        ],
        csv,
    )
        .into_response())
}

/// Quotes a CSV cell, and neutralizes text a spreadsheet would run as a
/// formula (a project named `=HYPERLINK(…)` must stay text).
fn csv_cell(value: &str) -> String {
    let guarded = if value.starts_with(['=', '+', '-', '@', '\t', '\r']) {
        format!("'{value}")
    } else {
        value.to_owned()
    };
    format!("\"{}\"", guarded.replace('"', "\"\""))
}

async fn emails(state: &AppState, entries: &[Entry]) -> Result<HashMap<Uuid, String>, ApiError> {
    let mut ids: Vec<Uuid> = entries
        .iter()
        .flat_map(|entry| [entry.actor_id, entry.target_user_id])
        .flatten()
        .collect();
    ids.sort_unstable();
    ids.dedup();
    Ok(state
        .identity
        .users_by_ids(&ids)
        .await?
        .into_iter()
        .map(|user| (user.id, user.email))
        .collect())
}

fn person(emails: &HashMap<Uuid, String>, id: Option<Uuid>) -> Option<PersonRef> {
    let id = id?;
    Some(PersonRef {
        id,
        email: emails.get(&id).cloned().unwrap_or_default(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cells_are_quoted_and_formulas_neutralized() {
        assert_eq!(csv_cell("plain"), "\"plain\"");
        assert_eq!(csv_cell("say \"hi\""), "\"say \"\"hi\"\"\"");
        assert_eq!(csv_cell("=HYPERLINK(\"x\")"), "\"'=HYPERLINK(\"\"x\"\")\"");
        assert_eq!(csv_cell("-1+1"), "\"'-1+1\"");
        assert_eq!(csv_cell("@SUM(A1)"), "\"'@SUM(A1)\"");
    }
}
