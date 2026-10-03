//! The one error shape of the API: `{ message, code, requestId, fields?, details? }`.
//!
//! Handlers return `ApiError`; this module is the only place that maps an error
//! to a status code, and it never exposes internal detail on a 500.

use crate::request_id;
use axum::extract::rejection::JsonRejection;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use donka_decision::{DecisionError, MAX_SCENARIO_NAME_CHARS};
use donka_decision_log::DecisionLogError;
use donka_engine::RuntimeError;
use donka_explain::ExplainError;
use donka_identity::IdentityError;
use donka_project::ProjectError;
use donka_release::{ReleaseError, MAX_NOTES_CHARS, MAX_TOKEN_NAME_CHARS};
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;
use utoipa::ToSchema;

/// Rejections from the JSON extractor can quote parts of the body; keep them short.
const MAX_REJECTION_LEN: usize = 300;

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ErrorBody {
    /// Safe to show to a person.
    pub message: String,
    /// Stable, machine-readable code, e.g. `DECISION_NOT_FOUND`.
    pub code: &'static str,
    /// Same value as the `x-request-id` response header; quote it when reporting a problem.
    pub request_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fields: Option<BTreeMap<String, String>>,
    /// Engine error document (failing node and reason) for evaluation failures.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<Value>,
}

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("invalid request: {0}")]
    InvalidRequest(String),
    #[error("decision '{0}' not found")]
    DecisionNotFound(String),
    #[error("decision '{key}' is invalid: {message}")]
    InvalidDecision { key: String, message: String },
    #[error("evaluation failed")]
    EvaluationFailed(Value),
    #[error("decision missing")]
    DecisionMissing,
    #[error("decision key already used")]
    DecisionKeyTaken,
    /// Someone saved the draft since it was loaded; `details` says who and when.
    #[error("draft conflict")]
    DecisionConflict(Value),
    #[error("version not found")]
    VersionNotFound,
    #[error("nothing changed since version {0}")]
    VersionUnchanged(i32),
    #[error("scenario not found")]
    ScenarioNotFound,
    #[error("scenario name already used")]
    ScenarioNameTaken,
    #[error("release not found")]
    ReleaseNotFound,
    #[error("deployment not found")]
    DeploymentNotFound,
    #[error("token not found")]
    TokenNotFound,
    #[error("environment not found")]
    EnvironmentNotFound,
    /// Decisions that need a version before a release (their keys in details).
    #[error("decisions without a version")]
    UnversionedDecisions(Vec<String>),
    #[error("nothing to release")]
    NothingToRelease,
    #[error("production needs approval")]
    ApprovalRequired,
    #[error("release version taken")]
    ReleaseConflict(String),
    #[error("deployment not retryable")]
    NotRetryable,
    #[error("approval not found")]
    ApprovalNotFound,
    #[error("release not live on staging")]
    NotOnStaging,
    #[error("no owner can approve")]
    NoApprover,
    #[error("an approval is pending")]
    ApprovalPending,
    #[error("self approval")]
    SelfApproval,
    #[error("approval already decided")]
    AlreadyDecided,
    #[error("not the requester")]
    NotRequester,
    #[error("release never approved for production")]
    NeverApproved,
    #[error("release already live")]
    AlreadyLive,
    #[error("decision record not found")]
    RecordNotFound,
    #[error("invalid decision-log token")]
    InvalidLogToken,
    #[error("invalid CI token")]
    InvalidCiToken,
    #[error("too many records in one batch")]
    BatchTooLarge,
    #[error("the explanation service declined")]
    ExplainDeclined,
    #[error("explanation service: {0}")]
    ExplainUnavailable(String),
    #[error("database unavailable")]
    DatabaseUnavailable,
    #[error("no such endpoint")]
    RouteNotFound,
    #[error("not signed in")]
    Unauthenticated,
    #[error("invalid credentials")]
    InvalidCredentials,
    #[error("missing CSRF header")]
    CsrfRequired,
    #[error("invalid password-setup link")]
    InvalidSetupLink,
    #[error("forbidden")]
    Forbidden,
    #[error("email already used")]
    EmailTaken,
    #[error("project not found")]
    ProjectNotFound,
    #[error("project key already used")]
    ProjectKeyTaken,
    #[error("project archived")]
    ProjectArchived,
    #[error("already a member")]
    AlreadyMember,
    #[error("member not found")]
    MemberNotFound,
    #[error("last owner")]
    LastOwner,
    #[error("no user with this email")]
    NoSuchUser,
    #[error("invalid field {field}: {message}")]
    InvalidField {
        field: &'static str,
        message: String,
    },
    #[error("internal error: {0}")]
    Internal(String),
}

