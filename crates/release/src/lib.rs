//! Release module: releases, the two environments of a project, Runtime
//! tokens and deployments.
//!
//! A release freezes the latest version of every decision of a project, with
//! a semantic version and notes, and copies them so it never changes. Each
//! project has two environments, staging and production. Deploying a release
//! to an environment queues a deployment; the publisher writes the artifact
//! (`artifact`) to `<environment>/<project-key>` in the bucket after commit,
//! retrying a failed write and showing why it failed. Runtime tokens are
//! issued per environment, shown once and kept only as hashes; issuing or
//! revoking one publishes the environment's release again with the new hashes.
//! Production deployments need a second person's approval (DNK-15).

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use chrono::{DateTime, Duration, Utc};
use donka_audit::{Action, Event};
use donka_db::PgPool;
use donka_decision::{DecisionError, Decisions, TestSummary};
use donka_project::{authorize_change, Access, ProjectError, Projects, Role};
use donka_shared::clock::Clock;
use donka_shared::page::{Page, PageRequest};
use donka_storage::ArtifactStore;
use serde_json::json;
use std::fmt;
use std::sync::Arc;
use tokio::sync::Notify;
use uuid::Uuid;

pub mod artifact;

pub const MAX_NOTES_CHARS: usize = 2000;
pub const MAX_TOKEN_NAME_CHARS: usize = 100;
/// Deployments handled per call to [`Releases::publish_due`].
const PUBLISH_BATCH: usize = 20;
/// Retry delays double from this value after each failed write...
const RETRY_BASE_SECONDS: i64 = 30;
/// ...up to this ceiling.
const RETRY_MAX_SECONDS: i64 = 3600;
/// Random bytes in a Runtime token (docs/artifact-format.md: at least 32).
const TOKEN_BYTES: usize = 32;
/// Every Runtime token starts with this, so a leaked one is easy to recognise.
const TOKEN_PREFIX: &str = "dnk_";

/// The two environments every project has.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Environment {
    Staging,
    Production,
}

impl Environment {
    pub const ALL: [Environment; 2] = [Environment::Staging, Environment::Production];

    pub fn as_str(self) -> &'static str {
        match self {
            Environment::Staging => "staging",
            Environment::Production => "production",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "staging" => Some(Environment::Staging),
            "production" => Some(Environment::Production),
            _ => None,
        }
    }
}

/// Which part of the version a new release raises.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bump {
    Major,
    Minor,
    Patch,
}

/// A release's semantic version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, sqlx::FromRow)]
pub struct SemVer {
    pub major: i32,
    pub minor: i32,
    pub patch: i32,
}

impl SemVer {
    pub const FIRST: SemVer = SemVer {
        major: 1,
        minor: 0,
        patch: 0,
    };

    /// The version after `latest` (the first release is 1.0.0, whatever the bump).
    pub fn next(latest: Option<SemVer>, bump: Bump) -> SemVer {
        let Some(v) = latest else { return Self::FIRST };
        match bump {
            Bump::Major => SemVer {
                major: v.major + 1,
                minor: 0,
                patch: 0,
            },
            Bump::Minor => SemVer {
                minor: v.minor + 1,
                patch: 0,
                ..v
            },
            Bump::Patch => SemVer {
                patch: v.patch + 1,
                ..v
            },
        }
    }
}

