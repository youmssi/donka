#![allow(clippy::unwrap_used)] // tests fail loudly on purpose

mod support;

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use chrono::{DateTime, Duration, Utc};
use donka_db::PgPool;
use serde_json::{json, Value};
use support::*;
use uuid::Uuid;

const GRACE: &str = "grace@bank.example";
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
async fn feed(app: &TestApp, token: Option<&str>, records: Value) -> Reply {
    let mut req = Request::post(format!("{BASE}/decision-log/records"))
        .header(header::CONTENT_TYPE, "application/json");
    if let Some(token) = token {
        req = req.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }
    send(
        &app.router,
        req.body(Body::from(json!({ "records": records }).to_string()))
            .unwrap(),
    )
    .await
}

fn table() -> Value {
    let path = format!(
        "{}/../../crates/engine/tests/fixtures/table.json",
        env!("CARGO_MANIFEST_DIR")
    );
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

/// A project whose release 1.0.0 (`limit`, a table: input 12 → output 10) is
/// live on staging. Returns (admin session, project id, release id).
async fn released(app: &TestApp) -> (String, String, String) {
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
    let d = call(
        app,
        "POST",
        &format!("/projects/{p}/decisions"),
        Some(json!({ "key": "limit", "content": table() })),
        &admin,
    )
    .await;
    assert_eq!(d.status, StatusCode::CREATED, "{}", d.body);
    let v = call(
        app,
        "POST",
        &format!(
            "/projects/{p}/decisions/{}/versions",
            d.body["id"].as_str().unwrap()
        ),
        Some(json!({ "message": "First", "revision": 1 })),
        &admin,
    )
    .await;
    assert_eq!(v.status, StatusCode::CREATED, "{}", v.body);
    let r = call(
        app,
        "POST",
        &format!("/projects/{p}/releases"),
        Some(json!({ "bump": "major", "notes": "First release" })),
        &admin,
    )
    .await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.body);
    let rid = r.body["id"].as_str().unwrap().to_owned();
    let deployed = call(
        app,
        "POST",
        &format!("/projects/{p}/environments/staging/deployments"),
        Some(json!({ "releaseId": rid })),
        &admin,
    )
    .await;
    assert_eq!(deployed.status, StatusCode::ACCEPTED, "{}", deployed.body);
    app.releases.publish_due().await.unwrap();
    (admin, p, rid)
}

async fn issue(app: &TestApp, session: &str, environment: &str) -> Reply {
    call(
        app,
        "POST",
        "/decision-log/tokens",
        Some(json!({ "environment": environment, "name": "runtime-1" })),
        session,
    )
    .await
}

async fn staging_token(app: &TestApp, admin: &str) -> String {
    let reply = issue(app, admin, "staging").await;
    assert_eq!(reply.status, StatusCode::CREATED, "{}", reply.body);
    reply.body["token"].as_str().unwrap().to_owned()
}

/// A record as the Runtime builds it.
fn record(project: &str, release: &str, evaluated_at: DateTime<Utc>, extra: Value) -> Value {
    let mut record = json!({
        "id": Uuid::new_v4(),
        "projectId": project,
        "releaseId": release,
        "environment": "staging",
        "decisionKey": "limit",
        "evaluatedAt": evaluated_at,
        "durationUs": 420,
        "status": "succeeded",
        "input": { "input": 12, "applicant": { "nationalId": "CM-1984-0042" } },
        "output": { "output": 10 },
        "trace": { "node": { "output": { "output": 10 } } }
    });
    for (key, value) in extra.as_object().unwrap() {
        record[key] = value.clone();
    }
    record
}