impl From<DecisionError> for ApiError {
    fn from(err: DecisionError) -> Self {
        match err {
            DecisionError::NotFound => Self::DecisionMissing,
            DecisionError::InvalidKey => Self::InvalidField {
                field: "key",
                message: err.to_string(),
            },
            DecisionError::KeyTaken => Self::DecisionKeyTaken,
            DecisionError::InvalidContent { key, message } => {
                Self::InvalidDecision { key, message }
            }
            // The save route adds the email of who saved (routes::decisions::save).
            DecisionError::Conflict { current } => Self::DecisionConflict(serde_json::json!({
                "revision": current.revision,
                "updatedAt": current.updated_at,
                "updatedBy": { "id": current.updated_by },
            })),
            DecisionError::VersionNotFound => Self::VersionNotFound,
            DecisionError::InvalidMessage => Self::InvalidField {
                field: "message",
                message: err.to_string(),
            },
            DecisionError::Unchanged(version) => Self::VersionUnchanged(version),
            DecisionError::ScenarioNotFound => Self::ScenarioNotFound,
            DecisionError::ScenarioNameTaken => Self::ScenarioNameTaken,
            DecisionError::InvalidScenario(field) => Self::InvalidField {
                field,
                message: match field {
                    "name" => format!("1 to {MAX_SCENARIO_NAME_CHARS} characters"),
                    _ => "must be a JSON object".to_owned(),
                },
            },
            DecisionError::Project(err) => err.into(),
            DecisionError::Database(err) => Self::Internal(err.to_string()),
        }
    }
}

impl From<ReleaseError> for ApiError {
    fn from(err: ReleaseError) -> Self {
        match err {
            ReleaseError::NotFound => Self::ReleaseNotFound,
            ReleaseError::DeploymentNotFound => Self::DeploymentNotFound,
            ReleaseError::TokenNotFound => Self::TokenNotFound,
            ReleaseError::InvalidCiToken => Self::InvalidCiToken,
            ReleaseError::Artifact(err) => Self::Internal(err),
            ReleaseError::Unversioned(keys) => Self::UnversionedDecisions(keys),
            ReleaseError::NothingToRelease => Self::NothingToRelease,
            ReleaseError::InvalidNotes => Self::InvalidField {
                field: "notes",
                message: format!("1 to {MAX_NOTES_CHARS} characters"),
            },
            ReleaseError::InvalidTokenName => Self::InvalidField {
                field: "name",
                message: format!("1 to {MAX_TOKEN_NAME_CHARS} characters"),
            },
            ReleaseError::ApprovalRequired => Self::ApprovalRequired,
            ReleaseError::VersionTaken(version) => Self::ReleaseConflict(version.to_string()),
            ReleaseError::NotRetryable => Self::NotRetryable,
            ReleaseError::ApprovalNotFound => Self::ApprovalNotFound,
            ReleaseError::NotOnStaging => Self::NotOnStaging,
            ReleaseError::NoApprover => Self::NoApprover,
            ReleaseError::ApprovalPending => Self::ApprovalPending,
            ReleaseError::SelfApproval => Self::SelfApproval,
            ReleaseError::AlreadyDecided => Self::AlreadyDecided,
            ReleaseError::NotRequester => Self::NotRequester,
            ReleaseError::NeverApproved => Self::NeverApproved,
            ReleaseError::AlreadyLive => Self::AlreadyLive,
            ReleaseError::InvalidReason => Self::InvalidField {
                field: "reason",
                message: format!("Use 1 to {} characters.", donka_release::MAX_REASON_CHARS),
            },
            ReleaseError::Random => Self::Internal("random token could not be generated".into()),
            ReleaseError::Project(err) => err.into(),
            ReleaseError::Decision(err) => err.into(),
            ReleaseError::Database(err) => Self::Internal(err.to_string()),
        }
    }
}

