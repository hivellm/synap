use super::auth_handlers;
use super::handlers::{self, AppState};
use super::mcp_server::SynapMcpService;
use crate::auth::{ApiKeyManager, AuthMiddleware, UserManager};
use crate::core::SynapError;
use crate::hub::{HubClient, HubUserContext, extract_access_key};
use axum::{
    Json, Router,
    response::{IntoResponse, Response},
    routing::{delete, get, post},
};
use std::sync::Arc;
use tower_http::{
    compression::CompressionLayer,
    cors::{Any, CorsLayer},
    trace::TraceLayer,
};
use tracing::{debug, warn};

/// Create the Axum router with all endpoints
/// Paths that must answer without credentials.
///
/// A liveness probe that requires authentication is not a liveness probe: the
/// container HEALTHCHECK, Kubernetes, and any load balancer all probe
/// unauthenticated. `/metrics` is the same contract — a scraper is not a user.
/// The rate limiter exempts them for the same reason.
pub(crate) fn is_public_path(path: &str) -> bool {
    matches!(path, "/health" | "/metrics")
}

/// Whether a Hub tenant may call `path`. An allow-list of the routes whose
/// handlers confine every resource they touch to the caller's tenant (or
/// filter what they list by it); a route added later is refused in Hub mode
/// until it is reviewed for scoping. Everything else answers 403 there:
/// keyspace-wide statistics, server introspection (`/info`, `/slowlog`,
/// `/clients`, `/memory`), `/snapshot`, scripts, the REST transaction (one id
/// shared by every caller), partitioned topics and consumer groups, cluster
/// management and local user and key management.
fn is_tenant_route(path: &str) -> bool {
    let segments: Vec<&str> = path.strip_prefix('/').unwrap_or(path).split('/').collect();
    match segments.as_slice() {
        // Confined per command or tool call.
        ["api", "v1", "command"] | ["mcp"] | ["umicp"] | ["umicp", "discover"] => true,
        ["hub", "quota"] => true,
        [
            "hash" | "set" | "sortedset" | "list" | "hyperloglog" | "bitmap" | "geospatial"
            | "pubsub",
            "stats",
        ] => false,
        ["kv", "ws" | "set" | "msetnx"] | ["kv", "get" | "del", _] => true,
        [
            "kv",
            _,
            "append" | "getrange" | "setrange" | "strlen" | "getset",
        ] => true,
        ["key", "randomkey"] | ["key", _, "type" | "exists" | "rename" | "renamenx" | "copy"] => {
            true
        }
        [
            "hash" | "set" | "sortedset" | "list" | "hyperloglog" | "bitmap" | "geospatial"
            | "stream" | "queue" | "pubsub",
            _,
            ..,
        ] => true,
        _ => false,
    }
}