fn now(app: &TestApp) -> DateTime<Utc> {
    use donka_shared::clock::Clock;
    app.clock.now()
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn a_runtime_sends_records_that_members_search_open_and_replay(db: PgPool) {
    let app = with_database(db.clone());
    let (admin, p, rid) = released(&app).await;
    let token = staging_token(&app, &admin).await;
    let at = now(&app);

    // The project reads its outcome from the `output` field of each answer.
    let settings = call(
        &app,
        "PUT",
        &format!("/projects/{p}/decision-log/settings"),
        Some(json!({ "outcomeField": "output" })),
        &admin,
    )
    .await;
    assert_eq!(settings.status, StatusCode::OK, "{}", settings.body);
    assert_eq!(settings.body["outcomeField"], "output");

    let approved = record(&p, &rid, at, json!({ "reference": "APP-2026-0042" }));
    let failed = record(
        &p,
        &rid,
        at + Duration::seconds(1),
        json!({ "status": "failed", "output": null, "error": { "message": "boom" }, "reference": "APP-2026-0043" }),
    );
    let production = record(&p, &rid, at, json!({ "environment": "production" }));
    let unknown_release = record(&p, &Uuid::new_v4().to_string(), at, json!({}));
    let malformed = record(&p, &rid, at, json!({ "decisionKey": "" }));
    let batch = json!([approved, failed, production, unknown_release, malformed]);

    let receipt = feed(&app, Some(&token), batch.clone()).await;
    assert_eq!(receipt.status, StatusCode::OK, "{}", receipt.body);
    assert_eq!(receipt.body["accepted"], 2);
    let rejected: Vec<(Value, Value)> = receipt.body["rejected"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| (r["id"].clone(), r["code"].clone()))
        .collect();
    assert_eq!(
        rejected,
        vec![
            (production["id"].clone(), json!("wrong_environment")),
            (unknown_release["id"].clone(), json!("unknown_release")),
            (malformed["id"].clone(), json!("invalid")),
        ]
    );

    // A batch sent again (a retry) is stored once.
    let again = feed(&app, Some(&token), batch).await;
    assert_eq!(again.body["accepted"], 2);
    let list = call(
        &app,
        "GET",
        &format!("/projects/{p}/decision-log"),
        None,
        &admin,
    )
    .await;
    assert_eq!(list.status, StatusCode::OK, "{}", list.body);
    assert_eq!(list.body["total"], 2);
    // Newest first, with nothing the decision read or answered.
    assert_eq!(list.body["items"][0]["id"], failed["id"]);
    assert_eq!(list.body["items"][0]["outcome"], "error");
    assert_eq!(list.body["items"][1]["outcome"], "10");
    assert!(!list.body.to_string().contains("CM-1984-0042"));

    // What was read and answered is encrypted at rest.
    let (payload,): (Vec<u8>,) =
        sqlx::query_as("SELECT payload FROM decision_records WHERE id = $1")
            .bind(Uuid::parse_str(approved["id"].as_str().unwrap()).unwrap())
            .fetch_one(&db)
            .await
            .unwrap();
    assert!(!String::from_utf8_lossy(&payload).contains("CM-1984-0042"));

    // Search by reference, decision, outcome, status, environment and date.
    let search = |query: &str| {
        let path = format!("/projects/{p}/decision-log?{query}");
        let app = &app;
        let admin = admin.clone();
        async move { call(app, "GET", &path, None, &admin).await.body }
    };
    let ids = |body: Value| -> Vec<Value> {
        body["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r["id"].clone())
            .collect()
    };
    assert_eq!(
        ids(search("reference=APP-2026-0042").await),
        vec![approved["id"].clone()]
    );
    assert_eq!(
        ids(search("outcome=10").await),
        vec![approved["id"].clone()]
    );
    assert_eq!(
        ids(search("status=failed").await),
        vec![failed["id"].clone()]
    );
    assert_eq!(search("decisionKey=limit").await["total"], 2);
    assert_eq!(search("decisionKey=other").await["total"], 0);
    assert_eq!(search("environment=production").await["total"], 0);
    let after = (at + Duration::milliseconds(500))
        .to_rfc3339()
        .replace('+', "%2B");
    assert_eq!(
        ids(search(&format!("from={after}")).await),
        vec![failed["id"].clone()]
    );
    assert_eq!(
        ids(search(&format!("until={after}")).await),
        vec![approved["id"].clone()]
    );
    let page = search("limit=1&offset=1").await;
    assert_eq!(
        (page["total"].clone(), ids(page)),
        (json!(2), vec![approved["id"].clone()])
    );

    // Opening a record shows it all, and is audited.
    let id = approved["id"].as_str().unwrap();
    let opened = call(
        &app,
        "GET",
        &format!("/projects/{p}/decision-log/{id}"),
        None,
        &admin,
    )
    .await;
    assert_eq!(opened.status, StatusCode::OK, "{}", opened.body);
    assert_eq!(opened.body["input"], approved["input"]);
    assert_eq!(opened.body["output"], json!({ "output": 10 }));
    assert_eq!(opened.body["trace"], approved["trace"]);
    assert_eq!(opened.body["releaseVersion"], "1.0.0");
    assert_eq!(opened.body["reference"], "APP-2026-0042");

    // Replay gives the recorded answer with the same release.
    let replayed = call(
        &app,
        "POST",
        &format!("/projects/{p}/decision-log/{id}/replay"),
        None,
        &admin,
    )
    .await;
    assert_eq!(replayed.status, StatusCode::OK, "{}", replayed.body);
    assert_eq!(replayed.body["identical"], true);
    assert_eq!(replayed.body["output"], json!({ "output": 10 }));

    // A record whose recorded answer differs from the release's is flagged.
    let altered = record(&p, &rid, at, json!({ "output": { "output": 11 } }));
    feed(&app, Some(&token), json!([altered])).await;
    let altered_id = altered["id"].as_str().unwrap();
    let replayed = call(
        &app,
        "POST",
        &format!("/projects/{p}/decision-log/{altered_id}/replay"),
        None,
        &admin,
    )
    .await;
    assert_eq!(replayed.body["identical"], false, "{}", replayed.body);
    assert_eq!(replayed.body["output"], json!({ "output": 10 }));

    let audit = call(&app, "GET", &format!("/projects/{p}/audit"), None, &admin).await;
    let actions: Vec<&str> = audit.body["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["action"].as_str().unwrap())
        .collect();
    assert!(actions.contains(&"decision_record.viewed"), "{actions:?}");
    assert_eq!(
        actions
            .iter()
            .filter(|a| **a == "decision_record.replayed")
            .count(),
        2
    );
    assert!(actions.contains(&"decision_log.settings_updated"));
    // A list is not an opening: no other decision-log event.
    assert_eq!(
        actions
            .iter()
            .filter(|a| a.starts_with("decision_record."))
            .count(),
        3
    );
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn the_feed_needs_a_live_token_for_its_environment(db: PgPool) {
    let app = with_database(db);
    let (admin, p, rid) = released(&app).await;
    let one = || json!([record(&p, &rid, Utc::now(), json!({}))]);

    assert_error(
        &feed(&app, None, one()).await,
        StatusCode::UNAUTHORIZED,
        "INVALID_TOKEN",
    );
    assert_error(
        &feed(&app, Some("dnk_log_not-a-token"), one()).await,
        StatusCode::UNAUTHORIZED,
        "INVALID_TOKEN",
    );

    // Tokens are shown once, listed by hint, and only administrators manage them.
    let grace = signed_in_user(&app, GRACE, false).await;
    assert_error(
        &issue(&app, &grace, "staging").await,
        StatusCode::FORBIDDEN,
        "FORBIDDEN",
    );
    assert_error(
        &call(&app, "GET", "/decision-log/tokens", None, &grace).await,
        StatusCode::FORBIDDEN,
        "FORBIDDEN",
    );
    let issued = issue(&app, &admin, "staging").await;
    let token = issued.body["token"].as_str().unwrap().to_owned();
    assert!(token.starts_with("dnk_log_"));
    assert_eq!(issued.body["environment"], "staging");
    let list = call(&app, "GET", "/decision-log/tokens", None, &admin).await;
    assert_eq!(list.body["items"][0]["hint"], token[token.len() - 4..]);
    assert!(!list.body.to_string().contains(&token));
    let blank = call(
        &app,
        "POST",
        "/decision-log/tokens",
        Some(json!({ "environment": "staging", "name": " " })),
        &admin,
    )
    .await;
    assert_error(&blank, StatusCode::BAD_REQUEST, "INVALID_REQUEST");

    // A production token cannot write staging records.
    let production = issue(&app, &admin, "production").await.body["token"]
        .as_str()
        .unwrap()
        .to_owned();
    let receipt = feed(&app, Some(&production), one()).await;
    assert_eq!(receipt.body["accepted"], 0);
    assert_eq!(receipt.body["rejected"][0]["code"], "wrong_environment");

    assert_eq!(feed(&app, Some(&token), one()).await.body["accepted"], 1);
    let too_many: Vec<Value> = (0..=donka_decision_log::MAX_BATCH_RECORDS)
        .map(|_| json!({}))
        .collect();
    assert_error(
        &feed(&app, Some(&token), json!(too_many)).await,
        StatusCode::BAD_REQUEST,
        "TOO_MANY_RECORDS",
    );

    // A revoked token is refused from then on.
    let id = issued.body["id"].as_str().unwrap();
    let revoked = call(
        &app,
        "DELETE",
        &format!("/decision-log/tokens/{id}"),
        None,
        &admin,
    )
    .await;
    assert_eq!(revoked.status, StatusCode::NO_CONTENT, "{}", revoked.body);
    assert_error(
        &feed(&app, Some(&token), one()).await,
        StatusCode::UNAUTHORIZED,
        "INVALID_TOKEN",
    );
    assert_error(
        &call(
            &app,
            "DELETE",
            &format!("/decision-log/tokens/{id}"),
            None,
            &admin,
        )
        .await,
        StatusCode::NOT_FOUND,
        "TOKEN_NOT_FOUND",
    );
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn every_member_reads_the_log_and_other_projects_see_nothing(db: PgPool) {
    let app = with_database(db);
    let (admin, p, rid) = released(&app).await;
    let token = staging_token(&app, &admin).await;
    let logged = record(&p, &rid, now(&app), json!({}));
    feed(&app, Some(&token), json!([logged])).await;
    let id = logged["id"].as_str().unwrap();

    let vera = signed_in_user(&app, VIEWER, false).await;
    let otto = signed_in_user(&app, OUTSIDER, false).await;
    let added = call(
        &app,
        "POST",
        &format!("/projects/{p}/members"),
        Some(json!({ "email": VIEWER, "role": "viewer" })),
        &admin,
    )
    .await;
    assert_eq!(added.status, StatusCode::CREATED, "{}", added.body);

    // A viewer searches, opens and replays.
    assert_eq!(
        call(
            &app,
            "GET",
            &format!("/projects/{p}/decision-log"),
            None,
            &vera
        )
        .await
        .body["total"],
        1
    );
    assert_eq!(
        call(
            &app,
            "GET",
            &format!("/projects/{p}/decision-log/{id}"),
            None,
            &vera
        )
        .await
        .status,
        StatusCode::OK
    );
    assert_eq!(
        call(
            &app,
            "POST",
            &format!("/projects/{p}/decision-log/{id}/replay"),
            None,
            &vera
        )
        .await
        .status,
        StatusCode::OK
    );
    // …but cannot change the settings.
    assert_error(
        &call(
            &app,
            "PUT",
            &format!("/projects/{p}/decision-log/settings"),
            Some(json!({ "outcomeField": "decision" })),
            &vera,
        )
        .await,
        StatusCode::FORBIDDEN,
        "FORBIDDEN",
    );

    // Someone outside the project sees nothing of it.
    for (method, path) in [
        ("GET", format!("/projects/{p}/decision-log")),
        ("GET", format!("/projects/{p}/decision-log/{id}")),
        ("POST", format!("/projects/{p}/decision-log/{id}/replay")),
        ("GET", format!("/projects/{p}/decision-log/settings")),
    ] {
        assert_error(
            &call(&app, method, &path, None, &otto).await,
            StatusCode::NOT_FOUND,
            "PROJECT_NOT_FOUND",
        );
    }

    // A record of another project is not found through this one.
    let other = call(
        &app,
        "POST",
        "/projects",
        Some(json!({ "key": "other", "name": "Other" })),
        &admin,
    )
    .await
    .body["id"]
        .as_str()
        .unwrap()
        .to_owned();
    assert_error(
        &call(
            &app,
            "GET",
            &format!("/projects/{other}/decision-log/{id}"),
            None,
            &admin,
        )
        .await,
        StatusCode::NOT_FOUND,
        "RECORD_NOT_FOUND",
    );
    assert_error(
        &call(
            &app,
            "GET",
            &format!("/projects/{p}/decision-log/not-a-uuid"),
            None,
            &admin,
        )
        .await,
        StatusCode::NOT_FOUND,
        "RECORD_NOT_FOUND",
    );
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn the_outcome_field_is_a_dotted_path_set_by_owners(db: PgPool) {
    let app = with_database(db);
    let (admin, p, _) = released(&app).await;
    let path = format!("/projects/{p}/decision-log/settings");

    assert_eq!(
        call(&app, "GET", &path, None, &admin).await.body["outcomeField"],
        Value::Null
    );
    for bad in ["1st", "a-b", "a..b", "result."] {
        assert_error(
            &call(
                &app,
                "PUT",
                &path,
                Some(json!({ "outcomeField": bad })),
                &admin,
            )
            .await,
            StatusCode::BAD_REQUEST,
            "INVALID_REQUEST",
        );
    }
    let set = call(
        &app,
        "PUT",
        &path,
        Some(json!({ "outcomeField": "result.band" })),
        &admin,
    )
    .await;
    assert_eq!(set.body["outcomeField"], "result.band");
    let cleared = call(
        &app,
        "PUT",
        &path,
        Some(json!({ "outcomeField": null })),
        &admin,
    )
    .await;
    assert_eq!(cleared.body["outcomeField"], Value::Null);
    assert_eq!(
        call(&app, "GET", &path, None, &admin).await.body["outcomeField"],
        Value::Null
    );
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn records_past_retention_are_purged_and_each_purge_is_audited(db: PgPool) {
    let app = with_database(db.clone());
    let (admin, p, rid) = released(&app).await;
    let token = staging_token(&app, &admin).await;
    let at = now(&app);
    let old = record(&p, &rid, at - Duration::days(RETENTION_DAYS + 1), json!({}));
    let recent = record(&p, &rid, at - Duration::days(RETENTION_DAYS - 1), json!({}));
    feed(&app, Some(&token), json!([old, recent])).await;

    // Records cannot be changed or deleted outside the purge.
    for statement in [
        "UPDATE decision_records SET outcome = 'approve'",
        "DELETE FROM decision_records",
        "TRUNCATE decision_records",
    ] {
        let err = sqlx::query(statement).execute(&db).await.unwrap_err();
        assert!(
            err.to_string().contains("append-only")
                || err.to_string().contains("permission denied"),
            "{statement}: {err}"
        );
    }

    assert_eq!(app.decision_log.purge().await.unwrap(), 1);
    assert_eq!(
        app.decision_log.purge().await.unwrap(),
        0,
        "nothing left to purge"
    );
    let list = call(
        &app,
        "GET",
        &format!("/projects/{p}/decision-log"),
        None,
        &admin,
    )
    .await;
    assert_eq!(list.body["total"], 1);
    assert_eq!(list.body["items"][0]["id"], recent["id"]);

    // One audit event for the purge that deleted something, none for the empty one.
    let audit = call(
        &app,
        "GET",
        &format!("/projects/{p}/audit?action=decision_log.purged"),
        None,
        &admin,
    )
    .await;
    assert_eq!(audit.body["total"], 1, "{}", audit.body);
    let event = &audit.body["items"][0];
    assert_eq!(event["actor"], Value::Null);
    assert_eq!(event["details"]["records"], 1);
    assert_eq!(event["details"]["retentionDays"], RETENTION_DAYS);
}

// ----- Explanations (DNK-19) -------------------------------------------------

/// A fake LLM endpoint: records what it is sent, answers what it is told.
#[derive(Clone)]
struct Llm {
    sent: std::sync::Arc<std::sync::Mutex<Vec<Value>>>,
    answer: std::sync::Arc<std::sync::Mutex<(StatusCode, Value)>>,
}

impl Llm {
    async fn start() -> (Self, String) {
        use axum::routing::post;
        let llm = Llm {
            sent: Default::default(),
            answer: std::sync::Arc::new(std::sync::Mutex::new(answering("No answer set."))),
        };
        let app = axum::Router::new()
            .route(
                "/v1/messages",
                post(
                    |axum::extract::State(llm): axum::extract::State<Llm>,
                     axum::Json(body): axum::Json<Value>| async move {
                        llm.sent.lock().unwrap().push(body);
                        let (status, answer) = llm.answer.lock().unwrap().clone();
                        (status, axum::Json(answer))
                    },
                ),
            )
            .with_state(llm.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/v1/messages", listener.local_addr().unwrap());
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        (llm, url)
    }

    fn answer(&self, status: StatusCode, body: Value) {
        *self.answer.lock().unwrap() = (status, body);
    }

    /// The text of the last request's user message.
    fn last_prompt(&self) -> String {
        let sent = self.sent.lock().unwrap();
        sent.last().unwrap()["messages"][0]["content"]
            .as_str()
            .unwrap()
            .to_owned()
    }
}

fn answering(text: &str) -> (StatusCode, Value) {
    (
        StatusCode::OK,
        json!({ "model": "bank-llm", "stop_reason": "end_turn", "content": [{ "type": "text", "text": text }] }),
    )
}

async fn explaining_app(db: PgPool) -> (TestApp, Llm) {
    let (llm, url) = Llm::start().await;
    let app = with_explainer(
        db,
        donka_explain::Settings {
            url,
            api_key: "llm-key".into(),
            protocol: donka_explain::Protocol::Anthropic,
            model: "bank-llm".into(),
            timeout: std::time::Duration::from_secs(5),
            fallbacks: false,
        },
    );
    (app, llm)
}

async fn explain(app: &TestApp, session: &str, p: &str, id: &str, language: &str) -> Reply {
    call(
        app,
        "POST",
        &format!("/projects/{p}/decision-log/{id}/explain"),
        Some(json!({ "language": language })),
        session,
    )
    .await
}

async fn actions(app: &TestApp, admin: &str, p: &str) -> Vec<String> {
    call(app, "GET", &format!("/projects/{p}/audit"), None, admin)
        .await
        .body["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["action"].as_str().unwrap().to_owned())
        .collect()
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn explanations_are_off_unless_configured(db: PgPool) {
    let app = with_database(db);
    let (admin, p, rid) = released(&app).await;
    let token = staging_token(&app, &admin).await;
    let logged = record(&p, &rid, now(&app), json!({}));
    feed(&app, Some(&token), json!([logged])).await;
    let id = logged["id"].as_str().unwrap();

    let opened = call(
        &app,
        "GET",
        &format!("/projects/{p}/decision-log/{id}"),
        None,
        &admin,
    )
    .await;
    assert_eq!(opened.body["explainable"], false);
    let settings = call(
        &app,
        "GET",
        &format!("/projects/{p}/decision-log/settings"),
        None,
        &admin,
    )
    .await;
    assert_eq!(settings.body["explainEnabled"], false);
    assert_eq!(settings.body["redactedFields"], json!([]));

    assert_error(
        &explain(&app, &admin, &p, id, "en").await,
        StatusCode::NOT_FOUND,
        "NOT_FOUND",
    );
    assert!(!actions(&app, &admin, &p)
        .await
        .contains(&"decision_record.explained".to_owned()));
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn a_member_reads_an_explanation_without_the_redacted_fields(db: PgPool) {
    let (app, llm) = explaining_app(db).await;
    let (admin, p, rid) = released(&app).await;
    let token = staging_token(&app, &admin).await;
    let logged = record(&p, &rid, now(&app), json!({ "reference": "APP-7" }));
    feed(&app, Some(&token), json!([logged])).await;
    let id = logged["id"].as_str().unwrap();

    let settings = call(
        &app,
        "PUT",
        &format!("/projects/{p}/decision-log/settings"),
        Some(json!({ "outcomeField": "output", "redactedFields": ["applicant.nationalId"] })),
        &admin,
    )
    .await;
    assert_eq!(settings.status, StatusCode::OK, "{}", settings.body);
    assert_eq!(
        settings.body["redactedFields"],
        json!(["applicant.nationalId"])
    );
    assert_eq!(settings.body["explainEnabled"], true);

    let vera = signed_in_user(&app, VIEWER, false).await;
    call(
        &app,
        "POST",
        &format!("/projects/{p}/members"),
        Some(json!({ "email": VIEWER, "role": "viewer" })),
        &admin,
    )
    .await;
    let opened = call(
        &app,
        "GET",
        &format!("/projects/{p}/decision-log/{id}"),
        None,
        &vera,
    )
    .await;
    assert_eq!(opened.body["explainable"], true);
    // Opening still shows everything; only what leaves Studio is redacted.
    assert_eq!(
        opened.body["input"]["applicant"]["nationalId"],
        "CM-1984-0042"
    );

    let (status, body) = answering("Le revenu dépasse le seuil de la table.");
    llm.answer(status, body);
    let explained = explain(&app, &vera, &p, id, "fr").await;
    assert_eq!(explained.status, StatusCode::OK, "{}", explained.body);
    assert_eq!(
        explained.body["explanation"],
        "Le revenu dépasse le seuil de la table."
    );
    assert_eq!(explained.body["model"], "bank-llm");

    let prompt = llm.last_prompt();
    assert!(!prompt.contains("CM-1984-0042"), "{prompt}");
    assert!(!prompt.contains("nationalId"), "{prompt}");
    assert!(prompt.contains("\"input\":12"), "{prompt}");
    assert!(prompt.contains("\"release\":\"1.0.0\""), "{prompt}");
    assert!(prompt.ends_with("Write the explanation in French."));

    // Each explanation is audited for the reader who asked.
    let audit = call(
        &app,
        "GET",
        &format!("/projects/{p}/audit?action=decision_record.explained"),
        None,
        &admin,
    )
    .await;
    assert_eq!(audit.body["total"], 1, "{}", audit.body);
    assert_eq!(audit.body["items"][0]["actor"]["email"], VIEWER);
    assert_eq!(audit.body["items"][0]["details"]["reference"], "APP-7");
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn a_failed_or_declined_explanation_says_so(db: PgPool) {
    let (app, llm) = explaining_app(db).await;
    let (admin, p, rid) = released(&app).await;
    let token = staging_token(&app, &admin).await;
    let logged = record(&p, &rid, now(&app), json!({}));
    feed(&app, Some(&token), json!([logged])).await;
    let id = logged["id"].as_str().unwrap();

    llm.answer(
        StatusCode::INTERNAL_SERVER_ERROR,
        json!({ "error": "overloaded" }),
    );
    assert_error(
        &explain(&app, &admin, &p, id, "en").await,
        StatusCode::BAD_GATEWAY,
        "EXPLAIN_UNAVAILABLE",
    );
    llm.answer(
        StatusCode::OK,
        json!({ "stop_reason": "refusal", "content": [] }),
    );
    assert_error(
        &explain(&app, &admin, &p, id, "en").await,
        StatusCode::UNPROCESSABLE_ENTITY,
        "EXPLAIN_DECLINED",
    );
    // The record was sent both times, so both are audited.
    let explained = actions(&app, &admin, &p)
        .await
        .into_iter()
        .filter(|a| a == "decision_record.explained")
        .count();
    assert_eq!(explained, 2);

    // Nothing is sent for a record that is not there or a project one cannot see.
    let sent = llm.sent.lock().unwrap().len();
    assert_error(
        &explain(&app, &admin, &p, &Uuid::new_v4().to_string(), "en").await,
        StatusCode::NOT_FOUND,
        "RECORD_NOT_FOUND",
    );
    let otto = signed_in_user(&app, OUTSIDER, false).await;
    assert_error(
        &explain(&app, &otto, &p, id, "en").await,
        StatusCode::NOT_FOUND,
        "PROJECT_NOT_FOUND",
    );
    assert_error(
        &explain(&app, &admin, &p, id, "de").await,
        StatusCode::BAD_REQUEST,
        "INVALID_REQUEST",
    );
    assert_eq!(llm.sent.lock().unwrap().len(), sent);
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn owners_list_the_redacted_fields(db: PgPool) {
    let app = with_database(db);
    let (admin, p, _) = released(&app).await;
    let path = format!("/projects/{p}/decision-log/settings");
    let put = |body: Value| {
        let (app, admin, path) = (&app, admin.clone(), path.clone());
        async move { call(app, "PUT", &path, Some(body), &admin).await }
    };

    for bad in [
        json!(["a-b"]),
        json!([""]),
        json!((0..=50).map(|i| format!("f{i}")).collect::<Vec<_>>()),
    ] {
        assert_error(
            &put(json!({ "outcomeField": null, "redactedFields": bad })).await,
            StatusCode::BAD_REQUEST,
            "INVALID_REQUEST",
        );
    }
    let set = put(json!({ "outcomeField": "decision", "redactedFields": [" applicant.name ", "applicant.name", "iban"] })).await;
    assert_eq!(
        set.body["redactedFields"],
        json!(["applicant.name", "iban"])
    );
    // Left out, the list stays; the outcome field is replaced as before.
    let kept = put(json!({ "outcomeField": null })).await;
    assert_eq!(
        kept.body["redactedFields"],
        json!(["applicant.name", "iban"])
    );
    assert_eq!(kept.body["outcomeField"], Value::Null);

    let audit = call(
        &app,
        "GET",
        &format!("/projects/{p}/audit?action=decision_log.settings_updated"),
        None,
        &admin,
    )
    .await;
    assert_eq!(
        audit.body["items"][1]["details"]["to"]["redactedFields"],
        json!(["applicant.name", "iban"])
    );

    let vera = signed_in_user(&app, VIEWER, false).await;
    call(
        &app,
        "POST",
        &format!("/projects/{p}/members"),
        Some(json!({ "email": VIEWER, "role": "editor" })),
        &admin,
    )
    .await;
    assert_error(
        &call(
            &app,
            "PUT",
            &path,
            Some(json!({ "outcomeField": null, "redactedFields": [] })),
            &vera,
        )
        .await,
        StatusCode::FORBIDDEN,
        "FORBIDDEN",
    );
}