impl From<ExplainError> for ApiError {
    fn from(err: ExplainError) -> Self {
        match err {
            ExplainError::Declined => Self::ExplainDeclined,
            other => Self::ExplainUnavailable(other.to_string()),
        }
    }
}

impl From<DecisionLogError> for ApiError {
    fn from(err: DecisionLogError) -> Self {
        match err {
            DecisionLogError::NotAdministrator => Self::Forbidden,
            DecisionLogError::InvalidTokenName => Self::InvalidField {
                field: "name",
                message: err.to_string(),
            },
            DecisionLogError::TokenNotFound => Self::TokenNotFound,
            DecisionLogError::InvalidToken => Self::InvalidLogToken,
            DecisionLogError::BatchTooLarge => Self::BatchTooLarge,
            DecisionLogError::InvalidOutcomeField => Self::InvalidField {
                field: "outcomeField",
                message: err.to_string(),
            },
            DecisionLogError::InvalidRedactedFields => Self::InvalidField {
                field: "redactedFields",
                message: err.to_string(),
            },
            DecisionLogError::RecordNotFound => Self::RecordNotFound,
            DecisionLogError::Unreadable(err) => Self::Internal(err.to_string()),
            DecisionLogError::Random => {
                Self::Internal("random token could not be generated".into())
            }
            DecisionLogError::Project(err) => err.into(),
            DecisionLogError::Release(err) => err.into(),
            DecisionLogError::Database(err) => Self::Internal(err.to_string()),
        }
    }
}

impl From<RuntimeError> for ApiError {
    fn from(err: RuntimeError) -> Self {
        match err {
            RuntimeError::NotFound(key) => Self::DecisionNotFound(key),
            RuntimeError::InvalidContent { key, message } => Self::InvalidDecision { key, message },
            RuntimeError::Evaluation { details } => Self::EvaluationFailed(details),
            RuntimeError::Internal(message) => Self::Internal(message),
        }
    }
}

impl From<IdentityError> for ApiError {
    fn from(err: IdentityError) -> Self {
        match err {
            IdentityError::InvalidCredentials => Self::InvalidCredentials,
            IdentityError::Unauthenticated => Self::Unauthenticated,
            IdentityError::InvalidSetupLink => Self::InvalidSetupLink,
            IdentityError::Forbidden => Self::Forbidden,
            IdentityError::EmailTaken => Self::EmailTaken,
            IdentityError::WeakPassword => Self::InvalidField {
                field: "password",
                message: IdentityError::WeakPassword.to_string(),
            },
            IdentityError::InvalidEmail => Self::InvalidField {
                field: "email",
                message: IdentityError::InvalidEmail.to_string(),
            },
            IdentityError::Database(err) => Self::Internal(err.to_string()),
            IdentityError::Internal(message) => Self::Internal(message),
        }
    }
}

impl From<ProjectError> for ApiError {
    fn from(err: ProjectError) -> Self {
        let field = |field: &'static str, err: &ProjectError| Self::InvalidField {
            field,
            message: err.to_string(),
        };
        match err {
            ProjectError::NotFound => Self::ProjectNotFound,
            ProjectError::Forbidden => Self::Forbidden,
            ProjectError::KeyTaken => Self::ProjectKeyTaken,
            ProjectError::InvalidKey => field("key", &err),
            ProjectError::InvalidName => field("name", &err),
            ProjectError::InvalidDescription => field("description", &err),
            ProjectError::Archived => Self::ProjectArchived,
            ProjectError::AlreadyMember => Self::AlreadyMember,
            ProjectError::MemberNotFound => Self::MemberNotFound,
            ProjectError::LastOwner => Self::LastOwner,
            ProjectError::Database(err) => Self::Internal(err.to_string()),
        }
    }
}

