//! Production approvals (DNK-15).
//!
//! Production is published only through an approval. An editor asks for the
//! release live on staging to go to production; the project's owners are
//! emailed, and one of them, never the release's creator nor the person
//! asking, approves (which queues the production deployment) or rejects it
//! with a reason. One request per project waits at a time; the person who
//! asked can withdraw it. Deciding is a conditional update on a pending row,
//! so two owners acting at once produce one outcome.

use crate::{
    enqueue, load_deployment, sql, unique_or, Deployment, DeploymentReason, Environment, Queued,
    ReleaseError, ReleaseSummary, Releases, SemVer, SummaryRow, Tx, SUMMARY,
};
use chrono::{DateTime, Utc};
use donka_audit::{Action, Event};
use donka_decision::TestSummary;
use donka_mail::{Email, Mailer};
use donka_project::{authorize_change, Access, Role};
use donka_shared::page::{Page, PageRequest};
use serde_json::json;
use uuid::Uuid;

pub const MAX_REASON_CHARS: usize = 1000;
/// Emails handled per call to [`Releases::deliver_due_emails`].
const EMAIL_BATCH: usize = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApprovalStatus {
    Pending,
    Approved,
    Rejected,
    Withdrawn,
}

impl ApprovalStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            ApprovalStatus::Pending => "pending",
            ApprovalStatus::Approved => "approved",
            ApprovalStatus::Rejected => "rejected",
            ApprovalStatus::Withdrawn => "withdrawn",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "pending" => Some(ApprovalStatus::Pending),
            "approved" => Some(ApprovalStatus::Approved),
            "rejected" => Some(ApprovalStatus::Rejected),
            "withdrawn" => Some(ApprovalStatus::Withdrawn),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Approval {
    pub id: Uuid,
    pub release_id: Uuid,
    pub release_version: SemVer,
    pub release_notes: String,
    /// Who made the release: they cannot approve it.
    pub release_created_by: Uuid,
    pub requested_by: Uuid,
    pub requested_at: DateTime<Utc>,
    pub status: ApprovalStatus,
    pub decided_by: Option<Uuid>,
    pub decided_at: Option<DateTime<Utc>>,
    /// Why it was rejected.
    pub reason: Option<String>,
    /// The production deployment the approval queued.
    pub deployment_id: Option<Uuid>,
}

impl Approval {
    /// The release's creator and the person asking may not decide.
    pub fn is_author(&self, user: Uuid) -> bool {
        user == self.release_created_by || user == self.requested_by
    }
}

/// How a decision differs between production and the release asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Change {
    Added,
    Changed,
    Removed,
    Unchanged,
}

impl Change {
    pub fn as_str(self) -> &'static str {
        match self {
            Change::Added => "added",
            Change::Changed => "changed",
            Change::Removed => "removed",
            Change::Unchanged => "unchanged",
        }
    }
}

#[derive(Debug, Clone)]
pub struct DecisionChange {
    pub key: String,
    pub decision_id: Uuid,
    pub change: Change,
    /// The version live in production, if any.
    pub from_version: Option<i32>,
    /// The version the release brings, if any.
    pub to_version: Option<i32>,
    /// The scenarios' results on the version the release brings.
    pub tests: Option<TestSummary>,
}

/// What an approver looks at: the request, what production runs now and
/// what would change, with the test results and the release notes.
#[derive(Debug, Clone)]
pub struct ApprovalReview {
    pub approval: Approval,
    /// The release live in production now.
    pub production: Option<SemVer>,
    pub changes: Vec<DecisionChange>,
    pub tests: TestSummary,
}

/// The language an approval email is written in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    En,
    Fr,
}

impl Language {
    fn as_str(self) -> &'static str {
        match self {
            Language::En => "en",
            Language::Fr => "fr",
        }
    }
}

/// An owner who may be asked to approve.
#[derive(Debug, Clone)]
pub struct Approver {
    pub user_id: Uuid,
    pub email: String,
    pub language: Language,
}

