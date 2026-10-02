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
mod tests {
    use super::*;
    use serde_json::json;

    fn record(extra: Value) -> Value {
        let mut record = json!({
            "id": "0f8fad5b-d9cb-469f-a165-70867728950e",
            "projectId": "7c9e6679-7425-40de-944b-e07fc1f90ae7",
            "releaseId": "16fd2706-8baf-433b-82eb-8c7fada847da",
            "environment": "production",
            "decisionKey": "person-score",
            "evaluatedAt": "2026-10-02T10:00:00Z",
            "durationUs": 840,
            "status": "succeeded",
            "input": { "income": 1200 },
            "output": { "decision": "approve", "limit": { "band": "B", "amount": 5000 } }
        });
        for (key, value) in extra.as_object().unwrap() {
            record[key] = value.clone();
        }
        record
    }

    #[test]
    fn a_record_in_the_feed_format_is_read() {
        let parsed = FeedRecord::parse(record(json!({ "reference": "APP-1" }))).unwrap();
        assert_eq!(parsed.reference.as_deref(), Some("APP-1"));
        assert_eq!(parsed.environment(), Some(Environment::Production));
    }

    #[test]
    fn a_malformed_record_is_refused_with_its_id_when_it_has_one() {
        let id = Uuid::parse_str("0f8fad5b-d9cb-469f-a165-70867728950e").ok();
        for bad in [
            json!({ "decisionKey": "" }),
            json!({ "reference": "x".repeat(201) }),
            json!({ "status": "maybe" }),
            json!({ "status": "failed" }),
            json!({ "projectId": "not-a-uuid" }),
        ] {
            assert_eq!(
                FeedRecord::parse(record(bad.clone())).unwrap_err(),
                id,
                "{bad}"
            );
        }
        assert_eq!(FeedRecord::parse(json!({ "id": 3 })).unwrap_err(), None);
    }

    #[test]
    fn the_outcome_is_read_from_the_named_output_field() {
        let parsed = FeedRecord::parse(record(json!({}))).unwrap();
        assert_eq!(parsed.outcome(Some("decision")).as_deref(), Some("approve"));
        assert_eq!(parsed.outcome(Some("limit.band")).as_deref(), Some("B"));
        assert_eq!(
            parsed.outcome(Some("limit.amount")).as_deref(),
            Some("5000")
        );
        assert_eq!(
            parsed.outcome(Some("limit")),
            None,
            "an object is not an outcome"
        );
        assert_eq!(parsed.outcome(Some("missing")), None);
        assert_eq!(parsed.outcome(None), None);

        let failed = FeedRecord::parse(record(json!({
            "status": "failed", "output": null, "error": { "message": "boom" }
        })))
        .unwrap();
        assert_eq!(failed.outcome(Some("decision")).as_deref(), Some("error"));
        assert_eq!(failed.outcome(None).as_deref(), Some("error"));
    }
}
