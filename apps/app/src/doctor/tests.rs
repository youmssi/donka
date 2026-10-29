#![allow(clippy::unwrap_used)] // tests fail loudly on purpose

use super::*;
use axum::http::StatusCode;
use axum::routing::post;
use axum::{Json, Router};
use serde_json::{json, Value};
use std::collections::HashMap;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;

const KEYS: [(&str, &str); 3] = [
    (
        "DONKA_DECISION_LOG_KEY",
        "AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8=",
    ),
    (
        "DONKA_SECOND_STEP_KEY",
        "ICEiIyQlJicoKSorLC0uLzAxMjM0NTY3ODk6Ozw9Pj8=",
    ),
    (
        "DONKA_PERSON_INDEX_KEY",
        "QEFCQ0RFRkdISUpLTE1OT1BRUlNUVVZXWFlaW1xdXl8=",
    ),
];

/// Studio's settings with `vars` over working defaults (a mail server and storage folder the
/// test provides).
fn config(vars: &[(&str, &str)]) -> Config {
    let mut map: HashMap<String, String> = KEYS
        .iter()
        .chain(&[
            ("DATABASE_URL", "postgres://donka:donka@127.0.0.1:1/donka"),
            ("DONKA_SMTP_URL", "smtp://127.0.0.1:1"),
            ("DONKA_SMTP_FROM", "donka@bank.example"),
            ("DONKA_STORAGE_URL", "file:///nonexistent"),
        ])
        .chain(vars)
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    map.retain(|_, v| !v.is_empty());
    Config::from_lookup(|k| map.get(k).cloned()).unwrap()
}

fn failing(check: &Check) -> (&str, &str) {
    match &check.outcome {
        Outcome::Failing { reason, fix } => (reason, fix),
        other => panic!("{} should fail: {other:?}", check.name),
    }
}

/// A port nothing listens on.
async fn closed_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    listener.local_addr().unwrap().port()
}

/// A mail server that greets, answers every command and never receives a message.
async fn fake_smtp() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        while let Ok((stream, _)) = listener.accept().await {
            tokio::spawn(async move {
                let (read, mut write) = stream.into_split();
                let mut lines = BufReader::new(read).lines();
                write.write_all(b"220 fake ESMTP\r\n").await.unwrap();
                while let Ok(Some(line)) = lines.next_line().await {
                    let command = line.to_ascii_uppercase();
                    let answer: &[u8] = if command.starts_with("EHLO") {
                        b"250 fake\r\n"
                    } else if command.starts_with("QUIT") {
                        b"221 bye\r\n"
                    } else if command.starts_with("DATA") || command.starts_with("MAIL") {
                        panic!("the check must not send a message");
                    } else {
                        b"250 OK\r\n"
                    };
                    if write.write_all(answer).await.is_err() || command.starts_with("QUIT") {
                        break;
                    }
                }
            });
        }
    });
    port
}

/// A language model endpoint answering `status` with `answer`.
async fn fake_model(status: StatusCode, answer: Value) -> String {
    let app = Router::new().route(
        "/v1/messages",
        post(move |Json(_): Json<Value>| {
            let answer = answer.clone();
            async move { (status, Json(answer)) }
        }),
    );
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    format!("http://{address}/v1/messages")
}

fn licence_key() -> PublicKey {
    donka_licence::SigningKeyPair::from_seed([7; 32]).public_key()
}

#[tokio::test]
async fn refused_settings_name_the_setting_and_stop() {
    let checks = run(
        Err(crate::config::ConfigError {
            var: "DONKA_SMTP_URL",
            reason: "is required".into(),
        }),
        licence_key(),
    )
    .await;
    assert_eq!(checks.len(), 1);
    let (reason, fix) = failing(&checks[0]);
    assert_eq!(reason, "DONKA_SMTP_URL is required");
    assert!(fix.contains("DONKA_SMTP_URL"), "{fix}");
}

#[tokio::test]
async fn an_unreachable_database_is_explained_without_its_password() {
    let port = closed_port().await;
    let url = format!("postgres://donka:s3cret-pass@127.0.0.1:{port}/donka");
    let config = config(&[("DATABASE_URL", &url)]);
    let mut checks = Vec::new();
    assert!(database(&config, &mut checks).await.is_none());
    let (reason, fix) = failing(&checks[0]);
    assert!(reason.starts_with("cannot reach PostgreSQL"), "{reason}");
    assert!(!reason.contains("s3cret-pass"));
    assert!(fix.contains("DATABASE_URL"), "{fix}");
}

#[tokio::test]
async fn a_reachable_database_counts_its_migrations() {
    // CI and the local checks provide PostgreSQL there.
    let Ok(url) = std::env::var("DATABASE_URL") else {
        return;
    };
    let mut checks = Vec::new();
    let pool = database(&config(&[("DATABASE_URL", &url)]), &mut checks).await;
    assert!(
        matches!(checks[0].outcome, Outcome::Working(_)),
        "{checks:?}"
    );
    assert!(pool.is_some());
}

