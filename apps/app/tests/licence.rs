//! The licence file (DNK-77): verified offline, it decides whether new versions and releases
//! are made, whether releases reach production, and which modules are offered. Reading and
//! exporting always work.
#![allow(clippy::unwrap_used)] // tests fail loudly on purpose

mod support;

use axum::http::StatusCode;
use base64::engine::general_purpose::URL_SAFE_NO_PAD as B64;
use base64::Engine as _;
use donka_db::PgPool;
use donka_licence::{Module, SigningKeyPair};
use serde_json::{json, Value};
use support::*;

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

async fn licence_of(app: &TestApp, session: &str) -> Value {
    let reply = call(app, "GET", "/licence", None, session).await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.body);
    reply.body
}

/// A graph that maps its input to its output, with a connector node or without.
fn graph(with_connector: bool) -> Value {
    let mut nodes = vec![
        json!({ "id": "in", "name": "Request", "type": "inputNode", "position": { "x": 0, "y": 0 } }),
        json!({ "id": "out", "name": "Response", "type": "outputNode", "position": { "x": 400, "y": 0 } }),
    ];
    let mut edges =
        vec![json!({ "id": "e1", "type": "edge", "sourceId": "in", "targetId": "out" })];
    if with_connector {
        nodes.push(json!({
            "id": "bureau", "name": "Bureau", "type": "customNode", "position": { "x": 200, "y": 0 },
            "content": { "kind": "donka.connector", "config": {
                "preset": "bureau-score",
                "url": "https://bureau.example/score",
                "auth": { "type": "header", "header": "X-Api-Key", "secret": "BUREAU_API_KEY" },
                "body": { "id": "{{ applicant.id }}" },
                "outputKey": "bureau",
                "mock": { "score": 700 }
            }}
        }));
        edges = vec![
            json!({ "id": "e1", "type": "edge", "sourceId": "in", "targetId": "bureau" }),
            json!({ "id": "e2", "type": "edge", "sourceId": "bureau", "targetId": "out" }),
        ];
    }
    json!({ "nodes": nodes, "edges": edges })
}

async fn project(app: &TestApp, session: &str, key: &str) -> String {
    let reply = call(
        app,
        "POST",
        "/projects",
        Some(json!({ "key": key, "name": format!("Project {key}") })),
        session,
    )
    .await;
    assert_eq!(reply.status, StatusCode::CREATED, "{}", reply.body);
    reply.body["id"].as_str().unwrap().to_owned()
}

/// A decision's path, created with `content`.
async fn decision(
    app: &TestApp,
    session: &str,
    project: &str,
    key: &str,
    content: Value,
) -> String {
    let reply = call(
        app,
        "POST",
        &format!("/projects/{project}/decisions"),
        Some(json!({ "key": key, "content": content })),
        session,
    )
    .await;
    assert_eq!(reply.status, StatusCode::CREATED, "{}", reply.body);
    format!(
        "/projects/{project}/decisions/{}",
        reply.body["id"].as_str().unwrap()
    )
}

async fn save_version(app: &TestApp, session: &str, decision: &str) -> Reply {
    call(
        app,
        "POST",
        &format!("{decision}/versions"),
        Some(json!({ "message": "First", "revision": 1 })),
        session,
    )
    .await
}

async fn release(app: &TestApp, session: &str, project: &str) -> Reply {
    call(
        app,
        "POST",
        &format!("/projects/{project}/releases"),
        Some(json!({ "bump": "minor", "notes": "First release" })),
        session,
    )
    .await
}

