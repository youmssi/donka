#![allow(clippy::unwrap_used)] // tests fail loudly on purpose

mod support;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use chrono::Duration;
use donka_db::PgPool;
use donka_identity::Locale;
use serde_json::json;
use support::*;
use tower::ServiceExt;

// --- first administrator and password setup ----------------------------------

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn the_first_administrator_is_created_only_on_an_empty_database(db: PgPool) {
    let app = with_database(db);
    assert!(app
        .identity
        .bootstrap_admin(ADMIN_EMAIL, Locale::En)
        .await
        .unwrap()
        .is_some());
    assert!(app
        .identity
        .bootstrap_admin("eve@bank.example", Locale::En)
        .await
        .unwrap()
        .is_none());
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn a_setup_link_works_once(db: PgPool) {
    let app = with_database(db);
    let token = app
        .identity
        .bootstrap_admin(ADMIN_EMAIL, Locale::En)
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
        .bootstrap_admin(ADMIN_EMAIL, Locale::En)
        .await
        .unwrap()
        .unwrap();
    app.clock
        .advance(policy().invitation_link_lifetime + Duration::seconds(1));

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
        .bootstrap_admin(ADMIN_EMAIL, Locale::En)
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
    app.identity
        .bootstrap_admin(ADMIN_EMAIL, Locale::En)
        .await
        .unwrap();
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

// --- rate limits (DNK-32) ----------------------------------------------------

/// A request as it arrives from `peer`, optionally through a proxy's `X-Forwarded-For`.
fn from(
    peer: &str,
    forwarded_for: Option<&str>,
    path: &str,
    body: &serde_json::Value,
) -> Request<Body> {
    let mut req = post(path, body, None);
    let addr: std::net::SocketAddr = format!("{peer}:40000").parse().unwrap();
    req.extensions_mut()
        .insert(axum::extract::ConnectInfo(addr));
    if let Some(value) = forwarded_for {
        req.headers_mut()
            .insert("x-forwarded-for", value.parse().unwrap());
    }
    req
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn sign_in_is_limited_per_client_address(db: PgPool) {
    let app = with_auth_limits(
        db,
        donka_app::rate_limit::AuthLimits::new(2, 100, Vec::new()),
    );
    admin_with_password(&app).await;
    let path = format!("{BASE}/auth/sign-in");
    // Wrong passwords against different accounts: the limit counts the address, not the account.
    for email in ["ada@bank.example", "bob@bank.example"] {
        let body = json!({ "email": email, "password": "wrong" });
        let reply = send(&app.router, from("203.0.113.7", None, &path, &body)).await;
        assert_eq!(reply.status, StatusCode::UNAUTHORIZED);
    }

    // Even the right password is refused while the address is over its limit.
    let right = json!({ "email": ADMIN_EMAIL, "password": ADMIN_PASSWORD });
    let res = app
        .router
        .clone()
        .oneshot(from("203.0.113.7", None, &path, &right))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::TOO_MANY_REQUESTS);
    let retry_after: u64 = res.headers()["retry-after"]
        .to_str()
        .unwrap()
        .parse()
        .unwrap();
    assert!((1..=30).contains(&retry_after), "{retry_after}");
    let reply = send(&app.router, from("203.0.113.7", None, &path, &right)).await;
    assert_eq!(reply.body["code"], "RATE_LIMITED");
    assert!(reply.body["details"]["retryAfterSeconds"].as_u64().unwrap() >= 1);
    assert!(reply.set_cookie.is_none());

    // Another address is not affected.
    let reply = send(&app.router, from("198.51.100.4", None, &path, &right)).await;
    assert_eq!(reply.status, StatusCode::OK);
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn password_reset_is_limited_per_client_address(db: PgPool) {
    let app = with_auth_limits(
        db,
        donka_app::rate_limit::AuthLimits::new(100, 1, Vec::new()),
    );
    admin_with_password(&app).await;
    let path = format!("{BASE}/auth/password-reset");
    let body = json!({ "email": ADMIN_EMAIL });
    let first = send(&app.router, from("203.0.113.7", None, &path, &body)).await;
    assert_eq!(first.status, StatusCode::ACCEPTED);
    let second = send(&app.router, from("203.0.113.7", None, &path, &body)).await;
    assert_eq!(second.status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(second.body["code"], "RATE_LIMITED");
}

#[sqlx::test(migrator = "donka_db::MIGRATOR")]
async fn forwarded_for_counts_only_through_a_trusted_proxy(db: PgPool) {
    let proxy = "10.0.0.2/32".parse().unwrap();
    let app = with_auth_limits(
        db,
        donka_app::rate_limit::AuthLimits::new(1, 100, vec![proxy]),
    );
    let path = format!("{BASE}/auth/sign-in");
    let body = json!({ "email": "nobody@bank.example", "password": "wrong" });

    // Behind the proxy, two clients each have their own budget.
    for client in ["203.0.113.7", "203.0.113.8"] {
        let reply = send(&app.router, from("10.0.0.2", Some(client), &path, &body)).await;
        assert_eq!(reply.status, StatusCode::UNAUTHORIZED, "{client}");
    }
    let again = send(
        &app.router,
        from("10.0.0.2", Some("203.0.113.7"), &path, &body),
    )
    .await;
    assert_eq!(again.status, StatusCode::TOO_MANY_REQUESTS);

    // A client that is not the proxy cannot pick its address with the header.
    let direct = send(
        &app.router,
        from("192.0.2.1", Some("203.0.113.9"), &path, &body),
    )
    .await;
    assert_eq!(direct.status, StatusCode::UNAUTHORIZED);
    let forged = send(
        &app.router,
        from("192.0.2.1", Some("203.0.113.10"), &path, &body),
    )
    .await;
    assert_eq!(forged.status, StatusCode::TOO_MANY_REQUESTS);
}
