//! What the table checks find in a decision (DNK-76), and the findings an owner acknowledged.
//!
//! Findings warn and never block a save. An owner who knows a finding is intended (a reason table
//! that may give no reason) acknowledges it with why; the acknowledgement holds for as long as
//! the same finding comes back, on later versions too, and is audited.

use crate::{fetch, DecisionError, Decisions};
use chrono::{DateTime, Utc};
use donka_audit::{Action, Event};
use donka_engine::tables::{self, TableFinding};
use donka_project::{authorize_change, Access, Role};
use serde_json::{json, Value};
use std::collections::HashMap;
use uuid::Uuid;

pub const MAX_ACKNOWLEDGEMENT_CHARS: usize = 1000;

/// A table finding with the acknowledgement it has, if any.
#[derive(Debug, Clone)]
pub struct Finding {
    pub finding: TableFinding,
    pub acknowledgement: Option<Acknowledgement>,
}

#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct Acknowledgement {
    pub reason: String,
    pub acknowledged_by: Uuid,
    pub acknowledged_at: DateTime<Utc>,
}

impl Decisions {
    /// The table findings of the decision's draft (any member).
    pub async fn draft_findings(
        &self,
        access: &Access,
        id: Uuid,
    ) -> Result<Vec<Finding>, DecisionError> {
        let mut conn = self.pool.acquire().await?;
        let decision = fetch(&mut conn, access, id).await?;
        let acknowledged = acknowledgements(&mut conn, &[id]).await?;
        Ok(with_acknowledgements(id, &decision.content, &acknowledged))
    }

    /// The table findings of some contents of this project's decisions, each with its
    /// acknowledgement: what a release brings, for its approval review (any member).
    pub async fn findings_of(
        &self,
        access: &Access,
        contents: &[(Uuid, &Value)],
    ) -> Result<Vec<Vec<Finding>>, DecisionError> {
        let ids: Vec<Uuid> = contents.iter().map(|(id, _)| *id).collect();
        let mut conn = self.pool.acquire().await?;
        // Only this project's decisions: another project's acknowledgements stay unseen.
        let ids: Vec<Uuid> =
            sqlx::query_scalar("SELECT id FROM decisions WHERE project_id = $1 AND id = ANY($2)")
                .bind(access.project_id())
                .bind(&ids)
                .fetch_all(&mut *conn)
                .await?;
        let acknowledged = acknowledgements(&mut conn, &ids).await?;
        Ok(contents
            .iter()
            .map(|(id, content)| match ids.contains(id) {
                true => with_acknowledgements(*id, content, &acknowledged),
                false => Vec::new(),
            })
            .collect())
    }

    /// Acknowledges a finding of the draft with why it is intended (owners). Acknowledging it
    /// again replaces the reason.
    pub async fn acknowledge_finding(
        &self,
        access: &Access,
        id: Uuid,
        fingerprint: &str,
        reason: &str,
    ) -> Result<Finding, DecisionError> {
        let reason = reason.trim();
        if reason.is_empty() || reason.chars().count() > MAX_ACKNOWLEDGEMENT_CHARS {
            return Err(DecisionError::InvalidReason);
        }
        let now = self.clock.now();
        let mut tx = self.pool.begin().await?;
        authorize_change(&mut tx, access, Role::Owner).await?;
        let decision = fetch(&mut tx, access, id).await?;
        let finding = tables::check(&decision.content)
            .into_iter()
            .find(|finding| finding.fingerprint == fingerprint)
            .ok_or(DecisionError::FindingNotFound)?;
        let acknowledgement = Acknowledgement {
            reason: reason.to_owned(),
            acknowledged_by: access.user_id(),
            acknowledged_at: now,
        };
        sqlx::query(
            "INSERT INTO table_finding_acknowledgements \
             (decision_id, fingerprint, reason, acknowledged_by, acknowledged_at) \
             VALUES ($1, $2, $3, $4, $5) \
             ON CONFLICT (decision_id, fingerprint) DO UPDATE SET \
             reason = EXCLUDED.reason, acknowledged_by = EXCLUDED.acknowledged_by, \
             acknowledged_at = EXCLUDED.acknowledged_at",
        )
        .bind(id)
        .bind(fingerprint)
        .bind(&acknowledgement.reason)
        .bind(acknowledgement.acknowledged_by)
        .bind(acknowledgement.acknowledged_at)
        .execute(&mut *tx)
        .await?;
        donka_audit::record(
            &mut tx,
            Event::new(
                now,
                Some(access.user_id()),
                Action::DecisionFindingAcknowledged,
            )
            .in_project(access.project_id())
            .with_details(json!({
                "key": decision.summary.key,
                "table": finding.table_name,
                "kind": finding.kind.as_str(),
                "row": finding.row,
                "reason": acknowledgement.reason,
            })),
        )
        .await?;
        tx.commit().await?;
        Ok(Finding {
            finding,
            acknowledgement: Some(acknowledgement),
        })
    }
}

type Acknowledged = HashMap<(Uuid, String), Acknowledgement>;

/// The findings of `content` with the acknowledgements kept for decision `id`.
pub(crate) fn with_acknowledgements(
    id: Uuid,
    content: &Value,
    acknowledged: &Acknowledged,
) -> Vec<Finding> {
    tables::check(content)
        .into_iter()
        .map(|finding| Finding {
            acknowledgement: acknowledged
                .get(&(id, finding.fingerprint.clone()))
                .cloned(),
            finding,
        })
        .collect()
}

pub(crate) async fn acknowledgements(
    conn: &mut sqlx::PgConnection,
    ids: &[Uuid],
) -> Result<Acknowledged, DecisionError> {
    #[derive(sqlx::FromRow)]
    struct Row {
        decision_id: Uuid,
        fingerprint: String,
        #[sqlx(flatten)]
        acknowledgement: Acknowledgement,
    }
    let rows: Vec<Row> = sqlx::query_as(
        "SELECT decision_id, fingerprint, reason, acknowledged_by, acknowledged_at \
         FROM table_finding_acknowledgements WHERE decision_id = ANY($1)",
    )
    .bind(ids)
    .fetch_all(conn)
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| ((row.decision_id, row.fingerprint), row.acknowledgement))
        .collect())
}
