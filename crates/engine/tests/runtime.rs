#![allow(clippy::unwrap_used)] // tests fail loudly on purpose

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
            EvaluateOptions {
                trace: true,
                ..Default::default()
            },
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

/// A graph whose connector node calls `url`, with or without a mock response.
fn connector_bundle(url: &str, mock: Option<Value>) -> Bundle {
    let mut config = json!({
        "preset": "bureau-score",
        "url": url,
        "auth": { "type": "header", "header": "X-Api-Key", "secret": "BUREAU_API_KEY" },
        "body": { "id": "{{ applicant.id }}" },
        "outputKey": "bureau"
    });
    if let Some(mock) = mock {
        config["mock"] = mock;
    }
    let graph = json!({
        "nodes": [
            { "id": "in", "name": "Request", "type": "inputNode", "position": { "x": 0, "y": 0 } },
            {
                "id": "bureau", "name": "Bureau", "type": "customNode", "position": { "x": 0, "y": 0 },
                "content": { "kind": "donka.connector", "config": config }
            },
            { "id": "out", "name": "Response", "type": "outputNode", "position": { "x": 0, "y": 0 } }
        ],
        "edges": [
            { "id": "e1", "type": "edge", "sourceId": "in", "targetId": "bureau" },
            { "id": "e2", "type": "edge", "sourceId": "bureau", "targetId": "out" }
        ]
    });
    Bundle::from_json(BTreeMap::from([("score".to_owned(), graph)])).unwrap()
}

#[tokio::test]
async fn connectors_answer_with_their_mock_and_call_nothing() {
    // A service that would notice a call: nothing may connect to it.
    let service = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    service.set_nonblocking(true).unwrap();
    let url = format!("http://{}/score", service.local_addr().unwrap());

    let out = ZenRuntime::new(1)
        .evaluate(
            &connector_bundle(&url, Some(json!({ "score": 712 }))),
            "score",
            json!({ "applicant": { "id": "A1" } }),
            EvaluateOptions {
                trace: true,
                ..Default::default()
            },
        )
        .await
        .unwrap();

    assert_eq!(
        out.result,
        json!({ "applicant": { "id": "A1" }, "bureau": { "score": 712 } })
    );
    let trace = out.trace.unwrap();
    assert_eq!(
        trace["bureau"]["traceData"],
        json!({ "mode": "mock", "outcome": "ok" })
    );
    assert_eq!(
        service.accept().map_err(|e| e.kind()).err(),
        Some(std::io::ErrorKind::WouldBlock),
        "simulation never calls the service"
    );
}

#[tokio::test]
async fn a_connector_without_a_mock_cannot_be_simulated() {
    let err = ZenRuntime::new(1)
        .evaluate(
            &connector_bundle("https://bureau.example/score", None),
            "score",
            json!({}),
            EvaluateOptions::default(),
        )
        .await
        .unwrap_err();

    let RuntimeError::Evaluation { details } = err else {
        panic!("expected an evaluation error, got {err:?}");
    };
    assert_eq!(details["nodeId"], json!("bureau"));
    assert!(
        details.to_string().contains("no mock response"),
        "{details}"
    );
}

#[tokio::test]
async fn a_replay_answers_connectors_with_the_recorded_trace() {
    // As the Runtime logged it: the bureau answered 640.
    let recorded = json!({
        "bureau": {
            "id": "bureau", "name": "Bureau", "order": 1,
            "input": { "applicant": { "id": "A1" } },
            "output": { "applicant": { "id": "A1" }, "bureau": { "score": 640 } }
        }
    });

    let out = ZenRuntime::new(1)
        .evaluate(
            &connector_bundle(
                "https://bureau.example/score",
                Some(json!({ "score": 712 })),
            ),
            "score",
            json!({ "applicant": { "id": "A1" } }),
            EvaluateOptions {
                trace: true,
                recorded_trace: Some(std::sync::Arc::new(recorded)),
            },
        )
        .await
        .unwrap();

    assert_eq!(out.result["bureau"], json!({ "score": 640 }));
    assert_eq!(
        out.trace.unwrap()["bureau"]["traceData"],
        json!({ "mode": "replay", "outcome": "ok" })
    );
}
