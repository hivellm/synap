use super::handlers;
use crate::core::KVStore;
use axum::{
    Router,
    routing::{delete, get, post},
};
use std::sync::Arc;
use tower_http::{
    cors::{Any, CorsLayer},
    trace::TraceLayer,
};

/// Create the Axum router with all endpoints
pub fn create_router(store: Arc<KVStore>) -> Router {
    // CORS configuration
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        // Health check
        .route("/health", get(handlers::health_check))
        // REST API endpoints
        .route("/kv/set", post(handlers::kv_set))
        .route("/kv/get/:key", get(handlers::kv_get))
        .route("/kv/del/:key", delete(handlers::kv_delete))
        .route("/kv/stats", get(handlers::kv_stats))
        // StreamableHTTP command endpoint
        .route("/api/v1/command", post(handlers::command_handler))
        // Add state and middleware
        .with_state(store)
        .layer(TraceLayer::new_for_http())
        .layer(cors)
}
