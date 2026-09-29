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

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn decisions_are_stored_per_project_under_their_key(db: PgPool) {
    let app = with_database(db);
    let admin = signed_in_admin(&app).await;
    let p = project(&app, &admin, "credit").await;

    let created = decision(&app, &admin, &p, "person-score", None).await;
    assert_eq!(created["key"], "person-score");
    assert_eq!(created["revision"], 1);
    assert_eq!(created["content"], json!({ "nodes": [], "edges": [] }));
    assert_eq!(created["updatedBy"]["email"], ADMIN_EMAIL);
    decision(&app, &admin, &p, "bureau/normalize", Some(fixture("table"))).await;

    let list = call(
        &app,
        "GET",
        &format!("/projects/{p}/decisions"),
        None,
        &admin,
    )
    .await;
    let keys: Vec<&str> = list.body["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["key"].as_str().unwrap())
        .collect();
    assert_eq!(keys, ["bureau/normalize", "person-score"]);

    let id = created["id"].as_str().unwrap();
    let one = call(
        &app,
        "GET",
        &format!("/projects/{p}/decisions/{id}"),
        None,
        &admin,
    )
    .await;
    assert_eq!(one.status, StatusCode::OK);
    assert_eq!(one.body["key"], "person-score");

    // The same key twice in one project is refused; another project may use it.
    let again = call(
        &app,
        "POST",
        &format!("/projects/{p}/decisions"),
        Some(json!({ "key": "person-score" })),
        &admin,
    )
    .await;
    assert_error(&again, StatusCode::CONFLICT, "DECISION_KEY_TAKEN");
    let other = project(&app, &admin, "cards").await;
    decision(&app, &admin, &other, "person-score", None).await;

    for bad in ["Person", "a//b", "bureau/", "-x", "a b"] {
        let reply = call(
            &app,
            "POST",
            &format!("/projects/{p}/decisions"),
            Some(json!({ "key": bad })),
            &admin,
        )
        .await;
        assert_error(&reply, StatusCode::BAD_REQUEST, "INVALID_REQUEST");
        assert!(reply.body["fields"]["key"].is_string(), "{bad}");
    }
    let broken = call(
        &app,
        "POST",
        &format!("/projects/{p}/decisions"),
        Some(json!({ "key": "broken", "content": { "nodes": 1 } })),
        &admin,
    )
    .await;
    assert_error(
        &broken,
        StatusCode::UNPROCESSABLE_ENTITY,
        "INVALID_DECISION",
    );
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn two_people_saving_the_same_draft_get_a_conflict_not_an_overwrite(db: PgPool) {
    let app = with_database(db);
    let admin = signed_in_admin(&app).await;
    let grace = signed_in_user(&app, GRACE, false).await;
    let p = project(&app, &admin, "credit").await;
    add(&app, &admin, &p, GRACE, "editor").await;
    let d = decision(&app, &admin, &p, "person-score", None).await;
    let path = format!("/projects/{p}/decisions/{}", d["id"].as_str().unwrap());

    // Both opened revision 1. Grace saves first.
    let saved = call(
        &app,
        "PUT",
        &path,
        Some(json!({ "content": fixture("table"), "revision": 1 })),
        &grace,
    )
    .await;
    assert_eq!(saved.status, StatusCode::OK, "{}", saved.body);
    assert_eq!(saved.body["revision"], 2);
    assert_eq!(saved.body["updatedBy"]["email"], GRACE);

    // The administrator's save from revision 1 would erase Grace's work: refused.
    let stale = call(
        &app,
        "PUT",
        &path,
        Some(json!({ "content": { "nodes": [], "edges": [] }, "revision": 1 })),
        &admin,
    )
    .await;
    assert_error(&stale, StatusCode::CONFLICT, "DECISION_CONFLICT");
    assert_eq!(stale.body["details"]["revision"], 2);
    assert_eq!(stale.body["details"]["updatedBy"]["email"], GRACE);

    let now = call(&app, "GET", &path, None, &admin).await;
    assert_eq!(now.body["content"], fixture("table"));
    assert_eq!(now.body["revision"], 2);

    // Saving from the latest revision works, and a broken graph is refused on save.
    let next = call(
        &app,
        "PUT",
        &path,
        Some(json!({ "content": { "nodes": [], "edges": [] }, "revision": 2 })),
        &admin,
    )
    .await;
    assert_eq!(next.body["revision"], 3);
    let broken = call(
        &app,
        "PUT",
        &path,
        Some(json!({ "content": { "edges": 7 }, "revision": 3 })),
        &admin,
    )
    .await;
    assert_error(
        &broken,
        StatusCode::UNPROCESSABLE_ENTITY,
        "INVALID_DECISION",
    );
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn simulate_evaluates_the_draft_with_every_sibling_decision(db: PgPool) {
    let app = with_database(db);
    let admin = signed_in_admin(&app).await;
    let p = project(&app, &admin, "credit").await;
    decision(&app, &admin, &p, "rules/table", Some(fixture("table"))).await;
    let parent = decision(
        &app,
        &admin,
        &p,
        "person-score",
        Some(calling("rules/table")),
    )
    .await;
    let path = format!(
        "/projects/{p}/decisions/{}/simulate",
        parent["id"].as_str().unwrap()
    );

    // The parent resolves its sub-decision from the project.
    let run = call(
        &app,
        "POST",
        &path,
        Some(json!({ "context": { "input": 12 } })),
        &admin,
    )
    .await;
    assert_eq!(run.status, StatusCode::OK, "{}", run.body);
    assert_eq!(run.body["result"], json!({ "output": 10 }));
    assert!(run.body["trace"].is_object());

    // Unsaved content from the editor is what runs; the saved draft is untouched.
    let unsaved = call(
        &app,
        "POST",
        &path,
        Some(json!({ "context": { "input": 12 }, "content": calling("missing") })),
        &admin,
    )
    .await;
    assert_eq!(
        unsaved.status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "{}",
        unsaved.body
    );
    let saved = call(
        &app,
        "GET",
        &format!("/projects/{p}/decisions/{}", parent["id"].as_str().unwrap()),
        None,
        &admin,
    )
    .await;
    assert_eq!(saved.body["content"], calling("rules/table"));
    assert_eq!(saved.body["revision"], 1);
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn viewers_open_and_simulate_but_cannot_change(db: PgPool) {
    let app = with_database(db);
    let admin = signed_in_admin(&app).await;
    let viewer = signed_in_user(&app, GRACE, false).await;
    let p = project(&app, &admin, "credit").await;
    add(&app, &admin, &p, GRACE, "viewer").await;
    let d = decision(&app, &admin, &p, "rules/table", Some(fixture("table"))).await;
    let path = format!("/projects/{p}/decisions/{}", d["id"].as_str().unwrap());

    let list = call(
        &app,
        "GET",
        &format!("/projects/{p}/decisions"),
        None,
        &viewer,
    )
    .await;
    assert_eq!(list.status, StatusCode::OK);
    let open = call(&app, "GET", &path, None, &viewer).await;
    assert_eq!(open.status, StatusCode::OK);
    let run = call(
        &app,
        "POST",
        &format!("{path}/simulate"),
        Some(json!({ "context": { "input": 12 } })),
        &viewer,
    )
    .await;
    assert_eq!(run.status, StatusCode::OK, "{}", run.body);

    let attempts = [
        (
            "PUT",
            path.clone(),
            Some(json!({ "content": fixture("table"), "revision": 1 })),
        ),
        ("DELETE", path.clone(), None),
        (
            "POST",
            format!("/projects/{p}/decisions"),
            Some(json!({ "key": "mine" })),
        ),
    ];
    for (method, target, body) in attempts {
        let reply = call(&app, method, &target, body, &viewer).await;
        assert_error(&reply, StatusCode::FORBIDDEN, "FORBIDDEN");
    }
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn decisions_stay_inside_their_project(db: PgPool) {
    let app = with_database(db);
    let admin = signed_in_admin(&app).await;
    let alan = signed_in_user(&app, ALAN, false).await;
    let a = project(&app, &admin, "project-a").await;
    let b = project(&app, &admin, "project-b").await;
    add(&app, &admin, &a, ALAN, "owner").await;
    let in_b = decision(&app, &admin, &b, "secret", None).await;
    let id = in_b["id"].as_str().unwrap();

    // Not a member of B: the project does not exist for Alan.
    let reply = call(
        &app,
        "GET",
        &format!("/projects/{b}/decisions/{id}"),
        None,
        &alan,
    )
    .await;
    assert_error(&reply, StatusCode::NOT_FOUND, "PROJECT_NOT_FOUND");
    // B's decision through A, where Alan is an owner: not found either.
    for (method, body) in [
        ("GET", None),
        (
            "PUT",
            Some(json!({ "content": { "nodes": [], "edges": [] }, "revision": 1 })),
        ),
        ("DELETE", None),
    ] {
        let reply = call(
            &app,
            method,
            &format!("/projects/{a}/decisions/{id}"),
            body,
            &alan,
        )
        .await;
        assert_error(&reply, StatusCode::NOT_FOUND, "DECISION_NOT_FOUND");
    }
    let run = call(
        &app,
        "POST",
        &format!("/projects/{a}/decisions/{id}/simulate"),
        Some(json!({})),
        &alan,
    )
    .await;
    assert_error(&run, StatusCode::NOT_FOUND, "DECISION_NOT_FOUND");
    let malformed = call(
        &app,
        "GET",
        &format!("/projects/{a}/decisions/nope"),
        None,
        &alan,
    )
    .await;
    assert_error(&malformed, StatusCode::NOT_FOUND, "DECISION_NOT_FOUND");
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn an_archived_project_keeps_its_decisions_read_only(db: PgPool) {
    let app = with_database(db);
    let admin = signed_in_admin(&app).await;
    let p = project(&app, &admin, "credit").await;
    let d = decision(&app, &admin, &p, "rules/table", Some(fixture("table"))).await;
    let path = format!("/projects/{p}/decisions/{}", d["id"].as_str().unwrap());
    call(
        &app,
        "POST",
        &format!("/projects/{p}/archive"),
        None,
        &admin,
    )
    .await;

    let save = call(
        &app,
        "PUT",
        &path,
        Some(json!({ "content": fixture("table"), "revision": 1 })),
        &admin,
    )
    .await;
    assert_error(&save, StatusCode::CONFLICT, "PROJECT_ARCHIVED");
    let create = call(
        &app,
        "POST",
        &format!("/projects/{p}/decisions"),
        Some(json!({ "key": "new" })),
        &admin,
    )
    .await;
    assert_error(&create, StatusCode::CONFLICT, "PROJECT_ARCHIVED");
    // Reading and simulating still work.
    assert_eq!(
        call(&app, "GET", &path, None, &admin).await.status,
        StatusCode::OK
    );
    let run = call(
        &app,
        "POST",
        &format!("{path}/simulate"),
        Some(json!({ "context": { "input": 12 } })),
        &admin,
    )
    .await;
    assert_eq!(run.status, StatusCode::OK);
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn creating_and_deleting_are_audited_but_draft_saves_are_not(db: PgPool) {
    let app = with_database(db);
    let admin = signed_in_admin(&app).await;
    let p = project(&app, &admin, "credit").await;
    let d = decision(&app, &admin, &p, "person-score", None).await;
    let path = format!("/projects/{p}/decisions/{}", d["id"].as_str().unwrap());
    call(
        &app,
        "PUT",
        &path,
        Some(json!({ "content": fixture("table"), "revision": 1 })),
        &admin,
    )
    .await;
    let deleted = call(&app, "DELETE", &path, None, &admin).await;
    assert_eq!(deleted.status, StatusCode::NO_CONTENT);
    let gone = call(&app, "GET", &path, None, &admin).await;
    assert_error(&gone, StatusCode::NOT_FOUND, "DECISION_NOT_FOUND");

    let log = call(
        &app,
        "GET",
        &format!("/projects/{p}/audit?action=decision.created"),
        None,
        &admin,
    )
    .await;
    assert_eq!(log.body["total"], 1);
    assert_eq!(
        log.body["items"][0]["details"],
        json!({ "key": "person-score" })
    );
    let all = call(&app, "GET", &format!("/projects/{p}/audit"), None, &admin).await;
    let actions: Vec<&str> = all.body["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["action"].as_str().unwrap())
        .collect();
    assert_eq!(
        actions,
        ["decision.deleted", "decision.created", "project.created"]
    );
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn two_saves_at_the_same_moment_give_one_save_and_one_conflict(db: PgPool) {
    let app = with_database(db);
    let admin = signed_in_admin(&app).await;
    let grace = signed_in_user(&app, GRACE, false).await;
    let p = project(&app, &admin, "credit").await;
    add(&app, &admin, &p, GRACE, "editor").await;
    let d = decision(&app, &admin, &p, "person-score", None).await;
    let path = format!(
        "{BASE}/projects/{p}/decisions/{}",
        d["id"].as_str().unwrap()
    );

    let mut both = tokio::task::JoinSet::new();
    for session in [admin, grace] {
        let router = app.router.clone();
        let path = path.clone();
        both.spawn(async move {
            let body = json!({ "content": fixture("table"), "revision": 1 });
            send(&router, request("PUT", &path, Some(&body), Some(&session)))
                .await
                .status
        });
    }
    let mut statuses = both.join_all().await;
    statuses.sort();
    assert_eq!(statuses, [StatusCode::OK, StatusCode::CONFLICT]);
}