impl From<JsonRejection> for ApiError {
    fn from(rejection: JsonRejection) -> Self {
        let mut text = rejection.body_text();
        text.truncate(MAX_REJECTION_LEN);
        Self::InvalidRequest(text)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let request_id = request_id::current().unwrap_or_default();
        let (status, code, message, fields, details) = match self {
            Self::InvalidRequest(reason) => (
                StatusCode::BAD_REQUEST,
                "INVALID_REQUEST",
                format!("The request body is not valid: {reason}"),
                None,
                None,
            ),
            Self::DecisionNotFound(key) => (
                StatusCode::NOT_FOUND,
                "DECISION_NOT_FOUND",
                format!("No decision named '{key}' in this project."),
                Some(BTreeMap::from([("key".to_owned(), key)])),
                None,
            ),
            Self::InvalidDecision { key, message } => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "INVALID_DECISION",
                format!("Decision '{key}' is not a valid decision model: {message}"),
                Some(BTreeMap::from([("key".to_owned(), key)])),
                None,
            ),
            Self::EvaluationFailed(details) => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "EVALUATION_FAILED",
                "The decision could not be evaluated with this input.".to_owned(),
                None,
                Some(details),
            ),
            Self::DecisionMissing => (
                StatusCode::NOT_FOUND,
                "DECISION_NOT_FOUND",
                "This decision does not exist in this project.".to_owned(),
                None,
                None,
            ),
            Self::DecisionKeyTaken => (
                StatusCode::CONFLICT,
                "DECISION_KEY_TAKEN",
                "Another decision in this project already uses this key.".to_owned(),
                Some(BTreeMap::from([("key".to_owned(), "already used".to_owned())])),
                None,
            ),
            Self::DecisionConflict(details) => (
                StatusCode::CONFLICT,
                "DECISION_CONFLICT",
                "Someone saved this decision since you opened it.".to_owned(),
                None,
                Some(details),
            ),
            Self::VersionNotFound => (
                StatusCode::NOT_FOUND,
                "VERSION_NOT_FOUND",
                "This decision has no such version.".to_owned(),
                None,
                None,
            ),
            Self::VersionUnchanged(version) => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "VERSION_UNCHANGED",
                format!("Nothing changed since version {version}."),
                None,
                Some(serde_json::json!({ "version": version })),
            ),
            Self::ScenarioNotFound => (
                StatusCode::NOT_FOUND,
                "SCENARIO_NOT_FOUND",
                "This project has no such test scenario.".to_owned(),
                None,
                None,
            ),
            Self::ScenarioNameTaken => (
                StatusCode::CONFLICT,
                "SCENARIO_NAME_TAKEN",
                "Another scenario of this decision already has this name.".to_owned(),
                Some(BTreeMap::from([("name".to_owned(), "already used".to_owned())])),
                None,
            ),
            Self::ReleaseNotFound => (
                StatusCode::NOT_FOUND,
                "RELEASE_NOT_FOUND",
                "This project has no such release.".to_owned(),
                None,
                None,
            ),
            Self::DeploymentNotFound => (
                StatusCode::NOT_FOUND,
                "DEPLOYMENT_NOT_FOUND",
                "This environment has no such deployment.".to_owned(),
                None,
                None,
            ),
            Self::TokenNotFound => (
                StatusCode::NOT_FOUND,
                "TOKEN_NOT_FOUND",
                "This environment has no such live token.".to_owned(),
                None,
                None,
            ),
            Self::EnvironmentNotFound => (
                StatusCode::NOT_FOUND,
                "ENVIRONMENT_NOT_FOUND",
                "Environments are staging and production.".to_owned(),
                None,
                None,
            ),
            Self::UnversionedDecisions(keys) => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "UNVERSIONED_DECISIONS",
                "Save a version of every decision before making a release.".to_owned(),
                None,
                Some(serde_json::json!({ "keys": keys })),
            ),
            Self::NothingToRelease => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "NOTHING_TO_RELEASE",
                "This project has no decision to release yet.".to_owned(),
                None,
                None,
            ),
            Self::ApprovalRequired => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "APPROVAL_REQUIRED",
                "Production is published after a second person approves the release.".to_owned(),
                None,
                None,
            ),
            Self::ReleaseConflict(version) => (
                StatusCode::CONFLICT,
                "RELEASE_CONFLICT",
                format!("Release {version} was just created by someone else. Try again."),
                None,
                Some(serde_json::json!({ "version": version })),
            ),
            Self::NotRetryable => (
                StatusCode::CONFLICT,
                "NOT_RETRYABLE",
                "Only a deployment that gave up can be retried.".to_owned(),
                None,
                None,
            ),
            Self::ApprovalNotFound => (
                StatusCode::NOT_FOUND,
                "APPROVAL_NOT_FOUND",
                "This project has no such approval request.".to_owned(),
                None,
                None,
            ),
            Self::NotOnStaging => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "NOT_ON_STAGING",
                "Only the release live on staging can be asked for production.".to_owned(),
                None,
                None,
            ),
            Self::NoApprover => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "NO_APPROVER",
                "No owner other than the release's creator and you can approve it. Add another owner."
                    .to_owned(),
                None,
                None,
            ),
            Self::ApprovalPending => (
                StatusCode::CONFLICT,
                "APPROVAL_PENDING",
                "Another request is waiting for approval. Decide or withdraw it first.".to_owned(),
                None,
                None,
            ),
            Self::SelfApproval => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "SELF_APPROVAL",
                "You made this release or asked for it: another owner must decide.".to_owned(),
                None,
                None,
            ),
            Self::AlreadyDecided => (
                StatusCode::CONFLICT,
                "APPROVAL_DECIDED",
                "This request has already been decided or withdrawn.".to_owned(),
                None,
                None,
            ),
            Self::NeverApproved => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "NEVER_APPROVED",
                "Production can only go back to a release that was approved for it.".to_owned(),
                None,
                None,
            ),
            Self::AlreadyLive => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "ALREADY_LIVE",
                "This release is already the one production runs.".to_owned(),
                None,
                None,
            ),
            Self::NotRequester => (
                StatusCode::FORBIDDEN,
                "NOT_REQUESTER",
                "Only the person who asked can withdraw this request.".to_owned(),
                None,
                None,
            ),
            Self::RecordNotFound => (
                StatusCode::NOT_FOUND,
                "RECORD_NOT_FOUND",
                "This decision record does not exist in this project.".to_owned(),
                None,
                None,
            ),
            Self::InvalidLogToken => (
                StatusCode::UNAUTHORIZED,
                "INVALID_TOKEN",
                "The decision-log token is missing, unknown or revoked.".to_owned(),
                None,
                None,
            ),
            Self::InvalidCiToken => (
                StatusCode::UNAUTHORIZED,
                "INVALID_TOKEN",
                "The CI token is missing, unknown or revoked.".to_owned(),
                None,
                None,
            ),
            Self::BatchTooLarge => (
                StatusCode::BAD_REQUEST,
                "TOO_MANY_RECORDS",
                format!(
                    "Send at most {} records at once.",
                    donka_decision_log::MAX_BATCH_RECORDS
                ),
                None,
                None,
            ),
            Self::ExplainDeclined => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "EXPLAIN_DECLINED",
                "The explanation service declined to explain this decision.".to_owned(),
                None,
                None,
            ),
            Self::ExplainUnavailable(_) => (
                StatusCode::BAD_GATEWAY,
                "EXPLAIN_UNAVAILABLE",
                "The explanation service did not answer. Try again in a moment.".to_owned(),
                None,
                None,
            ),
            Self::DatabaseUnavailable => (
                StatusCode::SERVICE_UNAVAILABLE,
                "DATABASE_UNAVAILABLE",
                "The database is not reachable. Try again shortly.".to_owned(),
                None,
                None,
            ),
            Self::RouteNotFound => (
                StatusCode::NOT_FOUND,
                "NOT_FOUND",
                "There is no endpoint at this path.".to_owned(),
                None,
                None,
            ),
            Self::Unauthenticated => (
                StatusCode::UNAUTHORIZED,
                "UNAUTHENTICATED",
                "Sign in to continue.".to_owned(),
                None,
                None,
            ),
            // One message for every sign-in failure: it must not reveal whether
            // the email exists or the account is locked.
            Self::InvalidCredentials => (
                StatusCode::UNAUTHORIZED,
                "INVALID_CREDENTIALS",
                "Email or password is incorrect.".to_owned(),
                None,
                None,
            ),
            Self::CsrfRequired => (
                StatusCode::FORBIDDEN,
                "CSRF_REQUIRED",
                format!("Requests that change data must send the {} header.", crate::auth::CSRF_HEADER),
                None,
                None,
            ),
            Self::InvalidSetupLink => (
                StatusCode::BAD_REQUEST,
                "INVALID_SETUP_LINK",
                "This link is invalid, already used or expired. Ask an administrator for a new one."
                    .to_owned(),
                None,
                None,
            ),
            Self::Forbidden => (
                StatusCode::FORBIDDEN,
                "FORBIDDEN",
                "You do not have permission to do this.".to_owned(),
                None,
                None,
            ),
            Self::EmailTaken => (
                StatusCode::CONFLICT,
                "EMAIL_TAKEN",
                "Someone already has an account with this email.".to_owned(),
                Some(BTreeMap::from([(
                    "email".to_owned(),
                    "already used".to_owned(),
                )])),
                None,
            ),
            // Also the answer for a project the reader is not a member of: whether
            // it exists is not theirs to know.
            Self::ProjectNotFound => (
                StatusCode::NOT_FOUND,
                "PROJECT_NOT_FOUND",
                "This project does not exist, or you are not one of its members.".to_owned(),
                None,
                None,
            ),
            Self::ProjectKeyTaken => (
                StatusCode::CONFLICT,
                "PROJECT_KEY_TAKEN",
                "Another project already uses this key.".to_owned(),
                Some(BTreeMap::from([("key".to_owned(), "already used".to_owned())])),
                None,
            ),
            Self::ProjectArchived => (
                StatusCode::CONFLICT,
                "PROJECT_ARCHIVED",
                "This project is archived. Restore it to make changes.".to_owned(),
                None,
                None,
            ),
            Self::AlreadyMember => (
                StatusCode::CONFLICT,
                "ALREADY_MEMBER",
                "This person is already a member of the project.".to_owned(),
                None,
                None,
            ),
            Self::MemberNotFound => (
                StatusCode::NOT_FOUND,
                "MEMBER_NOT_FOUND",
                "This person is not a member of the project.".to_owned(),
                None,
                None,
            ),
            Self::LastOwner => (
                StatusCode::CONFLICT,
                "LAST_OWNER",
                "A project needs at least one owner. Make someone else an owner first.".to_owned(),
                None,
                None,
            ),
            Self::NoSuchUser => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "NO_SUCH_USER",
                "No Studio account uses this email. Ask an administrator to invite them first."
                    .to_owned(),
                Some(BTreeMap::from([("email".to_owned(), "no account".to_owned())])),
                None,
            ),
            Self::InvalidField { field, message } => (
                StatusCode::BAD_REQUEST,
                "INVALID_REQUEST",
                format!("Check the {field} field."),
                Some(BTreeMap::from([(field.to_owned(), message)])),
                None,
            ),
            Self::Internal(reason) => {
                tracing::error!(%reason, "unexpected failure");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "INTERNAL_ERROR",
                    "Something went wrong on our side. Quote the request id if you report it."
                        .to_owned(),
                    None,
                    None,
                )
            }
        };
        let body = ErrorBody {
            message,
            code,
            request_id,
            fields,
            details,
        };
        (status, Json(body)).into_response()
    }
}
