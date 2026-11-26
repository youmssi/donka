use super::*;
use serde_json::json;

fn paths(mismatches: &[Mismatch]) -> Vec<&str> {
    mismatches.iter().map(|m| m.path.as_str()).collect()
}

#[test]
fn partial_checks_only_the_expected_fields() {
    let actual = json!({ "approved": true, "limit": 150000, "reason": "income" });
    assert!(compare(&json!({ "approved": true }), &actual, Match::Partial).is_empty());
    let wrong = compare(
        &json!({ "approved": false, "score": 3 }),
        &actual,
        Match::Partial,
    );
    assert_eq!(
        wrong,
        [
            Mismatch {
                path: "approved".into(),
                expected: Some(json!(false)),
                actual: Some(json!(true))
            },
            Mismatch {
                path: "score".into(),
                expected: Some(json!(3)),
                actual: None
            },
        ]
    );
}

#[test]
fn exact_also_refuses_extra_fields_at_any_depth() {
    let expected = json!({ "decision": { "approved": true } });
    let actual = json!({ "decision": { "approved": true, "limit": 1 } });
    assert!(compare(&expected, &actual, Match::Partial).is_empty());
    let extra = compare(&expected, &actual, Match::Exact);
    assert_eq!(paths(&extra), ["decision.limit"]);
    assert_eq!(extra[0].expected, None);
    assert_eq!(extra[0].actual, Some(json!(1)));
}

#[test]
fn numbers_compare_by_value_and_arrays_as_a_whole() {
    assert!(compare(
        &json!({ "n": 150000 }),
        &json!({ "n": 150000.0 }),
        Match::Exact
    )
    .is_empty());
    assert!(compare(
        &json!({ "l": [1, 2] }),
        &json!({ "l": [1.0, 2] }),
        Match::Exact
    )
    .is_empty());
    let order = compare(
        &json!({ "l": [1, 2] }),
        &json!({ "l": [2, 1] }),
        Match::Partial,
    );
    assert_eq!(paths(&order), ["l"]);
}

#[test]
fn an_output_that_is_not_an_object_is_one_mismatch() {
    let whole = compare(&json!({ "a": 1 }), &json!(null), Match::Partial);
    assert_eq!(paths(&whole), [""]);
}

#[test]
fn match_modes_read_and_write() {
    for mode in [Match::Exact, Match::Partial] {
        assert_eq!(Match::parse(mode.as_str()), Some(mode));
    }
    assert_eq!(Match::parse("fuzzy"), None);
}
