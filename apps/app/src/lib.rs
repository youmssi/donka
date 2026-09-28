//! Donka Studio backend.
//!
//! Route handlers depend on `DecisionRuntime`, never on `zen_engine`, so tests
//! can swap the engine and the engine can be upgraded in one place.

pub mod auth;
pub mod config;
pub mod error;
pub mod extract;
pub mod request_id;
pub mod routes;

use auth::CookieSettings;
use axum::extract::DefaultBodyLimit;
use axum::routing::get;
use axum::{middleware, Json, Router};
use donka_db::PgPool;
use donka_engine::DecisionRuntime;
use donka_identity::Identity;
use std::sync::Arc;
use tower_http::compression::CompressionLayer;
use tower_http::trace::{DefaultOnResponse, TraceLayer};
use tracing::Level;
use utoipa::openapi::{OpenApi as OpenApiDoc, Server};
use utoipa::OpenApi;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

/// Decision models are JSON documents that can reach a few MB with large tables.
const MAX_BODY_BYTES: usize = 16 * 1024 * 1024;

#[derive(Clone)]
pub struct AppState {
    pub runtime: Arc<dyn DecisionRuntime>,
    pub db: PgPool,
    pub identity: Identity,
    pub cookies: CookieSettings,
}

#[derive(OpenApi)]
#[openapi(info(
    title = "Donka Studio API",
    description = "Control plane of Donka: authoring, simulation, releases."
))]
struct ApiDoc;

/// All endpoints, mounted under `api_base_path` (e.g. `/api/v1`).
///
/// Deny by default: only the routes in `public` answer without a session.
pub fn router(state: AppState, api_base_path: &str) -> Router {
    let public = OpenApiRouter::with_openapi(ApiDoc::openapi())
        .routes(routes!(routes::health::health))
        .routes(routes!(routes::health::ready))
        .routes(routes!(routes::auth::sign_in))
        .routes(routes!(routes::auth::password_setup));

    let protected = OpenApiRouter::new()
        .routes(routes!(routes::auth::me))
        .routes(routes!(routes::auth::sign_out))
        .routes(routes!(routes::simulate::simulate))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            auth::require_session,
        ));

    let (api, mut doc) = public.merge(protected).with_state(state).split_for_parts();
    doc.servers = Some(vec![Server::new(api_base_path)]);

    let api = api
        .route("/openapi.json", get(move || openapi(doc.clone())))
        .layer(DefaultBodyLimit::max(MAX_BODY_BYTES))
        .layer(middleware::from_fn(auth::require_csrf_header));

    Router::new()
        .nest(api_base_path, api)
        .layer(TraceLayer::new_for_http().on_response(DefaultOnResponse::new().level(Level::INFO)))
        .layer(CompressionLayer::new())
        .layer(middleware::from_fn(request_id::middleware))
}

async fn openapi(doc: OpenApiDoc) -> Json<OpenApiDoc> {
    Json(doc)
}
