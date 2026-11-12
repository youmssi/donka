//! Donka Studio backend.
//!
//! Route handlers depend on `DecisionRuntime`, never on `zen_engine`, so tests
//! can swap the engine and the engine can be upgraded in one place.

pub mod auth;
pub mod config;
pub mod demo;
pub mod doctor;
pub mod email_worker;
pub mod error;
pub mod extract;
pub mod index_people;
pub mod install;
pub mod licence;
pub mod load;
pub mod publish_worker;
pub mod purge_worker;
pub mod rate_limit;
pub mod request_id;
pub mod reseal;
pub mod routes;
pub mod support;
pub mod telemetry;
pub mod updates;
pub mod upgrade;
pub mod web;
pub mod wiring;

use auth::CookieSettings;
use axum::extract::DefaultBodyLimit;
use axum::http::header::{
    HeaderName, HeaderValue, CONTENT_SECURITY_POLICY as CONTENT_SECURITY_POLICY_HEADER,
    REFERRER_POLICY, STRICT_TRANSPORT_SECURITY, X_CONTENT_TYPE_OPTIONS, X_FRAME_OPTIONS,
};
use axum::routing::get;
use axum::{middleware, Json, Router};
use donka_audit::AuditLog;
use donka_db::PgPool;
use donka_decision::Decisions;
use donka_decision_log::DecisionLog;
use donka_engine::DecisionRuntime;
use donka_identity::Identity;
use donka_pack::{Catalogue, Packs};
use donka_project::Projects;
use donka_release::Releases;
use std::path::Path;
use std::sync::Arc;
use tower_http::compression::CompressionLayer;
use tower_http::set_header::SetResponseHeaderLayer;
use tower_http::trace::{DefaultOnResponse, TraceLayer};
use tracing::Level;
use utoipa::openapi::{OpenApi as OpenApiDoc, Server};
use utoipa::OpenApi;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use utoipa_scalar::Servable;

/// Set by the `SERVICE_VERSION` build argument of the container image. Read at
/// compile time so the binary reports what it was built from, whatever its
/// runtime environment says.
pub const VERSION: &str = match option_env!("SERVICE_VERSION") {
    Some(version) => version,
    None => "unknown",
};

/// Decision models are JSON documents that can reach a few MB with large tables.
const MAX_BODY_BYTES: usize = 16 * 1024 * 1024;

/// Browser security policy for every Studio response (DNK-85): only Studio's own
/// scripts, styles, fonts and API. Inline styles stay allowed because the
/// component libraries set them at runtime. WASM compilation and blob workers
/// are allowed only for the decision editor and Monaco. The static export
/// inlines its own bootstrap scripts in every page; `content_security_policy`
/// lists their SHA-256 hashes, so any other inline script stays blocked.
pub const CONTENT_SECURITY_POLICY: &str = "default-src 'self'; script-src 'self' 'wasm-unsafe-eval'; style-src 'self' 'unsafe-inline'; font-src 'self' data:; img-src 'self' data: blob:; connect-src 'self'; worker-src 'self' blob:; frame-ancestors 'none'; base-uri 'self'; form-action 'self'; object-src 'none'";

/// The policy actually sent: the base policy plus the hash of every inline
/// script in the export Studio serves (DNK-85). Without an export (API-only
/// tests) the base policy is sent as it is.
fn content_security_policy(web_dir: Option<&Path>) -> String {
    let Some(dir) = web_dir else {
        return CONTENT_SECURITY_POLICY.to_owned();
    };
    let mut hashes = inline_script_hashes(dir);
    hashes.sort();
    hashes.dedup();
    if hashes.is_empty() {
        return CONTENT_SECURITY_POLICY.to_owned();
    }
    let listed = hashes
        .iter()
        .map(|hash| format!(" 'sha256-{hash}'"))
        .collect::<String>();
    CONTENT_SECURITY_POLICY.replacen(
        "script-src 'self' 'wasm-unsafe-eval'",
        &format!("script-src 'self' 'wasm-unsafe-eval'{listed}"),
        1,
    )
}

/// SHA-256 (base64) of the text of every `<script>` without a `src` in the
/// export's HTML pages. The browser hashes the same text, so a script that is
/// not part of the served export never matches.
fn inline_script_hashes(dir: &Path) -> Vec<String> {
    use base64::Engine;
    use sha2::Digest;
    let mut hashes = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(path) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&path) else {
            continue;
        };
        for entry in entries.flatten() {
            let entry_path = entry.path();
            if entry_path.is_dir() {
                stack.push(entry_path);
            } else if entry_path.extension().is_some_and(|ext| ext == "html") {
                let Ok(html) = std::fs::read(&entry_path) else {
                    continue;
                };
                for script in inline_scripts(&html) {
                    let digest = sha2::Sha256::digest(script);
                    hashes.push(base64::engine::general_purpose::STANDARD.encode(digest));
                }
            }
        }
    }
    hashes
}

