//! Whether a decision gave the output a scenario expects, field by field.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// How much of the output a scenario pins down.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Match {
    /// The output is exactly the expected document: a field it adds is a mismatch.
    Exact,
    /// Every expected field has its value; other fields of the output are free.
    Partial,
}

impl Match {
    pub fn as_str(self) -> &'static str {
        match self {
            Match::Exact => "exact",
            Match::Partial => "partial",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "exact" => Some(Match::Exact),
            "partial" => Some(Match::Partial),
            _ => None,
        }
    }
}

/// One field where the output differs from what was expected. `path` is the
/// field's dotted path (`decision.limit`), empty for the whole output; a side
/// is absent when the field is missing there.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Mismatch {
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actual: Option<Value>,
}

/// The fields where `actual` differs from `expected`; empty when it matches.
/// Objects are compared field by field; arrays and values as a whole. Numbers
/// compare by value, so `150000` and `150000.0` are the same.
pub fn compare(expected: &Value, actual: &Value, mode: Match) -> Vec<Mismatch> {
    let mut out = Vec::new();
    walk("", Some(expected), Some(actual), mode, &mut out);
    out
}

fn walk(
    path: &str,
    expected: Option<&Value>,
    actual: Option<&Value>,
    mode: Match,
    out: &mut Vec<Mismatch>,
) {
    if let (Some(Value::Object(e)), Some(Value::Object(a))) = (expected, actual) {
        for key in keys(e, a, mode) {
            let child = if path.is_empty() {
                key.clone()
            } else {
                format!("{path}.{key}")
            };
            walk(&child, e.get(&key), a.get(&key), mode, out);
        }
        return;
    }
    let same = match (expected, actual) {
        (Some(e), Some(a)) => same(e, a),
        (None, None) => true,
        _ => false,
    };
    if !same {
        out.push(Mismatch {
            path: path.to_owned(),
            expected: expected.cloned(),
            actual: actual.cloned(),
        });
    }
}

/// The fields to look at: the expected ones, plus the output's own when exact.
fn keys(expected: &Map<String, Value>, actual: &Map<String, Value>, mode: Match) -> Vec<String> {
    let mut keys: Vec<String> = expected.keys().cloned().collect();
    if mode == Match::Exact {
        keys.extend(
            actual
                .keys()
                .filter(|k| !expected.contains_key(*k))
                .cloned(),
        );
    }
    keys
}

fn same(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => x.as_f64() == y.as_f64(),
        (Value::Array(x), Value::Array(y)) => {
            x.len() == y.len() && x.iter().zip(y).all(|(x, y)| same(x, y))
        }
        (Value::Object(x), Value::Object(y)) => {
            x.len() == y.len() && x.iter().all(|(k, v)| y.get(k).is_some_and(|w| same(v, w)))
        }
        _ => a == b,
    }
}

#[cfg(test)]
mod tests {
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
}
