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
#[path = "compare/tests.rs"]
mod tests;
