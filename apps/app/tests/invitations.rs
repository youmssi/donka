#![allow(clippy::unwrap_used)] // tests fail loudly on purpose

mod support;

use axum::http::StatusCode;
use chrono::Duration;
use donka_db::PgPool;
use serde_json::{json, Value};
use support::*;

const INVITEE: &str = "grace@bank.example";
const INVITEE_PASSWORD: &str = "a long enough passphrase";

async fn invite(app: &TestApp, session: &str, body: Value) -> Reply {
    send(
        &app.router,
        post(&format!("{BASE}/users/invitations"), &body, Some(session)),
    )
    .await
}

async fn set_password(app: &TestApp, token: &str, password: &str) -> Reply {
    let body = json!({ "token": token, "password": password });
    send(
        &app.router,
        post(&format!("{BASE}/auth/password-setup"), &body, None),
    )
    .await
}

async fn request_reset(app: &TestApp, email: &str) -> Reply {
    send(
        &app.router,
        post(
            &format!("{BASE}/auth/password-reset"),
            &json!({ "email": email }),
            None,
        ),
    )
    .await
}

/// An invited, activated, non-admin user; returns their session.
async fn signed_in_member(app: &TestApp, admin: &str) -> String {
    invite(app, admin, json!({ "email": INVITEE })).await;
    deliver(app).await;
    let token = token_in(emails_to(app, INVITEE).last().unwrap());
    set_password(app, &token, INVITEE_PASSWORD).await;
    session_from(&sign_in(app, INVITEE, INVITEE_PASSWORD).await)
}

