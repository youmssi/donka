//! Shared helpers for the HTTP tests.
#![allow(clippy::unwrap_used, dead_code)] // each test binary uses a subset

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use axum::Router;
use chrono::{TimeZone, Utc};
use donka_app::auth::{CookieSettings, CSRF_HEADER, SESSION_COOKIE};
use donka_app::{router, AppState};
use donka_audit::AuditLog;
use donka_db::{DbOptions, PgPool};
use donka_decision::Decisions;
use donka_engine::{DecisionRuntime, ZenRuntime};
use donka_identity::{Identity, Locale, Policy};
use donka_mail::testing::RecordingMailer;
use donka_mail::Email;
use donka_project::Projects;
use donka_shared::clock::ManualClock;
use http_body_util::BodyExt;
use serde_json::Value;
use std::sync::Arc;
use tower::ServiceExt;

pub const BASE: &str = "/api/v1";
pub const ADMIN_EMAIL: &str = "ada@bank.example";
pub const ADMIN_PASSWORD: &str = "correct horse battery staple";

pub const PUBLIC_URL: &str = "https://studio.bank.example";

pub struct TestApp {
    pub router: Router,
    pub identity: Identity,
    pub clock: Arc<ManualClock>,
    pub mailer: Arc<RecordingMailer>,
}

pub fn policy() -> Policy {
    Policy::default()
}

pub fn build(db: PgPool, runtime: Arc<dyn DecisionRuntime>, base: &str) -> TestApp {
    build_with_web(db, runtime, base, None)
}

pub fn build_with_web(
    db: PgPool,
    runtime: Arc<dyn DecisionRuntime>,
    base: &str,
    web_dir: Option<&std::path::Path>,
) -> TestApp {
    let clock = Arc::new(ManualClock::new(
        Utc.with_ymd_and_hms(2026, 9, 28, 9, 0, 0).unwrap(),
    ));
    let identity = Identity::new(db.clone(), clock.clone(), policy());
    let projects = Projects::new(db.clone(), clock.clone());
    let decisions = Decisions::new(db.clone(), clock.clone(), runtime.clone());
    let audit = AuditLog::new(db.clone());
    let router = router(
        AppState {
            runtime,
            db,
            identity: identity.clone(),
            projects,
            decisions,
            audit,
            cookies: CookieSettings {
                secure: true,
                max_age_seconds: 8 * 3600,
            },
        },
        base,
        web_dir,
    );
    TestApp {
        router,
        identity,
        clock,
        mailer: Arc::new(RecordingMailer::default()),
    }
}

/// An app whose database is unreachable (port 1 refuses immediately): only
/// routes that never touch the database can succeed with it.
pub fn without_database() -> TestApp {
    let options = DbOptions {
        acquire_timeout: std::time::Duration::from_millis(300),
        ..DbOptions::default()
    };
    let db = donka_db::connect_lazy("postgres://nobody@127.0.0.1:1/none", &options).unwrap();
    build(db, Arc::new(ZenRuntime::new(1)), BASE)
}

pub fn with_database(db: PgPool) -> TestApp {
    build(db, Arc::new(ZenRuntime::new(1)), BASE)
}

pub struct Reply {
    pub status: StatusCode,
    pub request_id: Option<String>,
    pub set_cookie: Option<String>,
    pub body: Value,
}

pub async fn send(app: &Router, req: Request<Body>) -> Reply {
    let res = app.clone().oneshot(req).await.unwrap();
    let status = res.status();
    // Read headers in their own scope so no borrow of `res` lives across the await.
    let (request_id, set_cookie) = {
        let header_text = |name: &str| {
            res.headers()
                .get(name)
                .map(|v| v.to_str().unwrap().to_owned())
        };
        (
            header_text("x-request-id"),
            header_text(header::SET_COOKIE.as_str()),
        )
    };
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let body = serde_json::from_slice(&bytes)
        .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&bytes).into_owned()));
    Reply {
        status,
        request_id,
        set_cookie,
        body,
    }
}

pub fn get(path: &str, session: Option<&str>) -> Request<Body> {
    let mut req = Request::get(path);
    if let Some(token) = session {
        req = req.header(header::COOKIE, format!("{SESSION_COOKIE}={token}"));
    }
    req.body(Body::empty()).unwrap()
}

