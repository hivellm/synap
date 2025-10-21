use crate::core::{KVStore, SynapError};
use crate::protocol::{Request, Response};
use axum::{
    Json,
    extract::{Path, State},
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{debug, error};

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
pub struct GetResponse {
    pub found: bool,
    pub value: Option<serde_json::Value>,
    pub ttl: Option<u64>,
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

/// SET endpoint - store a key-value pair
pub async fn kv_set(
    State(store): State<Arc<KVStore>>,
    Json(req): Json<SetRequest>,
) -> Result<Json<SetResponse>, SynapError> {
    debug!("REST SET key={}", req.key);

    let value_bytes = serde_json::to_vec(&req.value)
        .map_err(|e| SynapError::SerializationError(e.to_string()))?;

    store.set(&req.key, value_bytes, req.ttl).await?;

    Ok(Json(SetResponse {
        success: true,
        key: req.key,
    }))
}

/// GET endpoint - retrieve a value by key
pub async fn kv_get(
    State(store): State<Arc<KVStore>>,
    Path(key): Path<String>,
) -> Result<Json<GetResponse>, SynapError> {
    debug!("REST GET key={}", key);

    let value_bytes = store.get(&key).await?;

    if let Some(bytes) = value_bytes {
        let value: serde_json::Value = serde_json::from_slice(&bytes)
            .map_err(|e| SynapError::SerializationError(e.to_string()))?;

        let ttl = store.ttl(&key).await.ok().flatten();

        Ok(Json(GetResponse {
            found: true,
            value: Some(value),
            ttl,
        }))
    } else {
        Ok(Json(GetResponse {
            found: false,
            value: None,
            ttl: None,
        }))
    }
}

/// DELETE endpoint - delete a key
pub async fn kv_delete(
    State(store): State<Arc<KVStore>>,
    Path(key): Path<String>,
) -> Result<Json<DeleteResponse>, SynapError> {
    debug!("REST DELETE key={}", key);

    let deleted = store.delete(&key).await?;

    Ok(Json(DeleteResponse { deleted, key }))
}

/// STATS endpoint - get store statistics
pub async fn kv_stats(
    State(store): State<Arc<KVStore>>,
) -> Result<Json<StatsResponse>, SynapError> {
    debug!("REST STATS");

    let stats = store.stats().await;

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

/// StreamableHTTP command handler
pub async fn command_handler(
    State(store): State<Arc<KVStore>>,
    Json(request): Json<Request>,
) -> Result<Json<Response>, SynapError> {
    debug!(
        "Command: {} (request_id={})",
        request.command, request.request_id
    );

    let response = handle_command(store, request).await?;
    Ok(Json(response))
}

/// Handle individual commands
async fn handle_command(store: Arc<KVStore>, request: Request) -> Result<Response, SynapError> {
    let request_id = request.request_id.clone();

    let result = match request.command.as_str() {
        "kv.set" => handle_kv_set_cmd(store, &request).await,
        "kv.get" => handle_kv_get_cmd(store, &request).await,
        "kv.del" => handle_kv_del_cmd(store, &request).await,
        "kv.exists" => handle_kv_exists_cmd(store, &request).await,
        "kv.incr" => handle_kv_incr_cmd(store, &request).await,
        "kv.decr" => handle_kv_decr_cmd(store, &request).await,
        "kv.mset" => handle_kv_mset_cmd(store, &request).await,
        "kv.mget" => handle_kv_mget_cmd(store, &request).await,
        "kv.mdel" => handle_kv_mdel_cmd(store, &request).await,
        "kv.scan" => handle_kv_scan_cmd(store, &request).await,
        "kv.stats" => handle_kv_stats_cmd(store, &request).await,
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
    store: Arc<KVStore>,
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

    store.set(key, value_bytes, ttl).await?;

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

    let value_bytes = store.get(key).await?;

    if let Some(bytes) = value_bytes {
        let value: serde_json::Value = serde_json::from_slice(&bytes)
            .map_err(|e| SynapError::SerializationError(e.to_string()))?;

        let ttl = store.ttl(key).await.ok().flatten();

        Ok(serde_json::json!({
            "found": true,
            "value": value,
            "ttl": ttl
        }))
    } else {
        Ok(serde_json::json!({ "found": false }))
    }
}

async fn handle_kv_del_cmd(
    store: Arc<KVStore>,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let key = request
        .payload
        .get("key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'key' field".to_string()))?;

    let deleted = store.delete(key).await?;

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
    store: Arc<KVStore>,
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

    let value = store.incr(key, amount).await?;

    Ok(serde_json::json!({ "value": value }))
}

async fn handle_kv_decr_cmd(
    store: Arc<KVStore>,
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

    let value = store.decr(key, amount).await?;

    Ok(serde_json::json!({ "value": value }))
}

async fn handle_kv_mset_cmd(
    store: Arc<KVStore>,
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

    store.mset(pairs).await?;

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
    store: Arc<KVStore>,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let keys_val = request
        .payload
        .get("keys")
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'keys' field".to_string()))?;

    let keys: Vec<String> = serde_json::from_value(keys_val.clone())
        .map_err(|e| SynapError::InvalidRequest(format!("Invalid keys array: {}", e)))?;

    let count = store.mdel(&keys).await?;

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
