//! The staging Runtime wired at Studio's first start (DNK-64): Studio issues its decision-log
//! token once, as the first administrator, into the file the Runtime reads.
#![allow(clippy::unwrap_used)] // tests fail loudly on purpose

mod support;

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use donka_app::wiring::{runtime_log_token, Wired, TOKEN_NAME};
use donka_db::PgPool;
use serde_json::json;
use support::*;

fn token_file() -> std::path::PathBuf {
    std::env::temp_dir()
        .join(format!("donka-wiring-{}", uuid::Uuid::new_v4()))
        .join("staging-decision-log-token")
}

/// Sends an empty batch with `token`, as the Runtime would.
async fn feed(app: &TestApp, token: &str) -> Reply {
    let req = Request::post(format!("{BASE}/decision-log/records"))
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::from(json!({ "records": [] }).to_string()))
        .unwrap();
    send(&app.router, req).await
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn the_first_start_issues_the_runtime_token_once(db: PgPool) {
    let app = with_database(db);
    let admin = signed_in_admin(&app).await;
    let file = token_file();

    let wired = runtime_log_token(&app.identity, &app.decision_log, &file, Some(ADMIN_EMAIL)).await;
    assert_eq!(wired, Ok(Wired::Issued));
    let token = std::fs::read_to_string(&file).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let mode = std::fs::metadata(&file).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }
    // The Runtime's batches are accepted with it.
    assert_eq!(feed(&app, &token).await.status, StatusCode::OK);

    // Administrators see it among the decision-log tokens, as staging's.
    let tokens = send(
        &app.router,
        get(&format!("{BASE}/decision-log/tokens"), Some(&admin)),
    )
    .await;
    let listed: Vec<_> = tokens.body["items"]
        .as_array()
        .or(tokens.body.as_array())
        .unwrap()
        .iter()
        .filter(|t| t["name"] == TOKEN_NAME)
        .collect();
    assert_eq!(listed.len(), 1, "{}", tokens.body);
    assert_eq!(listed[0]["environment"], "staging");

    // Later starts keep it: no second token, the file unchanged.
    let again = runtime_log_token(&app.identity, &app.decision_log, &file, Some(ADMIN_EMAIL)).await;
    assert_eq!(again, Ok(Wired::Already));
    assert_eq!(std::fs::read_to_string(&file).unwrap(), token);
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn without_an_administrator_nothing_is_written(db: PgPool) {
    let app = with_database(db);
    signed_in_admin(&app).await;
    signed_in_user(&app, "grace@bank.example", false).await;

    for email in [
        None,
        Some("grace@bank.example"),
        Some("nobody@bank.example"),
    ] {
        let file = token_file();
        let wired = runtime_log_token(&app.identity, &app.decision_log, &file, email).await;
        assert!(wired.is_err(), "{email:?}");
        assert!(!file.exists(), "{email:?}");
    }
}