impl fmt::Display for SemVer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ReleaseError {
    /// No such release in this project (also the answer for another project's).
    #[error("release not found")]
    NotFound,
    #[error("deployment not found")]
    DeploymentNotFound,
    #[error("token not found")]
    TokenNotFound,
    /// These decisions have no saved version yet: a release must hold every decision.
    #[error("decisions without a version: {}", .0.join(", "))]
    Unversioned(Vec<String>),
    #[error("the project has no decision to release")]
    NothingToRelease,
    #[error("notes are 1 to {MAX_NOTES_CHARS} characters")]
    InvalidNotes,
    #[error("token names are 1 to {MAX_TOKEN_NAME_CHARS} characters")]
    InvalidTokenName,
    /// Production is published through an approval (DNK-15), not deployed directly.
    #[error("production needs a second person's approval")]
    ApprovalRequired,
    /// Someone created a release with this version at the same moment.
    #[error("release {0} was just created by someone else")]
    VersionTaken(SemVer),
    /// Only a deployment that gave up can be retried.
    #[error("this deployment is not waiting for a retry")]
    NotRetryable,
    #[error("random token could not be generated")]
    Random,
    #[error(transparent)]
    Project(#[from] ProjectError),
    #[error(transparent)]
    Decision(#[from] DecisionError),
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

/// A release as lists show it.
#[derive(Debug, Clone)]
pub struct ReleaseSummary {
    pub id: Uuid,
    pub version: SemVer,
    pub notes: String,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub decisions: i64,
    /// The scenarios' results on the frozen versions, added up.
    pub tests: TestSummary,
    /// The environments this release is live in now.
    pub live_in: Vec<Environment>,
}

/// A decision as a release froze it.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ReleasedDecision {
    pub decision_id: Uuid,
    pub key: String,
    pub version: i32,
    #[sqlx(flatten)]
    pub tests: TestSummary,
}

#[derive(Debug, Clone)]
pub struct Release {
    pub summary: ReleaseSummary,
    pub decisions: Vec<ReleasedDecision>,
}

/// What a new release would be, for the person about to make one.
#[derive(Debug, Clone)]
pub struct ReleasePreview {
    pub latest: Option<SemVer>,
    pub decisions: Vec<ReleasedDecision>,
    /// Decisions without a version: while any remain, no release can be made.
    pub unversioned: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeploymentStatus {
    /// Waiting for the publisher.
    Pending,
    /// A write failed; it will be tried again at `next_attempt_at`.
    Retrying,
    /// Gave up after too many failed writes; it can be retried by hand.
    Failed,
    Published,
    /// A newer deployment of the environment replaced it before it was published.
    Superseded,
}

impl DeploymentStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            DeploymentStatus::Pending => "pending",
            DeploymentStatus::Retrying => "retrying",
            DeploymentStatus::Failed => "failed",
            DeploymentStatus::Published => "published",
            DeploymentStatus::Superseded => "superseded",
        }
    }
}

/// Why a deployment exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeploymentReason {
    /// Someone deployed the release.
    Deploy,
    /// The environment's tokens changed; its release is published again.
    Tokens,
}

