//! Audit module: who changed what, and when.
//!
//! Modules call [`record`] with the transaction of the change itself, so an
//! event exists exactly when the change was committed. Rows are never updated
//! or deleted: the table has an append-only trigger and the application role
//! holds no UPDATE, DELETE or TRUNCATE privilege on it (migration
//! `20260929000001_audit_events.sql`).

use chrono::{DateTime, Utc};
use donka_db::PgPool;
use donka_shared::page::{Page, PageRequest};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::PgConnection;
use uuid::Uuid;

/// Most rows one CSV export returns; a narrower filter gets the rest.
pub const MAX_EXPORT_ROWS: i64 = 50_000;

/// Every kind of state change. The stored text is part of the audit record:
/// never rename a value, only add new ones.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "text")]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub enum Action {
    #[serde(rename = "user.signed_in")]
    #[sqlx(rename = "user.signed_in")]
    UserSignedIn,
    /// A wrong password for an existing account; `details.locked` when it locked the account.
    #[serde(rename = "user.sign_in_failed")]
    #[sqlx(rename = "user.sign_in_failed")]
    UserSignInFailed,
    #[serde(rename = "user.signed_out")]
    #[sqlx(rename = "user.signed_out")]
    UserSignedOut,
    /// From an invitation, a reset or the first-administrator link.
    #[serde(rename = "user.password_set")]
    #[sqlx(rename = "user.password_set")]
    UserPasswordSet,
    #[serde(rename = "user.password_reset_requested")]
    #[sqlx(rename = "user.password_reset_requested")]
    UserPasswordResetRequested,
    #[serde(rename = "user.invited")]
    #[sqlx(rename = "user.invited")]
    UserInvited,
    #[serde(rename = "project.created")]
    #[sqlx(rename = "project.created")]
    ProjectCreated,
    #[serde(rename = "project.updated")]
    #[sqlx(rename = "project.updated")]
    ProjectUpdated,
    #[serde(rename = "project.archived")]
    #[sqlx(rename = "project.archived")]
    ProjectArchived,
    #[serde(rename = "project.restored")]
    #[sqlx(rename = "project.restored")]
    ProjectRestored,
    #[serde(rename = "member.added")]
    #[sqlx(rename = "member.added")]
    MemberAdded,
    #[serde(rename = "member.role_changed")]
    #[sqlx(rename = "member.role_changed")]
    MemberRoleChanged,
    #[serde(rename = "member.removed")]
    #[sqlx(rename = "member.removed")]
    MemberRemoved,
    #[serde(rename = "decision.created")]
    #[sqlx(rename = "decision.created")]
    DecisionCreated,
    #[serde(rename = "decision.deleted")]
    #[sqlx(rename = "decision.deleted")]
    DecisionDeleted,
    #[serde(rename = "decision.version_saved")]
    #[sqlx(rename = "decision.version_saved")]
    DecisionVersionSaved,
    #[serde(rename = "decision.version_restored")]
    #[sqlx(rename = "decision.version_restored")]
    DecisionVersionRestored,
    #[serde(rename = "scenario.created")]
    #[sqlx(rename = "scenario.created")]
    ScenarioCreated,
    #[serde(rename = "scenario.updated")]
    #[sqlx(rename = "scenario.updated")]
    ScenarioUpdated,
    #[serde(rename = "scenario.deleted")]
    #[sqlx(rename = "scenario.deleted")]
    ScenarioDeleted,
    #[serde(rename = "release.created")]
    #[sqlx(rename = "release.created")]
    ReleaseCreated,
    #[serde(rename = "release.deployed")]
    #[sqlx(rename = "release.deployed")]
    ReleaseDeployed,
    #[serde(rename = "token.issued")]
    #[sqlx(rename = "token.issued")]
    TokenIssued,
    #[serde(rename = "token.revoked")]
    #[sqlx(rename = "token.revoked")]
    TokenRevoked,
    #[serde(rename = "release.rolled_back")]
    #[sqlx(rename = "release.rolled_back")]
    ReleaseRolledBack,
    #[serde(rename = "approval.requested")]
    #[sqlx(rename = "approval.requested")]
    ApprovalRequested,
    #[serde(rename = "approval.approved")]
    #[sqlx(rename = "approval.approved")]
    ApprovalApproved,
    #[serde(rename = "approval.rejected")]
    #[sqlx(rename = "approval.rejected")]
    ApprovalRejected,
    #[serde(rename = "approval.withdrawn")]
    #[sqlx(rename = "approval.withdrawn")]
    ApprovalWithdrawn,
    /// Someone opened a decision record (it holds personal data).
    #[serde(rename = "decision_record.viewed")]
    #[sqlx(rename = "decision_record.viewed")]
    DecisionRecordViewed,
    #[serde(rename = "decision_record.replayed")]
    #[sqlx(rename = "decision_record.replayed")]
    DecisionRecordReplayed,
    /// Records older than the retention period were deleted; no actor.
    #[serde(rename = "decision_log.purged")]
    #[sqlx(rename = "decision_log.purged")]
    DecisionLogPurged,
    #[serde(rename = "decision_log.settings_updated")]
    #[sqlx(rename = "decision_log.settings_updated")]
    DecisionLogSettingsUpdated,
    #[serde(rename = "decision_log_token.issued")]
    #[sqlx(rename = "decision_log_token.issued")]
    DecisionLogTokenIssued,
    #[serde(rename = "decision_log_token.revoked")]
    #[sqlx(rename = "decision_log_token.revoked")]
    DecisionLogTokenRevoked,
}

