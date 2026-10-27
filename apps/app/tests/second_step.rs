#![allow(clippy::unwrap_used)] // tests fail loudly on purpose

//! Two-step sign-in with an authenticator app (DNK-59).

mod support;

use axum::http::StatusCode;
use chrono::Duration;
use donka_app::config::MfaRequired;
use donka_db::PgPool;
use serde_json::{json, Value};
use support::*;

const EDITOR: &str = "grace@bank.example";
/// The password `signed_in_user` gives invited people.
const USER_PASSWORD: &str = "a long enough passphrase";
/// A second key, for rotation.
const NEW_KEY: &str = "QEFCQ0RFRkdISUpLTE1OT1BRUlNUVVZXWFlaW1xdXl8=";

fn path(rest: &str) -> String {
    format!("{BASE}{rest}")
}

/// Turns the second step on for the signed-in person; returns the key and the
/// recovery codes. Moves the clock to the next 30-second step afterwards, so the
/// next code differs from the one just used.
async fn enrol(app: &TestApp, session: &str) -> (String, Vec<String>) {
    let started = send(
        &app.router,
        post(
            &path("/account/second-step/enrolment"),
            &json!({}),
            Some(session),
        ),
    )
    .await;
    assert_eq!(started.status, StatusCode::OK, "{}", started.body);
    let key = started.body["key"].as_str().unwrap().to_owned();
    assert!(started.body["otpauthUrl"]
        .as_str()
        .unwrap()
        .starts_with("otpauth://totp/Donka:"));
    let confirmed = send(
        &app.router,
        post(
            &path("/account/second-step/enrolment/confirm"),
            &json!({ "code": code_at(app, &key) }),
            Some(session),
        ),
    )
    .await;
    assert_eq!(confirmed.status, StatusCode::OK, "{}", confirmed.body);
    let codes = recovery_codes(&confirmed.body["recoveryCodes"]);
    app.clock.advance(Duration::seconds(30));
    (key, codes)
}

fn recovery_codes(value: &Value) -> Vec<String> {
    value
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c.as_str().unwrap().to_owned())
        .collect()
}

/// Password step that must answer 202 with `next`; returns the pending token.
async fn password_step(app: &TestApp, email: &str, next: &str) -> String {
    let password = if email == ADMIN_EMAIL {
        ADMIN_PASSWORD
    } else {
        USER_PASSWORD
    };
    let reply = sign_in(app, email, password).await;
    assert_eq!(reply.status, StatusCode::ACCEPTED, "{}", reply.body);
    assert_eq!(reply.body["next"], next);
    let cookie = reply.set_cookie.as_deref().unwrap();
    assert!(
        !cookie.starts_with("donka_session="),
        "no session after the password alone"
    );
    assert!(cookie.contains("HttpOnly") && cookie.contains("SameSite=Strict"));
    pending_from(&reply)
}

async fn code_step(app: &TestApp, pending: &str, code: &str) -> Reply {
    send(
        &app.router,
        post_pending(
            &path("/auth/sign-in/code"),
            &json!({ "code": code }),
            pending,
        ),
    )
    .await
}

async fn actions(db: &PgPool) -> Vec<String> {
    sqlx::query_scalar("SELECT action FROM audit_events ORDER BY occurred_at, id")
        .fetch_all(db)
        .await
        .unwrap()
}

