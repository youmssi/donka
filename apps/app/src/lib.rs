//! Donka Studio backend.
//!
//! Route handlers depend on `DecisionRuntime`, never on `zen_engine`, so tests
//! can swap the engine and the engine can be upgraded in one place.

pub mod config;
pub mod routes;

use axum::extract::DefaultBodyLimit;
use axum::routing::{get, post};
use axum::Router;
use donka_engine::DecisionRuntime;
use std::sync::Arc;
use tower_http::compression::CompressionLayer;
use tower_http::trace::TraceLayer;

/// Decision models are JSON documents that can reach a few MB with large tables.
const MAX_BODY_BYTES: usize = 16 * 1024 * 1024;

#[derive(Clone)]
pub struct AppState {
    pub runtime: Arc<dyn DecisionRuntime>,
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/api/health", get(routes::health))
        .route(
            "/api/simulate",
            post(routes::simulate).layer(DefaultBodyLimit::max(MAX_BODY_BYTES)),
        )
        .with_state(state)
        .layer(TraceLayer::new_for_http())
        .layer(CompressionLayer::new())
}
