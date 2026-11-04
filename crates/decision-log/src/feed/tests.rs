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
