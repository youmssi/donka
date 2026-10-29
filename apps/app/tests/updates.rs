#![allow(clippy::unwrap_used)] // tests fail loudly on purpose
//! A newer Donka release, as administrators learn of it (DNK-64).

mod support;

use axum::http::StatusCode;
use axum::routing::get as route;
use axum::Router;
use donka_db::PgPool;
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
use support::*;

const GRACE: &str = "grace@bank.example";

/// A release feed answering whatever the test sets, on a free local port.
async fn feed(answer: (StatusCode, Value)) -> (String, Arc<Mutex<(StatusCode, Value)>>) {
    let answer = Arc::new(Mutex::new(answer));
    let shared = answer.clone();
    let app = Router::new().route(
        "/releases/latest",
        route(move || {
            let (status, body) = shared.lock().unwrap().clone();
            async move { (status, axum::Json(body)) }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/releases/latest", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (url, answer)
}

fn release(tag: &str) -> Value {
    json!({
        "tag_name": tag,
        "html_url": format!("https://github.com/youmssi/donka/releases/tag/{tag}"),
        "published_at": "2026-10-20T09:00:00Z",
        "draft": false,
        "prerelease": false
    })
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn administrators_learn_of_a_newer_release(db: PgPool) {
    let (url, answer) = feed((StatusCode::OK, release("v1.6.0"))).await;
    let app = with_update_feed(db, &url);
    let admin = signed_in_admin(&app).await;

    // Before the first check there is nothing to say.
    let before = send(&app.router, get("/api/v1/updates", Some(&admin))).await;
    assert_eq!(before.status, StatusCode::OK, "{}", before.body);
    assert_eq!(before.body["current"], "1.4.0");
    assert_eq!(before.body["checking"], true);
    assert_eq!(before.body["newer"], Value::Null);

    app.updates.check().await.unwrap();
    let after = send(&app.router, get("/api/v1/updates", Some(&admin))).await;
    assert_eq!(after.body["newer"]["version"], "1.6.0");
    assert_eq!(
        after.body["newer"]["url"],
        "https://github.com/youmssi/donka/releases/tag/v1.6.0"
    );
    assert_eq!(after.body["newer"]["publishedAt"], "2026-10-20T09:00:00Z");

    // A feed that fails keeps what Studio knew.
    *answer.lock().unwrap() = (StatusCode::BAD_GATEWAY, json!({}));
    assert!(app.updates.check().await.is_err());
    let failed = send(&app.router, get("/api/v1/updates", Some(&admin))).await;
    assert_eq!(failed.body["newer"]["version"], "1.6.0");

    // Once the installation runs the latest release, there is nothing newer.
    *answer.lock().unwrap() = (StatusCode::OK, release("v1.4.0"));
    app.updates.check().await.unwrap();
    let current = send(&app.router, get("/api/v1/updates", Some(&admin))).await;
    assert_eq!(current.body["newer"], Value::Null);
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn only_administrators_ask(db: PgPool) {
    let (url, _) = feed((StatusCode::OK, release("v1.6.0"))).await;
    let app = with_update_feed(db, &url);
    let _admin = signed_in_admin(&app).await;
    let grace = signed_in_user(&app, GRACE, false).await;
    let reply = send(&app.router, get("/api/v1/updates", Some(&grace))).await;
    assert_error(&reply, StatusCode::FORBIDDEN, "FORBIDDEN");
    let anonymous = send(&app.router, get("/api/v1/updates", None)).await;
    assert_error(&anonymous, StatusCode::UNAUTHORIZED, "UNAUTHENTICATED");
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn an_offline_installation_never_checks(db: PgPool) {
    let app = with_database(db);
    let admin = signed_in_admin(&app).await;
    app.updates.check().await.unwrap();
    let reply = send(&app.router, get("/api/v1/updates", Some(&admin))).await;
    assert_eq!(reply.body["checking"], false);
    assert_eq!(reply.body["newer"], Value::Null);
}
