//! Donka Studio backend.
//!
//! Route handlers depend on `DecisionRuntime`, never on `zen_engine`, so tests
//! can swap the engine and the engine can be upgraded in one place.

pub mod auth;
pub mod config;
pub mod email_worker;
pub mod error;
pub mod extract;
pub mod request_id;
pub mod routes;
pub mod web;

use auth::CookieSettings;
use axum::extract::DefaultBodyLimit;
use axum::http::header::{
    HeaderName, HeaderValue, REFERRER_POLICY, X_CONTENT_TYPE_OPTIONS, X_FRAME_OPTIONS,
};
use axum::routing::get;
use axum::{middleware, Json, Router};
use donka_db::PgPool;
use donka_engine::DecisionRuntime;
use donka_identity::Identity;
use donka_project::Projects;
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
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            auth::require_session,
        ));

    let (api, mut doc) = public.merge(protected).with_state(state).split_for_parts();
    doc.servers = Some(vec![Server::new(api_base_path)]);
    doc.info.version = VERSION.to_owned();

    let api = api
        .route("/openapi.json", get(move || openapi(doc.clone())))
        .fallback(api_not_found)
        .layer(DefaultBodyLimit::max(MAX_BODY_BYTES))
        .layer(middleware::from_fn(auth::require_csrf_header));

    let app = Router::new().nest(api_base_path, api);
    let app = match web_dir {
        Some(dir) => app.merge(web::routes(dir)),
        None => app,
    };
    app.layer(security_header(X_CONTENT_TYPE_OPTIONS, "nosniff"))
        .layer(security_header(X_FRAME_OPTIONS, "DENY"))
        // Setup links carry their token in the query string: never send it to another site.
        .layer(security_header(REFERRER_POLICY, "same-origin"))
        .layer(TraceLayer::new_for_http().on_response(DefaultOnResponse::new().level(Level::INFO)))
        .layer(CompressionLayer::new())
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
