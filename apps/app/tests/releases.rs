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