/// The text between `>` and `</script>` of every script tag that loads no
/// external file.
fn inline_scripts(html: &[u8]) -> Vec<&[u8]> {
    let mut scripts = Vec::new();
    let mut rest = html;
    while let Some(start) = find(rest, b"<script") {
        let tag_and_after = &rest[start..];
        let Some(tag_end) = tag_and_after.iter().position(|byte| *byte == b'>') else {
            break;
        };
        let tag = &tag_and_after[..tag_end];
        let body = &tag_and_after[tag_end + 1..];
        let Some(body_end) = find(body, b"</script>") else {
            break;
        };
        if !tag
            .windows(5)
            .any(|window| window == b" src=" || window == b"\tsrc=" || window == b"\nsrc=")
        {
            scripts.push(&body[..body_end]);
        }
        rest = &body[body_end + b"</script>".len()..];
    }
    scripts
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

/// Sent only when the installation's public URL is HTTPS (DNK-85).
const HSTS_VALUE: &str = "max-age=63072000; includeSubDomains";

fn hsts_value(public_url: &str) -> Option<&'static str> {
    public_url.starts_with("https://").then_some(HSTS_VALUE)
}

#[derive(Clone)]
pub struct AppState {
    pub runtime: Arc<dyn DecisionRuntime>,
    pub db: PgPool,
    pub identity: Identity,
    pub projects: Projects,
    pub decisions: Decisions,
    pub releases: Releases,
    pub decision_log: DecisionLog,
    /// Explains logged decisions; `None` when the installation does not (DNK-19).
    pub explainer: Option<donka_explain::Explainer>,
    pub audit: AuditLog,
    pub packs: Packs,
    /// The packs this installation offers (DNK-43).
    pub catalogue: Arc<Catalogue>,
    pub cookies: CookieSettings,
    /// Who must sign in with a second step (DNK-59).
    pub mfa_required: config::MfaRequired,
    /// Per-client-address limits on sign-in and password reset (DNK-32).
    pub auth_limits: rate_limit::AuthLimits,
    /// The installation's licence (DNK-77).
    pub licensing: licence::Licensing,
    /// Whether a newer Donka release is published (DNK-64).
    pub updates: updates::Updates,
    /// The organisation served and the first-run assistant (DNK-74).
    pub installation: donka_installation::Installation,
    /// Where the installation's Runtime lets connectors call (DNK-84); `None` when Studio is
    /// not told, and then saving a version does not check it.
    pub connector_destinations: Option<Arc<donka_engine::Destinations>>,
    /// The installation's public URL (`DONKA_PUBLIC_URL`); HSTS is sent only
    /// when it is HTTPS (DNK-85).
    pub public_url: String,
}

#[derive(OpenApi)]
#[openapi(info(
    title = "Donka Studio API",
    description = "Control plane of Donka: authoring, simulation, releases."
))]
struct ApiDoc;