impl DeploymentReason {
    pub fn as_str(self) -> &'static str {
        match self {
            DeploymentReason::Deploy => "deploy",
            DeploymentReason::Tokens => "tokens",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Deployment {
    pub id: Uuid,
    pub environment: Environment,
    pub release_id: Uuid,
    pub release_version: SemVer,
    pub reason: DeploymentReason,
    pub requested_by: Uuid,
    pub requested_at: DateTime<Utc>,
    pub status: DeploymentStatus,
    pub attempts: i32,
    pub last_error: Option<String>,
    pub next_attempt_at: Option<DateTime<Utc>>,
    pub published_at: Option<DateTime<Utc>>,
}

/// One environment of a project: what is live, the latest deployment asked
/// for (which may still be on its way), and how many tokens open it.
#[derive(Debug, Clone)]
pub struct EnvironmentState {
    pub environment: Environment,
    pub live: Option<Deployment>,
    pub latest: Option<Deployment>,
    pub tokens: i64,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct RuntimeToken {
    pub id: Uuid,
    pub name: String,
    /// The token's last characters, to tell tokens apart.
    pub hint: String,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub revoked_by: Option<Uuid>,
    pub revoked_at: Option<DateTime<Utc>>,
}

/// A token just issued: the only time its value exists outside the customer's system.
#[derive(Debug, Clone)]
pub struct IssuedToken {
    pub token: String,
    pub details: RuntimeToken,
}

#[derive(Clone)]
pub struct Releases {
    pool: PgPool,
    clock: Arc<dyn Clock>,
    decisions: Decisions,
    projects: Projects,
    store: Arc<dyn ArtifactStore>,
    /// Failed writes before a deployment is given up.
    max_attempts: u32,
    outbox: Arc<Notify>,
}

type Tx<'a> = sqlx::Transaction<'a, sqlx::Postgres>;

#[derive(sqlx::FromRow)]
struct SummaryRow {
    id: Uuid,
    #[sqlx(flatten)]
    version: SemVer,
    notes: String,
    created_by: Uuid,
    created_at: DateTime<Utc>,
    decisions: i64,
    #[sqlx(flatten)]
    tests: TestSummary,
    live_in: Vec<String>,
}

impl From<SummaryRow> for ReleaseSummary {
    fn from(row: SummaryRow) -> Self {
        Self {
            id: row.id,
            version: row.version,
            notes: row.notes,
            created_by: row.created_by,
            created_at: row.created_at,
            decisions: row.decisions,
            tests: row.tests,
            live_in: row
                .live_in
                .iter()
                .filter_map(|env| Environment::parse(env))
                .collect(),
        }
    }
}

/// A release with its counts and where it is live (`$1` is the project).
const SUMMARY: &str = "SELECT r.id, r.major, r.minor, r.patch, r.notes, r.created_by, r.created_at, \
     (SELECT count(*) FROM release_decisions rd WHERE rd.release_id = r.id) AS decisions, \
     (SELECT coalesce(sum(tests_passed), 0)::bigint FROM release_decisions rd WHERE rd.release_id = r.id) AS passed, \
     (SELECT coalesce(sum(tests_failed), 0)::bigint FROM release_decisions rd WHERE rd.release_id = r.id) AS failed, \
     (SELECT coalesce(sum(tests_errors), 0)::bigint FROM release_decisions rd WHERE rd.release_id = r.id) AS errors, \
     ARRAY(SELECT live.environment FROM (SELECT DISTINCT ON (environment) environment, release_id \
       FROM deployments WHERE project_id = r.project_id AND published_at IS NOT NULL \
       ORDER BY environment, published_at DESC) live WHERE live.release_id = r.id ORDER BY live.environment DESC) AS live_in \
     FROM releases r WHERE r.project_id = $1";

#[derive(sqlx::FromRow)]
struct DeploymentRow {
    id: Uuid,
    environment: String,
    release_id: Uuid,
    #[sqlx(flatten)]
    version: SemVer,
    reason: String,
    requested_by: Uuid,
    requested_at: DateTime<Utc>,
    attempts: i32,
    last_error: Option<String>,
    next_attempt_at: DateTime<Utc>,
    published_at: Option<DateTime<Utc>>,
    abandoned_at: Option<DateTime<Utc>>,
    superseded_at: Option<DateTime<Utc>>,
}

impl From<DeploymentRow> for Deployment {
    fn from(row: DeploymentRow) -> Self {
        let status = if row.published_at.is_some() {
            DeploymentStatus::Published
        } else if row.superseded_at.is_some() {
            DeploymentStatus::Superseded
        } else if row.abandoned_at.is_some() {
            DeploymentStatus::Failed
        } else if row.attempts > 0 {
            DeploymentStatus::Retrying
        } else {
            DeploymentStatus::Pending
        };
        let waiting = matches!(
            status,
            DeploymentStatus::Pending | DeploymentStatus::Retrying
        );
        Self {
            id: row.id,
            // The table only holds the two environments (CHECK constraint).
            environment: Environment::parse(&row.environment).unwrap_or(Environment::Staging),
            release_id: row.release_id,
            release_version: row.version,
            reason: if row.reason == "tokens" {
                DeploymentReason::Tokens
            } else {
                DeploymentReason::Deploy
            },
            requested_by: row.requested_by,
            requested_at: row.requested_at,
            status,
            attempts: row.attempts,
            last_error: row.last_error,
            next_attempt_at: waiting.then_some(row.next_attempt_at),
            published_at: row.published_at,
        }
    }
}

/// A deployment with its release's version (`$1` is the project, `$2` the environment).
const DEPLOYMENTS: &str =
    "SELECT d.id, d.environment, d.release_id, r.major, r.minor, r.patch, d.reason, \
     d.requested_by, d.requested_at, d.attempts, d.last_error, d.next_attempt_at, d.published_at, \
     d.abandoned_at, d.superseded_at \
     FROM deployments d JOIN releases r ON r.id = d.release_id \
     WHERE d.project_id = $1 AND d.environment = $2";

const TOKENS: &str = "SELECT id, name, hint, created_by, created_at, revoked_by, revoked_at \
     FROM runtime_tokens WHERE project_id = $1 AND environment = $2";

/// A query assembled from this module's constant fragments: no value from
/// outside ever reaches the text, only bound parameters.
fn sql(text: String) -> sqlx::AssertSqlSafe<String> {
    sqlx::AssertSqlSafe(text)
}

impl Releases {
    pub fn new(
        pool: PgPool,
        clock: Arc<dyn Clock>,
        decisions: Decisions,
        projects: Projects,
        store: Arc<dyn ArtifactStore>,
        max_attempts: u32,
    ) -> Self {
        Self {
            pool,
            clock,
            decisions,
            projects,
            store,
            max_attempts,
            outbox: Arc::new(Notify::new()),
        }
    }

    /// Resolves when a deployment has been queued since the last call; the
    /// publisher waits on it between polls.
    pub async fn deployment_queued(&self) {
        self.outbox.notified().await;
    }

    // ----- Releases -------------------------------------------------------

    /// The project's releases, newest first (any member).
    pub async fn list(
        &self,
        access: &Access,
        page: PageRequest,
    ) -> Result<Page<ReleaseSummary>, ReleaseError> {
        let rows: Vec<SummaryRow> = sqlx::query_as(sql(format!(
            "{SUMMARY} ORDER BY r.major DESC, r.minor DESC, r.patch DESC LIMIT $2 OFFSET $3"
        )))
        .bind(access.project_id())
        .bind(page.limit)
        .bind(page.offset)
        .fetch_all(&self.pool)
        .await?;
        let (total,): (i64,) =
            sqlx::query_as("SELECT count(*) FROM releases WHERE project_id = $1")
                .bind(access.project_id())
                .fetch_one(&self.pool)
                .await?;
        Ok(Page {
            items: rows.into_iter().map(ReleaseSummary::from).collect(),
            total,
        })
    }

    /// One release with the version of each decision it froze (any member).
    pub async fn get(&self, access: &Access, id: Uuid) -> Result<Release, ReleaseError> {
        let mut conn = self.pool.acquire().await?;
        load_release(&mut conn, access.project_id(), id).await
    }

    /// What a release made now would hold (any member).
    pub async fn preview(&self, access: &Access) -> Result<ReleasePreview, ReleaseError> {
        let snapshot = self.decisions.release_snapshot(access).await?;
        let mut conn = self.pool.acquire().await?;
        Ok(ReleasePreview {
            latest: latest_version(&mut conn, access.project_id()).await?,
            decisions: snapshot
                .versions
                .into_iter()
                .map(|frozen| ReleasedDecision {
                    decision_id: frozen.decision_id,
                    key: frozen.key,
                    version: frozen.number,
                    tests: frozen.tests,
                })
                .collect(),
            unversioned: snapshot.unversioned,
        })
    }

    /// Freezes the latest version of every decision as a new release
    /// (editors). Refused while any decision has no version.
    pub async fn create(
        &self,
        access: &Access,
        bump: Bump,
        notes: &str,
    ) -> Result<Release, ReleaseError> {
        let notes = notes.trim();
        if notes.is_empty() || notes.chars().count() > MAX_NOTES_CHARS {
            return Err(ReleaseError::InvalidNotes);
        }
        let snapshot = self.decisions.release_snapshot(access).await?;
        if !snapshot.unversioned.is_empty() {
            return Err(ReleaseError::Unversioned(snapshot.unversioned));
        }
        if snapshot.versions.is_empty() {
            return Err(ReleaseError::NothingToRelease);
        }

        let now = self.clock.now();
        let id = Uuid::new_v4();
        let mut tx = self.pool.begin().await?;
        authorize_change(&mut tx, access, Role::Editor).await?;
        let version = SemVer::next(latest_version(&mut tx, access.project_id()).await?, bump);
        sqlx::query(
            "INSERT INTO releases (id, project_id, major, minor, patch, notes, created_by, created_at) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
        )
        .bind(id)
        .bind(access.project_id())
        .bind(version.major)
        .bind(version.minor)
        .bind(version.patch)
        .bind(notes)
        .bind(access.user_id())
        .bind(now)
        .execute(&mut *tx)
        .await
        .map_err(|err| unique_or(err, ReleaseError::VersionTaken(version)))?;
        for frozen in &snapshot.versions {
            sqlx::query(
                "INSERT INTO release_decisions \
                 (release_id, decision_id, key, version_number, content, tests_passed, tests_failed, tests_errors) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
            )
            .bind(id)
            .bind(frozen.decision_id)
            .bind(&frozen.key)
            .bind(frozen.number)
            .bind(&frozen.content)
            .bind(frozen.tests.passed)
            .bind(frozen.tests.failed)
            .bind(frozen.tests.errors)
            .execute(&mut *tx)
            .await?;
        }
        donka_audit::record(
            &mut tx,
            Event::new(now, Some(access.user_id()), Action::ReleaseCreated)
                .in_project(access.project_id())
                .with_details(json!({
                    "version": version.to_string(),
                    "decisions": snapshot.versions.len(),
                })),
        )
        .await?;
        let release = load_release(&mut tx, access.project_id(), id).await?;
        tx.commit().await?;
        Ok(release)
    }

    // ----- Environments and deployments ----------------------------------

    /// Both environments: what is live, what was last asked for, how many tokens (any member).
    pub async fn environments(
        &self,
        access: &Access,
    ) -> Result<Vec<EnvironmentState>, ReleaseError> {
        let mut conn = self.pool.acquire().await?;
        let mut states = Vec::with_capacity(Environment::ALL.len());
        for environment in Environment::ALL {
            let live: Option<DeploymentRow> = sqlx::query_as(sql(format!(
                "{DEPLOYMENTS} AND d.published_at IS NOT NULL ORDER BY d.published_at DESC LIMIT 1"
            )))
            .bind(access.project_id())
            .bind(environment.as_str())
            .fetch_optional(&mut *conn)
            .await?;
            let latest: Option<DeploymentRow> = sqlx::query_as(sql(format!(
                "{DEPLOYMENTS} AND d.superseded_at IS NULL ORDER BY d.seq DESC LIMIT 1"
            )))
            .bind(access.project_id())
            .bind(environment.as_str())
            .fetch_optional(&mut *conn)
            .await?;
            let (tokens,): (i64,) = sqlx::query_as(
                "SELECT count(*) FROM runtime_tokens \
                 WHERE project_id = $1 AND environment = $2 AND revoked_at IS NULL",
            )
            .bind(access.project_id())
            .bind(environment.as_str())
            .fetch_one(&mut *conn)
            .await?;
            states.push(EnvironmentState {
                environment,
                live: live.map(Deployment::from),
                latest: latest.map(Deployment::from),
                tokens,
            });
        }
        Ok(states)
    }

    /// An environment's deployments, newest first (any member).
    pub async fn deployments(
        &self,
        access: &Access,
        environment: Environment,
        page: PageRequest,
    ) -> Result<Page<Deployment>, ReleaseError> {
        let rows: Vec<DeploymentRow> = sqlx::query_as(sql(format!(
            "{DEPLOYMENTS} ORDER BY d.seq DESC LIMIT $3 OFFSET $4"
        )))
        .bind(access.project_id())
        .bind(environment.as_str())
        .bind(page.limit)
        .bind(page.offset)
        .fetch_all(&self.pool)
        .await?;
        let (total,): (i64,) = sqlx::query_as(
            "SELECT count(*) FROM deployments WHERE project_id = $1 AND environment = $2",
        )
        .bind(access.project_id())
        .bind(environment.as_str())
        .fetch_one(&self.pool)
        .await?;
        Ok(Page {
            items: rows.into_iter().map(Deployment::from).collect(),
            total,
        })
    }

    /// Deploys a release to staging (editors). The artifact is written after
    /// commit by the publisher; production goes through an approval.
    pub async fn deploy(
        &self,
        access: &Access,
        environment: Environment,
        release_id: Uuid,
    ) -> Result<Deployment, ReleaseError> {
        if environment == Environment::Production {
            return Err(ReleaseError::ApprovalRequired);
        }
        let project = self.projects.get(access).await?;
        let now = self.clock.now();
        let mut tx = self.pool.begin().await?;
        authorize_change(&mut tx, access, Role::Editor).await?;
        let version: Option<SemVer> = sqlx::query_as(
            "SELECT major, minor, patch FROM releases WHERE project_id = $1 AND id = $2",
        )
        .bind(access.project_id())
        .bind(release_id)
        .fetch_optional(&mut *tx)
        .await?;
        let version = version.ok_or(ReleaseError::NotFound)?;
        let id = enqueue(
            &mut tx,
            access,
            &project,
            environment,
            release_id,
            DeploymentReason::Deploy,
            now,
        )
        .await?;
        donka_audit::record(
            &mut tx,
            Event::new(now, Some(access.user_id()), Action::ReleaseDeployed)
                .in_project(access.project_id())
                .with_details(json!({
                    "version": version.to_string(),
                    "environment": environment.as_str(),
                })),
        )
        .await?;
        let deployment = load_deployment(&mut tx, access.project_id(), environment, id).await?;
        tx.commit().await?;
        self.outbox.notify_one();
        Ok(deployment)
    }

    /// Tries a failed deployment again now (editors).
    pub async fn retry(
        &self,
        access: &Access,
        environment: Environment,
        id: Uuid,
    ) -> Result<Deployment, ReleaseError> {
        let mut tx = self.pool.begin().await?;
        authorize_change(&mut tx, access, Role::Editor).await?;
        let current = load_deployment(&mut tx, access.project_id(), environment, id).await?;
        if current.status != DeploymentStatus::Failed {
            return Err(ReleaseError::NotRetryable);
        }
        sqlx::query(
            "UPDATE deployments SET abandoned_at = NULL, next_attempt_at = $2 WHERE id = $1",
        )
        .bind(id)
        .bind(self.clock.now())
        .execute(&mut *tx)
        .await?;
        let deployment = load_deployment(&mut tx, access.project_id(), environment, id).await?;
        tx.commit().await?;
        self.outbox.notify_one();
        Ok(deployment)
    }

    /// Writes the artifacts of the deployments that are due. Runs in the
    /// background; each deployment is locked while it is written, so several
    /// Studio instances never write the same one twice.
    pub async fn publish_due(&self) -> Result<usize, ReleaseError> {
        let mut published = 0;
        for _ in 0..PUBLISH_BATCH {
            let now = self.clock.now();
            let mut tx = self.pool.begin().await?;
            let due: Option<DueDeployment> = sqlx::query_as(
                "SELECT d.id, d.project_id, d.environment, d.release_id, d.project_key, \
                 d.project_name, d.attempts, r.major, r.minor, r.patch \
                 FROM deployments d JOIN releases r ON r.id = d.release_id \
                 WHERE d.published_at IS NULL AND d.abandoned_at IS NULL AND d.superseded_at IS NULL \
                 AND d.next_attempt_at <= $1 ORDER BY d.seq LIMIT 1 \
                 FOR UPDATE OF d SKIP LOCKED",
            )
            .bind(now)
            .fetch_optional(&mut *tx)
            .await?;
            let Some(due) = due else { break };

            let decisions: Vec<(String, serde_json::Value)> = sqlx::query_as(
                "SELECT key, content FROM release_decisions WHERE release_id = $1 ORDER BY key",
            )
            .bind(due.release_id)
            .fetch_all(&mut *tx)
            .await?;
            let tokens: Vec<(Uuid, String)> = sqlx::query_as(
                "SELECT id, hash FROM runtime_tokens \
                 WHERE project_id = $1 AND environment = $2 AND revoked_at IS NULL ORDER BY created_at",
            )
            .bind(due.project_id)
            .bind(&due.environment)
            .fetch_all(&mut *tx)
            .await?;
            let input = artifact::ArtifactInput {
                project_id: due.project_id.to_string(),
                project_key: &due.project_key,
                project_name: &due.project_name,
                release_id: due.release_id.to_string(),
                release_version: due.version.to_string(),
                environment: &due.environment,
                deployment_id: due.id.to_string(),
                deployed_at: now.to_rfc3339(),
                token_hashes: tokens
                    .into_iter()
                    .map(|(id, hash)| (id.to_string(), hash))
                    .collect(),
                decisions,
            };
            let key = artifact::object_key(&due.environment, &due.project_key);
            let written = match artifact::build(&input) {
                Ok(bytes) => self
                    .store
                    .put(&key, bytes)
                    .await
                    .map_err(|err| err.to_string()),
                Err(err) => Err(err),
            };
            let attempts = due.attempts.saturating_add(1);
            match written {
                Ok(()) => {
                    sqlx::query(
                        "UPDATE deployments SET published_at = $2, attempts = $3, last_error = NULL \
                         WHERE id = $1",
                    )
                    .bind(due.id)
                    .bind(now)
                    .bind(attempts)
                    .execute(&mut *tx)
                    .await?;
                    tracing::info!(deployment_id = %due.id, %key, "release published");
                    published += 1;
                }
                Err(error) => {
                    let max = i32::try_from(self.max_attempts).unwrap_or(i32::MAX);
                    let abandoned_at = (attempts >= max).then_some(now);
                    if abandoned_at.is_some() {
                        tracing::error!(deployment_id = %due.id, %key, %error, attempts, "deployment given up");
                    } else {
                        tracing::warn!(deployment_id = %due.id, %key, %error, attempts, "deployment not written; will retry");
                    }
                    sqlx::query(
                        "UPDATE deployments SET attempts = $2, last_error = $3, next_attempt_at = $4, \
                         abandoned_at = $5 WHERE id = $1",
                    )
                    .bind(due.id)
                    .bind(attempts)
                    .bind(&error)
                    .bind(now + retry_delay(attempts))
                    .bind(abandoned_at)
                    .execute(&mut *tx)
                    .await?;
                }
            }
            tx.commit().await?;
        }
        Ok(published)
    }

    // ----- Runtime tokens -------------------------------------------------

    /// An environment's tokens, live ones first; never their value (any member).
    pub async fn tokens(
        &self,
        access: &Access,
        environment: Environment,
    ) -> Result<Vec<RuntimeToken>, ReleaseError> {
        Ok(sqlx::query_as(sql(format!(
            "{TOKENS} ORDER BY revoked_at IS NOT NULL, created_at DESC"
        )))
        .bind(access.project_id())
        .bind(environment.as_str())
        .fetch_all(&self.pool)
        .await?)
    }

    /// Issues a token for an environment (owners). Its value is returned once
    /// and only its hash is kept; the environment's release is published again
    /// so the Runtime accepts it.
    pub async fn issue_token(
        &self,
        access: &Access,
        environment: Environment,
        name: &str,
    ) -> Result<IssuedToken, ReleaseError> {
        let name = name.trim();
        if name.is_empty() || name.chars().count() > MAX_TOKEN_NAME_CHARS {
            return Err(ReleaseError::InvalidTokenName);
        }
        let mut bytes = [0u8; TOKEN_BYTES];
        getrandom::fill(&mut bytes).map_err(|_| ReleaseError::Random)?;
        let token = format!("{TOKEN_PREFIX}{}", URL_SAFE_NO_PAD.encode(bytes));
        let hint: String = token
            .chars()
            .rev()
            .take(4)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        let project = self.projects.get(access).await?;
        let now = self.clock.now();
        let id = Uuid::new_v4();

        let mut tx = self.pool.begin().await?;
        authorize_change(&mut tx, access, Role::Owner).await?;
        sqlx::query(
            "INSERT INTO runtime_tokens (id, project_id, environment, name, hash, hint, created_by, created_at) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
        )
        .bind(id)
        .bind(access.project_id())
        .bind(environment.as_str())
        .bind(name)
        .bind(artifact::hash_token(&token))
        .bind(&hint)
        .bind(access.user_id())
        .bind(now)
        .execute(&mut *tx)
        .await?;
        donka_audit::record(
            &mut tx,
            Event::new(now, Some(access.user_id()), Action::TokenIssued)
                .in_project(access.project_id())
                .with_details(json!({ "environment": environment.as_str(), "name": name })),
        )
        .await?;
        republish(&mut tx, access, &project, environment, now).await?;
        tx.commit().await?;
        self.outbox.notify_one();
        Ok(IssuedToken {
            token,
            details: RuntimeToken {
                id,
                name: name.to_owned(),
                hint,
                created_by: access.user_id(),
                created_at: now,
                revoked_by: None,
                revoked_at: None,
            },
        })
    }

    /// Revokes a token (owners); the environment's release is published again without it.
    pub async fn revoke_token(
        &self,
        access: &Access,
        environment: Environment,
        id: Uuid,
    ) -> Result<(), ReleaseError> {
        let project = self.projects.get(access).await?;
        let now = self.clock.now();
        let mut tx = self.pool.begin().await?;
        authorize_change(&mut tx, access, Role::Owner).await?;
        let revoked: Option<(String,)> = sqlx::query_as(
            "UPDATE runtime_tokens SET revoked_at = $4, revoked_by = $5 \
             WHERE project_id = $1 AND environment = $2 AND id = $3 AND revoked_at IS NULL \
             RETURNING name",
        )
        .bind(access.project_id())
        .bind(environment.as_str())
        .bind(id)
        .bind(now)
        .bind(access.user_id())
        .fetch_optional(&mut *tx)
        .await?;
        let (name,) = revoked.ok_or(ReleaseError::TokenNotFound)?;
        donka_audit::record(
            &mut tx,
            Event::new(now, Some(access.user_id()), Action::TokenRevoked)
                .in_project(access.project_id())
                .with_details(json!({ "environment": environment.as_str(), "name": name })),
        )
        .await?;
        republish(&mut tx, access, &project, environment, now).await?;
        tx.commit().await?;
        self.outbox.notify_one();
        Ok(())
    }
}

#[derive(sqlx::FromRow)]
struct DueDeployment {
    id: Uuid,
    project_id: Uuid,
    environment: String,
    release_id: Uuid,
    project_key: String,
    project_name: String,
    attempts: i32,
    #[sqlx(flatten)]
    version: SemVer,
}

async fn latest_version(
    conn: &mut sqlx::PgConnection,
    project_id: Uuid,
) -> Result<Option<SemVer>, ReleaseError> {
    Ok(sqlx::query_as(
        "SELECT major, minor, patch FROM releases WHERE project_id = $1 \
         ORDER BY major DESC, minor DESC, patch DESC LIMIT 1",
    )
    .bind(project_id)
    .fetch_optional(conn)
    .await?)
}

async fn load_release(
    conn: &mut sqlx::PgConnection,
    project_id: Uuid,
    id: Uuid,
) -> Result<Release, ReleaseError> {
    let row: Option<SummaryRow> = sqlx::query_as(sql(format!("{SUMMARY} AND r.id = $2")))
        .bind(project_id)
        .bind(id)
        .fetch_optional(&mut *conn)
        .await?;
    let summary = row.ok_or(ReleaseError::NotFound)?.into();
    let decisions = sqlx::query_as(
        "SELECT decision_id, key, version_number AS version, tests_passed AS passed, \
         tests_failed AS failed, tests_errors AS errors \
         FROM release_decisions WHERE release_id = $1 ORDER BY key",
    )
    .bind(id)
    .fetch_all(&mut *conn)
    .await?;
    Ok(Release { summary, decisions })
}

async fn load_deployment(
    conn: &mut sqlx::PgConnection,
    project_id: Uuid,
    environment: Environment,
    id: Uuid,
) -> Result<Deployment, ReleaseError> {
    let row: Option<DeploymentRow> = sqlx::query_as(sql(format!("{DEPLOYMENTS} AND d.id = $3")))
        .bind(project_id)
        .bind(environment.as_str())
        .bind(id)
        .fetch_optional(conn)
        .await?;
    row.map(Deployment::from)
        .ok_or(ReleaseError::DeploymentNotFound)
}

/// Queues a deployment; any earlier one of the environment not yet published
/// is superseded, so an older release never lands after a newer one.
async fn enqueue(
    tx: &mut Tx<'_>,
    access: &Access,
    project: &donka_project::Project,
    environment: Environment,
    release_id: Uuid,
    reason: DeploymentReason,
    now: DateTime<Utc>,
) -> Result<Uuid, ReleaseError> {
    sqlx::query(
        "UPDATE deployments SET superseded_at = $3 \
         WHERE project_id = $1 AND environment = $2 AND published_at IS NULL AND superseded_at IS NULL",
    )
    .bind(access.project_id())
    .bind(environment.as_str())
    .bind(now)
    .execute(&mut **tx)
    .await?;
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO deployments (id, project_id, environment, release_id, project_key, project_name, \
         reason, requested_by, requested_at, next_attempt_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $9)",
    )
    .bind(id)
    .bind(access.project_id())
    .bind(environment.as_str())
    .bind(release_id)
    .bind(&project.key)
    .bind(&project.name)
    .bind(reason.as_str())
    .bind(access.user_id())
    .bind(now)
    .execute(&mut **tx)
    .await?;
    Ok(id)
}

/// The environment's tokens changed: publish its release again, if it has one.
async fn republish(
    tx: &mut Tx<'_>,
    access: &Access,
    project: &donka_project::Project,
    environment: Environment,
    now: DateTime<Utc>,
) -> Result<(), ReleaseError> {
    let latest: Option<(Uuid,)> = sqlx::query_as(
        "SELECT release_id FROM deployments WHERE project_id = $1 AND environment = $2 \
         AND reason = 'deploy' ORDER BY seq DESC LIMIT 1",
    )
    .bind(access.project_id())
    .bind(environment.as_str())
    .fetch_optional(&mut **tx)
    .await?;
    if let Some((release_id,)) = latest {
        enqueue(
            tx,
            access,
            project,
            environment,
            release_id,
            DeploymentReason::Tokens,
            now,
        )
        .await?;
    }
    Ok(())
}

fn retry_delay(attempts: i32) -> Duration {
    let doublings = u32::try_from(attempts.saturating_sub(1))
        .unwrap_or(0)
        .min(16);
    Duration::seconds((RETRY_BASE_SECONDS << doublings).min(RETRY_MAX_SECONDS))
}

/// A unique violation means someone did the same thing at the same moment.
fn unique_or(err: sqlx::Error, taken: ReleaseError) -> ReleaseError {
    match err.as_database_error().and_then(|db| db.code()) {
        Some(code) if code == "23505" => taken,
        _ => err.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_count_up_from_1_0_0() {
        assert_eq!(SemVer::next(None, Bump::Patch), SemVer::FIRST);
        let v = SemVer {
            major: 1,
            minor: 4,
            patch: 2,
        };
        assert_eq!(SemVer::next(Some(v), Bump::Patch).to_string(), "1.4.3");
        assert_eq!(SemVer::next(Some(v), Bump::Minor).to_string(), "1.5.0");
        assert_eq!(SemVer::next(Some(v), Bump::Major).to_string(), "2.0.0");
    }

    #[test]
    fn retries_back_off_up_to_an_hour() {
        assert_eq!(retry_delay(1), Duration::seconds(30));
        assert_eq!(retry_delay(2), Duration::seconds(60));
        assert_eq!(retry_delay(20), Duration::seconds(3600));
    }

    #[test]
    fn environments_read_and_write() {
        for environment in Environment::ALL {
            assert_eq!(Environment::parse(environment.as_str()), Some(environment));
        }
        assert_eq!(Environment::parse("dev"), None);
    }
}