#[derive(sqlx::FromRow)]
struct ApprovalRow {
    id: Uuid,
    release_id: Uuid,
    #[sqlx(flatten)]
    version: SemVer,
    notes: String,
    release_created_by: Uuid,
    requested_by: Uuid,
    requested_at: DateTime<Utc>,
    status: String,
    decided_by: Option<Uuid>,
    decided_at: Option<DateTime<Utc>>,
    reason: Option<String>,
    deployment_id: Option<Uuid>,
}

impl From<ApprovalRow> for Approval {
    fn from(row: ApprovalRow) -> Self {
        Self {
            id: row.id,
            release_id: row.release_id,
            release_version: row.version,
            release_notes: row.notes,
            release_created_by: row.release_created_by,
            requested_by: row.requested_by,
            requested_at: row.requested_at,
            // The table only holds the four statuses (CHECK constraint).
            status: ApprovalStatus::parse(&row.status).unwrap_or(ApprovalStatus::Pending),
            decided_by: row.decided_by,
            decided_at: row.decided_at,
            reason: row.reason,
            deployment_id: row.deployment_id,
        }
    }
}

/// An approval with its release (`$1` is the project).
const APPROVALS: &str = "SELECT a.id, a.release_id, r.major, r.minor, r.patch, r.notes, \
     r.created_by AS release_created_by, a.requested_by, a.requested_at, a.status, a.decided_by, \
     a.decided_at, a.reason, a.deployment_id \
     FROM approvals a JOIN releases r ON r.id = a.release_id WHERE a.project_id = $1";

#[derive(sqlx::FromRow)]
struct FrozenRow {
    decision_id: Uuid,
    key: String,
    version_number: i32,
    #[sqlx(flatten)]
    tests: TestSummary,
}

#[derive(sqlx::FromRow)]
struct DueEmail {
    id: Uuid,
    recipient: String,
    subject: String,
    body: String,
    attempts: i32,
}

