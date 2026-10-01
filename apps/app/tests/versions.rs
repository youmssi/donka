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

/// A project with one decision owned by the administrator; returns (project, decision id).
async fn setup(app: &TestApp, admin: &str) -> (String, String) {
    let p = project(app, admin, "credit").await;
    let d = decision(app, admin, &p, "person-score", Some(fixture("table"))).await;
    (p, d["id"].as_str().unwrap().to_owned())
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

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn save_version_keeps_the_draft_with_its_author_time_and_message(db: PgPool) {
    let app = with_database(db);
    let admin = signed_in_admin(&app).await;
    let (p, id) = setup(&app, &admin).await;
    let base = format!("/projects/{p}/decisions/{id}");

    let fresh = call(&app, "GET", &base, None, &admin).await;
    assert!(fresh.body["latestVersion"].is_null());
    assert_eq!(fresh.body["changedSinceVersion"], true);

    let v1 = save_version(&app, &admin, &base, 1, "  First cut of the table  ").await;
    assert_eq!(v1.status, StatusCode::CREATED, "{}", v1.body);
    assert_eq!(v1.body["number"], 1);
    assert_eq!(v1.body["message"], "First cut of the table");
    assert_eq!(v1.body["createdBy"]["email"], ADMIN_EMAIL);
    assert!(v1.body["createdAt"].is_string());

    let now = call(&app, "GET", &base, None, &admin).await;
    assert_eq!(now.body["latestVersion"], 1);
    assert_eq!(now.body["changedSinceVersion"], false);

    // Nothing changed: no empty version.
    let again = save_version(&app, &admin, &base, 1, "Again").await;
    assert_error(
        &again,
        StatusCode::UNPROCESSABLE_ENTITY,
        "VERSION_UNCHANGED",
    );
    assert_eq!(again.body["details"]["version"], 1);

    // Saving the same graph again (as an editor does when it opens) changes nothing.
    save_draft(&app, &admin, &base, fixture("table"), 1).await;
    let same = call(&app, "GET", &base, None, &admin).await;
    assert_eq!(same.body["revision"], 2);
    assert_eq!(same.body["changedSinceVersion"], false);
    let still = save_version(&app, &admin, &base, 2, "Again").await;
    assert_error(
        &still,
        StatusCode::UNPROCESSABLE_ENTITY,
        "VERSION_UNCHANGED",
    );

    // The draft moves on; the version keeps what was saved.
    save_draft(&app, &admin, &base, calling("other"), 2).await;
    let list = call(
        &app,
        "GET",
        &format!("/projects/{p}/decisions"),
        None,
        &admin,
    )
    .await;
    assert_eq!(list.body["items"][0]["changedSinceVersion"], true);
    let kept = call(&app, "GET", &format!("{base}/versions/1"), None, &admin).await;
    assert_eq!(kept.body["content"], fixture("table"));

    // A save names the draft revision it shows; a stale one is a conflict.
    let stale = save_version(&app, &admin, &base, 2, "Stale").await;
    assert_error(&stale, StatusCode::CONFLICT, "DECISION_CONFLICT");
    assert_eq!(stale.body["details"]["revision"], 3);
    for bad in ["", "   ", &"x".repeat(501)] {
        let reply = save_version(&app, &admin, &base, 3, bad).await;
        assert_error(&reply, StatusCode::BAD_REQUEST, "INVALID_REQUEST");
        assert!(reply.body["fields"]["message"].is_string());
    }
    let v2 = save_version(&app, &admin, &base, 3, "Calls the other decision").await;
    assert_eq!(v2.body["number"], 2);
    let v2_content = call(&app, "GET", &format!("{base}/versions/2"), None, &admin).await;
    assert_eq!(v2_content.body["content"], calling("other"));

    let log = call(
        &app,
        "GET",
        &format!("/projects/{p}/audit?action=decision.version_saved"),
        None,
        &admin,
    )
    .await;
    assert_eq!(log.body["total"], 2);
    assert_eq!(
        log.body["items"][0]["details"],
        json!({ "key": "person-score", "version": 2, "message": "Calls the other decision" })
    );
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn history_lists_versions_newest_first_a_page_at_a_time(db: PgPool) {
    let app = with_database(db);
    let admin = signed_in_admin(&app).await;
    let (p, id) = setup(&app, &admin).await;
    let base = format!("/projects/{p}/decisions/{id}");
    // Each round versions the draft at revision n, then moves it to n + 1.
    for n in 1..=3 {
        save_version(&app, &admin, &base, n, &format!("Version {n}")).await;
        save_draft(&app, &admin, &base, calling(&format!("d{n}")), n).await;
    }

    let first = call(
        &app,
        "GET",
        &format!("{base}/versions?limit=2"),
        None,
        &admin,
    )
    .await;
    assert_eq!(first.body["total"], 3);
    let numbers: Vec<i64> = first.body["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v["number"].as_i64().unwrap())
        .collect();
    assert_eq!(numbers, [3, 2]);
    let second = call(
        &app,
        "GET",
        &format!("{base}/versions?limit=2&offset=2"),
        None,
        &admin,
    )
    .await;
    assert_eq!(second.body["items"][0]["number"], 1);
    assert_eq!(second.body["items"].as_array().unwrap().len(), 1);
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn versions_cannot_be_changed_or_deleted(db: PgPool) {
    let app = with_database(db.clone());
    let admin = signed_in_admin(&app).await;
    let (p, id) = setup(&app, &admin).await;
    save_version(
        &app,
        &admin,
        &format!("/projects/{p}/decisions/{id}"),
        1,
        "Kept",
    )
    .await;

    for statement in [
        "UPDATE decision_versions SET message = 'rewritten'",
        "DELETE FROM decision_versions",
        // CASCADE: test results reference versions, which alone refuses a plain TRUNCATE.
        "TRUNCATE decision_versions CASCADE",
    ] {
        let err = sqlx::query(statement).execute(&db).await.unwrap_err();
        let code = err.as_database_error().and_then(|e| e.code()).unwrap();
        assert_eq!(code, "42501", "{statement}");
    }
    let (message,): (String,) = sqlx::query_as("SELECT message FROM decision_versions")
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(message, "Kept");
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn restoring_adds_a_version_and_never_rewrites_history(db: PgPool) {
    let app = with_database(db);
    let admin = signed_in_admin(&app).await;
    let (p, id) = setup(&app, &admin).await;
    let base = format!("/projects/{p}/decisions/{id}");
    save_version(&app, &admin, &base, 1, "Table").await;
    save_draft(&app, &admin, &base, calling("other"), 1).await;
    save_version(&app, &admin, &base, 2, "Calls other").await;

    // Restore version 1 from the draft the editor shows (revision 2).
    let restored = call(
        &app,
        "POST",
        &format!("{base}/versions/1/restore"),
        Some(json!({ "revision": 2 })),
        &admin,
    )
    .await;
    assert_eq!(restored.status, StatusCode::CREATED, "{}", restored.body);
    assert_eq!(restored.body["version"]["number"], 3);
    assert_eq!(restored.body["version"]["restoredFrom"], 1);
    assert_eq!(restored.body["version"]["message"], "Restored version 1");
    assert_eq!(restored.body["decision"]["revision"], 3);
    assert_eq!(restored.body["decision"]["latestVersion"], 3);
    assert_eq!(restored.body["decision"]["changedSinceVersion"], false);
    assert_eq!(restored.body["decision"]["content"], fixture("table"));

    let draft = call(&app, "GET", &base, None, &admin).await;
    assert_eq!(draft.body["content"], fixture("table"));
    let history = call(&app, "GET", &format!("{base}/versions"), None, &admin).await;
    let numbers: Vec<i64> = history.body["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v["number"].as_i64().unwrap())
        .collect();
    assert_eq!(numbers, [3, 2, 1]);
    let two = call(&app, "GET", &format!("{base}/versions/2"), None, &admin).await;
    assert_eq!(two.body["content"], calling("other"));

    // A draft that moved on since the editor loaded it is not replaced unseen.
    let stale = call(
        &app,
        "POST",
        &format!("{base}/versions/2/restore"),
        Some(json!({ "revision": 2 })),
        &admin,
    )
    .await;
    assert_error(&stale, StatusCode::CONFLICT, "DECISION_CONFLICT");
    for missing in ["9", "abc"] {
        let reply = call(
            &app,
            "POST",
            &format!("{base}/versions/{missing}/restore"),
            Some(json!({ "revision": 3 })),
            &admin,
        )
        .await;
        assert_error(&reply, StatusCode::NOT_FOUND, "VERSION_NOT_FOUND");
    }
    let audit = call(
        &app,
        "GET",
        &format!("/projects/{p}/audit?action=decision.version_restored"),
        None,
        &admin,
    )
    .await;
    assert_eq!(
        audit.body["items"][0]["details"],
        json!({ "key": "person-score", "version": 3, "from": 1 })
    );
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn viewers_read_the_history_but_cannot_save_or_restore(db: PgPool) {
    let app = with_database(db);
    let admin = signed_in_admin(&app).await;
    let viewer = signed_in_user(&app, GRACE, false).await;
    let (p, id) = setup(&app, &admin).await;
    add(&app, &admin, &p, GRACE, "viewer").await;
    let base = format!("/projects/{p}/decisions/{id}");
    save_version(&app, &admin, &base, 1, "Table").await;

    let list = call(&app, "GET", &format!("{base}/versions"), None, &viewer).await;
    assert_eq!(list.body["total"], 1);
    let one = call(&app, "GET", &format!("{base}/versions/1"), None, &viewer).await;
    assert_eq!(one.status, StatusCode::OK);

    save_draft(&app, &admin, &base, calling("other"), 1).await;
    let save = save_version(&app, &viewer, &base, 2, "Mine").await;
    assert_error(&save, StatusCode::FORBIDDEN, "FORBIDDEN");
    let restore = call(
        &app,
        "POST",
        &format!("{base}/versions/1/restore"),
        Some(json!({ "revision": 2 })),
        &viewer,
    )
    .await;
    assert_error(&restore, StatusCode::FORBIDDEN, "FORBIDDEN");

    // Another project's decision has no history here.
    let alan = signed_in_user(&app, ALAN, false).await;
    let other = project(&app, &admin, "other").await;
    add(&app, &admin, &other, ALAN, "owner").await;
    let reply = call(
        &app,
        "GET",
        &format!("/projects/{other}/decisions/{id}/versions"),
        None,
        &alan,
    )
    .await;
    assert_error(&reply, StatusCode::NOT_FOUND, "DECISION_NOT_FOUND");
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn a_deleted_decision_keeps_its_history_and_frees_its_key(db: PgPool) {
    let app = with_database(db.clone());
    let admin = signed_in_admin(&app).await;
    let (p, id) = setup(&app, &admin).await;
    let base = format!("/projects/{p}/decisions/{id}");
    save_version(&app, &admin, &base, 1, "Table").await;

    let deleted = call(&app, "DELETE", &base, None, &admin).await;
    assert_eq!(deleted.status, StatusCode::NO_CONTENT);
    let gone = call(&app, "GET", &format!("{base}/versions"), None, &admin).await;
    assert_error(&gone, StatusCode::NOT_FOUND, "DECISION_NOT_FOUND");
    let (kept,): (i64,) = sqlx::query_as("SELECT count(*) FROM decision_versions")
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(kept, 1);

    // The key is free again, for a new decision with its own history.
    let again = decision(&app, &admin, &p, "person-score", None).await;
    assert_ne!(again["id"], id.as_str());
    assert!(again["latestVersion"].is_null());
    let list = call(
        &app,
        "GET",
        &format!("/projects/{p}/decisions"),
        None,
        &admin,
    )
    .await;
    assert_eq!(list.body["items"].as_array().unwrap().len(), 1);
}