impl Action {
    /// The stored and exported name, e.g. `member.added`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::UserSignedIn => "user.signed_in",
            Self::UserSignInFailed => "user.sign_in_failed",
            Self::UserSignedOut => "user.signed_out",
            Self::UserPasswordSet => "user.password_set",
            Self::UserPasswordResetRequested => "user.password_reset_requested",
            Self::UserInvited => "user.invited",
            Self::ProjectCreated => "project.created",
            Self::ProjectUpdated => "project.updated",
            Self::ProjectArchived => "project.archived",
            Self::ProjectRestored => "project.restored",
            Self::MemberAdded => "member.added",
            Self::MemberRoleChanged => "member.role_changed",
            Self::MemberRemoved => "member.removed",
            Self::DecisionCreated => "decision.created",
            Self::DecisionDeleted => "decision.deleted",
            Self::DecisionVersionSaved => "decision.version_saved",
            Self::DecisionVersionRestored => "decision.version_restored",
            Self::ScenarioCreated => "scenario.created",
            Self::ScenarioUpdated => "scenario.updated",
            Self::ScenarioDeleted => "scenario.deleted",
            Self::ReleaseCreated => "release.created",
            Self::ReleaseDeployed => "release.deployed",
            Self::TokenIssued => "token.issued",
            Self::TokenRevoked => "token.revoked",
            Self::ReleaseRolledBack => "release.rolled_back",
            Self::ApprovalRequested => "approval.requested",
            Self::ApprovalApproved => "approval.approved",
            Self::ApprovalRejected => "approval.rejected",
            Self::ApprovalWithdrawn => "approval.withdrawn",
            Self::DecisionRecordViewed => "decision_record.viewed",
            Self::DecisionRecordReplayed => "decision_record.replayed",
            Self::DecisionLogPurged => "decision_log.purged",
            Self::DecisionLogSettingsUpdated => "decision_log.settings_updated",
            Self::DecisionLogTokenIssued => "decision_log_token.issued",
            Self::DecisionLogTokenRevoked => "decision_log_token.revoked",
        }
    }
}

/// A change to record. Build it with [`Event::new`] and the `with_*` methods.
#[derive(Debug, Clone)]
pub struct Event {
    at: DateTime<Utc>,
    actor: Option<Uuid>,
    action: Action,
    project: Option<Uuid>,
    target_user: Option<Uuid>,
    details: Value,
}

impl Event {
    pub fn new(at: DateTime<Utc>, actor: Option<Uuid>, action: Action) -> Self {
        Self {
            at,
            actor,
            action,
            project: None,
            target_user: None,
            details: Value::Object(Default::default()),
        }
    }

    pub fn in_project(mut self, project: Uuid) -> Self {
        self.project = Some(project);
        self
    }

    pub fn about_user(mut self, user: Uuid) -> Self {
        self.target_user = Some(user);
        self
    }

    /// What changed. Never put a secret (password, token) here.
    pub fn with_details(mut self, details: Value) -> Self {
        self.details = details;
        self
    }
}

