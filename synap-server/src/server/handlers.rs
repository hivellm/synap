use crate::core::{HashStore, KVStore, Message, QueueManager, SortedSetStore, SynapError};
use crate::protocol::{Request, Response};
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
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

/// Application state shared across handlers
#[derive(Clone)]
pub struct AppState {
    pub kv_store: Arc<KVStore>,
    pub hash_store: Arc<HashStore>,
    pub list_store: Arc<crate::core::ListStore>,
    pub set_store: Arc<crate::core::SetStore>,
    pub sorted_set_store: Arc<SortedSetStore>,
    pub queue_manager: Option<Arc<QueueManager>>,
    pub stream_manager: Option<Arc<crate::core::StreamManager>>,
    pub partition_manager: Option<Arc<crate::core::PartitionManager>>,
    pub consumer_group_manager: Option<Arc<crate::core::ConsumerGroupManager>>,
    pub pubsub_router: Option<Arc<crate::core::PubSubRouter>>,
    pub persistence: Option<Arc<crate::persistence::PersistenceLayer>>,
}

// Request/Response types for REST API
#[derive(Debug, Deserialize)]
pub struct SetRequest {
    pub key: String,
    pub value: serde_json::Value,
    pub ttl: Option<u64>,
}

#[derive(Debug, Serialize)]
pub struct SetResponse {
    pub success: bool,
    pub key: String,
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

/// SET endpoint - store a key-value pair
pub async fn kv_set(
    State(state): State<AppState>,
    Json(req): Json<SetRequest>,
) -> Result<Json<SetResponse>, SynapError> {
    debug!("REST SET key={}", req.key);

    let value_bytes = serde_json::to_vec(&req.value)
        .map_err(|e| SynapError::SerializationError(e.to_string()))?;

    // Set in KV store
    state
        .kv_store
        .set(&req.key, value_bytes.clone(), req.ttl)
        .await?;

    // Log to WAL (async, non-blocking)
    if let Some(ref persistence) = state.persistence {
        if let Err(e) = persistence
            .log_kv_set(req.key.clone(), value_bytes, req.ttl)
            .await
        {
            error!("Failed to log KV SET to WAL: {}", e);
            // Don't fail the request, data is already in memory
        }
    }

    Ok(Json(SetResponse {
        success: true,
        key: req.key,
    }))
}

/// GET endpoint - retrieve a value by key
pub async fn kv_get(
    State(state): State<AppState>,
    Path(key): Path<String>,
    axum::extract::Query(params): axum::extract::Query<HashMap<String, String>>,
) -> Result<Json<GetResponse>, SynapError> {
    let return_type = params.get("type").map(|s| s.as_str()).unwrap_or("string");
    debug!("REST GET key={}, type={}", key, return_type);

    let value_bytes = state.kv_store.get(&key).await?;

    if let Some(bytes) = value_bytes {
        match return_type {
            "bytes" => Ok(Json(GetResponse::Bytes(bytes))),
            _ => {
                // Default: return as string
                let value_str = String::from_utf8(bytes)
                    .unwrap_or_else(|e| format!("<binary data: {} bytes>", e.as_bytes().len()));

                Ok(Json(GetResponse::String(value_str)))
            }
        }
    } else {
        // Key not found
        Ok(Json(GetResponse::NotFound(
            serde_json::json!({"error": "Key not found"}),
        )))
    }
}

/// DELETE endpoint - delete a key
pub async fn kv_delete(
    State(state): State<AppState>,
    Path(key): Path<String>,
) -> Result<Json<DeleteResponse>, SynapError> {
    debug!("REST DELETE key={}", key);

    let deleted = state.kv_store.delete(&key).await?;

    // Log to WAL if persistence is enabled
    if deleted {
        if let Some(ref persistence) = state.persistence {
            if let Err(e) = persistence.log_kv_del(vec![key.clone()]).await {
                error!("Failed to log KV DELETE to WAL: {}", e);
            }
        }
    }

    Ok(Json(DeleteResponse { deleted, key }))
}

/// SNAPSHOT endpoint - manually trigger a snapshot
pub async fn trigger_snapshot(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, SynapError> {
    debug!("REST SNAPSHOT TRIGGER");

    if let Some(ref persistence) = state.persistence {
        persistence
            .maybe_snapshot(
                &state.kv_store,
                state.queue_manager.as_deref(),
                state.stream_manager.as_deref(),
            )
            .await
            .map_err(|e| SynapError::InternalError(format!("Snapshot failed: {}", e)))?;

        Ok(Json(serde_json::json!({
            "success": true,
            "message": "Snapshot triggered successfully"
        })))
    } else {
        Err(SynapError::InvalidRequest(
            "Persistence is disabled".to_string(),
        ))
    }
}

/// STATS endpoint - get store statistics
pub async fn kv_stats(State(state): State<AppState>) -> Result<Json<StatsResponse>, SynapError> {
    debug!("REST STATS");

    let stats = state.kv_store.stats().await;

    Ok(Json(StatsResponse {
        total_keys: stats.total_keys,
        total_memory_bytes: stats.total_memory_bytes,
        operations: OperationStats {
            gets: stats.gets,
            sets: stats.sets,
            dels: stats.dels,
            hits: stats.hits,
            misses: stats.misses,
        },
        hit_rate: stats.hit_rate(),
    }))
}

// ==================== Hash REST Endpoints ====================

// Hash request/response types
#[derive(Debug, Deserialize)]
pub struct HashSetRequest {
    pub field: String,
    pub value: serde_json::Value,
}

#[derive(Debug, Deserialize)]
pub struct HashMSetRequest {
    pub fields: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct HashMGetRequest {
    pub fields: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct HashDelRequest {
    pub fields: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct HashIncrByRequest {
    pub field: String,
    pub increment: i64,
}

#[derive(Debug, Deserialize)]
pub struct HashIncrByFloatRequest {
    pub field: String,
    pub increment: f64,
}

#[derive(Debug, Serialize)]
pub struct HashSetResponse {
    pub created: bool,
    pub key: String,
    pub field: String,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum HashGetResponse {
    Found(serde_json::Value),
    NotFound { found: bool },
}

#[derive(Debug, Serialize)]
pub struct HashDelResponse {
    pub deleted: usize,
    pub key: String,
}

#[derive(Debug, Serialize)]
pub struct HashStatsResponse {
    pub total_hashes: usize,
    pub total_fields: usize,
    pub operations: HashOperationStats,
}

#[derive(Debug, Serialize)]
pub struct HashOperationStats {
    pub hset_count: u64,
    pub hget_count: u64,
    pub hdel_count: u64,
}

/// POST /hash/:key/set - Set a field in hash
pub async fn hash_set(
    State(state): State<AppState>,
    Path(key): Path<String>,
    Json(req): Json<HashSetRequest>,
) -> Result<Json<HashSetResponse>, SynapError> {
    debug!("REST HSET key={} field={}", key, req.field);

    let value = serde_json::to_vec(&req.value)
        .map_err(|e| SynapError::InvalidValue(format!("Failed to serialize value: {}", e)))?;

    let created = state.hash_store.hset(&key, &req.field, value)?;

    Ok(Json(HashSetResponse {
        created,
        key,
        field: req.field,
    }))
}

/// GET /hash/:key/:field - Get a field from hash
pub async fn hash_get(
    State(state): State<AppState>,
    Path((key, field)): Path<(String, String)>,
) -> Result<Json<HashGetResponse>, SynapError> {
    debug!("REST HGET key={} field={}", key, field);

    match state.hash_store.hget(&key, &field)? {
        Some(value) => {
            let json_value: serde_json::Value =
                serde_json::from_slice(&value).unwrap_or_else(|_| {
                    serde_json::Value::String(String::from_utf8_lossy(&value).to_string())
                });
            Ok(Json(HashGetResponse::Found(json_value)))
        }
        None => Ok(Json(HashGetResponse::NotFound { found: false })),
    }
}

/// GET /hash/:key/getall - Get all fields from hash
pub async fn hash_getall(
    State(state): State<AppState>,
    Path(key): Path<String>,
) -> Result<Json<HashMap<String, serde_json::Value>>, SynapError> {
    debug!("REST HGETALL key={}", key);

    let all = state.hash_store.hgetall(&key)?;

    let result: HashMap<String, serde_json::Value> = all
        .into_iter()
        .map(|(k, v)| {
            let json_value: serde_json::Value = serde_json::from_slice(&v).unwrap_or_else(|_| {
                serde_json::Value::String(String::from_utf8_lossy(&v).to_string())
            });
            (k, json_value)
        })
        .collect();

    Ok(Json(result))
}

/// GET /hash/:key/keys - Get all field names from hash
pub async fn hash_keys(
    State(state): State<AppState>,
    Path(key): Path<String>,
) -> Result<Json<Vec<String>>, SynapError> {
    debug!("REST HKEYS key={}", key);
    let keys = state.hash_store.hkeys(&key)?;
    Ok(Json(keys))
}

/// GET /hash/:key/vals - Get all values from hash
pub async fn hash_vals(
    State(state): State<AppState>,
    Path(key): Path<String>,
) -> Result<Json<Vec<serde_json::Value>>, SynapError> {
    debug!("REST HVALS key={}", key);

    let values = state.hash_store.hvals(&key)?;
    let result: Vec<serde_json::Value> = values
        .into_iter()
        .map(|v| {
            serde_json::from_slice(&v).unwrap_or_else(|_| {
                serde_json::Value::String(String::from_utf8_lossy(&v).to_string())
            })
        })
        .collect();

    Ok(Json(result))
}

/// GET /hash/:key/len - Get number of fields in hash
pub async fn hash_len(
    State(state): State<AppState>,
    Path(key): Path<String>,
) -> Result<Json<serde_json::Value>, SynapError> {
    debug!("REST HLEN key={}", key);
    let len = state.hash_store.hlen(&key)?;
    Ok(Json(json!({ "length": len })))
}

/// POST /hash/:key/mset - Set multiple fields
pub async fn hash_mset(
    State(state): State<AppState>,
    Path(key): Path<String>,
    Json(req): Json<HashMSetRequest>,
) -> Result<Json<serde_json::Value>, SynapError> {
    debug!("REST HMSET key={} fields={}", key, req.fields.len());

    let fields: HashMap<String, Vec<u8>> = req
        .fields
        .into_iter()
        .map(|(k, v)| {
            let bytes = serde_json::to_vec(&v).map_err(|e| {
                SynapError::InvalidValue(format!("Failed to serialize field {}: {}", k, e))
            })?;
            Ok((k, bytes))
        })
        .collect::<Result<HashMap<_, _>, SynapError>>()?;

    state.hash_store.hmset(&key, fields)?;

    Ok(Json(json!({ "success": true, "key": key })))
}

/// POST /hash/:key/mget - Get multiple fields
pub async fn hash_mget(
    State(state): State<AppState>,
    Path(key): Path<String>,
    Json(req): Json<HashMGetRequest>,
) -> Result<Json<Vec<Option<serde_json::Value>>>, SynapError> {
    debug!("REST HMGET key={} fields={:?}", key, req.fields);

    let values = state.hash_store.hmget(&key, &req.fields)?;

    let result: Vec<Option<serde_json::Value>> = values
        .into_iter()
        .map(|opt_v| {
            opt_v.map(|v| {
                serde_json::from_slice(&v).unwrap_or_else(|_| {
                    serde_json::Value::String(String::from_utf8_lossy(&v).to_string())
                })
            })
        })
        .collect();

    Ok(Json(result))
}

/// DELETE /hash/:key - Delete fields from hash
pub async fn hash_del(
    State(state): State<AppState>,
    Path(key): Path<String>,
    Json(req): Json<HashDelRequest>,
) -> Result<Json<HashDelResponse>, SynapError> {
    debug!("REST HDEL key={} fields={:?}", key, req.fields);

    let deleted = state.hash_store.hdel(&key, &req.fields)?;

    Ok(Json(HashDelResponse { deleted, key }))
}

/// GET /hash/:key/:field/exists - Check if field exists
pub async fn hash_exists(
    State(state): State<AppState>,
    Path((key, field)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>, SynapError> {
    debug!("REST HEXISTS key={} field={}", key, field);

    let exists = state.hash_store.hexists(&key, &field)?;

    Ok(Json(json!({ "exists": exists })))
}

/// POST /hash/:key/incrby - Increment field by integer
pub async fn hash_incrby(
    State(state): State<AppState>,
    Path(key): Path<String>,
    Json(req): Json<HashIncrByRequest>,
) -> Result<Json<serde_json::Value>, SynapError> {
    debug!(
        "REST HINCRBY key={} field={} increment={}",
        key, req.field, req.increment
    );

    let new_value = state.hash_store.hincrby(&key, &req.field, req.increment)?;

    Ok(Json(json!({ "value": new_value })))
}

/// POST /hash/:key/incrbyfloat - Increment field by float
pub async fn hash_incrbyfloat(
    State(state): State<AppState>,
    Path(key): Path<String>,
    Json(req): Json<HashIncrByFloatRequest>,
) -> Result<Json<serde_json::Value>, SynapError> {
    debug!(
        "REST HINCRBYFLOAT key={} field={} increment={}",
        key, req.field, req.increment
    );

    let new_value = state
        .hash_store
        .hincrbyfloat(&key, &req.field, req.increment)?;

    Ok(Json(json!({ "value": new_value })))
}

/// POST /hash/:key/setnx - Set field only if it doesn't exist
pub async fn hash_setnx(
    State(state): State<AppState>,
    Path(key): Path<String>,
    Json(req): Json<HashSetRequest>,
) -> Result<Json<serde_json::Value>, SynapError> {
    debug!("REST HSETNX key={} field={}", key, req.field);

    let value = serde_json::to_vec(&req.value)
        .map_err(|e| SynapError::InvalidValue(format!("Failed to serialize value: {}", e)))?;

    let created = state.hash_store.hsetnx(&key, &req.field, value)?;

    Ok(Json(
        json!({ "created": created, "key": key, "field": req.field }),
    ))
}

/// GET /hash/stats - Get hash statistics
pub async fn hash_stats(
    State(state): State<AppState>,
) -> Result<Json<HashStatsResponse>, SynapError> {
    debug!("REST HASH STATS");

    let stats = state.hash_store.stats();

    Ok(Json(HashStatsResponse {
        total_hashes: stats.total_hashes,
        total_fields: stats.total_fields,
        operations: HashOperationStats {
            hset_count: stats.hset_count,
            hget_count: stats.hget_count,
            hdel_count: stats.hdel_count,
        },
    }))
}

// ==================== Event Stream REST Endpoints ====================

/// Create stream room
pub async fn stream_create_room(
    State(state): State<AppState>,
    Path(room_name): Path<String>,
) -> Result<Json<serde_json::Value>, SynapError> {
    debug!("REST CREATE STREAM ROOM: {}", room_name);

    let stream_manager = state
        .stream_manager
        .as_ref()
        .ok_or_else(|| SynapError::InvalidRequest("Stream system disabled".to_string()))?;

    stream_manager
        .create_room(&room_name)
        .await
        .map_err(SynapError::InvalidRequest)?;

    Ok(Json(serde_json::json!({
        "success": true,
        "room": room_name
    })))
}

/// Publish event to stream room
pub async fn stream_publish(
    State(state): State<AppState>,
    Path(room_name): Path<String>,
    Json(req): Json<StreamPublishRequest>,
) -> Result<Json<StreamPublishResponse>, SynapError> {
    debug!("REST STREAM PUBLISH to room: {}", room_name);

    let stream_manager = state
        .stream_manager
        .as_ref()
        .ok_or_else(|| SynapError::InvalidRequest("Stream system disabled".to_string()))?;

    let data_bytes =
        serde_json::to_vec(&req.data).map_err(|e| SynapError::SerializationError(e.to_string()))?;

    let offset = stream_manager
        .publish(&room_name, &req.event, data_bytes)
        .await
        .map_err(SynapError::InvalidRequest)?;

    Ok(Json(StreamPublishResponse {
        offset,
        room: room_name,
    }))
}

/// Consume events from stream room
pub async fn stream_consume(
    State(state): State<AppState>,
    Path((room_name, subscriber_id)): Path<(String, String)>,
    axum::extract::Query(params): axum::extract::Query<HashMap<String, String>>,
) -> Result<Json<StreamConsumeResponse>, SynapError> {
    debug!(
        "REST STREAM CONSUME from room: {}, subscriber: {}",
        room_name, subscriber_id
    );

    let stream_manager = state
        .stream_manager
        .as_ref()
        .ok_or_else(|| SynapError::InvalidRequest("Stream system disabled".to_string()))?;

    let from_offset = params
        .get("from_offset")
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0);

    let limit = params
        .get("limit")
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(100);

    let events = stream_manager
        .consume(&room_name, &subscriber_id, from_offset, limit)
        .await
        .map_err(SynapError::InvalidRequest)?;

    let next_offset = events.last().map(|e| e.offset + 1).unwrap_or(from_offset);

    Ok(Json(StreamConsumeResponse {
        events,
        next_offset,
    }))
}

/// Get stream room statistics
pub async fn stream_room_stats(
    State(state): State<AppState>,
    Path(room_name): Path<String>,
) -> Result<Json<crate::core::RoomStats>, SynapError> {
    debug!("REST STREAM STATS for room: {}", room_name);

    let stream_manager = state
        .stream_manager
        .as_ref()
        .ok_or_else(|| SynapError::InvalidRequest("Stream system disabled".to_string()))?;

    let stats = stream_manager
        .room_stats(&room_name)
        .await
        .map_err(SynapError::InvalidRequest)?;

    Ok(Json(stats))
}

/// List all stream rooms
pub async fn stream_list_rooms(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, SynapError> {
    debug!("REST STREAM LIST ROOMS");

    let stream_manager = state
        .stream_manager
        .as_ref()
        .ok_or_else(|| SynapError::InvalidRequest("Stream system disabled".to_string()))?;

    let rooms = stream_manager.list_rooms().await;

    Ok(Json(serde_json::json!({
        "rooms": rooms,
        "count": rooms.len()
    })))
}

/// Delete stream room
pub async fn stream_delete_room(
    State(state): State<AppState>,
    Path(room_name): Path<String>,
) -> Result<Json<serde_json::Value>, SynapError> {
    debug!("REST DELETE STREAM ROOM: {}", room_name);

    let stream_manager = state
        .stream_manager
        .as_ref()
        .ok_or_else(|| SynapError::InvalidRequest("Stream system disabled".to_string()))?;

    stream_manager
        .delete_room(&room_name)
        .await
        .map_err(SynapError::InvalidRequest)?;

    Ok(Json(serde_json::json!({
        "success": true,
        "room": room_name
    })))
}

// ==================== Queue REST Endpoints ====================

/// Create queue endpoint
pub async fn queue_create(
    State(state): State<AppState>,
    Path(queue_name): Path<String>,
    Json(req): Json<CreateQueueRequest>,
) -> Result<Json<serde_json::Value>, SynapError> {
    debug!("REST CREATE QUEUE: {}", queue_name);

    let queue_manager = state
        .queue_manager
        .as_ref()
        .ok_or_else(|| SynapError::InvalidRequest("Queue system disabled".to_string()))?;

    let config = if req.max_depth.is_some() || req.ack_deadline_secs.is_some() {
        Some(crate::core::QueueConfig {
            max_depth: req.max_depth.unwrap_or(100_000),
            ack_deadline_secs: req.ack_deadline_secs.unwrap_or(30),
            default_max_retries: req.default_max_retries.unwrap_or(3),
            default_priority: req.default_priority.unwrap_or(5),
        })
    } else {
        None
    };

    queue_manager.create_queue(&queue_name, config).await?;

    Ok(Json(serde_json::json!({
        "success": true,
        "queue": queue_name
    })))
}

/// Publish message endpoint
pub async fn queue_publish(
    State(state): State<AppState>,
    Path(queue_name): Path<String>,
    Json(req): Json<PublishRequest>,
) -> Result<Json<PublishResponse>, SynapError> {
    debug!("REST PUBLISH to queue: {}", queue_name);

    let queue_manager = state
        .queue_manager
        .as_ref()
        .ok_or_else(|| SynapError::InvalidRequest("Queue system disabled".to_string()))?;

    let message_id = queue_manager
        .publish(&queue_name, req.payload, req.priority, req.max_retries)
        .await?;

    // Note: Queue persistence will be integrated in a future update
    // For now, queue operations are not persisted to WAL
    // TODO: Add persistence.log_queue_publish() when queue persistence is ready

    Ok(Json(PublishResponse { message_id }))
}

/// Consume message endpoint
pub async fn queue_consume(
    State(state): State<AppState>,
    Path((queue_name, consumer_id)): Path<(String, String)>,
) -> Result<Json<ConsumeResponse>, SynapError> {
    debug!("REST CONSUME from queue: {} by {}", queue_name, consumer_id);

    let queue_manager = state
        .queue_manager
        .as_ref()
        .ok_or_else(|| SynapError::InvalidRequest("Queue system disabled".to_string()))?;

    let message = queue_manager.consume(&queue_name, &consumer_id).await?;

    if let Some(msg) = message {
        Ok(Json(ConsumeResponse {
            message_id: Some(msg.id),
            payload: Some((*msg.payload).clone()), // Convert Arc<Vec<u8>> to Vec<u8>
            priority: Some(msg.priority),
            retry_count: Some(msg.retry_count),
            headers: Some(msg.headers),
        }))
    } else {
        Ok(Json(ConsumeResponse {
            message_id: None,
            payload: None,
            priority: None,
            retry_count: None,
            headers: None,
        }))
    }
}

/// ACK message endpoint
pub async fn queue_ack(
    State(state): State<AppState>,
    Path(queue_name): Path<String>,
    Json(req): Json<AckRequest>,
) -> Result<Json<serde_json::Value>, SynapError> {
    debug!(
        "REST ACK message: {} in queue: {}",
        req.message_id, queue_name
    );

    let queue_manager = state
        .queue_manager
        .as_ref()
        .ok_or_else(|| SynapError::InvalidRequest("Queue system disabled".to_string()))?;

    queue_manager.ack(&queue_name, &req.message_id).await?;

    Ok(Json(serde_json::json!({ "success": true })))
}

/// NACK message endpoint
pub async fn queue_nack(
    State(state): State<AppState>,
    Path(queue_name): Path<String>,
    Json(req): Json<NackRequest>,
) -> Result<Json<serde_json::Value>, SynapError> {
    debug!(
        "REST NACK message: {} in queue: {}",
        req.message_id, queue_name
    );

    let queue_manager = state
        .queue_manager
        .as_ref()
        .ok_or_else(|| SynapError::InvalidRequest("Queue system disabled".to_string()))?;

    queue_manager
        .nack(&queue_name, &req.message_id, req.requeue)
        .await?;

    Ok(Json(serde_json::json!({ "success": true })))
}

/// Queue stats endpoint
pub async fn queue_stats(
    State(state): State<AppState>,
    Path(queue_name): Path<String>,
) -> Result<Json<serde_json::Value>, SynapError> {
    debug!("REST QUEUE STATS: {}", queue_name);

    let queue_manager = state
        .queue_manager
        .as_ref()
        .ok_or_else(|| SynapError::InvalidRequest("Queue system disabled".to_string()))?;

    let stats = queue_manager.stats(&queue_name).await?;

    Ok(Json(serde_json::to_value(stats).map_err(|e| {
        SynapError::SerializationError(e.to_string())
    })?))
}

/// List queues endpoint
pub async fn queue_list(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, SynapError> {
    debug!("REST LIST QUEUES");

    let queue_manager = state
        .queue_manager
        .as_ref()
        .ok_or_else(|| SynapError::InvalidRequest("Queue system disabled".to_string()))?;

    let queues = queue_manager.list_queues().await?;

    Ok(Json(serde_json::json!({ "queues": queues })))
}

/// Purge queue endpoint
pub async fn queue_purge(
    State(state): State<AppState>,
    Path(queue_name): Path<String>,
) -> Result<Json<serde_json::Value>, SynapError> {
    debug!("REST PURGE QUEUE: {}", queue_name);

    let queue_manager = state
        .queue_manager
        .as_ref()
        .ok_or_else(|| SynapError::InvalidRequest("Queue system disabled".to_string()))?;

    let count = queue_manager.purge(&queue_name).await?;

    Ok(Json(serde_json::json!({
        "success": true,
        "purged": count
    })))
}

/// Delete queue endpoint
pub async fn queue_delete(
    State(state): State<AppState>,
    Path(queue_name): Path<String>,
) -> Result<Json<serde_json::Value>, SynapError> {
    debug!("REST DELETE QUEUE: {}", queue_name);

    let queue_manager = state
        .queue_manager
        .as_ref()
        .ok_or_else(|| SynapError::InvalidRequest("Queue system disabled".to_string()))?;

    let deleted = queue_manager.delete_queue(&queue_name).await?;

    Ok(Json(serde_json::json!({
        "success": true,
        "deleted": deleted
    })))
}

/// StreamableHTTP command handler
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
        "kv.set" => handle_kv_set_cmd(&state, &request).await,
        "kv.get" => handle_kv_get_cmd(state.kv_store.clone(), &request).await,
        "kv.del" => handle_kv_del_cmd(&state, &request).await,
        "kv.exists" => handle_kv_exists_cmd(state.kv_store.clone(), &request).await,
        "kv.incr" => handle_kv_incr_cmd(&state, &request).await,
        "kv.decr" => handle_kv_decr_cmd(&state, &request).await,
        "kv.mset" => handle_kv_mset_cmd(&state, &request).await,
        "kv.mget" => handle_kv_mget_cmd(state.kv_store.clone(), &request).await,
        "kv.mdel" => handle_kv_mdel_cmd(&state, &request).await,
        "kv.scan" => handle_kv_scan_cmd(state.kv_store.clone(), &request).await,
        "kv.keys" => handle_kv_keys_cmd(state.kv_store.clone(), &request).await,
        "kv.dbsize" => handle_kv_dbsize_cmd(state.kv_store.clone(), &request).await,
        "kv.flushdb" => handle_kv_flushdb_cmd(state.kv_store.clone(), &request).await,
        "kv.flushall" => handle_kv_flushall_cmd(state.kv_store.clone(), &request).await,
        "kv.expire" => handle_kv_expire_cmd(state.kv_store.clone(), &request).await,
        "kv.ttl" => handle_kv_ttl_cmd(state.kv_store.clone(), &request).await,
        "kv.persist" => handle_kv_persist_cmd(state.kv_store.clone(), &request).await,
        "kv.stats" => handle_kv_stats_cmd(state.kv_store.clone(), &request).await,
        // Hash commands
        "hash.set" => handle_hash_set_cmd(&state, &request).await,
        "hash.get" => handle_hash_get_cmd(&state, &request).await,
        "hash.getall" => handle_hash_getall_cmd(&state, &request).await,
        "hash.del" => handle_hash_del_cmd(&state, &request).await,
        "hash.exists" => handle_hash_exists_cmd(&state, &request).await,
        "hash.len" => handle_hash_len_cmd(&state, &request).await,
        "hash.keys" => handle_hash_keys_cmd(&state, &request).await,
        "hash.vals" => handle_hash_vals_cmd(&state, &request).await,
        "hash.mset" => handle_hash_mset_cmd(&state, &request).await,
        "hash.mget" => handle_hash_mget_cmd(&state, &request).await,
        "hash.incrby" => handle_hash_incrby_cmd(&state, &request).await,
        "hash.incrbyfloat" => handle_hash_incrbyfloat_cmd(&state, &request).await,
        "hash.setnx" => handle_hash_setnx_cmd(&state, &request).await,
        "hash.stats" => handle_hash_stats_cmd(&state, &request).await,
        // List commands
        "list.lpush" => handle_list_lpush_cmd(&state, &request).await,
        "list.lpushx" => handle_list_lpushx_cmd(&state, &request).await,
        "list.rpush" => handle_list_rpush_cmd(&state, &request).await,
        "list.rpushx" => handle_list_rpushx_cmd(&state, &request).await,
        "list.lpop" => handle_list_lpop_cmd(&state, &request).await,
        "list.rpop" => handle_list_rpop_cmd(&state, &request).await,
        "list.lrange" => handle_list_lrange_cmd(&state, &request).await,
        "list.llen" => handle_list_llen_cmd(&state, &request).await,
        "list.lindex" => handle_list_lindex_cmd(&state, &request).await,
        "list.lset" => handle_list_lset_cmd(&state, &request).await,
        "list.ltrim" => handle_list_ltrim_cmd(&state, &request).await,
        "list.lrem" => handle_list_lrem_cmd(&state, &request).await,
        "list.linsert" => handle_list_linsert_cmd(&state, &request).await,
        "list.lpos" => handle_list_lpos_cmd(&state, &request).await,
        "list.rpoplpush" => handle_list_rpoplpush_cmd(&state, &request).await,
        "list.stats" => handle_list_stats_cmd(&state, &request).await,
        "queue.create" => handle_queue_create_cmd(&state, &request).await,
        "queue.delete" => handle_queue_delete_cmd(&state, &request).await,
        "queue.publish" => handle_queue_publish_cmd(&state, &request).await,
        "queue.consume" => handle_queue_consume_cmd(&state, &request).await,
        "queue.ack" => handle_queue_ack_cmd(&state, &request).await,
        "queue.nack" => handle_queue_nack_cmd(&state, &request).await,
        "queue.list" => handle_queue_list_cmd(&state, &request).await,
        "queue.stats" => handle_queue_stats_cmd(&state, &request).await,
        "queue.purge" => handle_queue_purge_cmd(&state, &request).await,
        "pubsub.subscribe" => handle_pubsub_subscribe_cmd(&state, &request).await,
        "pubsub.publish" => handle_pubsub_publish_cmd(&state, &request).await,
        "pubsub.unsubscribe" => handle_pubsub_unsubscribe_cmd(&state, &request).await,
        "pubsub.stats" => handle_pubsub_stats_cmd(&state, &request).await,
        "pubsub.topics" => handle_pubsub_topics_cmd(&state, &request).await,
        "pubsub.info" => handle_pubsub_info_cmd(&state, &request).await,
        "stream.create" => handle_stream_create_cmd(&state, &request).await,
        "stream.publish" => handle_stream_publish_cmd(&state, &request).await,
        "stream.consume" => handle_stream_consume_cmd(&state, &request).await,
        "stream.stats" => handle_stream_stats_cmd(&state, &request).await,
        "stream.list" => handle_stream_list_cmd(&state, &request).await,
        "stream.delete" => handle_stream_delete_cmd(&state, &request).await,
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

async fn handle_kv_set_cmd(
    state: &AppState,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let key = request
        .payload
        .get("key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'key' field".to_string()))?;

    let value = request
        .payload
        .get("value")
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'value' field".to_string()))?;

    let ttl = request.payload.get("ttl").and_then(|v| v.as_u64());

    let value_bytes =
        serde_json::to_vec(value).map_err(|e| SynapError::SerializationError(e.to_string()))?;

    state.kv_store.set(key, value_bytes.clone(), ttl).await?;

    // Log to WAL
    if let Some(ref persistence) = state.persistence {
        let _ = persistence
            .log_kv_set(key.to_string(), value_bytes, ttl)
            .await;
    }

    Ok(serde_json::json!({ "success": true }))
}

async fn handle_kv_get_cmd(
    store: Arc<KVStore>,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let key = request
        .payload
        .get("key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'key' field".to_string()))?;

    let return_type = request
        .payload
        .get("type")
        .and_then(|v| v.as_str())
        .unwrap_or("string");

    let value_bytes = store.get(key).await?;

    if let Some(bytes) = value_bytes {
        match return_type {
            "bytes" => Ok(serde_json::json!(bytes)),
            _ => {
                // Default: return as string
                let value_str = String::from_utf8(bytes)
                    .unwrap_or_else(|e| format!("<binary data: {} bytes>", e.as_bytes().len()));

                Ok(serde_json::json!(value_str))
            }
        }
    } else {
        Ok(serde_json::json!(null))
    }
}

async fn handle_kv_del_cmd(
    state: &AppState,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let key = request
        .payload
        .get("key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'key' field".to_string()))?;

    let deleted = state.kv_store.delete(key).await?;

    // Log to WAL if deleted
    if deleted {
        if let Some(ref persistence) = state.persistence {
            let _ = persistence.log_kv_del(vec![key.to_string()]).await;
        }
    }

    Ok(serde_json::json!({ "deleted": deleted }))
}

async fn handle_kv_exists_cmd(
    store: Arc<KVStore>,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let key = request
        .payload
        .get("key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'key' field".to_string()))?;

    let exists = store.exists(key).await?;

    Ok(serde_json::json!({ "exists": exists }))
}

async fn handle_kv_incr_cmd(
    state: &AppState,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let key = request
        .payload
        .get("key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'key' field".to_string()))?;

    let amount = request
        .payload
        .get("amount")
        .and_then(|v| v.as_i64())
        .unwrap_or(1);

    let value = state.kv_store.incr(key, amount).await?;

    // Log final value to WAL (INCR is a SET operation)
    if let Some(ref persistence) = state.persistence {
        let _ = persistence
            .log_kv_set(key.to_string(), value.to_string().into_bytes(), None)
            .await;
    }

    Ok(serde_json::json!({ "value": value }))
}

async fn handle_kv_decr_cmd(
    state: &AppState,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let key = request
        .payload
        .get("key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'key' field".to_string()))?;

    let amount = request
        .payload
        .get("amount")
        .and_then(|v| v.as_i64())
        .unwrap_or(1);

    let value = state.kv_store.decr(key, amount).await?;

    // Log final value to WAL (DECR is a SET operation)
    if let Some(ref persistence) = state.persistence {
        let _ = persistence
            .log_kv_set(key.to_string(), value.to_string().into_bytes(), None)
            .await;
    }

    Ok(serde_json::json!({ "value": value }))
}

async fn handle_kv_mset_cmd(
    state: &AppState,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let pairs_val = request
        .payload
        .get("pairs")
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'pairs' field".to_string()))?;

    let pairs_arr = pairs_val
        .as_array()
        .ok_or_else(|| SynapError::InvalidRequest("'pairs' must be an array".to_string()))?;

    let mut pairs = Vec::new();
    for pair in pairs_arr {
        let key = pair
            .get("key")
            .and_then(|v| v.as_str())
            .ok_or_else(|| SynapError::InvalidRequest("Each pair must have 'key'".to_string()))?;

        let value = pair
            .get("value")
            .ok_or_else(|| SynapError::InvalidRequest("Each pair must have 'value'".to_string()))?;

        let value_bytes =
            serde_json::to_vec(value).map_err(|e| SynapError::SerializationError(e.to_string()))?;

        pairs.push((key.to_string(), value_bytes));
    }

    state.kv_store.mset(pairs.clone()).await?;

    // Log each SET to WAL
    if let Some(ref persistence) = state.persistence {
        for (key, value) in pairs {
            let _ = persistence.log_kv_set(key, value, None).await;
        }
    }

    Ok(serde_json::json!({ "success": true }))
}

async fn handle_kv_mget_cmd(
    store: Arc<KVStore>,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let keys_val = request
        .payload
        .get("keys")
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'keys' field".to_string()))?;

    let keys: Vec<String> = serde_json::from_value(keys_val.clone())
        .map_err(|e| SynapError::InvalidRequest(format!("Invalid keys array: {}", e)))?;

    let values = store.mget(&keys).await?;

    let results: Vec<Option<serde_json::Value>> = values
        .into_iter()
        .map(|opt_bytes| opt_bytes.and_then(|bytes| serde_json::from_slice(&bytes).ok()))
        .collect();

    Ok(serde_json::json!({ "values": results }))
}

async fn handle_kv_mdel_cmd(
    state: &AppState,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let keys_val = request
        .payload
        .get("keys")
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'keys' field".to_string()))?;

    let keys: Vec<String> = serde_json::from_value(keys_val.clone())
        .map_err(|e| SynapError::InvalidRequest(format!("Invalid keys array: {}", e)))?;

    let count = state.kv_store.mdel(&keys).await?;

    // Log deletions to WAL
    if count > 0 {
        if let Some(ref persistence) = state.persistence {
            let _ = persistence.log_kv_del(keys).await;
        }
    }

    Ok(serde_json::json!({ "deleted": count }))
}

async fn handle_kv_scan_cmd(
    store: Arc<KVStore>,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let prefix = request.payload.get("prefix").and_then(|v| v.as_str());
    let limit = request
        .payload
        .get("limit")
        .and_then(|v| v.as_u64())
        .unwrap_or(100) as usize;

    let keys = store.scan(prefix, limit).await?;

    Ok(serde_json::json!({
        "keys": keys,
        "count": keys.len()
    }))
}

async fn handle_kv_stats_cmd(
    store: Arc<KVStore>,
    _request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let stats = store.stats().await;

    Ok(serde_json::json!({
        "total_keys": stats.total_keys,
        "total_memory_bytes": stats.total_memory_bytes,
        "operations": {
            "gets": stats.gets,
            "sets": stats.sets,
            "dels": stats.dels,
            "hits": stats.hits,
            "misses": stats.misses,
        },
        "hit_rate": stats.hit_rate()
    }))
}

async fn handle_kv_keys_cmd(
    store: Arc<KVStore>,
    _request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let keys = store.keys().await?;
    Ok(serde_json::json!({ "keys": keys, "count": keys.len() }))
}

async fn handle_kv_dbsize_cmd(
    store: Arc<KVStore>,
    _request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let size = store.dbsize().await?;
    Ok(serde_json::json!({ "size": size }))
}

async fn handle_kv_flushdb_cmd(
    store: Arc<KVStore>,
    _request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let count = store.flushdb().await?;
    Ok(serde_json::json!({ "flushed": count }))
}

async fn handle_kv_flushall_cmd(
    store: Arc<KVStore>,
    _request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let count = store.flushall().await?;
    Ok(serde_json::json!({ "flushed": count }))
}

async fn handle_kv_expire_cmd(
    store: Arc<KVStore>,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let key = request
        .payload
        .get("key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'key' field".to_string()))?;

    let ttl = request
        .payload
        .get("ttl")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'ttl' field".to_string()))?;

    let result = store.expire(key, ttl).await?;
    Ok(serde_json::json!({ "result": result }))
}

async fn handle_kv_ttl_cmd(
    store: Arc<KVStore>,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let key = request
        .payload
        .get("key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'key' field".to_string()))?;

    let ttl = store.ttl(key).await?;
    Ok(serde_json::json!({ "ttl": ttl }))
}

async fn handle_kv_persist_cmd(
    store: Arc<KVStore>,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let key = request
        .payload
        .get("key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'key' field".to_string()))?;

