//! Rules sync (DNK-20): CI pipelines pull a project's release artifact with a
//! CI token an owner issued for the project.
//!
//! A pipeline names a project (key or id) and a target:
//! - `main`: the project's newest release;
//! - `commit:<id>`: a release by its id (releases are Donka's immutable snapshots);
//! - `release:<version>`: a release by its semantic version (`1.4.0`);
//! - `env:<key>`: what is live in `staging` or `production`.
//!
//! The answer says where to download the artifact and its SHA-256. A
//! release's artifact has no environment and lists no token, so a Runtime
//! given it refuses every request; an environment's artifact is the one its
//! Runtime reads, tokens included. Artifacts are rebuilt from the database on
//! download and are byte-for-byte reproducible, so the checksum holds.
//!
//! A token reaches only its own project: any other project, existing or not,
//! answers `no_access`, and a download of another project's artifact `404`.

use crate::artifact::{self, ArtifactInput, DeploymentInput};
use crate::{sql, Environment, ReleaseError, Releases, SemVer, MAX_TOKEN_NAME_CHARS};
use chrono::{DateTime, Utc};
use donka_audit::{Action, Event};
use donka_project::{authorize_change, Access, Role};
use donka_shared::secret::{self, IssuedSecret};
use serde_json::json;
use sha2::{Digest, Sha256};
use uuid::Uuid;

/// Every CI token starts with this, so a leaked one is easy to recognise.
const CI_TOKEN_PREFIX: &str = "dnk_ci_";
/// Deployments one sync request may ask about.
pub const MAX_SYNC_DEPLOYMENTS: usize = 50;

/// A CI token as members see it; never its value.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct CiToken {
    pub id: Uuid,
    pub name: String,
    pub hint: String,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub revoked_by: Option<Uuid>,
    pub revoked_at: Option<DateTime<Utc>>,
    pub last_used_at: Option<DateTime<Utc>>,
}

/// A token just issued: its value is shown this once.
#[derive(Debug, Clone)]
pub struct IssuedCiToken {
    pub token: String,
    pub details: CiToken,
}

/// What a request's CI token gives access to: one project, read-only.
#[derive(Debug, Clone, Copy)]
pub struct CiAccess {
    project_id: Uuid,
}

/// What a pipeline asks to pull.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    Main,
    Release(Uuid),
    Version(SemVer),
    Environment(Environment),
}

impl Target {
    /// Reads a target; the error is the code the pipeline gets back.
    pub fn parse(text: &str) -> Result<Self, &'static str> {
        let text = text.trim();
        if text == "main" {
            return Ok(Self::Main);
        }
        let (kind, value) = text.split_once(':').ok_or("INVALID_TARGET")?;
        match kind {
            "commit" => Uuid::parse_str(value)
                .map(Self::Release)
                .map_err(|_| "INVALID_TARGET"),
            "release" => parse_version(value)
                .map(Self::Version)
                .ok_or("INVALID_TARGET"),
            "env" => Environment::parse(value)
                .map(Self::Environment)
                .ok_or("INVALID_TARGET"),
            // Donka has no branches: releases are its only snapshots.
            "branch" => Err("UNSUPPORTED_TARGET"),
            _ => Err("INVALID_TARGET"),
        }
    }
}

/// `1.4.0` or `v1.4.0`.
fn parse_version(text: &str) -> Option<SemVer> {
    let text = text.strip_prefix('v').unwrap_or(text);
    let mut parts = text.split('.').map(|part| {
        (!part.is_empty() && part.chars().all(|c| c.is_ascii_digit()))
            .then(|| part.parse::<i32>().ok())
            .flatten()
    });
    let version = SemVer {
        major: parts.next()??,
        minor: parts.next()??,
        patch: parts.next()??,
    };
    parts.next().is_none().then_some(version)
}

/// One deployment a pipeline asks about.
#[derive(Debug, Clone)]
pub struct SyncRequest {
    /// The project's key or id.
    pub project: String,
    pub target: String,
    /// Ids the pipeline already holds (a release id, or for `env:` the
    /// deployment id it was given as `commit`); a match answers `NoChange`.
    pub current: Vec<String>,
}

/// The project a target resolved in.
#[derive(Debug, Clone)]
pub struct SyncProject {
    pub id: Uuid,
    pub key: String,
}

/// What a target resolved to.
#[derive(Debug, Clone)]
pub struct Resolved {
    pub project: SyncProject,
    pub release_id: Uuid,
    pub version: SemVer,
    pub notes: String,
    /// For `env:` targets: the environment and the deployment live in it.
    pub deployment: Option<(Environment, Uuid)>,
}