/// All endpoints, mounted under `api_base_path` (e.g. `/api/v1`), and the web
/// app's static export when `web_dir` is given.
///
/// Deny by default: only the routes in `public` answer without a session.
pub fn router(state: AppState, api_base_path: &str, web_dir: Option<&Path>) -> Router {
    let hsts = hsts_value(&state.public_url);
    let public = OpenApiRouter::with_openapi(ApiDoc::openapi())
        .routes(routes!(routes::health::health))
        .routes(routes!(routes::health::ready))
        .routes(routes!(routes::health::version))
        .routes(routes!(routes::auth::sign_in))
        .routes(routes!(routes::auth::sign_in_code))
        .routes(routes!(routes::auth::sign_in_enrolment))
        .routes(routes!(routes::auth::sign_in_enrolment_confirm))
        .routes(routes!(routes::auth::password_setup))
        .routes(routes!(routes::auth::password_reset))
        .routes(routes!(routes::installation::get));

    let protected = OpenApiRouter::new()
        .routes(routes!(routes::auth::me))
        .routes(routes!(routes::licence::get))
        .routes(routes!(routes::installation::finish_first_run))
        .routes(routes!(routes::installation::rename))
        .routes(routes!(routes::updates::get))
        .routes(routes!(routes::auth::sign_out))
        .routes(routes!(routes::simulate::simulate))
        .routes(routes!(routes::users::invite))
        .routes(routes!(routes::users::list))
        .routes(routes!(routes::second_step::status))
        .routes(routes!(routes::second_step::begin_enrolment))
        .routes(routes!(routes::second_step::confirm_enrolment))
        .routes(routes!(routes::second_step::regenerate_recovery_codes))
        .routes(routes!(routes::second_step::disable))
        .routes(routes!(routes::second_step::reset))
        .routes(routes!(routes::projects::list, routes::projects::create))
        .routes(routes!(routes::projects::get, routes::projects::update))
        .routes(routes!(routes::projects::get_by_key))
        .routes(routes!(routes::projects::archive))
        .routes(routes!(routes::projects::restore))
        .routes(routes!(
            routes::projects::members,
            routes::projects::add_member
        ))
        .routes(routes!(
            routes::projects::change_role,
            routes::projects::remove_member
        ))
        .routes(routes!(routes::decisions::list, routes::decisions::create))
        .routes(routes!(
            routes::decisions::get,
            routes::decisions::save,
            routes::decisions::delete
        ))
        .routes(routes!(routes::decisions::simulate))
        .routes(routes!(routes::versions::list, routes::versions::save))
        .routes(routes!(routes::versions::get))
        .routes(routes!(routes::findings::list))
        .routes(routes!(routes::findings::acknowledge))
        .routes(routes!(routes::versions::restore))
        .routes(routes!(routes::scenarios::test_results))
        .routes(routes!(routes::releases::list, routes::releases::create))
        .routes(routes!(routes::releases::preview))
        .routes(routes!(routes::releases::get))
        .routes(routes!(routes::releases::environments))
        .routes(routes!(
            routes::releases::deployments,
            routes::releases::deploy
        ))
        .routes(routes!(routes::releases::retry))
        .routes(routes!(
            routes::releases::tokens,
            routes::releases::issue_token
        ))
        .routes(routes!(routes::releases::revoke_token))
        .routes(routes!(routes::approvals::list, routes::approvals::request))
        .routes(routes!(routes::approvals::get))
        .routes(routes!(routes::approvals::approve))
        .routes(routes!(routes::approvals::reject))
        .routes(routes!(routes::approvals::withdraw))
        .routes(routes!(routes::approvals::rollback_targets))
        .routes(routes!(routes::approvals::rollback))
        .routes(routes!(routes::scenarios::list, routes::scenarios::create))
        .routes(routes!(
            routes::scenarios::get,
            routes::scenarios::update,
            routes::scenarios::delete
        ))
        .routes(routes!(routes::onboarding::checklist))
        .routes(routes!(routes::onboarding::tours))
        .routes(routes!(routes::onboarding::tour_seen))
        .routes(routes!(routes::packs::list))
        .routes(routes!(routes::packs::inspect))
        .routes(routes!(routes::packs::import))
        .routes(routes!(routes::packs::import_file))
        .routes(routes!(routes::packs::duplicate))
        .routes(routes!(routes::packs::export))
        .routes(routes!(routes::audit::list))
        .routes(routes!(routes::audit::export))
        .routes(routes!(
            routes::decision_log::tokens,
            routes::decision_log::issue_token
        ))
        .routes(routes!(routes::decision_log::revoke_token))
        .routes(routes!(
            routes::decision_log::settings,
            routes::decision_log::update_settings
        ))
        .routes(routes!(routes::decision_log::search))
        .routes(routes!(routes::decision_log::get))
        .routes(routes!(routes::decision_log::replay))
        .routes(routes!(routes::decision_log::explain))
        .routes(routes!(routes::rights::search))
        .routes(routes!(routes::rights::report))
        .routes(routes!(routes::rights::anonymise))
        .routes(routes!(routes::rights::rules, routes::rights::create_rule))
        .routes(routes!(routes::rights::end_rule))
        .routes(routes!(
            routes::rights::contests,
            routes::rights::open_contest
        ))
        .routes(routes!(routes::rights::review_contest))
        .routes(routes!(
            routes::rules_sync::ci_tokens,
            routes::rules_sync::issue_ci_token
        ))
        .routes(routes!(routes::rules_sync::revoke_ci_token))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            auth::require_session,
        ));

    // Runtimes and CI pipelines authenticate with a bearer token, not a
    // cookie: no session, no CSRF header.
    let feed = OpenApiRouter::new()
        .routes(routes!(routes::decision_log::receive))
        .routes(routes!(routes::rules_sync::sync))
        .routes(routes!(routes::rules_sync::release_artifact))
        .routes(routes!(routes::rules_sync::deployment_artifact));

    let (api, mut doc) = public
        .merge(protected)
        .with_state(state.clone())
        .split_for_parts();
    let (feed, feed_doc) = feed.with_state(state).split_for_parts();
    doc.merge(feed_doc);
    doc.servers = Some(vec![Server::new(api_base_path)]);
    doc.info.version = VERSION.to_owned();
    let scalar_html = r#"<!doctype html>