    let result = store.persist(key).await?;
    Ok(serde_json::json!({ "result": result }))
}

// ==================== Hash Command Handlers ====================

async fn handle_hash_set_cmd(
    state: &AppState,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let key = request
        .payload
        .get("key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'key' field".to_string()))?;

    let field = request
        .payload
        .get("field")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'field' field".to_string()))?;

    let value = request
        .payload
        .get("value")
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'value' field".to_string()))?;

    let value_bytes =
        serde_json::to_vec(value).map_err(|e| SynapError::SerializationError(e.to_string()))?;

    let created = state.hash_store.hset(key, field, value_bytes)?;

    Ok(serde_json::json!({ "created": created, "key": key, "field": field }))
}

async fn handle_hash_get_cmd(
    state: &AppState,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let key = request
        .payload
        .get("key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'key' field".to_string()))?;

    let field = request
        .payload
        .get("field")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'field' field".to_string()))?;

    match state.hash_store.hget(key, field)? {
        Some(value_bytes) => {
            let value: serde_json::Value =
                serde_json::from_slice(&value_bytes).unwrap_or_else(|_| {
                    serde_json::Value::String(String::from_utf8_lossy(&value_bytes).to_string())
                });
            Ok(serde_json::json!({ "found": true, "value": value }))
        }
        None => Ok(serde_json::json!({ "found": false })),
    }
}

