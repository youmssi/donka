use crate::auth::CurrentUser;
use crate::error::{ApiError, ErrorBody};
use crate::extract::{ApiJson, MemberId, ProjectAccess};
use crate::AppState;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::{Extension, Json};
use chrono::{DateTime, Utc};
use donka_project::{
    Listing, Member, Project, ProjectSummary, Role, KEY_PATTERN, MAX_DESCRIPTION_CHARS,
    MAX_KEY_CHARS, MAX_NAME_CHARS, MIN_KEY_CHARS,
};
use donka_shared::page::PageRequest;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProjectResponse {
    pub id: Uuid,
    /// Immutable, URL-safe identifier.
    pub key: String,
    pub name: String,
    pub description: String,
    pub created_at: DateTime<Utc>,
    /// Set when the project is archived (read-only).
    pub archived_at: Option<DateTime<Utc>>,
    /// The reader's role in the project.
    pub role: Role,
}

impl ProjectResponse {
    pub(crate) fn new(project: Project, role: Role) -> Self {
        Self {
            id: project.id,
            key: project.key,
            name: project.name,
            description: project.description,
            created_at: project.created_at,
            archived_at: project.archived_at,
            role,
        }
    }
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSummaryResponse {
    pub id: Uuid,
    pub key: String,
    pub name: String,
    pub description: String,
    pub archived_at: Option<DateTime<Utc>>,
    pub role: Role,
}

impl From<ProjectSummary> for ProjectSummaryResponse {
    fn from(project: ProjectSummary) -> Self {
        Self {
            id: project.id,
            key: project.key,
            name: project.name,
            description: project.description,
            archived_at: project.archived_at,
            role: project.role,
        }
    }
}

#[derive(Serialize, ToSchema)]
pub struct ProjectListResponse {
    pub items: Vec<ProjectSummaryResponse>,
    /// Projects matching the filter, across all pages.
    pub total: i64,
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ListProjectsQuery {
    /// `true` lists archived projects instead of active ones.
    #[serde(default)]
    pub archived: bool,
    /// Page size, 1 to 100 (default 50).
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Deserialize, ToSchema)]
pub struct CreateProjectRequest {
    /// Lowercase letters, digits and single hyphens, starting with a letter. Cannot change later.
    #[schema(schema_with = key_schema)]
    pub key: String,
    #[schema(schema_with = name_schema)]
    pub name: String,
    #[serde(default)]
    #[schema(schema_with = description_schema)]
    pub description: String,
}

#[derive(Deserialize, ToSchema)]
pub struct UpdateProjectRequest {
    #[schema(schema_with = name_schema)]
    pub name: String,
    #[serde(default)]
    #[schema(schema_with = description_schema)]
    pub description: String,
}

// The project rules, published so the web app validates with the same limits.

pub(crate) fn key_schema() -> utoipa::openapi::Object {
    string_schema(MIN_KEY_CHARS, MAX_KEY_CHARS)
        .pattern(Some(KEY_PATTERN))
        .build()
}

pub(crate) fn name_schema() -> utoipa::openapi::Object {
    string_schema(1, MAX_NAME_CHARS).build()
}

fn description_schema() -> utoipa::openapi::Object {
    string_schema(0, MAX_DESCRIPTION_CHARS).build()
}

fn string_schema(min: usize, max: usize) -> utoipa::openapi::ObjectBuilder {
    utoipa::openapi::ObjectBuilder::new()
        .schema_type(utoipa::openapi::schema::Type::String)
        .min_length(Some(min))
        .max_length(Some(max))
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MemberResponse {
    pub user_id: Uuid,
    pub email: String,
    pub role: Role,
    pub added_at: DateTime<Utc>,
}

#[derive(Serialize, ToSchema)]
pub struct MemberListResponse {
    pub items: Vec<MemberResponse>,
}

#[derive(Deserialize, ToSchema)]
pub struct AddMemberRequest {
    /// Email of an existing Studio account.
    pub email: String,
    pub role: Role,
}

#[derive(Deserialize, ToSchema)]
pub struct ChangeRoleRequest {
    pub role: Role,
}

/// Projects the signed-in user is a member of, by name.
#[utoipa::path(
    get,
    path = "/projects",
    tag = "projects",
    params(ListProjectsQuery),
    responses(
        (status = 200, body = ProjectListResponse),
        (status = 401, description = "Not signed in (UNAUTHENTICATED)", body = ErrorBody),
    )
)]
pub async fn list(
    State(state): State<AppState>,
    Extension(current): Extension<CurrentUser>,
    Query(query): Query<ListProjectsQuery>,
) -> Result<Json<ProjectListResponse>, ApiError> {
    let listing = if query.archived {
        Listing::Archived
    } else {
        Listing::Active
    };
    let page = state
        .projects
        .list_for(
            current.user.id,
            listing,
            PageRequest::new(query.limit, query.offset),
        )
        .await?;
    Ok(Json(ProjectListResponse {
        items: page.items.into_iter().map(Into::into).collect(),
        total: page.total,
    }))
}