#[tokio::test]
async fn storage_is_written_and_cleaned_up() {
    let dir = std::env::temp_dir().join(format!("donka-doctor-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let url = format!("file://{}", dir.display());
    let check = storage(&config(&[("DONKA_STORAGE_URL", &url)])).await;
    assert!(matches!(check.outcome, Outcome::Working(_)), "{check:?}");
    assert_eq!(
        std::fs::read_dir(&dir).unwrap().count(),
        0,
        "the test file is removed"
    );

    // A file where the folder should be: nothing can be written under it.
    let file = dir.join("not-a-folder");
    std::fs::write(&file, "").unwrap();
    let url = format!("file://{}", file.display());
    let check = storage(&config(&[("DONKA_STORAGE_URL", &url)])).await;
    let (_, fix) = failing(&check);
    assert!(fix.contains("DONKA_STORAGE_URL"), "{fix}");
    std::fs::remove_dir_all(&dir).unwrap();
}

#[tokio::test]
async fn mail_is_reached_without_sending_anything() {
    let port = fake_smtp().await;
    let url = format!("smtp://127.0.0.1:{port}");
    let check = mail(&config(&[("DONKA_SMTP_URL", &url)])).await;
    assert!(matches!(check.outcome, Outcome::Working(_)), "{check:?}");

    let port = closed_port().await;
    let url = format!("smtp://127.0.0.1:{port}");
    let check = mail(&config(&[("DONKA_SMTP_URL", &url)])).await;
    let (_, fix) = failing(&check);
    assert!(fix.contains("secrets/smtp_url"), "{fix}");
}

fn with_model(url: &str) -> Config {
    config(&[
        ("DONKA_EXPLAIN_URL", url),
        ("DONKA_EXPLAIN_API", "anthropic"),
        ("DONKA_EXPLAIN_MODEL", "the-model"),
        ("DONKA_EXPLAIN_API_KEY", "the-key"),
    ])
}

#[tokio::test]
async fn the_language_model_answers_or_says_why_not() {
    assert!(matches!(
        language_model(&config(&[])).await.outcome,
        Outcome::Off(_)
    ));

    let url = fake_model(
        StatusCode::OK,
        json!({ "model": "the-model-2026", "content": [{ "type": "text", "text": "OK" }] }),
    )
    .await;
    let check = language_model(&with_model(&url)).await;
    assert_eq!(
        check.outcome,
        Outcome::Working(format!("the-model-2026 answers at {url}"))
    );

    let url = fake_model(StatusCode::UNAUTHORIZED, json!({})).await;
    let check = language_model(&with_model(&url)).await;
    let (reason, fix) = failing(&check);
    assert!(reason.contains("refused the key"), "{reason}");
    assert!(fix.contains("secrets/explain_api_key"), "{fix}");

    let url = fake_model(StatusCode::NOT_FOUND, json!({})).await;
    let check = language_model(&with_model(&url)).await;
    let (_, fix) = failing(&check);
    assert!(fix.contains("DONKA_EXPLAIN_MODEL"), "{fix}");
}

#[test]
fn the_licence_is_verified_offline() {
    let key = licence_key();
    assert!(matches!(
        licence(&config(&[]), &key).outcome,
        Outcome::Off(_)
    ));

    let file = std::env::temp_dir().join(format!("donka-licence-{}.json", uuid::Uuid::new_v4()));
    std::fs::write(&file, "{ not a licence").unwrap();
    let path = file.display().to_string();
    let check = licence(&config(&[("DONKA_LICENCE_FILE", &path)]), &key);
    let (_, fix) = failing(&check);
    assert!(fix.contains("licence"), "{fix}");
    std::fs::remove_file(&file).unwrap();
}

#[tokio::test]
async fn connector_endpoints_are_reached_without_a_request() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let open = listener.local_addr().unwrap();
    assert_eq!(reach(&format!("http://{open}/score")).await, Ok(()));

    let port = closed_port().await;
    let refused = reach(&format!("https://127.0.0.1:{port}/score"))
        .await
        .unwrap_err();
    assert!(refused.contains("refused the connection"), "{refused}");

    let unknown = reach("https://bureau.invalid/score").await.unwrap_err();
    assert!(unknown.contains("does not resolve"), "{unknown}");
}

#[test]
fn the_report_puts_the_fix_under_a_failing_check() {
    let checks = [
        Check::working("database", "reachable, every migration applied"),
        Check::failing("mail", "connection refused", "check secrets/smtp_url"),
        Check::off("licence", "none: evaluation"),
    ];
    let report = render(&checks);
    assert_eq!(
        report,
        "  ok    database  reachable, every migration applied\n  \
         FAIL  mail      connection refused\n                  \
         → check secrets/smtp_url\n  \
         off   licence   none: evaluation\n\
         doctor: 1 check failed\n"
    );
    assert_eq!(
        render(&checks[..1]).lines().last(),
        Some("doctor: everything Studio depends on works")
    );
}
