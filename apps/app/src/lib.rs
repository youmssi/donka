//! Donka Studio backend.
//!
//! Route handlers depend on `DecisionRuntime`, never on `zen_engine`, so tests
//! can swap the engine and the engine can be upgraded in one place.

pub mod auth;
pub mod config;
pub mod email_worker;
pub mod error;
pub mod extract;
pub mod publish_worker;
pub mod purge_worker;
pub mod request_id;
pub mod routes;
pub mod telemetry;
pub mod web;

use auth::CookieSettings;
use axum::extract::DefaultBodyLimit;
use axum::http::header::{
    HeaderName, HeaderValue, REFERRER_POLICY, X_CONTENT_TYPE_OPTIONS, X_FRAME_OPTIONS,
};
use axum::routing::get;
use axum::{middleware, Json, Router};
use donka_audit::AuditLog;
use donka_db::PgPool;
use donka_decision::Decisions;
use donka_decision_log::DecisionLog;
use donka_engine::DecisionRuntime;
use donka_identity::Identity;
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

/// Set by the `SERVICE_VERSION` build argument of the container image. Read at
/// compile time so the binary reports what it was built from, whatever its
/// runtime environment says.
pub const VERSION: &str = match option_env!("SERVICE_VERSION") {
    Some(version) => version,
    None => "unknown",
};

/// Decision models are JSON documents that can reach a few MB with large tables.
const MAX_BODY_BYTES: usize = 16 * 1024 * 1024;

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
    pub cookies: CookieSettings,
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
    let public = OpenApiRouter::with_openapi(ApiDoc::openapi())
        .routes(routes!(routes::health::health))
        .routes(routes!(routes::health::ready))
        .routes(routes!(routes::health::version))
        .routes(routes!(routes::auth::sign_in))
        .routes(routes!(routes::auth::password_setup))
        .routes(routes!(routes::auth::password_reset));

    let protected = OpenApiRouter::new()
        .routes(routes!(routes::auth::me))
        .routes(routes!(routes::auth::sign_out))
        .routes(routes!(routes::simulate::simulate))
        .routes(routes!(routes::users::invite))
        .routes(routes!(routes::users::list))
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

    let api = api
        .layer(middleware::from_fn(auth::require_csrf_header))
        .merge(feed)
        .route("/openapi.json", get(move || openapi(doc.clone())))
        .fallback(api_not_found)
        .layer(DefaultBodyLimit::max(MAX_BODY_BYTES));

    let app = Router::new().nest(api_base_path, api);
    let app = match web_dir {
        Some(dir) => app.merge(web::routes(dir)),
        None => app,
    };
    app.layer(security_header(X_CONTENT_TYPE_OPTIONS, "nosniff"))
        .layer(security_header(X_FRAME_OPTIONS, "DENY"))
        // Setup links carry their token in the query string: never send it to another site.
        .layer(security_header(REFERRER_POLICY, "same-origin"))
        .layer(
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
