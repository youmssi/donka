#![allow(clippy::unwrap_used)] // tests fail loudly on purpose

use super::*;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::routing::post;
use axum::{Json, Router};
use std::sync::atomic::{AtomicUsize, Ordering};

#[test]
fn percentiles_are_nearest_rank() {
    let latencies: Vec<Duration> = (1..=100).map(Duration::from_millis).collect();
    let measured = report(latencies, 2, Duration::from_secs(10));
    assert_eq!(measured.requests, 100);
    assert_eq!(measured.errors, 2);
    assert_eq!(measured.throughput, 10.0);
    assert_eq!(
        (
            measured.p50_ms,
            measured.p95_ms,
            measured.p99_ms,
            measured.max_ms
        ),
        (50.0, 95.0, 99.0, 100.0)
    );
    assert_eq!(report(Vec::new(), 0, Duration::from_secs(1)).p99_ms, 0.0);
}

#[derive(Clone)]
struct Runtime {
    delay: Duration,
    status: StatusCode,
    calls: Arc<AtomicUsize>,
    tokens: Arc<std::sync::Mutex<Vec<String>>>,
}

async fn evaluate(
    State(runtime): State<Runtime>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> (StatusCode, Json<Value>) {
    assert!(body["context"].is_object(), "{body}");
    runtime.calls.fetch_add(1, Ordering::Relaxed);
    if let Some(token) = headers.get("x-access-token") {
        runtime
            .tokens
            .lock()
            .unwrap()
            .push(token.to_str().unwrap().to_owned());
    }
    tokio::time::sleep(runtime.delay).await;
    (runtime.status, Json(json!({ "result": { "ok": true } })))
}

async fn runtime(delay_ms: u64, status: StatusCode) -> (String, Runtime) {
    let runtime = Runtime {
        delay: Duration::from_millis(delay_ms),
        status,
        calls: Arc::default(),
        tokens: Arc::default(),
    };
    let app = Router::new()
        .route("/evaluate", post(evaluate))
        .with_state(runtime.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (format!("http://{address}/evaluate"), runtime)
}

fn options(url: String, load: Load) -> Options {
    Options {
        url,
        token: Some("dnk_test".to_owned()),
        inputs: vec![json!({ "income": 1 }), json!({ "income": 2 })],
        load,
        duration: Duration::from_millis(1000),
        warmup: Duration::from_millis(200),
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn at_a_rate_every_request_leaves_on_schedule_and_is_timed() {
    let (url, runtime) = runtime(20, StatusCode::OK).await;
    let report = run(options(url, Load::Rate(50))).await.unwrap();
    // 50 a second for 1.2 s, of which the first 0.2 s are not counted.
    assert!((45..=55).contains(&report.requests), "{report:?}");
    assert!((55..=65).contains(&runtime.calls.load(Ordering::Relaxed)));
    assert_eq!(report.sent, runtime.calls.load(Ordering::Relaxed));
    assert_eq!(report.answered, report.sent);
    assert_eq!(report.errors, 0);
    assert!(report.p50_ms >= 20.0 && report.p99_ms < 200.0, "{report:?}");
    assert!(runtime
        .tokens
        .lock()
        .unwrap()
        .iter()
        .all(|t| t == "dnk_test"));
}

#[tokio::test(flavor = "multi_thread")]
async fn as_fast_as_it_answers_the_callers_wait_for_each_answer() {
    let (url, _) = runtime(10, StatusCode::OK).await;
    let report = run(options(url, Load::Concurrency(4))).await.unwrap();
    // 4 callers at about 10 ms a request: about 400 a second, never more.
    assert!(
        report.throughput > 150.0 && report.throughput <= 400.0,
        "{report:?}"
    );
    assert!(report.p50_ms >= 10.0, "{report:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn refused_and_unanswered_requests_are_errors() {
    let (url, _) = runtime(1, StatusCode::UNPROCESSABLE_ENTITY).await;
    let report = run(options(url, Load::Rate(20))).await.unwrap();
    assert_eq!(report.errors, report.requests);
    assert_eq!(report.answered, 0);
    let report = run(options(
        "http://127.0.0.1:1/evaluate".to_owned(),
        Load::Rate(20),
    ))
    .await
    .unwrap();
    assert_eq!(report.errors, report.requests);
    assert!(report.requests > 0);
}
