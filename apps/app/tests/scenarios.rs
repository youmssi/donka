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

async fn scenario(
    app: &TestApp,
    session: &str,
    project: &str,
    decision: &str,
    (name, input, expected, mode): (&str, Value, Value, &str),
) -> Reply {
    call(
        app,
        "POST",
        &format!("/projects/{project}/test-scenarios"),
        Some(json!({
            "decisionId": decision, "name": name, "input": input,
            "expected": expected, "match": mode
        })),
        session,
    )
    .await
}

async fn audited(app: &TestApp, owner: &str, project: &str, action: &str) -> Vec<Value> {
    let log = call(
        app,
        "GET",
        &format!("/projects/{project}/audit?action={action}"),
        None,
        owner,
    )
    .await;
    log.body["items"].as_array().unwrap().clone()
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn editors_write_scenarios_with_a_unique_name_per_decision(db: PgPool) {
    let app = with_database(db);
    let admin = signed_in_admin(&app).await;
    let (p, id) = setup(&app, &admin).await;

    let created = scenario(
        &app,
        &admin,
        &p,
        &id,
        (
            " Big income ",
            json!({ "input": 12 }),
            json!({ "output": 10 }),
            "partial",
        ),
    )
    .await;
    assert_eq!(created.status, StatusCode::CREATED, "{}", created.body);
    assert_eq!(created.body["name"], "Big income");
    assert_eq!(created.body["decisionKey"], "person-score");
    assert_eq!(created.body["match"], "partial");
    assert_eq!(created.body["updatedBy"]["email"], ADMIN_EMAIL);
    let sid = created.body["id"].as_str().unwrap().to_owned();

    // Names are unique per decision, whatever the case.
    let twice = scenario(
        &app,
        &admin,
        &p,
        &id,
        ("BIG INCOME", json!({}), json!({}), "exact"),
    )
    .await;
    assert_error(&twice, StatusCode::CONFLICT, "SCENARIO_NAME_TAKEN");
    for (name, input, expected, field) in [
        ("  ", json!({}), json!({}), "name"),
        (&"x".repeat(201)[..], json!({}), json!({}), "name"),
        ("Input", json!([1]), json!({}), "input"),
        ("Expected", json!({}), json!(10), "expected"),
    ] {
        let reply = scenario(&app, &admin, &p, &id, (name, input, expected, "exact")).await;
        assert_error(&reply, StatusCode::BAD_REQUEST, "INVALID_REQUEST");
        assert!(
            reply.body["fields"][field].is_string(),
            "{field}: {}",
            reply.body
        );
    }
    let unknown = scenario(
        &app,
        &admin,
        &p,
        &uuid::Uuid::new_v4().to_string(),
        ("x", json!({}), json!({}), "exact"),
    )
    .await;
    assert_error(&unknown, StatusCode::NOT_FOUND, "DECISION_NOT_FOUND");

    let other = decision(&app, &admin, &p, "other", None).await;
    let other_id = other["id"].as_str().unwrap();
    scenario(
        &app,
        &admin,
        &p,
        other_id,
        ("Big income", json!({}), json!({}), "exact"),
    )
    .await;
    let all = call(
        &app,
        "GET",
        &format!("/projects/{p}/test-scenarios"),
        None,
        &admin,
    )
    .await;
    assert_eq!(all.body["total"], 2);
    let mine = call(
        &app,
        "GET",
        &format!("/projects/{p}/test-scenarios?decisionId={id}"),
        None,
        &admin,
    )
    .await;
    assert_eq!(mine.body["total"], 1);

    let path = format!("/projects/{p}/test-scenarios/{sid}");
    let updated = call(
        &app,
        "PUT",
        &path,
        Some(json!({
            "name": "Income above 10", "input": { "input": 11 },
            "expected": { "output": 10 }, "match": "exact"
        })),
        &admin,
    )
    .await;
    assert_eq!(updated.status, StatusCode::OK, "{}", updated.body);
    assert_eq!(updated.body["match"], "exact");
    assert_eq!(
        call(&app, "GET", &path, None, &admin).await.body["input"],
        json!({ "input": 11 })
    );

    let deleted = call(&app, "DELETE", &path, None, &admin).await;
    assert_eq!(deleted.status, StatusCode::NO_CONTENT);
    assert_error(
        &call(&app, "GET", &path, None, &admin).await,
        StatusCode::NOT_FOUND,
        "SCENARIO_NOT_FOUND",
    );
    assert_error(
        &call(
            &app,
            "GET",
            &format!("/projects/{p}/test-scenarios/nope"),
            None,
            &admin,
        )
        .await,
        StatusCode::NOT_FOUND,
        "SCENARIO_NOT_FOUND",
    );

    for action in ["scenario.created", "scenario.updated", "scenario.deleted"] {
        let events = audited(&app, &admin, &p, action).await;
        assert!(
            events.iter().any(|e| e["details"]["key"] == "person-score"),
            "{action}: {events:?}"
        );
    }
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn viewers_read_scenarios_and_other_projects_see_nothing(db: PgPool) {
    let app = with_database(db);
    let admin = signed_in_admin(&app).await;
    let viewer = signed_in_user(&app, GRACE, false).await;
    let stranger = signed_in_user(&app, ALAN, false).await;
    let (p, id) = setup(&app, &admin).await;
    add(&app, &admin, &p, GRACE, "viewer").await;
    let created = scenario(
        &app,
        &admin,
        &p,
        &id,
        (
            "Big",
            json!({ "input": 12 }),
            json!({ "output": 10 }),
            "partial",
        ),
    )
    .await;
    let path = format!(
        "/projects/{p}/test-scenarios/{}",
        created.body["id"].as_str().unwrap()
    );

    assert_eq!(
        call(&app, "GET", &path, None, &viewer).await.status,
        StatusCode::OK
    );
    let write = scenario(
        &app,
        &viewer,
        &p,
        &id,
        ("Mine", json!({}), json!({}), "exact"),
    )
    .await;
    assert_error(&write, StatusCode::FORBIDDEN, "FORBIDDEN");
    assert_error(
        &call(&app, "DELETE", &path, None, &viewer).await,
        StatusCode::FORBIDDEN,
        "FORBIDDEN",
    );
    assert_error(
        &call(&app, "GET", &path, None, &stranger).await,
        StatusCode::NOT_FOUND,
        "PROJECT_NOT_FOUND",
    );
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn every_version_runs_the_project_scenarios_and_keeps_the_results(db: PgPool) {
    let app = with_database(db.clone());
    let admin = signed_in_admin(&app).await;
    let p = project(&app, &admin, "credit").await;
    let normalize = decision(&app, &admin, &p, "bureau/normalize", Some(fixture("table"))).await;
    let n = normalize["id"].as_str().unwrap().to_owned();
    let score = decision(
        &app,
        &admin,
        &p,
        "person-score",
        Some(calling("bureau/normalize")),
    )
    .await;
    let s = score["id"].as_str().unwrap().to_owned();

    scenario(
        &app,
        &admin,
        &p,
        &n,
        (
            "Above ten",
            json!({ "input": 12 }),
            json!({ "output": 10 }),
            "exact",
        ),
    )
    .await;
    let wrong = scenario(
        &app,
        &admin,
        &p,
        &n,
        (
            "Small",
            json!({ "input": 3 }),
            json!({ "output": 5 }),
            "partial",
        ),
    )
    .await;
    scenario(
        &app,
        &admin,
        &p,
        &s,
        (
            "Score calls normalize",
            json!({ "input": 12 }),
            json!({ "output": 10 }),
            "partial",
        ),
    )
    .await;

    // normalize v1: its own scenarios run; person-score has no version yet.
    let base = format!("/projects/{p}/decisions/{n}");
    let v1 = save_version(&app, &admin, &base, 1, "Table").await;
    assert_eq!(v1.status, StatusCode::CREATED, "{}", v1.body);
    assert_eq!(
        v1.body["tests"],
        json!({ "passed": 1, "failed": 1, "errors": 1 })
    );

    let results = call(
        &app,
        "GET",
        &format!("{base}/versions/1/test-results"),
        None,
        &admin,
    )
    .await;
    assert_eq!(results.status, StatusCode::OK, "{}", results.body);
    assert_eq!(
        results.body["summary"],
        json!({ "passed": 1, "failed": 1, "errors": 1 })
    );
    let items = results.body["items"].as_array().unwrap();
    let by_name = |name: &str| items.iter().find(|r| r["name"] == name).unwrap().clone();
    assert_eq!(by_name("Above ten")["status"], "passed");
    let small = by_name("Small");
    assert_eq!(small["status"], "failed");
    assert_eq!(small["actual"], json!({ "output": 0 }));
    assert_eq!(
        small["mismatches"],
        json!([{ "path": "output", "expected": 5, "actual": 0 }])
    );
    let calls = by_name("Score calls normalize");
    assert_eq!(calls["status"], "error");
    assert_eq!(calls["missingDecision"], "person-score");

    // person-score v1 runs against normalize v1: now everything can run.
    let v1 = save_version(
        &app,
        &admin,
        &format!("/projects/{p}/decisions/{s}"),
        1,
        "Calls",
    )
    .await;
    assert_eq!(
        v1.body["tests"],
        json!({ "passed": 2, "failed": 1, "errors": 0 })
    );

    // Fixing the scenario does not rewrite what was tested.
    let path = format!(
        "/projects/{p}/test-scenarios/{}",
        wrong.body["id"].as_str().unwrap()
    );
    call(&app, "PUT", &path, Some(json!({
        "name": "Small", "input": { "input": 3 }, "expected": { "output": 0 }, "match": "partial"
    })), &admin).await;
    let kept = call(
        &app,
        "GET",
        &format!("{base}/versions/1/test-results"),
        None,
        &admin,
    )
    .await;
    assert_eq!(kept.body["summary"]["failed"], 1);
    let listed = call(&app, "GET", &format!("{base}/versions"), None, &admin).await;
    assert_eq!(listed.body["items"][0]["tests"]["failed"], 1);

    // A restore is a new version: it is tested too, with the scenarios as they are now.
    save_draft(&app, &admin, &base, calling("person-score"), 1).await;
    save_version(&app, &admin, &base, 2, "Loop").await;
    let restored = call(
        &app,
        "POST",
        &format!("{base}/versions/1/restore"),
        Some(json!({ "revision": 2 })),
        &admin,
    )
    .await;
    assert_eq!(restored.status, StatusCode::CREATED, "{}", restored.body);
    assert_eq!(
        restored.body["version"]["tests"],
        json!({ "passed": 3, "failed": 0, "errors": 0 })
    );

    let missing = call(
        &app,
        "GET",
        &format!("{base}/versions/9/test-results"),
        None,
        &admin,
    )
    .await;
    assert_error(&missing, StatusCode::NOT_FOUND, "VERSION_NOT_FOUND");
    for statement in [
        "UPDATE test_results SET status = 'passed'",
        "DELETE FROM test_results",
        "TRUNCATE test_results",
    ] {
        let err = sqlx::query(statement).execute(&db).await.unwrap_err();
        let code = err.as_database_error().and_then(|e| e.code()).unwrap();
        assert_eq!(code, "42501", "{statement}");
    }
}
