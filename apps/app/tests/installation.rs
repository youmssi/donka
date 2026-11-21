#![allow(clippy::unwrap_used)] // tests fail loudly on purpose

//! The installation (DNK-74): its first-run assistant and the organisation it serves.

mod support;

use axum::http::StatusCode;
use donka_db::PgPool;
use serde_json::{json, Value};
use std::borrow::Cow;
use support::*;

const INSTALLATION: &str = "/api/v1/installation";
const FIRST_RUN: &str = "/api/v1/installation/first-run";
const ORGANISATION: &str = "/api/v1/installation/organisation";

async fn installation(app: &TestApp) -> Value {
    let reply = send(&app.router, get(INSTALLATION, None)).await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.body);
    reply.body
}

/// Turns the second step on for the signed-in person.
async fn enrol(app: &TestApp, session: &str) {
    let started = send(
        &app.router,
        post(
            "/api/v1/account/second-step/enrolment",
            &json!({}),
            Some(session),
        ),
    )
    .await;
    assert_eq!(started.status, StatusCode::OK, "{}", started.body);
    let key = started.body["key"].as_str().unwrap().to_owned();
    let confirmed = send(
        &app.router,
        post(
            "/api/v1/account/second-step/enrolment/confirm",
            &json!({ "code": code_at(app, &key) }),
            Some(session),
        ),
    )
    .await;
    assert_eq!(confirmed.status, StatusCode::OK, "{}", confirmed.body);
}

async fn actions(db: &PgPool) -> Vec<(String, Value)> {
    sqlx::query_as(
        "SELECT action, details FROM audit_events WHERE action LIKE 'installation.%' \
         ORDER BY occurred_at, id",
    )
    .fetch_all(db)
    .await
    .unwrap()
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn a_new_installation_is_in_its_first_run_until_an_administrator_finishes_it(db: PgPool) {
    let app = with_database(db.clone());
    let admin = signed_in_admin(&app).await;
    assert_eq!(
        installation(&app).await,
        json!({ "firstRun": true, "organisation": null })
    );

    // Not without a second step: the first administrator holds every right.
    let organisation = json!({ "organisation": "  Microfinance SA " });
    let reply = send(&app.router, post(FIRST_RUN, &organisation, Some(&admin))).await;
    assert_error(&reply, StatusCode::CONFLICT, "SECOND_STEP_REQUIRED");
    // Nor by someone who is not an administrator, nor anonymously.
    let editor = signed_in_user(&app, "grace@bank.example", false).await;
    let reply = send(&app.router, post(FIRST_RUN, &organisation, Some(&editor))).await;
    assert_error(&reply, StatusCode::FORBIDDEN, "FORBIDDEN");
    let reply = send(&app.router, post(FIRST_RUN, &organisation, None)).await;
    assert_eq!(reply.status, StatusCode::UNAUTHORIZED);

    enrol(&app, &admin).await;
    let reply = send(
        &app.router,
        post(
            FIRST_RUN,
            &json!({ "organisation": "two\nlines" }),
            Some(&admin),
        ),
    )
    .await;
    assert_error(&reply, StatusCode::BAD_REQUEST, "INVALID_REQUEST");
    let reply = send(&app.router, post(FIRST_RUN, &organisation, Some(&admin))).await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.body);
    let set_up = json!({ "firstRun": false, "organisation": "Microfinance SA" });
    assert_eq!(reply.body, set_up);
    assert_eq!(installation(&app).await, set_up);

    // Once only.
    let reply = send(&app.router, post(FIRST_RUN, &organisation, Some(&admin))).await;
    assert_error(&reply, StatusCode::CONFLICT, "ALREADY_SET_UP");
    assert_eq!(
        actions(&db).await,
        vec![(
            "installation.set_up".to_owned(),
            json!({ "organisation": "Microfinance SA" })
        )]
    );
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn administrators_rename_the_organisation_once_set_up(db: PgPool) {
    let app = with_database(db.clone());
    let admin = signed_in_admin(&app).await;
    let rename = |name: &str| json!({ "organisation": name });
    let reply = send(
        &app.router,
        request("PUT", ORGANISATION, Some(&rename("Early")), Some(&admin)),
    )
    .await;
    assert_error(&reply, StatusCode::CONFLICT, "NOT_SET_UP");

    enrol(&app, &admin).await;
    let reply = send(
        &app.router,
        post(FIRST_RUN, &rename("Microfinance SA"), Some(&admin)),
    )
    .await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.body);

    let editor = signed_in_user(&app, "grace@bank.example", false).await;
    let reply = send(
        &app.router,
        request(
            "PUT",
            ORGANISATION,
            Some(&rename("Grace & Co")),
            Some(&editor),
        ),
    )
    .await;
    assert_error(&reply, StatusCode::FORBIDDEN, "FORBIDDEN");
    let reply = send(
        &app.router,
        request(
            "PUT",
            ORGANISATION,
            Some(&rename(&"x".repeat(101))),
            Some(&admin),
        ),
    )
    .await;
    assert_error(&reply, StatusCode::BAD_REQUEST, "INVALID_REQUEST");

    for _ in 0..2 {
        let reply = send(
            &app.router,
            request(
                "PUT",
                ORGANISATION,
                Some(&rename("Crédit Mutuel SA")),
                Some(&admin),
            ),
        )
        .await;
        assert_eq!(reply.status, StatusCode::OK, "{}", reply.body);
        assert_eq!(reply.body["organisation"], "Crédit Mutuel SA");
    }
    // Emails name the organisation, whatever their language.
    let reply = send(
        &app.router,
        post(
            "/api/v1/users/invitations",
            &json!({ "email": "lin@bank.example", "locale": "fr" }),
            Some(&admin),
        ),
    )
    .await;
    assert_eq!(reply.status, StatusCode::CREATED, "{}", reply.body);
    deliver(&app).await;
    let invitation = emails_to(&app, "lin@bank.example").pop().unwrap();
    assert!(
        invitation
            .text
            .ends_with("\n— Donka Studio · Crédit Mutuel SA\n"),
        "{}",
        invitation.text
    );

    // The same name again changes nothing, so it is audited once.
    let audited = actions(&db).await;
    assert_eq!(audited.len(), 2, "{audited:?}");
    assert_eq!(
        audited[1],
        (
            "installation.organisation_renamed".to_owned(),
            json!({ "from": "Microfinance SA", "to": "Crédit Mutuel SA" })
        )
    );
}

/// An installation people already sign in to is not sent through the assistant after the
/// upgrade that brings it.
#[sqlx::test(migrations = false)]
async fn an_installation_in_use_is_set_up_by_the_upgrade(db: PgPool) {
    const ASSISTANT: i64 = 20261029000001;
    let before = sqlx::migrate::Migrator {
        migrations: Cow::Owned(
            donka_db::MIGRATOR
                .iter()
                .filter(|migration| migration.version < ASSISTANT)
                .cloned()
                .collect(),
        ),
        ..sqlx::migrate::Migrator::DEFAULT
    };
    before.run(&db).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, email, password_hash, is_admin, created_at) \
         VALUES ($1, 'ada@bank.example', 'argon2id-hash', true, now())",
    )
    .bind(uuid::Uuid::new_v4())
    .execute(&db)
    .await
    .unwrap();
    donka_db::migrate(&db).await.unwrap();

    let app = with_database(db);
    assert_eq!(
        installation(&app).await,
        json!({ "firstRun": false, "organisation": null })
    );
}