impl Resolved {
    /// The id a pipeline holds once it has this artifact: the deployment's
    /// for an environment (its tokens change with redeployments), else the release's.
    pub fn snapshot_id(&self) -> Uuid {
        self.deployment.map_or(self.release_id, |(_, id)| id)
    }
}

/// Where to download an artifact, relative to the API base path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArtifactPath {
    Release { project: Uuid, release: Uuid },
    Deployment { project: Uuid, deployment: Uuid },
}

impl ArtifactPath {
    pub fn url(self) -> String {
        match self {
            Self::Release { project, release } => {
                format!("/rules-sync/artifacts/{project}/releases/{release}")
            }
            Self::Deployment {
                project,
                deployment,
            } => format!("/rules-sync/artifacts/{project}/deployments/{deployment}"),
        }
    }
}

#[derive(Debug, Clone)]
pub enum SyncOutcome {
    /// The token cannot reach this project (or it does not exist).
    NoAccess,
    /// The project has no release yet, or nothing is live in the environment.
    NoRelease { project: SyncProject },
    /// The target could not be read or resolved; `code` says why.
    Error {
        project: Option<SyncProject>,
        code: &'static str,
    },
    /// The pipeline already holds this artifact.
    NoChange(Resolved),
    Load {
        resolved: Resolved,
        path: ArtifactPath,
        sha256: String,
    },
}

const CI_TOKENS: &str =
    "SELECT id, name, hint, created_by, created_at, revoked_by, revoked_at, last_used_at \
     FROM ci_tokens WHERE project_id = $1";

#[derive(sqlx::FromRow)]
struct ReleaseRow {
    id: Uuid,
    major: i32,
    minor: i32,
    patch: i32,
    notes: String,
}

impl Releases {
    // ----- CI tokens --------------------------------------------------------

    /// The project's CI tokens, live ones first; never their value (any member).
    pub async fn ci_tokens(&self, access: &Access) -> Result<Vec<CiToken>, ReleaseError> {
        Ok(sqlx::query_as(sql(format!(
            "{CI_TOKENS} ORDER BY revoked_at IS NOT NULL, created_at DESC"
        )))
        .bind(access.project_id())
        .fetch_all(&self.pool)
        .await?)
    }

    /// Issues a read-only CI token for the project (owners). Its value is
    /// returned once and only its hash is kept.
    pub async fn issue_ci_token(
        &self,
        access: &Access,
        name: &str,
    ) -> Result<IssuedCiToken, ReleaseError> {
        let name = name.trim();
        if name.is_empty() || name.chars().count() > MAX_TOKEN_NAME_CHARS {
            return Err(ReleaseError::InvalidTokenName);
        }
        let IssuedSecret { token, hash, hint } =
            secret::issue(CI_TOKEN_PREFIX).map_err(|_| ReleaseError::Random)?;
        let now = self.clock.now();
        let id = Uuid::new_v4();
        let mut tx = self.pool.begin().await?;
        authorize_change(&mut tx, access, Role::Owner).await?;
        sqlx::query(
            "INSERT INTO ci_tokens (id, project_id, name, hash, hint, created_by, created_at) \
             VALUES ($1, $2, $3, $4, $5, $6, $7)",
        )
        .bind(id)
        .bind(access.project_id())
        .bind(name)
        .bind(&hash)
        .bind(&hint)
        .bind(access.user_id())
        .bind(now)
        .execute(&mut *tx)
        .await?;
        donka_audit::record(
            &mut tx,
            Event::new(now, Some(access.user_id()), Action::CiTokenIssued)
                .in_project(access.project_id())
                .with_details(json!({ "name": name })),
        )
        .await?;
        tx.commit().await?;
        Ok(IssuedCiToken {
            token,
            details: CiToken {
                id,
                name: name.to_owned(),
                hint,
                created_by: access.user_id(),
                created_at: now,
                revoked_by: None,
                revoked_at: None,
                last_used_at: None,
            },
        })
    }

    /// Revokes a CI token (owners); pipelines using it are refused from then on.
    pub async fn revoke_ci_token(&self, access: &Access, id: Uuid) -> Result<(), ReleaseError> {
        let now = self.clock.now();
        let mut tx = self.pool.begin().await?;
        authorize_change(&mut tx, access, Role::Owner).await?;
        let revoked: Option<(String,)> = sqlx::query_as(
            "UPDATE ci_tokens SET revoked_at = $3, revoked_by = $4 \
             WHERE project_id = $1 AND id = $2 AND revoked_at IS NULL RETURNING name",
        )
        .bind(access.project_id())
        .bind(id)
        .bind(now)
        .bind(access.user_id())
        .fetch_optional(&mut *tx)
        .await?;
        let (name,) = revoked.ok_or(ReleaseError::TokenNotFound)?;
        donka_audit::record(
            &mut tx,
            Event::new(now, Some(access.user_id()), Action::CiTokenRevoked)
                .in_project(access.project_id())
                .with_details(json!({ "name": name })),
        )
        .await?;
        tx.commit().await?;
        Ok(())
    }

