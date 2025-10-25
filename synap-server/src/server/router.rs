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
pub fn create_router(state: AppState, rate_limit_config: crate::config::RateLimitConfig) -> Router {
    // CORS configuration
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // Create MCP router (stateless)
    let state_arc = Arc::new(state.clone());
    let mcp_router = create_mcp_router(state_arc.clone());

    // Create UMICP router
    let umicp_router = create_umicp_router(state_arc.clone());

    // Create main API router with state
    let api_router = Router::new()
        // Health check
        .route("/health", get(handlers::health_check))
        // Prometheus metrics
        .route("/metrics", get(super::metrics_handler::metrics_handler))
        // KV endpoints
        .route("/kv/ws", get(handlers::kv_websocket)) // WebSocket for WATCH (future)
        .route("/kv/set", post(handlers::kv_set))
        .route("/kv/get/{key}", get(handlers::kv_get))
        .route("/kv/del/{key}", delete(handlers::kv_delete))
        .route("/kv/stats", get(handlers::kv_stats))
        // Hash endpoints
        .route("/hash/{key}/set", post(handlers::hash_set))
        .route("/hash/{key}/{field}", get(handlers::hash_get))
        .route("/hash/{key}/getall", get(handlers::hash_getall))
        .route("/hash/{key}/keys", get(handlers::hash_keys))
        .route("/hash/{key}/vals", get(handlers::hash_vals))
        .route("/hash/{key}/len", get(handlers::hash_len))
        .route("/hash/{key}/mset", post(handlers::hash_mset))
        .route("/hash/{key}/mget", post(handlers::hash_mget))
        .route("/hash/{key}/del", delete(handlers::hash_del))
        .route("/hash/{key}/{field}/exists", get(handlers::hash_exists))
        .route("/hash/{key}/incrby", post(handlers::hash_incrby))
        .route("/hash/{key}/incrbyfloat", post(handlers::hash_incrbyfloat))
        .route("/hash/{key}/setnx", post(handlers::hash_setnx))
        .route("/hash/stats", get(handlers::hash_stats))
        // Set endpoints
        .route("/set/{key}/add", post(handlers::set_add))
        .route("/set/{key}/rem", post(handlers::set_rem))
        .route("/set/{key}/ismember", post(handlers::set_ismember))
        .route("/set/{key}/members", get(handlers::set_members))
        .route("/set/{key}/card", get(handlers::set_card))
        .route("/set/{key}/pop", post(handlers::set_pop))
        .route("/set/{key}/randmember", get(handlers::set_randmember))
        .route("/set/{source}/move/{destination}", post(handlers::set_move))
        .route("/set/inter", post(handlers::set_inter))
        .route("/set/union", post(handlers::set_union))
        .route("/set/diff", post(handlers::set_diff))
        .route("/set/stats", get(handlers::set_stats))
        // Sorted Set endpoints
        .route("/sortedset/{key}/zadd", post(handlers::sortedset_zadd))
        .route("/sortedset/{key}/zrem", post(handlers::sortedset_zrem))
        .route(
            "/sortedset/{key}/{member}/zscore",
            get(handlers::sortedset_zscore),
        )
        .route("/sortedset/{key}/zcard", get(handlers::sortedset_zcard))
        .route(
            "/sortedset/{key}/zincrby",
            post(handlers::sortedset_zincrby),
        )
        .route("/sortedset/{key}/zrange", get(handlers::sortedset_zrange))
        .route(
            "/sortedset/{key}/zrevrange",
            get(handlers::sortedset_zrevrange),
        )
        .route(
            "/sortedset/{key}/{member}/zrank",
            get(handlers::sortedset_zrank),
        )
        .route(
            "/sortedset/zinterstore",
            post(handlers::sortedset_zinterstore),
        )
        .route(
            "/sortedset/zunionstore",
            post(handlers::sortedset_zunionstore),
        )
        .route("/sortedset/stats", get(handlers::sortedset_stats))
        // List endpoints
        .route("/list/{key}/lpush", post(handlers::list_lpush))
        .route("/list/{key}/lpushx", post(handlers::list_lpushx))
        .route("/list/{key}/rpush", post(handlers::list_rpush))
        .route("/list/{key}/rpushx", post(handlers::list_rpushx))
        .route("/list/{key}/lpop", post(handlers::list_lpop))
        .route("/list/{key}/rpop", post(handlers::list_rpop))
        .route("/list/{key}/range", get(handlers::list_range))
        .route("/list/{key}/len", get(handlers::list_len))
        .route("/list/{key}/index/{index}", get(handlers::list_index))
        .route("/list/{key}/set", post(handlers::list_set))
        .route("/list/{key}/trim", post(handlers::list_trim))
        .route("/list/{key}/rem", post(handlers::list_rem))
        .route("/list/{key}/insert", post(handlers::list_insert))
        .route(
            "/list/{source}/rpoplpush/{destination}",
            post(handlers::list_rpoplpush),
        )
        .route("/list/stats", get(handlers::list_stats))
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
        .route(
            "/topics/{topic}/publish",
            post(handlers::publish_to_partition),
        )
        .route(
            "/topics/{topic}/partitions/{partition_id}/consume",
            post(handlers::consume_from_partition),
        )
        // Consumer Group endpoints
        .route("/consumer-groups", get(handlers::list_consumer_groups))
        .route(
            "/consumer-groups/{group_id}",
            post(handlers::create_consumer_group),
        )
        .route(
            "/consumer-groups/{group_id}/join",
            post(handlers::join_consumer_group),
        )
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

    // Merge all routers: MCP + UMICP + API
    let router = mcp_router
        .merge(umicp_router) // UMICP protocol endpoints (/umicp, /umicp/discover)
        .merge(api_router) // Main API endpoints
        .layer(CompressionLayer::new()) // Gzip compression for responses
        .layer(TraceLayer::new_for_http())
        .layer(cors);

    // NOTE: Rate limiting implementation available but disabled by default
    // The rate_limit::RateLimiter is fully implemented with token bucket algorithm
    // To enable, set rate_limit.enabled = true in config.yml
    // Implementation details in src/server/rate_limit.rs

    if rate_limit_config.enabled {
        tracing::warn!(
            "Rate limiting configured ({} req/s, burst: {}) but not active - requires middleware integration",
            rate_limit_config.requests_per_second,
            rate_limit_config.burst_size
        );
    } else {
        tracing::info!("Rate limiting disabled (default)");
    }

    router
}

/// Create MCP router with StreamableHTTP service
fn create_mcp_router(state: Arc<AppState>) -> Router {
    use hyper_util::service::TowerToHyperService;
    use rmcp::transport::streamable_http_server::StreamableHttpService;
    use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;

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
            let service = hyper_service.clone();
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

/// Create UMICP router with discovery and message endpoints
fn create_umicp_router(state: Arc<AppState>) -> Router {
    use super::umicp::{UmicpState, transport};

    let umicp_state = UmicpState { app_state: state };

    Router::new()
        .route("/umicp", post(transport::umicp_handler))
        .route("/umicp/discover", get(transport::umicp_discover_handler))
        .with_state(umicp_state)
}