async fn handle_hash_getall_cmd(
    state: &AppState,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let key = request
        .payload
        .get("key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'key' field".to_string()))?;

    let all = state.hash_store.hgetall(key)?;

    let result: HashMap<String, serde_json::Value> = all
        .into_iter()
        .map(|(k, v)| {
            let json_value: serde_json::Value = serde_json::from_slice(&v).unwrap_or_else(|_| {
                serde_json::Value::String(String::from_utf8_lossy(&v).to_string())
            });
            (k, json_value)
        })
        .collect();

    Ok(serde_json::json!({ "fields": result, "count": result.len() }))
}

async fn handle_hash_del_cmd(
    state: &AppState,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let key = request
        .payload
        .get("key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'key' field".to_string()))?;

    let fields: Vec<String> = request
        .payload
        .get("fields")
        .and_then(|v| v.as_array())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'fields' array".to_string()))?
        .iter()
        .filter_map(|v| v.as_str().map(|s| s.to_string()))
        .collect();

    let deleted = state.hash_store.hdel(key, &fields)?;

    Ok(serde_json::json!({ "deleted": deleted }))
}

async fn handle_hash_exists_cmd(
    state: &AppState,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let key = request
        .payload
        .get("key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'key' field".to_string()))?;

    let field = request
        .payload
        .get("field")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'field' field".to_string()))?;

    let exists = state.hash_store.hexists(key, field)?;

    Ok(serde_json::json!({ "exists": exists }))
}

async fn handle_hash_len_cmd(
    state: &AppState,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let key = request
        .payload
        .get("key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'key' field".to_string()))?;

    let len = state.hash_store.hlen(key)?;

    Ok(serde_json::json!({ "length": len }))
}

async fn handle_hash_keys_cmd(
    state: &AppState,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let key = request
        .payload
        .get("key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'key' field".to_string()))?;

    let keys = state.hash_store.hkeys(key)?;

    Ok(serde_json::json!({ "keys": keys, "count": keys.len() }))
}

async fn handle_hash_vals_cmd(
    state: &AppState,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let key = request
        .payload
        .get("key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'key' field".to_string()))?;

    let values = state.hash_store.hvals(key)?;

    let result: Vec<serde_json::Value> = values
        .into_iter()
        .map(|v| {
            serde_json::from_slice(&v).unwrap_or_else(|_| {
                serde_json::Value::String(String::from_utf8_lossy(&v).to_string())
            })
        })
        .collect();

    Ok(serde_json::json!({ "values": result, "count": result.len() }))
}

async fn handle_hash_mset_cmd(
    state: &AppState,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let key = request
        .payload
        .get("key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'key' field".to_string()))?;

    let fields_obj = request
        .payload
        .get("fields")
        .and_then(|v| v.as_object())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'fields' object".to_string()))?;

    let fields: HashMap<String, Vec<u8>> = fields_obj
        .iter()
        .map(|(k, v)| {
            let bytes = serde_json::to_vec(v).map_err(|e| {
                SynapError::SerializationError(format!("Failed to serialize field {}: {}", k, e))
            })?;
            Ok((k.clone(), bytes))
        })
        .collect::<Result<HashMap<_, _>, SynapError>>()?;

    state.hash_store.hmset(key, fields)?;

    Ok(serde_json::json!({ "success": true }))
}

async fn handle_hash_mget_cmd(
    state: &AppState,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let key = request
        .payload
        .get("key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'key' field".to_string()))?;

    let fields: Vec<String> = request
        .payload
        .get("fields")
        .and_then(|v| v.as_array())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'fields' array".to_string()))?
        .iter()
        .filter_map(|v| v.as_str().map(|s| s.to_string()))
        .collect();

    let values = state.hash_store.hmget(key, &fields)?;

    let result: Vec<Option<serde_json::Value>> = values
        .into_iter()
        .map(|opt_v| {
            opt_v.map(|v| {
                serde_json::from_slice(&v).unwrap_or_else(|_| {
                    serde_json::Value::String(String::from_utf8_lossy(&v).to_string())
                })
            })
        })
        .collect();

    Ok(serde_json::json!({ "values": result }))
}

async fn handle_hash_incrby_cmd(
    state: &AppState,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let key = request
        .payload
        .get("key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'key' field".to_string()))?;

    let field = request
        .payload
        .get("field")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'field' field".to_string()))?;

    let increment = request
        .payload
        .get("increment")
        .and_then(|v| v.as_i64())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'increment' field".to_string()))?;

    let new_value = state.hash_store.hincrby(key, field, increment)?;

    Ok(serde_json::json!({ "value": new_value }))
}

async fn handle_hash_incrbyfloat_cmd(
    state: &AppState,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let key = request
        .payload
        .get("key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'key' field".to_string()))?;

    let field = request
        .payload
        .get("field")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'field' field".to_string()))?;

    let increment = request
        .payload
        .get("increment")
        .and_then(|v| v.as_f64())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'increment' field".to_string()))?;

    let new_value = state.hash_store.hincrbyfloat(key, field, increment)?;

    Ok(serde_json::json!({ "value": new_value }))
}

async fn handle_hash_setnx_cmd(
    state: &AppState,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let key = request
        .payload
        .get("key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'key' field".to_string()))?;

    let field = request
        .payload
        .get("field")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'field' field".to_string()))?;

    let value = request
        .payload
        .get("value")
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'value' field".to_string()))?;

    let value_bytes =
        serde_json::to_vec(value).map_err(|e| SynapError::SerializationError(e.to_string()))?;

    let created = state.hash_store.hsetnx(key, field, value_bytes)?;

    Ok(serde_json::json!({ "created": created }))
}

async fn handle_hash_stats_cmd(
    state: &AppState,
    _request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let stats = state.hash_store.stats();

    Ok(serde_json::json!({
        "total_hashes": stats.total_hashes,
        "total_fields": stats.total_fields,
        "operations": {
            "hset_count": stats.hset_count,
            "hget_count": stats.hget_count,
            "hdel_count": stats.hdel_count,
        }
    }))
}

