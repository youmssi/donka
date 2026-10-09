#![allow(clippy::unwrap_used)] // tests fail loudly on purpose

//! Packs (DNK-24, DNK-43): every pack of the repository imports from the catalogue and its
//! scenarios pass; a project duplicates, exports as a pack file, and that file imports again.

mod support;

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use donka_app::auth::{CSRF_HEADER, SESSION_COOKIE};
use donka_db::PgPool;
use http_body_util::BodyExt;
use serde_json::{json, Value};
use support::*;
use tower::ServiceExt;

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

/// Sends a pack file (a zip) as the web app does.
async fn upload(app: &TestApp, path: &str, file: Vec<u8>, session: &str) -> Reply {
    let req = Request::post(format!("{BASE}{path}"))
        .header(header::CONTENT_TYPE, "application/zip")
        .header(CSRF_HEADER, "1")
        .header(header::COOKIE, format!("{SESSION_COOKIE}={session}"))
        .body(Body::from(file))
        .unwrap();
    send(&app.router, req).await
}

struct Download {
    status: StatusCode,
    content_type: String,
    disposition: String,
    bytes: Vec<u8>,
}

async fn download(app: &TestApp, path: &str, session: &str) -> Download {
    let res = app
        .router
        .clone()
        .oneshot(get(&format!("{BASE}{path}"), Some(session)))
        .await
        .unwrap();
    let header_text = |name: header::HeaderName| {
        res.headers()
            .get(name)
            .map(|v| v.to_str().unwrap().to_owned())
            .unwrap_or_default()
    };
    let (content_type, disposition) = (
        header_text(header::CONTENT_TYPE),
        header_text(header::CONTENT_DISPOSITION),
    );
    let status = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes().to_vec();
    Download {
        status,
        content_type,
        disposition,
        bytes,
    }
}

async fn import(app: &TestApp, session: &str, pack: &str, key: &str, name: &str) -> Reply {
    call(
        app,
        "POST",
        &format!("/packs/{pack}/import"),
        Some(json!({ "key": key, "name": name })),
        session,
    )
    .await
}

/// Imports a pack of the catalogue; returns the new project's id.
async fn imported(app: &TestApp, session: &str, pack: &str, key: &str) -> String {
    let reply = import(app, session, pack, key, key).await;
    assert_eq!(reply.status, StatusCode::CREATED, "{}", reply.body);
    reply.body["project"]["id"].as_str().unwrap().to_owned()
}

async fn audit(app: &TestApp, session: &str, project: &str) -> Vec<Value> {
    let reply = call(
        app,
        "GET",
        &format!("/projects/{project}/audit"),
        None,
        session,
    )
    .await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.body);
    reply.body["items"].as_array().unwrap().clone()
}

fn event<'a>(events: &'a [Value], action: &str) -> &'a Value {
    events
        .iter()
        .find(|event| event["action"] == action)
        .unwrap_or_else(|| panic!("no {action} in {events:?}"))
}

