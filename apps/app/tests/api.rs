#![allow(clippy::unwrap_used)] // tests fail loudly on purpose

mod support;

use async_trait::async_trait;
use axum::http::{Request, StatusCode};
use donka_db::PgPool;
use donka_engine::{Bundle, DecisionRuntime, EvaluateOptions, Evaluation, RuntimeError};
use serde_json::{json, Value};
use std::sync::Arc;
use support::*;

fn fixture(name: &str) -> Value {
    let path = format!(
        "{}/../../crates/engine/tests/fixtures/{name}.json",
        env!("CARGO_MANIFEST_DIR")
    );
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

// --- base path -------------------------------------------------------------

#[tokio::test]
async fn health_lives_under_the_base_path() {
    let app = without_database();
    let reply = send(&app.router, get("/api/v1/health", None)).await;
    assert_eq!(reply.status, StatusCode::OK);
    assert_eq!(reply.body, Value::String("ok".into()));
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn changing_the_base_path_moves_every_route(db: PgPool) {
    let app = build(db, Arc::new(donka_engine::ZenRuntime::new(1)), "/studio/v2");
    admin_with_password(&app).await;

    assert_eq!(
        send(&app.router, get("/studio/v2/health", None))
            .await
            .status,
        StatusCode::OK
    );
    assert_eq!(
        send(&app.router, get("/api/v1/health", None)).await.status,
        StatusCode::NOT_FOUND
    );
    let credentials = json!({ "email": ADMIN_EMAIL, "password": ADMIN_PASSWORD });
    let reply = send(
        &app.router,
        post("/studio/v2/auth/sign-in", &credentials, None),
    )
    .await;
    assert_eq!(reply.status, StatusCode::OK);
    let session = session_from(&reply);
    let body = json!({ "decisions": { "table": fixture("table") }, "key": "table" });
    let reply = send(
        &app.router,
        post("/studio/v2/simulate", &body, Some(&session)),
    )
    .await;
    assert_eq!(reply.status, StatusCode::OK);
}

// --- readiness --------------------------------------------------------------

#[tokio::test]
async fn ready_is_503_when_the_database_is_unreachable() {
    let app = without_database();
    let reply = send(&app.router, get("/api/v1/ready", None)).await;
    assert_error(
        &reply,
        StatusCode::SERVICE_UNAVAILABLE,
        "DATABASE_UNAVAILABLE",
    );
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn ready_is_200_when_the_database_answers(db: PgPool) {
    let app = with_database(db);
    let reply = send(&app.router, get("/api/v1/ready", None)).await;
    assert_eq!(reply.status, StatusCode::OK);
    assert_eq!(reply.body, Value::String("ready".into()));
}

#[tokio::test]
async fn health_does_not_need_the_database() {
    let app = without_database();
    assert_eq!(
        send(&app.router, get("/api/v1/health", None)).await.status,
        StatusCode::OK
    );
}

#[tokio::test]
async fn version_needs_neither_session_nor_database() {
    let app = without_database();
    let reply = send(&app.router, get("/api/v1/version", None)).await;
    assert_eq!(reply.status, StatusCode::OK);
    assert_eq!(reply.body["version"], donka_app::VERSION);
}

// --- simulate ---------------------------------------------------------------

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn simulate_returns_result_and_trace(db: PgPool) {
    let app = with_database(db);
    let session = signed_in_admin(&app).await;
    let body = json!({ "decisions": { "table": fixture("table") }, "key": "table", "context": { "input": 12 } });
    let reply = send(&app.router, post("/api/v1/simulate", &body, Some(&session))).await;

    assert_eq!(reply.status, StatusCode::OK);
    assert_eq!(reply.body["result"], json!({ "output": 10 }));
    assert!(reply.body["trace"].is_object());
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn unknown_decision_is_404_decision_not_found(db: PgPool) {
    let app = with_database(db);
    let session = signed_in_admin(&app).await;
    let body = json!({ "decisions": { "table": fixture("table") }, "key": "missing" });
    let reply = send(&app.router, post("/api/v1/simulate", &body, Some(&session))).await;

    assert_error(&reply, StatusCode::NOT_FOUND, "DECISION_NOT_FOUND");
    assert_eq!(reply.body["fields"]["key"], "missing");
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn invalid_model_is_422_invalid_decision(db: PgPool) {
    let app = with_database(db);
    let session = signed_in_admin(&app).await;
    let body = json!({ "decisions": { "bad": { "nodes": 1 } }, "key": "bad" });
    let reply = send(&app.router, post("/api/v1/simulate", &body, Some(&session))).await;

    assert_error(&reply, StatusCode::UNPROCESSABLE_ENTITY, "INVALID_DECISION");
    assert_eq!(reply.body["fields"]["key"], "bad");
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn engine_rejection_is_422_evaluation_failed_with_details(db: PgPool) {
    let app = with_database(db);
    let session = signed_in_admin(&app).await;
    // The parent references a sub-decision that is not in the bundle.
    let body =
        json!({ "decisions": { "nodes-parent": fixture("nodes-parent") }, "key": "nodes-parent" });
    let reply = send(&app.router, post("/api/v1/simulate", &body, Some(&session))).await;

    assert_error(
        &reply,
        StatusCode::UNPROCESSABLE_ENTITY,
        "EVALUATION_FAILED",
    );
    assert!(!reply.body["details"].is_null());
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn malformed_json_is_400_invalid_request(db: PgPool) {
    let app = with_database(db);
    let session = signed_in_admin(&app).await;
    let raw = Request::post("/api/v1/simulate")
        .header("content-type", "application/json")
        .header("x-donka-csrf", "1")
        .header("cookie", format!("donka_session={session}"))
        .body(axum::body::Body::from("{ not json"))
        .unwrap();
    assert_error(
        &send(&app.router, raw).await,
        StatusCode::BAD_REQUEST,
        "INVALID_REQUEST",
    );

    let reply = send(
        &app.router,
        post(
            "/api/v1/simulate",
            &json!({ "decisions": {} }),
            Some(&session),
        ),
    )
    .await;
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

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn unexpected_failure_is_500_without_internal_detail(db: PgPool) {
    let app = build(db, Arc::new(BrokenRuntime), BASE);
    let session = signed_in_admin(&app).await;
    let body = json!({ "decisions": { "table": fixture("table") }, "key": "table" });
    let reply = send(&app.router, post("/api/v1/simulate", &body, Some(&session))).await;

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
    let app = without_database();
    let id = send(&app.router, get("/api/v1/health", None))
        .await
        .request_id
        .unwrap();
    assert_eq!(id.len(), 36, "uuid: {id}");
}

#[tokio::test]
async fn a_safe_incoming_request_id_is_reused() {
    let app = without_database();
    let req = Request::get("/api/v1/health")
        .header("x-request-id", "lb-7f3a.42")
        .body(axum::body::Body::empty())
        .unwrap();
    assert_eq!(
        send(&app.router, req).await.request_id.as_deref(),
        Some("lb-7f3a.42")
    );
}

#[tokio::test]
async fn an_unsafe_incoming_request_id_is_replaced() {
    let app = without_database();
    let req = Request::get("/api/v1/health")
        .header("x-request-id", "abc def")
        .body(axum::body::Body::empty())
        .unwrap();
    assert_ne!(send(&app.router, req).await.request_id.unwrap(), "abc def");
}

// --- contract ---------------------------------------------------------------

#[tokio::test]
async fn openapi_document_describes_the_endpoints() {
    let app = without_database();
    let reply = send(&app.router, get("/api/v1/openapi.json", None)).await;

    assert_eq!(reply.status, StatusCode::OK);
    assert_eq!(reply.body["servers"][0]["url"], "/api/v1");
    for (path, method) in [
        ("/health", "get"),
        ("/ready", "get"),
        ("/simulate", "post"),
        ("/auth/sign-in", "post"),
        ("/auth/sign-out", "post"),
        ("/auth/me", "get"),
        ("/auth/password-setup", "post"),
        ("/auth/password-reset", "post"),
        ("/users/invitations", "post"),
        ("/version", "get"),
        ("/projects", "get"),
        ("/projects", "post"),
        ("/projects/{project_id}", "get"),
        ("/projects/{project_id}", "patch"),
        ("/projects/{project_id}/archive", "post"),
        ("/projects/{project_id}/restore", "post"),
        ("/projects/{project_id}/members", "get"),
        ("/projects/{project_id}/members", "post"),
        ("/projects/{project_id}/members/{user_id}", "patch"),
        ("/projects/{project_id}/members/{user_id}", "delete"),
    ] {
        assert!(
            reply.body["paths"][path][method].is_object(),
            "{method} {path}"
        );
    }
    assert!(reply.body["components"]["schemas"]["ErrorBody"].is_object());
    assert_eq!(reply.body["info"]["version"], donka_app::VERSION);
}

/// The web app's typed client is generated from `apps/web/openapi.json`; this keeps
/// that copy in step with the handlers. Refresh it with
/// `DONKA_UPDATE_OPENAPI=1 cargo test -p donka-app --test api`, then
/// `pnpm --dir apps/web api:generate`.
#[tokio::test]
async fn the_web_app_has_the_current_openapi_document() {
    let app = without_database();
    let reply = send(&app.router, get("/api/v1/openapi.json", None)).await;
    let current = serde_json::to_string_pretty(&reply.body).unwrap() + "\n";
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../web/openapi.json");
    if std::env::var_os("DONKA_UPDATE_OPENAPI").is_some() {
        std::fs::write(path, &current).unwrap();
        return;
    }
    let committed = std::fs::read_to_string(path).unwrap_or_default();
    assert!(
        committed == current,
        "apps/web/openapi.json is out of date: run DONKA_UPDATE_OPENAPI=1 cargo test -p donka-app --test api, \
         then pnpm --dir apps/web api:generate"
    );
}