/// A JSON POST as Studio's own pages send it: with the CSRF header.
pub fn post(path: &str, body: &Value, session: Option<&str>) -> Request<Body> {
    let mut req = Request::post(path)
        .header(header::CONTENT_TYPE, "application/json")
        .header(CSRF_HEADER, "1");
    if let Some(token) = session {
        req = req.header(header::COOKIE, format!("{SESSION_COOKIE}={token}"));
    }
    req.body(Body::from(body.to_string())).unwrap()
}

/// The session token from a `Set-Cookie` header.
pub fn session_from(reply: &Reply) -> String {
    let cookie = reply.set_cookie.as_deref().expect("a Set-Cookie header");
    cookie
        .strip_prefix(&format!("{SESSION_COOKIE}="))
        .and_then(|rest| rest.split(';').next())
        .unwrap()
        .to_owned()
}

/// Creates the first administrator with a password.
pub async fn admin_with_password(app: &TestApp) {
    let token = app
        .identity
        .bootstrap_admin(ADMIN_EMAIL, Locale::En)
        .await
        .unwrap()
        .unwrap();
    app.identity
        .complete_password_setup(token.expose(), ADMIN_PASSWORD)
        .await
        .unwrap();
}

pub async fn sign_in(app: &TestApp, email: &str, password: &str) -> Reply {
    let body = serde_json::json!({ "email": email, "password": password });
    send(
        &app.router,
        post(&format!("{BASE}/auth/sign-in"), &body, None),
    )
    .await
}

/// An administrator who is signed in; returns the session token.
pub async fn signed_in_admin(app: &TestApp) -> String {
    admin_with_password(app).await;
    let reply = sign_in(app, ADMIN_EMAIL, ADMIN_PASSWORD).await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.body);
    session_from(&reply)
}

pub fn assert_error(reply: &Reply, status: StatusCode, code: &str) {
    assert_eq!(reply.status, status, "{}", reply.body);
    assert_eq!(reply.body["code"], code, "{}", reply.body);
    assert!(reply.body["message"]
        .as_str()
        .is_some_and(|m| !m.is_empty()));
    let id = reply.body["requestId"].as_str().expect("requestId in body");
    assert_eq!(
        Some(id),
        reply.request_id.as_deref(),
        "body and header ids match"
    );
}

/// Runs one pass of the email worker; returns how many emails were sent.
pub async fn deliver(app: &TestApp) -> usize {
    app.identity
        .deliver_due_emails(app.mailer.as_ref(), PUBLIC_URL)
        .await
        .unwrap()
}

/// Emails sent so far to `to`, oldest first.
pub fn emails_to(app: &TestApp, to: &str) -> Vec<Email> {
    app.mailer
        .sent()
        .into_iter()
        .filter(|e| e.to == to)
        .collect()
}

/// The one-time token in the link of an email, checking the link opens the
/// setup page in `locale`.
pub fn token_in_locale(email: &Email, locale: &str) -> String {
    let prefix = format!("{PUBLIC_URL}/{locale}/setup-password/?token=");
    let start = email.text.find(&prefix).expect("a setup link") + prefix.len();
    email.text[start..]
        .split_whitespace()
        .next()
        .unwrap()
        .to_owned()
}

/// The one-time token in the link of an email sent in English.
pub fn token_in(email: &Email) -> String {
    token_in_locale(email, "en")
}

/// A JSON request with any method, as Studio's pages send it (CSRF header included).
pub fn request(
    method: &str,
    path: &str,
    body: Option<&Value>,
    session: Option<&str>,
) -> Request<Body> {
    let mut req = Request::builder()
        .method(method)
        .uri(path)
        .header(CSRF_HEADER, "1");
    if let Some(token) = session {
        req = req.header(header::COOKIE, format!("{SESSION_COOKIE}={token}"));
    }
    match body {
        Some(body) => req
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body.to_string()))
            .unwrap(),
        None => req.body(Body::empty()).unwrap(),
    }
}

/// A signed-in user invited by the first administrator, who must exist already.
/// Returns their session.
pub async fn signed_in_user(app: &TestApp, email: &str, is_admin: bool) -> String {
    let admin = app
        .identity
        .find_by_email(ADMIN_EMAIL)
        .await
        .unwrap()
        .unwrap();
    app.identity
        .invite(&admin, email, Locale::En, is_admin)
        .await
        .unwrap();
    deliver(app).await;
    let token = token_in(emails_to(app, email).last().unwrap());
    let password = "a long enough passphrase";
    app.identity
        .complete_password_setup(&token, password)
        .await
        .unwrap();
    session_from(&sign_in(app, email, password).await)
}
