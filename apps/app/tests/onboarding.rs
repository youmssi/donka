#![allow(clippy::unwrap_used)] // tests fail loudly on purpose

//! First-run onboarding (DNK-41): the Get started checklist follows what really happened in the
//! user's projects, and each user's tours seen are kept on the server.

mod support;

use axum::http::StatusCode;
use donka_db::PgPool;
use serde_json::{json, Value};
use support::*;

const GRACE: &str = "grace@bank.example";

async fn call(
    app: &TestApp,
    method: &str,
    path: &str,
    body: Option<Value>,
    session: &str,
) -> Reply {
    send(
        &app.router,
        request(
            method,
            &format!("{BASE}{path}"),
            body.as_ref(),
            Some(session),
        ),
    )
    .await
}

/// Which steps are done, in order.
async fn done(app: &TestApp, session: &str) -> (Vec<bool>, Value) {
    let reply = call(app, "GET", "/onboarding", None, session).await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.body);
    let steps = reply.body["steps"].as_array().unwrap();
    let names: Vec<&str> = steps.iter().map(|s| s["step"].as_str().unwrap()).collect();
    assert_eq!(
        names,
        ["project", "simulation", "version", "staging", "token"]
    );
    let done = steps.iter().map(|s| s["done"].as_bool().unwrap()).collect();
    (done, reply.body)
}

/// Imports the retail-credit pack under `key`: a project whose decisions have a version.
async fn imported(app: &TestApp, session: &str, key: &str) -> String {
    let reply = call(
        app,
        "POST",
        "/packs/retail-credit/import",
        Some(json!({ "key": key, "name": key })),
        session,
    )
    .await;
    assert_eq!(reply.status, StatusCode::CREATED, "{}", reply.body);
    reply.body["project"]["id"].as_str().unwrap().to_owned()
}

