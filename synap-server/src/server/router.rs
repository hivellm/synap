use super::handlers::{self, AppState};
use super::mcp_server::SynapMcpService;
use axum::{
    Router,
    routing::{delete, get, post},
};
use std::sync::Arc;
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

    // Create MCP router (stateless)
    let state_arc = Arc::new(state.clone());
    let mcp_router = create_mcp_router(state_arc);

    // Create main API router with state
    let api_router = Router::new()
        // Health check
        .route("/health", get(handlers::health_check))
        // KV endpoints
        .route("/kv/ws", get(handlers::kv_websocket)) // WebSocket for WATCH (future)
        .route("/kv/set", post(handlers::kv_set))
        .route("/kv/get/{key}", get(handlers::kv_get))
        .route("/kv/del/{key}", delete(handlers::kv_delete))
        .route("/kv/stats", get(handlers::kv_stats))
        // Persistence endpoints
        .route("/snapshot", post(handlers::trigger_snapshot))
        // Event Stream endpoints
        .route(
            "/stream/{room}/ws/{subscriber_id}",
            get(handlers::stream_websocket),
        ) // WebSocket for real-time push
        .route("/stream/{room}", post(handlers::stream_create_room))
        .route("/stream/{room}/publish", post(handlers::stream_publish))
        .route(
            "/stream/{room}/consume/{subscriber_id}",
            get(handlers::stream_consume),
        )
        .route("/stream/{room}/stats", get(handlers::stream_room_stats))
        .route("/stream/{room}", delete(handlers::stream_delete_room))
        .route("/stream/list", get(handlers::stream_list_rooms))
        // Queue endpoints
        .route(
            "/queue/{name}/ws/{consumer_id}",
            get(handlers::queue_websocket),
        ) // WebSocket for continuous consume
        .route("/queue/{name}", post(handlers::queue_create))
        .route("/queue/{name}/publish", post(handlers::queue_publish))
        .route(
            "/queue/{name}/consume/{consumer_id}",
            get(handlers::queue_consume),
        )
        .route("/queue/{name}/ack", post(handlers::queue_ack))
        .route("/queue/{name}/nack", post(handlers::queue_nack))
        .route("/queue/{name}/stats", get(handlers::queue_stats))
        .route("/queue/{name}/purge", post(handlers::queue_purge))
        .route("/queue/{name}", delete(handlers::queue_delete))
        .route("/queue/list", get(handlers::queue_list))
        // Pub/Sub endpoints
        .route("/pubsub/ws", get(handlers::pubsub_websocket)) // WebSocket for subscriptions
        .route("/pubsub/subscribe", post(handlers::pubsub_subscribe)) // Legacy REST (deprecated)
        .route("/pubsub/{topic}/publish", post(handlers::pubsub_publish))
        .route("/pubsub/unsubscribe", post(handlers::pubsub_unsubscribe))
        .route("/pubsub/stats", get(handlers::pubsub_stats))
        .route("/pubsub/topics", get(handlers::pubsub_list_topics))
        .route("/pubsub/{topic}/info", get(handlers::pubsub_topic_info))
        // Partitioned Stream endpoints (Kafka-style)
        .route("/topics", get(handlers::list_topics))
        .route("/topics/{topic}", post(handlers::create_partitioned_topic))
        .route("/topics/{topic}", delete(handlers::delete_topic))
        .route("/topics/{topic}/stats", get(handlers::get_topic_stats))
        .route("/topics/{topic}/publish", post(handlers::publish_to_partition))
        .route(
            "/topics/{topic}/partitions/{partition_id}/consume",
            post(handlers::consume_from_partition),
        )
        // Consumer Group endpoints
        .route("/consumer-groups", get(handlers::list_consumer_groups))
        .route("/consumer-groups/{group_id}", post(handlers::create_consumer_group))
        .route("/consumer-groups/{group_id}/join", post(handlers::join_consumer_group))
        .route(
            "/consumer-groups/{group_id}/members/{member_id}/leave",
            delete(handlers::leave_consumer_group),
        )
        .route(
            "/consumer-groups/{group_id}/members/{member_id}/assignment",
            get(handlers::get_partition_assignment),
        )
        .route(
            "/consumer-groups/{group_id}/members/{member_id}/heartbeat",
            post(handlers::consumer_heartbeat),
        )
        .route(
            "/consumer-groups/{group_id}/offsets/commit",
            post(handlers::commit_offset),
        )
        .route(
            "/consumer-groups/{group_id}/offsets/{partition_id}",
            get(handlers::get_committed_offset),
        )
        .route(
            "/consumer-groups/{group_id}/stats",
            get(handlers::get_consumer_group_stats),
        )
        // StreamableHTTP command endpoint
        .route("/api/v1/command", post(handlers::command_handler))
        // Add state
        .with_state(state);

    // Merge MCP router (stateless) with API router (stateful)
    mcp_router
        .merge(api_router)
        .layer(CompressionLayer::new()) // Gzip compression for responses
        .layer(TraceLayer::new_for_http())
        .layer(cors)
    // NOTE: Rate limiting disabled for now due to Clone requirements
    // Will be implemented with a different approach (e.g., middleware per-route or governor crate)
}

/// Create MCP router with StreamableHTTP service
fn create_mcp_router(state: Arc<AppState>) -> Router {
    use hyper_util::service::TowerToHyperService;
    use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
    use rmcp::transport::streamable_http_server::StreamableHttpService;

    // Create StreamableHTTP service
    let streamable_service = StreamableHttpService::new(
        move || {
            Ok(SynapMcpService {
                state: state.clone(),
            })
        },
        LocalSessionManager::default().into(),
        Default::default(),
    );

    // Convert to hyper service
    let hyper_service = TowerToHyperService::new(streamable_service);

    // Create router with the MCP endpoint
    Router::new().route(
        "/mcp",
        axum::routing::any(move |req: axum::extract::Request| {
            use hyper::service::Service;
            let mut service = hyper_service.clone();
            async move {
                // Forward request to hyper service
                match service.call(req).await {
                    Ok(response) => Ok(response),
                    Err(_) => Err(axum::http::StatusCode::INTERNAL_SERVER_ERROR),
                }
            }
        }),
    )
}
