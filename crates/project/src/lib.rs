//! Project module: projects, their members and roles.
//!
//! Every operation on a project goes through an [`Access`], which only
//! [`Projects::access`] can produce: holding one proves the user is a member,
//! and each operation checks the role it needs. Other modules use this crate
//! only through what is exported here.

use chrono::{DateTime, Utc};
use donka_audit::{Action, Event};
use donka_db::PgPool;
use donka_shared::clock::Clock;
use donka_shared::page::{Page, PageRequest};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;
use uuid::Uuid;

pub const MIN_KEY_CHARS: usize = 2;
pub const MAX_KEY_CHARS: usize = 40;
/// The key rule as a regular expression, for API clients (the check below is the same rule).
pub const KEY_PATTERN: &str = "^[a-z][a-z0-9]*(-[a-z0-9]+)*$";
pub const MAX_NAME_CHARS: usize = 100;
pub const MAX_DESCRIPTION_CHARS: usize = 1000;

/// What a member may do in a project. Each role includes the ones below it;
/// variants are declared from least to most, so `Ord` follows that order.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, sqlx::Type,
)]
#[serde(rename_all = "lowercase")]
#[sqlx(type_name = "text", rename_all = "lowercase")]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub enum Role {
    /// Reads everything in the project.
    Viewer,
    /// Also changes decisions, tests and releases (from DNK-8 on).
    Editor,
    /// Also renames, archives and manages members.
    Owner,
}

impl Role {
    /// Whether this role may do what `needed` may do.
    pub fn includes(self, needed: Role) -> bool {
        self >= needed
    }
}

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct Project {
    pub id: Uuid,
    pub key: String,
    pub name: String,
    pub description: String,
    pub created_at: DateTime<Utc>,
    pub archived_at: Option<DateTime<Utc>>,
}

/// A project in a list, with the reader's role in it.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct ProjectSummary {
    pub id: Uuid,
    pub key: String,
    pub name: String,
    pub description: String,
    pub archived_at: Option<DateTime<Utc>>,
    pub role: Role,
}

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct Member {
    pub user_id: Uuid,
    pub role: Role,
    pub added_at: DateTime<Utc>,
}

/// Proof that a user is a member of a project, with their role. Only this crate
/// creates one, so a caller cannot claim access it does not have.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Access {
    project_id: Uuid,
    user_id: Uuid,
    role: Role,
}

impl Access {
    pub fn project_id(&self) -> Uuid {
        self.project_id
    }

    pub fn user_id(&self) -> Uuid {
        self.user_id
    }

    pub fn role(&self) -> Role {
        self.role
    }

    /// Fails with `Forbidden` unless the member's role includes `needed`.
    pub fn require(&self, needed: Role) -> Result<(), ProjectError> {
        if self.role.includes(needed) {
            Ok(())
        } else {
            Err(ProjectError::Forbidden)
        }
    }
}

/// Which projects a list shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Listing {
    Active,
    Archived,
}

#[derive(Debug, thiserror::Error)]
pub enum ProjectError {
    /// The project does not exist or the user is not a member: deliberately the same.
    #[error("project not found")]
    NotFound,
    #[error("your role in this project does not allow this")]
    Forbidden,
    #[error("a project with this key already exists")]
    KeyTaken,
    #[error("keys are {MIN_KEY_CHARS} to {MAX_KEY_CHARS} lowercase letters, digits and single hyphens, starting with a letter")]
    InvalidKey,
    #[error("names are 1 to {MAX_NAME_CHARS} characters")]
    InvalidName,
    #[error("descriptions are at most {MAX_DESCRIPTION_CHARS} characters")]
    InvalidDescription,
    #[error("the project is archived")]
    Archived,
    #[error("this person is already a member")]
    AlreadyMember,
    #[error("this person is not a member")]
    MemberNotFound,
    #[error("a project needs at least one owner")]
    LastOwner,
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

#[derive(Clone)]
pub struct Projects {
    pool: PgPool,
    clock: Arc<dyn Clock>,
}

impl Projects {
    pub fn new(pool: PgPool, clock: Arc<dyn Clock>) -> Self {
        Self { pool, clock }
    }

