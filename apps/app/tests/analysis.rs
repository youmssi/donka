#![allow(clippy::unwrap_used)] // tests fail loudly on purpose

//! Impact analysis over the decision log (DNK-56): what a pending release would have decided
//! on the records production actually produced.

mod support;

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use chrono::{DateTime, Duration, Utc};
use donka_db::PgPool;
use serde_json::{json, Value};
use support::*;
use uuid::Uuid;

/// The second owner: the author of a release cannot approve it.
const ALAN: &str = "alan@bank.example";
const VIEWER: &str = "vera@bank.example";
const OUTSIDER: &str = "otto@bank.example";

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

/// A batch as a Runtime sends it: a bearer token, no cookie, no CSRF header.
async fn feed(app: &TestApp, token: &str, records: Value) -> Reply {
    send(
        &app.router,
        Request::post(format!("{BASE}/decision-log/records"))
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::AUTHORIZATION, format!("Bearer {token}"))
            .body(Body::from(json!({ "records": records }).to_string()))
            .unwrap(),
    )
    .await
}

/// The live decision: input over 10 scores 10, anything else scores 0.
fn table() -> Value {
    let path = format!(
        "{}/../../crates/engine/tests/fixtures/table.json",
        env!("CARGO_MANIFEST_DIR")
    );
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

/// The proposed decision: the same rule, scoring 20 instead of 10, so every record above the
/// threshold moves and the rest stay where they were.
fn proposed() -> Value {
    let mut content = table();
    let rules = content["nodes"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|node| node["type"] == "decisionTableNode")
        .unwrap()["content"]["rules"]
        .as_array_mut()
        .unwrap();
    for rule in rules.iter_mut() {
        if rule["xWauegxfG7"] == "> 10" {
            rule["qGAHmak0xj"] = json!("20");
        }
    }
    content
}

/// A production record as the Runtime sends it.
fn record(project: &str, release: &str, at: DateTime<Utc>, input: i64, output: i64) -> Value {
    json!({
        "id": Uuid::new_v4(),
        "projectId": project,
        "releaseId": release,
        "environment": "production",
        "decisionKey": "limit",
        "evaluatedAt": at,
        "durationUs": 420,
        "status": "succeeded",
        "input": { "input": input, "applicant": { "nationalId": "CM-1984-0042" } },
        "output": { "output": output },
        "trace": { "node": { "output": { "output": output } } }
    })
}

fn now(app: &TestApp) -> DateTime<Utc> {
    use donka_shared::clock::Clock;
    app.clock.now()
}

/// A project whose decision `limit` is live in production, with a second owner who approved
/// it. Returns the two owners' sessions, the project, the live release and the decision.
async fn in_production(app: &TestApp) -> (String, String, String, String, String) {
    let admin = signed_in_admin(app).await;
    let p = call(
        app,
        "POST",
        "/projects",
        Some(json!({ "key": "credit-pme", "name": "Crédit PME" })),
        &admin,
    )
    .await
    .body["id"]
        .as_str()
        .unwrap()
        .to_owned();

    let alan = signed_in_user(app, ALAN, false).await;
    let added = call(
        app,
        "POST",
        &format!("/projects/{p}/members"),
        Some(json!({ "email": ALAN, "role": "owner" })),
        &admin,
    )
    .await;
    assert_eq!(added.status, StatusCode::CREATED, "{}", added.body);

    let decision = call(
        app,
        "POST",
        &format!("/projects/{p}/decisions"),
        Some(json!({ "key": "limit", "content": table() })),
        &admin,
    )
    .await;
    assert_eq!(decision.status, StatusCode::CREATED, "{}", decision.body);
    let d = decision.body["id"].as_str().unwrap().to_owned();
    let rid = stage(app, &admin, &p, &d, 1, "First").await;
    approve(app, &admin, &alan, &p, &rid).await;
    (admin, alan, p, rid, d)
}

/// Versions the current draft, releases it and deploys that release to staging.
async fn stage(
    app: &TestApp,
    owner: &str,
    p: &str,
    decision: &str,
    revision: u32,
    notes: &str,
) -> String {
    let version = call(
        app,
        "POST",
        &format!("/projects/{p}/decisions/{decision}/versions"),
        Some(json!({ "message": notes, "revision": revision })),
        owner,
    )
    .await;
    assert_eq!(version.status, StatusCode::CREATED, "{}", version.body);
    let bump = if revision == 1 { "major" } else { "minor" };
    let release = call(
        app,
        "POST",
        &format!("/projects/{p}/releases"),
        Some(json!({ "bump": bump, "notes": notes })),
        owner,
    )
    .await;
    assert_eq!(release.status, StatusCode::CREATED, "{}", release.body);
    let rid = release.body["id"].as_str().unwrap().to_owned();
    let deployed = call(
        app,
        "POST",
        &format!("/projects/{p}/environments/staging/deployments"),
        Some(json!({ "releaseId": rid })),
        owner,
    )
    .await;
    assert_eq!(deployed.status, StatusCode::ACCEPTED, "{}", deployed.body);
    app.releases.publish_due().await.unwrap();
    rid
}

/// Asks for production, has the second owner approve it, and publishes it.
async fn approve(app: &TestApp, owner: &str, second: &str, p: &str, release: &str) {
    let asked = request_approval(app, owner, p, release).await;
    assert_eq!(asked.status, StatusCode::CREATED, "{}", asked.body);
    let id = asked.body["id"].as_str().unwrap();
    let approved = call(
        app,
        "POST",
        &format!("/projects/{p}/approvals/{id}/approve"),
        None,
        second,
    )
    .await;
    assert_eq!(approved.status, StatusCode::OK, "{}", approved.body);
    app.releases.publish_due().await.unwrap();
}

async fn request_approval(app: &TestApp, owner: &str, p: &str, release: &str) -> Reply {
    call(
        app,
        "POST",
        &format!("/projects/{p}/approvals"),
        Some(json!({ "releaseId": release })),
        owner,
    )
    .await
}

/// The project reads its outcome from the `output` field of each answer.
async fn outcome_field(app: &TestApp, owner: &str, p: &str) {
    let settings = call(
        app,
        "PUT",
        &format!("/projects/{p}/decision-log/settings"),
        Some(json!({ "outcomeField": "output" })),
        owner,
    )
    .await;
    assert_eq!(settings.status, StatusCode::OK, "{}", settings.body);
}

async fn production_token(app: &TestApp, owner: &str) -> String {
    let issued = call(
        app,
        "POST",
        "/decision-log/tokens",
        Some(json!({ "environment": "production", "name": "runtime-1" })),
        owner,
    )
    .await;
    assert_eq!(issued.status, StatusCode::CREATED, "{}", issued.body);
    issued.body["token"].as_str().unwrap().to_owned()
}

async fn start(
    app: &TestApp,
    session: &str,
    p: &str,
    approval: &str,
    key: &str,
    from: DateTime<Utc>,
    until: DateTime<Utc>,
) -> Reply {
    call(
        app,
        "POST",
        &format!("/projects/{p}/approvals/{approval}/analysis"),
        Some(json!({ "decisionKey": key, "from": from, "until": until })),
        session,
    )
    .await
}

/// Reads the job until it stops moving. The run is a background task, so the alternative is
/// asserting on a job that has not finished reading yet.
async fn settled(app: &TestApp, owner: &str, p: &str, job: &str) -> Value {
    for _ in 0..100 {
        let reply = call(
            app,
            "GET",
            &format!("/projects/{p}/analysis/{job}"),
            None,
            owner,
        )
        .await;
        assert_eq!(reply.status, StatusCode::OK, "{}", reply.body);
        match reply.body["status"].as_str().unwrap() {
            "queued" | "running" => tokio::time::sleep(std::time::Duration::from_millis(50)).await,
            _ => return reply.body,
        }
    }
    panic!("the analysis never reached a terminal state");
}

async fn audit_of(app: &TestApp, owner: &str, p: &str, action: &str) -> Value {
    call(
        app,
        "GET",
        &format!("/projects/{p}/audit?action={action}"),
        None,
        owner,
    )
    .await
    .body
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn an_owner_compares_a_pending_release_with_what_production_decided(db: PgPool) {
    let app = with_database(db.clone());
    let (admin, alan, p, live, decision) = in_production(&app).await;
    outcome_field(&app, &admin, &p).await;
    let at = now(&app);

    // Three real production decisions: two above the threshold, one below it.
    let token = production_token(&app, &admin).await;
    let batch = json!([
        record(&p, &live, at, 12, 10),
        record(&p, &live, at + Duration::seconds(1), 42, 10),
        record(&p, &live, at + Duration::seconds(2), 5, 0),
    ]);
    let receipt = feed(&app, &token, batch).await;
    assert_eq!(receipt.status, StatusCode::OK, "{}", receipt.body);
    assert_eq!(receipt.body["accepted"], 3, "{}", receipt.body);

    // The proposal: the same rule, scoring 20. It is on staging and waiting for a second owner.
    let saved = call(
        &app,
        "PUT",
        &format!("/projects/{p}/decisions/{decision}"),
        Some(json!({ "content": proposed(), "revision": 1 })),
        &admin,
    )
    .await;
    assert_eq!(saved.status, StatusCode::OK, "{}", saved.body);
    let candidate = stage(&app, &admin, &p, &decision, 2, "Higher limit").await;
    let asked = request_approval(&app, &admin, &p, &candidate).await;
    assert_eq!(asked.status, StatusCode::CREATED, "{}", asked.body);
    let approval = asked.body["id"].as_str().unwrap().to_owned();

    let queued = start(
        &app,
        &admin,
        &p,
        &approval,
        "limit",
        at - Duration::hours(1),
        at + Duration::hours(1),
    )
    .await;
    assert_eq!(queued.status, StatusCode::ACCEPTED, "{}", queued.body);
    assert_eq!(queued.body["status"], "queued");
    assert_eq!(queued.body["recordsRead"], 0, "nothing is read yet");
    let job = queued.body["id"].as_str().unwrap().to_owned();

    let finished = settled(&app, &admin, &p, &job).await;
    assert_eq!(finished["status"], "succeeded", "{finished}");
    assert_eq!(finished["recordsRead"], 3, "{finished}");
    let result = &finished["result"];
    assert_eq!(result["recordsRead"], 3);
    assert_eq!(result["recordsAvailable"], 3);
    assert_eq!(result["recordsCompared"], 3);
    assert_eq!(result["truncated"], false, "every record was read");
    // What production decided, and what the proposal would have decided for the same people.
    assert_eq!(result["before"], json!({ "0": 1, "10": 2 }), "{finished}");
    assert_eq!(result["after"], json!({ "0": 1, "20": 2 }), "{finished}");
    assert_eq!(result["uncomparable"], json!({}), "{finished}");

    // Only the cell that moved names its records: an unchanged cell has nothing to look at.
    let cells = result["transitions"].as_array().unwrap();
    let moved = cells
        .iter()
        .find(|cell| cell["before"] == "10" && cell["after"] == "20")
        .unwrap();
    assert_eq!(moved["count"], 2, "{finished}");
    assert_eq!(moved["records"].as_array().unwrap().len(), 2, "{finished}");
    let unchanged = cells
        .iter()
        .find(|cell| cell["before"] == "0" && cell["after"] == "0")
        .unwrap();
    assert_eq!(unchanged["count"], 1);
    assert!(
        unchanged["records"].as_array().unwrap().is_empty(),
        "an unchanged cell lists nobody: {unchanged}"
    );

    // The approval shows its most recent run without being told the job's identifier.
    let latest = call(
        &app,
        "GET",
        &format!("/projects/{p}/approvals/{approval}/analysis"),
        None,
        &admin,
    )
    .await;
    assert_eq!(latest.status, StatusCode::OK, "{}", latest.body);
    assert_eq!(latest.body["id"], json!(job));
    assert_eq!(latest.body["status"], "succeeded");

    // Starting and finishing a run are both recorded, once each.
    assert_eq!(
        audit_of(&app, &admin, &p, "analysis.run_started").await["total"],
        1
    );
    let completed = audit_of(&app, &admin, &p, "analysis.run_completed").await;
    assert_eq!(completed["total"], 1, "{completed}");
    assert_eq!(completed["items"][0]["details"]["status"], "succeeded");
    assert_eq!(completed["items"][0]["details"]["recordsRead"], 3);

    // Cancelling a run that is already over changes nothing and still answers with it.
    let late = call(
        &app,
        "POST",
        &format!("/projects/{p}/analysis/{job}/cancel"),
        None,
        &admin,
    )
    .await;
    assert_eq!(late.status, StatusCode::OK, "{}", late.body);
    assert_eq!(late.body["status"], "succeeded", "{}", late.body);

    // A job of another project, or none at all, is not found rather than refused.
    let unknown = Uuid::new_v4();
    assert_error(
        &call(
            &app,
            "GET",
            &format!("/projects/{p}/analysis/{unknown}"),
            None,
            &admin,
        )
        .await,
        StatusCode::NOT_FOUND,
        "ANALYSIS_NOT_FOUND",
    );
    assert_error(
        &call(
            &app,
            "POST",
            &format!("/projects/{p}/analysis/{unknown}/cancel"),
            None,
            &admin,
        )
        .await,
        StatusCode::NOT_FOUND,
        "ANALYSIS_NOT_FOUND",
    );

    // The second owner may read the run as well: this is a project matter, not the author's.
    let theirs = call(
        &app,
        "GET",
        &format!("/projects/{p}/analysis/{job}"),
        None,
        &alan,
    )
    .await;
    assert_eq!(theirs.status, StatusCode::OK, "{}", theirs.body);

    // A viewer may not: the result names the records of real people.
    let vera = signed_in_user(&app, VIEWER, false).await;
    let added = call(
        &app,
        "POST",
        &format!("/projects/{p}/members"),
        Some(json!({ "email": VIEWER, "role": "viewer" })),
        &admin,
    )
    .await;
    assert_eq!(added.status, StatusCode::CREATED, "{}", added.body);
    for (method, path) in [
        ("GET", format!("/projects/{p}/analysis/{job}")),
        (
            "GET",
            format!("/projects/{p}/approvals/{approval}/analysis"),
        ),
        ("POST", format!("/projects/{p}/analysis/{job}/cancel")),
    ] {
        assert_error(
            &call(&app, method, &path, None, &vera).await,
            StatusCode::FORBIDDEN,
            "FORBIDDEN",
        );
    }
    assert_error(
        &start(
            &app,
            &vera,
            &p,
            &approval,
            "limit",
            at - Duration::hours(1),
            at,
        )
        .await,
        StatusCode::FORBIDDEN,
        "FORBIDDEN",
    );

    // Someone outside the project sees no project at all, not a forbidden one.
    let otto = signed_in_user(&app, OUTSIDER, false).await;
    assert_error(
        &call(
            &app,
            "GET",
            &format!("/projects/{p}/analysis/{job}"),
            None,
            &otto,
        )
        .await,
        StatusCode::NOT_FOUND,
        "PROJECT_NOT_FOUND",
    );
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn an_analysis_that_cannot_answer_says_why_before_opening_a_record(db: PgPool) {
    let app = with_database(db.clone());
    let (admin, alan, p, _live, decision) = in_production(&app).await;
    let at = now(&app);

    let saved = call(
        &app,
        "PUT",
        &format!("/projects/{p}/decisions/{decision}"),
        Some(json!({ "content": proposed(), "revision": 1 })),
        &admin,
    )
    .await;
    assert_eq!(saved.status, StatusCode::OK, "{}", saved.body);
    let candidate = stage(&app, &admin, &p, &decision, 2, "Higher limit").await;
    let asked = request_approval(&app, &admin, &p, &candidate).await;
    assert_eq!(asked.status, StatusCode::CREATED, "{}", asked.body);
    let approval = asked.body["id"].as_str().unwrap().to_owned();

    // Without an outcome field there is nothing to compare, and the records stay closed.
    assert_error(
        &start(
            &app,
            &admin,
            &p,
            &approval,
            "limit",
            at - Duration::hours(1),
            at,
        )
        .await,
        StatusCode::UNPROCESSABLE_ENTITY,
        "ANALYSIS_OUTCOME_UNCONFIGURED",
    );
    outcome_field(&app, &admin, &p).await;

    // A period that ends before it starts is refused rather than silently read as empty.
    assert_error(
        &start(
            &app,
            &admin,
            &p,
            &approval,
            "limit",
            at,
            at - Duration::hours(1),
        )
        .await,
        StatusCode::BAD_REQUEST,
        "INVALID_REQUEST",
    );

    // A decision the proposed release does not contain names itself in the refusal.
    assert_error(
        &start(
            &app,
            &admin,
            &p,
            &approval,
            "pricing",
            at - Duration::hours(1),
            at,
        )
        .await,
        StatusCode::NOT_FOUND,
        "DECISION_NOT_FOUND",
    );

    // The run itself: the period holds no production record, so it succeeds with nothing
    // rather than failing, and nothing was opened.
    let queued = start(
        &app,
        &admin,
        &p,
        &approval,
        "limit",
        at - Duration::days(400),
        at - Duration::days(399),
    )
    .await;
    assert_eq!(queued.status, StatusCode::ACCEPTED, "{}", queued.body);
    let job = queued.body["id"].as_str().unwrap().to_owned();
    let finished = settled(&app, &admin, &p, &job).await;
    assert_eq!(finished["status"], "succeeded", "{finished}");
    assert_eq!(finished["recordsRead"], 0);
    assert_eq!(finished["result"]["recordsAvailable"], 0);
    assert_eq!(finished["result"]["truncated"], false);
    assert_eq!(finished["result"]["transitions"], json!([]));

    // Once the second owner has decided, the approval is settled: there is no longer a
    // proposal to compare production with.
    let approved = call(
        &app,
        "POST",
        &format!("/projects/{p}/approvals/{approval}/approve"),
        None,
        &alan,
    )
    .await;
    assert_eq!(approved.status, StatusCode::OK, "{}", approved.body);
    assert_error(
        &start(
            &app,
            &admin,
            &p,
            &approval,
            "limit",
            at - Duration::hours(1),
            at,
        )
        .await,
        StatusCode::NOT_FOUND,
        "APPROVAL_NOT_FOUND",
    );
}
