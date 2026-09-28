#![allow(clippy::unwrap_used)] // tests fail loudly on purpose

use async_trait::async_trait;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::Router;
use donka_app::{router, AppState};
use donka_db::{DbOptions, PgPool};
use donka_engine::{
    Bundle, DecisionRuntime, EvaluateOptions, Evaluation, RuntimeError, ZenRuntime,
};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use std::sync::Arc;
use tower::ServiceExt;

const BASE: &str = "/api/v1";

fn table() -> Value {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../crates/engine/tests/fixtures/table.json"
    );
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

/// A pool that never connects: port 1 refuses immediately. Routes that do not need
/// the database must work with it; readiness must report it unavailable.
fn unreachable_db() -> PgPool {
    let options = DbOptions {
        acquire_timeout: std::time::Duration::from_millis(300),
        ..DbOptions::default()
    };
    donka_db::connect_lazy("postgres://nobody@127.0.0.1:1/none", &options).unwrap()
}

fn app_with(runtime: Arc<dyn DecisionRuntime>, base: &str) -> Router {
    router(
        AppState {
            runtime,
            db: unreachable_db(),
        },
        base,
    )
}

fn app() -> Router {
    app_with(Arc::new(ZenRuntime::new(1)), BASE)
}

struct Reply {
    status: StatusCode,
    request_id: Option<String>,
    body: Value,
}

async fn send(app: Router, req: Request<Body>) -> Reply {
    let res = app.oneshot(req).await.unwrap();
    let status = res.status();
    let request_id = res
        .headers()
        .get("x-request-id")
        .map(|v| v.to_str().unwrap().to_owned());
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let body = serde_json::from_slice(&bytes)
        .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&bytes).into_owned()));
    Reply {
        status,
        request_id,
        body,
    }
}

fn post_json(path: &str, body: &str) -> Request<Body> {
    Request::post(path)
        .header("content-type", "application/json")
        .body(Body::from(body.to_owned()))
        .unwrap()
}

fn get(path: &str) -> Request<Body> {
    Request::get(path).body(Body::empty()).unwrap()
}

// --- base path -------------------------------------------------------------

#[tokio::test]
async fn health_lives_under_the_base_path() {
    let reply = send(app(), get("/api/v1/health")).await;
    assert_eq!(reply.status, StatusCode::OK);
    assert_eq!(reply.body, Value::String("ok".into()));
}

#[tokio::test]
async fn changing_the_base_path_moves_every_route() {
    let app = app_with(Arc::new(ZenRuntime::new(1)), "/studio/v2");

    assert_eq!(
        send(app.clone(), get("/studio/v2/health")).await.status,
        StatusCode::OK
    );
    assert_eq!(
        send(app.clone(), get("/api/v1/health")).await.status,
        StatusCode::NOT_FOUND
    );
    let body = json!({ "decisions": { "table": table() }, "key": "table" }).to_string();
    assert_eq!(
        send(app, post_json("/studio/v2/simulate", &body))
            .await
            .status,
        StatusCode::OK
    );
}

// --- readiness --------------------------------------------------------------

