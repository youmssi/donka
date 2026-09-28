#![allow(clippy::unwrap_used)] // tests fail loudly on purpose

mod support;

use axum::http::{header, StatusCode};
use donka_engine::ZenRuntime;
use std::path::PathBuf;
use std::sync::Arc;
use support::*;

/// A miniature static export, like `apps/web/out`.
struct Export(PathBuf);

impl Export {
    fn new() -> Self {
        let dir = std::env::temp_dir().join(format!("donka-web-{}", uuid::Uuid::new_v4()));
        for (path, content) in [
            ("index.html", "root"),
            ("404.html", "not found page"),
            ("en/index.html", "home en"),
            ("en/sign-in/index.html", "sign in en"),
            ("_next/static/chunks/app-3f2a.js", "js"),
        ] {
            let file = dir.join(path);
            std::fs::create_dir_all(file.parent().unwrap()).unwrap();
            std::fs::write(file, content).unwrap();
        }
        Self(dir)
    }

    fn app(&self) -> TestApp {
        let options = donka_db::DbOptions::default();
        let db = donka_db::connect_lazy("postgres://nobody@127.0.0.1:1/none", &options).unwrap();
        build_with_web(db, Arc::new(ZenRuntime::new(1)), BASE, Some(&self.0))
    }
}

impl Drop for Export {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

async fn header_of(
    app: &TestApp,
    path: &str,
    name: header::HeaderName,
) -> (StatusCode, Option<String>) {
    use tower::ServiceExt;
    let res = app.router.clone().oneshot(get(path, None)).await.unwrap();
    let value = res
        .headers()
        .get(name)
        .map(|v| v.to_str().unwrap().to_owned());
    (res.status(), value)
}

#[tokio::test]
async fn pages_are_served_on_the_same_origin_as_the_api() {
    let export = Export::new();
    let app = export.app();

    let page = send(&app.router, get("/en/sign-in/", None)).await;
    assert_eq!(page.status, StatusCode::OK);
    assert_eq!(page.body, "sign in en");
    let api = send(&app.router, get("/api/v1/health", None)).await;
    assert_eq!(api.body, "ok");
}

#[tokio::test]
async fn a_page_path_without_its_trailing_slash_redirects_to_it() {
    let export = Export::new();
    let app = export.app();
    let (status, location) = header_of(&app, "/en/sign-in", header::LOCATION).await;
    assert!(status.is_redirection(), "{status}");
    assert_eq!(location.as_deref(), Some("/en/sign-in/"));
}

#[tokio::test]
async fn unknown_paths_get_the_app_not_found_page() {
    let export = Export::new();
    let app = export.app();
    let reply = send(&app.router, get("/en/no-such-page/", None)).await;
    assert_eq!(reply.status, StatusCode::NOT_FOUND);
    assert_eq!(reply.body, "not found page");
}

#[tokio::test]
async fn unknown_api_paths_answer_in_the_api_error_shape() {
    let export = Export::new();
    let app = export.app();
    let reply = send(&app.router, get("/api/v1/no-such-endpoint", None)).await;
    assert_error(&reply, StatusCode::NOT_FOUND, "NOT_FOUND");
}

#[tokio::test]
async fn hashed_assets_are_cached_and_pages_revalidated() {
    let export = Export::new();
    let app = export.app();
    let (_, asset) = header_of(
        &app,
        "/_next/static/chunks/app-3f2a.js",
        header::CACHE_CONTROL,
    )
    .await;
    assert_eq!(
        asset.as_deref(),
        Some("public, max-age=31536000, immutable")
    );
    let (_, page) = header_of(&app, "/en/", header::CACHE_CONTROL).await;
    assert_eq!(page.as_deref(), Some("no-cache"));
}

#[tokio::test]
async fn every_response_carries_the_security_headers() {
    let export = Export::new();
    let app = export.app();
    for path in ["/en/", "/api/v1/health"] {
        let (_, referrer) = header_of(&app, path, header::REFERRER_POLICY).await;
        assert_eq!(referrer.as_deref(), Some("same-origin"), "{path}");
        let (_, frames) = header_of(&app, path, header::X_FRAME_OPTIONS).await;
        assert_eq!(frames.as_deref(), Some("DENY"), "{path}");
        let (_, sniff) = header_of(&app, path, header::X_CONTENT_TYPE_OPTIONS).await;
        assert_eq!(sniff.as_deref(), Some("nosniff"), "{path}");
    }
}

#[test]
fn a_directory_without_an_export_is_refused_at_startup() {
    let export = Export::new();
    assert!(donka_app::web::check_export(&export.0).is_ok());
    let empty = std::env::temp_dir().join(format!("donka-empty-{}", uuid::Uuid::new_v4()));
    let err = donka_app::web::check_export(&empty).unwrap_err();
    assert!(err.contains("DONKA_WEB_DIR"), "{err}");
}
