//! Donka Studio backend.
//!
//! Route handlers depend on `DecisionRuntime`, never on `zen_engine`, so tests
//! can swap the engine and the engine can be upgraded in one place.

pub mod config;
pub mod error;
pub mod extract;
pub mod request_id;
pub mod routes;

use axum::extract::DefaultBodyLimit;
use axum::routing::get;
use axum::{middleware, Json, Router};
use donka_db::PgPool;
use donka_engine::DecisionRuntime;
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
}

#[derive(OpenApi)]
#[openapi(info(
    title = "Donka Studio API",
    description = "Control plane of Donka: authoring, simulation, releases."
))]
struct ApiDoc;

/// All endpoints, mounted under `api_base_path` (e.g. `/api/v1`).
pub fn router(state: AppState, api_base_path: &str) -> Router {
    let (api, mut doc) = OpenApiRouter::with_openapi(ApiDoc::openapi())
        .routes(routes!(routes::health::health))
        .routes(routes!(routes::health::ready))
        .routes(routes!(routes::simulate::simulate))
        .with_state(state)
        .split_for_parts();
    doc.servers = Some(vec![Server::new(api_base_path)]);

    let api = api
        .route("/openapi.json", get(move || openapi(doc.clone())))
        .layer(DefaultBodyLimit::max(MAX_BODY_BYTES));

    Router::new()
        .nest(api_base_path, api)
        .layer(TraceLayer::new_for_http().on_response(DefaultOnResponse::new().level(Level::INFO)))
        .layer(CompressionLayer::new())
        .layer(middleware::from_fn(request_id::middleware))
}

async fn openapi(doc: OpenApiDoc) -> Json<OpenApiDoc> {
    Json(doc)
}