/// Creates a project (administrators only). The creator becomes its owner.
#[utoipa::path(
    post,
    path = "/projects",
    tag = "projects",
    request_body = CreateProjectRequest,
    responses(
        (status = 201, body = ProjectResponse),
        (status = 400, description = "Invalid key, name or description (INVALID_REQUEST)", body = ErrorBody),
        (status = 403, description = "Not an administrator (FORBIDDEN)", body = ErrorBody),
        (status = 409, description = "Key already used (PROJECT_KEY_TAKEN)", body = ErrorBody),
    )
)]
pub async fn create(
    State(state): State<AppState>,
    Extension(current): Extension<CurrentUser>,
    ApiJson(req): ApiJson<CreateProjectRequest>,
) -> Result<(StatusCode, Json<ProjectResponse>), ApiError> {
    if !current.user.is_admin {
        return Err(ApiError::Forbidden);
    }
    let project = state
        .projects
        .create(current.user.id, &req.key, &req.name, &req.description)
        .await?;
    Ok((
        StatusCode::CREATED,
        Json(ProjectResponse::new(project, Role::Owner)),
    ))
}

/// A project the signed-in user is a member of.
#[utoipa::path(
    get,
    path = "/projects/{project_id}",
    tag = "projects",
    params(("project_id" = Uuid, Path)),
    responses(
        (status = 200, body = ProjectResponse),
        (status = 404, description = "No such project, or not a member (PROJECT_NOT_FOUND)", body = ErrorBody),
    )
)]
pub async fn get(
    State(state): State<AppState>,
    ProjectAccess(access): ProjectAccess,
) -> Result<Json<ProjectResponse>, ApiError> {
    let project = state.projects.get(&access).await?;
    Ok(Json(ProjectResponse::new(project, access.role())))
}

/// A project the signed-in user is a member of, found by its key: the web app
/// addresses projects by key so its links stay short and readable.
#[utoipa::path(
    get,
    path = "/projects/by-key/{key}",
    tag = "projects",
    params(("key" = String, Path, description = "The project's key, e.g. `credit-pme`.")),
    responses(
        (status = 200, body = ProjectResponse),
        (status = 404, description = "No such project, or not a member (PROJECT_NOT_FOUND)", body = ErrorBody),
    )
)]
pub async fn get_by_key(
    State(state): State<AppState>,
    Extension(current): Extension<CurrentUser>,
    Path(key): Path<String>,
) -> Result<Json<ProjectResponse>, ApiError> {
    let access = state.projects.access_by_key(current.user.id, &key).await?;
    let project = state.projects.get(&access).await?;
    Ok(Json(ProjectResponse::new(project, access.role())))
}

