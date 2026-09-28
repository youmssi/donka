//! Serves the web app's static export (`apps/web/out`) next to the API, on the same
//! origin, so the session cookie and CSRF rules need no cross-origin setup (ADR-002).

use axum::extract::Request;
use axum::http::header::{HeaderValue, CACHE_CONTROL};
use axum::middleware::{self, Next};
use axum::response::Response;
use axum::Router;
use std::path::Path;
use tower_http::services::{ServeDir, ServeFile};

/// Build output under this prefix has a content hash in its name and never changes.
const IMMUTABLE_PREFIX: &str = "/_next/static/";

/// Pages and assets of the export. `/en/sign-in` redirects to `/en/sign-in/` and
/// serves its `index.html`; any other unknown path gets the export's 404 page.
pub fn routes(dir: &Path) -> Router {
    let files = ServeDir::new(dir)
        .append_index_html_on_directories(true)
        .not_found_service(ServeFile::new(dir.join("404.html")));
    Router::new()
        .fallback_service(files)
        .layer(middleware::from_fn(cache_headers))
}

/// Hashed assets are cached for a year; pages are revalidated on every visit so a
/// new release shows up at once.
async fn cache_headers(req: Request, next: Next) -> Response {
    let immutable = req.uri().path().starts_with(IMMUTABLE_PREFIX);
    let mut res = next.run(req).await;
    let value = if immutable && res.status().is_success() {
        "public, max-age=31536000, immutable"
    } else {
        "no-cache"
    };
    res.headers_mut()
        .insert(CACHE_CONTROL, HeaderValue::from_static(value));
    res
}

/// Checked at startup, so a wrong path fails fast instead of serving 404s.
pub fn check_export(dir: &Path) -> Result<(), String> {
    for file in ["index.html", "404.html"] {
        if !dir.join(file).is_file() {
            return Err(format!(
                "DONKA_WEB_DIR {} has no {file}: point it at the output of `pnpm --dir apps/web build` (apps/web/out)",
                dir.display()
            ));
        }
    }
    Ok(())
}
