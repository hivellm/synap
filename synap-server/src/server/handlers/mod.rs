use crate::auth::{Action, AuthContextExtractor, require_permission, require_resource_permission};
use crate::core::types::{Expiry, SetOptions};
use crate::core::{
    GeospatialStore, HashStore, HyperLogLogStore, KVStore, KeyManager, Message, QueueManager,
    SortedSetStore, SynapError, TransactionManager,
};
use crate::monitoring::{
    InfoSection, KeyspaceInfo, MemoryInfo, MemoryUsage, ReplicationInfo, ServerInfo, StatsInfo,
};
use crate::protocol::{Request, Response};
use crate::scripting::{ScriptExecContext, ScriptManager};
use axum::{
    Json,
    extract::{
        Path, Query, State,
        ws::{WebSocket, WebSocketUpgrade},
    },
    response::{IntoResponse, Response as AxumResponse},
};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::net::SocketAddr;
use std::sync::Arc;
use std::{collections::HashMap, time::Duration};
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

pub mod admin_cmd;
pub mod bitmap;
pub mod cluster;
pub mod geospatial;
pub mod hash;
pub mod hll;
pub mod kv;
pub mod kv_cmd;
pub mod list;
pub mod partition;
pub mod pubsub;
pub mod queue;
pub mod script;
pub mod set;
pub mod sorted_set;
pub mod stream;
pub mod websocket;

pub use bitmap::*;
pub use cluster::*;
pub use geospatial::*;
pub use hash::*;
pub use hll::*;
pub use kv::*;
pub use list::*;
pub use partition::*;
pub use pubsub::*;
pub use queue::*;
pub use script::*;
pub use set::*;
pub use sorted_set::*;
pub use stream::*;
pub use websocket::*;

/// Application state shared across handlers
#[derive(Clone)]
pub struct AppState {
    pub kv_store: Arc<KVStore>,
    pub hash_store: Arc<HashStore>,
    pub list_store: Arc<crate::core::ListStore>,
    pub set_store: Arc<crate::core::SetStore>,
    pub sorted_set_store: Arc<SortedSetStore>,
    pub hyperloglog_store: Arc<HyperLogLogStore>,
    pub bitmap_store: Arc<crate::core::BitmapStore>,
    pub geospatial_store: Arc<GeospatialStore>,
    pub queue_manager: Option<Arc<QueueManager>>,
    pub stream_manager: Option<Arc<crate::core::StreamManager>>,
    pub partition_manager: Option<Arc<crate::core::PartitionManager>>,
    pub consumer_group_manager: Option<Arc<crate::core::ConsumerGroupManager>>,
    pub pubsub_router: Option<Arc<crate::core::PubSubRouter>>,
    pub persistence: Option<Arc<crate::persistence::PersistenceLayer>>,
    pub monitoring: Arc<crate::monitoring::MonitoringManager>,
    pub transaction_manager: Arc<TransactionManager>,
    pub script_manager: Arc<ScriptManager>,
    pub client_list_manager: Arc<crate::monitoring::ClientListManager>,
    /// Optional cluster topology (cluster mode)
    pub cluster_topology: Option<Arc<crate::cluster::topology::ClusterTopology>>,
    /// Optional cluster migration manager (cluster mode)
    pub cluster_migration: Option<Arc<crate::cluster::migration::SlotMigrationManager>>,
    /// Optional Hub client (Hub integration mode)
    pub hub_client: Option<Arc<crate::hub::HubClient>>,
}

// Request/Response types for REST API
#[derive(Debug, Deserialize)]
pub struct SetRequest {
    pub key: String,
    pub value: serde_json::Value,
    /// Legacy TTL in seconds. Superseded by `expiry` for new callers.
    pub ttl: Option<u64>,
    /// Rich expiry specification (EX / PX / EXAT / PXAT). Takes precedence
    /// over `ttl` when both are present.
    #[serde(default)]
    pub expiry: Option<Expiry>,
    /// Only set if the key does NOT exist (NX). Mutually exclusive with `xx`.
    #[serde(default)]
    pub nx: bool,
    /// Only set if the key DOES exist (XX). Mutually exclusive with `nx`.
    #[serde(default)]
    pub xx: bool,
    /// Retain the existing TTL of the key (KEEPTTL). Ignores `ttl`/`expiry`.
    #[serde(default)]
    pub keepttl: bool,
    /// Return the previous value in the response (GET).
    #[serde(default)]
    pub get: bool,
}

