use crate::auth::CurrentUser;
use crate::error::{ApiError, ErrorBody};
use crate::extract::ApiJson;
use crate::routes::auth::UserResponse;
use crate::AppState;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::{Extension, Json};
use chrono::{DateTime, Utc};
use donka_identity::{Account, Locale};
use donka_shared::page::PageRequest;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

#[derive(Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct InvitationRequest {
    pub email: String,
    /// Language of the invitation email and of the account. Defaults to the inviter's.
    pub locale: Option<Locale>,
    #[serde(default)]
    pub is_admin: bool,
}

/// Invites a person by email (administrators only). Inviting someone who has not
/// accepted yet sends a new link and revokes the previous one.
#[utoipa::path(
    post,
    path = "/users/invitations",
    tag = "identity",
    request_body = InvitationRequest,
    responses(
        (status = 201, description = "Account created; the invitation email is on its way", body = UserResponse),
        (status = 400, description = "Invalid email (INVALID_REQUEST)", body = ErrorBody),
        (status = 401, description = "Not signed in (UNAUTHENTICATED)", body = ErrorBody),
        (status = 403, description = "Not an administrator (FORBIDDEN)", body = ErrorBody),
        (status = 409, description = "The person already has an account (EMAIL_TAKEN)", body = ErrorBody),
    )
)]
pub async fn invite(
    State(state): State<AppState>,
    Extension(current): Extension<CurrentUser>,
    ApiJson(req): ApiJson<InvitationRequest>,
) -> Result<(StatusCode, Json<UserResponse>), ApiError> {
    let locale = req.locale.unwrap_or(current.user.locale);
    let user = state
        .identity
        .invite(&current.user, &req.email, locale, req.is_admin)
        .await?;
    Ok((StatusCode::CREATED, Json(user.into())))
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AccountResponse {
    pub id: Uuid,
    pub email: String,
    pub is_admin: bool,
    pub locale: Locale,
    /// False while the invitation is pending (no password chosen yet).
    pub active: bool,
    pub created_at: DateTime<Utc>,
}

impl From<Account> for AccountResponse {
    fn from(account: Account) -> Self {
        Self {
            id: account.id,
            email: account.email,
            is_admin: account.is_admin,
            locale: account.locale,
            active: account.active,
            created_at: account.created_at,
        }
    }
}

#[derive(Serialize, ToSchema)]
pub struct AccountListResponse {
    pub items: Vec<AccountResponse>,
    /// Accounts across all pages.
    pub total: i64,
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ListAccountsQuery {
    /// Page size, 1 to 100 (default 50).
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

/// Every Studio account, by email (administrators only).
#[utoipa::path(
    get,
    path = "/users",
    tag = "identity",
    params(ListAccountsQuery),
    responses(
        (status = 200, body = AccountListResponse),
        (status = 401, description = "Not signed in (UNAUTHENTICATED)", body = ErrorBody),
        (status = 403, description = "Not an administrator (FORBIDDEN)", body = ErrorBody),
    )
)]
pub async fn list(
    State(state): State<AppState>,
    Extension(current): Extension<CurrentUser>,
    Query(query): Query<ListAccountsQuery>,
) -> Result<Json<AccountListResponse>, ApiError> {
    let page = state
        .identity
        .list_accounts(&current.user, PageRequest::new(query.limit, query.offset))
        .await?;
    Ok(Json(AccountListResponse {
        items: page.items.into_iter().map(Into::into).collect(),
        total: page.total,
    }))
}