// --- invitations ---------------------------------------------------------------

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn an_invited_person_chooses_a_password_and_signs_in(db: PgPool) {
    let app = with_database(db);
    let admin = signed_in_admin(&app).await;

    let reply = invite(
        &app,
        &admin,
        json!({ "email": " Grace@Bank.example ", "locale": "fr" }),
    )
    .await;
    assert_eq!(reply.status, StatusCode::CREATED, "{}", reply.body);
    assert_eq!(reply.body["email"], INVITEE);
    assert_eq!(reply.body["isAdmin"], false);
    assert_eq!(reply.body["locale"], "fr");

    // Queued, not sent inside the request: the worker sends it after commit.
    assert!(app.mailer.sent().is_empty());
    assert_eq!(deliver(&app).await, 1);
    let emails = emails_to(&app, INVITEE);
    assert_eq!(emails.len(), 1);
    assert_eq!(emails[0].subject, "Vous êtes invité(e) sur Donka");
    assert!(emails[0].text.contains("72 heures"), "{}", emails[0].text);

    let token = token_in_locale(&emails[0], "fr");
    assert_eq!(
        set_password(&app, &token, INVITEE_PASSWORD).await.status,
        StatusCode::NO_CONTENT
    );
    let signed_in = sign_in(&app, INVITEE, INVITEE_PASSWORD).await;
    assert_eq!(signed_in.status, StatusCode::OK, "{}", signed_in.body);
    assert_eq!(signed_in.body["locale"], "fr");
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn an_invitation_link_works_once(db: PgPool) {
    let app = with_database(db);
    let admin = signed_in_admin(&app).await;
    invite(&app, &admin, json!({ "email": INVITEE })).await;
    deliver(&app).await;
    let token = token_in(&emails_to(&app, INVITEE)[0]);

    assert_eq!(
        set_password(&app, &token, INVITEE_PASSWORD).await.status,
        StatusCode::NO_CONTENT
    );
    let again = set_password(&app, &token, "another long passphrase").await;
    assert_error(&again, StatusCode::BAD_REQUEST, "INVALID_SETUP_LINK");
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn an_invitation_link_expires(db: PgPool) {
    let app = with_database(db);
    let admin = signed_in_admin(&app).await;
    invite(&app, &admin, json!({ "email": INVITEE })).await;
    deliver(&app).await;
    let token = token_in(&emails_to(&app, INVITEE)[0]);

    app.clock
        .advance(policy().invitation_link_lifetime + Duration::minutes(1));
    let reply = set_password(&app, &token, INVITEE_PASSWORD).await;
    assert_error(&reply, StatusCode::BAD_REQUEST, "INVALID_SETUP_LINK");
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn links_are_stored_only_as_hashes(db: PgPool) {
    let app = with_database(db.clone());
    let admin = signed_in_admin(&app).await;
    invite(&app, &admin, json!({ "email": INVITEE })).await;
    deliver(&app).await;
    let token = token_in(&emails_to(&app, INVITEE)[0]);

    let (hashed,): (i64,) = sqlx::query_as(
        "SELECT count(*) FROM password_setup_tokens WHERE token_hash = sha256(convert_to($1, 'UTF8'))",
    )
    .bind(&token)
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(hashed, 1);
    // Neither table holds the token in clear, in any column.
    let (leaks,): (i64,) = sqlx::query_as(
        "SELECT (SELECT count(*) FROM password_setup_tokens t WHERE t::text LIKE '%' || $1 || '%') \
              + (SELECT count(*) FROM account_emails e WHERE e::text LIKE '%' || $1 || '%')",
    )
    .bind(&token)
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(leaks, 0);
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn inviting_again_before_acceptance_replaces_the_link(db: PgPool) {
    let app = with_database(db);
    let admin = signed_in_admin(&app).await;
    invite(&app, &admin, json!({ "email": INVITEE })).await;
    deliver(&app).await;
    let first = token_in(&emails_to(&app, INVITEE)[0]);

    let again = invite(&app, &admin, json!({ "email": INVITEE, "isAdmin": true })).await;
    assert_eq!(again.status, StatusCode::CREATED, "{}", again.body);
    assert_eq!(again.body["isAdmin"], true);
    deliver(&app).await;
    let emails = emails_to(&app, INVITEE);
    assert_eq!(emails.len(), 2);

    let old = set_password(&app, &first, INVITEE_PASSWORD).await;
    assert_error(&old, StatusCode::BAD_REQUEST, "INVALID_SETUP_LINK");
    let new = set_password(&app, &token_in(&emails[1]), INVITEE_PASSWORD).await;
    assert_eq!(new.status, StatusCode::NO_CONTENT);
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn inviting_someone_who_has_an_account_is_refused(db: PgPool) {
    let app = with_database(db);
    let admin = signed_in_admin(&app).await;
    let reply = invite(&app, &admin, json!({ "email": ADMIN_EMAIL })).await;
    assert_error(&reply, StatusCode::CONFLICT, "EMAIL_TAKEN");
    assert_eq!(reply.body["fields"]["email"], "already used");
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn only_administrators_invite(db: PgPool) {
    let app = with_database(db);
    let admin = signed_in_admin(&app).await;
    let member = signed_in_member(&app, &admin).await;

    let reply = invite(&app, &member, json!({ "email": "eve@bank.example" })).await;
    assert_error(&reply, StatusCode::FORBIDDEN, "FORBIDDEN");
    let anonymous = send(
        &app.router,
        post(
            &format!("{BASE}/users/invitations"),
            &json!({ "email": "eve@bank.example" }),
            None,
        ),
    )
    .await;
    assert_error(&anonymous, StatusCode::UNAUTHORIZED, "UNAUTHENTICATED");
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn a_malformed_invitation_names_the_field(db: PgPool) {
    let app = with_database(db);
    let admin = signed_in_admin(&app).await;
    let reply = invite(&app, &admin, json!({ "email": "grace" })).await;
    assert_error(&reply, StatusCode::BAD_REQUEST, "INVALID_REQUEST");
    assert!(reply.body["fields"]["email"].is_string());
    let reply = invite(&app, &admin, json!({ "email": INVITEE, "locale": "de" })).await;
    assert_error(&reply, StatusCode::BAD_REQUEST, "INVALID_REQUEST");
}

// --- password reset --------------------------------------------------------------

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn reset_answers_the_same_for_known_and_unknown_emails(db: PgPool) {
    let app = with_database(db);
    admin_with_password(&app).await;

    let known = request_reset(&app, ADMIN_EMAIL).await;
    let unknown = request_reset(&app, "nobody@bank.example").await;
    assert_eq!(known.status, StatusCode::ACCEPTED);
    assert_eq!(unknown.status, StatusCode::ACCEPTED);
    assert_eq!(known.body, unknown.body);
    assert_eq!(known.set_cookie, unknown.set_cookie);

    assert_eq!(deliver(&app).await, 1);
    assert_eq!(emails_to(&app, ADMIN_EMAIL).len(), 1);
    assert!(emails_to(&app, "nobody@bank.example").is_empty());
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn a_reset_link_sets_a_new_password_and_ends_sessions(db: PgPool) {
    let app = with_database(db);
    let session = signed_in_admin(&app).await;
    request_reset(&app, ADMIN_EMAIL).await;
    deliver(&app).await;
    let email = &emails_to(&app, ADMIN_EMAIL)[0];
    assert_eq!(email.subject, "Reset your Donka password");
    assert!(email.text.contains("30 minutes"), "{}", email.text);

    let new_password = "a brand new passphrase";
    assert_eq!(
        set_password(&app, &token_in(email), new_password)
            .await
            .status,
        StatusCode::NO_CONTENT
    );
    let me = send(&app.router, get(&format!("{BASE}/auth/me"), Some(&session))).await;
    assert_error(&me, StatusCode::UNAUTHORIZED, "UNAUTHENTICATED");
    let old = sign_in(&app, ADMIN_EMAIL, ADMIN_PASSWORD).await;
    assert_error(&old, StatusCode::UNAUTHORIZED, "INVALID_CREDENTIALS");
    assert_eq!(
        sign_in(&app, ADMIN_EMAIL, new_password).await.status,
        StatusCode::OK
    );
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn a_reset_link_is_short_lived(db: PgPool) {
    let app = with_database(db);
    admin_with_password(&app).await;
    request_reset(&app, ADMIN_EMAIL).await;
    deliver(&app).await;
    let token = token_in(&emails_to(&app, ADMIN_EMAIL)[0]);

    app.clock
        .advance(policy().reset_link_lifetime + Duration::minutes(1));
    let reply = set_password(&app, &token, "a brand new passphrase").await;
    assert_error(&reply, StatusCode::BAD_REQUEST, "INVALID_SETUP_LINK");
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn repeated_reset_requests_queue_one_email(db: PgPool) {
    let app = with_database(db);
    admin_with_password(&app).await;
    for _ in 0..5 {
        request_reset(&app, ADMIN_EMAIL).await;
    }
    assert_eq!(deliver(&app).await, 1);
    assert_eq!(emails_to(&app, ADMIN_EMAIL).len(), 1);
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn a_reset_request_is_validated_and_needs_the_csrf_header(db: PgPool) {
    let app = with_database(db);
    let malformed = request_reset(&app, "not-an-email").await;
    assert_error(&malformed, StatusCode::BAD_REQUEST, "INVALID_REQUEST");

    let without_csrf = send(
        &app.router,
        axum::http::Request::post(format!("{BASE}/auth/password-reset"))
            .header("content-type", "application/json")
            .body(axum::body::Body::from(
                json!({ "email": ADMIN_EMAIL }).to_string(),
            ))
            .unwrap(),
    )
    .await;
    assert_error(&without_csrf, StatusCode::FORBIDDEN, "CSRF_REQUIRED");
}

// --- delivery ----------------------------------------------------------------------

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn a_failed_send_is_retried_with_backoff_and_leaves_no_working_link(db: PgPool) {
    let app = with_database(db.clone());
    admin_with_password(&app).await;
    request_reset(&app, ADMIN_EMAIL).await;

    app.mailer.set_unreachable(true);
    assert_eq!(deliver(&app).await, 0);
    let (attempts, error): (i32, Option<String>) =
        sqlx::query_as("SELECT attempts, last_error FROM account_emails")
            .fetch_one(&db)
            .await
            .unwrap();
    assert_eq!(attempts, 1);
    assert!(error.is_some_and(|e| e.contains("connection refused")));
    // The link created for the failed attempt was rolled back with it.
    let (live,): (i64,) =
        sqlx::query_as("SELECT count(*) FROM password_setup_tokens WHERE used_at IS NULL")
            .fetch_one(&db)
            .await
            .unwrap();
    assert_eq!(live, 0);

    // Not due yet, even once the server is back.
    app.mailer.set_unreachable(false);
    assert_eq!(deliver(&app).await, 0);
    app.clock.advance(Duration::seconds(31));
    assert_eq!(deliver(&app).await, 1);
    let email = &emails_to(&app, ADMIN_EMAIL)[0];
    assert_eq!(
        set_password(&app, &token_in(email), "a brand new passphrase")
            .await
            .status,
        StatusCode::NO_CONTENT
    );
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn an_email_that_keeps_failing_is_abandoned_and_can_be_requested_again(db: PgPool) {
    let app = with_database(db.clone());
    admin_with_password(&app).await;
    request_reset(&app, ADMIN_EMAIL).await;

    app.mailer.set_unreachable(true);
    for _ in 0..policy().email_max_attempts {
        deliver(&app).await;
        app.clock.advance(Duration::hours(1));
    }
    let (attempts, abandoned): (i32, bool) =
        sqlx::query_as("SELECT attempts, abandoned_at IS NOT NULL FROM account_emails")
            .fetch_one(&db)
            .await
            .unwrap();
    assert_eq!(
        attempts,
        i32::try_from(policy().email_max_attempts).unwrap()
    );
    assert!(abandoned);
    assert_eq!(deliver(&app).await, 0, "no more attempts");

    app.mailer.set_unreachable(false);
    request_reset(&app, ADMIN_EMAIL).await;
    assert_eq!(deliver(&app).await, 1);
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn an_email_the_server_refuses_is_not_retried(db: PgPool) {
    let app = with_database(db.clone());
    admin_with_password(&app).await;
    request_reset(&app, ADMIN_EMAIL).await;

    app.mailer.set_rejecting(true);
    assert_eq!(deliver(&app).await, 0);
    let (attempts, abandoned, error): (i32, bool, String) =
        sqlx::query_as("SELECT attempts, abandoned_at IS NOT NULL, last_error FROM account_emails")
            .fetch_one(&db)
            .await
            .unwrap();
    assert_eq!(attempts, 1);
    assert!(abandoned);
    assert!(error.contains("550"), "{error}");
}

// --- people ------------------------------------------------------------------------

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn administrators_see_every_account_and_whether_it_is_active(db: PgPool) {
    let app = with_database(db);
    let admin = signed_in_admin(&app).await;
    invite(&app, &admin, json!({ "email": INVITEE, "locale": "fr" })).await;
    invite(
        &app,
        &admin,
        json!({ "email": "alan@bank.example", "isAdmin": true }),
    )
    .await;

    let reply = send(&app.router, get(&format!("{BASE}/users"), Some(&admin))).await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.body);
    assert_eq!(reply.body["total"], 3);
    let accounts: Vec<(&str, bool, bool, &str)> = reply.body["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| {
            (
                a["email"].as_str().unwrap(),
                a["active"].as_bool().unwrap(),
                a["isAdmin"].as_bool().unwrap(),
                a["locale"].as_str().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        accounts,
        [
            (ADMIN_EMAIL, true, true, "en"),
            ("alan@bank.example", false, true, "en"),
            (INVITEE, false, false, "fr"),
        ]
    );

    let page = send(
        &app.router,
        get(&format!("{BASE}/users?limit=1&offset=1"), Some(&admin)),
    )
    .await;
    assert_eq!(page.body["total"], 3);
    assert_eq!(page.body["items"][0]["email"], "alan@bank.example");
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn only_administrators_see_accounts(db: PgPool) {
    let app = with_database(db);
    let admin = signed_in_admin(&app).await;
    let member = signed_in_member(&app, &admin).await;
    let reply = send(&app.router, get(&format!("{BASE}/users"), Some(&member))).await;
    assert_error(&reply, StatusCode::FORBIDDEN, "FORBIDDEN");
    let anonymous = send(&app.router, get(&format!("{BASE}/users"), None)).await;
    assert_error(&anonymous, StatusCode::UNAUTHORIZED, "UNAUTHENTICATED");
}
