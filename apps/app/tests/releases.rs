#![allow(clippy::unwrap_used)] // tests fail loudly on purpose

mod support;

use axum::http::StatusCode;
use donka_db::PgPool;
use serde_json::{json, Value};
use support::*;

const GRACE: &str = "grace@bank.example";
const ALAN: &str = "alan@bank.example";

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

fn fixture(name: &str) -> Value {
    let path = format!(
        "{}/../../crates/engine/tests/fixtures/{name}.json",
        env!("CARGO_MANIFEST_DIR")
    );
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

/// A graph whose only work is to call the decision `key` with its input.
fn calling(key: &str) -> Value {
    json!({
        "nodes": [
            { "id": "in", "name": "request", "type": "inputNode", "position": { "x": 0, "y": 0 } },
            { "id": "call", "name": "child", "type": "decisionNode", "content": { "key": key },
              "position": { "x": 200, "y": 0 } },
            { "id": "out", "name": "response", "type": "outputNode", "position": { "x": 400, "y": 0 } }
        ],
        "edges": [
            { "id": "e1", "type": "edge", "sourceId": "in", "targetId": "call" },
            { "id": "e2", "type": "edge", "sourceId": "call", "targetId": "out" }
        ]
    })
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

async fn add(app: &TestApp, owner: &str, project: &str, email: &str, role: &str) {
    let reply = call(
        app,
        "POST",
        &format!("/projects/{project}/members"),
        Some(json!({ "email": email, "role": role })),
        owner,
    )
    .await;
    assert_eq!(reply.status, StatusCode::CREATED, "{}", reply.body);
}

async fn decision(
    app: &TestApp,
    session: &str,
    project: &str,
    key: &str,
    content: Option<Value>,
) -> Value {
    let reply = call(
        app,
        "POST",
        &format!("/projects/{project}/decisions"),
        Some(json!({ "key": key, "content": content })),
        session,
    )
    .await;
    assert_eq!(reply.status, StatusCode::CREATED, "{}", reply.body);
    reply.body
}

async fn save_version(
    app: &TestApp,
    session: &str,
    base: &str,
    revision: i64,
    message: &str,
) -> Reply {
    call(
        app,
        "POST",
        &format!("{base}/versions"),
        Some(json!({ "message": message, "revision": revision })),
        session,
    )
    .await
}

async fn save_draft(app: &TestApp, session: &str, base: &str, content: Value, revision: i64) {
    let reply = call(
        app,
        "PUT",
        base,
        Some(json!({ "content": content, "revision": revision })),
        session,
    )
    .await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.body);
}

fn read_zip(bytes: Vec<u8>) -> std::collections::BTreeMap<String, Value> {
    use std::io::Read;
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut entries = std::collections::BTreeMap::new();
    for i in 0..zip.len() {
        let mut file = zip.by_index(i).unwrap();
        let mut text = String::new();
        file.read_to_string(&mut text).unwrap();
        entries.insert(file.name().to_owned(), serde_json::from_str(&text).unwrap());
    }
    entries
}

fn sha256(token: &str) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(token.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// A project with `bureau/normalize` (a table) and `person-score` (calling it),
/// both at version 1; returns the project id.
async fn versioned_project(app: &TestApp, admin: &str) -> String {
    let p = project(app, admin, "credit-pme").await;
    let n = decision(app, admin, &p, "bureau/normalize", Some(fixture("table"))).await;
    let s = decision(
        app,
        admin,
        &p,
        "person-score",
        Some(calling("bureau/normalize")),
    )
    .await;
    for d in [n, s] {
        let base = format!("/projects/{p}/decisions/{}", d["id"].as_str().unwrap());
        let v = save_version(app, admin, &base, 1, "First").await;
        assert_eq!(v.status, StatusCode::CREATED, "{}", v.body);
    }
    p
}

async fn release(app: &TestApp, session: &str, p: &str, bump: &str, notes: &str) -> Reply {
    call(
        app,
        "POST",
        &format!("/projects/{p}/releases"),
        Some(json!({ "bump": bump, "notes": notes })),
        session,
    )
    .await
}

async fn deploy(app: &TestApp, session: &str, p: &str, env: &str, release: &str) -> Reply {
    call(
        app,
        "POST",
        &format!("/projects/{p}/environments/{env}/deployments"),
        Some(json!({ "releaseId": release })),
        session,
    )
    .await
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn a_release_freezes_every_decision_with_a_semantic_version(db: PgPool) {
    let app = with_database(db.clone());
    let admin = signed_in_admin(&app).await;
    let p = project(&app, &admin, "credit-pme").await;

    // Nothing to release yet, then a decision without a version blocks it.
    assert_error(
        &release(&app, &admin, &p, "minor", "First").await,
        StatusCode::UNPROCESSABLE_ENTITY,
        "NOTHING_TO_RELEASE",
    );
    let n = decision(&app, &admin, &p, "bureau/normalize", Some(fixture("table"))).await;
    decision(
        &app,
        &admin,
        &p,
        "person-score",
        Some(calling("bureau/normalize")),
    )
    .await;
    let blocked = release(&app, &admin, &p, "minor", "First").await;
    assert_error(
        &blocked,
        StatusCode::UNPROCESSABLE_ENTITY,
        "UNVERSIONED_DECISIONS",
    );
    assert_eq!(
        blocked.body["details"]["keys"],
        json!(["bureau/normalize", "person-score"])
    );
    let preview = call(
        &app,
        "GET",
        &format!("/projects/{p}/releases/preview"),
        None,
        &admin,
    )
    .await;
    assert_eq!(preview.status, StatusCode::OK, "{}", preview.body);
    assert_eq!(
        preview.body["unversioned"],
        json!(["bureau/normalize", "person-score"])
    );
    assert_eq!(
        preview.body["next"],
        json!({ "major": "1.0.0", "minor": "1.0.0", "patch": "1.0.0" })
    );

    let nbase = format!("/projects/{p}/decisions/{}", n["id"].as_str().unwrap());
    save_version(&app, &admin, &nbase, 1, "Table").await;
    let sid = call(
        &app,
        "GET",
        &format!("/projects/{p}/decisions"),
        None,
        &admin,
    )
    .await
    .body["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["key"] == "person-score")
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned();
    save_version(
        &app,
        &admin,
        &format!("/projects/{p}/decisions/{sid}"),
        1,
        "Calls",
    )
    .await;

    for bad in ["", "  ", &"x".repeat(2001)] {
        let reply = release(&app, &admin, &p, "minor", bad).await;
        assert_error(&reply, StatusCode::BAD_REQUEST, "INVALID_REQUEST");
        assert!(reply.body["fields"]["notes"].is_string());
    }
    let first = release(&app, &admin, &p, "minor", " Launch of the credit rules ").await;
    assert_eq!(first.status, StatusCode::CREATED, "{}", first.body);
    assert_eq!(first.body["version"], "1.0.0");
    assert_eq!(first.body["notes"], "Launch of the credit rules");
    assert_eq!(first.body["decisions"].as_array().unwrap().len(), 2);
    assert_eq!(first.body["decisions"][0]["key"], "bureau/normalize");
    assert_eq!(first.body["decisions"][0]["version"], 1);
    assert_eq!(first.body["createdBy"]["email"], ADMIN_EMAIL);
    assert_eq!(first.body["liveIn"], json!([]));

    // Later versions of a decision do not change a release already made.
    save_draft(&app, &admin, &nbase, calling("person-score"), 1).await;
    save_version(&app, &admin, &nbase, 2, "Loop").await;
    let second = release(&app, &admin, &p, "minor", "Second").await;
    assert_eq!(second.body["version"], "1.1.0");
    assert_eq!(second.body["decisions"][0]["version"], 2);
    assert_eq!(
        release(&app, &admin, &p, "patch", "Fix").await.body["version"],
        "1.1.1"
    );
    assert_eq!(
        release(&app, &admin, &p, "major", "Big").await.body["version"],
        "2.0.0"
    );
    let id = first.body["id"].as_str().unwrap();
    let again = call(
        &app,
        "GET",
        &format!("/projects/{p}/releases/{id}"),
        None,
        &admin,
    )
    .await;
    assert_eq!(again.body["decisions"][0]["version"], 1);
    let list = call(
        &app,
        "GET",
        &format!("/projects/{p}/releases"),
        None,
        &admin,
    )
    .await;
    assert_eq!(list.body["total"], 4);
    assert_eq!(list.body["items"][0]["version"], "2.0.0");
    assert_error(
        &call(
            &app,
            "GET",
            &format!("/projects/{p}/releases/nope"),
            None,
            &admin,
        )
        .await,
        StatusCode::NOT_FOUND,
        "RELEASE_NOT_FOUND",
    );

    for statement in [
        "UPDATE releases SET notes = 'rewritten'",
        "DELETE FROM release_decisions",
        "TRUNCATE releases CASCADE",
    ] {
        let err = sqlx::query(statement).execute(&db).await.unwrap_err();
        assert_eq!(
            err.as_database_error().and_then(|e| e.code()).unwrap(),
            "42501",
            "{statement}"
        );
    }
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn deploying_to_staging_publishes_the_artifact_after_commit(db: PgPool) {
    let app = with_database(db);
    let admin = signed_in_admin(&app).await;
    let p = versioned_project(&app, &admin).await;

    let envs = call(
        &app,
        "GET",
        &format!("/projects/{p}/environments"),
        None,
        &admin,
    )
    .await;
    assert_eq!(envs.status, StatusCode::OK, "{}", envs.body);
    let names: Vec<&str> = envs.body["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["environment"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["staging", "production"]);
    assert!(envs.body["items"][0]["live"].is_null());

    let r = release(&app, &admin, &p, "minor", "Launch").await;
    let rid = r.body["id"].as_str().unwrap().to_owned();
    let token = call(
        &app,
        "POST",
        &format!("/projects/{p}/environments/staging/tokens"),
        Some(json!({ "name": "Loan origination" })),
        &admin,
    )
    .await;
    let token = token.body["token"].as_str().unwrap().to_owned();

    let queued = deploy(&app, &admin, &p, "staging", &rid).await;
    assert_eq!(queued.status, StatusCode::ACCEPTED, "{}", queued.body);
    assert_eq!(queued.body["status"], "pending");
    assert_eq!(queued.body["releaseVersion"], "1.0.0");
    assert!(
        app.store.get("staging/credit-pme").is_none(),
        "nothing written before the publisher runs"
    );

    assert_eq!(app.releases.publish_due().await.unwrap(), 1);
    let artifact = read_zip(app.store.get("staging/credit-pme").unwrap());
    assert_eq!(
        artifact.keys().collect::<Vec<_>>(),
        [".config/project.json", "bureau/normalize", "person-score"]
    );
    let config = &artifact[".config/project.json"];
    assert_eq!(config["version"], "2");
    assert_eq!(config["project"]["key"], "credit-pme");
    assert_eq!(config["release"]["version"], "1.0.0");
    assert_eq!(config["environment"]["key"], "staging");
    assert_eq!(config["accessTokenHashes"][0]["hash"], sha256(&token));
    assert_eq!(config["accessTokenHashes"][0]["environment"], "staging");
    assert!(config.get("accessTokens").is_none());
    assert_eq!(artifact["bureau/normalize"], fixture("table"));

    let envs = call(
        &app,
        "GET",
        &format!("/projects/{p}/environments"),
        None,
        &admin,
    )
    .await;
    assert_eq!(envs.body["items"][0]["live"]["releaseVersion"], "1.0.0");
    assert_eq!(envs.body["items"][0]["live"]["status"], "published");
    assert_eq!(envs.body["items"][0]["tokens"], 1);
    let listed = call(
        &app,
        "GET",
        &format!("/projects/{p}/releases"),
        None,
        &admin,
    )
    .await;
    assert_eq!(listed.body["items"][0]["liveIn"], json!(["staging"]));

    // Production goes through an approval (DNK-15).
    assert_error(
        &deploy(&app, &admin, &p, "production", &rid).await,
        StatusCode::UNPROCESSABLE_ENTITY,
        "APPROVAL_REQUIRED",
    );
    assert_error(
        &deploy(&app, &admin, &p, "dev", &rid).await,
        StatusCode::NOT_FOUND,
        "ENVIRONMENT_NOT_FOUND",
    );
    assert_error(
        &deploy(
            &app,
            &admin,
            &p,
            "staging",
            &uuid::Uuid::new_v4().to_string(),
        )
        .await,
        StatusCode::NOT_FOUND,
        "RELEASE_NOT_FOUND",
    );

    let log = call(
        &app,
        "GET",
        &format!("/projects/{p}/audit?action=release.deployed"),
        None,
        &admin,
    )
    .await;
    assert_eq!(
        log.body["items"][0]["details"],
        json!({ "version": "1.0.0", "environment": "staging" })
    );
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn a_storage_failure_is_retried_and_visible(db: PgPool) {
    let app = with_database(db);
    let admin = signed_in_admin(&app).await;
    let p = versioned_project(&app, &admin).await;
    let r = release(&app, &admin, &p, "minor", "Launch").await;
    let rid = r.body["id"].as_str().unwrap().to_owned();
    let queued = deploy(&app, &admin, &p, "staging", &rid).await;
    let did = queued.body["id"].as_str().unwrap().to_owned();
    let latest = |body: &Value| body["items"][0]["latest"].clone();

    app.store.fail_next(1);
    assert_eq!(app.releases.publish_due().await.unwrap(), 0);
    let envs = call(
        &app,
        "GET",
        &format!("/projects/{p}/environments"),
        None,
        &admin,
    )
    .await;
    let waiting = latest(&envs.body);
    assert_eq!(waiting["status"], "retrying");
    assert_eq!(waiting["attempts"], 1);
    assert!(waiting["lastError"]
        .as_str()
        .unwrap()
        .contains("unreachable"));
    assert!(waiting["nextAttemptAt"].is_string());

    // Not due yet; due once the delay has passed.
    assert_eq!(app.releases.publish_due().await.unwrap(), 0);
    app.clock.advance(chrono::Duration::minutes(1));
    assert_eq!(app.releases.publish_due().await.unwrap(), 1);
    let envs = call(
        &app,
        "GET",
        &format!("/projects/{p}/environments"),
        None,
        &admin,
    )
    .await;
    assert_eq!(latest(&envs.body)["status"], "published");
    assert!(latest(&envs.body)["lastError"].is_null());

    // After too many failures it gives up; it can then be retried by hand.
    let again = deploy(&app, &admin, &p, "staging", &rid).await;
    let again_id = again.body["id"].as_str().unwrap().to_owned();
    app.store.fail_next(usize::MAX);
    for _ in 0..PUBLISH_MAX_ATTEMPTS {
        app.releases.publish_due().await.unwrap();
        app.clock.advance(chrono::Duration::hours(2));
    }
    let envs = call(
        &app,
        "GET",
        &format!("/projects/{p}/environments"),
        None,
        &admin,
    )
    .await;
    assert_eq!(latest(&envs.body)["status"], "failed");
    assert_eq!(
        envs.body["items"][0]["live"]["id"],
        json!(did),
        "the live release stays"
    );
    let retry_path = format!("/projects/{p}/environments/staging/deployments/{again_id}/retry");
    app.store.fail_next(0);
    let retried = call(&app, "POST", &retry_path, None, &admin).await;
    assert_eq!(retried.status, StatusCode::ACCEPTED, "{}", retried.body);
    assert_eq!(retried.body["status"], "retrying");
    assert_eq!(app.releases.publish_due().await.unwrap(), 1);
    assert_error(
        &call(&app, "POST", &retry_path, None, &admin).await,
        StatusCode::CONFLICT,
        "NOT_RETRYABLE",
    );
    let history = call(
        &app,
        "GET",
        &format!("/projects/{p}/environments/staging/deployments"),
        None,
        &admin,
    )
    .await;
    assert_eq!(history.body["total"], 2);
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn a_newer_deployment_supersedes_one_not_yet_published(db: PgPool) {
    let app = with_database(db);
    let admin = signed_in_admin(&app).await;
    let p = versioned_project(&app, &admin).await;
    let r1 = release(&app, &admin, &p, "minor", "One").await;
    let r2 = release(&app, &admin, &p, "minor", "Two").await;
    deploy(&app, &admin, &p, "staging", r1.body["id"].as_str().unwrap()).await;
    deploy(&app, &admin, &p, "staging", r2.body["id"].as_str().unwrap()).await;
    assert_eq!(app.releases.publish_due().await.unwrap(), 1);
    let artifact = read_zip(app.store.get("staging/credit-pme").unwrap());
    assert_eq!(
        artifact[".config/project.json"]["release"]["version"],
        "1.1.0"
    );
    let history = call(
        &app,
        "GET",
        &format!("/projects/{p}/environments/staging/deployments"),
        None,
        &admin,
    )
    .await;
    let statuses: Vec<&str> = history.body["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["status"].as_str().unwrap())
        .collect();
    assert_eq!(statuses, ["published", "superseded"]);
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn owners_issue_and_revoke_tokens_shown_once(db: PgPool) {
    let app = with_database(db);
    let admin = signed_in_admin(&app).await;
    let editor = signed_in_user(&app, GRACE, false).await;
    let viewer = signed_in_user(&app, ALAN, false).await;
    let p = versioned_project(&app, &admin).await;
    add(&app, &admin, &p, GRACE, "editor").await;
    add(&app, &admin, &p, ALAN, "viewer").await;
    let tokens = format!("/projects/{p}/environments/staging/tokens");

    for bad in ["", &"x".repeat(101)] {
        let reply = call(&app, "POST", &tokens, Some(json!({ "name": bad })), &admin).await;
        assert_error(&reply, StatusCode::BAD_REQUEST, "INVALID_REQUEST");
    }
    let issued = call(
        &app,
        "POST",
        &tokens,
        Some(json!({ "name": "Loan origination" })),
        &admin,
    )
    .await;
    assert_eq!(issued.status, StatusCode::CREATED, "{}", issued.body);
    let token = issued.body["token"].as_str().unwrap().to_owned();
    assert!(token.starts_with("dnk_") && token.len() >= 40, "{token}");
    assert_eq!(issued.body["hint"], token[token.len() - 4..]);
    let tid = issued.body["id"].as_str().unwrap().to_owned();

    // The list never shows a token again; viewers read it, editors cannot issue.
    let list = call(&app, "GET", &tokens, None, &viewer).await;
    assert_eq!(list.status, StatusCode::OK);
    assert!(list.body["items"][0].get("token").is_none());
    assert!(!list.body.to_string().contains(&token));
    assert_error(
        &call(
            &app,
            "POST",
            &tokens,
            Some(json!({ "name": "Mine" })),
            &editor,
        )
        .await,
        StatusCode::FORBIDDEN,
        "FORBIDDEN",
    );
    // A staging token is not a production token.
    let production = call(
        &app,
        "GET",
        &format!("/projects/{p}/environments/production/tokens"),
        None,
        &admin,
    )
    .await;
    assert_eq!(production.body["items"], json!([]));

    // With a release live, a token change publishes it again with the new hashes.
    let r = release(&app, &admin, &p, "minor", "Launch").await;
    deploy(&app, &admin, &p, "staging", r.body["id"].as_str().unwrap()).await;
    app.releases.publish_due().await.unwrap();
    let second = call(
        &app,
        "POST",
        &tokens,
        Some(json!({ "name": "Batch scoring" })),
        &admin,
    )
    .await;
    let second = second.body["token"].as_str().unwrap().to_owned();
    assert_eq!(app.releases.publish_due().await.unwrap(), 1);
    let hashes = |app: &TestApp| -> Vec<Value> {
        read_zip(app.store.get("staging/credit-pme").unwrap())[".config/project.json"]
            ["accessTokenHashes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|h| h["hash"].clone())
            .collect()
    };
    assert_eq!(
        hashes(&app),
        [json!(sha256(&token)), json!(sha256(&second))]
    );

    assert_error(
        &call(&app, "DELETE", &format!("{tokens}/{tid}"), None, &editor).await,
        StatusCode::FORBIDDEN,
        "FORBIDDEN",
    );
    let revoked = call(&app, "DELETE", &format!("{tokens}/{tid}"), None, &admin).await;
    assert_eq!(revoked.status, StatusCode::NO_CONTENT);
    assert_eq!(app.releases.publish_due().await.unwrap(), 1);
    assert_eq!(hashes(&app), [json!(sha256(&second))]);
    assert_error(
        &call(&app, "DELETE", &format!("{tokens}/{tid}"), None, &admin).await,
        StatusCode::NOT_FOUND,
        "TOKEN_NOT_FOUND",
    );
    let list = call(&app, "GET", &tokens, None, &admin).await;
    assert!(list.body["items"][1]["revokedAt"].is_string());
    assert_eq!(list.body["items"][1]["revokedBy"]["email"], ADMIN_EMAIL);

    for action in ["token.issued", "token.revoked", "release.created"] {
        let log = call(
            &app,
            "GET",
            &format!("/projects/{p}/audit?action={action}"),
            None,
            &admin,
        )
        .await;
        assert!(log.body["total"].as_i64().unwrap() >= 1, "{action}");
    }
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn viewers_read_releases_but_cannot_release_or_deploy(db: PgPool) {
    let app = with_database(db);
    let admin = signed_in_admin(&app).await;
    let viewer = signed_in_user(&app, ALAN, false).await;
    let editor = signed_in_user(&app, GRACE, false).await;
    let p = versioned_project(&app, &admin).await;
    add(&app, &admin, &p, ALAN, "viewer").await;
    add(&app, &admin, &p, GRACE, "editor").await;

    assert_error(
        &release(&app, &viewer, &p, "minor", "Mine").await,
        StatusCode::FORBIDDEN,
        "FORBIDDEN",
    );
    let r = release(&app, &editor, &p, "minor", "Editors release").await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.body);
    let rid = r.body["id"].as_str().unwrap();
    assert_error(
        &deploy(&app, &viewer, &p, "staging", rid).await,
        StatusCode::FORBIDDEN,
        "FORBIDDEN",
    );
    assert_eq!(
        deploy(&app, &editor, &p, "staging", rid).await.status,
        StatusCode::ACCEPTED
    );
    assert_eq!(
        call(
            &app,
            "GET",
            &format!("/projects/{p}/releases"),
            None,
            &viewer
        )
        .await
        .status,
        StatusCode::OK
    );
    assert_eq!(
        call(
            &app,
            "GET",
            &format!("/projects/{p}/environments"),
            None,
            &viewer
        )
        .await
        .status,
        StatusCode::OK
    );
}

// ----- Production approvals (DNK-15) -------------------------------------

const ALICE: &str = "alice@bank.example";

async fn approval(app: &TestApp, session: &str, p: &str, release: &str) -> Reply {
    call(
        app,
        "POST",
        &format!("/projects/{p}/approvals"),
        Some(json!({ "releaseId": release })),
        session,
    )
    .await
}

async fn decide(
    app: &TestApp,
    session: &str,
    p: &str,
    id: &str,
    how: &str,
    body: Option<Value>,
) -> Reply {
    call(
        app,
        "POST",
        &format!("/projects/{p}/approvals/{id}/{how}"),
        body,
        session,
    )
    .await
}

/// Approval emails sent in one pass of the email worker.
async fn deliver_approvals(app: &TestApp) -> usize {
    app.releases
        .deliver_due_emails(app.mailer.as_ref())
        .await
        .unwrap()
}

/// The admin owns a versioned project with Grace (editor) and Alan (owner);
/// Grace makes release 1.0.0 and it is published to staging. Returns the
/// sessions (admin, grace, alan), the project and the release.
async fn staged(app: &TestApp) -> (String, String, String, String, String) {
    let admin = signed_in_admin(app).await;
    let grace = signed_in_user(app, GRACE, false).await;
    let alan = signed_in_user(app, ALAN, false).await;
    let p = versioned_project(app, &admin).await;
    add(app, &admin, &p, GRACE, "editor").await;
    add(app, &admin, &p, ALAN, "owner").await;
    let r = release(app, &grace, &p, "minor", "Raise the SME ceiling").await;
    let rid = r.body["id"].as_str().unwrap().to_owned();
    assert_eq!(
        deploy(app, &grace, &p, "staging", &rid).await.status,
        StatusCode::ACCEPTED
    );
    app.releases.publish_due().await.unwrap();
    (admin, grace, alan, p, rid)
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn production_is_published_after_an_owner_approves(db: PgPool) {
    let app = with_database(db);
    let (admin, grace, alan, p, rid) = staged(&app).await;

    // Production is not deployed directly.
    assert_error(
        &deploy(&app, &grace, &p, "production", &rid).await,
        StatusCode::UNPROCESSABLE_ENTITY,
        "APPROVAL_REQUIRED",
    );

    // Only the release live on staging can be asked for.
    let other = release(&app, &grace, &p, "patch", "Not on staging").await;
    assert_error(
        &approval(&app, &grace, &p, other.body["id"].as_str().unwrap()).await,
        StatusCode::UNPROCESSABLE_ENTITY,
        "NOT_ON_STAGING",
    );

    let asked = approval(&app, &grace, &p, &rid).await;
    assert_eq!(asked.status, StatusCode::CREATED, "{}", asked.body);
    assert_eq!(asked.body["status"], "pending");
    assert_eq!(asked.body["releaseVersion"], "1.0.0");
    assert_eq!(asked.body["requestedBy"]["email"], GRACE);
    let id = asked.body["id"].as_str().unwrap().to_owned();

    // The owners who may decide are emailed, with a link to the review.
    assert_eq!(deliver_approvals(&app).await, 2);
    for owner in [ADMIN_EMAIL, ALAN] {
        let email = emails_to(&app, owner).pop().unwrap();
        assert!(email.subject.contains("1.0.0"), "{}", email.subject);
        assert!(
            email.text.contains(&format!(
                "{PUBLIC_URL}/en/projects/approval/?p=credit-pme&a={id}"
            )),
            "{}",
            email.text
        );
    }
    assert!(emails_to(&app, GRACE)
        .iter()
        .all(|e| !e.subject.contains("approval")));

    // One request waits at a time.
    assert_error(
        &approval(&app, &grace, &p, &rid).await,
        StatusCode::CONFLICT,
        "APPROVAL_PENDING",
    );
    // Editors ask; owners decide.
    assert_error(
        &decide(&app, &grace, &p, &id, "approve", None).await,
        StatusCode::FORBIDDEN,
        "FORBIDDEN",
    );

    let approved = decide(&app, &alan, &p, &id, "approve", None).await;
    assert_eq!(approved.status, StatusCode::OK, "{}", approved.body);
    assert_eq!(approved.body["status"], "approved");
    assert_eq!(approved.body["decidedBy"]["email"], ALAN);
    assert!(approved.body["deploymentId"].is_string());
    assert!(app.store.get("production/credit-pme").is_none());
    assert_eq!(app.releases.publish_due().await.unwrap(), 1);
    let artifact = read_zip(app.store.get("production/credit-pme").unwrap());
    assert_eq!(
        artifact[".config/project.json"]["environment"]["key"],
        "production"
    );
    assert_eq!(
        artifact[".config/project.json"]["release"]["version"],
        "1.0.0"
    );

    let envs = call(
        &app,
        "GET",
        &format!("/projects/{p}/environments"),
        None,
        &admin,
    )
    .await;
    assert_eq!(envs.body["items"][1]["live"]["releaseVersion"], "1.0.0");
    let audit = call(&app, "GET", &format!("/projects/{p}/audit"), None, &admin).await;
    let actions: Vec<&str> = audit.body["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["action"].as_str().unwrap())
        .collect();
    assert!(actions.contains(&"approval.requested") && actions.contains(&"approval.approved"));

    // Decided once and for all.
    assert_error(
        &decide(&app, &admin, &p, &id, "approve", None).await,
        StatusCode::CONFLICT,
        "APPROVAL_DECIDED",
    );
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn the_releases_author_cannot_approve_it(db: PgPool) {
    let app = with_database(db);
    let (admin, _grace, alan, p, rid) = staged(&app).await;
    let alice = signed_in_user(&app, ALICE, false).await;
    add(&app, &admin, &p, ALICE, "owner").await;

    // Alan, an owner, asks: neither he nor Grace (who made the release) can approve.
    let asked = approval(&app, &alan, &p, &rid).await;
    assert_eq!(asked.status, StatusCode::CREATED, "{}", asked.body);
    let id = asked.body["id"].as_str().unwrap().to_owned();
    assert_error(
        &decide(&app, &alan, &p, &id, "approve", None).await,
        StatusCode::UNPROCESSABLE_ENTITY,
        "SELF_APPROVAL",
    );
    assert_error(
        &decide(
            &app,
            &alan,
            &p,
            &id,
            "reject",
            Some(json!({ "reason": "Mine" })),
        )
        .await,
        StatusCode::UNPROCESSABLE_ENTITY,
        "SELF_APPROVAL",
    );
    let review = call(
        &app,
        "GET",
        &format!("/projects/{p}/approvals/{id}"),
        None,
        &alan,
    )
    .await;
    assert_eq!(review.body["canDecide"], false);
    let review = call(
        &app,
        "GET",
        &format!("/projects/{p}/approvals/{id}"),
        None,
        &alice,
    )
    .await;
    assert_eq!(review.body["canDecide"], true);
    assert_eq!(
        decide(&app, &alice, &p, &id, "approve", None).await.status,
        StatusCode::OK
    );
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn a_project_needs_another_owner_to_approve(db: PgPool) {
    let app = with_database(db);
    let admin = signed_in_admin(&app).await;
    let p = versioned_project(&app, &admin).await;
    let r = release(&app, &admin, &p, "minor", "Solo").await;
    let rid = r.body["id"].as_str().unwrap().to_owned();
    deploy(&app, &admin, &p, "staging", &rid).await;
    app.releases.publish_due().await.unwrap();
    assert_error(
        &approval(&app, &admin, &p, &rid).await,
        StatusCode::UNPROCESSABLE_ENTITY,
        "NO_APPROVER",
    );
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn rejecting_records_a_reason_and_withdrawing_is_for_the_asker(db: PgPool) {
    let app = with_database(db);
    let (admin, grace, alan, p, rid) = staged(&app).await;
    let id = approval(&app, &grace, &p, &rid).await.body["id"]
        .as_str()
        .unwrap()
        .to_owned();

    let empty = decide(
        &app,
        &alan,
        &p,
        &id,
        "reject",
        Some(json!({ "reason": "  " })),
    )
    .await;
    assert_error(&empty, StatusCode::BAD_REQUEST, "INVALID_REQUEST");
    assert!(empty.body["fields"]["reason"].is_string(), "{}", empty.body);
    let rejected = decide(
        &app,
        &alan,
        &p,
        &id,
        "reject",
        Some(json!({ "reason": "The SME ceiling needs risk sign-off first." })),
    )
    .await;
    assert_eq!(rejected.status, StatusCode::OK, "{}", rejected.body);
    assert_eq!(rejected.body["status"], "rejected");
    assert_eq!(
        rejected.body["reason"],
        "The SME ceiling needs risk sign-off first."
    );
    assert!(app.store.get("production/credit-pme").is_none());
    assert_eq!(app.releases.publish_due().await.unwrap(), 0);

    // A new request can wait now; only Grace, who asked, can withdraw it.
    let again = approval(&app, &grace, &p, &rid).await;
    assert_eq!(again.status, StatusCode::CREATED, "{}", again.body);
    let again = again.body["id"].as_str().unwrap().to_owned();
    assert_error(
        &decide(&app, &admin, &p, &again, "withdraw", None).await,
        StatusCode::FORBIDDEN,
        "NOT_REQUESTER",
    );
    let withdrawn = decide(&app, &grace, &p, &again, "withdraw", None).await;
    assert_eq!(withdrawn.body["status"], "withdrawn", "{}", withdrawn.body);
    assert_error(
        &decide(&app, &alan, &p, &again, "approve", None).await,
        StatusCode::CONFLICT,
        "APPROVAL_DECIDED",
    );

    let listed = call(
        &app,
        "GET",
        &format!("/projects/{p}/approvals"),
        None,
        &grace,
    )
    .await;
    assert_eq!(listed.body["total"], 2);
    assert_eq!(listed.body["items"][0]["status"], "withdrawn");
    assert_eq!(listed.body["items"][1]["status"], "rejected");
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn two_owners_deciding_at_once_produce_one_outcome(db: PgPool) {
    let app = with_database(db);
    let (admin, grace, alan, p, rid) = staged(&app).await;
    let id = approval(&app, &grace, &p, &rid).await.body["id"]
        .as_str()
        .unwrap()
        .to_owned();

    let (first, second) = tokio::join!(
        decide(&app, &admin, &p, &id, "approve", None),
        decide(
            &app,
            &alan,
            &p,
            &id,
            "reject",
            Some(json!({ "reason": "Not this week" }))
        ),
    );
    let mut statuses = [first.status, second.status];
    statuses.sort();
    assert_eq!(
        statuses,
        [StatusCode::OK, StatusCode::CONFLICT],
        "{} / {}",
        first.body,
        second.body
    );
    let loser = if first.status == StatusCode::CONFLICT {
        &first
    } else {
        &second
    };
    assert_error(loser, StatusCode::CONFLICT, "APPROVAL_DECIDED");

    let review = call(
        &app,
        "GET",
        &format!("/projects/{p}/approvals/{id}"),
        None,
        &grace,
    )
    .await;
    let deployments = call(
        &app,
        "GET",
        &format!("/projects/{p}/environments/production/deployments"),
        None,
        &grace,
    )
    .await;
    let queued = deployments.body["total"].as_i64().unwrap();
    match review.body["status"].as_str().unwrap() {
        "approved" => assert_eq!(queued, 1),
        "rejected" => assert_eq!(queued, 0),
        other => panic!("unexpected outcome {other}"),
    }
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn the_review_shows_what_changes_the_tests_and_the_notes(db: PgPool) {
    let app = with_database(db);
    let (admin, grace, alan, p, rid) = staged(&app).await;
    let first = approval(&app, &grace, &p, &rid).await.body["id"]
        .as_str()
        .unwrap()
        .to_owned();
    decide(&app, &alan, &p, &first, "approve", None).await;
    app.releases.publish_due().await.unwrap();

    // A new version of person-score, released and staged.
    let decisions = call(
        &app,
        "GET",
        &format!("/projects/{p}/decisions"),
        None,
        &grace,
    )
    .await;
    let score = decisions.body["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["key"] == "person-score")
        .unwrap()
        .clone();
    let base = format!("/projects/{p}/decisions/{}", score["id"].as_str().unwrap());
    let revision = score["revision"].as_i64().unwrap();
    let mut changed = calling("bureau/normalize");
    changed["nodes"][1]["name"] = json!("normalize");
    // Version 2 also gets an input contract (DNK-37): `input` is now required.
    let contract = json!({
        "type": "object",
        "required": ["input"],
        "properties": { "input": { "type": "number" } }
    });
    changed["nodes"][0]["content"] = json!({ "schema": contract.to_string() });
    save_draft(&app, &grace, &base, changed, revision).await;
    let v = save_version(&app, &grace, &base, revision + 1, "Second").await;
    assert_eq!(v.status, StatusCode::CREATED, "{}", v.body);
    assert_eq!(
        v.body["warnings"],
        json!([]),
        "the child decision reads `input`"
    );
    let next = release(&app, &grace, &p, "minor", "Score v2 for SMEs").await;
    let next_id = next.body["id"].as_str().unwrap().to_owned();
    deploy(&app, &grace, &p, "staging", &next_id).await;
    app.releases.publish_due().await.unwrap();
    // The artifact carries the contract where Runtimes that do not know it never look.
    let artifact = read_zip(app.store.get("staging/credit-pme").unwrap());
    assert_eq!(
        artifact[".config/contracts/person-score/input.schema.json"],
        contract
    );
    assert!(!artifact.contains_key(".config/contracts/bureau/normalize/input.schema.json"));

    let asked = approval(&app, &grace, &p, &next_id).await;
    let id = asked.body["id"].as_str().unwrap();
    let review = call(
        &app,
        "GET",
        &format!("/projects/{p}/approvals/{id}"),
        None,
        &admin,
    )
    .await;
    assert_eq!(review.status, StatusCode::OK, "{}", review.body);
    assert_eq!(review.body["releaseVersion"], "1.1.0");
    assert_eq!(review.body["releaseNotes"], "Score v2 for SMEs");
    assert_eq!(review.body["productionVersion"], "1.0.0");
    let changes = review.body["changes"].as_array().unwrap();
    assert_eq!(changes[0]["key"], "person-score");
    assert_eq!(changes[0]["change"], "changed");
    assert_eq!(changes[0]["fromVersion"], 1);
    assert_eq!(changes[0]["toVersion"], 2);
    assert_eq!(
        changes[0]["contract"],
        json!([{ "path": "input", "kind": "added", "breaking": true }])
    );
    assert_eq!(changes[1]["key"], "bureau/normalize");
    assert_eq!(changes[1]["change"], "unchanged");
    assert_eq!(changes[1]["contract"], json!([]));
    assert!(review.body["tests"]["passed"].is_i64());
    assert_eq!(review.body["canDecide"], true);

    // The first request still compares with production as it was then.
    let old = call(
        &app,
        "GET",
        &format!("/projects/{p}/approvals/{first}"),
        None,
        &admin,
    )
    .await;
    assert!(old.body["productionVersion"].is_null(), "{}", old.body);
    assert_eq!(old.body["changes"][0]["change"], "added");
}

// ----- Rolling production back (DNK-16) ----------------------------------

async fn rollback(app: &TestApp, session: &str, p: &str, release: &str, reason: &str) -> Reply {
    call(
        app,
        "POST",
        &format!("/projects/{p}/rollbacks"),
        Some(json!({ "releaseId": release, "reason": reason })),
        session,
    )
    .await
}

/// Approves the request for `release` (live on staging) as Alan and publishes it.
async fn to_production(app: &TestApp, grace: &str, alan: &str, p: &str, release: &str) {
    let asked = approval(app, grace, p, release).await;
    assert_eq!(asked.status, StatusCode::CREATED, "{}", asked.body);
    let id = asked.body["id"].as_str().unwrap();
    let approved = decide(app, alan, p, id, "approve", None).await;
    assert_eq!(approved.status, StatusCode::OK, "{}", approved.body);
    app.releases.publish_due().await.unwrap();
}

/// Releases 1.0.0 then 1.1.0 to production; returns their ids.
async fn two_in_production(app: &TestApp, grace: &str, alan: &str, p: &str, first: &str) -> String {
    to_production(app, grace, alan, p, first).await;
    let second = release(app, grace, p, "minor", "Second").await;
    let second = second.body["id"].as_str().unwrap().to_owned();
    deploy(app, grace, p, "staging", &second).await;
    app.releases.publish_due().await.unwrap();
    to_production(app, grace, alan, p, &second).await;
    second
}

fn production_version(app: &TestApp) -> Value {
    read_zip(app.store.get("production/credit-pme").unwrap())[".config/project.json"]["release"]
        ["version"]
        .clone()
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn an_owner_rolls_production_back_with_a_reason(db: PgPool) {
    let app = with_database(db);
    let (admin, grace, alan, p, first) = staged(&app).await;
    two_in_production(&app, &grace, &alan, &p, &first).await;
    assert_eq!(production_version(&app), "1.1.0");

    let targets = call(
        &app,
        "GET",
        &format!("/projects/{p}/rollback-targets"),
        None,
        &grace,
    )
    .await;
    let versions: Vec<&str> = targets.body["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["version"].as_str().unwrap())
        .collect();
    assert_eq!(versions, ["1.0.0"], "the live release is not a target");

    // Grace made 1.0.0 and asked for it: as an owner she could still roll back
    // to it, but she is an editor, and only owners roll back.
    assert_error(
        &rollback(&app, &grace, &p, &first, "Scores too low").await,
        StatusCode::FORBIDDEN,
        "FORBIDDEN",
    );
    let empty = rollback(&app, &admin, &p, &first, "  ").await;
    assert_error(&empty, StatusCode::BAD_REQUEST, "INVALID_REQUEST");
    assert!(empty.body["fields"]["reason"].is_string(), "{}", empty.body);

    let back = rollback(&app, &admin, &p, &first, "1.1.0 rejects good SME files").await;
    assert_eq!(back.status, StatusCode::ACCEPTED, "{}", back.body);
    assert_eq!(back.body["reason"], "rollback");
    assert_eq!(back.body["rollbackReason"], "1.1.0 rejects good SME files");
    assert_eq!(back.body["releaseVersion"], "1.0.0");
    assert_eq!(back.body["environment"], "production");
    // No new approval: it is queued straight away and published.
    assert_eq!(app.releases.publish_due().await.unwrap(), 1);
    assert_eq!(production_version(&app), "1.0.0");

    let envs = call(
        &app,
        "GET",
        &format!("/projects/{p}/environments"),
        None,
        &admin,
    )
    .await;
    assert_eq!(envs.body["items"][1]["live"]["releaseVersion"], "1.0.0");
    assert_eq!(envs.body["items"][1]["live"]["reason"], "rollback");

    let audit = call(&app, "GET", &format!("/projects/{p}/audit"), None, &admin).await;
    let event = audit.body["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["action"] == "release.rolled_back")
        .unwrap()
        .clone();
    assert_eq!(event["actor"]["email"], ADMIN_EMAIL);
    assert_eq!(event["details"]["from"], "1.1.0");
    assert_eq!(event["details"]["version"], "1.0.0");
    assert_eq!(event["details"]["reason"], "1.1.0 rejects good SME files");

    // 1.1.0 can come back the same way (it was approved once).
    let targets = call(
        &app,
        "GET",
        &format!("/projects/{p}/rollback-targets"),
        None,
        &admin,
    )
    .await;
    assert_eq!(targets.body["items"][0]["version"], "1.1.0");
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn only_a_release_approved_for_production_can_come_back(db: PgPool) {
    let app = with_database(db);
    let (admin, grace, alan, p, first) = staged(&app).await;

    // Live on staging but never approved for production.
    assert_error(
        &rollback(&app, &admin, &p, &first, "Try it").await,
        StatusCode::UNPROCESSABLE_ENTITY,
        "NEVER_APPROVED",
    );
    // A rejected request does not count as an approval.
    let asked = approval(&app, &grace, &p, &first).await;
    decide(
        &app,
        &alan,
        &p,
        asked.body["id"].as_str().unwrap(),
        "reject",
        Some(json!({ "reason": "No" })),
    )
    .await;
    assert_error(
        &rollback(&app, &admin, &p, &first, "Try it").await,
        StatusCode::UNPROCESSABLE_ENTITY,
        "NEVER_APPROVED",
    );

    to_production(&app, &grace, &alan, &p, &first).await;
    assert_error(
        &rollback(&app, &admin, &p, &first, "Again").await,
        StatusCode::UNPROCESSABLE_ENTITY,
        "ALREADY_LIVE",
    );
    let unknown = "00000000-0000-4000-8000-000000000000";
    assert_error(
        &rollback(&app, &admin, &p, unknown, "Nope").await,
        StatusCode::NOT_FOUND,
        "RELEASE_NOT_FOUND",
    );
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn a_rollback_keeps_waiting_requests_and_survives_token_changes(db: PgPool) {
    let app = with_database(db);
    let (admin, grace, alan, p, first) = staged(&app).await;
    two_in_production(&app, &grace, &alan, &p, &first).await;

    // 1.2.0 waits for approval while production goes back to 1.0.0.
    let third = release(&app, &grace, &p, "minor", "Third").await;
    let third = third.body["id"].as_str().unwrap().to_owned();
    deploy(&app, &grace, &p, "staging", &third).await;
    app.releases.publish_due().await.unwrap();
    let waiting = approval(&app, &grace, &p, &third).await;
    let waiting = waiting.body["id"].as_str().unwrap().to_owned();

    assert_eq!(
        rollback(&app, &admin, &p, &first, "Incident").await.status,
        StatusCode::ACCEPTED
    );
    app.releases.publish_due().await.unwrap();
    let review = call(
        &app,
        "GET",
        &format!("/projects/{p}/approvals/{waiting}"),
        None,
        &admin,
    )
    .await;
    assert_eq!(review.body["status"], "pending");
    assert_eq!(
        review.body["productionVersion"], "1.0.0",
        "compared with production now"
    );

    // A new production token publishes production again: still 1.0.0.
    let token = call(
        &app,
        "POST",
        &format!("/projects/{p}/environments/production/tokens"),
        Some(json!({ "name": "Core banking" })),
        &admin,
    )
    .await;
    assert_eq!(token.status, StatusCode::CREATED, "{}", token.body);
    app.releases.publish_due().await.unwrap();
    assert_eq!(production_version(&app), "1.0.0");
}