// ==================== List Command Handlers ====================

async fn handle_list_lpush_cmd(
    state: &AppState,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let key = request
        .payload
        .get("key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'key' field".to_string()))?;

    let values: Vec<Vec<u8>> = request
        .payload
        .get("values")
        .and_then(|v| v.as_array())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'values' array".to_string()))?
        .iter()
        .map(|v| serde_json::to_vec(v).map_err(|e| SynapError::SerializationError(e.to_string())))
        .collect::<Result<Vec<_>, _>>()?;

    let length = state.list_store.lpush(key, values, false)?;

    Ok(serde_json::json!({ "length": length, "key": key }))
}

async fn handle_list_lpushx_cmd(
    state: &AppState,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let key = request
        .payload
        .get("key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'key' field".to_string()))?;

    let values: Vec<Vec<u8>> = request
        .payload
        .get("values")
        .and_then(|v| v.as_array())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'values' array".to_string()))?
        .iter()
        .map(|v| serde_json::to_vec(v).map_err(|e| SynapError::SerializationError(e.to_string())))
        .collect::<Result<Vec<_>, _>>()?;

    let length = state.list_store.lpush(key, values, true)?;

    Ok(serde_json::json!({ "length": length, "key": key }))
}

async fn handle_list_rpush_cmd(
    state: &AppState,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let key = request
        .payload
        .get("key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'key' field".to_string()))?;

    let values: Vec<Vec<u8>> = request
        .payload
        .get("values")
        .and_then(|v| v.as_array())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'values' array".to_string()))?
        .iter()
        .map(|v| serde_json::to_vec(v).map_err(|e| SynapError::SerializationError(e.to_string())))
        .collect::<Result<Vec<_>, _>>()?;

    let length = state.list_store.rpush(key, values, false)?;

    Ok(serde_json::json!({ "length": length, "key": key }))
}

async fn handle_list_rpushx_cmd(
    state: &AppState,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let key = request
        .payload
        .get("key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'key' field".to_string()))?;

    let values: Vec<Vec<u8>> = request
        .payload
        .get("values")
        .and_then(|v| v.as_array())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'values' array".to_string()))?
        .iter()
        .map(|v| serde_json::to_vec(v).map_err(|e| SynapError::SerializationError(e.to_string())))
        .collect::<Result<Vec<_>, _>>()?;

    let length = state.list_store.rpush(key, values, true)?;

    Ok(serde_json::json!({ "length": length, "key": key }))
}

async fn handle_list_lpop_cmd(
    state: &AppState,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let key = request
        .payload
        .get("key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'key' field".to_string()))?;

    let count = request
        .payload
        .get("count")
        .and_then(|v| v.as_u64())
        .map(|c| c as usize);

    let values = state.list_store.lpop(key, count)?;

    let json_values: Vec<serde_json::Value> = values
        .into_iter()
        .map(|v| {
            serde_json::from_slice(&v).unwrap_or_else(|_| {
                serde_json::Value::String(String::from_utf8_lossy(&v).to_string())
            })
        })
        .collect();

    Ok(serde_json::json!({ "values": json_values, "key": key }))
}

async fn handle_list_rpop_cmd(
    state: &AppState,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let key = request
        .payload
        .get("key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'key' field".to_string()))?;

    let count = request
        .payload
        .get("count")
        .and_then(|v| v.as_u64())
        .map(|c| c as usize);

    let values = state.list_store.rpop(key, count)?;

    let json_values: Vec<serde_json::Value> = values
        .into_iter()
        .map(|v| {
            serde_json::from_slice(&v).unwrap_or_else(|_| {
                serde_json::Value::String(String::from_utf8_lossy(&v).to_string())
            })
        })
        .collect();

    Ok(serde_json::json!({ "values": json_values, "key": key }))
}

async fn handle_list_lrange_cmd(
    state: &AppState,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let key = request
        .payload
        .get("key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'key' field".to_string()))?;

    let start = request
        .payload
        .get("start")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);

    let stop = request
        .payload
        .get("stop")
        .and_then(|v| v.as_i64())
        .unwrap_or(-1);

    let values = state.list_store.lrange(key, start, stop)?;

    let json_values: Vec<serde_json::Value> = values
        .into_iter()
        .map(|v| {
            serde_json::from_slice(&v).unwrap_or_else(|_| {
                serde_json::Value::String(String::from_utf8_lossy(&v).to_string())
            })
        })
        .collect();

    Ok(serde_json::json!({ "values": json_values, "key": key }))
}

async fn handle_list_llen_cmd(
    state: &AppState,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let key = request
        .payload
        .get("key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'key' field".to_string()))?;

    let length = state.list_store.llen(key)?;

    Ok(serde_json::json!({ "length": length, "key": key }))
}

async fn handle_list_lindex_cmd(
    state: &AppState,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let key = request
        .payload
        .get("key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'key' field".to_string()))?;

    let index = request
        .payload
        .get("index")
        .and_then(|v| v.as_i64())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'index' field".to_string()))?;

    let value = state.list_store.lindex(key, index)?;

    let json_value: serde_json::Value = serde_json::from_slice(&value)
        .unwrap_or_else(|_| serde_json::Value::String(String::from_utf8_lossy(&value).to_string()));

    Ok(serde_json::json!({ "value": json_value, "key": key, "index": index }))
}

async fn handle_list_lset_cmd(
    state: &AppState,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let key = request
        .payload
        .get("key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'key' field".to_string()))?;

    let index = request
        .payload
        .get("index")
        .and_then(|v| v.as_i64())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'index' field".to_string()))?;

    let value = request
        .payload
        .get("value")
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'value' field".to_string()))?;

    let value_bytes =
        serde_json::to_vec(value).map_err(|e| SynapError::SerializationError(e.to_string()))?;

    state.list_store.lset(key, index, value_bytes)?;

    Ok(serde_json::json!({ "success": true, "key": key, "index": index }))
}

async fn handle_list_ltrim_cmd(
    state: &AppState,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let key = request
        .payload
        .get("key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'key' field".to_string()))?;

    let start = request
        .payload
        .get("start")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);

    let stop = request
        .payload
        .get("stop")
        .and_then(|v| v.as_i64())
        .unwrap_or(-1);

    state.list_store.ltrim(key, start, stop)?;

    Ok(serde_json::json!({ "success": true, "key": key }))
}

async fn handle_list_lrem_cmd(
    state: &AppState,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let key = request
        .payload
        .get("key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'key' field".to_string()))?;

    let count = request
        .payload
        .get("count")
        .and_then(|v| v.as_i64())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'count' field".to_string()))?;

    let value = request
        .payload
        .get("value")
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'value' field".to_string()))?;

    let value_bytes =
        serde_json::to_vec(value).map_err(|e| SynapError::SerializationError(e.to_string()))?;

    let removed = state.list_store.lrem(key, count, value_bytes)?;

    Ok(serde_json::json!({ "removed": removed, "key": key }))
}

async fn handle_list_linsert_cmd(
    state: &AppState,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let key = request
        .payload
        .get("key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'key' field".to_string()))?;

    let before = request
        .payload
        .get("before")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    let pivot = request
        .payload
        .get("pivot")
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'pivot' field".to_string()))?;

    let value = request
        .payload
        .get("value")
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'value' field".to_string()))?;

    let pivot_bytes =
        serde_json::to_vec(pivot).map_err(|e| SynapError::SerializationError(e.to_string()))?;

    let value_bytes =
        serde_json::to_vec(value).map_err(|e| SynapError::SerializationError(e.to_string()))?;

    let length = state
        .list_store
        .linsert(key, before, pivot_bytes, value_bytes)?;

    Ok(serde_json::json!({ "length": length, "key": key }))
}

async fn handle_list_lpos_cmd(
    state: &AppState,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let key = request
        .payload
        .get("key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'key' field".to_string()))?;

    let value = request
        .payload
        .get("value")
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'value' field".to_string()))?;

    let value_bytes =
        serde_json::to_vec(value).map_err(|e| SynapError::SerializationError(e.to_string()))?;

    let position = state.list_store.lpos(key, value_bytes)?;

    Ok(serde_json::json!({ "position": position, "key": key }))
}

async fn handle_list_rpoplpush_cmd(
    state: &AppState,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let source = request
        .payload
        .get("source")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'source' field".to_string()))?;

    let destination = request
        .payload
        .get("destination")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'destination' field".to_string()))?;

    let value = state.list_store.rpoplpush(source, destination)?;

    let json_value: serde_json::Value = serde_json::from_slice(&value)
        .unwrap_or_else(|_| serde_json::Value::String(String::from_utf8_lossy(&value).to_string()));

    Ok(serde_json::json!({ "value": json_value, "source": source, "destination": destination }))
}

async fn handle_list_stats_cmd(
    state: &AppState,
    _request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let stats = state.list_store.stats();

    Ok(serde_json::json!({
        "total_lists": stats.total_lists,
        "total_elements": stats.total_elements,
        "operations": {
            "lpush_count": stats.lpush_count,
            "rpush_count": stats.rpush_count,
            "lpop_count": stats.lpop_count,
            "rpop_count": stats.rpop_count,
            "lrange_count": stats.lrange_count,
            "llen_count": stats.llen_count,
            "lindex_count": stats.lindex_count,
            "lset_count": stats.lset_count,
            "ltrim_count": stats.ltrim_count,
            "lrem_count": stats.lrem_count,
            "linsert_count": stats.linsert_count,
            "rpoplpush_count": stats.rpoplpush_count,
            "blpop_count": stats.blpop_count,
            "brpop_count": stats.brpop_count,
        }
    }))
}

// ==================== Queue Command Handlers ====================

async fn handle_queue_create_cmd(
    state: &AppState,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let queue_manager = state
        .queue_manager
        .as_ref()
        .ok_or_else(|| SynapError::InvalidRequest("Queue system disabled".to_string()))?;

    let name = request
        .payload
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'name' field".to_string()))?;

    let config = request.payload.get("config").and_then(|v| {
        let max_depth = v
            .get("max_depth")
            .and_then(|d| d.as_u64())
            .map(|d| d as usize);
        let ack_deadline_secs = v.get("ack_deadline_secs").and_then(|d| d.as_u64());
        let default_max_retries = v
            .get("default_max_retries")
            .and_then(|d| d.as_u64())
            .map(|d| d as u32);
        let default_priority = v
            .get("default_priority")
            .and_then(|d| d.as_u64())
            .map(|d| d as u8);

        if max_depth.is_some()
            || ack_deadline_secs.is_some()
            || default_max_retries.is_some()
            || default_priority.is_some()
        {
            Some(crate::core::QueueConfig {
                max_depth: max_depth.unwrap_or(100_000),
                ack_deadline_secs: ack_deadline_secs.unwrap_or(30),
                default_max_retries: default_max_retries.unwrap_or(3),
                default_priority: default_priority.unwrap_or(5),
            })
        } else {
            None
        }
    });

    queue_manager.create_queue(name, config).await?;
    Ok(serde_json::json!({ "success": true }))
}

async fn handle_queue_delete_cmd(
    state: &AppState,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let queue_manager = state
        .queue_manager
        .as_ref()
        .ok_or_else(|| SynapError::InvalidRequest("Queue system disabled".to_string()))?;

    let name = request
        .payload
        .get("queue")
        .or_else(|| request.payload.get("name"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'queue' or 'name' field".to_string()))?;

    let deleted = queue_manager.delete_queue(name).await?;
    Ok(serde_json::json!({ "deleted": deleted }))
}

async fn handle_queue_publish_cmd(
    state: &AppState,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let queue_manager = state
        .queue_manager
        .as_ref()
        .ok_or_else(|| SynapError::InvalidRequest("Queue system disabled".to_string()))?;

    let queue = request
        .payload
        .get("queue")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'queue' field".to_string()))?;

    let payload_arr = request
        .payload
        .get("payload")
        .and_then(|v| v.as_array())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'payload' field".to_string()))?;

    let payload_bytes: Vec<u8> = payload_arr
        .iter()
        .filter_map(|v| v.as_u64().map(|n| n as u8))
        .collect();

    let priority = request
        .payload
        .get("priority")
        .and_then(|v| v.as_u64())
        .map(|p| p as u8);
    let max_retries = request
        .payload
        .get("max_retries")
        .and_then(|v| v.as_u64())
        .map(|r| r as u32);
    // Note: headers are ignored for now - not supported by the queue manager
    // let headers = request.payload.get("headers")...

    let message_id = queue_manager
        .publish(queue, payload_bytes, priority, max_retries)
        .await?;
    Ok(serde_json::json!({ "message_id": message_id }))
}

async fn handle_queue_consume_cmd(
    state: &AppState,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let queue_manager = state
        .queue_manager
        .as_ref()
        .ok_or_else(|| SynapError::InvalidRequest("Queue system disabled".to_string()))?;

    let queue = request
        .payload
        .get("queue")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'queue' field".to_string()))?;

    let consumer_id = request
        .payload
        .get("consumer_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'consumer_id' field".to_string()))?;

    let message = queue_manager.consume(queue, consumer_id).await?;

    if let Some(msg) = message {
        Ok(serde_json::json!({
            "message": {
                "id": msg.id,
                "payload": (*msg.payload).clone(), // Convert Arc<Vec<u8>> to Vec<u8>
                "priority": msg.priority,
                "retry_count": msg.retry_count,
                "headers": msg.headers,
            }
        }))
    } else {
        Ok(serde_json::json!({ "message": null }))
    }
}

async fn handle_queue_ack_cmd(
    state: &AppState,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let queue_manager = state
        .queue_manager
        .as_ref()
        .ok_or_else(|| SynapError::InvalidRequest("Queue system disabled".to_string()))?;

    let queue = request
        .payload
        .get("queue")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'queue' field".to_string()))?;

    let message_id = request
        .payload
        .get("message_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'message_id' field".to_string()))?;

    queue_manager.ack(queue, message_id).await?;
    Ok(serde_json::json!({ "success": true }))
}

async fn handle_queue_nack_cmd(
    state: &AppState,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let queue_manager = state
        .queue_manager
        .as_ref()
        .ok_or_else(|| SynapError::InvalidRequest("Queue system disabled".to_string()))?;

    let queue = request
        .payload
        .get("queue")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'queue' field".to_string()))?;

    let message_id = request
        .payload
        .get("message_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'message_id' field".to_string()))?;

    let requeue = request
        .payload
        .get("requeue")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    queue_manager.nack(queue, message_id, requeue).await?;
    Ok(serde_json::json!({ "success": true }))
}

async fn handle_queue_list_cmd(
    state: &AppState,
    _request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let queue_manager = state
        .queue_manager
        .as_ref()
        .ok_or_else(|| SynapError::InvalidRequest("Queue system disabled".to_string()))?;

    let queues = queue_manager.list_queues().await?;
    Ok(serde_json::json!({ "queues": queues }))
}

async fn handle_queue_stats_cmd(
    state: &AppState,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let queue_manager = state
        .queue_manager
        .as_ref()
        .ok_or_else(|| SynapError::InvalidRequest("Queue system disabled".to_string()))?;

    let queue = request
        .payload
        .get("queue")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'queue' field".to_string()))?;

    let stats = queue_manager.stats(queue).await?;
    serde_json::to_value(stats).map_err(|e| SynapError::SerializationError(e.to_string()))
}