// --- enrolment ----------------------------------------------------------------

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn enrolment_needs_a_valid_code_and_shows_recovery_codes_once(db: PgPool) {
    let app = with_database(db.clone());
    let session = signed_in_admin(&app).await;
    let started = send(
        &app.router,
        post(
            &path("/account/second-step/enrolment"),
            &json!({}),
            Some(&session),
        ),
    )
    .await;
    let key = started.body["key"].as_str().unwrap().to_owned();

    let wrong = send(
        &app.router,
        post(
            &path("/account/second-step/enrolment/confirm"),
            &json!({ "code": "000000" }),
            Some(&session),
        ),
    )
    .await;
    assert_error(&wrong, StatusCode::BAD_REQUEST, "INVALID_CODE");
    let status = send(
        &app.router,
        get(&path("/account/second-step"), Some(&session)),
    )
    .await;
    assert_eq!(status.body["enabled"], false, "not on before a valid code");

    let confirmed = send(
        &app.router,
        post(
            &path("/account/second-step/enrolment/confirm"),
            &json!({ "code": code_at(&app, &key) }),
            Some(&session),
        ),
    )
    .await;
    assert_eq!(confirmed.status, StatusCode::OK, "{}", confirmed.body);
    let codes = recovery_codes(&confirmed.body["recoveryCodes"]);
    assert_eq!(codes.len(), 10);

    let status = send(
        &app.router,
        get(&path("/account/second-step"), Some(&session)),
    )
    .await;
    assert_eq!(
        status.body,
        json!({ "enabled": true, "required": false, "recoveryCodesLeft": 10 })
    );
    // The key is never shown again, and nothing stores it or the codes in clear.
    let again = send(
        &app.router,
        post(
            &path("/account/second-step/enrolment"),
            &json!({}),
            Some(&session),
        ),
    )
    .await;
    assert_error(&again, StatusCode::CONFLICT, "SECOND_STEP_ALREADY_ON");
    let stored: (Vec<u8>,) = sqlx::query_as("SELECT ciphertext FROM second_steps")
        .fetch_one(&db)
        .await
        .unwrap();
    assert!(!String::from_utf8_lossy(&stored.0).contains(&key));
    let dump: String = sqlx::query_scalar(
        "SELECT string_agg(encode(code_hash, 'escape'), '') FROM recovery_codes",
    )
    .fetch_one(&db)
    .await
    .unwrap();
    assert!(!dump.contains(&codes[0]));

    assert!(actions(&db)
        .await
        .contains(&"user.second_step_enabled".to_owned()));
    deliver(&app).await;
    let notice = emails_to(&app, ADMIN_EMAIL).pop().unwrap();
    assert_eq!(notice.subject, "Two-step sign-in is on");
    assert!(!notice.text.contains("http"));
}

