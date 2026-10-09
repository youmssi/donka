//! First-run onboarding (DNK-41): the Get started checklist, read from what happened in the
//! signed-in user's projects, and the guided tours each user has seen.

use crate::auth::CurrentUser;
use crate::error::{ApiError, ErrorBody};
use crate::AppState;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::{Extension, Json};
use donka_identity::Tour;
use donka_project::ProjectSummary;
use serde::Serialize;
use utoipa::ToSchema;

/// Projects read for the checklist; a newcomer has a handful.
const MAX_PROJECTS: i64 = 100;

/// A step from an empty Studio to a decision a Runtime answers, in order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum Step {
    /// The user is a member of a project (an administrator imports a starter pack).
    Project,
    /// A decision of the project was simulated.
    Simulation,
    /// A decision has a saved version.
    Version,
    /// A release is live on staging.
    Staging,
    /// A Runtime token opens one of the project's environments.
    Token,
}

#[derive(Serialize, ToSchema)]
pub struct StepResponse {
    pub step: Step,
    pub done: bool,
}

#[derive(Serialize, ToSchema)]
pub struct ChecklistProject {
    pub key: String,
    pub name: String,
}

#[derive(Serialize, ToSchema)]
pub struct OnboardingResponse {
    /// Every step, in order.
    pub steps: Vec<StepResponse>,
    /// The project the steps lead to: the one furthest along; none without a project.
    pub project: Option<ChecklistProject>,
    /// Every step is done: the checklist is not shown again.
    pub complete: bool,
}

#[derive(Serialize, ToSchema)]
pub struct ToursResponse {
    /// Tours the user saw or skipped, which do not start on their own again.
    pub seen: Vec<Tour>,
}

/// How far the signed-in user got, over their active projects.
#[utoipa::path(
    get,
    path = "/onboarding",
    tag = "onboarding",
    responses(
        (status = 200, body = OnboardingResponse),
        (status = 401, description = "Not signed in (UNAUTHENTICATED)", body = ErrorBody),
    )
)]
pub async fn checklist(
    State(state): State<AppState>,
    Extension(current): Extension<CurrentUser>,
) -> Result<Json<OnboardingResponse>, ApiError> {
    let memberships = state
        .projects
        .memberships(current.user.id, MAX_PROJECTS)
        .await?;
    let accesses: Vec<_> = memberships.iter().map(|(access, _)| *access).collect();
    let decisions = state.decisions.progress(&accesses).await?;
    let releases = state.releases.progress(&accesses).await?;

    // The first project, by name, with the most steps done: where the rest is quickest.
    let mut furthest: Option<(&ProjectSummary, [bool; 4])> = None;
    for (_, project) in &memberships {
        let done = [
            decisions.simulated.contains(&project.id),
            decisions.versioned.contains(&project.id),
            releases.live_on_staging.contains(&project.id),
            releases.with_tokens.contains(&project.id),
        ];
        let count = |steps: &[bool; 4]| steps.iter().filter(|done| **done).count();
        if furthest.is_none_or(|(_, best)| count(&done) > count(&best)) {
            furthest = Some((project, done));
        }
    }

    let after_project = furthest.map(|(_, done)| done).unwrap_or_default();
    let steps: Vec<StepResponse> = [
        (Step::Project, furthest.is_some()),
        (Step::Simulation, after_project[0]),
        (Step::Version, after_project[1]),
        (Step::Staging, after_project[2]),
        (Step::Token, after_project[3]),
    ]
    .into_iter()
    .map(|(step, done)| StepResponse { step, done })
    .collect();
    let complete = steps.iter().all(|step| step.done);
    Ok(Json(OnboardingResponse {
        steps,
        project: furthest.map(|(project, _)| ChecklistProject {
            key: project.key.clone(),
            name: project.name.clone(),
        }),
        complete,
    }))
}

/// The guided tours the signed-in user has seen.
#[utoipa::path(
    get,
    path = "/me/tours",
    tag = "onboarding",
    responses(
        (status = 200, body = ToursResponse),
        (status = 401, description = "Not signed in (UNAUTHENTICATED)", body = ErrorBody),
    )
)]
pub async fn tours(
    State(state): State<AppState>,
    Extension(current): Extension<CurrentUser>,
) -> Result<Json<ToursResponse>, ApiError> {
    let seen = state.identity.tours_seen(current.user.id).await?;
    Ok(Json(ToursResponse { seen }))
}

/// Notes that the signed-in user finished or skipped a tour, so it does not start again.
#[utoipa::path(
    put,
    path = "/me/tours/{tour}",
    tag = "onboarding",
    params(("tour" = Tour, Path)),
    responses(
        (status = 204, description = "Noted"),
        (status = 400, description = "Not a tour (INVALID_REQUEST)", body = ErrorBody),
        (status = 401, description = "Not signed in (UNAUTHENTICATED)", body = ErrorBody),
    )
)]
pub async fn tour_seen(
    State(state): State<AppState>,
    Extension(current): Extension<CurrentUser>,
    Path(tour): Path<String>,
) -> Result<StatusCode, ApiError> {
    let tour: Tour = tour.parse().map_err(ApiError::InvalidRequest)?;
    state.identity.mark_tour_seen(current.user.id, tour).await?;
    Ok(StatusCode::NO_CONTENT)
}
