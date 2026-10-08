#![allow(clippy::unwrap_used)] // tests fail loudly on purpose

mod support;

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
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

/// A sync as a pipeline sends it: a bearer token, no cookie, no CSRF header.
async fn sync(app: &TestApp, token: Option<&str>, deployments: Value) -> Reply {
    let mut req = Request::post(format!("{BASE}/rules-sync"))
        .header(header::CONTENT_TYPE, "application/json");
    if let Some(token) = token {
        req = req.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }
    send(
        &app.router,
        req.body(Body::from(
            json!({ "deployments": deployments }).to_string(),
        ))
        .unwrap(),
    )
    .await
}

/// Downloads an artifact URL from a sync answer.
async fn download(app: &TestApp, token: &str, url: &str) -> (StatusCode, Vec<u8>) {
    let res = app
        .router
        .clone()
        .oneshot(
            Request::get(format!("{BASE}{url}"))
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = res.status();
    (
        status,
        res.into_body().collect().await.unwrap().to_bytes().to_vec(),
    )
}

fn sha256(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn config(zip: Vec<u8>) -> Value {
    use std::io::Read;
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(zip)).unwrap();
    let mut text = String::new();
    zip.by_name(".config/project.json")
        .unwrap()
        .read_to_string(&mut text)
        .unwrap();
    serde_json::from_str(&text).unwrap()
}

fn table() -> Value {
    let path = format!(
        "{}/../../crates/engine/tests/fixtures/table.json",
        env!("CARGO_MANIFEST_DIR")
    );
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

async fn project(app: &TestApp, admin: &str, key: &str) -> String {
    let reply = call(
        app,
        "POST",
        "/projects",
        Some(json!({ "key": key, "name": key })),
        admin,
    )
    .await;
    assert_eq!(reply.status, StatusCode::CREATED, "{}", reply.body);
    reply.body["id"].as_str().unwrap().to_owned()
}

/// A decision with a saved version, then a release; returns the release id.
async fn released(app: &TestApp, admin: &str, p: &str, bump: &str) -> String {
    let decisions = call(app, "GET", &format!("/projects/{p}/decisions"), None, admin).await;
    let id = match decisions.body["items"]
        .as_array()
        .and_then(|items| items.first())
    {
        Some(existing) => existing["id"].as_str().unwrap().to_owned(),
        None => {
            let d = call(
                app,
                "POST",
                &format!("/projects/{p}/decisions"),
                Some(json!({ "key": "limit", "content": table() })),
                admin,
            )
            .await;
            d.body["id"].as_str().unwrap().to_owned()
        }
    };
    let base = format!("/projects/{p}/decisions/{id}");
    let draft = call(app, "GET", &base, None, admin).await;
    let revision = draft.body["revision"].as_i64().unwrap();
    // A release needs something new: change the draft before each version.
    let mut content = table();
    content["nodes"][1]["name"] = json!(format!("Table {bump} {revision}"));
    let saved = call(
        app,
        "PUT",
        &base,
        Some(json!({ "content": content, "revision": revision })),
        admin,
    )
    .await;
    assert_eq!(saved.status, StatusCode::OK, "{}", saved.body);
    let v = call(
        app,
        "POST",
        &format!("{base}/versions"),
        Some(json!({ "message": "A version", "revision": revision + 1 })),
        admin,
    )
    .await;
    assert_eq!(v.status, StatusCode::CREATED, "{}", v.body);
    let r = call(
        app,
        "POST",
        &format!("/projects/{p}/releases"),
        Some(json!({ "bump": bump, "notes": format!("Release {bump}") })),
        admin,
    )
    .await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.body);
    r.body["id"].as_str().unwrap().to_owned()
}

async fn deploy_staging(app: &TestApp, admin: &str, p: &str, release: &str) {
    let d = call(
        app,
        "POST",
        &format!("/projects/{p}/environments/staging/deployments"),
        Some(json!({ "releaseId": release })),
        admin,
    )
    .await;
    assert_eq!(d.status, StatusCode::ACCEPTED, "{}", d.body);
    app.releases.publish_due().await.unwrap();
}

async fn ci_token(app: &TestApp, session: &str, p: &str) -> Reply {
    call(
        app,
        "POST",
        &format!("/projects/{p}/ci-tokens"),
        Some(json!({ "name": "loan-service deploy" })),
        session,
    )
    .await
}

fn by_target(reply: &Reply, target: &str) -> Value {
    reply.body["deployments"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["target"] == target)
        .unwrap_or_else(|| panic!("no answer for {target}: {}", reply.body))
        .clone()
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn a_pipeline_pulls_every_kind_of_target(db: PgPool) {
    let app = with_database(db);
    let admin = signed_in_admin(&app).await;
    let p = project(&app, &admin, "credit-pme").await;
    let token = ci_token(&app, &admin, &p).await.body["token"]
        .as_str()
        .unwrap()
        .to_owned();

    // Nothing released yet.
    let empty = sync(&app, Some(&token), json!([{ "project": "credit-pme" }])).await;
    assert_eq!(empty.status, StatusCode::OK, "{}", empty.body);
    assert_eq!(empty.body["nextPollAt"], Value::Null);
    assert_eq!(empty.body["deployments"][0]["action"], "no_release");
    assert_eq!(empty.body["deployments"][0]["target"], "main");

    let first = released(&app, &admin, &p, "major").await;
    let runtime_token = call(
        &app,
        "POST",
        &format!("/projects/{p}/environments/staging/tokens"),
        Some(json!({ "name": "runtime" })),
        &admin,
    )
    .await;
    assert_eq!(
        runtime_token.status,
        StatusCode::CREATED,
        "{}",
        runtime_token.body
    );
    deploy_staging(&app, &admin, &p, &first).await;
    let second = released(&app, &admin, &p, "minor").await;

    let reply = sync(
        &app,
        Some(&token),
        json!([
            { "project": "credit-pme", "target": "main", "alias": "newest" },
            { "project": p, "target": "release:1.0.0" },
            { "project": "credit-pme", "target": format!("commit:{first}") },
            { "project": "credit-pme", "target": "env:staging" },
            { "project": "credit-pme", "target": "env:production" },
            { "project": "credit-pme", "target": "release:9.9.9" },
            { "project": "credit-pme", "target": "branch:feature" },
            { "project": "credit-pme", "target": "latest" }
        ]),
    )
    .await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.body);

    let main = by_target(&reply, "main");
    assert_eq!(main["action"], "load");
    assert_eq!(main["alias"], "newest");
    assert_eq!(main["project"], json!({ "id": p, "key": "credit-pme" }));
    assert_eq!(main["release"]["id"], second);
    assert_eq!(main["release"]["semanticVersion"], "1.1.0");
    assert_eq!(main["commit"]["id"], second);
    assert!(main.get("environment").is_none());
    let (status, zip) = download(&app, &token, main["artifact"]["url"].as_str().unwrap()).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(sha256(&zip), main["artifact"]["sha256"]);
    let released_config = config(zip);
    assert_eq!(released_config["release"]["version"], "1.1.0");
    assert!(
        released_config.get("environment").is_none(),
        "{released_config}"
    );
    assert_eq!(released_config["accessTokenHashes"], json!([]));

    assert_eq!(by_target(&reply, "release:1.0.0")["release"]["id"], first);
    let commit = by_target(&reply, &format!("commit:{first}"));
    assert_eq!(
        (
            commit["action"].clone(),
            commit["release"]["version"].clone()
        ),
        (json!("load"), json!("1.0.0"))
    );

    // An environment's artifact is the one its Runtime reads, tokens included.
    let staging = by_target(&reply, "env:staging");
    assert_eq!(staging["action"], "load");
    assert_eq!(staging["release"]["id"], first);
    assert_eq!(staging["environment"]["key"], "staging");
    assert_ne!(
        staging["commit"]["id"],
        json!(first),
        "the deployment, not the release"
    );
    let (_, zip) = download(&app, &token, staging["artifact"]["url"].as_str().unwrap()).await;
    assert_eq!(sha256(&zip), staging["artifact"]["sha256"]);
    let staging_config = config(zip);
    assert_eq!(staging_config["environment"]["key"], "staging");
    assert_eq!(staging_config["deployment"]["id"], staging["commit"]["id"]);
    assert_eq!(
        staging_config["accessTokenHashes"][0]["environment"],
        "staging"
    );
    // Downloaded again, the same bytes.
    let (_, again) = download(&app, &token, staging["artifact"]["url"].as_str().unwrap()).await;
    assert_eq!(sha256(&again), staging["artifact"]["sha256"]);

    assert_eq!(by_target(&reply, "env:production")["action"], "no_release");
    for (target, code) in [
        ("release:9.9.9", "RELEASE_NOT_FOUND"),
        ("branch:feature", "UNSUPPORTED_TARGET"),
        ("latest", "INVALID_TARGET"),
    ] {
        let answer = by_target(&reply, target);
        assert_eq!(
            (answer["action"].clone(), answer["code"].clone()),
            (json!("error"), json!(code)),
            "{target}"
        );
    }

    // The token shows when it was last used.
    let tokens = call(
        &app,
        "GET",
        &format!("/projects/{p}/ci-tokens"),
        None,
        &admin,
    )
    .await;
    assert!(
        tokens.body["items"][0]["lastUsedAt"].is_string(),
        "{}",
        tokens.body
    );
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn an_unchanged_target_answers_no_change(db: PgPool) {
    let app = with_database(db);
    let admin = signed_in_admin(&app).await;
    let p = project(&app, &admin, "credit-pme").await;
    let token = ci_token(&app, &admin, &p).await.body["token"]
        .as_str()
        .unwrap()
        .to_owned();
    let release = released(&app, &admin, &p, "major").await;
    deploy_staging(&app, &admin, &p, &release).await;

    let held = |target: &str, id: &str| json!([{ "project": "credit-pme", "target": target, "current": { "commitId": id, "releaseId": id } }]);
    let main = sync(&app, Some(&token), held("main", &release)).await;
    assert_eq!(
        main.body["deployments"][0]["action"], "no_change",
        "{}",
        main.body
    );
    assert!(main.body["deployments"][0].get("artifact").is_none());

    let staging = sync(
        &app,
        Some(&token),
        json!([{ "project": "credit-pme", "target": "env:staging" }]),
    )
    .await;
    let deployment = staging.body["deployments"][0]["commit"]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let unchanged = sync(&app, Some(&token), held("env:staging", &deployment)).await;
    assert_eq!(unchanged.body["deployments"][0]["action"], "no_change");
    // The release id is not enough for an environment: its tokens can change.
    let by_release = sync(&app, Some(&token), held("env:staging", &release)).await;
    assert_eq!(by_release.body["deployments"][0]["action"], "load");

    // A new Runtime token republishes staging: the pipeline is told to load again.
    call(
        &app,
        "POST",
        &format!("/projects/{p}/environments/staging/tokens"),
        Some(json!({ "name": "runtime" })),
        &admin,
    )
    .await;
    app.releases.publish_due().await.unwrap();
    let after = sync(&app, Some(&token), held("env:staging", &deployment)).await;
    assert_eq!(
        after.body["deployments"][0]["action"], "load",
        "{}",
        after.body
    );
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn a_token_reaches_only_its_project(db: PgPool) {
    let app = with_database(db);
    let admin = signed_in_admin(&app).await;
    let p = project(&app, &admin, "credit-pme").await;
    let other = project(&app, &admin, "sme-treasury").await;
    let other_release = released(&app, &admin, &other, "major").await;
    let token = ci_token(&app, &admin, &p).await.body["token"]
        .as_str()
        .unwrap()
        .to_owned();
    let other_token = ci_token(&app, &admin, &other).await.body["token"]
        .as_str()
        .unwrap()
        .to_owned();

    let reply = sync(
        &app,
        Some(&token),
        json!([
            { "project": "sme-treasury" },
            { "project": other },
            { "project": "no-such-project" }
        ]),
    )
    .await;
    for answer in reply.body["deployments"].as_array().unwrap() {
        assert_eq!(answer["action"], "no_access", "{answer}");
        assert_eq!(answer["project"], Value::Null);
    }

    // Its artifact is not found with another project's token.
    let theirs = sync(
        &app,
        Some(&other_token),
        json!([{ "project": "sme-treasury" }]),
    )
    .await;
    let url = theirs.body["deployments"][0]["artifact"]["url"]
        .as_str()
        .unwrap()
        .to_owned();
    assert!(url.contains(&other_release));
    assert_eq!(download(&app, &other_token, &url).await.0, StatusCode::OK);
    let (status, body) = download(&app, &token, &url).await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "{}",
        String::from_utf8_lossy(&body)
    );
    assert_eq!(
        download(&app, &token, "/rules-sync/artifacts/x/releases/y")
            .await
            .0,
        StatusCode::NOT_FOUND
    );

    // No token, a wrong one, or a session cookie: refused.
    assert_error(
        &sync(&app, None, json!([])).await,
        StatusCode::UNAUTHORIZED,
        "INVALID_TOKEN",
    );
    assert_error(
        &sync(&app, Some("dnk_ci_nope"), json!([])).await,
        StatusCode::UNAUTHORIZED,
        "INVALID_TOKEN",
    );
    let with_cookie = call(
        &app,
        "POST",
        "/rules-sync",
        Some(json!({ "deployments": [] })),
        &admin,
    )
    .await;
    assert_error(&with_cookie, StatusCode::UNAUTHORIZED, "INVALID_TOKEN");
    let too_many: Vec<Value> = (0..51)
        .map(|_| json!({ "project": "credit-pme" }))
        .collect();
    assert_error(
        &sync(&app, Some(&token), json!(too_many)).await,
        StatusCode::BAD_REQUEST,
        "INVALID_REQUEST",
    );
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn owners_issue_and_revoke_ci_tokens_shown_once(db: PgPool) {
    let app = with_database(db);
    let admin = signed_in_admin(&app).await;
    let p = project(&app, &admin, "credit-pme").await;
    let grace = signed_in_user(&app, GRACE, false).await;
    call(
        &app,
        "POST",
        &format!("/projects/{p}/members"),
        Some(json!({ "email": GRACE, "role": "editor" })),
        &admin,
    )
    .await;

    assert_error(
        &ci_token(&app, &grace, &p).await,
        StatusCode::FORBIDDEN,
        "FORBIDDEN",
    );
    let blank = call(
        &app,
        "POST",
        &format!("/projects/{p}/ci-tokens"),
        Some(json!({ "name": " " })),
        &admin,
    )
    .await;
    assert_error(&blank, StatusCode::BAD_REQUEST, "INVALID_REQUEST");

    let issued = ci_token(&app, &admin, &p).await;
    assert_eq!(issued.status, StatusCode::CREATED, "{}", issued.body);
    let token = issued.body["token"].as_str().unwrap().to_owned();
    assert!(token.starts_with("dnk_ci_"));
    assert_eq!(issued.body["lastUsedAt"], Value::Null);

    // Members see the tokens by hint, never their value.
    let list = call(
        &app,
        "GET",
        &format!("/projects/{p}/ci-tokens"),
        None,
        &grace,
    )
    .await;
    assert_eq!(list.status, StatusCode::OK);
    assert_eq!(list.body["items"][0]["hint"], token[token.len() - 4..]);
    assert_eq!(list.body["items"][0]["createdBy"]["email"], ADMIN_EMAIL);
    assert!(!list.body.to_string().contains(&token));

    let id = issued.body["id"].as_str().unwrap();
    assert_error(
        &call(
            &app,
            "DELETE",
            &format!("/projects/{p}/ci-tokens/{id}"),
            None,
            &grace,
        )
        .await,
        StatusCode::FORBIDDEN,
        "FORBIDDEN",
    );
    let revoked = call(
        &app,
        "DELETE",
        &format!("/projects/{p}/ci-tokens/{id}"),
        None,
        &admin,
    )
    .await;
    assert_eq!(revoked.status, StatusCode::NO_CONTENT, "{}", revoked.body);
    assert_error(
        &sync(&app, Some(&token), json!([])).await,
        StatusCode::UNAUTHORIZED,
        "INVALID_TOKEN",
    );
    assert_error(
        &call(
            &app,
            "DELETE",
            &format!("/projects/{p}/ci-tokens/{id}"),
            None,
            &admin,
        )
        .await,
        StatusCode::NOT_FOUND,
        "TOKEN_NOT_FOUND",
    );

    let audit = call(&app, "GET", &format!("/projects/{p}/audit"), None, &admin).await;
    let actions: Vec<&str> = audit.body["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["action"].as_str().unwrap())
        .collect();
    assert!(
        actions.contains(&"ci_token.issued") && actions.contains(&"ci_token.revoked"),
        "{actions:?}"
    );
}