// --- sign-in ------------------------------------------------------------------

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn sign_in_asks_for_the_code_after_the_password(db: PgPool) {
    let app = with_database(db);
    let session = signed_in_admin(&app).await;
    let (key, _) = enrol(&app, &session).await;

    let pending = password_step(&app, ADMIN_EMAIL, "code").await;
    let me = send(&app.router, get(&path("/auth/me"), None)).await;
    assert_error(&me, StatusCode::UNAUTHORIZED, "UNAUTHENTICATED");

    let code = code_at(&app, &key);
    let reply = code_step(&app, &pending, &code).await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.body);
    assert_eq!(reply.body["email"], ADMIN_EMAIL);
    let new_session = session_from(&reply);
    let me = send(&app.router, get(&path("/auth/me"), Some(&new_session))).await;
    assert_eq!(me.status, StatusCode::OK);

    // The pending step is single-use...
    let reused = code_step(&app, &pending, &code_at(&app, &key)).await;
    assert_error(&reused, StatusCode::UNAUTHORIZED, "SIGN_IN_EXPIRED");
    // ...and so is a code, even with a fresh password step.
    let pending = password_step(&app, ADMIN_EMAIL, "code").await;
    let replayed = code_step(&app, &pending, &code).await;
    assert_error(&replayed, StatusCode::UNAUTHORIZED, "INVALID_CREDENTIALS");
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn the_password_step_expires(db: PgPool) {
    let app = with_database(db);
    let session = signed_in_admin(&app).await;
    let (key, _) = enrol(&app, &session).await;
    let pending = password_step(&app, ADMIN_EMAIL, "code").await;
    app.clock.advance(Duration::minutes(6));
    let late = code_step(&app, &pending, &code_at(&app, &key)).await;
    assert_error(&late, StatusCode::UNAUTHORIZED, "SIGN_IN_EXPIRED");

    let missing = send(
        &app.router,
        post(
            &path("/auth/sign-in/code"),
            &json!({ "code": "123456" }),
            None,
        ),
    )
    .await;
    assert_error(&missing, StatusCode::UNAUTHORIZED, "SIGN_IN_EXPIRED");
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn wrong_codes_count_towards_the_lockout(db: PgPool) {
    let app = with_database(db);
    let session = signed_in_admin(&app).await;
    let (key, _) = enrol(&app, &session).await;
    let pending = password_step(&app, ADMIN_EMAIL, "code").await;
    for _ in 0..policy().max_failed_sign_ins {
        let wrong = code_step(&app, &pending, "000000").await;
        assert_error(&wrong, StatusCode::UNAUTHORIZED, "INVALID_CREDENTIALS");
    }
    // Locked: a right code gets the same answer as a wrong one, and so does the password.
    let right = code_step(&app, &pending, &code_at(&app, &key)).await;
    assert_error(&right, StatusCode::UNAUTHORIZED, "INVALID_CREDENTIALS");
    let password = sign_in(&app, ADMIN_EMAIL, ADMIN_PASSWORD).await;
    assert_error(&password, StatusCode::UNAUTHORIZED, "INVALID_CREDENTIALS");

    app.clock
        .advance(policy().lock_duration + Duration::seconds(1));
    let pending = password_step(&app, ADMIN_EMAIL, "code").await;
    let reply = code_step(&app, &pending, &code_at(&app, &key)).await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.body);
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn a_recovery_code_signs_in_once_and_new_codes_replace_old_ones(db: PgPool) {
    let app = with_database(db.clone());
    let session = signed_in_admin(&app).await;
    let (key, codes) = enrol(&app, &session).await;

    let pending = password_step(&app, ADMIN_EMAIL, "code").await;
    let loose = codes[0].to_lowercase().replace('-', " ");
    let reply = code_step(&app, &pending, &loose).await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.body);
    let pending = password_step(&app, ADMIN_EMAIL, "code").await;
    let again = code_step(&app, &pending, &codes[0]).await;
    assert_error(&again, StatusCode::UNAUTHORIZED, "INVALID_CREDENTIALS");

    deliver(&app).await;
    assert!(emails_to(&app, ADMIN_EMAIL)
        .iter()
        .any(|e| e.subject == "A recovery code was used"));
    assert!(actions(&db)
        .await
        .contains(&"user.recovery_code_used".to_owned()));

    let wrong = send(
        &app.router,
        post(
            &path("/account/second-step/recovery-codes"),
            &json!({ "code": codes[1] }),
            Some(&session),
        ),
    )
    .await;
    assert_error(&wrong, StatusCode::BAD_REQUEST, "INVALID_CODE");
    app.clock.advance(Duration::seconds(30));
    let fresh = send(
        &app.router,
        post(
            &path("/account/second-step/recovery-codes"),
            &json!({ "code": code_at(&app, &key) }),
            Some(&session),
        ),
    )
    .await;
    assert_eq!(fresh.status, StatusCode::OK, "{}", fresh.body);
    let fresh = recovery_codes(&fresh.body["recoveryCodes"]);
    assert_eq!(fresh.len(), 10);

    let pending = password_step(&app, ADMIN_EMAIL, "code").await;
    let old = code_step(&app, &pending, &codes[1]).await;
    assert_error(&old, StatusCode::UNAUTHORIZED, "INVALID_CREDENTIALS");
    let new = code_step(&app, &pending, &fresh[0]).await;
    assert_eq!(new.status, StatusCode::OK, "{}", new.body);
}