<html>
<head>
    <title>Donka Studio API</title>
    <meta charset="utf-8"/>
    <meta name="viewport" content="width=device-width, initial-scale=1"/>
</head>
<body>
<script id="api-reference" type="application/json" data-configuration='{"agent":{"disabled":true},"showDeveloperTools":"never","withDefaultFonts":false}'>
$spec
</script>
<script src="/scalar-api-reference.js"></script>
</body>
</html>"#;
    let scalar = utoipa_scalar::Scalar::with_url("/api/docs", doc.clone()).custom_html(scalar_html);

    let api = api
        .layer(middleware::from_fn(auth::require_csrf_header))
        .merge(feed)
        .route("/openapi.json", get(move || openapi(doc.clone())))
        .fallback(api_not_found)
        .layer(DefaultBodyLimit::max(MAX_BODY_BYTES));

    let app = Router::new().merge(scalar).nest(api_base_path, api);
    let app = match web_dir {
        Some(dir) => app.merge(web::routes(dir)),
        None => app,
    };
    // Built once here: it lists the served export's own inline scripts by hash.
    let policy = HeaderValue::from_str(&content_security_policy(web_dir))
        .unwrap_or_else(|_| HeaderValue::from_static(CONTENT_SECURITY_POLICY));
    let app = app
        .layer(security_header(X_CONTENT_TYPE_OPTIONS, "nosniff"))
        .layer(security_header(X_FRAME_OPTIONS, "DENY"))
        // Setup links carry their token in the query string: never send it to another site.
        .layer(security_header(REFERRER_POLICY, "same-origin"))
        .layer(SetResponseHeaderLayer::if_not_present(
            CONTENT_SECURITY_POLICY_HEADER,
            policy,
        ));
    // HSTS only over HTTPS: plain-HTTP local and demo installs must not pin
    // browsers to HTTPS (DNK-85).
    let app = match hsts {
        Some(value) => app.layer(security_header(STRICT_TRANSPORT_SECURITY, value)),
        None => app,
    };
    app.layer(
        TraceLayer::new_for_http()
            // Not the default span: it records the full URI, query string included, and a
            // password-setup link carries its token there.
            .make_span_with(|req: &axum::http::Request<_>| {
                tracing::info_span!(
                    "http",
                    method = %req.method(),
                    route = %telemetry::route(req),
                    request_id = %request_id::current().unwrap_or_default(),
                )
            })
            .on_response(DefaultOnResponse::new().level(Level::INFO)),
    )
    .layer(CompressionLayer::new())
    .layer(middleware::from_fn(telemetry::record_request))
    .layer(middleware::from_fn(request_id::middleware))
}

async fn openapi(doc: OpenApiDoc) -> Json<OpenApiDoc> {
    Json(doc)
}

async fn api_not_found() -> error::ApiError {
    error::ApiError::RouteNotFound
}

fn security_header(name: HeaderName, value: &'static str) -> SetResponseHeaderLayer<HeaderValue> {
    SetResponseHeaderLayer::if_not_present(name, HeaderValue::from_static(value))
}

#[cfg(test)]
mod tests {
    use super::{content_security_policy, hsts_value, inline_scripts};

    #[test]
    fn hsts_is_sent_only_for_an_https_public_url() {
        assert_eq!(
            hsts_value("https://studio.bank.example"),
            Some("max-age=63072000; includeSubDomains")
        );
        assert_eq!(hsts_value("http://localhost:8080"), None);
    }

    #[test]
    fn only_inline_scripts_are_listed_by_hash() {
        let html = b"<script src=\"/app.js\"></script><script>first</script><script id=\"x\">second</script>";
        let scripts = inline_scripts(html);
        assert_eq!(scripts, vec![b"first".as_slice(), b"second".as_slice()]);
        // Without an export the base policy is sent unchanged.
        assert_eq!(
            content_security_policy(None),
            super::CONTENT_SECURITY_POLICY
        );
    }
}
