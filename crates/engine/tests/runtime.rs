use donka_engine::{Bundle, DecisionRuntime, EvaluateOptions, RuntimeError, ZenRuntime};
use serde_json::{json, Value};
use std::collections::BTreeMap;

fn fixture(name: &str) -> Value {
    let path = format!("{}/tests/fixtures/{name}.json", env!("CARGO_MANIFEST_DIR"));
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn bundle(names: &[&str]) -> Bundle {
    Bundle::from_json(names.iter().map(|n| (n.to_string(), fixture(n))).collect()).unwrap()
}

#[tokio::test]
async fn evaluates_a_decision_table() {
    let runtime = ZenRuntime::new(1);
    let out = runtime
        .evaluate(
            &bundle(&["table"]),
            "table",
            json!({ "input": 12 }),
            EvaluateOptions::default(),
        )
        .await
        .unwrap();

    assert_eq!(out.result, json!({ "output": 10 }));
    assert!(out.trace.is_none());
}

#[tokio::test]
async fn returns_a_trace_when_asked() {
    let runtime = ZenRuntime::new(1);
    let out = runtime
        .evaluate(
            &bundle(&["table"]),
            "table",
            json!({ "input": 12 }),
            EvaluateOptions { trace: true },
        )
        .await
        .unwrap();

    assert!(out
        .trace
        .is_some_and(|t| t.as_object().is_some_and(|o| !o.is_empty())));
}

#[tokio::test]
async fn resolves_sub_decisions_inside_the_bundle() {
    let runtime = ZenRuntime::new(1);
    let out = runtime
        .evaluate(
            &bundle(&["nodes-parent", "nodes-child"]),
            "nodes-parent",
            json!({ "hello": "world" }),
            EvaluateOptions::default(),
        )
        .await
        .unwrap();

    assert_eq!(out.result["expressionRequest"], json!({ "hello": "world" }));
}

#[tokio::test]
async fn a_missing_sub_decision_is_an_evaluation_error() {
    let runtime = ZenRuntime::new(1);
    let err = runtime
        .evaluate(
            &bundle(&["nodes-parent"]),
            "nodes-parent",
            json!({}),
            EvaluateOptions::default(),
        )
        .await
        .unwrap_err();

    assert!(matches!(err, RuntimeError::Evaluation { .. }), "{err:?}");
}

#[tokio::test]
async fn unknown_key_is_not_found() {
    let err = ZenRuntime::new(1)
        .evaluate(
            &bundle(&["table"]),
            "nope",
            json!({}),
            EvaluateOptions::default(),
        )
        .await
        .unwrap_err();

    assert!(matches!(err, RuntimeError::NotFound(k) if k == "nope"));
}

#[test]
fn invalid_content_names_the_decision() {
    let err = Bundle::from_json(BTreeMap::from([(
        "broken".to_string(),
        json!({ "nodes": 3 }),
    )]))
    .unwrap_err();

    assert!(matches!(err, RuntimeError::InvalidContent { key, .. } if key == "broken"));
}