impl Releases {
    /// Asks for the release live on staging to go to production (editors).
    /// `owners` are the project's owners; those who may decide are emailed.
    pub async fn request_approval(
        &self,
        access: &Access,
        release_id: Uuid,
        owners: Vec<Approver>,
    ) -> Result<Approval, ReleaseError> {
        let project = self.projects.get(access).await?;
        let now = self.clock.now();
        let mut tx = self.pool.begin().await?;
        authorize_change(&mut tx, access, Role::Editor).await?;
        let release: Option<(SemVer, Uuid)> = sqlx::query_as(
            "SELECT major, minor, patch, created_by FROM releases WHERE project_id = $1 AND id = $2",
        )
        .bind(access.project_id())
        .bind(release_id)
        .fetch_optional(&mut *tx)
        .await?
        .map(|(major, minor, patch, created_by)| (SemVer { major, minor, patch }, created_by));
        let (version, created_by) = release.ok_or(ReleaseError::NotFound)?;
        if live_release(&mut tx, access.project_id(), Environment::Staging).await?
            != Some(release_id)
        {
            return Err(ReleaseError::NotOnStaging);
        }
        let approvers: Vec<Approver> = owners
            .into_iter()
            .filter(|owner| owner.user_id != created_by && owner.user_id != access.user_id())
            .collect();
        if approvers.is_empty() {
            return Err(ReleaseError::NoApprover);
        }

        let id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO approvals (id, project_id, release_id, requested_by, requested_at, status) \
             VALUES ($1, $2, $3, $4, $5, 'pending')",
        )
        .bind(id)
        .bind(access.project_id())
        .bind(release_id)
        .bind(access.user_id())
        .bind(now)
        .execute(&mut *tx)
        .await
        .map_err(|err| unique_or(err, ReleaseError::ApprovalPending))?;
        for approver in &approvers {
            let link = format!(
                "{}/{}/projects/approval/?p={}&a={id}",
                self.settings.public_url.trim_end_matches('/'),
                approver.language.as_str(),
                project.key,
            );
            let email = crate::emails::approval_requested(
                approver.language,
                &project.name,
                &version.to_string(),
                &link,
            );
            sqlx::query(
                "INSERT INTO approval_emails (id, approval_id, recipient, subject, body, created_at, \
                 next_attempt_at) VALUES ($1, $2, $3, $4, $5, $6, $6)",
            )
            .bind(Uuid::new_v4())
            .bind(id)
            .bind(&approver.email)
            .bind(&email.subject)
            .bind(&email.body)
            .bind(now)
            .execute(&mut *tx)
            .await?;
        }
        donka_audit::record(
            &mut tx,
            Event::new(now, Some(access.user_id()), Action::ApprovalRequested)
                .in_project(access.project_id())
                .with_details(json!({ "version": version.to_string() })),
        )
        .await?;
        let approval = load(&mut tx, access.project_id(), id).await?;
        tx.commit().await?;
        self.emails.notify_one();
        Ok(approval)
    }

    /// The project's approval requests, newest first (any member).
    pub async fn approvals(
        &self,
        access: &Access,
        page: PageRequest,
    ) -> Result<Page<Approval>, ReleaseError> {
        let rows: Vec<ApprovalRow> = sqlx::query_as(sql(format!(
            "{APPROVALS} ORDER BY a.seq DESC LIMIT $2 OFFSET $3"
        )))
        .bind(access.project_id())
        .bind(page.limit)
        .bind(page.offset)
        .fetch_all(&self.pool)
        .await?;
        let (total,): (i64,) =
            sqlx::query_as("SELECT count(*) FROM approvals WHERE project_id = $1")
                .bind(access.project_id())
                .fetch_one(&self.pool)
                .await?;
        Ok(Page {
            items: rows.into_iter().map(Approval::from).collect(),
            total,
        })
    }

    /// One request with what it would change in production (any member).
    pub async fn approval(
        &self,
        access: &Access,
        id: Uuid,
    ) -> Result<ApprovalReview, ReleaseError> {
        let mut conn = self.pool.acquire().await?;
        let approval = load(&mut conn, access.project_id(), id).await?;
        // Compared with what production ran before the request's own deployment,
        // when it was decided, or now.
        let production = production_before(&mut conn, access.project_id(), &approval).await?;
        let production_version: Option<SemVer> = match production {
            Some(release) => {
                sqlx::query_as("SELECT major, minor, patch FROM releases WHERE id = $1")
                    .bind(release)
                    .fetch_optional(&mut *conn)
                    .await?
            }
            None => None,
        };
        let from = match production {
            Some(release) => frozen(&mut conn, release).await?,
            None => Vec::new(),
        };
        let to = frozen(&mut conn, approval.release_id).await?;
        let changes = compare(&from, &to);
        let tests = to
            .iter()
            .fold(TestSummary::default(), |sum, row| TestSummary {
                passed: sum.passed + row.tests.passed,
                failed: sum.failed + row.tests.failed,
                errors: sum.errors + row.tests.errors,
            });
        Ok(ApprovalReview {
            approval,
            production: production_version,
            changes,
            tests,
        })
    }

    /// Approves a request and queues the production deployment (owners who
    /// neither made the release nor asked).
    pub async fn approve(&self, access: &Access, id: Uuid) -> Result<Approval, ReleaseError> {
        let project = self.projects.get(access).await?;
        let now = self.clock.now();
        let mut tx = self.pool.begin().await?;
        authorize_change(&mut tx, access, Role::Owner).await?;
        let current = lock_pending(&mut tx, access.project_id(), id).await?;
        if current.is_author(access.user_id()) {
            return Err(ReleaseError::SelfApproval);
        }
        let deployment = enqueue(
            &mut tx,
            access,
            &project,
            Queued {
                environment: Environment::Production,
                release_id: current.release_id,
                reason: DeploymentReason::Deploy,
                rollback_reason: None,
            },
            now,
        )
        .await?;
        let outcome = Outcome::Approved(deployment);
        decide(&mut tx, access, id, outcome, now).await?;
        let version = current.release_version.to_string();
        for action in [Action::ApprovalApproved, Action::ReleaseDeployed] {
            donka_audit::record(
                &mut tx,
                Event::new(now, Some(access.user_id()), action)
                    .in_project(access.project_id())
                    .with_details(json!({ "version": version, "environment": "production" })),
            )
            .await?;
        }
        let approval = load(&mut tx, access.project_id(), id).await?;
        tx.commit().await?;
        self.outbox.notify_one();
        Ok(approval)
    }

    /// Rejects a request with a reason (owners who neither made the release nor asked).
    pub async fn reject(
        &self,
        access: &Access,
        id: Uuid,
        reason: &str,
    ) -> Result<Approval, ReleaseError> {
        let reason = reason.trim();
        if reason.is_empty() || reason.chars().count() > MAX_REASON_CHARS {
            return Err(ReleaseError::InvalidReason);
        }
        let now = self.clock.now();
        let mut tx = self.pool.begin().await?;
        authorize_change(&mut tx, access, Role::Owner).await?;
        let current = lock_pending(&mut tx, access.project_id(), id).await?;
        if current.is_author(access.user_id()) {
            return Err(ReleaseError::SelfApproval);
        }
        decide(&mut tx, access, id, Outcome::Rejected(reason), now).await?;
        donka_audit::record(
            &mut tx,
            Event::new(now, Some(access.user_id()), Action::ApprovalRejected)
                .in_project(access.project_id())
                .with_details(json!({
                    "version": current.release_version.to_string(),
                    "reason": reason,
                })),
        )
        .await?;
        let approval = load(&mut tx, access.project_id(), id).await?;
        tx.commit().await?;
        Ok(approval)
    }

    /// Withdraws a request (the person who asked).
    pub async fn withdraw(&self, access: &Access, id: Uuid) -> Result<Approval, ReleaseError> {
        let now = self.clock.now();
        let mut tx = self.pool.begin().await?;
        authorize_change(&mut tx, access, Role::Editor).await?;
        let current = lock_pending(&mut tx, access.project_id(), id).await?;
        if current.requested_by != access.user_id() {
            return Err(ReleaseError::NotRequester);
        }
        decide(&mut tx, access, id, Outcome::Withdrawn, now).await?;
        donka_audit::record(
            &mut tx,
            Event::new(now, Some(access.user_id()), Action::ApprovalWithdrawn)
                .in_project(access.project_id())
                .with_details(json!({ "version": current.release_version.to_string() })),
        )
        .await?;
        let approval = load(&mut tx, access.project_id(), id).await?;
        tx.commit().await?;
        Ok(approval)
    }

    /// The releases production can be rolled back to: each one approved for
    /// production once, except the one live now, newest first (any member).
    pub async fn rollback_targets(
        &self,
        access: &Access,
    ) -> Result<Vec<ReleaseSummary>, ReleaseError> {
        let mut conn = self.pool.acquire().await?;
        let live = live_release(&mut conn, access.project_id(), Environment::Production).await?;
        let rows: Vec<SummaryRow> = sqlx::query_as(sql(format!(
            "{SUMMARY} AND EXISTS (SELECT 1 FROM approvals a WHERE a.release_id = r.id \
             AND a.status = 'approved') AND r.id IS DISTINCT FROM $2 \
             ORDER BY r.major DESC, r.minor DESC, r.patch DESC"
        )))
        .bind(access.project_id())
        .bind(live)
        .fetch_all(&mut *conn)
        .await?;
        Ok(rows.into_iter().map(ReleaseSummary::from).collect())
    }

    /// Puts a release once approved for production back in production, with
    /// a reason and without a new approval (owners). A request waiting for
    /// approval stays as it is.
    pub async fn rollback(
        &self,
        access: &Access,
        release_id: Uuid,
        reason: &str,
    ) -> Result<Deployment, ReleaseError> {
        let reason = reason.trim();
        if reason.is_empty() || reason.chars().count() > MAX_REASON_CHARS {
            return Err(ReleaseError::InvalidReason);
        }
        let project = self.projects.get(access).await?;
        let now = self.clock.now();
        let mut tx = self.pool.begin().await?;
        authorize_change(&mut tx, access, Role::Owner).await?;
        let target: Option<SemVer> = sqlx::query_as(
            "SELECT major, minor, patch FROM releases WHERE project_id = $1 AND id = $2",
        )
        .bind(access.project_id())
        .bind(release_id)
        .fetch_optional(&mut *tx)
        .await?;
        let target = target.ok_or(ReleaseError::NotFound)?;
        let (approved,): (bool,) = sqlx::query_as(
            "SELECT EXISTS (SELECT 1 FROM approvals WHERE project_id = $1 AND release_id = $2 \
             AND status = 'approved')",
        )
        .bind(access.project_id())
        .bind(release_id)
        .fetch_one(&mut *tx)
        .await?;
        if !approved {
            return Err(ReleaseError::NeverApproved);
        }
        let live = live_release(&mut tx, access.project_id(), Environment::Production).await?;
        if live == Some(release_id) {
            return Err(ReleaseError::AlreadyLive);
        }
        let from: Option<SemVer> = match live {
            Some(live) => {
                sqlx::query_as("SELECT major, minor, patch FROM releases WHERE id = $1")
                    .bind(live)
                    .fetch_optional(&mut *tx)
                    .await?
            }
            None => None,
        };
        let id = enqueue(
            &mut tx,
            access,
            &project,
            Queued {
                environment: Environment::Production,
                release_id,
                reason: DeploymentReason::Rollback,
                rollback_reason: Some(reason),
            },
            now,
        )
        .await?;
        donka_audit::record(
            &mut tx,
            Event::new(now, Some(access.user_id()), Action::ReleaseRolledBack)
                .in_project(access.project_id())
                .with_details(json!({
                    "from": from.map(|v| v.to_string()),
                    "version": target.to_string(),
                    "reason": reason,
                })),
        )
        .await?;
        let deployment =
            load_deployment(&mut tx, access.project_id(), Environment::Production, id).await?;
        tx.commit().await?;
        self.outbox.notify_one();
        Ok(deployment)
    }

    /// Resolves when an approval email has been queued since the last call.
    pub async fn email_queued(&self) {
        self.emails.notified().await;
    }

    /// Sends the approval emails that are due, each in its own transaction,
    /// and returns how many were sent. Safe to run from several instances.
    pub async fn deliver_due_emails(&self, mailer: &dyn Mailer) -> Result<usize, ReleaseError> {
        let mut sent = 0;
        for _ in 0..EMAIL_BATCH {
            let now = self.clock.now();
            let mut tx = self.pool.begin().await?;
            let due: Option<DueEmail> = sqlx::query_as(
                "SELECT id, recipient, subject, body, attempts FROM approval_emails \
                 WHERE sent_at IS NULL AND abandoned_at IS NULL AND next_attempt_at <= $1 \
                 ORDER BY next_attempt_at LIMIT 1 FOR UPDATE SKIP LOCKED",
            )
            .bind(now)
            .fetch_optional(&mut *tx)
            .await?;
            let Some(due) = due else { break };
            let attempts = due.attempts.saturating_add(1);
            let email = Email {
                to: due.recipient,
                subject: due.subject,
                text: due.body,
            };
            match mailer.send(&email).await {
                Ok(()) => {
                    sqlx::query(
                        "UPDATE approval_emails SET sent_at = $2, attempts = $3, last_error = NULL \
                         WHERE id = $1",
                    )
                    .bind(due.id)
                    .bind(now)
                    .bind(attempts)
                    .execute(&mut *tx)
                    .await?;
                    sent += 1;
                }
                Err(err) => {
                    let max = i32::try_from(self.settings.email_max_attempts).unwrap_or(i32::MAX);
                    let abandoned_at = (err.is_permanent() || attempts >= max).then_some(now);
                    if abandoned_at.is_some() {
                        tracing::error!(email_id = %due.id, %err, attempts, "approval email abandoned");
                    } else {
                        tracing::warn!(email_id = %due.id, %err, attempts, "approval email not sent; will retry");
                    }
                    sqlx::query(
                        "UPDATE approval_emails SET attempts = $2, last_error = $3, \
                         next_attempt_at = $4, abandoned_at = $5 WHERE id = $1",
                    )
                    .bind(due.id)
                    .bind(attempts)
                    .bind(err.to_string())
                    .bind(now + crate::retry_delay(attempts))
                    .bind(abandoned_at)
                    .execute(&mut *tx)
                    .await?;
                }
            }
            tx.commit().await?;
        }
        Ok(sent)
    }
}