    /// The access a pipeline's bearer token gives; unknown and revoked tokens
    /// are refused alike. Records when the token was last used.
    pub async fn authenticate_ci(&self, bearer: &str) -> Result<CiAccess, ReleaseError> {
        if !bearer.starts_with(CI_TOKEN_PREFIX) {
            return Err(ReleaseError::InvalidCiToken);
        }
        let found: Option<(Uuid,)> = sqlx::query_as(
            "UPDATE ci_tokens SET last_used_at = $2 \
             WHERE hash = $1 AND revoked_at IS NULL RETURNING project_id",
        )
        .bind(secret::hash(bearer))
        .bind(self.clock.now())
        .fetch_optional(&self.pool)
        .await?;
        let (project_id,) = found.ok_or(ReleaseError::InvalidCiToken)?;
        Ok(CiAccess { project_id })
    }

    // ----- Sync -------------------------------------------------------------

    /// Resolves what a pipeline asks for.
    pub async fn sync(
        &self,
        ci: CiAccess,
        request: &SyncRequest,
    ) -> Result<SyncOutcome, ReleaseError> {
        let reference = request.project.trim();
        let project = match self.projects.find(ci.project_id).await {
            Ok(project) => project,
            // The token's project is gone: it reaches nothing.
            Err(donka_project::ProjectError::NotFound) => return Ok(SyncOutcome::NoAccess),
            Err(err) => return Err(err.into()),
        };
        if reference != project.key && reference != project.id.to_string() {
            return Ok(SyncOutcome::NoAccess);
        }
        let project = SyncProject {
            id: project.id,
            key: project.key,
        };
        let target = match Target::parse(&request.target) {
            Ok(target) => target,
            Err(code) => {
                return Ok(SyncOutcome::Error {
                    project: Some(project),
                    code,
                })
            }
        };
        let Some(resolved) = self.resolve(&project, target).await? else {
            return Ok(match target {
                Target::Main | Target::Environment(_) => SyncOutcome::NoRelease { project },
                Target::Release(_) | Target::Version(_) => SyncOutcome::Error {
                    project: Some(project),
                    code: "RELEASE_NOT_FOUND",
                },
            });
        };
        let held = resolved.snapshot_id().to_string();
        if request.current.iter().any(|id| id.trim() == held) {
            return Ok(SyncOutcome::NoChange(resolved));
        }
        let path = match resolved.deployment {
            Some((_, deployment)) => ArtifactPath::Deployment {
                project: project.id,
                deployment,
            },
            None => ArtifactPath::Release {
                project: project.id,
                release: resolved.release_id,
            },
        };
        let bytes = self.ci_artifact(ci, path).await?;
        Ok(SyncOutcome::Load {
            resolved,
            path,
            sha256: hex(&Sha256::digest(&bytes)),
        })
    }

    async fn resolve(
        &self,
        project: &SyncProject,
        target: Target,
    ) -> Result<Option<Resolved>, ReleaseError> {
        const RELEASES: &str =
            "SELECT id, major, minor, patch, notes FROM releases WHERE project_id = $1";
        let mut deployment = None;
        let row: Option<ReleaseRow> = match target {
            Target::Main => {
                sqlx::query_as(sql(format!(
                    "{RELEASES} ORDER BY major DESC, minor DESC, patch DESC LIMIT 1"
                )))
                .bind(project.id)
                .fetch_optional(&self.pool)
                .await?
            }
            Target::Release(id) => {
                sqlx::query_as(sql(format!("{RELEASES} AND id = $2")))
                    .bind(project.id)
                    .bind(id)
                    .fetch_optional(&self.pool)
                    .await?
            }
            Target::Version(v) => {
                sqlx::query_as(sql(format!(
                    "{RELEASES} AND major = $2 AND minor = $3 AND patch = $4"
                )))
                .bind(project.id)
                .bind(v.major)
                .bind(v.minor)
                .bind(v.patch)
                .fetch_optional(&self.pool)
                .await?
            }
            Target::Environment(environment) => {
                let live: Option<(Uuid, Uuid)> = sqlx::query_as(
                    "SELECT id, release_id FROM deployments \
                     WHERE project_id = $1 AND environment = $2 AND published_at IS NOT NULL \
                     ORDER BY published_at DESC, seq DESC LIMIT 1",
                )
                .bind(project.id)
                .bind(environment.as_str())
                .fetch_optional(&self.pool)
                .await?;
                let Some((deployment_id, release_id)) = live else {
                    return Ok(None);
                };
                deployment = Some((environment, deployment_id));
                sqlx::query_as(sql(format!("{RELEASES} AND id = $2")))
                    .bind(project.id)
                    .bind(release_id)
                    .fetch_optional(&self.pool)
                    .await?
            }
        };
        Ok(row.map(|row| Resolved {
            project: project.clone(),
            release_id: row.id,
            version: SemVer {
                major: row.major,
                minor: row.minor,
                patch: row.patch,
            },
            notes: row.notes,
            deployment,
        }))
    }