/// Renames the project or changes its description (owners). The key never changes.
#[utoipa::path(
    patch,
    path = "/projects/{project_id}",
    tag = "projects",
    params(("project_id" = Uuid, Path)),
    request_body = UpdateProjectRequest,
    responses(
        (status = 200, body = ProjectResponse),
        (status = 400, description = "Invalid name or description (INVALID_REQUEST)", body = ErrorBody),
        (status = 403, description = "Not an owner (FORBIDDEN)", body = ErrorBody),
        (status = 404, description = "PROJECT_NOT_FOUND", body = ErrorBody),
        (status = 409, description = "Archived (PROJECT_ARCHIVED)", body = ErrorBody),
    )
)]
pub async fn update(
    State(state): State<AppState>,
    ProjectAccess(access): ProjectAccess,
    ApiJson(req): ApiJson<UpdateProjectRequest>,
) -> Result<Json<ProjectResponse>, ApiError> {
    let project = state
        .projects
        .update_details(&access, &req.name, &req.description)
        .await?;
    Ok(Json(ProjectResponse::new(project, access.role())))
}

/// Archives the project: read-only and hidden from the default list (owners).
#[utoipa::path(
    post,
    path = "/projects/{project_id}/archive",
    tag = "projects",
    params(("project_id" = Uuid, Path)),
    responses(
        (status = 200, body = ProjectResponse),
        (status = 403, description = "Not an owner (FORBIDDEN)", body = ErrorBody),
        (status = 404, description = "PROJECT_NOT_FOUND", body = ErrorBody),
    )
)]
pub async fn archive(
    State(state): State<AppState>,
    ProjectAccess(access): ProjectAccess,
) -> Result<Json<ProjectResponse>, ApiError> {
    let project = state.projects.set_archived(&access, true).await?;
    Ok(Json(ProjectResponse::new(project, access.role())))
}

/// Restores an archived project (owners).
#[utoipa::path(
    post,
    path = "/projects/{project_id}/restore",
    tag = "projects",
    params(("project_id" = Uuid, Path)),
    responses(
        (status = 200, body = ProjectResponse),
        (status = 403, description = "Not an owner (FORBIDDEN)", body = ErrorBody),
        (status = 404, description = "PROJECT_NOT_FOUND", body = ErrorBody),
    )
)]
pub async fn restore(
    State(state): State<AppState>,
    ProjectAccess(access): ProjectAccess,
) -> Result<Json<ProjectResponse>, ApiError> {
    let project = state.projects.set_archived(&access, false).await?;
    Ok(Json(ProjectResponse::new(project, access.role())))
}

/// Members of the project, owners first, then by email (any member).
#[utoipa::path(
    get,
    path = "/projects/{project_id}/members",
    tag = "projects",
    params(("project_id" = Uuid, Path)),
    responses(
        (status = 200, body = MemberListResponse),
        (status = 404, description = "PROJECT_NOT_FOUND", body = ErrorBody),
    )
)]
pub async fn members(
    State(state): State<AppState>,
    ProjectAccess(access): ProjectAccess,
) -> Result<Json<MemberListResponse>, ApiError> {
    let members = state.projects.members(&access).await?;
    let items = with_emails(&state, members).await?;
    Ok(Json(MemberListResponse { items }))
}