async fn load(
    conn: &mut sqlx::PgConnection,
    project_id: Uuid,
    id: Uuid,
) -> Result<Approval, ReleaseError> {
    let row: Option<ApprovalRow> = sqlx::query_as(sql(format!("{APPROVALS} AND a.id = $2")))
        .bind(project_id)
        .bind(id)
        .fetch_optional(conn)
        .await?;
    row.map(Approval::from)
        .ok_or(ReleaseError::ApprovalNotFound)
}

/// How a pending request ends.
enum Outcome<'a> {
    /// With the production deployment it queued.
    Approved(Uuid),
    Rejected(&'a str),
    Withdrawn,
}

/// A request that is still pending, locked until the transaction ends: a
/// second owner deciding at the same moment waits, then finds it decided.
async fn lock_pending(
    tx: &mut Tx<'_>,
    project_id: Uuid,
    id: Uuid,
) -> Result<Approval, ReleaseError> {
    let row: Option<ApprovalRow> =
        sqlx::query_as(sql(format!("{APPROVALS} AND a.id = $2 FOR UPDATE OF a")))
            .bind(project_id)
            .bind(id)
            .fetch_optional(&mut **tx)
            .await?;
    let approval = row
        .map(Approval::from)
        .ok_or(ReleaseError::ApprovalNotFound)?;
    if approval.status != ApprovalStatus::Pending {
        return Err(ReleaseError::AlreadyDecided);
    }
    Ok(approval)
}