#[derive(Debug, Serialize)]
pub struct SetResponse {
    pub success: bool,
    pub key: String,
    /// `false` when an NX/XX condition was not satisfied.
    pub written: bool,
    /// Previous value (populated only when `get: true` was requested).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub old_value: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum GetResponse {
    String(String),
    Bytes(Vec<u8>),
    NotFound(serde_json::Value),
}

#[derive(Debug, Serialize)]
pub struct DeleteResponse {
    pub deleted: bool,
    pub key: String,
}

#[derive(Debug, Serialize)]
pub struct StatsResponse {
    pub total_keys: usize,
    pub total_memory_bytes: usize,
    pub operations: OperationStats,
    pub hit_rate: f64,
}

#[derive(Debug, Serialize)]
pub struct OperationStats {
    pub gets: u64,
    pub sets: u64,
    pub dels: u64,
    pub hits: u64,
    pub misses: u64,
}

#[derive(Debug, Deserialize)]
pub struct EvalScriptRequest {
    pub script: String,
    #[serde(default)]
    pub keys: Vec<String>,
    #[serde(default)]
    pub args: Vec<serde_json::Value>,
    pub timeout_ms: Option<u64>,
}

#[derive(Debug, Serialize)]
pub struct EvalScriptResponse {
    pub result: serde_json::Value,
    pub sha1: String,
}

#[derive(Debug, Deserialize)]
pub struct EvalShaRequest {
    pub sha1: String,
    #[serde(default)]
    pub keys: Vec<String>,
    #[serde(default)]
    pub args: Vec<serde_json::Value>,
    pub timeout_ms: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct ScriptLoadRequest {
    pub script: String,
}

#[derive(Debug, Serialize)]
pub struct ScriptLoadResponse {
    pub sha1: String,
}

#[derive(Debug, Deserialize)]
pub struct ScriptExistsRequest {
    pub hashes: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct ScriptExistsResponse {
    pub exists: Vec<bool>,
}

#[derive(Debug, Serialize)]
pub struct ScriptFlushResponse {
    pub cleared: usize,
}

#[derive(Debug, Serialize)]
pub struct ScriptKillResponse {
    pub terminated: bool,
}

/// Health check endpoint
pub async fn health_check() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "healthy",
        "service": "synap",
        "version": env!("CARGO_PKG_VERSION")
    }))
}

// Queue REST API types
#[derive(Debug, Deserialize)]
pub struct CreateQueueRequest {
    pub max_depth: Option<usize>,
    pub ack_deadline_secs: Option<u64>,
    pub default_max_retries: Option<u32>,
    pub default_priority: Option<u8>,
}

#[derive(Debug, Deserialize)]
pub struct PublishRequest {
    pub payload: Vec<u8>,
    pub priority: Option<u8>,
    pub max_retries: Option<u32>,
    pub headers: Option<HashMap<String, String>>,
}

#[derive(Debug, Serialize)]
pub struct PublishResponse {
    pub message_id: String,
}

#[derive(Debug, Serialize)]
pub struct ConsumeResponse {
    pub message_id: Option<String>,
    pub payload: Option<Vec<u8>>,
    pub priority: Option<u8>,
    pub retry_count: Option<u32>,
    pub max_retries: Option<u32>,
    pub headers: Option<HashMap<String, String>>,
}

#[derive(Debug, Deserialize)]
pub struct AckRequest {
    pub message_id: String,
}

#[derive(Debug, Deserialize)]
pub struct NackRequest {
    pub message_id: String,
    pub requeue: bool,
}

// Stream request/response types
#[derive(Debug, Deserialize)]
pub struct StreamPublishRequest {
    pub event: String,
    pub data: serde_json::Value,
}

#[derive(Debug, Serialize)]
pub struct StreamPublishResponse {
    pub offset: u64,
    pub room: String,
}