async fn handle_queue_purge_cmd(
    state: &AppState,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let queue_manager = state
        .queue_manager
        .as_ref()
        .ok_or_else(|| SynapError::InvalidRequest("Queue system disabled".to_string()))?;

    let queue = request
        .payload
        .get("queue")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'queue' field".to_string()))?;

    let count = queue_manager.purge(queue).await?;
    Ok(serde_json::json!({ "purged": count }))
}

// ============================================================================
// Pub/Sub WebSocket Handler
// ============================================================================

// ============================================================================
// KV Store WebSocket Handler
// ============================================================================

/// WebSocket handler for KV WATCH (real-time key change notifications)
/// GET /kv/ws?keys=key1,key2,prefix:*
pub async fn kv_websocket(
    State(_state): State<AppState>,
    _ws: WebSocketUpgrade,
    axum::extract::Query(params): axum::extract::Query<HashMap<String, String>>,
) -> AxumResponse {
    // Parse keys from query params
    let keys_str = params.get("keys").cloned().unwrap_or_default();
    let keys: Vec<String> = keys_str
        .split(',')
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect();

    if keys.is_empty() {
        return (
            axum::http::StatusCode::BAD_REQUEST,
            "At least one key required in query param: ?keys=key1,key2",
        )
            .into_response();
    }

    info!("KV WebSocket WATCH connection for keys: {:?}", keys);

    // Note: Full implementation would require KVStore to support change notifications
    // For now, return not implemented
    (
        axum::http::StatusCode::NOT_IMPLEMENTED,
        "KV WebSocket WATCH not yet implemented - use polling for now",
    )
        .into_response()
}

// ============================================================================
// Queue WebSocket Handler
// ============================================================================

/// WebSocket handler for Queue continuous consume (real-time message delivery)
/// GET /queue/:name/ws/:consumer_id
pub async fn queue_websocket(
    State(state): State<AppState>,
    Path((queue_name, consumer_id)): Path<(String, String)>,
    ws: WebSocketUpgrade,
) -> AxumResponse {
    let queue_manager = match state.queue_manager.as_ref() {
        Some(qm) => qm.clone(),
        None => {
            return (
                axum::http::StatusCode::SERVICE_UNAVAILABLE,
                "Queue system disabled",
            )
                .into_response();
        }
    };

    info!(
        "Queue WebSocket connection: queue={}, consumer={}",
        queue_name, consumer_id
    );

    ws.on_upgrade(move |socket| handle_queue_socket(socket, queue_manager, queue_name, consumer_id))
}

/// Handle Queue WebSocket connection
async fn handle_queue_socket(
    socket: WebSocket,
    queue_manager: Arc<crate::core::QueueManager>,
    queue_name: String,
    consumer_id: String,
) {
    let (mut ws_sender, mut ws_receiver) = socket.split();

    // Send welcome message
    let welcome = json!({
        "type": "connected",
        "queue": queue_name,
        "consumer_id": consumer_id
    });

    if ws_sender
        .send(axum::extract::ws::Message::Text(welcome.to_string().into()))
        .await
        .is_err()
    {
        warn!("Failed to send welcome to consumer: {}", consumer_id);
        return;
    }

    loop {
        tokio::select! {
            // Try to consume a message (non-blocking with timeout)
            _ = tokio::time::sleep(tokio::time::Duration::from_millis(100)) => {
                match queue_manager.consume(&queue_name, &consumer_id).await {
                    Ok(Some(msg)) => {
                        let msg_json = json!({
                            "type": "message",
                            "message_id": msg.id,
                            "payload": (*msg.payload).clone(),  // Clone Vec<u8> from Arc
                            "priority": msg.priority,
                            "retry_count": msg.retry_count,
                            "created_at": msg.created_at,
                            "headers": msg.headers
                        });

                        if ws_sender.send(axum::extract::ws::Message::Text(msg_json.to_string().into())).await.is_err() {
                            warn!("Failed to send message to consumer: {}", consumer_id);
                            break;
                        }
                    }
                    Ok(None) => {
                        // No messages available, continue waiting
                    }
                    Err(e) => {
                         error!("Queue consume error: {}", e);
                        let _ = ws_sender.send(axum::extract::ws::Message::Text(
                            json!({"type": "error", "error": e.to_string()}).to_string().into()
                        )).await;
                        break;
                    }
                }
            }

            // Handle incoming WebSocket messages (ACK/NACK commands)
            Some(msg) = ws_receiver.next() => {
                match msg {
                    Ok(axum::extract::ws::Message::Text(text)) => {
                        if let Ok(cmd) = serde_json::from_str::<serde_json::Value>(&text) {
                            match cmd["command"].as_str() {
                                Some("ack") => {
                                    if let Some(msg_id) = cmd["message_id"].as_str() {
                                        if let Err(e) = queue_manager.ack(&queue_name, msg_id).await {
                                            error!("ACK error: {}", e);
                                        }
                                    }
                                }
                                Some("nack") => {
                                    if let Some(msg_id) = cmd["message_id"].as_str() {
                                        let requeue = cmd["requeue"].as_bool().unwrap_or(true);
                                        if let Err(e) = queue_manager.nack(&queue_name, msg_id, requeue).await {
                                            error!("NACK error: {}", e);
                                        }
                                    }
                                }
                                _ => {
                                    warn!("Unknown command: {:?}", cmd["command"]);
                                }
                            }
                        }
                    }
                    Ok(axum::extract::ws::Message::Close(_)) => {
                        info!("Queue consumer {} closed connection", consumer_id);
                        break;
                    }
                    Ok(axum::extract::ws::Message::Ping(data)) => {
                        if ws_sender.send(axum::extract::ws::Message::Pong(data)).await.is_err() {
                            break;
                        }
                    }
                    Ok(_) => {}
                    Err(e) => {
                        warn!("WebSocket error for consumer {}: {}", consumer_id, e);
                        break;
                    }
                }
            }

            else => {
                break;
            }
        }
    }

    info!("Queue consumer {} disconnected", consumer_id);
}

// ============================================================================
// Event Streams WebSocket Handler
// ============================================================================

/// WebSocket handler for Event Streams (real-time event push)
/// GET /stream/:room/ws/:subscriber_id?from_offset=0
pub async fn stream_websocket(
    State(state): State<AppState>,
    Path((room_name, subscriber_id)): Path<(String, String)>,
    axum::extract::Query(params): axum::extract::Query<HashMap<String, String>>,
    ws: WebSocketUpgrade,
) -> AxumResponse {
    let stream_manager = match state.stream_manager.as_ref() {
        Some(sm) => sm.clone(),
        None => {
            return (
                axum::http::StatusCode::SERVICE_UNAVAILABLE,
                "Stream system disabled",
            )
                .into_response();
        }
    };

    let from_offset = params
        .get("from_offset")
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0);

    info!(
        "Stream WebSocket connection: room={}, subscriber={}, from_offset={}",
        room_name, subscriber_id, from_offset
    );

    ws.on_upgrade(move |socket| {
        handle_stream_socket(
            socket,
            stream_manager,
            room_name,
            subscriber_id,
            from_offset,
        )
    })
}

/// Handle Event Stream WebSocket connection
async fn handle_stream_socket(
    socket: WebSocket,
    stream_manager: Arc<crate::core::StreamManager>,
    room_name: String,
    subscriber_id: String,
    mut current_offset: u64,
) {
    let (mut ws_sender, mut ws_receiver) = socket.split();

    // Send welcome message
    let welcome = json!({
        "type": "connected",
        "room": room_name,
        "subscriber_id": subscriber_id,
        "from_offset": current_offset
    });

    if ws_sender
        .send(axum::extract::ws::Message::Text(welcome.to_string().into()))
        .await
        .is_err()
    {
        warn!(
            "Failed to send welcome to stream subscriber: {}",
            subscriber_id
        );
        return;
    }

    loop {
        tokio::select! {
            // Poll for new events (100ms interval)
            _ = tokio::time::sleep(tokio::time::Duration::from_millis(100)) => {
                match stream_manager.consume(&room_name, &subscriber_id, current_offset, 100).await {
                    Ok(events) => {
                        if !events.is_empty() {
                            for event in &events {
                                // Deserialize data from bytes to JSON
                                let data_json: serde_json::Value = serde_json::from_slice(&event.data)
                                    .unwrap_or(serde_json::Value::Null);

                                let event_json = json!({
                                    "type": "event",
                                    "offset": event.offset,
                                    "event": event.event,
                                    "data": data_json,
                                    "timestamp": event.timestamp
                                });

                                if ws_sender.send(axum::extract::ws::Message::Text(event_json.to_string().into())).await.is_err() {
                                    warn!("Failed to send event to subscriber: {}", subscriber_id);
                                    return;
                                }
                            }

                            // Update offset to next expected event
                            current_offset = events.last().unwrap().offset + 1;
                        }
                    }
                    Err(e) => {
                        error!("Stream consume error: {}", e);
                        let _ = ws_sender.send(axum::extract::ws::Message::Text(
                            json!({"type": "error", "error": e}).to_string().into()
                        )).await;
                        break;
                    }
                }
            }

            // Handle incoming WebSocket messages (control messages)
            Some(msg) = ws_receiver.next() => {
                match msg {
                    Ok(axum::extract::ws::Message::Close(_)) => {
                        info!("Stream subscriber {} closed connection", subscriber_id);
                        break;
                    }
                    Ok(axum::extract::ws::Message::Ping(data)) => {
                        if ws_sender.send(axum::extract::ws::Message::Pong(data)).await.is_err() {
                            break;
                        }
                    }
                    Ok(_) => {}
                    Err(e) => {
                        warn!("WebSocket error for stream subscriber {}: {}", subscriber_id, e);
                        break;
                    }
                }
            }

            else => {
                break;
            }
        }
    }

    info!(
        "Stream subscriber {} disconnected from room {}",
        subscriber_id, room_name
    );
}

// ============================================================================
// Pub/Sub WebSocket Handler
// ============================================================================

/// WebSocket handler for Pub/Sub subscriptions
/// GET /pubsub/ws?topics=topic1,topic2,*.wildcard
pub async fn pubsub_websocket(
    State(state): State<AppState>,
    ws: WebSocketUpgrade,
    axum::extract::Query(params): axum::extract::Query<HashMap<String, String>>,
) -> AxumResponse {
    let pubsub_router = match state.pubsub_router.as_ref() {
        Some(router) => router.clone(),
        None => {
            return (
                axum::http::StatusCode::SERVICE_UNAVAILABLE,
                "Pub/Sub system disabled",
            )
                .into_response();
        }
    };

    // Parse topics from query params
    let topics_str = params.get("topics").cloned().unwrap_or_default();
    let topics: Vec<String> = topics_str
        .split(',')
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect();

    if topics.is_empty() {
        return (
            axum::http::StatusCode::BAD_REQUEST,
            "At least one topic required in query param: ?topics=topic1,topic2",
        )
            .into_response();
    }

    info!("WebSocket connection requested for topics: {:?}", topics);

    ws.on_upgrade(move |socket| handle_pubsub_socket(socket, pubsub_router, topics))
}

/// Handle individual WebSocket connection for Pub/Sub
async fn handle_pubsub_socket(
    socket: WebSocket,
    pubsub_router: Arc<crate::core::PubSubRouter>,
    topics: Vec<String>,
) {
    let (mut ws_sender, mut ws_receiver) = socket.split();

    // Subscribe to topics
    let subscribe_result = match pubsub_router.subscribe(topics.clone()) {
        Ok(result) => result,
        Err(e) => {
            error!("Failed to subscribe: {}", e);
            let _ = ws_sender
                .send(axum::extract::ws::Message::Text(
                    json!({
                        "error": e.to_string()
                    })
                    .to_string()
                    .into(),
                ))
                .await;
            return;
        }
    };

    let subscriber_id = subscribe_result.subscriber_id.clone();
    info!(
        "Subscriber {} connected to topics: {:?}",
        subscriber_id, topics
    );

    // Create channel for receiving messages
    let (tx, mut rx) = mpsc::unbounded_channel::<Message>();

    // Register connection
    pubsub_router.register_connection(subscriber_id.clone(), tx);

    // Send welcome message in the loop (first iteration will handle it)
    let welcome_msg = json!({
        "type": "connected",
        "subscriber_id": subscriber_id,
        "topics": topics,
        "subscription_count": subscribe_result.subscription_count
    });

    // Send welcome message
    if ws_sender
        .send(axum::extract::ws::Message::Text(
            welcome_msg.to_string().into(),
        ))
        .await
        .is_err()
    {
        warn!(
            "Failed to send welcome message to subscriber: {}",
            subscriber_id
        );
        pubsub_router.unregister_connection(&subscriber_id);
        return;
    }

    // Process both incoming WebSocket messages and outgoing Pub/Sub messages
    loop {
        tokio::select! {
            // Receive messages from Pub/Sub channel
            Some(message) = rx.recv() => {
                let msg_json = serde_json::to_string(&json!({
                    "type": "message",
                    "message_id": message.id,
                    "topic": message.topic,
                    "payload": message.payload,
                    "metadata": message.metadata,
                    "timestamp": message.timestamp
                }))
                .unwrap();

                if ws_sender
                    .send(axum::extract::ws::Message::Text(msg_json.into()))
                    .await
                    .is_err()
                {
                    warn!("Failed to send message to subscriber: {}", subscriber_id);
                    break;
                }
            }

            // Handle incoming WebSocket messages (keepalive/pings)
            Some(msg) = ws_receiver.next() => {
                match msg {
                    Ok(axum::extract::ws::Message::Close(_)) => {
                        info!("Subscriber {} closed connection", subscriber_id);
                        break;
                    }
                    Ok(axum::extract::ws::Message::Ping(data)) => {
                        if ws_sender.send(axum::extract::ws::Message::Pong(data)).await.is_err() {
                            break;
                        }
                    }
                    Ok(_) => {
                        // Ignore other message types
                    }
                    Err(e) => {
                        warn!("WebSocket error for subscriber {}: {}", subscriber_id, e);
                        break;
                    }
                }
            }

            else => {
                // Both channels closed
                break;
            }
        }
    }

    // Cleanup
    pubsub_router.unregister_connection(&subscriber_id);
    let _ = pubsub_router.unsubscribe(&subscriber_id, None);
    info!("Subscriber {} disconnected and cleaned up", subscriber_id);
}

// ============================================================================
// Pub/Sub REST API Handlers
// ============================================================================