async fn decision_keys(app: &TestApp, session: &str, project: &str) -> Vec<String> {
    let reply = call(
        app,
        "GET",
        &format!("/projects/{project}/decisions"),
        None,
        session,
    )
    .await;
    let mut keys: Vec<String> = reply.body["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["key"].as_str().unwrap().to_owned())
        .collect();
    keys.sort();
    keys
}

async fn project_keys(app: &TestApp, session: &str) -> Vec<String> {
    let reply = call(app, "GET", "/projects", None, session).await;
    let mut keys: Vec<String> = reply.body["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["key"].as_str().unwrap().to_owned())
        .collect();
    keys.sort();
    keys
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn every_pack_of_the_catalogue_imports_and_its_scenarios_pass(db: PgPool) {
    let app = with_database(db);
    let session = signed_in_admin(&app).await;
    let listed = call(&app, "GET", "/packs", None, &session).await;
    assert_eq!(listed.status, StatusCode::OK);
    let packs = listed.body["items"].as_array().unwrap().clone();
    assert!(packs.len() >= 2, "{packs:?}");

    for pack in &packs {
        let key = pack["key"].as_str().unwrap();
        let reply = import(
            &app,
            &session,
            key,
            key,
            pack["name"]["en"].as_str().unwrap(),
        )
        .await;
        assert_eq!(reply.status, StatusCode::CREATED, "{key}: {}", reply.body);
        assert_eq!(
            reply.body["tests"],
            json!({ "passed": pack["scenarios"], "failed": 0, "errors": 0 }),
            "{key}"
        );
        assert_eq!(reply.body["project"]["role"], "owner");
        let project = reply.body["project"]["id"].as_str().unwrap();

        let mut expected: Vec<String> = pack["decisions"]
            .as_array()
            .unwrap()
            .iter()
            .map(|d| d.as_str().unwrap().to_owned())
            .collect();
        expected.sort();
        assert_eq!(decision_keys(&app, &session, project).await, expected);

        // The audit log opens with where the project came from.
        let events = audit(&app, &session, project).await;
        let created = event(&events, "project.created");
        assert_eq!(created["details"]["from"]["kind"], "pack", "{key}");
        assert_eq!(created["details"]["from"]["key"], key);
        assert_eq!(created["details"]["from"]["version"], pack["version"]);
    }
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn a_pack_imports_twice_as_independent_projects(db: PgPool) {
    let app = with_database(db);
    let session = signed_in_admin(&app).await;
    imported(&app, &session, "retail-credit", "retail-credit").await;
    let second = import(
        &app,
        &session,
        "retail-credit",
        "salary-advance",
        "Salary advance",
    )
    .await;
    assert_eq!(second.status, StatusCode::CREATED, "{}", second.body);
    assert_eq!(second.body["project"]["name"], "Salary advance");

    // A key in use is refused before anything is made.
    assert_error(
        &import(&app, &session, "sme-treasury", "retail-credit", "Again").await,
        StatusCode::CONFLICT,
        "PROJECT_KEY_TAKEN",
    );
    assert_eq!(
        project_keys(&app, &session).await,
        ["retail-credit", "salary-advance"]
    );
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn only_administrators_make_projects_from_packs(db: PgPool) {
    let app = with_database(db);
    let admin = signed_in_admin(&app).await;
    let grace = signed_in_user(&app, GRACE, false).await;
    let project = imported(&app, &admin, "retail-credit", "retail-credit").await;
    let added = call(
        &app,
        "POST",
        &format!("/projects/{project}/members"),
        Some(json!({ "email": GRACE, "role": "owner" })),
        &admin,
    )
    .await;
    assert_eq!(added.status, StatusCode::CREATED, "{}", added.body);

    // Grace reads the catalogue and owns the project, but cannot create projects.
    assert_eq!(
        call(&app, "GET", "/packs", None, &grace).await.status,
        StatusCode::OK
    );
    assert_error(
        &import(&app, &grace, "retail-credit", "mine", "Mine").await,
        StatusCode::FORBIDDEN,
        "FORBIDDEN",
    );
    let file = download(&app, &format!("/projects/{project}/export"), &grace).await;
    assert_eq!(file.status, StatusCode::OK);
    assert_error(
        &upload(&app, "/packs/import?key=mine&name=Mine", file.bytes, &grace).await,
        StatusCode::FORBIDDEN,
        "FORBIDDEN",
    );
    assert_error(
        &call(
            &app,
            "POST",
            &format!("/projects/{project}/duplicate"),
            Some(json!({ "key": "mine", "name": "Mine" })),
            &grace,
        )
        .await,
        StatusCode::FORBIDDEN,
        "FORBIDDEN",
    );

    assert_error(
        &import(&app, &admin, "no-such-pack", "x-pack", "X").await,
        StatusCode::NOT_FOUND,
        "PACK_NOT_FOUND",
    );
    assert_error(
        &import(&app, &admin, "retail-credit", "Not A Key", "X").await,
        StatusCode::BAD_REQUEST,
        "INVALID_REQUEST",
    );
    assert_eq!(project_keys(&app, &admin).await, ["retail-credit"]);
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn an_exported_project_imports_in_another_installation(db: PgPool) {
    let app = with_database(db);
    let admin = signed_in_admin(&app).await;
    let project = imported(&app, &admin, "sme-treasury", "treasury").await;
    let settings = call(
        &app,
        "GET",
        &format!("/projects/{project}/decision-log/settings"),
        None,
        &admin,
    )
    .await
    .body;

    let file = download(&app, &format!("/projects/{project}/export"), &admin).await;
    assert_eq!(file.status, StatusCode::OK);
    assert_eq!(file.content_type, "application/zip");
    assert_eq!(
        file.disposition,
        "attachment; filename=\"treasury.donka-pack.zip\""
    );
    let exported = event(&audit(&app, &admin, &project).await, "project.exported").clone();
    assert_eq!(exported["details"]["release"], Value::Null);

    // The other installation reads the file first, then imports it under its own key.
    let read = upload(&app, "/packs/inspect", file.bytes.clone(), &admin).await;
    assert_eq!(read.status, StatusCode::OK, "{}", read.body);
    assert_eq!(read.body["key"], "treasury");
    assert_eq!(exported["details"]["scenarios"], read.body["scenarios"]);
    let copy = upload(
        &app,
        "/packs/import?key=treasury-copy&name=Treasury%20copy",
        file.bytes,
        &admin,
    )
    .await;
    assert_eq!(copy.status, StatusCode::CREATED, "{}", copy.body);
    assert_eq!(
        copy.body["tests"],
        json!({ "passed": read.body["scenarios"], "failed": 0, "errors": 0 })
    );
    assert_eq!(copy.body["project"]["name"], "Treasury copy");
    let copy_id = copy.body["project"]["id"].as_str().unwrap();
    assert_eq!(
        decision_keys(&app, &admin, copy_id).await,
        decision_keys(&app, &admin, &project).await
    );
    let copy_settings = call(
        &app,
        "GET",
        &format!("/projects/{copy_id}/decision-log/settings"),
        None,
        &admin,
    )
    .await
    .body;
    assert_eq!(copy_settings, settings);
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn a_broken_pack_file_is_refused_with_its_reason(db: PgPool) {
    let app = with_database(db);
    let admin = signed_in_admin(&app).await;
    let reply = upload(
        &app,
        "/packs/import?key=broken&name=Broken",
        b"not a zip".to_vec(),
        &admin,
    )
    .await;
    assert_error(&reply, StatusCode::UNPROCESSABLE_ENTITY, "INVALID_PACK");
    assert!(
        reply.body["details"]["reason"].is_string(),
        "{}",
        reply.body
    );
    assert_error(
        &upload(&app, "/packs/inspect", vec![0; 6 * 1024 * 1024], &admin).await,
        StatusCode::PAYLOAD_TOO_LARGE,
        "PACK_TOO_LARGE",
    );
    assert!(project_keys(&app, &admin).await.is_empty());
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn a_project_duplicates_from_its_drafts_or_a_release(db: PgPool) {
    let app = with_database(db);
    let admin = signed_in_admin(&app).await;
    let grace = signed_in_user(&app, GRACE, false).await;
    let project = imported(&app, &admin, "retail-credit", "retail-credit").await;
    let added = call(
        &app,
        "POST",
        &format!("/projects/{project}/members"),
        Some(json!({ "email": GRACE, "role": "editor" })),
        &admin,
    )
    .await;
    assert_eq!(added.status, StatusCode::CREATED, "{}", added.body);
    let release = call(
        &app,
        "POST",
        &format!("/projects/{project}/releases"),
        Some(json!({ "bump": "minor", "notes": "First" })),
        &admin,
    )
    .await;
    assert_eq!(release.status, StatusCode::CREATED, "{}", release.body);
    // A decision added after the release is in the drafts only.
    let extra = call(
        &app,
        "POST",
        &format!("/projects/{project}/decisions"),
        Some(json!({ "key": "pricing" })),
        &admin,
    )
    .await;
    assert_eq!(extra.status, StatusCode::CREATED, "{}", extra.body);

    let duplicate = |key: &'static str, release_id: Value| {
        let (app, admin, project) = (&app, &admin, &project);
        async move {
            call(
                app,
                "POST",
                &format!("/projects/{project}/duplicate"),
                Some(json!({ "key": key, "name": key, "releaseId": release_id })),
                admin,
            )
            .await
        }
    };
    let drafts = duplicate("from-drafts", Value::Null).await;
    assert_eq!(drafts.status, StatusCode::CREATED, "{}", drafts.body);
    let drafts_id = drafts.body["project"]["id"].as_str().unwrap();
    assert_eq!(
        decision_keys(&app, &admin, drafts_id).await,
        ["affordability", "pricing", "scorecard"]
    );

    let frozen = duplicate("from-release", release.body["id"].clone()).await;
    assert_eq!(frozen.status, StatusCode::CREATED, "{}", frozen.body);
    assert_eq!(frozen.body["tests"]["failed"], 0);
    let frozen_id = frozen.body["project"]["id"].as_str().unwrap();
    assert_eq!(
        decision_keys(&app, &admin, frozen_id).await,
        ["affordability", "scorecard"]
    );
    let created = event(&audit(&app, &admin, frozen_id).await, "project.created").clone();
    assert_eq!(
        created["details"]["from"],
        json!({
            "kind": "project",
            "key": "retail-credit",
            "name": "retail-credit",
            "release": release.body["version"],
        })
    );
    let events = audit(&app, &admin, &project).await;
    let duplicated: Vec<&Value> = events
        .iter()
        .filter(|event| event["action"] == "project.duplicated")
        .collect();
    assert_eq!(duplicated.len(), 2);
    assert_eq!(duplicated[0]["details"]["toKey"], "from-release");

    // Members, releases and tokens stay behind: the copy is the administrator's alone.
    assert_error(
        &call(&app, "GET", &format!("/projects/{frozen_id}"), None, &grace).await,
        StatusCode::NOT_FOUND,
        "PROJECT_NOT_FOUND",
    );
    let releases = call(
        &app,
        "GET",
        &format!("/projects/{frozen_id}/releases"),
        None,
        &admin,
    )
    .await;
    assert_eq!(releases.body["total"], 0);

    // A release of another project is not this project's.
    let other = imported(&app, &admin, "sme-treasury", "treasury").await;
    let reply = call(
        &app,
        "POST",
        &format!("/projects/{other}/duplicate"),
        Some(json!({ "key": "nope", "name": "Nope", "releaseId": release.body["id"] })),
        &admin,
    )
    .await;
    assert_error(&reply, StatusCode::NOT_FOUND, "RELEASE_NOT_FOUND");
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn only_owners_export(db: PgPool) {
    let app = with_database(db);
    let admin = signed_in_admin(&app).await;
    let grace = signed_in_user(&app, GRACE, false).await;
    let project = imported(&app, &admin, "retail-credit", "retail-credit").await;
    let added = call(
        &app,
        "POST",
        &format!("/projects/{project}/members"),
        Some(json!({ "email": GRACE, "role": "editor" })),
        &admin,
    )
    .await;
    assert_eq!(added.status, StatusCode::CREATED, "{}", added.body);
    let refused = download(&app, &format!("/projects/{project}/export"), &grace).await;
    assert_eq!(refused.status, StatusCode::FORBIDDEN);
    assert!(audit(&app, &admin, &project)
        .await
        .iter()
        .all(|event| event["action"] != "project.exported"));
}
