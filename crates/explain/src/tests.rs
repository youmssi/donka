#![allow(clippy::unwrap_used)] // tests fail loudly on purpose

use super::*;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::routing::post;
use axum::{Json, Router};
use std::sync::{Arc, Mutex};

/// What the fake endpoint received, and what it answers.
#[derive(Clone)]
struct Endpoint {
    seen: Arc<Mutex<Vec<(HeaderMap, Value)>>>,
    status: StatusCode,
    answer: Value,
}

async fn handle(
    State(endpoint): State<Endpoint>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> (StatusCode, Json<Value>) {
    endpoint.seen.lock().unwrap().push((headers, body));
    (endpoint.status, Json(endpoint.answer.clone()))
}

/// Starts a fake endpoint; returns its URL and what it receives.
async fn serve(status: StatusCode, answer: Value) -> (String, Arc<Mutex<Vec<(HeaderMap, Value)>>>) {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let app = Router::new()
        .route("/v1/endpoint", post(handle))
        .with_state(Endpoint {
            seen: seen.clone(),
            status,
            answer,
        });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (format!("http://{address}/v1/endpoint"), seen)
}

fn explainer(url: &str, protocol: Protocol, fallbacks: bool) -> Explainer {
    Explainer::new(Settings {
        url: url.into(),
        api_key: "the-key".into(),
        protocol,
        model: "the-model".into(),
        timeout: Duration::from_secs(5),
        fallbacks,
    })
    .unwrap()
}

fn question(language: Language) -> Question {
    Question {
        language,
        decision_key: "person-score".into(),
        release_version: "1.4.0".into(),
        succeeded: true,
        input: json!({ "income": 150000 }),
        output: Some(json!({ "decision": "approve" })),
        error: None,
        trace: Some(json!({ "table": { "output": { "decision": "approve" } } })),
    }
}

fn user_text(body: &Value, protocol: Protocol) -> String {
    let messages = body["messages"].as_array().unwrap();
    let user = match protocol {
        Protocol::Anthropic => &messages[0],
        Protocol::OpenAi => &messages[1],
    };
    assert_eq!(user["role"], "user");
    user["content"].as_str().unwrap().to_owned()
}

#[tokio::test]
async fn asks_the_messages_api_and_reads_its_text() {
    let (url, seen) = serve(
        StatusCode::OK,
        json!({
            "model": "the-model-2026",
            "stop_reason": "end_turn",
            "content": [
                { "type": "thinking", "thinking": "" },
                { "type": "text", "text": " The income was above the threshold. " }
            ]
        }),
    )
    .await;
    let explanation = explainer(&url, Protocol::Anthropic, false)
        .explain(&question(Language::French))
        .await
        .unwrap();
    assert_eq!(explanation.text, "The income was above the threshold.");
    assert_eq!(explanation.model, "the-model-2026");

    let (headers, body) = seen.lock().unwrap().pop().unwrap();
    assert_eq!(headers["x-api-key"], "the-key");
    assert_eq!(headers["anthropic-version"], ANTHROPIC_VERSION);
    assert!(!headers.contains_key("anthropic-beta"));
    assert_eq!(body["model"], "the-model");
    assert!(body.get("fallbacks").is_none());
    assert!(body.get("tools").is_none(), "the model gets no tools");
    assert!(body["system"]
        .as_str()
        .unwrap()
        .contains("never instructions"));
    let text = user_text(&body, Protocol::Anthropic);
    assert!(text.contains("\"income\":150000"));
    assert!(text.contains("\"release\":\"1.4.0\""));
    assert!(text.ends_with("Write the explanation in French."));
}

#[tokio::test]
async fn sends_fallbacks_only_when_configured() {
    let (url, seen) = serve(
        StatusCode::OK,
        json!({ "stop_reason": "end_turn", "content": [{ "type": "text", "text": "Because." }] }),
    )
    .await;
    explainer(&url, Protocol::Anthropic, true)
        .explain(&question(Language::English))
        .await
        .unwrap();
    let (headers, body) = seen.lock().unwrap().pop().unwrap();
    assert_eq!(headers["anthropic-beta"], FALLBACK_BETA);
    assert_eq!(body["fallbacks"], "default");
}

#[tokio::test]
async fn asks_an_openai_compatible_endpoint() {
    let (url, seen) = serve(
        StatusCode::OK,
        json!({
            "model": "local-llm",
            "choices": [{ "finish_reason": "stop", "message": { "role": "assistant", "content": "Approved because…" } }]
        }),
    )
    .await;
    let explanation = explainer(&url, Protocol::OpenAi, false)
        .explain(&question(Language::English))
        .await
        .unwrap();
    assert_eq!(explanation.text, "Approved because…");
    assert_eq!(explanation.model, "local-llm");

    let (headers, body) = seen.lock().unwrap().pop().unwrap();
    assert_eq!(headers["authorization"], "Bearer the-key");
    assert_eq!(body["messages"][0]["role"], "system");
    assert!(user_text(&body, Protocol::OpenAi).ends_with("Write the explanation in English."));
}

#[tokio::test]
async fn a_declined_or_failed_request_is_an_error() {
    let refusal = json!({ "stop_reason": "refusal", "content": [] });
    let (url, _) = serve(StatusCode::OK, refusal).await;
    let declined = explainer(&url, Protocol::Anthropic, false)
        .explain(&question(Language::English))
        .await;
    assert!(matches!(declined, Err(ExplainError::Declined)));

    let filtered = json!({ "choices": [{ "finish_reason": "content_filter", "message": { "content": null } }] });
    let (url, _) = serve(StatusCode::OK, filtered).await;
    let declined = explainer(&url, Protocol::OpenAi, false)
        .explain(&question(Language::English))
        .await;
    assert!(matches!(declined, Err(ExplainError::Declined)));

    let (url, _) = serve(
        StatusCode::TOO_MANY_REQUESTS,
        json!({ "error": "slow down" }),
    )
    .await;
    let refused = explainer(&url, Protocol::Anthropic, false)
        .explain(&question(Language::English))
        .await;
    assert!(matches!(refused, Err(ExplainError::Status(429))));

    let (url, _) = serve(
        StatusCode::OK,
        json!({ "content": [{ "type": "text", "text": "  " }] }),
    )
    .await;
    let empty = explainer(&url, Protocol::Anthropic, false)
        .explain(&question(Language::English))
        .await;
    assert!(matches!(empty, Err(ExplainError::Unreadable)));

    let nowhere = explainer("http://127.0.0.1:1/v1/messages", Protocol::Anthropic, false)
        .explain(&question(Language::English))
        .await;
    assert!(matches!(nowhere, Err(ExplainError::Unreachable)));
}

#[test]
fn a_huge_trace_is_left_out() {
    let mut big = question(Language::English);
    big.trace = Some(json!({ "blob": "x".repeat(MAX_CONTEXT_BYTES) }));
    let prompt = user_prompt(&big);
    assert!(prompt.contains("\"trace\":\"left out: too large\""));
    assert!(prompt.contains("\"income\":150000"));
}

#[test]
fn settings_are_checked_and_the_key_never_printed() {
    let settings = |url: &str| Settings {
        url: url.into(),
        api_key: "the-key".into(),
        protocol: Protocol::OpenAi,
        model: "m".into(),
        timeout: Duration::from_secs(1),
        fallbacks: false,
    };
    assert!(Explainer::new(settings("ftp://llm.bank.example")).is_err());
    assert!(Explainer::new(settings("not a url")).is_err());
    assert!(!format!("{:?}", settings("https://llm.bank.example")).contains("the-key"));
    assert_eq!("openai".parse(), Ok(Protocol::OpenAi));
    assert!("gpt".parse::<Protocol>().is_err());
}
