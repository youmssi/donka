#![allow(clippy::unwrap_used)] // tests fail loudly on purpose

//! The starter packs (DNK-24): each one imports through the API the way
//! `scripts/import-pack.sh` does, and every one of its test scenarios passes.

mod support;

use axum::http::StatusCode;
use donka_db::PgPool;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
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

fn read(path: &Path) -> Value {
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn packs() -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packs");
    let mut packs: Vec<PathBuf> = std::fs::read_dir(root)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.join("pack.json").is_file())
        .collect();
    packs.sort();
    packs
}

/// Imports `pack` as project `key`; returns the test summary of the last version saved.
async fn import(app: &TestApp, session: &str, pack: &Path, key: &str) -> Value {
    let manifest = read(&pack.join("pack.json"));
    assert_eq!(manifest["format"], 1, "{}", pack.display());
    let project = call(
        app,
        "POST",
        "/projects",
        Some(json!({ "key": key, "name": manifest["name"]["en"], "description": manifest["description"]["en"] })),
        session,
    )
    .await;
    assert_eq!(project.status, StatusCode::CREATED, "{}", project.body);
    let base = format!("/projects/{}", project.body["id"].as_str().unwrap());

    let mut created = Vec::new();
    for decision in manifest["decisions"].as_array().unwrap() {
        let key = decision.as_str().unwrap();
        let content = read(&pack.join("decisions").join(format!("{key}.json")));
        let reply = call(
            app,
            "POST",
            &format!("{base}/decisions"),
            Some(json!({ "key": key, "content": content })),
            session,
        )
        .await;
        assert_eq!(reply.status, StatusCode::CREATED, "{key}: {}", reply.body);
        created.push((key.to_owned(), reply.body));
    }
    let settings = call(
        app,
        "PUT",
        &format!("{base}/decision-log/settings"),
        Some(manifest["decisionLog"].clone()),
        session,
    )
    .await;
    assert_eq!(settings.status, StatusCode::OK, "{}", settings.body);

    let scenarios = read(&pack.join("scenarios.json"));
    assert!(!scenarios.as_array().unwrap().is_empty());
    for scenario in scenarios.as_array().unwrap() {
        let (_, decision) = created
            .iter()
            .find(|(key, _)| scenario["decision"] == key.as_str())
            .unwrap_or_else(|| panic!("no decision {} in the pack", scenario["decision"]));
        let reply = call(
            app,
            "POST",
            &format!("{base}/test-scenarios"),
            Some(json!({
                "decisionId": decision["id"],
                "name": scenario["name"],
                "input": scenario["input"],
                "expected": scenario["expected"],
                "match": scenario["match"],
            })),
            session,
        )
        .await;
        assert_eq!(reply.status, StatusCode::CREATED, "{}", reply.body);
    }

    let mut tests = Value::Null;
    for (key, decision) in &created {
        let reply = call(
            app,
            "POST",
            &format!(
                "{base}/decisions/{}/versions",
                decision["id"].as_str().unwrap()
            ),
            Some(json!({ "message": "Imported", "revision": decision["revision"] })),
            session,
        )
        .await;
        assert_eq!(reply.status, StatusCode::CREATED, "{key}: {}", reply.body);
        tests = reply.body["tests"].clone();
    }
    tests
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn every_pack_imports_and_its_scenarios_pass(db: PgPool) {
    let app = with_database(db);
    let session = signed_in_admin(&app).await;
    let packs = packs();
    assert!(packs.len() >= 2, "{packs:?}");
    for pack in &packs {
        let manifest = read(&pack.join("pack.json"));
        let key = manifest["key"].as_str().unwrap();
        let scenarios = read(&pack.join("scenarios.json")).as_array().unwrap().len();
        let tests = import(&app, &session, pack, key).await;
        assert_eq!(
            tests,
            json!({ "passed": scenarios, "failed": 0, "errors": 0 }),
            "{key}"
        );
    }
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn a_pack_imports_twice_as_independent_projects(db: PgPool) {
    let app = with_database(db);
    let session = signed_in_admin(&app).await;
    let pack = packs()
        .into_iter()
        .find(|p| p.ends_with("retail-credit"))
        .unwrap();
    for key in ["retail-credit", "salary-advance"] {
        let tests = import(&app, &session, &pack, key).await;
        assert_eq!(tests["failed"], 0, "{key}");
        assert_eq!(tests["errors"], 0, "{key}");
    }
    let list = call(&app, "GET", "/projects", None, &session).await;
    let keys: Vec<&str> = list.body["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["key"].as_str().unwrap())
        .collect();
    assert!(
        keys.contains(&"retail-credit") && keys.contains(&"salary-advance"),
        "{keys:?}"
    );
}