/// Writes the event on the caller's connection, normally inside the
/// transaction of the change it describes.
pub async fn record(conn: &mut PgConnection, event: Event) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO audit_events (occurred_at, actor_id, action, project_id, target_user_id, details) \
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(event.at)
    .bind(event.actor)
    .bind(event.action)
    .bind(event.project)
    .bind(event.target_user)
    .bind(event.details)
    .execute(conn)
    .await?;
    Ok(())
}

/// A recorded event.
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct Entry {
    pub id: i64,
    pub occurred_at: DateTime<Utc>,
    pub actor_id: Option<Uuid>,
    pub action: Action,
    pub target_user_id: Option<Uuid>,
    pub details: Value,
}

/// Which events of a project to read; every field narrows the result.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Filter {
    pub actor: Option<Uuid>,
    pub action: Option<Action>,
    /// Inclusive.
    pub from: Option<DateTime<Utc>>,
    /// Exclusive.
    pub until: Option<DateTime<Utc>>,
}

#[derive(Clone)]
pub struct AuditLog {
    pool: PgPool,
}

impl AuditLog {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// A project's events, newest first. The caller has checked the reader may
    /// see this project's log.
    pub async fn list(
        &self,
        project: Uuid,
        filter: &Filter,
        page: PageRequest,
    ) -> Result<Page<Entry>, sqlx::Error> {
        let items = sqlx::query_as(
            "SELECT id, occurred_at, actor_id, action, target_user_id, details FROM audit_events \
             WHERE project_id = $1 AND ($2::uuid IS NULL OR actor_id = $2) \
               AND ($3::text IS NULL OR action = $3) \
               AND ($4::timestamptz IS NULL OR occurred_at >= $4) \
               AND ($5::timestamptz IS NULL OR occurred_at < $5) \
             ORDER BY occurred_at DESC, id DESC LIMIT $6 OFFSET $7",
        )
        .bind(project)
        .bind(filter.actor)
        .bind(filter.action)
        .bind(filter.from)
        .bind(filter.until)
        .bind(page.limit)
        .bind(page.offset)
        .fetch_all(&self.pool)
        .await?;
        let (total,): (i64,) = sqlx::query_as(
            "SELECT count(*) FROM audit_events \
             WHERE project_id = $1 AND ($2::uuid IS NULL OR actor_id = $2) \
               AND ($3::text IS NULL OR action = $3) \
               AND ($4::timestamptz IS NULL OR occurred_at >= $4) \
               AND ($5::timestamptz IS NULL OR occurred_at < $5)",
        )
        .bind(project)
        .bind(filter.actor)
        .bind(filter.action)
        .bind(filter.from)
        .bind(filter.until)
        .fetch_one(&self.pool)
        .await?;
        Ok(Page { items, total })
    }

    /// Everything [`list`](Self::list) would return across pages, up to
    /// [`MAX_EXPORT_ROWS`], for a CSV export.
    pub async fn export(&self, project: Uuid, filter: &Filter) -> Result<Vec<Entry>, sqlx::Error> {
        let page = PageRequest {
            limit: MAX_EXPORT_ROWS,
            offset: 0,
        };
        Ok(self.list(project, filter, page).await?.items)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stored_names_match_the_serialized_ones() {
        for action in [
            Action::UserSignedIn,
            Action::UserSignInFailed,
            Action::UserSignedOut,
            Action::UserPasswordSet,
            Action::UserPasswordResetRequested,
            Action::UserInvited,
            Action::ProjectCreated,
            Action::ProjectUpdated,
            Action::ProjectArchived,
            Action::ProjectRestored,
            Action::MemberAdded,
            Action::MemberRoleChanged,
            Action::MemberRemoved,
            Action::DecisionCreated,
            Action::DecisionDeleted,
            Action::DecisionVersionSaved,
            Action::DecisionVersionRestored,
            Action::ScenarioCreated,
            Action::ScenarioUpdated,
            Action::ScenarioDeleted,
            Action::ReleaseCreated,
            Action::ReleaseDeployed,
            Action::TokenIssued,
            Action::TokenRevoked,
            Action::ReleaseRolledBack,
            Action::ApprovalRequested,
            Action::ApprovalApproved,
            Action::ApprovalRejected,
            Action::ApprovalWithdrawn,
        ] {
            assert_eq!(
                serde_json::to_value(action).unwrap(),
                Value::String(action.as_str().to_owned())
            );
        }
    }
}
