//! What the installation's licence allows (DNK-77), for the web app to hide modules and tell
//! administrators when the licence needs attention.

use crate::AppState;
use axum::extract::State;
use axum::Json;
use chrono::NaiveDate;
use donka_licence::{LicenceError, Module, Status, NOTICE_DAYS};
use serde::Serialize;
use utoipa::ToSchema;

/// Where the installation stands.
#[derive(Serialize, ToSchema, Clone, Copy)]
#[serde(rename_all = "camelCase")]
pub enum LicenceStatus {
    /// No licence file: everything works except deploying to production.
    Evaluation,
    Active,
    NotYetValid,
    Expired,
    /// A licence file is configured but cannot be trusted (`reason` says why).
    Refused,
}

/// An optional part of Studio a licence includes or not.
#[derive(Serialize, ToSchema, Clone, Copy)]
#[serde(rename_all = "camelCase")]
pub enum LicensedModule {
    Explain,
    Connectors,
    Analysis,
}

impl From<Module> for LicensedModule {
    fn from(module: Module) -> Self {
        match module {
            Module::Explain => Self::Explain,
            Module::Connectors => Self::Connectors,
            Module::Analysis => Self::Analysis,
        }
    }
}

/// Why a licence file was refused.
#[derive(Serialize, ToSchema, Clone, Copy)]
#[serde(rename_all = "camelCase")]
pub enum LicenceRefusal {
    /// Not a licence file this Studio reads, or a file it cannot read.
    Unreadable,
    /// Issued with another key than the one this Studio trusts.
    Foreign,
    /// Changed after it was signed.
    Tampered,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct LicenceResponse {
    pub status: LicenceStatus,
    /// Who the licence was issued to; absent in evaluation and when refused.
    pub customer: Option<String>,
    /// First and last valid days (UTC, both included).
    pub valid_from: Option<NaiveDate>,
    pub valid_until: Option<NaiveDate>,
    /// Valid days left after today, while active.
    pub days_left: Option<i64>,
    /// The licence ends within the notice period: administrators should renew it.
    pub expiring: bool,
    /// Days before the end at which `expiring` turns on.
    #[schema(example = 30)]
    pub notice_days: i64,
    /// Optional modules this installation offers: the licence includes them and they are on.
    pub modules: Vec<LicensedModule>,
    /// Modules the licence includes that the installation turned off (`DONKA_MODULES`,
    /// DNK-74); their data is kept.
    pub turned_off: Vec<LicensedModule>,
    /// New versions and releases can be made.
    pub allows_changes: bool,
    /// Releases can reach production (anything but evaluation).
    pub allows_production: bool,
    /// Why a licence file was refused, for the web app to say in the reader's language.
    pub refusal: Option<LicenceRefusal>,
    /// The same, in words (English), as Studio's log says it.
    pub reason: Option<String>,
}

/// What the installation's licence allows (any signed-in user).
#[utoipa::path(
    get,
    path = "/licence",
    tag = "operations",
    responses((status = 200, body = LicenceResponse))
)]
pub async fn get(State(state): State<AppState>) -> Json<LicenceResponse> {
    Json(response(&state.licensing.status(), |module| {
        state.licensing.turned_on(module)
    }))
}

fn response(status: &Status, turned_on: impl Fn(Module) -> bool) -> LicenceResponse {
    let licence = status.licence();
    LicenceResponse {
        status: match status {
            Status::Evaluation => LicenceStatus::Evaluation,
            Status::Active { .. } => LicenceStatus::Active,
            Status::NotYetValid { .. } => LicenceStatus::NotYetValid,
            Status::Expired { .. } => LicenceStatus::Expired,
            Status::Refused { .. } => LicenceStatus::Refused,
        },
        customer: licence.map(|l| l.customer.clone()),
        valid_from: licence.map(|l| l.valid_from),
        valid_until: licence.map(|l| l.valid_until),
        days_left: match status {
            Status::Active { days_left, .. } => Some(*days_left),
            _ => None,
        },
        expiring: status.expiring(),
        notice_days: NOTICE_DAYS,
        modules: Module::ALL
            .into_iter()
            .filter(|module| status.includes(*module) && turned_on(*module))
            .map(LicensedModule::from)
            .collect(),
        turned_off: Module::ALL
            .into_iter()
            .filter(|module| status.includes(*module) && !turned_on(*module))
            .map(LicensedModule::from)
            .collect(),
        allows_changes: status.allows_changes(),
        allows_production: status.allows_production(),
        refusal: match status {
            Status::Refused { reason } => Some(match reason {
                LicenceError::Foreign { .. } => LicenceRefusal::Foreign,
                LicenceError::Tampered => LicenceRefusal::Tampered,
                LicenceError::Unreadable(_)
                | LicenceError::Invalid(_)
                | LicenceError::InvalidKey(_) => LicenceRefusal::Unreadable,
            }),
            _ => None,
        },
        reason: match status {
            Status::Refused { reason } => Some(reason.to_string()),
            _ => None,
        },
    }
}