/// Hub mode's authentication layer, installed instead of local
/// authentication. Every route but `/health` and `/metrics` needs a valid Hub
/// access key (`x-hub-access-key`, or `Authorization: Bearer`): without one, or
/// with one the Hub rejects, the answer is 401, and local credentials are not
/// accepted. Only tenant routes ([`is_tenant_route`]) answer; the rest are 403.
/// The request then runs as the key's tenant: a [`HubUserContext`] that scopes
/// its resources and the tenant's `AuthContext` that authorizes them.
async fn hub_gate(
    hub: &HubClient,
    mut req: axum::extract::Request,
    next: axum::middleware::Next,
) -> Response {
    let client_ip = AuthMiddleware::get_client_ip(&req);
    if is_public_path(req.uri().path()) {
        req.extensions_mut()
            .insert(crate::auth::AuthContext::anonymous(client_ip));
        return next.run(req).await;
    }

    let Some(access_key) = extract_access_key(req.headers()) else {
        return SynapError::Unauthorized("Hub mode: a Hub access key is required".to_string())
            .into_response();
    };
    let (user_id, plan) = match hub.validate_access_key(&access_key).await {
        Ok(owner) => owner,
        Err(SynapError::Unauthorized(reason)) => {
            debug!("Hub access key refused: {}", reason);
            return SynapError::Unauthorized("Invalid or expired Hub access key".to_string())
                .into_response();
        }
        Err(e) => {
            warn!("Hub access key could not be validated: {}", e);
            return (
                axum::http::StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({ "error": e.to_string(), "code": 503 })),
            )
                .into_response();
        }
    };

    if !is_tenant_route(req.uri().path()) {
        return SynapError::Forbidden(format!("{} is not available in Hub mode", req.uri().path()))
            .into_response();
    }

    let tenant = HubUserContext::new(user_id, plan, access_key);
    req.extensions_mut().insert(tenant.auth_context(client_ip));
    req.extensions_mut().insert(tenant);
    next.run(req).await
}

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
        auth_enabled,
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

    // Hub mode authenticates with Hub access keys instead of local credentials.
    let hub_client = state.hub_client.clone();

    // Create MCP router (stateless)
    let state_arc = Arc::new(state.clone());
    let mcp_router = create_mcp_router(state_arc.clone(), mcp_config.clone(), auth_enabled);

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
        .route("/kv/ws", get(handlers::kv_websocket)) // WebSocket for WATCH
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
        .route(
            "/stream/{room}",
            post(handlers::stream_create_room)
                .put(handlers::stream_get_or_create_room)
                .delete(handlers::stream_delete_room),
        )
        .route("/stream/{room}/publish", post(handlers::stream_publish))
        .route(
            "/stream/{room}/consume/{subscriber_id}",
            get(handlers::stream_consume),
        )
        .route("/stream/{room}/stats", get(handlers::stream_room_stats))
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

    // Rate limiting (`server::rate_limit`): the principal layer goes on first
    // so it runs inside authentication, where the principal is known; the
    // peer-address layer goes on after it so it runs before authentication.
    let rate_limiter = rate_limit_config.enabled.then(|| {
        Arc::new(super::rate_limit::RateLimiter::new(
            rate_limit_config.clone(),
        ))
    });
    if let Some(limiter) = &rate_limiter {
        let limiter = Arc::clone(limiter);
        router = router.layer(axum::middleware::from_fn(
            move |req: axum::extract::Request, next: axum::middleware::Next| {
                let limiter = Arc::clone(&limiter);
                async move { super::rate_limit::limit_by_principal(limiter, req, next).await }
            },
        ));
    }

    // Apply authentication middleware: Hub authentication in Hub mode, local
    // authentication otherwise (its behavior depends on auth_enabled).
    if let Some(hub) = hub_client {
        router = router.layer(axum::middleware::from_fn(
            move |req: axum::extract::Request, next: axum::middleware::Next| {
                let hub = Arc::clone(&hub);
                async move { hub_gate(&hub, req, next).await }
            },
        ));
    } else if let Some(auth) = auth_middleware {
        let auth_clone = auth.clone();
        router = router.layer(axum::middleware::from_fn(
            move |mut req: axum::extract::Request, next: axum::middleware::Next| {
                let auth = auth_clone.clone();
                async move {
                    let client_ip = AuthMiddleware::get_client_ip(&req);

                    // `/health` and `/metrics` are declared "always public" at
                    // their route definitions, but this middleware is a layer
                    // over the whole router, so the declaration was never
                    // enforced: under `require_auth` they answered 401 like
                    // anything else.
                    //
                    // That made the container's own HEALTHCHECK fail — it sends
                    // an unauthenticated GET /health — so every authenticated
                    // deployment reported `unhealthy` and an orchestrator would
                    // restart a server that was serving perfectly. A liveness
                    // probe cannot require credentials; the RPC port already
                    // answers PING before authentication for the same reason.
                    if is_public_path(req.uri().path()) {
                        req.extensions_mut()
                            .insert(crate::auth::AuthContext::anonymous(client_ip));
                        return Ok(next.run(req).await);
                    }

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

    // Outside authentication: limited clients are refused before their
    // credentials are checked.
    if let Some(limiter) = rate_limiter {
        router = router.layer(axum::middleware::from_fn(
            move |req: axum::extract::Request, next: axum::middleware::Next| {
                let limiter = Arc::clone(&limiter);
                async move { super::rate_limit::limit_by_peer(limiter, req, next).await }
            },
        ));
        tracing::info!(
            "Rate limiting enforced: {} req/s, burst {}, per client address before \
             authentication and per authenticated principal (/health and /metrics exempt)",
            rate_limit_config.requests_per_second,
            rate_limit_config.burst_size
        );
    } else {
        tracing::info!("Rate limiting disabled: requests are not rate limited");
    }

    router
        .layer(CompressionLayer::new()) // Gzip compression for responses
        .layer(TraceLayer::new_for_http())
        .layer(cors)
}

/// The credentials behind an MCP request: the authenticated user and, when it
/// came through one, the API key. A session is bound to exactly these.
#[derive(Debug, Clone, PartialEq, Eq)]
struct McpPrincipal {
    user: Option<String>,
    api_key: Option<String>,
}

impl McpPrincipal {
    fn of(req: &axum::extract::Request) -> Option<Self> {
        req.extensions()
            .get::<crate::auth::AuthContext>()
            .map(|ctx| Self {
                user: ctx.user_id.clone(),
                api_key: ctx.api_key_id.clone(),
            })
    }
}

/// Owner of every live legacy MCP session (`mcp-session-id` → principal).
#[derive(Clone, Default)]
struct McpSessionBindings {
    owners: Arc<parking_lot::Mutex<std::collections::HashMap<String, Option<McpPrincipal>>>>,
}

impl McpSessionBindings {
    /// Bindings kept before dead sessions are pruned.
    const PRUNE_ABOVE: usize = 10_000;

    /// Whether `principal` may use session `id`. A session not seen yet is
    /// bound to its first user.
    fn admit(&self, id: &str, principal: &Option<McpPrincipal>) -> bool {
        let mut owners = self.owners.lock();
        match owners.get(id) {
            Some(owner) => owner == principal,
            None => {
                owners.insert(id.to_owned(), principal.clone());
                true
            }
        }
    }

    /// Record the owner of a session the server just created, pruning the
    /// bindings of sessions that ended without a `DELETE` once there are many.
    async fn bind(
        &self,
        id: &str,
        principal: Option<McpPrincipal>,
        sessions: &rmcp::transport::streamable_http_server::session::local::LocalSessionManager,
    ) {
        use rmcp::transport::streamable_http_server::session::SessionManager;

        let known: Vec<String> = {
            let mut owners = self.owners.lock();
            owners.entry(id.to_owned()).or_insert(principal);
            if owners.len() <= Self::PRUNE_ABOVE {
                return;
            }
            owners.keys().cloned().collect()
        };
        let mut dead = Vec::new();
        for session in known {
            if !matches!(
                sessions.has_session(&session.as_str().into()).await,
                Ok(true)
            ) {
                dead.push(session);
            }
        }
        let mut owners = self.owners.lock();
        for session in dead {
            owners.remove(&session);
        }
    }

    fn forget(&self, id: &str) {
        self.owners.lock().remove(id);
    }
}

/// Create MCP router with StreamableHTTP service and authentication
fn create_mcp_router(
    state: Arc<AppState>,
    mcp_config: crate::config::McpConfig,
    auth_enabled: bool,
) -> Router {
    use hyper_util::service::TowerToHyperService;
    use rmcp::transport::streamable_http_server::StreamableHttpService;
    use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;

    // Create StreamableHTTP service. Each tool call reads its own request's
    // identity from the HTTP parts rmcp hands the handler (`SynapMcpService::
    // caller`), so nothing here passes identity through ambient state.
    let session_manager = Arc::new(LocalSessionManager::default());
    let streamable_service = StreamableHttpService::new(
        move || {
            Ok(SynapMcpService {
                state: state.clone(),
                mcp_config: mcp_config.clone(),
                auth_enabled,
            })
        },
        Arc::clone(&session_manager),
        Default::default(),
    );

    // Convert to hyper service
    let hyper_service = TowerToHyperService::new(streamable_service);

    // A legacy MCP session belongs to the identity that initialized it: a
    // request carrying its `mcp-session-id` under other credentials is refused.
    let bindings = McpSessionBindings::default();

    // Create router with the MCP endpoint
    let router = Router::new().route(
        "/mcp",
        axum::routing::any(move |req: axum::extract::Request| {
            use hyper::service::Service;
            let service = hyper_service.clone();
            let bindings = bindings.clone();
            let session_manager = Arc::clone(&session_manager);
            async move {
                let principal = McpPrincipal::of(&req);
                let session = req
                    .headers()
                    .get("mcp-session-id")
                    .and_then(|v| v.to_str().ok())
                    .map(str::to_owned);
                if let Some(id) = &session
                    && !bindings.admit(id, &principal)
                {
                    debug!("MCP: session {} reused under another identity", id);
                    return Err(axum::http::StatusCode::FORBIDDEN);
                }
                let closing = req.method() == axum::http::Method::DELETE;

                let response = service
                    .call(req)
                    .await
                    .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

                // The request that created a session (initialize) owns it.
                if let Some(created) = response
                    .headers()
                    .get("mcp-session-id")
                    .and_then(|v| v.to_str().ok())
                {
                    bindings.bind(created, principal, &session_manager).await;
                }
                if closing && let Some(id) = session {
                    bindings.forget(&id);
                }
                Ok(response)
            }
        }),
    );

    // Authentication is the main router's layer, which covers `/mcp` like
    // every other route; a second layer here would verify each credential
    // twice (bcrypt included) per MCP request.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hub_mode_allows_tenant_routes_only() {
        for path in [
            "/api/v1/command",
            "/mcp",
            "/umicp",
            "/umicp/discover",
            "/hub/quota",
            "/kv/set",
            "/kv/get/config",
            "/kv/del/config",
            "/kv/ws",
            "/kv/config/append",
            "/kv/msetnx",
            "/key/config/rename",
            "/key/randomkey",
            "/hash/h/set",
            "/hash/h/field",
            "/set/a/move/b",
            "/set/inter",
            "/sortedset/z/zadd",
            "/sortedset/zunionstore",
            "/list/l/rpoplpush/m",
            "/hyperloglog/h/pfmerge",
            "/bitmap/d/bitop",
            "/geospatial/g/geoadd",
            "/stream/room",
            "/stream/room/ws/sub",
            "/stream/list",
            "/queue/jobs",
            "/queue/jobs/stats",
            "/queue/jobs/ws/c1",
            "/queue/list",
            "/pubsub/ws",
            "/pubsub/news/publish",
            "/pubsub/topics",
        ] {
            assert!(is_tenant_route(path), "{path} should be a tenant route");
        }
        for path in [
            "/kv/stats",
            "/hash/stats",
            "/set/stats",
            "/sortedset/stats",
            "/list/stats",
            "/hyperloglog/stats",
            "/bitmap/stats",
            "/geospatial/stats",
            "/pubsub/stats",
            "/info",
            "/slowlog",
            "/clients",
            "/memory/config/usage",
            "/snapshot",
            "/transaction/multi",
            "/transaction/exec",
            "/script/eval",
            "/script/flush",
            "/topics",
            "/topics/t/publish",
            "/consumer-groups/g/join",
            "/cluster/info",
            "/cluster/nodes",
            "/auth/login",
            "/auth/users",
            "/health/extra",
            "/",
        ] {
            assert!(!is_tenant_route(path), "{path} must be refused in Hub mode");
        }
    }
}