    /// Creates a project with `creator` as its first owner.
    pub async fn create(
        &self,
        creator: Uuid,
        key: &str,
        name: &str,
        description: &str,
    ) -> Result<Project, ProjectError> {
        let key = check_key(key)?;
        let name = check_name(name)?;
        let description = check_description(description)?;
        let now = self.clock.now();
        let mut tx = self.pool.begin().await?;
        let project: Project = sqlx::query_as(
            "INSERT INTO projects (id, key, name, description, created_by, created_at) \
             VALUES ($1, $2, $3, $4, $5, $6) \
             RETURNING id, key, name, description, created_at, archived_at",
        )
        .bind(Uuid::new_v4())
        .bind(&key)
        .bind(&name)
        .bind(&description)
        .bind(creator)
        .bind(now)
        .fetch_one(&mut *tx)
        .await
        .map_err(|err| match err {
            sqlx::Error::Database(db) if db.is_unique_violation() => ProjectError::KeyTaken,
            other => other.into(),
        })?;
        sqlx::query(
            "INSERT INTO project_members (project_id, user_id, role, added_at) \
             VALUES ($1, $2, 'owner', $3)",
        )
        .bind(project.id)
        .bind(creator)
        .bind(now)
        .execute(&mut *tx)
        .await?;
        donka_audit::record(
            &mut tx,
            Event::new(now, Some(creator), Action::ProjectCreated)
                .in_project(project.id)
                .with_details(json!({ "key": project.key, "name": project.name })),
        )
        .await?;
        tx.commit().await?;
        tracing::info!(project_id = %project.id, key = %project.key, creator = %creator, "project created");
        Ok(project)
    }

    /// The projects `user` is a member of, by name.
    pub async fn list_for(
        &self,
        user: Uuid,
        listing: Listing,
        page: PageRequest,
    ) -> Result<Page<ProjectSummary>, ProjectError> {
        let archived = listing == Listing::Archived;
        let items: Vec<ProjectSummary> = sqlx::query_as(
            "SELECT p.id, p.key, p.name, p.description, p.archived_at, m.role \
             FROM projects p JOIN project_members m ON m.project_id = p.id \
             WHERE m.user_id = $1 AND (p.archived_at IS NOT NULL) = $2 \
             ORDER BY lower(p.name), p.key LIMIT $3 OFFSET $4",
        )
        .bind(user)
        .bind(archived)
        .bind(page.limit)
        .bind(page.offset)
        .fetch_all(&self.pool)
        .await?;
        let (total,): (i64,) = sqlx::query_as(
            "SELECT count(*) FROM projects p JOIN project_members m ON m.project_id = p.id \
             WHERE m.user_id = $1 AND (p.archived_at IS NOT NULL) = $2",
        )
        .bind(user)
        .bind(archived)
        .fetch_one(&self.pool)
        .await?;
        Ok(Page { items, total })
    }

    /// The user's access to the project; `NotFound` when they are not a member,
    /// whether or not the project exists.
    pub async fn access(&self, user: Uuid, project_id: Uuid) -> Result<Access, ProjectError> {
        let role: Option<(Role,)> = sqlx::query_as(
            "SELECT role FROM project_members WHERE project_id = $1 AND user_id = $2",
        )
        .bind(project_id)
        .bind(user)
        .fetch_optional(&self.pool)
        .await?;
        let (role,) = role.ok_or(ProjectError::NotFound)?;
        Ok(Access {
            project_id,
            user_id: user,
            role,
        })
    }

