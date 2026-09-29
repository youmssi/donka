#![allow(clippy::unwrap_used)] // tests fail loudly on purpose

mod support;

use axum::http::{header, StatusCode};
use chrono::Duration;
use donka_db::PgPool;
use donka_shared::clock::Clock;
use serde_json::{json, Value};
use support::*;

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

async fn project(app: &TestApp, session: &str, key: &str, name: &str) -> String {
    let reply = call(
        app,
        "POST",
        "/projects",
        Some(json!({ "key": key, "name": name })),
        session,
    )
    .await;
    assert_eq!(reply.status, StatusCode::CREATED, "{}", reply.body);
    reply.body["id"].as_str().unwrap().to_owned()
}

async fn user_id(app: &TestApp, email: &str) -> String {
    app.identity
        .find_by_email(email)
        .await
        .unwrap()
        .unwrap()
        .id
        .to_string()
}

/// The project's log as (action, actor email, target email), newest first.
async fn log(
    app: &TestApp,
    session: &str,
    project: &str,
    query: &str,
) -> Vec<(String, String, String)> {
    let reply = call(
        app,
        "GET",
        &format!("/projects/{project}/audit{query}"),
        None,
        session,
    )
    .await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.body);
    reply.body["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| {
            (
                e["action"].as_str().unwrap().to_owned(),
                e["actor"]["email"].as_str().unwrap_or("").to_owned(),
                e["target"]["email"].as_str().unwrap_or("").to_owned(),
            )
        })
        .collect()
}