// Pub/Sub REST API types
#[derive(Debug, Deserialize)]
pub struct SubscribeRequest {
    pub topics: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct PublishMessageRequest {
    pub payload: serde_json::Value,
    pub metadata: Option<HashMap<String, String>>,
}

#[derive(Debug, Deserialize)]
pub struct UnsubscribeRequest {
    pub subscriber_id: String,
    pub topics: Option<Vec<String>>,
}

/// POST /pubsub/subscribe - Subscribe to topics
pub async fn pubsub_subscribe(
    State(state): State<AppState>,
    Json(req): Json<SubscribeRequest>,
) -> Result<Json<serde_json::Value>, Json<serde_json::Value>> {
    debug!("POST /pubsub/subscribe - topics: {:?}", req.topics);

    let pubsub_router = state.pubsub_router.as_ref().ok_or_else(|| {
        Json(serde_json::json!({
            "error": "Pub/Sub system disabled"
        }))
    })?;

    match pubsub_router.subscribe(req.topics) {
        Ok(result) => Ok(Json(serde_json::json!({
            "subscriber_id": result.subscriber_id,
            "topics": result.topics,
            "subscription_count": result.subscription_count,
        }))),
        Err(e) => {
            error!("Subscribe error: {}", e);
            Err(Json(serde_json::json!({
                "error": e.to_string()
            })))
        }
    }
}

/// POST /pubsub/:topic/publish - Publish message to topic
pub async fn pubsub_publish(
    State(state): State<AppState>,
    Path(topic): Path<String>,
    Json(req): Json<PublishMessageRequest>,
) -> Result<Json<serde_json::Value>, Json<serde_json::Value>> {
    debug!("POST /pubsub/{}/publish", topic);

    let pubsub_router = state.pubsub_router.as_ref().ok_or_else(|| {
        Json(serde_json::json!({
            "error": "Pub/Sub system disabled"
        }))
    })?;

    match pubsub_router.publish(&topic, req.payload, req.metadata) {
        Ok(result) => Ok(Json(serde_json::json!({
            "message_id": result.message_id,
            "topic": result.topic,
            "subscribers_matched": result.subscribers_matched,
        }))),
        Err(e) => {
            error!("Publish error: {}", e);
            Err(Json(serde_json::json!({
                "error": e.to_string()
            })))
        }
    }
}

/// POST /pubsub/unsubscribe - Unsubscribe from topics
pub async fn pubsub_unsubscribe(
    State(state): State<AppState>,
    Json(req): Json<UnsubscribeRequest>,
) -> Result<Json<serde_json::Value>, Json<serde_json::Value>> {
    debug!(
        "POST /pubsub/unsubscribe - subscriber_id: {}",
        req.subscriber_id
    );

    let pubsub_router = state.pubsub_router.as_ref().ok_or_else(|| {
        Json(serde_json::json!({
            "error": "Pub/Sub system disabled"
        }))
    })?;

    match pubsub_router.unsubscribe(&req.subscriber_id, req.topics) {
        Ok(count) => Ok(Json(serde_json::json!({
            "unsubscribed": count
        }))),
        Err(e) => {
            error!("Unsubscribe error: {}", e);
            Err(Json(serde_json::json!({
                "error": e.to_string()
            })))
        }
    }
}

/// GET /pubsub/stats - Get Pub/Sub statistics
pub async fn pubsub_stats(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, Json<serde_json::Value>> {
    debug!("GET /pubsub/stats");

    let pubsub_router = state.pubsub_router.as_ref().ok_or_else(|| {
        Json(serde_json::json!({
            "error": "Pub/Sub system disabled"
        }))
    })?;

    let stats = pubsub_router.get_stats();
    Ok(Json(serde_json::to_value(stats).unwrap()))
}

/// GET /pubsub/topics - List all topics
pub async fn pubsub_list_topics(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, Json<serde_json::Value>> {
    debug!("GET /pubsub/topics");

    let pubsub_router = state.pubsub_router.as_ref().ok_or_else(|| {
        Json(serde_json::json!({
            "error": "Pub/Sub system disabled"
        }))
    })?;

    let topics = pubsub_router.list_topics();
    Ok(Json(serde_json::json!({
        "topics": topics,
        "count": topics.len()
    })))
}

/// GET /pubsub/:topic/info - Get topic information
pub async fn pubsub_topic_info(
    State(state): State<AppState>,
    Path(topic): Path<String>,
) -> Result<Json<serde_json::Value>, Json<serde_json::Value>> {
    debug!("GET /pubsub/{}/info", topic);

    let pubsub_router = state.pubsub_router.as_ref().ok_or_else(|| {
        Json(serde_json::json!({
            "error": "Pub/Sub system disabled"
        }))
    })?;

    match pubsub_router.get_topic_info(&topic) {
        Some(info) => Ok(Json(serde_json::to_value(info).unwrap())),
        None => Err(Json(serde_json::json!({
            "error": "Topic not found"
        }))),
    }
}

// ============================================================================
// Pub/Sub StreamableHTTP Command Handlers
// ============================================================================

async fn handle_pubsub_subscribe_cmd(
    state: &AppState,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let pubsub_router = state
        .pubsub_router
        .as_ref()
        .ok_or_else(|| SynapError::InvalidRequest("Pub/Sub system disabled".to_string()))?;

    let topics_val = request
        .payload
        .get("topics")
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'topics' field".to_string()))?;

    let topics_arr = topics_val
        .as_array()
        .ok_or_else(|| SynapError::InvalidRequest("'topics' must be an array".to_string()))?;

    let topics: Vec<String> = topics_arr
        .iter()
        .filter_map(|v| v.as_str().map(|s| s.to_string()))
        .collect();

    if topics.is_empty() {
        return Err(SynapError::InvalidRequest(
            "At least one topic required".to_string(),
        ));
    }

    let result = pubsub_router.subscribe(topics)?;

    Ok(serde_json::json!({
        "subscriber_id": result.subscriber_id,
        "topics": result.topics,
        "subscription_count": result.subscription_count
    }))
}

async fn handle_pubsub_publish_cmd(
    state: &AppState,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let pubsub_router = state
        .pubsub_router
        .as_ref()
        .ok_or_else(|| SynapError::InvalidRequest("Pub/Sub system disabled".to_string()))?;

    let topic = request
        .payload
        .get("topic")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'topic' field".to_string()))?;

    let payload = request
        .payload
        .get("payload")
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'payload' field".to_string()))?
        .clone();

    let metadata = request.payload.get("metadata").and_then(|v| {
        if let serde_json::Value::Object(map) = v {
            Some(
                map.iter()
                    .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                    .collect(),
            )
        } else {
            None
        }
    });

    let result = pubsub_router.publish(topic, payload, metadata)?;

    Ok(serde_json::json!({
        "message_id": result.message_id,
        "topic": result.topic,
        "subscribers_matched": result.subscribers_matched
    }))
}

async fn handle_pubsub_unsubscribe_cmd(
    state: &AppState,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let pubsub_router = state
        .pubsub_router
        .as_ref()
        .ok_or_else(|| SynapError::InvalidRequest("Pub/Sub system disabled".to_string()))?;

    let subscriber_id = request
        .payload
        .get("subscriber_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'subscriber_id' field".to_string()))?;

    let topics = request.payload.get("topics").and_then(|v| {
        v.as_array().map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
    });

    let count = pubsub_router.unsubscribe(subscriber_id, topics)?;

    Ok(serde_json::json!({ "unsubscribed": count }))
}

async fn handle_pubsub_stats_cmd(
    state: &AppState,
    _request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let pubsub_router = state
        .pubsub_router
        .as_ref()
        .ok_or_else(|| SynapError::InvalidRequest("Pub/Sub system disabled".to_string()))?;

    let stats = pubsub_router.get_stats();
    serde_json::to_value(stats).map_err(|e| SynapError::SerializationError(e.to_string()))
}

async fn handle_pubsub_topics_cmd(
    state: &AppState,
    _request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let pubsub_router = state
        .pubsub_router
        .as_ref()
        .ok_or_else(|| SynapError::InvalidRequest("Pub/Sub system disabled".to_string()))?;

    let topics = pubsub_router.list_topics();
    Ok(serde_json::json!({
        "topics": topics,
        "count": topics.len()
    }))
}

async fn handle_pubsub_info_cmd(
    state: &AppState,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let pubsub_router = state
        .pubsub_router
        .as_ref()
        .ok_or_else(|| SynapError::InvalidRequest("Pub/Sub system disabled".to_string()))?;

    let topic = request
        .payload
        .get("topic")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'topic' field".to_string()))?;

    match pubsub_router.get_topic_info(topic) {
        Some(info) => Ok(serde_json::to_value(info)
            .map_err(|e| SynapError::SerializationError(e.to_string()))?),
        None => Err(SynapError::InvalidRequest(format!(
            "Topic '{}' not found",
            topic
        ))),
    }
}

// ============================================================================
// Event Streams StreamableHTTP Command Handlers
// ============================================================================

async fn handle_stream_create_cmd(
    state: &AppState,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let stream_manager = state
        .stream_manager
        .as_ref()
        .ok_or_else(|| SynapError::InvalidRequest("Stream system disabled".to_string()))?;

    let room = request
        .payload
        .get("room")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'room' field".to_string()))?;

    stream_manager
        .create_room(room)
        .await
        .map_err(SynapError::InvalidRequest)?;

    Ok(serde_json::json!({
        "success": true,
        "room": room
    }))
}

async fn handle_stream_publish_cmd(
    state: &AppState,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let stream_manager = state
        .stream_manager
        .as_ref()
        .ok_or_else(|| SynapError::InvalidRequest("Stream system disabled".to_string()))?;

    let room = request
        .payload
        .get("room")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'room' field".to_string()))?;

    let event = request
        .payload
        .get("event")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'event' field".to_string()))?;

    let data = request
        .payload
        .get("data")
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'data' field".to_string()))?;

    let data_bytes =
        serde_json::to_vec(data).map_err(|e| SynapError::SerializationError(e.to_string()))?;

    let offset = stream_manager
        .publish(room, event, data_bytes)
        .await
        .map_err(SynapError::InvalidRequest)?;

    Ok(serde_json::json!({
        "offset": offset,
        "room": room
    }))
}

async fn handle_stream_consume_cmd(
    state: &AppState,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let stream_manager = state
        .stream_manager
        .as_ref()
        .ok_or_else(|| SynapError::InvalidRequest("Stream system disabled".to_string()))?;

    let room = request
        .payload
        .get("room")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'room' field".to_string()))?;

    let subscriber_id = request
        .payload
        .get("subscriber_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'subscriber_id' field".to_string()))?;

    let from_offset = request
        .payload
        .get("from_offset")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);

    let limit = request
        .payload
        .get("limit")
        .and_then(|v| v.as_u64())
        .unwrap_or(100) as usize;

    let events = stream_manager
        .consume(room, subscriber_id, from_offset, limit)
        .await
        .map_err(SynapError::InvalidRequest)?;

    let next_offset = events.last().map(|e| e.offset + 1).unwrap_or(from_offset);

    Ok(serde_json::json!({
        "events": events,
        "next_offset": next_offset
    }))
}

async fn handle_stream_stats_cmd(
    state: &AppState,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let stream_manager = state
        .stream_manager
        .as_ref()
        .ok_or_else(|| SynapError::InvalidRequest("Stream system disabled".to_string()))?;

    let room = request
        .payload
        .get("room")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'room' field".to_string()))?;

    let stats = stream_manager
        .room_stats(room)
        .await
        .map_err(SynapError::InvalidRequest)?;

    serde_json::to_value(stats).map_err(|e| SynapError::SerializationError(e.to_string()))
}

async fn handle_stream_list_cmd(
    state: &AppState,
    _request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let stream_manager = state
        .stream_manager
        .as_ref()
        .ok_or_else(|| SynapError::InvalidRequest("Stream system disabled".to_string()))?;

    let rooms = stream_manager.list_rooms().await;

    Ok(serde_json::json!({
        "rooms": rooms,
        "count": rooms.len()
    }))
}

async fn handle_stream_delete_cmd(
    state: &AppState,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let stream_manager = state
        .stream_manager
        .as_ref()
        .ok_or_else(|| SynapError::InvalidRequest("Stream system disabled".to_string()))?;

    let room = request
        .payload
        .get("room")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'room' field".to_string()))?;

    stream_manager
        .delete_room(room)
        .await
        .map_err(SynapError::InvalidRequest)?;

    Ok(serde_json::json!({
        "success": true,
        "deleted": room
    }))
}

// =====================================
// Partitioned Stream Handlers (Kafka-style)
// =====================================

