use axum::body::Body;
use axum::http::{Request, StatusCode};
use donka_app::{router, AppState};
use donka_engine::ZenRuntime;
use http_body_util::BodyExt;
use serde_json::{json, Value};
use std::sync::Arc;
use tower::ServiceExt;

fn table() -> Value {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../crates/engine/tests/fixtures/table.json"
    );
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

async fn post(body: Value) -> (StatusCode, Value) {
    let app = router(AppState {
        runtime: Arc::new(ZenRuntime::new(1)),
    });
    let res = app
        .oneshot(
            Request::post("/api/simulate")
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    (status, serde_json::from_slice(&bytes).unwrap())
}

#[tokio::test]
async fn simulate_returns_result_and_trace() {
    let (status, body) = post(json!({
        "decisions": { "table": table() },
        "key": "table",
        "context": { "input": 12 }
    }))
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["result"], json!({ "output": 10 }));
    assert!(body["trace"].is_object());
}

#[tokio::test]
async fn simulate_rejects_an_unknown_key() {
    let (status, body) = post(json!({ "decisions": { "table": table() }, "key": "missing" })).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["code"], "decision_not_found");
}

#[tokio::test]
async fn simulate_names_the_broken_decision() {
    let (status, body) =
        post(json!({ "decisions": { "bad": { "nodes": 1 } }, "key": "bad" })).await;

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["key"], "bad");
}