async fn count(db: &PgPool) -> i64 {
    let (n,): (i64,) = sqlx::query_as("SELECT count(*) FROM audit_events")
        .fetch_one(db)
        .await
        .unwrap();
    n
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn every_project_change_writes_exactly_one_event(db: PgPool) {
    let app = with_database(db);
    let admin = signed_in_admin(&app).await;
    signed_in_user(&app, GRACE, false).await;
    let p = project(&app, &admin, "credit", "Credit").await;
    let grace = user_id(&app, GRACE).await;

    let steps: Vec<(&str, String, Option<Value>)> = vec![
        (
            "PATCH",
            format!("/projects/{p}"),
            Some(json!({ "name": "Credit scoring" })),
        ),
        // The same values again: no change, no event.
        (
            "PATCH",
            format!("/projects/{p}"),
            Some(json!({ "name": "Credit scoring" })),
        ),
        (
            "POST",
            format!("/projects/{p}/members"),
            Some(json!({ "email": GRACE, "role": "viewer" })),
        ),
        (
            "PATCH",
            format!("/projects/{p}/members/{grace}"),
            Some(json!({ "role": "editor" })),
        ),
        (
            "PATCH",
            format!("/projects/{p}/members/{grace}"),
            Some(json!({ "role": "editor" })),
        ),
        ("DELETE", format!("/projects/{p}/members/{grace}"), None),
        ("POST", format!("/projects/{p}/archive"), None),
        ("POST", format!("/projects/{p}/archive"), None),
        ("POST", format!("/projects/{p}/restore"), None),
    ];
    for (method, path, body) in steps {
        let reply = call(&app, method, &path, body, &admin).await;
        assert!(reply.status.is_success(), "{method} {path}: {}", reply.body);
    }

    let entries = log(&app, &admin, &p, "").await;
    let actions: Vec<&str> = entries.iter().map(|(a, _, _)| a.as_str()).collect();
    assert_eq!(
        actions,
        [
            "project.restored",
            "project.archived",
            "member.removed",
            "member.role_changed",
            "member.added",
            "project.updated",
            "project.created",
        ]
    );
    assert!(entries.iter().all(|(_, actor, _)| actor == ADMIN_EMAIL));
    assert_eq!(entries[3].2, GRACE, "the role change names who it is about");

    let reply = call(
        &app,
        "GET",
        &format!("/projects/{p}/audit?action=member.role_changed"),
        None,
        &admin,
    )
    .await;
    assert_eq!(
        reply.body["items"][0]["details"],
        json!({ "from": "viewer", "to": "editor" })
    );
    let reply = call(
        &app,
        "GET",
        &format!("/projects/{p}/audit?action=project.updated"),
        None,
        &admin,
    )
    .await;
    assert_eq!(reply.body["items"][0]["details"]["from"]["name"], "Credit");
    assert_eq!(
        reply.body["items"][0]["details"]["to"]["name"],
        "Credit scoring"
    );
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn account_changes_are_recorded_too(db: PgPool) {
    let app = with_database(db.clone());
    let admin = signed_in_admin(&app).await;
    // A wrong password for an existing account, a sign-out and a reset request.
    let wrong = sign_in(&app, ADMIN_EMAIL, "not the password").await;
    assert_eq!(wrong.status, StatusCode::UNAUTHORIZED);
    // Unknown accounts change nothing and leave no event.
    sign_in(&app, "nobody@bank.example", "whatever it is").await;
    send(
        &app.router,
        request("POST", &format!("{BASE}/auth/sign-out"), None, Some(&admin)),
    )
    .await;
    send(
        &app.router,
        request(
            "POST",
            &format!("{BASE}/auth/password-reset"),
            Some(&json!({ "email": ADMIN_EMAIL })),
            None,
        ),
    )
    .await;
    deliver(&app).await;

    let rows: Vec<(String, Option<String>, Value)> = sqlx::query_as(
        "SELECT e.action, a.email, e.details FROM audit_events e \
         LEFT JOIN users a ON a.id = e.actor_id WHERE e.project_id IS NULL ORDER BY e.id",
    )
    .fetch_all(&db)
    .await
    .unwrap();
    let actions: Vec<&str> = rows.iter().map(|(a, _, _)| a.as_str()).collect();
    assert_eq!(
        actions,
        [
            "user.invited",
            "user.password_set",
            "user.signed_in",
            "user.sign_in_failed",
            "user.signed_out",
            "user.password_reset_requested",
        ]
    );
    assert_eq!(rows[0].2["firstAdministrator"], true);
    assert_eq!(rows[3].1, None, "nobody is signed in when a sign-in fails");
    assert_eq!(rows[3].2, json!({ "locked": false }));
    assert_eq!(rows[5].2, json!({ "delivered": true }));
    // No secret ever lands in the log.
    let (leaks,): (i64,) = sqlx::query_as(
        "SELECT count(*) FROM audit_events WHERE details::text ILIKE '%password\"%' OR details::text ILIKE '%token%'",
    )
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(leaks, 0);
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn a_refused_change_leaves_no_event(db: PgPool) {
    let app = with_database(db.clone());
    let admin = signed_in_admin(&app).await;
    let viewer = signed_in_user(&app, GRACE, false).await;
    let p = project(&app, &admin, "credit", "Credit").await;
    call(
        &app,
        "POST",
        &format!("/projects/{p}/members"),
        Some(json!({ "email": GRACE, "role": "viewer" })),
        &admin,
    )
    .await;
    let admin_id = user_id(&app, ADMIN_EMAIL).await;
    let before = count(&db).await;

    let refused = [
        // The last owner cannot step down.
        call(
            &app,
            "PATCH",
            &format!("/projects/{p}/members/{admin_id}"),
            Some(json!({ "role": "viewer" })),
            &admin,
        )
        .await,
        // A viewer cannot rename.
        call(
            &app,
            "PATCH",
            &format!("/projects/{p}"),
            Some(json!({ "name": "Mine" })),
            &viewer,
        )
        .await,
        // Already a member.
        call(
            &app,
            "POST",
            &format!("/projects/{p}/members"),
            Some(json!({ "email": GRACE, "role": "owner" })),
            &admin,
        )
        .await,
    ];
    assert!(refused.iter().all(|r| r.status.is_client_error()));
    assert_eq!(count(&db).await, before);
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn events_cannot_be_changed_or_deleted(db: PgPool) {
    let app = with_database(db.clone());
    signed_in_admin(&app).await;
    for sql in [
        "UPDATE audit_events SET action = 'user.signed_out'",
        "DELETE FROM audit_events",
        "TRUNCATE audit_events",
    ] {
        let err = sqlx::query(sql).execute(&db).await.unwrap_err();
        let code = err.as_database_error().unwrap().code();
        // Refused by the application role's missing privilege (and, behind it, the trigger).
        assert_eq!(code.as_deref(), Some("42501"), "{sql}");
    }
    assert!(count(&db).await > 0);
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn owners_filter_by_person_action_and_date(db: PgPool) {
    let app = with_database(db);
    let admin = signed_in_admin(&app).await;
    signed_in_user(&app, GRACE, false).await;
    let p = project(&app, &admin, "credit", "Credit").await;
    call(
        &app,
        "POST",
        &format!("/projects/{p}/members"),
        Some(json!({ "email": GRACE, "role": "owner" })),
        &admin,
    )
    .await;
    let grace_session = session_from(&sign_in(&app, GRACE, "a long enough passphrase").await);

    app.clock.advance(Duration::hours(1));
    let later = app.clock.now();
    call(
        &app,
        "PATCH",
        &format!("/projects/{p}"),
        Some(json!({ "name": "Renamed by Grace" })),
        &grace_session,
    )
    .await;

    let grace = user_id(&app, GRACE).await;
    let by_grace = log(&app, &admin, &p, &format!("?actor={grace}")).await;
    assert_eq!(
        by_grace,
        [(
            "project.updated".to_owned(),
            GRACE.to_owned(),
            String::new()
        )]
    );

    let added = log(&app, &admin, &p, "?action=member.added").await;
    assert_eq!(added.len(), 1);

    let since = later.to_rfc3339().replace('+', "%2B");
    let recent = log(&app, &admin, &p, &format!("?from={since}")).await;
    assert_eq!(recent.len(), 1);
    let before = log(&app, &admin, &p, &format!("?until={since}")).await;
    assert_eq!(before.len(), 2, "created and member added");

    let page = call(
        &app,
        "GET",
        &format!("/projects/{p}/audit?limit=1&offset=1"),
        None,
        &admin,
    )
    .await;
    assert_eq!(page.body["total"], 3);
    assert_eq!(page.body["items"][0]["action"], "member.added");

    let bad = call(
        &app,
        "GET",
        &format!("/projects/{p}/audit?action=project.exploded"),
        None,
        &admin,
    )
    .await;
    assert_eq!(bad.status, StatusCode::BAD_REQUEST);
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn only_owners_read_the_audit_log(db: PgPool) {
    let app = with_database(db);
    let admin = signed_in_admin(&app).await;
    let grace = signed_in_user(&app, GRACE, false).await;
    let p = project(&app, &admin, "credit", "Credit").await;
    let other = project(&app, &admin, "other", "Other").await;
    call(
        &app,
        "POST",
        &format!("/projects/{p}/members"),
        Some(json!({ "email": GRACE, "role": "editor" })),
        &admin,
    )
    .await;

    for path in [
        format!("/projects/{p}/audit"),
        format!("/projects/{p}/audit/export"),
    ] {
        let reply = call(&app, "GET", &path, None, &grace).await;
        assert_error(&reply, StatusCode::FORBIDDEN, "FORBIDDEN");
    }
    for path in [
        format!("/projects/{other}/audit"),
        format!("/projects/{other}/audit/export"),
    ] {
        let reply = call(&app, "GET", &path, None, &grace).await;
        assert_error(&reply, StatusCode::NOT_FOUND, "PROJECT_NOT_FOUND");
    }
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn the_export_is_csv_with_the_same_filters(db: PgPool) {
    let app = with_database(db);
    let admin = signed_in_admin(&app).await;
    signed_in_user(&app, GRACE, false).await;
    // A name a spreadsheet would run as a formula.
    let p = project(
        &app,
        &admin,
        "credit",
        "=HYPERLINK(\"https://evil.example\")",
    )
    .await;
    call(
        &app,
        "POST",
        &format!("/projects/{p}/members"),
        Some(json!({ "email": GRACE, "role": "viewer" })),
        &admin,
    )
    .await;

    let res = {
        use tower::ServiceExt;
        app.router
            .clone()
            .oneshot(get(
                &format!("{BASE}/projects/{p}/audit/export"),
                Some(&admin),
            ))
            .await
            .unwrap()
    };
    assert_eq!(res.status(), StatusCode::OK);
    let header_value = |name| {
        res.headers()
            .get(name)
            .unwrap()
            .to_str()
            .unwrap()
            .to_owned()
    };
    assert_eq!(
        header_value(header::CONTENT_TYPE),
        "text/csv; charset=utf-8"
    );
    assert_eq!(
        header_value(header::CONTENT_DISPOSITION),
        "attachment; filename=\"audit-credit.csv\""
    );
    let body = http_body_util::BodyExt::collect(res.into_body())
        .await
        .unwrap()
        .to_bytes();
    let csv = String::from_utf8(body.to_vec()).unwrap();
    let lines: Vec<&str> = csv.trim_end().split("\r\n").collect();
    assert_eq!(lines[0], "occurred_at,actor,action,target,details");
    assert_eq!(lines.len(), 3, "{csv}");
    assert!(lines[1].contains("\"member.added\"") && lines[1].contains(GRACE));
    assert!(lines[2].contains("\"project.created\""));
    // The JSON details start with '{', so the name inside them is inert; the cell is quoted.
    assert!(lines[2].contains("HYPERLINK"));

    let filtered = send(
        &app.router,
        get(
            &format!("{BASE}/projects/{p}/audit/export?action=project.created"),
            Some(&admin),
        ),
    )
    .await;
    let rows = filtered
        .body
        .as_str()
        .unwrap()
        .trim_end()
        .split("\r\n")
        .count();
    assert_eq!(rows, 2, "header and one event");
}