#[derive(Debug, Deserialize)]
pub struct CreateTopicRequest {
    pub num_partitions: Option<usize>,
    pub replication_factor: Option<usize>,
    pub retention_policy: Option<RetentionPolicyRequest>,
    pub segment_bytes: Option<u64>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum RetentionPolicyRequest {
    Time {
        retention_secs: u64,
    },
    Size {
        max_bytes: u64,
    },
    Messages {
        max_messages: u64,
    },
    Combined {
        retention_secs: Option<u64>,
        max_bytes: Option<u64>,
        max_messages: Option<u64>,
    },
    Infinite,
}

impl From<RetentionPolicyRequest> for crate::core::RetentionPolicy {
    fn from(req: RetentionPolicyRequest) -> Self {
        match req {
            RetentionPolicyRequest::Time { retention_secs } => {
                crate::core::RetentionPolicy::Time { retention_secs }
            }
            RetentionPolicyRequest::Size { max_bytes } => {
                crate::core::RetentionPolicy::Size { max_bytes }
            }
            RetentionPolicyRequest::Messages { max_messages } => {
                crate::core::RetentionPolicy::Messages { max_messages }
            }
            RetentionPolicyRequest::Combined {
                retention_secs,
                max_bytes,
                max_messages,
            } => crate::core::RetentionPolicy::Combined {
                retention_secs,
                max_bytes,
                max_messages,
            },
            RetentionPolicyRequest::Infinite => crate::core::RetentionPolicy::Infinite,
        }
    }
}

/// Create partitioned topic
pub async fn create_partitioned_topic(
    State(state): State<AppState>,
    Path(topic): Path<String>,
    Json(req): Json<CreateTopicRequest>,
) -> Result<Json<serde_json::Value>, SynapError> {
    let partition_manager = state
        .partition_manager
        .as_ref()
        .ok_or_else(|| SynapError::InvalidRequest("Partition system disabled".to_string()))?;

    let mut config = crate::core::PartitionConfig::default();
    if let Some(num) = req.num_partitions {
        config.num_partitions = num;
    }
    if let Some(rep) = req.replication_factor {
        config.replication_factor = rep;
    }
    if let Some(retention) = req.retention_policy {
        config.retention = retention.into();
    }
    if let Some(seg) = req.segment_bytes {
        config.segment_bytes = seg;
    }

    partition_manager
        .create_topic(&topic, Some(config.clone()))
        .await
        .map_err(SynapError::InvalidRequest)?;

    Ok(Json(json!({
        "success": true,
        "topic": topic,
        "num_partitions": config.num_partitions,
        "replication_factor": config.replication_factor
    })))
}

#[derive(Debug, Deserialize)]
pub struct PartitionPublishRequest {
    pub event_type: String,
    pub key: Option<String>,
    pub data: serde_json::Value,
}

/// Publish to partitioned topic
pub async fn publish_to_partition(
    State(state): State<AppState>,
    Path(topic): Path<String>,
    Json(req): Json<PartitionPublishRequest>,
) -> Result<Json<serde_json::Value>, SynapError> {
    let partition_manager = state
        .partition_manager
        .as_ref()
        .ok_or_else(|| SynapError::InvalidRequest("Partition system disabled".to_string()))?;

    let data =
        serde_json::to_vec(&req.data).map_err(|e| SynapError::SerializationError(e.to_string()))?;

    let key = req.key.map(|k| k.into_bytes());

    let (partition_id, offset) = partition_manager
        .publish(&topic, &req.event_type, key, data)
        .await
        .map_err(SynapError::InvalidRequest)?;

    Ok(Json(json!({
        "partition_id": partition_id,
        "offset": offset,
        "topic": topic
    })))
}

#[derive(Debug, Deserialize)]
pub struct ConsumePartitionRequest {
    pub from_offset: Option<u64>,
    pub limit: Option<usize>,
}

/// Consume from specific partition
pub async fn consume_from_partition(
    State(state): State<AppState>,
    Path((topic, partition_id)): Path<(String, usize)>,
    Json(req): Json<ConsumePartitionRequest>,
) -> Result<Json<serde_json::Value>, SynapError> {
    let partition_manager = state
        .partition_manager
        .as_ref()
        .ok_or_else(|| SynapError::InvalidRequest("Partition system disabled".to_string()))?;

    let from_offset = req.from_offset.unwrap_or(0);
    let limit = req.limit.unwrap_or(100).min(1000);

    let events = partition_manager
        .consume_partition(&topic, partition_id, from_offset, limit)
        .await
        .map_err(SynapError::InvalidRequest)?;

    let next_offset = events.last().map(|e| e.offset + 1).unwrap_or(from_offset);

    Ok(Json(json!({
        "topic": topic,
        "partition_id": partition_id,
        "events": events,
        "next_offset": next_offset,
        "count": events.len()
    })))
}

/// Get topic stats
pub async fn get_topic_stats(
    State(state): State<AppState>,
    Path(topic): Path<String>,
) -> Result<Json<serde_json::Value>, SynapError> {
    let partition_manager = state
        .partition_manager
        .as_ref()
        .ok_or_else(|| SynapError::InvalidRequest("Partition system disabled".to_string()))?;

    let stats = partition_manager
        .topic_stats(&topic)
        .await
        .map_err(SynapError::InvalidRequest)?;

    Ok(Json(json!({
        "topic": topic,
        "partitions": stats
    })))
}

/// List all topics
pub async fn list_topics(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, SynapError> {
    let partition_manager = state
        .partition_manager
        .as_ref()
        .ok_or_else(|| SynapError::InvalidRequest("Partition system disabled".to_string()))?;

    let topics = partition_manager.list_topics().await;

    Ok(Json(json!({
        "topics": topics,
        "count": topics.len()
    })))
}

/// Delete topic
pub async fn delete_topic(
    State(state): State<AppState>,
    Path(topic): Path<String>,
) -> Result<Json<serde_json::Value>, SynapError> {
    let partition_manager = state
        .partition_manager
        .as_ref()
        .ok_or_else(|| SynapError::InvalidRequest("Partition system disabled".to_string()))?;

    partition_manager
        .delete_topic(&topic)
        .await
        .map_err(SynapError::InvalidRequest)?;

    Ok(Json(json!({
        "success": true,
        "deleted": topic
    })))
}

// =====================================
// Consumer Group Handlers
// =====================================

#[derive(Debug, Deserialize)]
pub struct CreateConsumerGroupRequest {
    pub topic: String,
    pub partition_count: usize,
    pub strategy: Option<String>,
    pub session_timeout_secs: Option<u64>,
}

/// Create consumer group
pub async fn create_consumer_group(
    State(state): State<AppState>,
    Path(group_id): Path<String>,
    Json(req): Json<CreateConsumerGroupRequest>,
) -> Result<Json<serde_json::Value>, SynapError> {
    let consumer_group_manager = state
        .consumer_group_manager
        .as_ref()
        .ok_or_else(|| SynapError::InvalidRequest("Consumer group system disabled".to_string()))?;

    let mut config = crate::core::ConsumerGroupConfig::default();

    if let Some(strategy) = req.strategy {
        config.strategy = match strategy.as_str() {
            "round_robin" => crate::core::AssignmentStrategy::RoundRobin,
            "range" => crate::core::AssignmentStrategy::Range,
            "sticky" => crate::core::AssignmentStrategy::Sticky,
            _ => {
                return Err(SynapError::InvalidRequest(
                    "Invalid assignment strategy".to_string(),
                ));
            }
        };
    }

    if let Some(timeout) = req.session_timeout_secs {
        config.session_timeout_secs = timeout;
    }

    consumer_group_manager
        .create_group(&group_id, &req.topic, req.partition_count, Some(config))
        .await
        .map_err(SynapError::InvalidRequest)?;

    Ok(Json(json!({
        "success": true,
        "group_id": group_id,
        "topic": req.topic
    })))
}

#[derive(Debug, Deserialize)]
pub struct JoinGroupRequest {
    pub session_timeout_secs: Option<u64>,
}

/// Join consumer group
pub async fn join_consumer_group(
    State(state): State<AppState>,
    Path(group_id): Path<String>,
    Json(req): Json<JoinGroupRequest>,
) -> Result<Json<serde_json::Value>, SynapError> {
    let consumer_group_manager = state
        .consumer_group_manager
        .as_ref()
        .ok_or_else(|| SynapError::InvalidRequest("Consumer group system disabled".to_string()))?;

    let timeout = req.session_timeout_secs.unwrap_or(30);

    let member = consumer_group_manager
        .join_group(&group_id, timeout)
        .await
        .map_err(SynapError::InvalidRequest)?;

    // Trigger rebalance
    let _ = consumer_group_manager.rebalance_group(&group_id).await;

    Ok(Json(json!({
        "member_id": member.id,
        "group_id": member.group_id
    })))
}

/// Leave consumer group
pub async fn leave_consumer_group(
    State(state): State<AppState>,
    Path((group_id, member_id)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>, SynapError> {
    let consumer_group_manager = state
        .consumer_group_manager
        .as_ref()
        .ok_or_else(|| SynapError::InvalidRequest("Consumer group system disabled".to_string()))?;

    consumer_group_manager
        .leave_group(&group_id, &member_id)
        .await
        .map_err(SynapError::InvalidRequest)?;

    // Trigger rebalance
    let _ = consumer_group_manager.rebalance_group(&group_id).await;

    Ok(Json(json!({
        "success": true,
        "member_id": member_id
    })))
}

/// Get partition assignment
pub async fn get_partition_assignment(
    State(state): State<AppState>,
    Path((group_id, member_id)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>, SynapError> {
    let consumer_group_manager = state
        .consumer_group_manager
        .as_ref()
        .ok_or_else(|| SynapError::InvalidRequest("Consumer group system disabled".to_string()))?;

    let assignment = consumer_group_manager
        .get_assignment(&group_id, &member_id)
        .await
        .map_err(SynapError::InvalidRequest)?;

    Ok(Json(json!({
        "member_id": member_id,
        "group_id": group_id,
        "partitions": assignment
    })))
}

#[derive(Debug, Deserialize)]
pub struct CommitOffsetRequest {
    pub partition_id: usize,
    pub offset: u64,
}

/// Commit offset
pub async fn commit_offset(
    State(state): State<AppState>,
    Path(group_id): Path<String>,
    Json(req): Json<CommitOffsetRequest>,
) -> Result<Json<serde_json::Value>, SynapError> {
    let consumer_group_manager = state
        .consumer_group_manager
        .as_ref()
        .ok_or_else(|| SynapError::InvalidRequest("Consumer group system disabled".to_string()))?;

    consumer_group_manager
        .commit_offset(&group_id, req.partition_id, req.offset)
        .await
        .map_err(SynapError::InvalidRequest)?;

    Ok(Json(json!({
        "success": true,
        "partition_id": req.partition_id,
        "offset": req.offset
    })))
}

/// Get committed offset
pub async fn get_committed_offset(
    State(state): State<AppState>,
    Path((group_id, partition_id)): Path<(String, usize)>,
) -> Result<Json<serde_json::Value>, SynapError> {
    let consumer_group_manager = state
        .consumer_group_manager
        .as_ref()
        .ok_or_else(|| SynapError::InvalidRequest("Consumer group system disabled".to_string()))?;

    let offset = consumer_group_manager
        .get_offset(&group_id, partition_id)
        .await
        .map_err(SynapError::InvalidRequest)?;

    Ok(Json(json!({
        "group_id": group_id,
        "partition_id": partition_id,
        "offset": offset
    })))
}

/// Get consumer group stats
pub async fn get_consumer_group_stats(
    State(state): State<AppState>,
    Path(group_id): Path<String>,
) -> Result<Json<serde_json::Value>, SynapError> {
    let consumer_group_manager = state
        .consumer_group_manager
        .as_ref()
        .ok_or_else(|| SynapError::InvalidRequest("Consumer group system disabled".to_string()))?;

    let stats = consumer_group_manager
        .group_stats(&group_id)
        .await
        .map_err(SynapError::InvalidRequest)?;

    Ok(Json(serde_json::to_value(stats).map_err(|e| {
        SynapError::SerializationError(e.to_string())
    })?))
}

/// List consumer groups
pub async fn list_consumer_groups(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, SynapError> {
    let consumer_group_manager = state
        .consumer_group_manager
        .as_ref()
        .ok_or_else(|| SynapError::InvalidRequest("Consumer group system disabled".to_string()))?;

    let groups = consumer_group_manager.list_groups().await;

    Ok(Json(json!({
        "groups": groups,
        "count": groups.len()
    })))
}

/// Heartbeat from consumer
pub async fn consumer_heartbeat(
    State(state): State<AppState>,
    Path((group_id, member_id)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>, SynapError> {
    let consumer_group_manager = state
        .consumer_group_manager
        .as_ref()
        .ok_or_else(|| SynapError::InvalidRequest("Consumer group system disabled".to_string()))?;

    consumer_group_manager
        .heartbeat(&group_id, &member_id)
        .await
        .map_err(SynapError::InvalidRequest)?;

    Ok(Json(json!({
        "success": true
    })))
}

// ============================================================================
// Set Handlers
// ============================================================================

// Request/Response types for Set operations
#[derive(Debug, Deserialize)]
pub struct SetAddRequest {
    pub members: Vec<serde_json::Value>,
}

#[derive(Debug, Serialize)]
pub struct SetAddResponse {
    pub added: usize,
    pub key: String,
}

#[derive(Debug, Deserialize)]
pub struct SetRemRequest {
    pub members: Vec<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct SetMemberRequest {
    pub member: serde_json::Value,
}

#[derive(Debug, Serialize)]
pub struct SetStatsResponse {
    pub total_sets: usize,
    pub total_members: usize,
    pub operations: SetOperationStats,
}

#[derive(Debug, Serialize)]
pub struct SetOperationStats {
    pub sadd_count: u64,
    pub srem_count: u64,
    pub sismember_count: u64,
    pub smembers_count: u64,
    pub scard_count: u64,
    pub spop_count: u64,
    pub srandmember_count: u64,
    pub smove_count: u64,
    pub sinter_count: u64,
    pub sunion_count: u64,
    pub sdiff_count: u64,
}

/// POST /set/:key/add - Add member(s) to set
pub async fn set_add(
    State(state): State<AppState>,
    Path(key): Path<String>,
    Json(req): Json<SetAddRequest>,
) -> Result<Json<SetAddResponse>, SynapError> {
    debug!("REST SADD key={} count={}", key, req.members.len());

    let members: Result<Vec<Vec<u8>>, _> = req
        .members
        .into_iter()
        .map(|v| serde_json::to_vec(&v).map_err(|e| SynapError::InvalidValue(e.to_string())))
        .collect();

    let added = state.set_store.sadd(&key, members?)?;

    Ok(Json(SetAddResponse { added, key }))
}

/// POST /set/:key/rem - Remove member(s) from set
pub async fn set_rem(
    State(state): State<AppState>,
    Path(key): Path<String>,
    Json(req): Json<SetRemRequest>,
) -> Result<Json<serde_json::Value>, SynapError> {
    debug!("REST SREM key={} count={}", key, req.members.len());

    let members: Result<Vec<Vec<u8>>, _> = req
        .members
        .into_iter()
        .map(|v| serde_json::to_vec(&v).map_err(|e| SynapError::InvalidValue(e.to_string())))
        .collect();

    let removed = state.set_store.srem(&key, members?)?;

    Ok(Json(json!({ "removed": removed, "key": key })))
}

/// POST /set/:key/ismember - Check if member exists
pub async fn set_ismember(
    State(state): State<AppState>,
    Path(key): Path<String>,
    Json(req): Json<SetMemberRequest>,
) -> Result<Json<serde_json::Value>, SynapError> {
    debug!("REST SISMEMBER key={}", key);

    let member = serde_json::to_vec(&req.member)
        .map_err(|e| SynapError::InvalidValue(format!("Failed to serialize member: {}", e)))?;

    let is_member = state.set_store.sismember(&key, member)?;

    Ok(Json(json!({ "is_member": is_member, "key": key })))
}

/// GET /set/:key/members - Get all members
pub async fn set_members(
    State(state): State<AppState>,
    Path(key): Path<String>,
) -> Result<Json<serde_json::Value>, SynapError> {
    debug!("REST SMEMBERS key={}", key);

    let members = state.set_store.smembers(&key)?;

    let json_members: Vec<serde_json::Value> = members
        .into_iter()
        .map(|m| {
            serde_json::from_slice(&m).unwrap_or_else(|_| {
                serde_json::Value::String(String::from_utf8_lossy(&m).to_string())
            })
        })
        .collect();

    Ok(Json(json!({ "members": json_members, "key": key })))
}

/// GET /set/:key/card - Get set cardinality
pub async fn set_card(
    State(state): State<AppState>,
    Path(key): Path<String>,
) -> Result<Json<serde_json::Value>, SynapError> {
    debug!("REST SCARD key={}", key);

    let count = state.set_store.scard(&key)?;

    Ok(Json(json!({ "count": count, "key": key })))
}

/// POST /set/:key/pop - Remove and return random member(s)
pub async fn set_pop(
    State(state): State<AppState>,
    Path(key): Path<String>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Result<Json<serde_json::Value>, SynapError> {
    let count = params.get("count").and_then(|s| s.parse::<usize>().ok());

    debug!("REST SPOP key={} count={:?}", key, count);

    let members = state.set_store.spop(&key, count)?;

    let json_members: Vec<serde_json::Value> = members
        .into_iter()
        .map(|m| {
            serde_json::from_slice(&m).unwrap_or_else(|_| {
                serde_json::Value::String(String::from_utf8_lossy(&m).to_string())
            })
        })
        .collect();

    Ok(Json(json!({ "members": json_members, "key": key })))
}

/// GET /set/:key/randmember - Get random member(s) without removing
pub async fn set_randmember(
    State(state): State<AppState>,
    Path(key): Path<String>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Result<Json<serde_json::Value>, SynapError> {
    let count = params.get("count").and_then(|s| s.parse::<usize>().ok());

    debug!("REST SRANDMEMBER key={} count={:?}", key, count);

    let members = state.set_store.srandmember(&key, count)?;

    let json_members: Vec<serde_json::Value> = members
        .into_iter()
        .map(|m| {
            serde_json::from_slice(&m).unwrap_or_else(|_| {
                serde_json::Value::String(String::from_utf8_lossy(&m).to_string())
            })
        })
        .collect();

    Ok(Json(json!({ "members": json_members, "key": key })))
}

/// POST /set/:source/move/:destination - Move member between sets
pub async fn set_move(
    State(state): State<AppState>,
    Path((source, destination)): Path<(String, String)>,
    Json(req): Json<SetMemberRequest>,
) -> Result<Json<serde_json::Value>, SynapError> {
    debug!("REST SMOVE source={} destination={}", source, destination);

    let member = serde_json::to_vec(&req.member)
        .map_err(|e| SynapError::InvalidValue(format!("Failed to serialize member: {}", e)))?;

    let moved = state.set_store.smove(&source, &destination, member)?;

    Ok(Json(
        json!({ "moved": moved, "source": source, "destination": destination }),
    ))
}

/// POST /set/inter - Intersection of multiple sets
pub async fn set_inter(
    State(state): State<AppState>,
    Json(req): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, SynapError> {
    let keys: Vec<String> = req
        .get("keys")
        .and_then(|v| v.as_array())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'keys' array".to_string()))?
        .iter()
        .filter_map(|v| v.as_str().map(String::from))
        .collect();

    debug!("REST SINTER keys={:?}", keys);

    let members = state.set_store.sinter(&keys)?;

    let json_members: Vec<serde_json::Value> = members
        .into_iter()
        .map(|m| {
            serde_json::from_slice(&m).unwrap_or_else(|_| {
                serde_json::Value::String(String::from_utf8_lossy(&m).to_string())
            })
        })
        .collect();

    Ok(Json(json!({ "members": json_members })))
}

/// POST /set/union - Union of multiple sets
pub async fn set_union(
    State(state): State<AppState>,
    Json(req): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, SynapError> {
    let keys: Vec<String> = req
        .get("keys")
        .and_then(|v| v.as_array())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'keys' array".to_string()))?
        .iter()
        .filter_map(|v| v.as_str().map(String::from))
        .collect();

    debug!("REST SUNION keys={:?}", keys);

    let members = state.set_store.sunion(&keys)?;

    let json_members: Vec<serde_json::Value> = members
        .into_iter()
        .map(|m| {
            serde_json::from_slice(&m).unwrap_or_else(|_| {
                serde_json::Value::String(String::from_utf8_lossy(&m).to_string())
            })
        })
        .collect();

    Ok(Json(json!({ "members": json_members })))
}

/// POST /set/diff - Difference of sets
pub async fn set_diff(
    State(state): State<AppState>,
    Json(req): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, SynapError> {
    let keys: Vec<String> = req
        .get("keys")
        .and_then(|v| v.as_array())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'keys' array".to_string()))?
        .iter()
        .filter_map(|v| v.as_str().map(String::from))
        .collect();

    debug!("REST SDIFF keys={:?}", keys);

    let members = state.set_store.sdiff(&keys)?;

    let json_members: Vec<serde_json::Value> = members
        .into_iter()
        .map(|m| {
            serde_json::from_slice(&m).unwrap_or_else(|_| {
                serde_json::Value::String(String::from_utf8_lossy(&m).to_string())
            })
        })
        .collect();

    Ok(Json(json!({ "members": json_members })))
}

/// GET /set/stats - Get set statistics
pub async fn set_stats(
    State(state): State<AppState>,
) -> Result<Json<SetStatsResponse>, SynapError> {
    debug!("REST SET STATS");

    let stats = state.set_store.stats();

    Ok(Json(SetStatsResponse {
        total_sets: stats.total_sets,
        total_members: stats.total_members,
        operations: SetOperationStats {
            sadd_count: stats.sadd_count,
            srem_count: stats.srem_count,
            sismember_count: stats.sismember_count,
            smembers_count: stats.smembers_count,
            scard_count: stats.scard_count,
            spop_count: stats.spop_count,
            srandmember_count: stats.srandmember_count,
            smove_count: stats.smove_count,
            sinter_count: stats.sinter_count,
            sunion_count: stats.sunion_count,
            sdiff_count: stats.sdiff_count,
        },
    }))
}

// ============================================================================
// List Handlers
// ============================================================================

// Request/Response types for List operations
#[derive(Debug, Deserialize)]
pub struct ListPushRequest {
    pub values: Vec<serde_json::Value>,
}

#[derive(Debug, Serialize)]
pub struct ListPushResponse {
    pub length: usize,
    pub key: String,
}

#[derive(Debug, Deserialize)]
pub struct ListPopRequest {
    pub count: Option<usize>,
}

#[derive(Debug, Serialize)]
pub struct ListPopResponse {
    pub values: Vec<serde_json::Value>,
    pub key: String,
}

#[derive(Debug, Serialize)]
pub struct ListRangeResponse {
    pub values: Vec<serde_json::Value>,
    pub key: String,
}

#[derive(Debug, Deserialize)]
pub struct ListSetRequest {
    pub index: i64,
    pub value: serde_json::Value,
}

#[derive(Debug, Deserialize)]
pub struct ListRemRequest {
    pub count: i64,
    pub value: serde_json::Value,
}

#[derive(Debug, Deserialize)]
pub struct ListInsertRequest {
    pub before: bool,
    pub pivot: serde_json::Value,
    pub value: serde_json::Value,
}

#[derive(Debug, Serialize)]
pub struct ListStatsResponse {
    pub total_lists: usize,
    pub total_elements: usize,
    pub operations: ListOperationStats,
}

#[derive(Debug, Serialize)]
pub struct ListOperationStats {
    pub lpush_count: u64,
    pub rpush_count: u64,
    pub lpop_count: u64,
    pub rpop_count: u64,
    pub lrange_count: u64,
    pub llen_count: u64,
    pub lindex_count: u64,
    pub lset_count: u64,
    pub ltrim_count: u64,
    pub lrem_count: u64,
    pub linsert_count: u64,
    pub rpoplpush_count: u64,
    pub blpop_count: u64,
    pub brpop_count: u64,
}

/// POST /list/:key/lpush - Push element(s) to left (front)
pub async fn list_lpush(
    State(state): State<AppState>,
    Path(key): Path<String>,
    Json(req): Json<ListPushRequest>,
) -> Result<Json<ListPushResponse>, SynapError> {
    debug!("REST LPUSH key={} count={}", key, req.values.len());

    let values: Result<Vec<Vec<u8>>, _> = req
        .values
        .into_iter()
        .map(|v| serde_json::to_vec(&v).map_err(|e| SynapError::InvalidValue(e.to_string())))
        .collect();

    let length = state.list_store.lpush(&key, values?, false)?;

    Ok(Json(ListPushResponse { length, key }))
}

/// POST /list/:key/lpushx - Push element(s) to left only if key exists
pub async fn list_lpushx(
    State(state): State<AppState>,
    Path(key): Path<String>,
    Json(req): Json<ListPushRequest>,
) -> Result<Json<ListPushResponse>, SynapError> {
    debug!("REST LPUSHX key={} count={}", key, req.values.len());

    let values: Result<Vec<Vec<u8>>, _> = req
        .values
        .into_iter()
        .map(|v| serde_json::to_vec(&v).map_err(|e| SynapError::InvalidValue(e.to_string())))
        .collect();

    let length = state.list_store.lpush(&key, values?, true)?;

    Ok(Json(ListPushResponse { length, key }))
}

/// POST /list/:key/rpush - Push element(s) to right (back)
pub async fn list_rpush(
    State(state): State<AppState>,
    Path(key): Path<String>,
    Json(req): Json<ListPushRequest>,
) -> Result<Json<ListPushResponse>, SynapError> {
    debug!("REST RPUSH key={} count={}", key, req.values.len());

    let values: Result<Vec<Vec<u8>>, _> = req
        .values
        .into_iter()
        .map(|v| serde_json::to_vec(&v).map_err(|e| SynapError::InvalidValue(e.to_string())))
        .collect();

    let length = state.list_store.rpush(&key, values?, false)?;

    Ok(Json(ListPushResponse { length, key }))
}

/// POST /list/:key/rpushx - Push element(s) to right only if key exists
pub async fn list_rpushx(
    State(state): State<AppState>,
    Path(key): Path<String>,
    Json(req): Json<ListPushRequest>,
) -> Result<Json<ListPushResponse>, SynapError> {
    debug!("REST RPUSHX key={} count={}", key, req.values.len());

    let values: Result<Vec<Vec<u8>>, _> = req
        .values
        .into_iter()
        .map(|v| serde_json::to_vec(&v).map_err(|e| SynapError::InvalidValue(e.to_string())))
        .collect();

    let length = state.list_store.rpush(&key, values?, true)?;

    Ok(Json(ListPushResponse { length, key }))
}

/// POST /list/:key/lpop - Pop element(s) from left (front)
pub async fn list_lpop(
    State(state): State<AppState>,
    Path(key): Path<String>,
    Json(req): Json<ListPopRequest>,
) -> Result<Json<ListPopResponse>, SynapError> {
    debug!("REST LPOP key={} count={:?}", key, req.count);

    let values = state.list_store.lpop(&key, req.count)?;

    let json_values: Result<Vec<serde_json::Value>, _> = values
        .into_iter()
        .map(|v| {
            serde_json::from_slice(&v).unwrap_or_else(|_| {
                serde_json::Value::String(String::from_utf8_lossy(&v).to_string())
            })
        })
        .map(Ok)
        .collect();

    Ok(Json(ListPopResponse {
        values: json_values?,
        key,
    }))
}

/// POST /list/:key/rpop - Pop element(s) from right (back)
pub async fn list_rpop(
    State(state): State<AppState>,
    Path(key): Path<String>,
    Json(req): Json<ListPopRequest>,
) -> Result<Json<ListPopResponse>, SynapError> {
    debug!("REST RPOP key={} count={:?}", key, req.count);

    let values = state.list_store.rpop(&key, req.count)?;

    let json_values: Vec<serde_json::Value> = values
        .into_iter()
        .map(|v| {
            serde_json::from_slice(&v).unwrap_or_else(|_| {
                serde_json::Value::String(String::from_utf8_lossy(&v).to_string())
            })
        })
        .collect();

    Ok(Json(ListPopResponse {
        values: json_values,
        key,
    }))
}

/// GET /list/:key/range?start=:start&stop=:stop - Get range of elements
pub async fn list_range(
    State(state): State<AppState>,
    Path(key): Path<String>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Result<Json<ListRangeResponse>, SynapError> {
    let start: i64 = params
        .get("start")
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(0);
    let stop: i64 = params
        .get("stop")
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(-1);

    debug!("REST LRANGE key={} start={} stop={}", key, start, stop);

    let values = state.list_store.lrange(&key, start, stop)?;

    let json_values: Vec<serde_json::Value> = values
        .into_iter()
        .map(|v| {
            serde_json::from_slice(&v).unwrap_or_else(|_| {
                serde_json::Value::String(String::from_utf8_lossy(&v).to_string())
            })
        })
        .collect();

    Ok(Json(ListRangeResponse {
        values: json_values,
        key,
    }))
}

/// GET /list/:key/len - Get list length
pub async fn list_len(
    State(state): State<AppState>,
    Path(key): Path<String>,
) -> Result<Json<serde_json::Value>, SynapError> {
    debug!("REST LLEN key={}", key);

    let length = state.list_store.llen(&key)?;

    Ok(Json(json!({ "length": length, "key": key })))
}

/// GET /list/:key/index/:index - Get element at index
pub async fn list_index(
    State(state): State<AppState>,
    Path((key, index)): Path<(String, i64)>,
) -> Result<Json<serde_json::Value>, SynapError> {
    debug!("REST LINDEX key={} index={}", key, index);

    let value = state.list_store.lindex(&key, index)?;

    let json_value: serde_json::Value = serde_json::from_slice(&value)
        .unwrap_or_else(|_| serde_json::Value::String(String::from_utf8_lossy(&value).to_string()));

    Ok(Json(
        json!({ "value": json_value, "key": key, "index": index }),
    ))
}

/// POST /list/:key/set - Set element at index
pub async fn list_set(
    State(state): State<AppState>,
    Path(key): Path<String>,
    Json(req): Json<ListSetRequest>,
) -> Result<Json<serde_json::Value>, SynapError> {
    debug!("REST LSET key={} index={}", key, req.index);

    let value = serde_json::to_vec(&req.value)
        .map_err(|e| SynapError::InvalidValue(format!("Failed to serialize value: {}", e)))?;

    state.list_store.lset(&key, req.index, value)?;

    Ok(Json(
        json!({ "success": true, "key": key, "index": req.index }),
    ))
}

/// POST /list/:key/trim - Trim list to range
pub async fn list_trim(
    State(state): State<AppState>,
    Path(key): Path<String>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Result<Json<serde_json::Value>, SynapError> {
    let start: i64 = params
        .get("start")
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(0);
    let stop: i64 = params
        .get("stop")
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(-1);

    debug!("REST LTRIM key={} start={} stop={}", key, start, stop);

    state.list_store.ltrim(&key, start, stop)?;

    Ok(Json(json!({ "success": true, "key": key })))
}

/// POST /list/:key/rem - Remove occurrences of value
pub async fn list_rem(
    State(state): State<AppState>,
    Path(key): Path<String>,
    Json(req): Json<ListRemRequest>,
) -> Result<Json<serde_json::Value>, SynapError> {
    debug!("REST LREM key={} count={}", key, req.count);

    let value = serde_json::to_vec(&req.value)
        .map_err(|e| SynapError::InvalidValue(format!("Failed to serialize value: {}", e)))?;

    let removed = state.list_store.lrem(&key, req.count, value)?;

    Ok(Json(json!({ "removed": removed, "key": key })))
}

/// POST /list/:key/insert - Insert value before/after pivot
pub async fn list_insert(
    State(state): State<AppState>,
    Path(key): Path<String>,
    Json(req): Json<ListInsertRequest>,
) -> Result<Json<serde_json::Value>, SynapError> {
    debug!("REST LINSERT key={} before={}", key, req.before);

    let pivot = serde_json::to_vec(&req.pivot)
        .map_err(|e| SynapError::InvalidValue(format!("Failed to serialize pivot: {}", e)))?;

    let value = serde_json::to_vec(&req.value)
        .map_err(|e| SynapError::InvalidValue(format!("Failed to serialize value: {}", e)))?;

    let length = state.list_store.linsert(&key, req.before, pivot, value)?;

    Ok(Json(json!({ "length": length, "key": key })))
}

/// POST /list/:source/rpoplpush/:destination - Atomically pop from source and push to destination
pub async fn list_rpoplpush(
    State(state): State<AppState>,
    Path((source, destination)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>, SynapError> {
    debug!(
        "REST RPOPLPUSH source={} destination={}",
        source, destination
    );

    let value = state.list_store.rpoplpush(&source, &destination)?;

    let json_value: serde_json::Value = serde_json::from_slice(&value)
        .unwrap_or_else(|_| serde_json::Value::String(String::from_utf8_lossy(&value).to_string()));

    Ok(Json(
        json!({ "value": json_value, "source": source, "destination": destination }),
    ))
}

/// GET /list/stats - Get list statistics
pub async fn list_stats(
    State(state): State<AppState>,
) -> Result<Json<ListStatsResponse>, SynapError> {
    debug!("REST LIST STATS");

    let stats = state.list_store.stats();

    Ok(Json(ListStatsResponse {
        total_lists: stats.total_lists,
        total_elements: stats.total_elements,
        operations: ListOperationStats {
            lpush_count: stats.lpush_count,
            rpush_count: stats.rpush_count,
            lpop_count: stats.lpop_count,
            rpop_count: stats.rpop_count,
            lrange_count: stats.lrange_count,
            llen_count: stats.llen_count,
            lindex_count: stats.lindex_count,
            lset_count: stats.lset_count,
            ltrim_count: stats.ltrim_count,
            lrem_count: stats.lrem_count,
            linsert_count: stats.linsert_count,
            rpoplpush_count: stats.rpoplpush_count,
            blpop_count: stats.blpop_count,
            brpop_count: stats.brpop_count,
        },
    }))
}