/// Records how a request locked by [`lock_pending`] ended.
async fn decide(
    tx: &mut Tx<'_>,
    access: &Access,
    id: Uuid,
    outcome: Outcome<'_>,
    now: DateTime<Utc>,
) -> Result<(), ReleaseError> {
    let (status, reason, deployment) = match outcome {
        Outcome::Approved(deployment) => (ApprovalStatus::Approved, None, Some(deployment)),
        Outcome::Rejected(reason) => (ApprovalStatus::Rejected, Some(reason), None),
        Outcome::Withdrawn => (ApprovalStatus::Withdrawn, None, None),
    };
    sqlx::query(
        "UPDATE approvals SET status = $2, decided_by = $3, decided_at = $4, reason = $5, \
         deployment_id = $6 WHERE id = $1",
    )
    .bind(id)
    .bind(status.as_str())
    .bind(access.user_id())
    .bind(now)
    .bind(reason)
    .bind(deployment)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// The release an environment runs now.
async fn live_release(
    conn: &mut sqlx::PgConnection,
    project_id: Uuid,
    environment: Environment,
) -> Result<Option<Uuid>, ReleaseError> {
    let row: Option<(Uuid,)> = sqlx::query_as(
        "SELECT release_id FROM deployments WHERE project_id = $1 AND environment = $2 \
         AND published_at IS NOT NULL ORDER BY published_at DESC LIMIT 1",
    )
    .bind(project_id)
    .bind(environment.as_str())
    .fetch_optional(conn)
    .await?;
    Ok(row.map(|(id,)| id))
}

/// The release production ran before a request's outcome: before the
/// deployment an approval queued, when a rejection or withdrawal happened, or
/// now while the request waits.
async fn production_before(
    conn: &mut sqlx::PgConnection,
    project_id: Uuid,
    approval: &Approval,
) -> Result<Option<Uuid>, ReleaseError> {
    let row: Option<(Uuid,)> = sqlx::query_as(
        "SELECT release_id FROM deployments WHERE project_id = $1 AND environment = 'production' \
         AND published_at IS NOT NULL \
         AND ($2::uuid IS NULL OR seq < (SELECT seq FROM deployments WHERE id = $2)) \
         AND ($2::uuid IS NOT NULL OR $3::timestamptz IS NULL OR published_at <= $3) \
         ORDER BY seq DESC LIMIT 1",
    )
    .bind(project_id)
    .bind(approval.deployment_id)
    .bind(approval.decided_at)
    .fetch_optional(conn)
    .await?;
    Ok(row.map(|(id,)| id))
}

async fn frozen(
    conn: &mut sqlx::PgConnection,
    release_id: Uuid,
) -> Result<Vec<FrozenRow>, ReleaseError> {
    Ok(sqlx::query_as(
        "SELECT decision_id, key, version_number, tests_passed AS passed, tests_failed AS failed, \
         tests_errors AS errors FROM release_decisions WHERE release_id = $1 ORDER BY key",
    )
    .bind(release_id)
    .fetch_all(conn)
    .await?)
}

/// Decisions changed first, then added, removed and unchanged, each by key.
fn compare(from: &[FrozenRow], to: &[FrozenRow]) -> Vec<DecisionChange> {
    let mut changes: Vec<DecisionChange> = to
        .iter()
        .map(|new| {
            let old = from.iter().find(|old| old.decision_id == new.decision_id);
            DecisionChange {
                key: new.key.clone(),
                decision_id: new.decision_id,
                change: match old {
                    None => Change::Added,
                    Some(old) if old.version_number != new.version_number => Change::Changed,
                    Some(_) => Change::Unchanged,
                },
                from_version: old.map(|old| old.version_number),
                to_version: Some(new.version_number),
                tests: Some(new.tests),
            }
        })
        .collect();
    changes.extend(
        from.iter()
            .filter(|old| !to.iter().any(|new| new.decision_id == old.decision_id))
            .map(|old| DecisionChange {
                key: old.key.clone(),
                decision_id: old.decision_id,
                change: Change::Removed,
                from_version: Some(old.version_number),
                to_version: None,
                tests: None,
            }),
    );
    let rank = |change: Change| match change {
        Change::Changed => 0,
        Change::Added => 1,
        Change::Removed => 2,
        Change::Unchanged => 3,
    };
    changes.sort_by(|a, b| rank(a.change).cmp(&rank(b.change)).then(a.key.cmp(&b.key)));
    changes
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(key: &str, id: u128, version: i32) -> FrozenRow {
        FrozenRow {
            decision_id: Uuid::from_u128(id),
            key: key.into(),
            version_number: version,
            tests: TestSummary::default(),
        }
    }

    #[test]
    fn compares_production_with_the_release() {
        let from = [row("a", 1, 1), row("b", 2, 3), row("gone", 3, 1)];
        let to = [row("a", 1, 2), row("b", 2, 3), row("new", 4, 1)];
        let changes: Vec<(String, Change, Option<i32>, Option<i32>)> = compare(&from, &to)
            .into_iter()
            .map(|c| (c.key, c.change, c.from_version, c.to_version))
            .collect();
        assert_eq!(
            changes,
            vec![
                ("a".into(), Change::Changed, Some(1), Some(2)),
                ("new".into(), Change::Added, None, Some(1)),
                ("gone".into(), Change::Removed, Some(1), None),
                ("b".into(), Change::Unchanged, Some(3), Some(3)),
            ]
        );
    }

    #[test]
    fn a_renamed_decision_is_the_same_decision() {
        let changes = compare(&[row("old-name", 1, 1)], &[row("new-name", 1, 2)]);
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].change, Change::Changed);
    }

    #[test]
    fn statuses_read_and_write() {
        for status in [
            ApprovalStatus::Pending,
            ApprovalStatus::Approved,
            ApprovalStatus::Rejected,
            ApprovalStatus::Withdrawn,
        ] {
            assert_eq!(ApprovalStatus::parse(status.as_str()), Some(status));
        }
    }
}
