#![allow(clippy::unwrap_used)] // tests fail loudly on purpose

mod support;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use chrono::Duration;
use donka_db::PgPool;
use serde_json::json;
use support::*;

// --- first administrator and password setup ----------------------------------

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn the_first_administrator_is_created_only_on_an_empty_database(db: PgPool) {
    let app = with_database(db);
    assert!(app
        .identity
        .bootstrap_admin(ADMIN_EMAIL)
        .await
        .unwrap()
        .is_some());
    assert!(app
        .identity
        .bootstrap_admin("eve@bank.example")
        .await
        .unwrap()
        .is_none());
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn a_setup_link_works_once(db: PgPool) {
    let app = with_database(db);
    let token = app
        .identity
        .bootstrap_admin(ADMIN_EMAIL)
        .await
        .unwrap()
        .unwrap();
    let body = json!({ "token": token.expose(), "password": ADMIN_PASSWORD });

    let first = send(
        &app.router,
        post("/api/v1/auth/password-setup", &body, None),
    )
    .await;
    assert_eq!(first.status, StatusCode::NO_CONTENT);
    let second = send(
        &app.router,
        post("/api/v1/auth/password-setup", &body, None),
    )
    .await;
    assert_error(&second, StatusCode::BAD_REQUEST, "INVALID_SETUP_LINK");
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn a_setup_link_expires(db: PgPool) {
    let app = with_database(db);
    let token = app
        .identity
        .bootstrap_admin(ADMIN_EMAIL)
        .await
        .unwrap()
        .unwrap();
    app.clock
        .advance(Duration::hours(24) + Duration::seconds(1));

    let body = json!({ "token": token.expose(), "password": ADMIN_PASSWORD });
    let reply = send(
        &app.router,
        post("/api/v1/auth/password-setup", &body, None),
    )
    .await;
    assert_error(&reply, StatusCode::BAD_REQUEST, "INVALID_SETUP_LINK");
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn a_short_password_is_refused_and_the_link_stays_usable(db: PgPool) {
    let app = with_database(db);
    let token = app
        .identity
        .bootstrap_admin(ADMIN_EMAIL)
        .await
        .unwrap()
        .unwrap();

    let short = json!({ "token": token.expose(), "password": "short" });
    let reply = send(
        &app.router,
        post("/api/v1/auth/password-setup", &short, None),
    )
    .await;
    assert_error(&reply, StatusCode::BAD_REQUEST, "INVALID_REQUEST");
    assert!(reply.body["fields"]["password"].is_string());

    let good = json!({ "token": token.expose(), "password": ADMIN_PASSWORD });
    let reply = send(
        &app.router,
        post("/api/v1/auth/password-setup", &good, None),
    )
    .await;
    assert_eq!(reply.status, StatusCode::NO_CONTENT);
}

// --- sign in ----------------------------------------------------------------

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn signing_in_sets_a_secure_http_only_cookie(db: PgPool) {
    let app = with_database(db);
    admin_with_password(&app).await;
    let reply = sign_in(&app, "  ADA@bank.example ", ADMIN_PASSWORD).await;

    assert_eq!(reply.status, StatusCode::OK);
    assert_eq!(reply.body["email"], ADMIN_EMAIL);
    assert_eq!(reply.body["isAdmin"], true);
    let cookie = reply.set_cookie.unwrap();
    for attribute in ["HttpOnly", "Secure", "SameSite=Lax", "Path=/"] {
        assert!(
            cookie.contains(attribute),
            "{attribute} missing from {cookie}"
        );
    }
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn wrong_password_and_unknown_email_look_the_same(db: PgPool) {
    let app = with_database(db);
    admin_with_password(&app).await;

    let wrong = sign_in(&app, ADMIN_EMAIL, "not the right password").await;
    let unknown = sign_in(&app, "nobody@bank.example", "not the right password").await;
    let malformed = sign_in(&app, "not-an-email", "not the right password").await;
    for reply in [&wrong, &unknown, &malformed] {
        assert_error(reply, StatusCode::UNAUTHORIZED, "INVALID_CREDENTIALS");
        assert!(reply.set_cookie.is_none());
    }
    assert_eq!(wrong.body["message"], unknown.body["message"]);
    assert_eq!(wrong.body["message"], malformed.body["message"]);
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn an_account_without_a_password_cannot_sign_in(db: PgPool) {
    let app = with_database(db);
    app.identity.bootstrap_admin(ADMIN_EMAIL).await.unwrap();
    let reply = sign_in(&app, ADMIN_EMAIL, "").await;
    assert_error(&reply, StatusCode::UNAUTHORIZED, "INVALID_CREDENTIALS");
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn five_failures_lock_the_account_for_fifteen_minutes(db: PgPool) {
    let app = with_database(db);
    admin_with_password(&app).await;
    for _ in 0..5 {
        let reply = sign_in(&app, ADMIN_EMAIL, "not the right password").await;
        assert_eq!(reply.status, StatusCode::UNAUTHORIZED);
    }

    // Locked: the right password is refused, with the same answer as a wrong one.
    let locked = sign_in(&app, ADMIN_EMAIL, ADMIN_PASSWORD).await;
    assert_error(&locked, StatusCode::UNAUTHORIZED, "INVALID_CREDENTIALS");

    app.clock.advance(Duration::minutes(14));
    assert_eq!(
        sign_in(&app, ADMIN_EMAIL, ADMIN_PASSWORD).await.status,
        StatusCode::UNAUTHORIZED
    );

    app.clock
        .advance(Duration::minutes(1) + Duration::seconds(1));
    assert_eq!(
        sign_in(&app, ADMIN_EMAIL, ADMIN_PASSWORD).await.status,
        StatusCode::OK
    );
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn failures_spread_over_more_than_the_window_do_not_lock(db: PgPool) {
    let app = with_database(db);
    admin_with_password(&app).await;
    for _ in 0..4 {
        sign_in(&app, ADMIN_EMAIL, "not the right password").await;
    }
    app.clock.advance(Duration::minutes(16));
    sign_in(&app, ADMIN_EMAIL, "not the right password").await;
    assert_eq!(
        sign_in(&app, ADMIN_EMAIL, ADMIN_PASSWORD).await.status,
        StatusCode::OK
    );
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn concurrent_failures_are_all_counted(db: PgPool) {
    let app = with_database(db);
    admin_with_password(&app).await;

    // Five wrong passwords at the same moment: the row lock must count every one.
    let mut attempts = tokio::task::JoinSet::new();
    for _ in 0..5 {
        let router = app.router.clone();
        attempts.spawn(async move {
            let body = json!({ "email": ADMIN_EMAIL, "password": "not the right password" });
            send(&router, post("/api/v1/auth/sign-in", &body, None))
                .await
                .status
        });
    }
    while let Some(status) = attempts.join_next().await {
        assert_eq!(status.unwrap(), StatusCode::UNAUTHORIZED);
    }

    let locked = sign_in(&app, ADMIN_EMAIL, ADMIN_PASSWORD).await;
    assert_error(&locked, StatusCode::UNAUTHORIZED, "INVALID_CREDENTIALS");
}

// --- sessions -----------------------------------------------------------------

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn protected_routes_need_a_session(db: PgPool) {
    let app = with_database(db);
    admin_with_password(&app).await;

    let me = send(&app.router, get("/api/v1/auth/me", None)).await;
    assert_error(&me, StatusCode::UNAUTHORIZED, "UNAUTHENTICATED");
    let forged = send(&app.router, get("/api/v1/auth/me", Some("forged-token"))).await;
    assert_error(&forged, StatusCode::UNAUTHORIZED, "UNAUTHENTICATED");
    let simulate = send(&app.router, post("/api/v1/simulate", &json!({}), None)).await;
    assert_error(&simulate, StatusCode::UNAUTHORIZED, "UNAUTHENTICATED");
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn me_returns_the_user_without_any_secret(db: PgPool) {
    let app = with_database(db);
    let session = signed_in_admin(&app).await;
    let reply = send(&app.router, get("/api/v1/auth/me", Some(&session))).await;

    assert_eq!(reply.status, StatusCode::OK);
    assert_eq!(reply.body["email"], ADMIN_EMAIL);
    let raw = reply.body.to_string();
    assert!(
        !raw.contains("argon2") && !raw.contains("password"),
        "{raw}"
    );
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn signing_out_ends_the_session_on_the_server(db: PgPool) {
    let app = with_database(db);
    let session = signed_in_admin(&app).await;

    let out = send(
        &app.router,
        post("/api/v1/auth/sign-out", &json!({}), Some(&session)),
    )
    .await;
    assert_eq!(out.status, StatusCode::NO_CONTENT);
    assert!(out.set_cookie.unwrap().contains("Max-Age=0"));

    // The old cookie value no longer works, even if the browser kept it.
    let reply = send(&app.router, get("/api/v1/auth/me", Some(&session))).await;
    assert_error(&reply, StatusCode::UNAUTHORIZED, "UNAUTHENTICATED");
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn a_session_ends_after_eight_idle_hours(db: PgPool) {
    let app = with_database(db);
    let session = signed_in_admin(&app).await;

    app.clock.advance(Duration::hours(7));
    let active = send(&app.router, get("/api/v1/auth/me", Some(&session))).await;
    assert_eq!(
        active.status,
        StatusCode::OK,
        "activity keeps the session alive"
    );

    app.clock.advance(Duration::hours(7));
    let still = send(&app.router, get("/api/v1/auth/me", Some(&session))).await;
    assert_eq!(still.status, StatusCode::OK, "7h idle is within the limit");

    app.clock.advance(Duration::hours(8) + Duration::seconds(1));
    let expired = send(&app.router, get("/api/v1/auth/me", Some(&session))).await;
    assert_error(&expired, StatusCode::UNAUTHORIZED, "UNAUTHENTICATED");
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn setting_a_new_password_ends_existing_sessions(db: PgPool) {
    let app = with_database(db.clone());
    let session = signed_in_admin(&app).await;
    // An administrator issues a new setup link (DNK-5 adds the endpoint); simulate it here.
    let (user_id,): (uuid::Uuid,) = sqlx::query_as("SELECT id FROM users")
        .fetch_one(&db)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO password_setup_tokens (token_hash, user_id, created_at, expires_at) \
         VALUES (sha256('reset-token'::bytea), $1, now(), now() + interval '1 day')",
    )
    .bind(user_id)
    .execute(&db)
    .await
    .unwrap();
    app.identity
        .complete_password_setup("reset-token", "another long passphrase")
        .await
        .unwrap();

    let reply = send(&app.router, get("/api/v1/auth/me", Some(&session))).await;
    assert_error(&reply, StatusCode::UNAUTHORIZED, "UNAUTHENTICATED");
}

// --- CSRF -------------------------------------------------------------------

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn state_changing_requests_without_the_csrf_header_are_refused(db: PgPool) {
    let app = with_database(db);
    admin_with_password(&app).await;
    let req = Request::post("/api/v1/auth/sign-in")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({ "email": ADMIN_EMAIL, "password": ADMIN_PASSWORD }).to_string(),
        ))
        .unwrap();
    let reply = send(&app.router, req).await;

    assert_error(&reply, StatusCode::FORBIDDEN, "CSRF_REQUIRED");
    assert!(reply.set_cookie.is_none());
}