    pub async fn get(&self, access: &Access) -> Result<Project, ProjectError> {
        sqlx::query_as(
            "SELECT id, key, name, description, created_at, archived_at FROM projects WHERE id = $1",
        )
        .bind(access.project_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(ProjectError::NotFound)
    }

    /// Changes the name and description; the key never changes. Owners only.
    pub async fn update_details(
        &self,
        access: &Access,
        name: &str,
        description: &str,
    ) -> Result<Project, ProjectError> {
        let name = check_name(name)?;
        let description = check_description(description)?;
        let mut tx = self.pool.begin().await?;
        lock_for_change(&mut tx, access, Role::Owner).await?;
        let before: Project = sqlx::query_as(
            "SELECT id, key, name, description, created_at, archived_at FROM projects WHERE id = $1",
        )
        .bind(access.project_id)
        .fetch_one(&mut *tx)
        .await?;
        if before.name == name && before.description == description {
            return Ok(before); // Nothing changed, nothing to record.
        }
        let project: Project = sqlx::query_as(
            "UPDATE projects SET name = $2, description = $3 WHERE id = $1 \
             RETURNING id, key, name, description, created_at, archived_at",
        )
        .bind(access.project_id)
        .bind(&name)
        .bind(&description)
        .fetch_one(&mut *tx)
        .await?;
        donka_audit::record(
            &mut tx,
            Event::new(
                self.clock.now(),
                Some(access.user_id),
                Action::ProjectUpdated,
            )
            .in_project(access.project_id)
            .with_details(json!({
                "from": { "name": before.name, "description": before.description },
                "to": { "name": project.name, "description": project.description },
            })),
        )
        .await?;
        tx.commit().await?;
        Ok(project)
    }

    /// Archives (read-only, hidden from the default list) or restores. Owners only.
    pub async fn set_archived(
        &self,
        access: &Access,
        archived: bool,
    ) -> Result<Project, ProjectError> {
        let now = self.clock.now();
        let mut tx = self.pool.begin().await?;
        let was_archived = lock_project(&mut tx, access, Role::Owner).await?.is_some();
        let project: Project = sqlx::query_as(
            "UPDATE projects SET archived_at = CASE WHEN $2 THEN COALESCE(archived_at, $3) END \
             WHERE id = $1 RETURNING id, key, name, description, created_at, archived_at",
        )
        .bind(access.project_id)
        .bind(archived)
        .bind(now)
        .fetch_one(&mut *tx)
        .await?;
        // Archiving an archived project (or restoring an active one) changes nothing.
        if was_archived != archived {
            let action = if archived {
                Action::ProjectArchived
            } else {
                Action::ProjectRestored
            };
            donka_audit::record(
                &mut tx,
                Event::new(now, Some(access.user_id), action).in_project(access.project_id),
            )
            .await?;
            tracing::info!(project_id = %access.project_id, archived, "project archive state changed");
        }
        tx.commit().await?;
        Ok(project)
    }

    pub async fn members(&self, access: &Access) -> Result<Vec<Member>, ProjectError> {
        Ok(sqlx::query_as(
            "SELECT user_id, role, added_at FROM project_members WHERE project_id = $1 \
             ORDER BY added_at, user_id",
        )
        .bind(access.project_id)
        .fetch_all(&self.pool)
        .await?)
    }

    /// Adds an existing Studio user. Owners only.
    pub async fn add_member(
        &self,
        access: &Access,
        user: Uuid,
        role: Role,
    ) -> Result<Member, ProjectError> {
        let now = self.clock.now();
        let mut tx = self.pool.begin().await?;
        lock_for_change(&mut tx, access, Role::Owner).await?;
        let member: Option<Member> = sqlx::query_as(
            "INSERT INTO project_members (project_id, user_id, role, added_at) \
             VALUES ($1, $2, $3, $4) ON CONFLICT DO NOTHING \
             RETURNING user_id, role, added_at",
        )
        .bind(access.project_id)
        .bind(user)
        .bind(role)
        .bind(now)
        .fetch_optional(&mut *tx)
        .await?;
        let member = member.ok_or(ProjectError::AlreadyMember)?;
        donka_audit::record(
            &mut tx,
            Event::new(now, Some(access.user_id), Action::MemberAdded)
                .in_project(access.project_id)
                .about_user(user)
                .with_details(json!({ "role": role })),
        )
        .await?;
        tx.commit().await?;
        tracing::info!(project_id = %access.project_id, user_id = %user, ?role, by = %access.user_id, "member added");
        Ok(member)
    }

    /// Changes a member's role; a project always keeps an owner. Owners only.
    pub async fn change_role(
        &self,
        access: &Access,
        user: Uuid,
        role: Role,
    ) -> Result<Member, ProjectError> {
        let mut tx = self.pool.begin().await?;
        lock_for_change(&mut tx, access, Role::Owner).await?;
        let current = current_role(&mut tx, access.project_id, user).await?;
        if current == Role::Owner && role != Role::Owner {
            ensure_another_owner(&mut tx, access.project_id).await?;
        }
        let member: Member = sqlx::query_as(
            "UPDATE project_members SET role = $3 WHERE project_id = $1 AND user_id = $2 \
             RETURNING user_id, role, added_at",
        )
        .bind(access.project_id)
        .bind(user)
        .bind(role)
        .fetch_one(&mut *tx)
        .await?;
        if current != role {
            donka_audit::record(
                &mut tx,
                Event::new(
                    self.clock.now(),
                    Some(access.user_id),
                    Action::MemberRoleChanged,
                )
                .in_project(access.project_id)
                .about_user(user)
                .with_details(json!({ "from": current, "to": role })),
            )
            .await?;
        }
        tx.commit().await?;
        tracing::info!(project_id = %access.project_id, user_id = %user, ?role, by = %access.user_id, "member role changed");
        Ok(member)
    }

    /// Removes a member; the last owner cannot be removed. Owners only.
    pub async fn remove_member(&self, access: &Access, user: Uuid) -> Result<(), ProjectError> {
        let mut tx = self.pool.begin().await?;
        lock_for_change(&mut tx, access, Role::Owner).await?;
        let role = current_role(&mut tx, access.project_id, user).await?;
        if role == Role::Owner {
            ensure_another_owner(&mut tx, access.project_id).await?;
        }
        sqlx::query("DELETE FROM project_members WHERE project_id = $1 AND user_id = $2")
            .bind(access.project_id)
            .bind(user)
            .execute(&mut *tx)
            .await?;
        donka_audit::record(
            &mut tx,
            Event::new(
                self.clock.now(),
                Some(access.user_id),
                Action::MemberRemoved,
            )
            .in_project(access.project_id)
            .about_user(user)
            .with_details(json!({ "role": role })),
        )
        .await?;
        tx.commit().await?;
        tracing::info!(project_id = %access.project_id, user_id = %user, by = %access.user_id, "member removed");
        Ok(())
    }
}

type Tx<'a> = sqlx::Transaction<'a, sqlx::Postgres>;

/// Locks the project row so concurrent changes run one after the other (two
/// owners demoting each other cannot leave the project without one), and
/// re-checks the actor's role under the lock: an owner demoted a moment ago
/// cannot finish an owner-only change. Returns when the project was archived.
async fn lock_project(
    tx: &mut Tx<'_>,
    access: &Access,
    needed: Role,
) -> Result<Option<DateTime<Utc>>, ProjectError> {
    access.require(needed)?;
    let row: Option<(Option<DateTime<Utc>>,)> =
        sqlx::query_as("SELECT archived_at FROM projects WHERE id = $1 FOR UPDATE")
            .bind(access.project_id)
            .fetch_optional(&mut **tx)
            .await?;
    let (archived_at,) = row.ok_or(ProjectError::NotFound)?;
    match current_role(tx, access.project_id, access.user_id).await {
        Ok(role) if role.includes(needed) => Ok(archived_at),
        Ok(_) => Err(ProjectError::Forbidden),
        Err(ProjectError::MemberNotFound) => Err(ProjectError::NotFound),
        Err(other) => Err(other),
    }
}

/// [`lock_project`], refusing changes to an archived project.
async fn lock_for_change(
    tx: &mut Tx<'_>,
    access: &Access,
    needed: Role,
) -> Result<(), ProjectError> {
    match lock_project(tx, access, needed).await? {
        Some(_) => Err(ProjectError::Archived),
        None => Ok(()),
    }
}

async fn current_role(tx: &mut Tx<'_>, project_id: Uuid, user: Uuid) -> Result<Role, ProjectError> {
    let role: Option<(Role,)> =
        sqlx::query_as("SELECT role FROM project_members WHERE project_id = $1 AND user_id = $2")
            .bind(project_id)
            .bind(user)
            .fetch_optional(&mut **tx)
            .await?;
    role.map(|(role,)| role).ok_or(ProjectError::MemberNotFound)
}

async fn ensure_another_owner(tx: &mut Tx<'_>, project_id: Uuid) -> Result<(), ProjectError> {
    let (owners,): (i64,) = sqlx::query_as(
        "SELECT count(*) FROM project_members WHERE project_id = $1 AND role = 'owner'",
    )
    .bind(project_id)
    .fetch_one(&mut **tx)
    .await?;
    if owners > 1 {
        Ok(())
    } else {
        Err(ProjectError::LastOwner)
    }
}

/// Keys are stored as given; they are part of URLs and storage paths, so they are
/// checked rather than silently rewritten.
fn check_key(key: &str) -> Result<String, ProjectError> {
    let well_formed = (MIN_KEY_CHARS..=MAX_KEY_CHARS).contains(&key.len())
        && key.starts_with(|c: char| c.is_ascii_lowercase())
        && key.split('-').all(|part| {
            !part.is_empty()
                && part
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        });
    if well_formed {
        Ok(key.to_owned())
    } else {
        Err(ProjectError::InvalidKey)
    }
}

fn check_name(name: &str) -> Result<String, ProjectError> {
    let name = name.trim();
    if (1..=MAX_NAME_CHARS).contains(&name.chars().count()) {
        Ok(name.to_owned())
    } else {
        Err(ProjectError::InvalidName)
    }
}

fn check_description(description: &str) -> Result<String, ProjectError> {
    let description = description.trim();
    if description.chars().count() <= MAX_DESCRIPTION_CHARS {
        Ok(description.to_owned())
    } else {
        Err(ProjectError::InvalidDescription)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roles_include_the_ones_below() {
        assert!(Role::Owner.includes(Role::Editor));
        assert!(Role::Editor.includes(Role::Viewer));
        assert!(!Role::Viewer.includes(Role::Editor));
        assert!(!Role::Editor.includes(Role::Owner));
    }

    #[test]
    fn keys_are_url_safe() {
        for good in ["credit-scoring", "sme2", "ab", "a-1-b"] {
            assert!(check_key(good).is_ok(), "{good}");
        }
        let too_long = "a".repeat(41);
        for bad in [
            "a",
            "Credit",
            "1credit",
            "credit_scoring",
            "credit--scoring",
            "credit-",
            "-credit",
            "crédit",
            "credit scoring",
            too_long.as_str(),
        ] {
            assert!(check_key(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn names_are_trimmed_and_bounded() {
        assert_eq!(check_name("  Retail scoring ").unwrap(), "Retail scoring");
        assert!(check_name("   ").is_err());
        assert!(check_name(&"é".repeat(100)).is_ok());
        assert!(check_name(&"é".repeat(101)).is_err());
        assert!(check_description(&"x".repeat(1001)).is_err());
    }
}