// --- policy ---------------------------------------------------------------------

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn without_a_policy_sign_in_is_unchanged(db: PgPool) {
    let app = with_database(db);
    admin_with_password(&app).await;
    let reply = sign_in(&app, ADMIN_EMAIL, ADMIN_PASSWORD).await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.body);
    session_from(&reply);
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn policy_makes_privileged_people_enrol_before_anything_opens(db: PgPool) {
    let app = with_second_step(db.clone(), MfaRequired::Privileged, &[SECOND_STEP_KEY]);
    admin_with_password(&app).await;
    let pending = password_step(&app, ADMIN_EMAIL, "enrol").await;
    // The enrolment token does not accept codes.
    let early = code_step(&app, &pending, "123456").await;
    assert_error(&early, StatusCode::UNAUTHORIZED, "SIGN_IN_EXPIRED");

    let started = send(
        &app.router,
        post_pending(&path("/auth/sign-in/enrolment"), &json!({}), &pending),
    )
    .await;
    assert_eq!(started.status, StatusCode::OK, "{}", started.body);
    let key = started.body["key"].as_str().unwrap().to_owned();
    let confirmed = send(
        &app.router,
        post_pending(
            &path("/auth/sign-in/enrolment/confirm"),
            &json!({ "code": code_at(&app, &key) }),
            &pending,
        ),
    )
    .await;
    assert_eq!(confirmed.status, StatusCode::OK, "{}", confirmed.body);
    assert_eq!(confirmed.body["user"]["email"], ADMIN_EMAIL);
    assert_eq!(recovery_codes(&confirmed.body["recoveryCodes"]).len(), 10);
    let session = session_from(&confirmed);

    // A person who is neither an administrator nor an owner signs in as before...
    let editor = signed_in_user(&app, EDITOR, false).await;
    assert!(!editor.is_empty());
    // ...until they own a project.
    let project = send(
        &app.router,
        post(
            &path("/projects"),
            &json!({ "key": "credit", "name": "Credit" }),
            Some(&session),
        ),
    )
    .await;
    assert_eq!(project.status, StatusCode::CREATED, "{}", project.body);
    let id = project.body["id"].as_str().unwrap();
    let added = send(
        &app.router,
        post(
            &path(&format!("/projects/{id}/members")),
            &json!({ "email": EDITOR, "role": "owner" }),
            Some(&session),
        ),
    )
    .await;
    assert_eq!(added.status, StatusCode::CREATED, "{}", added.body);
    password_step(&app, EDITOR, "enrol").await;

    // Required people cannot turn their second step off.
    app.clock.advance(Duration::seconds(30));
    let off = send(
        &app.router,
        post(
            &path("/account/second-step/disable"),
            &json!({ "code": code_at(&app, &key) }),
            Some(&session),
        ),
    )
    .await;
    assert_error(&off, StatusCode::CONFLICT, "SECOND_STEP_REQUIRED");
    let status = send(
        &app.router,
        get(&path("/account/second-step"), Some(&session)),
    )
    .await;
    assert_eq!(status.body["required"], true);
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn policy_all_covers_everyone(db: PgPool) {
    let app = with_second_step(db, MfaRequired::All, &[SECOND_STEP_KEY]);
    admin_with_password(&app).await;
    let admin = app
        .identity
        .find_by_email(ADMIN_EMAIL)
        .await
        .unwrap()
        .unwrap();
    app.identity
        .invite(&admin, EDITOR, donka_identity::Locale::En, false)
        .await
        .unwrap();
    deliver(&app).await;
    let token = token_in(emails_to(&app, EDITOR).last().unwrap());
    app.identity
        .complete_password_setup(&token, USER_PASSWORD)
        .await
        .unwrap();
    password_step(&app, EDITOR, "enrol").await;
}

// --- turning off and resetting ----------------------------------------------------

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn a_person_turns_their_second_step_off_with_a_code(db: PgPool) {
    let app = with_database(db.clone());
    let session = signed_in_admin(&app).await;
    let (_, codes) = enrol(&app, &session).await;
    let wrong = send(
        &app.router,
        post(
            &path("/account/second-step/disable"),
            &json!({ "code": "000000" }),
            Some(&session),
        ),
    )
    .await;
    assert_error(&wrong, StatusCode::BAD_REQUEST, "INVALID_CODE");
    let off = send(
        &app.router,
        post(
            &path("/account/second-step/disable"),
            &json!({ "code": codes[0] }),
            Some(&session),
        ),
    )
    .await;
    assert_eq!(off.status, StatusCode::NO_CONTENT, "{}", off.body);
    let reply = sign_in(&app, ADMIN_EMAIL, ADMIN_PASSWORD).await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.body);
    assert!(actions(&db)
        .await
        .contains(&"user.second_step_removed".to_owned()));
    deliver(&app).await;
    assert!(emails_to(&app, ADMIN_EMAIL)
        .iter()
        .any(|e| e.subject == "Two-step sign-in is off"));
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn another_administrator_resets_a_lost_device(db: PgPool) {
    let app = with_database(db.clone());
    let admin = signed_in_admin(&app).await;
    let editor = signed_in_user(&app, EDITOR, false).await;
    enrol(&app, &editor).await;
    let editor_id: uuid::Uuid = sqlx::query_scalar("SELECT id FROM users WHERE email = $1")
        .bind(EDITOR)
        .fetch_one(&db)
        .await
        .unwrap();
    let reset = path(&format!("/users/{editor_id}/second-step/reset"));

    let people = send(&app.router, get(&path("/users"), Some(&admin))).await;
    let row = people.body["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["email"] == EDITOR)
        .unwrap()
        .clone();
    assert_eq!(row["secondStep"], true);

    let by_editor = send(&app.router, post(&reset, &json!({}), Some(&editor))).await;
    assert_error(&by_editor, StatusCode::FORBIDDEN, "FORBIDDEN");
    let admin_id: uuid::Uuid = sqlx::query_scalar("SELECT id FROM users WHERE email = $1")
        .bind(ADMIN_EMAIL)
        .fetch_one(&db)
        .await
        .unwrap();
    let own = send(
        &app.router,
        post(
            &path(&format!("/users/{admin_id}/second-step/reset")),
            &json!({}),
            Some(&admin),
        ),
    )
    .await;
    assert_error(&own, StatusCode::FORBIDDEN, "FORBIDDEN");

    let done = send(&app.router, post(&reset, &json!({}), Some(&admin))).await;
    assert_eq!(done.status, StatusCode::NO_CONTENT, "{}", done.body);
    let me = send(&app.router, get(&path("/auth/me"), Some(&editor))).await;
    assert_error(&me, StatusCode::UNAUTHORIZED, "UNAUTHENTICATED");
    let reply = sign_in(&app, EDITOR, USER_PASSWORD).await;
    assert_eq!(
        reply.status,
        StatusCode::OK,
        "no second step after the reset"
    );
    let twice = send(&app.router, post(&reset, &json!({}), Some(&admin))).await;
    assert_error(&twice, StatusCode::CONFLICT, "SECOND_STEP_OFF");

    let (action, actor): (String, uuid::Uuid) = sqlx::query_as(
        "SELECT action, actor_id FROM audit_events WHERE action = 'user.second_step_reset'",
    )
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(
        (action.as_str(), actor),
        ("user.second_step_reset", admin_id)
    );
    deliver(&app).await;
    assert!(emails_to(&app, EDITOR)
        .iter()
        .any(|e| e.subject == "Your two-step sign-in was reset"));
}

// --- keys -------------------------------------------------------------------------

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn secrets_survive_a_key_rotation(db: PgPool) {
    let before = with_database(db.clone());
    let session = signed_in_admin(&before).await;
    let (key, _) = enrol(&before, &session).await;

    let rotated = with_second_step(db.clone(), MfaRequired::None, &[NEW_KEY, SECOND_STEP_KEY]);
    rotated.clock.advance(Duration::minutes(1));
    let pending = password_step(&rotated, ADMIN_EMAIL, "code").await;
    let reply = code_step(&rotated, &pending, &code_at(&rotated, &key)).await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.body);

    // Re-sealed at that use: the previous key can now go.
    let only_new = with_second_step(db, MfaRequired::None, &[NEW_KEY]);
    only_new.clock.advance(Duration::minutes(2));
    let pending = password_step(&only_new, ADMIN_EMAIL, "code").await;
    let reply = code_step(&only_new, &pending, &code_at(&only_new, &key)).await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.body);
}
