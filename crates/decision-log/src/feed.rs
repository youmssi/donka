//! Records as the Runtime sends them (docs/decision-log-feed.md), checked one
//! by one so a bad record never costs the rest of its batch.

use chrono::{DateTime, Utc};
use donka_release::Environment;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

/// Longest decision key, reference and outcome kept.
pub const MAX_TEXT_CHARS: usize = 200;

/// One evaluation, as the Runtime reports it.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FeedRecord {
    pub id: Uuid,
    pub project_id: Uuid,
    pub release_id: Uuid,
    pub environment: String,
    pub decision_key: String,
    #[serde(default)]
    pub reference: Option<String>,
    pub evaluated_at: DateTime<Utc>,
    pub duration_us: u64,
    pub status: Status,
    pub input: Value,
    #[serde(default)]
    pub output: Option<Value>,
    #[serde(default)]
    pub error: Option<Value>,
    #[serde(default)]
    pub trace: Option<Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[serde(rename_all = "lowercase")]
#[sqlx(type_name = "text", rename_all = "lowercase")]
pub enum Status {
    Succeeded,
    Failed,
}

/// What is encrypted: everything the decision read and answered.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Payload {
    pub input: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trace: Option<Value>,
}

/// Why a record was not stored, as the Runtime logs it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rejection {
    /// Not a record in the feed format.
    Invalid,
    /// For an environment the token was not issued for.
    WrongEnvironment,
    /// A release Studio never published to that environment of that project.
    UnknownRelease,
}

impl Rejection {
    pub fn code(self) -> &'static str {
        match self {
            Rejection::Invalid => "invalid",
            Rejection::WrongEnvironment => "wrong_environment",
            Rejection::UnknownRelease => "unknown_release",
        }
    }
}

impl FeedRecord {
    /// Parses a record; `Err` holds its id when one could be read.
    pub fn parse(value: Value) -> Result<Self, Option<Uuid>> {
        let id = value
            .get("id")
            .and_then(Value::as_str)
            .and_then(|id| Uuid::parse_str(id).ok());
        let record: FeedRecord = serde_json::from_value(value).map_err(|_| id)?;
        let text_ok = |text: &str| !text.is_empty() && text.chars().count() <= MAX_TEXT_CHARS;
        let well_formed = text_ok(&record.decision_key)
            && record.reference.as_deref().is_none_or(text_ok)
            && i64::try_from(record.duration_us).is_ok()
            && match record.status {
                Status::Succeeded => record.output.is_some(),
                Status::Failed => record.error.is_some(),
            };
        if well_formed {
            Ok(record)
        } else {
            Err(Some(record.id))
        }
    }

    pub fn environment(&self) -> Option<Environment> {
        Environment::parse(&self.environment)
    }

    /// The record's outcome: `error` when the evaluation failed, otherwise the
    /// value at `field` (a dotted path) in the output, when it is a string, a
    /// number or a boolean.
    pub fn outcome(&self, field: Option<&str>) -> Option<String> {
        if self.status == Status::Failed {
            return Some("error".to_owned());
        }
        let mut value = self.output.as_ref()?;
        for part in field?.split('.') {
            value = value.get(part)?;
        }
        let text = match value {
            Value::String(text) => text.trim().to_owned(),
            Value::Number(number) => number.to_string(),
            Value::Bool(flag) => flag.to_string(),
            _ => return None,
        };
        (!text.is_empty()).then(|| text.chars().take(MAX_TEXT_CHARS).collect())
    }

    pub fn into_payload(self) -> Payload {
        Payload {
            input: self.input,
            output: self.output,
            error: self.error,
            trace: self.trace,
        }
    }
}

#[cfg(test)]
#[path = "feed/tests.rs"]
mod tests;
