//! Packs (DNK-43): the catalogue an installation offers, new projects from a pack or a pack
//! file, copies of a project, and a project exported as a pack file.

use crate::auth::CurrentUser;
use crate::error::{ApiError, ErrorBody};
use crate::extract::{ApiJson, ProjectAccess};
use crate::routes::projects::{key_schema, name_schema, ProjectResponse};
use crate::routes::scenarios::TestSummaryResponse;
use crate::AppState;
use axum::body::Bytes;
use axum::extract::{Path, Query, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use donka_pack::{Imported, Localized, Pack, Source};
use donka_project::Role;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

/// Media type of a pack file.
const PACK_CONTENT_TYPE: &str = "application/zip";

#[derive(Serialize, ToSchema)]
pub struct LocalizedText {
    pub en: String,
    pub fr: String,
}

impl From<&Localized> for LocalizedText {
    fn from(text: &Localized) -> Self {
        Self {
            en: text.en.clone(),
            fr: text.fr.clone(),
        }
    }
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PackResponse {
    pub key: String,
    pub name: LocalizedText,
    pub description: LocalizedText,
    /// The pack's own version, e.g. `1.2`.
    pub version: Option<String>,
    /// ISO country codes of the market the policy was written for, e.g. `CM`.
    pub market: Option<String>,
    /// ISO currency code of the amounts, e.g. `XAF`.
    pub currency: Option<String>,
    /// The decision a caller asks for.
    pub entry: Option<String>,
    /// Decision keys, a decision another one calls before it.
    pub decisions: Vec<String>,
    /// Test scenarios the pack carries.
    pub scenarios: usize,
}

impl From<&Pack> for PackResponse {
    fn from(pack: &Pack) -> Self {
        let manifest = &pack.manifest;
        Self {
            key: manifest.key.clone(),
            name: (&manifest.name).into(),
            description: (&manifest.description).into(),
            version: manifest.version.clone(),
            market: manifest.market.clone(),
            currency: manifest.currency.clone(),
            entry: manifest.entry.clone(),
            decisions: manifest.decisions.clone(),
            scenarios: pack.scenarios.len(),
        }
    }
}

#[derive(Serialize, ToSchema)]
pub struct PackListResponse {
    pub items: Vec<PackResponse>,
}

/// The key and name of the new project.
#[derive(Deserialize, ToSchema, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct NewProjectFields {
    /// Lowercase letters, digits and single hyphens, starting with a letter. Cannot change later.
    #[schema(schema_with = key_schema)]
    pub key: String,
    #[schema(schema_with = name_schema)]
    pub name: String,
}

#[derive(Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateProjectRequest {
    #[schema(schema_with = key_schema)]
    pub key: String,
    #[schema(schema_with = name_schema)]
    pub name: String,
    /// Copy the versions this release froze; each decision's current draft when absent.
    pub release_id: Option<Uuid>,
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
#[serde(rename_all = "camelCase")]
pub struct ExportQuery {
    /// Export the versions this release froze; each decision's current draft when absent.
    pub release_id: Option<Uuid>,
}

#[derive(Serialize, ToSchema)]
pub struct ImportedResponse {
    pub project: ProjectResponse,
    /// How the scenarios went on the new project's first versions.
    pub tests: TestSummaryResponse,
}

impl From<Imported> for ImportedResponse {
    fn from(imported: Imported) -> Self {
        Self {
            project: ProjectResponse::new(imported.project, Role::Owner),
            tests: imported.tests.into(),
        }
    }
}

fn source(release_id: Option<Uuid>) -> Source {
    release_id.map_or(Source::Drafts, Source::Release)
}

/// The packs this installation offers (`DONKA_PACKS_DIR`), by key.
#[utoipa::path(
    get,
    path = "/packs",
    tag = "packs",
    responses(
        (status = 200, body = PackListResponse),
        (status = 401, description = "Not signed in (UNAUTHENTICATED)", body = ErrorBody),
    )
)]
pub async fn list(State(state): State<AppState>) -> Json<PackListResponse> {
    Json(PackListResponse {
        items: state.catalogue.packs().iter().map(Into::into).collect(),
    })
}

/// Reads a pack file and says what it holds, without importing it.
#[utoipa::path(
    post,
    path = "/packs/inspect",
    tag = "packs",
    request_body(content_type = "application/zip", description = "A pack file (zip, at most 5 MB)"),
    responses(
        (status = 200, body = PackResponse),
        (status = 413, description = "Larger than 5 MB (PACK_TOO_LARGE)", body = ErrorBody),
        (status = 422, description = "Not a usable pack; why in details.reason (INVALID_PACK)", body = ErrorBody),
    )
)]
pub async fn inspect(body: Bytes) -> Result<Json<PackResponse>, ApiError> {
    let pack = Pack::from_zip(&body)?;
    Ok(Json((&pack).into()))
}

