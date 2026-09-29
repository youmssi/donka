//! People as responses name them: an id with the email to show.

use crate::error::ApiError;
use crate::AppState;
use serde::Serialize;
use std::collections::HashMap;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Serialize, ToSchema)]
pub struct PersonRef {
    pub id: Uuid,
    pub email: String,
}

/// The emails of these people (absent ids are skipped), in one lookup.
pub async fn emails(
    state: &AppState,
    ids: impl IntoIterator<Item = Option<Uuid>>,
) -> Result<HashMap<Uuid, String>, ApiError> {
    let mut ids: Vec<Uuid> = ids.into_iter().flatten().collect();
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

pub fn person(emails: &HashMap<Uuid, String>, id: Option<Uuid>) -> Option<PersonRef> {
    let id = id?;
    Some(PersonRef {
        id,
        email: emails.get(&id).cloned().unwrap_or_default(),
    })
}
