use super::handlers::{self, AppState};
use axum::{
    Router,
    routing::{delete, get, post},
};
use tower_http::{
    compression::CompressionLayer,
    cors::{Any, CorsLayer},
    trace::TraceLayer,
};

/// Create the Axum router with all endpoints
pub fn create_router(
    state: AppState,
    _rate_limit_enabled: bool,
    _requests_per_second: u64,
) -> Router {
    // CORS configuration
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        // Health check
        .route("/health", get(handlers::health_check))
        // KV REST API endpoints
        .route("/kv/set", post(handlers::kv_set))
        .route("/kv/get/:key", get(handlers::kv_get))
        .route("/kv/del/:key", delete(handlers::kv_delete))
        .route("/kv/stats", get(handlers::kv_stats))
        // Persistence endpoints
        .route("/snapshot", post(handlers::trigger_snapshot))
        // Event Stream REST API endpoints
        .route("/stream/:room", post(handlers::stream_create_room))
        .route("/stream/:room/publish", post(handlers::stream_publish))
        .route("/stream/:room/consume/:subscriber_id", get(handlers::stream_consume))
        .route("/stream/:room/stats", get(handlers::stream_room_stats))
        .route("/stream/:room", delete(handlers::stream_delete_room))
        .route("/stream/list", get(handlers::stream_list_rooms))
        // Queue REST API endpoints
        .route("/queue/:name", post(handlers::queue_create))
        .route("/queue/:name/publish", post(handlers::queue_publish))
        .route(
            "/queue/:name/consume/:consumer_id",
            get(handlers::queue_consume),
        )
        .route("/queue/:name/ack", post(handlers::queue_ack))
        .route("/queue/:name/nack", post(handlers::queue_nack))
        .route("/queue/:name/stats", get(handlers::queue_stats))
        .route("/queue/:name/purge", post(handlers::queue_purge))
        .route("/queue/:name", delete(handlers::queue_delete))
        .route("/queue/list", get(handlers::queue_list))
        // Pub/Sub REST API endpoints
        .route("/pubsub/subscribe", post(handlers::pubsub_subscribe))
        .route("/pubsub/:topic/publish", post(handlers::pubsub_publish))
        .route("/pubsub/unsubscribe", post(handlers::pubsub_unsubscribe))
        .route("/pubsub/stats", get(handlers::pubsub_stats))
        .route("/pubsub/topics", get(handlers::pubsub_list_topics))
        .route("/pubsub/:topic/info", get(handlers::pubsub_topic_info))
        // StreamableHTTP command endpoint
        .route("/api/v1/command", post(handlers::command_handler))
        // Add state and middleware
        .with_state(state)
        .layer(CompressionLayer::new()) // Gzip compression for responses
        .layer(TraceLayer::new_for_http())
        .layer(cors)
    // NOTE: Rate limiting disabled for now due to Clone requirements
    // Will be implemented with a different approach (e.g., middleware per-route or governor crate)
}