/// A new project from a pack of the catalogue (administrators). The importer becomes its
/// only owner; the decisions get a first version, which runs the pack's scenarios.
#[utoipa::path(
    post,
    path = "/packs/{key}/import",
    tag = "packs",
    params(("key" = String, Path, description = "The pack's key, e.g. `retail-credit`.")),
    request_body = NewProjectFields,
    responses(
        (status = 201, body = ImportedResponse),
        (status = 400, description = "Invalid key or name (INVALID_REQUEST)", body = ErrorBody),
        (status = 403, description = "Not an administrator (FORBIDDEN)", body = ErrorBody),
        (status = 404, description = "No pack with this key (PACK_NOT_FOUND)", body = ErrorBody),
        (status = 409, description = "Key already used (PROJECT_KEY_TAKEN)", body = ErrorBody),
    )
)]
pub async fn import(
    State(state): State<AppState>,
    Extension(current): Extension<CurrentUser>,
    Path(key): Path<String>,
    ApiJson(req): ApiJson<NewProjectFields>,
) -> Result<(StatusCode, Json<ImportedResponse>), ApiError> {
    let pack = state.catalogue.get(&key)?;
    let imported = state
        .packs
        .import(&current.user, pack, &req.key, &req.name)
        .await?;
    Ok((StatusCode::CREATED, Json(imported.into())))
}

/// A new project from a pack file (administrators), e.g. one exported by another
/// installation. The key and name of the new project are in the query string.
#[utoipa::path(
    post,
    path = "/packs/import",
    tag = "packs",
    params(NewProjectFields),
    request_body(content_type = "application/zip", description = "A pack file (zip, at most 5 MB)"),
    responses(
        (status = 201, body = ImportedResponse),
        (status = 400, description = "Invalid key or name (INVALID_REQUEST)", body = ErrorBody),
        (status = 403, description = "Not an administrator (FORBIDDEN)", body = ErrorBody),
        (status = 409, description = "Key already used (PROJECT_KEY_TAKEN)", body = ErrorBody),
        (status = 413, description = "Larger than 5 MB (PACK_TOO_LARGE)", body = ErrorBody),
        (status = 422, description = "Not a usable pack; why in details.reason (INVALID_PACK)", body = ErrorBody),
    )
)]
pub async fn import_file(
    State(state): State<AppState>,
    Extension(current): Extension<CurrentUser>,
    Query(fields): Query<NewProjectFields>,
    body: Bytes,
) -> Result<(StatusCode, Json<ImportedResponse>), ApiError> {
    if !current.user.is_admin {
        return Err(ApiError::Forbidden);
    }
    let pack = Pack::from_zip(&body)?;
    let imported = state
        .packs
        .import(&current.user, &pack, &fields.key, &fields.name)
        .await?;
    Ok((StatusCode::CREATED, Json(imported.into())))
}

/// A new project with this project's decisions, scenarios and decision-log settings
/// (administrators who are members). Releases, tokens, records and members stay behind.
#[utoipa::path(
    post,
    path = "/projects/{project_id}/duplicate",
    tag = "packs",
    params(("project_id" = Uuid, Path)),
    request_body = DuplicateProjectRequest,
    responses(
        (status = 201, body = ImportedResponse),
        (status = 400, description = "Invalid key or name (INVALID_REQUEST)", body = ErrorBody),
        (status = 403, description = "Not an administrator (FORBIDDEN)", body = ErrorBody),
        (status = 404, description = "PROJECT_NOT_FOUND or RELEASE_NOT_FOUND", body = ErrorBody),
        (status = 409, description = "Key already used (PROJECT_KEY_TAKEN)", body = ErrorBody),
        (status = 422, description = "Nothing to copy (INVALID_PACK)", body = ErrorBody),
    )
)]
pub async fn duplicate(
    State(state): State<AppState>,
    Extension(current): Extension<CurrentUser>,
    ProjectAccess(access): ProjectAccess,
    ApiJson(req): ApiJson<DuplicateProjectRequest>,
) -> Result<(StatusCode, Json<ImportedResponse>), ApiError> {
    let imported = state
        .packs
        .duplicate(
            &current.user,
            &access,
            source(req.release_id),
            &req.key,
            &req.name,
        )
        .await?;
    Ok((StatusCode::CREATED, Json(imported.into())))
}

/// The project as a pack file another team or installation imports (owners).
#[utoipa::path(
    get,
    path = "/projects/{project_id}/export",
    tag = "packs",
    params(("project_id" = Uuid, Path), ExportQuery),
    responses(
        (status = 200, description = "The pack file, `<key>.donka-pack.zip`", content_type = "application/zip"),
        (status = 403, description = "Not an owner (FORBIDDEN)", body = ErrorBody),
        (status = 404, description = "PROJECT_NOT_FOUND or RELEASE_NOT_FOUND", body = ErrorBody),
        (status = 422, description = "Nothing to export (INVALID_PACK)", body = ErrorBody),
    )
)]
pub async fn export(
    State(state): State<AppState>,
    ProjectAccess(access): ProjectAccess,
    Query(query): Query<ExportQuery>,
) -> Result<Response, ApiError> {
    let pack = state
        .packs
        .export(&access, source(query.release_id))
        .await?;
    let zip = pack.to_zip()?;
    let filename = format!("{}.donka-pack.zip", pack.manifest.key);
    Ok((
        [
            (header::CONTENT_TYPE, PACK_CONTENT_TYPE.to_owned()),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{filename}\""),
            ),
        ],
        zip,
    )
        .into_response())
}