async fn simulate_scorecard(app: &TestApp, session: &str, project: &str) -> Reply {
    let decisions = call(
        app,
        "GET",
        &format!("/projects/{project}/decisions"),
        None,
        session,
    )
    .await;
    let scorecard = decisions.body["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["key"] == "scorecard")
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let scenarios: Value = serde_json::from_str(
        &std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../packs/retail-credit/scenarios.json"
        ))
        .unwrap(),
    )
    .unwrap();
    let input = scenarios
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["decision"] == "scorecard")
        .unwrap()["input"]
        .clone();
    call(
        app,
        "POST",
        &format!("/projects/{project}/decisions/{scorecard}/simulate"),
        Some(json!({ "context": input })),
        session,
    )
    .await
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn the_checklist_follows_what_happened_in_the_project(db: PgPool) {
    let app = with_database(db);
    let admin = signed_in_admin(&app).await;

    let (steps, body) = done(&app, &admin).await;
    assert_eq!(steps, [false; 5]);
    assert_eq!(body["project"], Value::Null);
    assert_eq!(body["complete"], false);

    // A starter pack makes a project whose decisions already have a version.
    let project = imported(&app, &admin, "retail-credit").await;
    let (steps, body) = done(&app, &admin).await;
    assert_eq!(steps, [true, false, true, false, false]);
    assert_eq!(
        body["project"],
        json!({ "key": "retail-credit", "name": "retail-credit" })
    );

    // A simulation that fails is not one that ran.
    let failed = call(
        &app,
        "POST",
        &format!(
            "/projects/{project}/decisions/{}/simulate",
            uuid::Uuid::new_v4()
        ),
        Some(json!({ "context": {} })),
        &admin,
    )
    .await;
    assert_eq!(failed.status, StatusCode::NOT_FOUND);
    assert!(!done(&app, &admin).await.0[1]);
    let simulated = simulate_scorecard(&app, &admin, &project).await;
    assert_eq!(simulated.status, StatusCode::OK, "{}", simulated.body);
    assert_eq!(done(&app, &admin).await.0, [true, true, true, false, false]);

    // Live on staging only once the deployment is published.
    let release = call(
        &app,
        "POST",
        &format!("/projects/{project}/releases"),
        Some(json!({ "bump": "minor", "notes": "First" })),
        &admin,
    )
    .await;
    assert_eq!(release.status, StatusCode::CREATED, "{}", release.body);
    let deployed = call(
        &app,
        "POST",
        &format!("/projects/{project}/environments/staging/deployments"),
        Some(json!({ "releaseId": release.body["id"] })),
        &admin,
    )
    .await;
    assert_eq!(deployed.status, StatusCode::ACCEPTED, "{}", deployed.body);
    assert!(!done(&app, &admin).await.0[3]);
    app.releases.publish_due().await.unwrap();
    assert_eq!(done(&app, &admin).await.0, [true, true, true, true, false]);

    let token = call(
        &app,
        "POST",
        &format!("/projects/{project}/environments/staging/tokens"),
        Some(json!({ "name": "Loan origination" })),
        &admin,
    )
    .await;
    assert_eq!(token.status, StatusCode::CREATED, "{}", token.body);
    let (steps, body) = done(&app, &admin).await;
    assert_eq!(steps, [true; 5]);
    assert_eq!(body["complete"], true);

    // A newer, empty project does not take the checklist back to the start.
    imported(&app, &admin, "another").await;
    let (steps, body) = done(&app, &admin).await;
    assert_eq!(steps, [true; 5]);
    assert_eq!(body["project"]["key"], "retail-credit");
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn the_checklist_reads_only_the_users_own_projects(db: PgPool) {
    let app = with_database(db);
    let admin = signed_in_admin(&app).await;
    let grace = signed_in_user(&app, GRACE, false).await;
    let project = imported(&app, &admin, "retail-credit").await;
    simulate_scorecard(&app, &admin, &project).await;

    // Not a member: nothing of the admin's project shows.
    assert_eq!(done(&app, &grace).await.0, [false; 5]);

    // A member sees where the project is, whoever did the work.
    let added = call(
        &app,
        "POST",
        &format!("/projects/{project}/members"),
        Some(json!({ "email": GRACE, "role": "viewer" })),
        &admin,
    )
    .await;
    assert_eq!(added.status, StatusCode::CREATED, "{}", added.body);
    assert_eq!(done(&app, &grace).await.0, [true, true, true, false, false]);

    // An archived project no longer counts.
    let archived = call(
        &app,
        "POST",
        &format!("/projects/{project}/archive"),
        None,
        &admin,
    )
    .await;
    assert_eq!(archived.status, StatusCode::OK, "{}", archived.body);
    assert_eq!(done(&app, &grace).await.0, [false; 5]);
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn each_user_sees_a_tour_once_on_any_device(db: PgPool) {
    let app = with_database(db);
    let admin = signed_in_admin(&app).await;
    let grace = signed_in_user(&app, GRACE, false).await;

    let seen = |session: String| {
        let app = &app;
        async move {
            let reply = call(app, "GET", "/me/tours", None, &session).await;
            assert_eq!(reply.status, StatusCode::OK, "{}", reply.body);
            reply.body["seen"].clone()
        }
    };
    assert_eq!(seen(admin.clone()).await, json!([]));
    for _ in 0..2 {
        let reply = call(&app, "PUT", "/me/tours/editor", None, &admin).await;
        assert_eq!(reply.status, StatusCode::NO_CONTENT, "{}", reply.body);
    }
    call(&app, "PUT", "/me/tours/releases", None, &admin).await;
    assert_eq!(seen(admin.clone()).await, json!(["editor", "releases"]));

    // Another device of the same user: a new session sees the same.
    let again = session_from(&sign_in(&app, ADMIN_EMAIL, ADMIN_PASSWORD).await);
    assert_eq!(seen(again).await, json!(["editor", "releases"]));
    // Another user has seen none.
    assert_eq!(seen(grace).await, json!([]));

    assert_error(
        &call(&app, "PUT", "/me/tours/settings", None, &admin).await,
        StatusCode::BAD_REQUEST,
        "INVALID_REQUEST",
    );
    let anonymous = send(&app.router, get(&format!("{BASE}/me/tours"), None)).await;
    assert_error(&anonymous, StatusCode::UNAUTHORIZED, "UNAUTHENTICATED");
}
