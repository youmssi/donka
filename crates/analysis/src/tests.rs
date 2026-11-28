use super::*;

#[test]
fn aggregates_outcome_mix_transitions_and_uncomparable_reasons() {
    let kept = Uuid::from_u128(1);
    let swapped = Uuid::from_u128(2);
    let result = ResultSummary::from_comparisons([
        Comparison::Compared {
            record_id: kept,
            before: "approved".into(),
            after: "approved".into(),
        },
        Comparison::Compared {
            record_id: swapped,
            before: "declined".into(),
            after: "approved".into(),
        },
        Comparison::Uncomparable {
            reason: UncomparableReason::ContractRejected,
        },
    ]);

    assert_eq!(result.records_read, 3);
    assert_eq!(result.records_compared, 2);
    assert_eq!(result.before["approved"], 1);
    assert_eq!(result.after["approved"], 2);
    assert_eq!(
        result.transitions,
        [
            TransitionCell {
                before: "approved".into(),
                after: "approved".into(),
                count: 1,
                records: vec![],
            },
            TransitionCell {
                before: "declined".into(),
                after: "approved".into(),
                count: 1,
                records: vec![swapped],
            }
        ]
    );
    assert_eq!(result.transitions[0].count, 1);
    assert!(result.transitions[0].records.is_empty());
    assert_eq!(
        result.uncomparable[&UncomparableReason::ContractRejected],
        1
    );
    let stored = serde_json::to_value(&result).expect("analysis result serializes");
    assert!(!stored.to_string().contains(&kept.to_string()));
    assert!(stored.to_string().contains(&swapped.to_string()));
}

#[test]
fn detects_unrecorded_connectors_and_time_dependent_expressions() {
    let decision = json!({
        "nodes": [{
            "id": "bureau",
            "type": "customNode",
            "content": { "kind": "donka.connector" }
        }]
    });
    assert!(connector_not_recorded(&decision, None));
    assert!(connector_not_recorded(
        &decision,
        Some(&json!({ "other": {} }))
    ));
    assert!(!connector_not_recorded(
        &decision,
        Some(&json!({ "bureau": {} }))
    ));
    assert!(time_dependent(&json!({
        "nodes": [{ "content": { "expressions": [{ "value": "today()" }] } }]
    })));
    assert!(!time_dependent(&json!({
        "nodes": [{ "content": { "expressions": [{ "value": "input * 2" }] } }]
    })));
}