    /// The artifact at `path`, rebuilt from the database (reproducibly).
    /// Another project's artifact, or one never published, is not found.
    pub async fn ci_artifact(
        &self,
        ci: CiAccess,
        path: ArtifactPath,
    ) -> Result<Vec<u8>, ReleaseError> {
        let (project_id, release_id, deployment) = match path {
            ArtifactPath::Release { project, release } => (project, release, None),
            ArtifactPath::Deployment {
                project,
                deployment,
            } => {
                let row: Option<(Uuid, String, DateTime<Utc>)> = sqlx::query_as(
                    "SELECT release_id, environment, published_at FROM deployments \
                     WHERE project_id = $1 AND id = $2 AND published_at IS NOT NULL",
                )
                .bind(project)
                .bind(deployment)
                .fetch_optional(&self.pool)
                .await?;
                let (release, environment, published_at) = row.ok_or(ReleaseError::NotFound)?;
                (
                    project,
                    release,
                    Some((deployment, environment, published_at)),
                )
            }
        };
        if project_id != ci.project_id {
            return Err(ReleaseError::NotFound);
        }
        let project = self.projects.find(project_id).await?;
        let release: ReleaseRow = sqlx::query_as(
            "SELECT id, major, minor, patch, notes FROM releases WHERE project_id = $1 AND id = $2",
        )
        .bind(project_id)
        .bind(release_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(ReleaseError::NotFound)?;
        let decisions: Vec<(String, serde_json::Value)> = sqlx::query_as(
            "SELECT key, content FROM release_decisions WHERE release_id = $1 ORDER BY key",
        )
        .bind(release_id)
        .fetch_all(&self.pool)
        .await?;
        let tokens = match &deployment {
            Some((_, environment, _)) => {
                sqlx::query_as::<_, (Uuid, String)>(
                    "SELECT id, hash FROM runtime_tokens \
                     WHERE project_id = $1 AND environment = $2 AND revoked_at IS NULL \
                     ORDER BY created_at",
                )
                .bind(project_id)
                .bind(environment)
                .fetch_all(&self.pool)
                .await?
            }
            None => Vec::new(),
        };
        let version = SemVer {
            major: release.major,
            minor: release.minor,
            patch: release.patch,
        };
        artifact::build(&ArtifactInput {
            project_id: project.id.to_string(),
            project_key: &project.key,
            project_name: &project.name,
            release_id: release.id.to_string(),
            release_version: version.to_string(),
            deployment: deployment.as_ref().map(|(id, environment, published_at)| {
                DeploymentInput {
                    environment,
                    id: id.to_string(),
                    deployed_at: published_at.to_rfc3339(),
                    token_hashes: tokens
                        .iter()
                        .map(|(id, hash)| (id.to_string(), hash.clone()))
                        .collect(),
                }
            }),
            decisions,
        })
        .map_err(ReleaseError::Artifact)
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn targets_read_as_pipelines_write_them() {
        let id = Uuid::new_v4();
        assert_eq!(Target::parse("main"), Ok(Target::Main));
        assert_eq!(
            Target::parse(&format!("commit:{id}")),
            Ok(Target::Release(id))
        );
        let v = SemVer {
            major: 1,
            minor: 4,
            patch: 0,
        };
        assert_eq!(Target::parse("release:1.4.0"), Ok(Target::Version(v)));
        assert_eq!(Target::parse("release:v1.4.0"), Ok(Target::Version(v)));
        assert_eq!(
            Target::parse("env:production"),
            Ok(Target::Environment(Environment::Production))
        );
        assert_eq!(Target::parse("branch:feature"), Err("UNSUPPORTED_TARGET"));
        for bad in [
            "",
            "latest",
            "commit:abc",
            "release:1.4",
            "release:1.4.0.1",
            "release:1.-4.0",
            "env:dev",
            "tag:x",
        ] {
            assert_eq!(Target::parse(bad), Err("INVALID_TARGET"), "{bad}");
        }
    }
}
