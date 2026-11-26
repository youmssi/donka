//! The demo option (DNK-64): at the first start, the retail-credit pack is imported, released
//! and deployed to staging as the first administrator.
#![allow(clippy::unwrap_used)] // tests fail loudly on purpose

mod support;

use axum::http::StatusCode;
use donka_app::demo;
use donka_db::PgPool;
use serde_json::Value;
use support::*;

async fn json(app: &TestApp, path: &str, session: &str) -> Value {
    let reply = send(&app.router, get(&format!("{BASE}{path}"), Some(session))).await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.body);
    reply.body
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn the_demo_releases_the_pack_to_staging(db: PgPool) {
    let app = with_database(db);
    let admin = signed_in_admin(&app).await;
    let catalogue = catalogue();

    let key = demo::set_up(
        &app.identity,
        &app.projects,
        &app.packs,
        &app.releases,
        &catalogue,
        ADMIN_EMAIL,
    )
    .await
    .unwrap();
    assert_eq!(key, "retail-credit");
    app.releases.publish_due().await.unwrap();

    let project = json(&app, "/projects/by-key/retail-credit", &admin).await;
    let id = project["id"].as_str().unwrap();
    let decisions = json(&app, &format!("/projects/{id}/decisions"), &admin).await;
    let keys: Vec<_> = decisions["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["key"].as_str().unwrap().to_owned())
        .collect();
    assert!(keys.contains(&"credit-application".to_owned()), "{keys:?}");

    // Its first release is live on staging, so the installed Runtime serves it.
    let environments = json(&app, &format!("/projects/{id}/environments"), &admin).await;
    let staging = environments
        .as_array()
        .or(environments["items"].as_array())
        .unwrap()
        .iter()
        .find(|e| e["environment"] == "staging")
        .unwrap()
        .clone();
    assert_eq!(staging["live"]["releaseVersion"], "1.0.0", "{staging}");
    assert_eq!(staging["live"]["status"], "published", "{staging}");

    // The Get started checklist shows the demo project's progress.
    let checklist = json(&app, "/onboarding", &admin).await;
    let done: Vec<_> = checklist["steps"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|s| s["done"] == true)
        .map(|s| s["key"].clone())
        .collect();
    assert!(done.len() >= 3, "{checklist}");
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn the_demo_needs_an_administrator(db: PgPool) {
    let app = with_database(db);
    signed_in_admin(&app).await;
    signed_in_user(&app, "grace@bank.example", false).await;
    for email in ["grace@bank.example", "nobody@bank.example"] {
        let failed = demo::set_up(
            &app.identity,
            &app.projects,
            &app.packs,
            &app.releases,
            &catalogue(),
            email,
        )
        .await;
        assert!(failed.is_err(), "{email}");
    }
}
