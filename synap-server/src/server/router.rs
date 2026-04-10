use super::auth_handlers;
use super::handlers::{self, AppState};
use super::mcp_server::SynapMcpService;
use crate::auth::{ApiKeyManager, AuthMiddleware, UserManager};
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
use tracing::debug;

/// Create the Axum router with all endpoints
pub fn create_router(
    state: AppState,
    rate_limit_config: crate::config::RateLimitConfig,
    mcp_config: crate::config::McpConfig,
    user_manager: Arc<UserManager>,
    api_key_manager: Arc<ApiKeyManager>,
    auth_enabled: bool,
    require_auth: bool,
) -> Router {
    // CORS configuration
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // Create auth state for auth handlers
    let auth_state = auth_handlers::AuthState {
        user_manager: user_manager.clone(),
        api_key_manager: api_key_manager.clone(),
    };

    // Create authentication middleware
    let auth_middleware = if auth_enabled {
        Some(AuthMiddleware::new(
            (*user_manager).clone(),
            (*api_key_manager).clone(),
            require_auth,
        ))
    } else {
        None
    };

    // Create MCP router (stateless)
    let state_arc = Arc::new(state.clone());
    let mcp_router = create_mcp_router(
        state_arc.clone(),
        mcp_config.clone(),
        user_manager.clone(),
        api_key_manager.clone(),
        auth_enabled,
        require_auth,
    );

    // Create UMICP router
    let umicp_router = create_umicp_router(state_arc.clone(), mcp_config.clone());

    // Create auth router (no auth required for auth endpoints themselves)
    let auth_router = Router::new()
        // Authentication endpoints
        .route("/auth/login", post(auth_handlers::auth_login))
        .route("/auth/me", get(auth_handlers::auth_me))
        // API key management
        .route("/auth/keys", post(auth_handlers::auth_create_key))
        .route("/auth/keys", get(auth_handlers::auth_list_keys))
        .route("/auth/keys/{id}", delete(auth_handlers::auth_revoke_key))
        // User management (admin only)
        .route("/auth/users", post(auth_handlers::auth_create_user))
        .route("/auth/users", get(auth_handlers::auth_list_users))
        .route("/auth/users/{username}", get(auth_handlers::auth_get_user))
        .route(
            "/auth/users/{username}",
            delete(auth_handlers::auth_delete_user),
        )
        .route(
            "/auth/users/{username}/password",
            post(auth_handlers::auth_change_password),
        )
        .route(
            "/auth/users/{username}/enable",
            post(auth_handlers::auth_enable_user),
        )
        .route(
            "/auth/users/{username}/disable",
            post(auth_handlers::auth_disable_user),
        )
        .route(
            "/auth/users/{username}/roles",
            post(auth_handlers::auth_grant_role),
        )
        .route(
            "/auth/users/{username}/roles/{role}",
            delete(auth_handlers::auth_revoke_role),
        )
        // Role management
        .route("/auth/roles", get(auth_handlers::auth_list_roles))
        .with_state(auth_state);

    // Create main API router with state
    let api_router = Router::new()
        // Health check (always public)
        .route("/health", get(handlers::health_check))
        // Prometheus metrics (always public)
        .route("/metrics", get(super::metrics_handler::metrics_handler))
        // KV endpoints
        .route("/kv/ws", get(handlers::kv_websocket)) // WebSocket for WATCH (future)
        .route("/kv/set", post(handlers::kv_set))
        .route("/kv/get/{key}", get(handlers::kv_get))
        .route("/kv/del/{key}", delete(handlers::kv_delete))
        .route("/kv/stats", get(handlers::kv_stats))
        // String extension endpoints
        .route("/kv/{key}/append", post(handlers::kv_append))
        .route("/kv/{key}/getrange", get(handlers::kv_getrange))
        .route("/kv/{key}/setrange", post(handlers::kv_setrange))
        .route("/kv/{key}/strlen", get(handlers::kv_strlen))
        .route("/kv/{key}/getset", post(handlers::kv_getset))
        .route("/kv/msetnx", post(handlers::kv_msetnx))
        // Key Management endpoints
        .route("/key/{key}/type", get(handlers::key_type))
        .route("/key/{key}/exists", get(handlers::key_exists))
        .route("/key/{key}/rename", post(handlers::key_rename))
        .route("/key/{key}/renamenx", post(handlers::key_renamenx))
        .route("/key/{key}/copy", post(handlers::key_copy))
        .route("/key/randomkey", get(handlers::key_randomkey))
        // Monitoring endpoints
        .route("/info", get(handlers::info))
        .route("/slowlog", get(handlers::slowlog))
        .route("/memory/{key}/usage", get(handlers::memory_usage))
        .route("/clients", get(handlers::client_list))
        // Transaction endpoints
        .route("/transaction/multi", post(handlers::transaction_multi))
        .route("/transaction/exec", post(handlers::transaction_exec))
        .route("/transaction/discard", post(handlers::transaction_discard))
        .route("/transaction/watch", post(handlers::transaction_watch))
        .route("/transaction/unwatch", post(handlers::transaction_unwatch))
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
        .route(
            "/sortedset/zdiffstore",
            post(handlers::sortedset_zdiffstore),
        )
        .route(
            "/sortedset/{key}/{member}/zrevrank",
            get(handlers::sortedset_zrevrank),
        )
        .route("/sortedset/{key}/zcount", get(handlers::sortedset_zcount))
        .route(
            "/sortedset/{key}/zmscore",
            post(handlers::sortedset_zmscore),
        )
        .route(
            "/sortedset/{key}/zrangebyscore",
            get(handlers::sortedset_zrangebyscore),
        )
        .route(
            "/sortedset/{key}/zpopmin",
            post(handlers::sortedset_zpopmin),
        )
        .route(
            "/sortedset/{key}/zpopmax",
            post(handlers::sortedset_zpopmax),
        )
        .route(
            "/sortedset/{key}/zremrangebyrank",
            post(handlers::sortedset_zremrangebyrank),
        )
        .route(
            "/sortedset/{key}/zremrangebyscore",
            post(handlers::sortedset_zremrangebyscore),
        )
        .route("/sortedset/stats", get(handlers::sortedset_stats))
        // Lua scripting endpoints
        .route("/script/eval", post(handlers::script_eval))
        .route("/script/evalsha", post(handlers::script_evalsha))
        .route("/script/load", post(handlers::script_load))
        .route("/script/exists", post(handlers::script_exists))
        .route("/script/flush", post(handlers::script_flush))
        .route("/script/kill", post(handlers::script_kill))
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
        // HyperLogLog endpoints
        .route(
            "/hyperloglog/{key}/pfadd",
            post(handlers::hyperloglog_pfadd),
        )
        .route(
            "/hyperloglog/{key}/pfcount",
            get(handlers::hyperloglog_pfcount),
        )
        .route(
            "/hyperloglog/{destination}/pfmerge",
            post(handlers::hyperloglog_pfmerge),
        )
        .route("/hyperloglog/stats", get(handlers::hyperloglog_stats))
        // Bitmap endpoints
        .route("/bitmap/{key}/setbit", post(handlers::bitmap_setbit))
        .route(
            "/bitmap/{key}/getbit/{offset}",
            get(handlers::bitmap_getbit),
        )
        .route("/bitmap/{key}/bitcount", get(handlers::bitmap_bitcount))
        .route("/bitmap/{key}/bitpos", get(handlers::bitmap_bitpos))
        .route("/bitmap/{destination}/bitop", post(handlers::bitmap_bitop))
        .route("/bitmap/{key}/bitfield", post(handlers::bitmap_bitfield))
        .route("/bitmap/stats", get(handlers::bitmap_stats))
        // Geospatial endpoints
        .route(
            "/geospatial/{key}/geoadd",
            post(handlers::geospatial_geoadd),
        )
        .route(
            "/geospatial/{key}/geodist/{member1}/{member2}",
            get(handlers::geospatial_geodist),
        )
        .route(
            "/geospatial/{key}/georadius",
            get(handlers::geospatial_georadius),
        )
        .route(
            "/geospatial/{key}/georadiusbymember/{member}",
            get(handlers::geospatial_georadiusbymember),
        )
        .route(
            "/geospatial/{key}/geopos",
            post(handlers::geospatial_geopos),
        )
        .route(
            "/geospatial/{key}/geohash",
            post(handlers::geospatial_geohash),
        )
        .route(
            "/geospatial/{key}/geosearch",
            post(handlers::geospatial_geosearch),
        )
        .route("/geospatial/stats", get(handlers::geospatial_stats))
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
        // Cluster management endpoints
        .route("/cluster/info", get(handlers::cluster_info))
        .route("/cluster/nodes", get(handlers::cluster_nodes))
        .route("/cluster/nodes", post(handlers::cluster_add_node))
        .route("/cluster/nodes/{node_id}", get(handlers::cluster_node_info))
        .route(
            "/cluster/nodes/{node_id}",
            delete(handlers::cluster_remove_node),
        )
        .route("/cluster/slots", get(handlers::cluster_slots))
        .route(
            "/cluster/slots/assign",
            post(handlers::cluster_assign_slots),
        )
        .route(
            "/cluster/migration/start",
            post(handlers::cluster_start_migration),
        )
        .route(
            "/cluster/migration/complete",
            post(handlers::cluster_complete_migration),
        )
        .route(
            "/cluster/migration/{slot}",
            get(handlers::cluster_migration_status),
        );

    // HiveHub Integration endpoints (conditionally compiled)

    let api_router = api_router.route("/hub/quota", get(handlers::hub_quota_stats));

    // Add state to API router
    let api_router = api_router.with_state(state);

    // Merge all routers: Auth + MCP + UMICP + API
    let mut router = mcp_router
        .merge(umicp_router) // UMICP protocol endpoints (/umicp, /umicp/discover)
        .merge(auth_router) // Authentication endpoints
        .merge(api_router); // Main API endpoints

    // Apply authentication middleware (always apply, but behavior depends on auth_enabled)
    if let Some(auth) = auth_middleware {
        let auth_clone = auth.clone();
        router = router.layer(axum::middleware::from_fn(
            move |mut req: axum::extract::Request, next: axum::middleware::Next| {
                let auth = auth_clone.clone();
                async move {
                    let client_ip = AuthMiddleware::get_client_ip(&req);
                    debug!("Processing authentication for IP: {}", client_ip);

                    // Try API Key authentication first (from header or query param)
                    match AuthMiddleware::authenticate_api_key(&auth, &req, client_ip) {
                        Ok(Some(auth_context)) => {
                            req.extensions_mut().insert(auth_context);
                            return Ok(next.run(req).await);
                        }
                        Err(_) => {
                            // API key provided but invalid - return 401
                            debug!("Invalid API key provided");
                            return Err(axum::http::StatusCode::UNAUTHORIZED);
                        }
                        Ok(None) => {
                            // No API key provided, continue to Basic Auth
                        }
                    }

                    // Try Basic Auth
                    match AuthMiddleware::authenticate_basic(&auth, &req, client_ip) {
                        Ok(Some(auth_context)) => {
                            req.extensions_mut().insert(auth_context);
                            return Ok(next.run(req).await);
                        }
                        Err(_) => {
                            // Basic Auth credentials provided but invalid - return 401
                            debug!("Invalid Basic Auth credentials");
                            return Err(axum::http::StatusCode::UNAUTHORIZED);
                        }
                        Ok(None) => {
                            // No Basic Auth provided, continue
                        }
                    }

                    // No authentication provided
                    if auth.require_auth {
                        debug!("Authentication required but not provided");
                        return Err(axum::http::StatusCode::UNAUTHORIZED);
                    }

                    // Allow anonymous access
                    req.extensions_mut()
                        .insert(crate::auth::AuthContext::anonymous(client_ip));
                    Ok(next.run(req).await)
                }
            },
        ));
    } else {
        // Auth disabled - insert anonymous context with all permissions (admin-like)
        router = router.layer(axum::middleware::from_fn(
            move |mut req: axum::extract::Request, next: axum::middleware::Next| {
                async move {
                    let client_ip = AuthMiddleware::get_client_ip(&req);
                    // Create anonymous context with admin privileges when auth is disabled
                    let mut anonymous_ctx = crate::auth::AuthContext::anonymous(client_ip);
                    anonymous_ctx.is_admin = true; // Grant all permissions when auth is disabled
                    req.extensions_mut().insert(anonymous_ctx);
                    next.run(req).await
                }
            },
        ));
    }

    router = router
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

/// Create MCP router with StreamableHTTP service and authentication
fn create_mcp_router(
    state: Arc<AppState>,
    mcp_config: crate::config::McpConfig,
    user_manager: Arc<UserManager>,
    api_key_manager: Arc<ApiKeyManager>,
    auth_enabled: bool,
    require_auth: bool,
) -> Router {
    use hyper_util::service::TowerToHyperService;
    use rmcp::transport::streamable_http_server::StreamableHttpService;
    use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;

    // Create authentication middleware for MCP
    let auth_middleware = if auth_enabled {
        Some(AuthMiddleware::new(
            (*user_manager).clone(),
            (*api_key_manager).clone(),
            require_auth,
        ))
    } else {
        None
    };

    // Create StreamableHTTP service
    let streamable_service = StreamableHttpService::new(
        move || {
            Ok(SynapMcpService {
                state: state.clone(),
                mcp_config: mcp_config.clone(),
            })
        },
        LocalSessionManager::default().into(),
        Default::default(),
    );

    // Convert to hyper service
    let hyper_service = TowerToHyperService::new(streamable_service);

    // Create router with the MCP endpoint
    let mut router = Router::new().route(
        "/mcp",
        axum::routing::any(move |req: axum::extract::Request| {
            use hyper::service::Service;
            let service = hyper_service.clone();
            async move {
                // Extract AuthContext from request extensions before passing to hyper service
                let auth_context = req.extensions().get::<crate::auth::AuthContext>().cloned();

                // Set auth context in thread-local storage for MCP handlers
                if let Some(ctx) = auth_context {
                    crate::auth::set_auth_context(ctx);
                }

                // Forward request to hyper service
                let result = match service.call(req).await {
                    Ok(response) => Ok(response),
                    Err(_) => Err(axum::http::StatusCode::INTERNAL_SERVER_ERROR),
                };

                // Clear auth context after request processing
                crate::auth::clear_auth_context();

                result
            }
        }),
    );

    // Apply authentication middleware if enabled
    if let Some(auth) = auth_middleware {
        let auth_clone = auth.clone();
        router = router.layer(axum::middleware::from_fn(
            move |mut req: axum::extract::Request, next: axum::middleware::Next| {
                let auth = auth_clone.clone();
                async move {
                    let client_ip = AuthMiddleware::get_client_ip(&req);
                    debug!("MCP: Processing authentication for IP: {}", client_ip);

                    // Try API Key authentication first
                    match AuthMiddleware::authenticate_api_key(&auth, &req, client_ip) {
                        Ok(Some(auth_context)) => {
                            req.extensions_mut().insert(auth_context);
                            return Ok(next.run(req).await);
                        }
                        Err(_) => {
                            debug!("MCP: Invalid API key provided");
                            return Err(axum::http::StatusCode::UNAUTHORIZED);
                        }
                        Ok(None) => {}
                    }

                    // Try Basic Auth
                    match AuthMiddleware::authenticate_basic(&auth, &req, client_ip) {
                        Ok(Some(auth_context)) => {
                            req.extensions_mut().insert(auth_context);
                            return Ok(next.run(req).await);
                        }
                        Err(_) => {
                            debug!("MCP: Invalid Basic Auth credentials");
                            return Err(axum::http::StatusCode::UNAUTHORIZED);
                        }
                        Ok(None) => {}
                    }

                    // No authentication provided
                    if auth.require_auth {
                        debug!("MCP: Authentication required but not provided");
                        return Err(axum::http::StatusCode::UNAUTHORIZED);
                    }

                    // Allow anonymous access
                    req.extensions_mut()
                        .insert(crate::auth::AuthContext::anonymous(client_ip));
                    Ok(next.run(req).await)
                }
            },
        ));
    } else {
        // Auth disabled - insert anonymous context
        router = router.layer(axum::middleware::from_fn(
            move |mut req: axum::extract::Request, next: axum::middleware::Next| {
                async move {
                    let client_ip = AuthMiddleware::get_client_ip(&req);
                    let mut anonymous_ctx = crate::auth::AuthContext::anonymous(client_ip);
                    anonymous_ctx.is_admin = true; // Grant all permissions when auth is disabled
                    req.extensions_mut().insert(anonymous_ctx);
                    next.run(req).await
                }
            },
        ));
    }

    router
}

/// Create UMICP router with discovery and message endpoints
fn create_umicp_router(state: Arc<AppState>, mcp_config: crate::config::McpConfig) -> Router {
    use super::umicp::{UmicpState, transport};

    let umicp_state = UmicpState {
        app_state: state,
        mcp_config,
    };

    Router::new()
        .route("/umicp", post(transport::umicp_handler))
        .route("/umicp/discover", get(transport::umicp_discover_handler))
        .with_state(umicp_state)
}