/// A project with one versioned decision; returns the project and the decision's path.
async fn versioned(app: &TestApp, session: &str) -> (String, String) {
    let p = project(app, session, "credit").await;
    let d = decision(app, session, &p, "score", graph(false)).await;
    let saved = save_version(app, session, &d).await;
    assert_eq!(saved.status, StatusCode::CREATED, "{}", saved.body);
    (p, d)
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn an_active_licence_names_the_customer_and_its_modules(db: PgPool) {
    let app = with_database(db);
    let admin = signed_in_admin(&app).await;
    install_signed(
        &app,
        &licence(&[Module::Explain], "2026-01-01", "2027-06-30"),
    );
    let shown = licence_of(&app, &admin).await;
    assert_eq!(shown["status"], "active");
    assert_eq!(shown["customer"], "Bank of Example");
    assert_eq!(shown["validFrom"], "2026-01-01");
    assert_eq!(shown["validUntil"], "2027-06-30");
    assert_eq!(shown["modules"], json!(["explain"]));
    assert_eq!(shown["allowsChanges"], true);
    assert_eq!(shown["allowsProduction"], true);
    assert_eq!(shown["expiring"], false);
    assert_eq!(shown["noticeDays"], 30);
    assert_eq!(shown["reason"], Value::Null);
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn evaluation_works_up_to_staging(db: PgPool) {
    let app = in_evaluation(db);
    let admin = signed_in_admin(&app).await;
    let shown = licence_of(&app, &admin).await;
    assert_eq!(shown["status"], "evaluation");
    assert_eq!(shown["allowsChanges"], true);
    assert_eq!(shown["allowsProduction"], false);
    assert_eq!(
        shown["modules"],
        json!(["explain", "connectors", "analysis"])
    );

    // Versions, releases and staging work; production does not.
    let (p, _) = versioned(&app, &admin).await;
    let made = release(&app, &admin, &p).await;
    assert_eq!(made.status, StatusCode::CREATED, "{}", made.body);
    let id = made.body["id"].as_str().unwrap();
    let staged = call(
        &app,
        "POST",
        &format!("/projects/{p}/environments/staging/deployments"),
        Some(json!({ "releaseId": id })),
        &admin,
    )
    .await;
    assert_eq!(staged.status, StatusCode::ACCEPTED, "{}", staged.body);
    app.releases.publish_due().await.unwrap();
    let asked = call(
        &app,
        "POST",
        &format!("/projects/{p}/approvals"),
        Some(json!({ "releaseId": id })),
        &admin,
    )
    .await;
    assert_error(&asked, StatusCode::FORBIDDEN, "PRODUCTION_NEEDS_LICENCE");
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn an_expired_licence_keeps_studio_readable_but_refuses_changes(db: PgPool) {
    let app = with_database(db);
    let admin = signed_in_admin(&app).await;
    let (p, d) = versioned(&app, &admin).await;
    // The test clock reads 2026-09-28.
    install_signed(&app, &licence(&Module::ALL, "2025-09-01", "2026-08-31"));

    let shown = licence_of(&app, &admin).await;
    assert_eq!(shown["status"], "expired");
    assert_eq!(shown["allowsChanges"], false);
    assert_eq!(shown["allowsProduction"], true);

    let refused = call(
        &app,
        "PUT",
        &d,
        Some(json!({ "content": graph(false), "revision": 1 })),
        &admin,
    )
    .await;
    // The draft still saves: only versions and releases wait for a licence.
    assert_eq!(refused.status, StatusCode::OK, "{}", refused.body);
    let version = call(
        &app,
        "POST",
        &format!("{d}/versions"),
        Some(json!({ "message": "Second", "revision": 2 })),
        &admin,
    )
    .await;
    assert_error(&version, StatusCode::FORBIDDEN, "LICENCE_INACTIVE");
    assert_eq!(version.body["details"]["status"], "expired");
    assert_error(
        &release(&app, &admin, &p).await,
        StatusCode::FORBIDDEN,
        "LICENCE_INACTIVE",
    );
    let imported = call(
        &app,
        "POST",
        "/packs/retail-credit/import",
        Some(json!({ "key": "copy", "name": "Copy" })),
        &admin,
    )
    .await;
    assert_error(&imported, StatusCode::FORBIDDEN, "LICENCE_INACTIVE");

    // Reading and exporting still work.
    let read = call(&app, "GET", &d, None, &admin).await;
    assert_eq!(read.status, StatusCode::OK, "{}", read.body);
    let versions = call(&app, "GET", &format!("{d}/versions"), None, &admin).await;
    assert_eq!(versions.status, StatusCode::OK, "{}", versions.body);
    let exported = call(&app, "GET", &format!("/projects/{p}/export"), None, &admin).await;
    assert_eq!(exported.status, StatusCode::OK);
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn administrators_are_told_thirty_days_before_the_end(db: PgPool) {
    let app = with_database(db);
    let admin = signed_in_admin(&app).await;
    // 2026-09-28 is 31 days before 2026-10-29, and 30 before 2026-10-28.
    install_signed(&app, &licence(&Module::ALL, "2026-01-01", "2026-10-29"));
    let shown = licence_of(&app, &admin).await;
    assert_eq!(
        (shown["expiring"].clone(), shown["daysLeft"].clone()),
        (json!(false), json!(31))
    );

    install_signed(&app, &licence(&Module::ALL, "2026-01-01", "2026-10-28"));
    let shown = licence_of(&app, &admin).await;
    assert_eq!(
        (shown["expiring"].clone(), shown["daysLeft"].clone()),
        (json!(true), json!(30))
    );

    // The day after the last one, the licence has expired without a reload.
    app.clock.advance(chrono::Duration::days(31));
    let admin = sign_in(&app, ADMIN_EMAIL, ADMIN_PASSWORD).await;
    let shown = licence_of(&app, &session_from(&admin)).await;
    assert_eq!(shown["status"], "expired");
    assert_eq!(shown["expiring"], false);
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn a_tampered_or_foreign_licence_is_refused_and_a_new_file_needs_no_restart(db: PgPool) {
    let app = with_database(db);
    let admin = signed_in_admin(&app).await;
    let (_, d) = versioned(&app, &admin).await;
    let genuine = app
        .licence_signer
        .issue(&licence(&[Module::Explain], "2026-01-01", "2026-12-31"))
        .unwrap();

    // Someone extends their own licence: the signature no longer matches.
    let mut file: Value = serde_json::from_str(&genuine).unwrap();
    let mut payload: Value =
        serde_json::from_slice(&B64.decode(file["payload"].as_str().unwrap()).unwrap()).unwrap();
    payload["validUntil"] = json!("2099-12-31");
    payload["modules"] = json!(["explain", "connectors", "analysis"]);
    file["payload"] = json!(B64.encode(serde_json::to_vec(&payload).unwrap()));
    install_licence(&app, &file.to_string());
    let shown = licence_of(&app, &admin).await;
    assert_eq!(shown["status"], "refused");
    assert!(
        shown["reason"]
            .as_str()
            .unwrap()
            .contains("changed after signing"),
        "{shown}"
    );
    assert_eq!(shown["refusal"], "tampered");
    assert_eq!(shown["modules"], json!([]));
    assert_eq!(shown["allowsChanges"], false);
    let version = call(
        &app,
        "POST",
        &format!("{d}/versions"),
        Some(json!({ "message": "Second", "revision": 1 })),
        &admin,
    )
    .await;
    assert_error(&version, StatusCode::FORBIDDEN, "LICENCE_INACTIVE");
    assert_eq!(version.body["details"]["status"], "refused");

    // A licence another key signed.
    let foreign = SigningKeyPair::from_seed([9; 32])
        .issue(&licence(&Module::ALL, "2026-01-01", "2099-12-31"))
        .unwrap();
    install_licence(&app, &foreign);
    let shown = licence_of(&app, &admin).await;
    assert_eq!(shown["status"], "refused");
    assert!(
        shown["reason"].as_str().unwrap().contains("another key"),
        "{shown}"
    );
    assert_eq!(shown["refusal"], "foreign");

    // Not a licence at all.
    install_licence(&app, "hello");
    let shown = licence_of(&app, &admin).await;
    assert_eq!(shown["status"], "refused");
    assert_eq!(shown["refusal"], "unreadable");

    // The genuine file, reloaded: valid again, without a restart.
    install_licence(&app, &genuine);
    let shown = licence_of(&app, &admin).await;
    assert_eq!(shown["status"], "active");
    assert_eq!(shown["modules"], json!(["explain"]));
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn modules_outside_the_licence_are_refused(db: PgPool) {
    let app = with_database(db);
    let admin = signed_in_admin(&app).await;
    install_signed(&app, &licence(&[], "2026-01-01", "2027-12-31"));
    assert_eq!(licence_of(&app, &admin).await["modules"], json!([]));

    let p = project(&app, &admin, "credit").await;
    let plain = decision(&app, &admin, &p, "plain", graph(false)).await;
    let saved = save_version(&app, &admin, &plain).await;
    assert_eq!(saved.status, StatusCode::CREATED, "{}", saved.body);

    // A connector node needs the connectors module.
    let calling = decision(&app, &admin, &p, "bureau", graph(true)).await;
    let refused = save_version(&app, &admin, &calling).await;
    assert_error(&refused, StatusCode::FORBIDDEN, "MODULE_NOT_LICENSED");
    assert_eq!(refused.body["details"]["module"], "connectors");

    // Explanations need the explain module.
    let explained = call(
        &app,
        "POST",
        &format!("/projects/{p}/decision-log/00000000-0000-0000-0000-000000000000/explain"),
        Some(json!({ "language": "en" })),
        &admin,
    )
    .await;
    assert_error(&explained, StatusCode::FORBIDDEN, "MODULE_NOT_LICENSED");
    assert_eq!(explained.body["details"]["module"], "explain");
    let settings = call(
        &app,
        "GET",
        &format!("/projects/{p}/decision-log/settings"),
        None,
        &admin,
    )
    .await;
    assert_eq!(settings.body["explainEnabled"], false);

    // With the module, the same version saves.
    install_signed(
        &app,
        &licence(&[Module::Connectors], "2026-01-01", "2027-12-31"),
    );
    let saved = save_version(&app, &admin, &calling).await;
    assert_eq!(saved.status, StatusCode::CREATED, "{}", saved.body);
}