#[tokio::test]
async fn ready_is_503_when_the_database_is_unreachable() {
    let reply = send(app(), get("/api/v1/ready")).await;
    assert_error(
        &reply,
        StatusCode::SERVICE_UNAVAILABLE,
        "DATABASE_UNAVAILABLE",
    );
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn ready_is_200_when_the_database_answers(db: PgPool) {
    let app = router(
        AppState {
            runtime: Arc::new(ZenRuntime::new(1)),
            db,
        },
        BASE,
    );
    let reply = send(app, get("/api/v1/ready")).await;
    assert_eq!(reply.status, StatusCode::OK);
    assert_eq!(reply.body, Value::String("ready".into()));
}

#[tokio::test]
async fn health_does_not_need_the_database() {
    assert_eq!(
        send(app(), get("/api/v1/health")).await.status,
        StatusCode::OK
    );
}

// --- simulate ---------------------------------------------------------------

#[tokio::test]
async fn simulate_returns_result_and_trace() {
    let body =
        json!({ "decisions": { "table": table() }, "key": "table", "context": { "input": 12 } });
    let reply = send(app(), post_json("/api/v1/simulate", &body.to_string())).await;

    assert_eq!(reply.status, StatusCode::OK);
    assert_eq!(reply.body["result"], json!({ "output": 10 }));
    assert!(reply.body["trace"].is_object());
}

// --- error model ------------------------------------------------------------

fn assert_error(reply: &Reply, status: StatusCode, code: &str) {
    assert_eq!(reply.status, status, "{}", reply.body);
    assert_eq!(reply.body["code"], code);
    assert!(reply.body["message"]
        .as_str()
        .is_some_and(|m| !m.is_empty()));
    let id = reply.body["requestId"].as_str().expect("requestId in body");
    assert_eq!(
        Some(id),
        reply.request_id.as_deref(),
        "body and header ids match"
    );
}

#[tokio::test]
async fn unknown_decision_is_404_decision_not_found() {
    let body = json!({ "decisions": { "table": table() }, "key": "missing" });
    let reply = send(app(), post_json("/api/v1/simulate", &body.to_string())).await;

    assert_error(&reply, StatusCode::NOT_FOUND, "DECISION_NOT_FOUND");
    assert_eq!(reply.body["fields"]["key"], "missing");
}

#[tokio::test]
async fn invalid_model_is_422_invalid_decision() {
    let body = json!({ "decisions": { "bad": { "nodes": 1 } }, "key": "bad" });
    let reply = send(app(), post_json("/api/v1/simulate", &body.to_string())).await;

    assert_error(&reply, StatusCode::UNPROCESSABLE_ENTITY, "INVALID_DECISION");
    assert_eq!(reply.body["fields"]["key"], "bad");
}

#[tokio::test]
async fn engine_rejection_is_422_evaluation_failed_with_details() {
    let parent_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../crates/engine/tests/fixtures/nodes-parent.json"
    );
    let parent: Value =
        serde_json::from_str(&std::fs::read_to_string(parent_path).unwrap()).unwrap();
    // The parent references a sub-decision that is not in the bundle.
    let body = json!({ "decisions": { "nodes-parent": parent }, "key": "nodes-parent" });
    let reply = send(app(), post_json("/api/v1/simulate", &body.to_string())).await;

    assert_error(
        &reply,
        StatusCode::UNPROCESSABLE_ENTITY,
        "EVALUATION_FAILED",
    );
    assert!(!reply.body["details"].is_null());
}

#[tokio::test]
async fn malformed_json_is_400_invalid_request() {
    let reply = send(app(), post_json("/api/v1/simulate", "{ not json")).await;
    assert_error(&reply, StatusCode::BAD_REQUEST, "INVALID_REQUEST");

    let reply = send(app(), post_json("/api/v1/simulate", r#"{"decisions":{}}"#)).await;
    assert_error(&reply, StatusCode::BAD_REQUEST, "INVALID_REQUEST");
}

struct BrokenRuntime;

#[async_trait]
impl DecisionRuntime for BrokenRuntime {
    async fn evaluate(
        &self,
        _: &Bundle,
        _: &str,
        _: Value,
        _: EvaluateOptions,
    ) -> Result<Evaluation, RuntimeError> {
        Err(RuntimeError::Internal(
            "worker pool poisoned at 0xdeadbeef".into(),
        ))
    }
}

#[tokio::test]
async fn unexpected_failure_is_500_without_internal_detail() {
    let app = app_with(Arc::new(BrokenRuntime), BASE);
    let body = json!({ "decisions": { "table": table() }, "key": "table" });
    let reply = send(app, post_json("/api/v1/simulate", &body.to_string())).await;

    assert_error(&reply, StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL_ERROR");
    let raw = reply.body.to_string();
    assert!(
        !raw.contains("poisoned") && !raw.contains("0xdeadbeef"),
        "{raw}"
    );
}

// --- request id -------------------------------------------------------------

#[tokio::test]
async fn every_response_carries_a_generated_request_id() {
    let reply = send(app(), get("/api/v1/health")).await;
    let id = reply.request_id.expect("x-request-id header");
    assert_eq!(id.len(), 36, "uuid: {id}");
}

#[tokio::test]
async fn a_safe_incoming_request_id_is_reused() {
    let req = Request::get("/api/v1/health")
        .header("x-request-id", "lb-7f3a.42")
        .body(Body::empty())
        .unwrap();
    assert_eq!(
        send(app(), req).await.request_id.as_deref(),
        Some("lb-7f3a.42")
    );
}

#[tokio::test]
async fn an_unsafe_incoming_request_id_is_replaced() {
    let req = Request::get("/api/v1/health")
        .header("x-request-id", "abc def")
        .body(Body::empty())
        .unwrap();
    let id = send(app(), req).await.request_id.unwrap();
    assert_ne!(id, "abc def");
}

// --- contract ---------------------------------------------------------------

#[tokio::test]
async fn openapi_document_describes_the_endpoints() {
    let reply = send(app(), get("/api/v1/openapi.json")).await;

    assert_eq!(reply.status, StatusCode::OK);
    assert_eq!(reply.body["servers"][0]["url"], "/api/v1");
    assert!(reply.body["paths"]["/health"]["get"].is_object());
    assert!(reply.body["paths"]["/ready"]["get"].is_object());
    assert!(reply.body["paths"]["/simulate"]["post"].is_object());
    assert!(reply.body["components"]["schemas"]["ErrorBody"].is_object());
}