/// Adds an existing Studio user to the project (owners).
#[utoipa::path(
    post,
    path = "/projects/{project_id}/members",
    tag = "projects",
    params(("project_id" = Uuid, Path)),
    request_body = AddMemberRequest,
    responses(
        (status = 201, body = MemberResponse),
        (status = 403, description = "Not an owner (FORBIDDEN)", body = ErrorBody),
        (status = 404, description = "PROJECT_NOT_FOUND", body = ErrorBody),
        (status = 409, description = "Already a member (ALREADY_MEMBER) or archived (PROJECT_ARCHIVED)", body = ErrorBody),
        (status = 422, description = "No account with this email (NO_SUCH_USER)", body = ErrorBody),
    )
)]
pub async fn add_member(
    State(state): State<AppState>,
    ProjectAccess(access): ProjectAccess,
    ApiJson(req): ApiJson<AddMemberRequest>,
) -> Result<(StatusCode, Json<MemberResponse>), ApiError> {
    // Checked before looking the email up, so non-owners learn nothing about accounts.
    access.require(Role::Owner)?;
    let user = state
        .identity
        .find_by_email(&req.email)
        .await?
        .ok_or(ApiError::NoSuchUser)?;
    let member = state
        .projects
        .add_member(&access, user.id, req.role)
        .await?;
    Ok((
        StatusCode::CREATED,
        Json(MemberResponse {
            user_id: member.user_id,
            email: user.email,
            role: member.role,
            added_at: member.added_at,
        }),
    ))
}

/// Changes a member's role; a project always keeps an owner (owners).
#[utoipa::path(
    patch,
    path = "/projects/{project_id}/members/{user_id}",
    tag = "projects",
    params(("project_id" = Uuid, Path), ("user_id" = Uuid, Path)),
    request_body = ChangeRoleRequest,
    responses(
        (status = 200, body = MemberResponse),
        (status = 403, description = "Not an owner (FORBIDDEN)", body = ErrorBody),
        (status = 404, description = "PROJECT_NOT_FOUND or MEMBER_NOT_FOUND", body = ErrorBody),
        (status = 409, description = "Last owner (LAST_OWNER) or archived (PROJECT_ARCHIVED)", body = ErrorBody),
    )
)]
pub async fn change_role(
    State(state): State<AppState>,
    ProjectAccess(access): ProjectAccess,
    MemberId(user): MemberId,
    ApiJson(req): ApiJson<ChangeRoleRequest>,
) -> Result<Json<MemberResponse>, ApiError> {
    let member = state.projects.change_role(&access, user, req.role).await?;
    let mut items = with_emails(&state, vec![member]).await?;
    items.pop().map(Json).ok_or(ApiError::MemberNotFound)
}

/// Removes a member; the last owner cannot be removed (owners).
#[utoipa::path(
    delete,
    path = "/projects/{project_id}/members/{user_id}",
    tag = "projects",
    params(("project_id" = Uuid, Path), ("user_id" = Uuid, Path)),
    responses(
        (status = 204, description = "Removed"),
        (status = 403, description = "Not an owner (FORBIDDEN)", body = ErrorBody),
        (status = 404, description = "PROJECT_NOT_FOUND or MEMBER_NOT_FOUND", body = ErrorBody),
        (status = 409, description = "Last owner (LAST_OWNER) or archived (PROJECT_ARCHIVED)", body = ErrorBody),
    )
)]
pub async fn remove_member(
    State(state): State<AppState>,
    ProjectAccess(access): ProjectAccess,
    MemberId(user): MemberId,
) -> Result<StatusCode, ApiError> {
    state.projects.remove_member(&access, user).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Joins members with their emails from the identity module (the two modules'
/// tables are never joined in SQL).
async fn with_emails(
    state: &AppState,
    members: Vec<Member>,
) -> Result<Vec<MemberResponse>, ApiError> {
    let ids: Vec<Uuid> = members.iter().map(|m| m.user_id).collect();
    let emails: HashMap<Uuid, String> = state
        .identity
        .users_by_ids(&ids)
        .await?
        .into_iter()
        .map(|user| (user.id, user.email))
        .collect();
    let mut items: Vec<MemberResponse> = members
        .into_iter()
        .filter_map(|m| {
            emails.get(&m.user_id).map(|email| MemberResponse {
                user_id: m.user_id,
                email: email.clone(),
                role: m.role,
                added_at: m.added_at,
            })
        })
        .collect();
    // Owners first, then editors, then viewers; alphabetical within a role.
    items.sort_by(|a, b| b.role.cmp(&a.role).then_with(|| a.email.cmp(&b.email)));
    Ok(items)
}