#[derive(Debug, Serialize)]
pub struct StreamConsumeResponse {
    pub events: Vec<crate::core::StreamEvent>,
    pub next_offset: u64,
}

pub fn default_unit() -> String {
    "m".to_string()
}

pub async fn command_handler(
    State(state): State<AppState>,
    Json(request): Json<Request>,
) -> Result<Json<Response>, SynapError> {
    debug!(
        "Command: {} (request_id={})",
        request.command, request.request_id
    );

    let response = handle_command(state, request).await?;
    Ok(Json(response))
}

/// Handle individual commands
async fn handle_command(state: AppState, request: Request) -> Result<Response, SynapError> {
    let request_id = request.request_id.clone();

    let result = match request.command.as_str() {
        "kv.set" => kv_cmd::handle_kv_set_cmd(&state, &request).await,
        "kv.get" => kv_cmd::handle_kv_get_cmd(state.kv_store.clone(), &request).await,
        "kv.del" => kv_cmd::handle_kv_del_cmd(&state, &request).await,
        "kv.exists" => kv_cmd::handle_kv_exists_cmd(state.kv_store.clone(), &request).await,
        "kv.incr" => kv_cmd::handle_kv_incr_cmd(&state, &request).await,
        "kv.decr" => kv_cmd::handle_kv_decr_cmd(&state, &request).await,
        "kv.mset" => kv_cmd::handle_kv_mset_cmd(&state, &request).await,
        "kv.mget" => kv_cmd::handle_kv_mget_cmd(state.kv_store.clone(), &request).await,
        "kv.mdel" => kv_cmd::handle_kv_mdel_cmd(&state, &request).await,
        "kv.scan" => kv_cmd::handle_kv_scan_cmd(state.kv_store.clone(), &request).await,
        "kv.keys" => kv_cmd::handle_kv_keys_cmd(state.kv_store.clone(), &request).await,
        "kv.dbsize" => kv_cmd::handle_kv_dbsize_cmd(state.kv_store.clone(), &request).await,
        "kv.flushdb" => kv_cmd::handle_kv_flushdb_cmd(state.kv_store.clone(), &request).await,
        "kv.flushall" => kv_cmd::handle_kv_flushall_cmd(state.kv_store.clone(), &request).await,
        "kv.expire" => kv_cmd::handle_kv_expire_cmd(state.kv_store.clone(), &request).await,
        "kv.ttl" => kv_cmd::handle_kv_ttl_cmd(state.kv_store.clone(), &request).await,
        "kv.persist" => kv_cmd::handle_kv_persist_cmd(state.kv_store.clone(), &request).await,
        "kv.stats" => kv_cmd::handle_kv_stats_cmd(state.kv_store.clone(), &request).await,
        // String extension commands
        "kv.append" => kv_cmd::handle_kv_append_cmd(&state, &request).await,
        "kv.getrange" => kv_cmd::handle_kv_getrange_cmd(state.kv_store.clone(), &request).await,
        "kv.setrange" => kv_cmd::handle_kv_setrange_cmd(&state, &request).await,
        "kv.strlen" => kv_cmd::handle_kv_strlen_cmd(state.kv_store.clone(), &request).await,
        "kv.getset" => kv_cmd::handle_kv_getset_cmd(&state, &request).await,
        "kv.msetnx" => kv_cmd::handle_kv_msetnx_cmd(&state, &request).await,
        // Key Management commands
        "key.type" => kv_cmd::handle_key_type_cmd(state.clone(), &request).await,
        "key.exists" => kv_cmd::handle_key_exists_cmd(state.clone(), &request).await,
        "key.rename" => kv_cmd::handle_key_rename_cmd(&state, &request).await,
        "key.renamenx" => kv_cmd::handle_key_renamenx_cmd(&state, &request).await,
        "key.copy" => kv_cmd::handle_key_copy_cmd(&state, &request).await,
        "key.randomkey" => kv_cmd::handle_key_randomkey_cmd(state.clone(), &request).await,
        // Monitoring commands
        "info" => admin_cmd::handle_info_cmd(state.clone(), &request).await,
        "slowlog.get" => admin_cmd::handle_slowlog_get_cmd(state.clone(), &request).await,
        "slowlog.reset" => admin_cmd::handle_slowlog_reset_cmd(state.clone(), &request).await,
        "memory.usage" => admin_cmd::handle_memory_usage_cmd(state.clone(), &request).await,
        "client.list" => admin_cmd::handle_client_list_cmd(state.clone(), &request).await,
        // Transaction commands
        "transaction.multi" => {
            admin_cmd::handle_transaction_multi_cmd(state.clone(), &request).await
        }
        "transaction.exec" => admin_cmd::handle_transaction_exec_cmd(state.clone(), &request).await,
        "transaction.discard" => {
            admin_cmd::handle_transaction_discard_cmd(state.clone(), &request).await
        }
        "transaction.watch" => {
            admin_cmd::handle_transaction_watch_cmd(state.clone(), &request).await
        }
        "transaction.unwatch" => {
            admin_cmd::handle_transaction_unwatch_cmd(state.clone(), &request).await
        }
        // Hash commands
        "hash.set" => hash::handle_hash_set_cmd(&state, &request).await,
        "hash.get" => hash::handle_hash_get_cmd(&state, &request).await,
        "hash.getall" => hash::handle_hash_getall_cmd(&state, &request).await,
        "hash.del" => hash::handle_hash_del_cmd(&state, &request).await,
        "hash.exists" => hash::handle_hash_exists_cmd(&state, &request).await,
        "hash.len" => hash::handle_hash_len_cmd(&state, &request).await,
        "hash.keys" => hash::handle_hash_keys_cmd(&state, &request).await,
        "hash.vals" => hash::handle_hash_vals_cmd(&state, &request).await,
        "hash.mset" => hash::handle_hash_mset_cmd(&state, &request).await,
        "hash.mget" => hash::handle_hash_mget_cmd(&state, &request).await,
        "hash.incrby" => hash::handle_hash_incrby_cmd(&state, &request).await,
        "hash.incrbyfloat" => hash::handle_hash_incrbyfloat_cmd(&state, &request).await,
        "hash.setnx" => hash::handle_hash_setnx_cmd(&state, &request).await,
        "hash.stats" => hash::handle_hash_stats_cmd(&state, &request).await,
        // List commands
        "list.lpush" => list::handle_list_lpush_cmd(&state, &request).await,
        "list.lpushx" => list::handle_list_lpushx_cmd(&state, &request).await,
        "list.rpush" => list::handle_list_rpush_cmd(&state, &request).await,
        "list.rpushx" => list::handle_list_rpushx_cmd(&state, &request).await,
        "list.lpop" => list::handle_list_lpop_cmd(&state, &request).await,
        "list.rpop" => list::handle_list_rpop_cmd(&state, &request).await,
        "list.lrange" => list::handle_list_lrange_cmd(&state, &request).await,
        "list.range" => list::handle_list_lrange_cmd(&state, &request).await, // Alias for SDK compatibility
        "list.llen" => list::handle_list_llen_cmd(&state, &request).await,
        "list.len" => list::handle_list_llen_cmd(&state, &request).await, // Alias for SDK compatibility
        "list.lindex" => list::handle_list_lindex_cmd(&state, &request).await,
        "list.index" => list::handle_list_lindex_cmd(&state, &request).await, // Alias for SDK compatibility
        "list.lset" => list::handle_list_lset_cmd(&state, &request).await,
        "list.set" => list::handle_list_lset_cmd(&state, &request).await, // Alias for SDK compatibility
        "list.ltrim" => list::handle_list_ltrim_cmd(&state, &request).await,
        "list.trim" => list::handle_list_ltrim_cmd(&state, &request).await, // Alias for SDK compatibility
        "list.lrem" => list::handle_list_lrem_cmd(&state, &request).await,
        "list.linsert" => list::handle_list_linsert_cmd(&state, &request).await,
        "list.lpos" => list::handle_list_lpos_cmd(&state, &request).await,
        "list.rpoplpush" => list::handle_list_rpoplpush_cmd(&state, &request).await,
        "list.stats" => list::handle_list_stats_cmd(&state, &request).await,
        "hyperloglog.pfadd" => hll::handle_hyperloglog_pfadd_cmd(&state, &request).await,
        "hyperloglog.pfcount" => hll::handle_hyperloglog_pfcount_cmd(&state, &request).await,
        "hyperloglog.pfmerge" => hll::handle_hyperloglog_pfmerge_cmd(&state, &request).await,
        "hyperloglog.stats" => hll::handle_hyperloglog_stats_cmd(&state, &request).await,
        "bitmap.setbit" => bitmap::handle_bitmap_setbit_cmd(&state, &request).await,
        "bitmap.getbit" => bitmap::handle_bitmap_getbit_cmd(&state, &request).await,
        "bitmap.bitcount" => bitmap::handle_bitmap_bitcount_cmd(&state, &request).await,
        "bitmap.bitpos" => bitmap::handle_bitmap_bitpos_cmd(&state, &request).await,
        "bitmap.bitop" => bitmap::handle_bitmap_bitop_cmd(&state, &request).await,
        "bitmap.bitfield" => bitmap::handle_bitmap_bitfield_cmd(&state, &request).await,
        "bitmap.stats" => bitmap::handle_bitmap_stats_cmd(&state, &request).await,
        "geospatial.geoadd" => geospatial::handle_geospatial_geoadd_cmd(&state, &request).await,
        "geospatial.geodist" => geospatial::handle_geospatial_geodist_cmd(&state, &request).await,
        "geospatial.georadius" => {
            geospatial::handle_geospatial_georadius_cmd(&state, &request).await
        }
        "geospatial.georadiusbymember" => {
            geospatial::handle_geospatial_georadiusbymember_cmd(&state, &request).await
        }
        "geospatial.geopos" => geospatial::handle_geospatial_geopos_cmd(&state, &request).await,
        "geospatial.geohash" => geospatial::handle_geospatial_geohash_cmd(&state, &request).await,
        "geospatial.geosearch" => {
            geospatial::handle_geospatial_geosearch_cmd(&state, &request).await
        }
        "geospatial.stats" => geospatial::handle_geospatial_stats_cmd(&state, &request).await,
        "queue.create" => queue::handle_queue_create_cmd(&state, &request).await,
        "queue.delete" => queue::handle_queue_delete_cmd(&state, &request).await,
        "queue.publish" => queue::handle_queue_publish_cmd(&state, &request).await,
        "queue.consume" => queue::handle_queue_consume_cmd(&state, &request).await,
        "queue.ack" => queue::handle_queue_ack_cmd(&state, &request).await,
        "queue.nack" => queue::handle_queue_nack_cmd(&state, &request).await,
        "queue.list" => queue::handle_queue_list_cmd(&state, &request).await,
        "queue.stats" => queue::handle_queue_stats_cmd(&state, &request).await,
        "queue.purge" => queue::handle_queue_purge_cmd(&state, &request).await,
        // Set commands
        "set.add" => set::handle_set_add_cmd(&state, &request).await,
        "set.rem" => set::handle_set_rem_cmd(&state, &request).await,
        "set.ismember" => set::handle_set_ismember_cmd(&state, &request).await,
        "set.members" => set::handle_set_members_cmd(&state, &request).await,
        "set.size" => set::handle_set_size_cmd(&state, &request).await,
        "set.card" => set::handle_set_size_cmd(&state, &request).await, // Alias for SDK compatibility (Redis-style)
        "set.pop" => set::handle_set_pop_cmd(&state, &request).await,
        "set.randmember" => set::handle_set_randmember_cmd(&state, &request).await,
        "set.move" => set::handle_set_move_cmd(&state, &request).await,
        "set.inter" => set::handle_set_inter_cmd(&state, &request).await,
        "set.interstore" => set::handle_set_inter_cmd(&state, &request).await, // Alias for SDK compatibility (note: returns result, doesn't store)
        "set.union" => set::handle_set_union_cmd(&state, &request).await,
        "set.diff" => set::handle_set_diff_cmd(&state, &request).await,
        "set.stats" => set::handle_set_stats_cmd(&state, &request).await,
        // Sorted Set commands
        "sortedset.zadd" => sorted_set::handle_sortedset_zadd_cmd(&state, &request).await,
        "sortedset.zrem" => sorted_set::handle_sortedset_zrem_cmd(&state, &request).await,
        "sortedset.zscore" => sorted_set::handle_sortedset_zscore_cmd(&state, &request).await,
        "sortedset.zcard" => sorted_set::handle_sortedset_zcard_cmd(&state, &request).await,
        "sortedset.zincrby" => sorted_set::handle_sortedset_zincrby_cmd(&state, &request).await,
        "sortedset.zrange" => sorted_set::handle_sortedset_zrange_cmd(&state, &request).await,
        "sortedset.zrevrange" => sorted_set::handle_sortedset_zrevrange_cmd(&state, &request).await,
        "sortedset.zrank" => sorted_set::handle_sortedset_zrank_cmd(&state, &request).await,
        "sortedset.zrevrank" => sorted_set::handle_sortedset_zrevrank_cmd(&state, &request).await,
        "sortedset.zcount" => sorted_set::handle_sortedset_zcount_cmd(&state, &request).await,
        "sortedset.zpopmin" => sorted_set::handle_sortedset_zpopmin_cmd(&state, &request).await,
        "sortedset.zpopmax" => sorted_set::handle_sortedset_zpopmax_cmd(&state, &request).await,
        "sortedset.zrangebyscore" => {
            sorted_set::handle_sortedset_zrangebyscore_cmd(&state, &request).await
        }
        "sortedset.zremrangebyrank" => {
            sorted_set::handle_sortedset_zremrangebyrank_cmd(&state, &request).await
        }
        "sortedset.zremrangebyscore" => {
            sorted_set::handle_sortedset_zremrangebyscore_cmd(&state, &request).await
        }
        "sortedset.zinterstore" => {
            sorted_set::handle_sortedset_zinterstore_cmd(&state, &request).await
        }
        "sortedset.zunionstore" => {
            sorted_set::handle_sortedset_zunionstore_cmd(&state, &request).await
        }
        "sortedset.zdiffstore" => {
            sorted_set::handle_sortedset_zdiffstore_cmd(&state, &request).await
        }
        "sortedset.zmscore" => sorted_set::handle_sortedset_zmscore_cmd(&state, &request).await,
        "sortedset.stats" => sorted_set::handle_sortedset_stats_cmd(&state, &request).await,
        "script.eval" => script::handle_script_eval_cmd(&state, &request).await,
        "script.evalsha" => script::handle_script_evalsha_cmd(&state, &request).await,
        "script.load" => script::handle_script_load_cmd(&state, &request).await,
        "script.exists" => script::handle_script_exists_cmd(&state, &request).await,
        "script.flush" => script::handle_script_flush_cmd(&state, &request).await,
        "script.kill" => script::handle_script_kill_cmd(&state, &request).await,
        "pubsub.subscribe" => pubsub::handle_pubsub_subscribe_cmd(&state, &request).await,
        "pubsub.publish" => pubsub::handle_pubsub_publish_cmd(&state, &request).await,
        "pubsub.unsubscribe" => pubsub::handle_pubsub_unsubscribe_cmd(&state, &request).await,
        "pubsub.stats" => pubsub::handle_pubsub_stats_cmd(&state, &request).await,
        "pubsub.topics" => pubsub::handle_pubsub_topics_cmd(&state, &request).await,
        "pubsub.info" => pubsub::handle_pubsub_info_cmd(&state, &request).await,
        "stream.create" => stream::handle_stream_create_cmd(&state, &request).await,
        "stream.publish" => stream::handle_stream_publish_cmd(&state, &request).await,
        "stream.consume" => stream::handle_stream_consume_cmd(&state, &request).await,
        "stream.stats" => stream::handle_stream_stats_cmd(&state, &request).await,
        "stream.list" => stream::handle_stream_list_cmd(&state, &request).await,
        "stream.delete" => stream::handle_stream_delete_cmd(&state, &request).await,
        _ => Err(SynapError::UnknownCommand(request.command.clone())),
    };

    match result {
        Ok(payload) => Ok(Response {
            success: true,
            request_id,
            payload: Some(payload),
            error: None,
        }),
        Err(e) => {
            error!("Command error: {}", e);
            Ok(Response {
                success: false,
                request_id,
                payload: None,
                error: Some(e.to_string()),
            })
        }
    }
}
