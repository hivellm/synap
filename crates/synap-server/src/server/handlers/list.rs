use super::*;

// ==================== List Command Handlers ====================

pub(super) async fn handle_list_lpush_cmd(
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

    // Check if there's an active transaction for this client_id
    let client_id = request
        .payload
        .get("client_id")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    if !client_id.is_empty() {
        let was_queued = state.transaction_manager.queue_command_if_transaction(
            client_id,
            crate::core::transaction::TransactionCommand::ListLPush {
                key: key.to_string(),
                values: values.clone(),
            },
        )?;

        if was_queued {
            // Command queued in transaction, return success immediately
            return Ok(serde_json::json!({ "success": true, "queued": true }));
        }
    }

    // No active transaction, execute immediately
    let length = state.list_store.lpush(key, values, false)?;

    // Update key version for WATCH (optimistic locking)
    state.transaction_manager.update_key_version(key);

    Ok(serde_json::json!({ "length": length, "key": key }))
}

pub(super) async fn handle_list_lpushx_cmd(
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

pub(super) async fn handle_list_rpush_cmd(
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

    // Check if there's an active transaction for this client_id
    let client_id = request
        .payload
        .get("client_id")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    if !client_id.is_empty() {
        let was_queued = state.transaction_manager.queue_command_if_transaction(
            client_id,
            crate::core::transaction::TransactionCommand::ListRPush {
                key: key.to_string(),
                values: values.clone(),
            },
        )?;

        if was_queued {
            // Command queued in transaction, return success immediately
            return Ok(serde_json::json!({ "success": true, "queued": true }));
        }
    }

    // No active transaction, execute immediately
    let length = state.list_store.rpush(key, values, false)?;

    // Update key version for WATCH (optimistic locking)
    state.transaction_manager.update_key_version(key);

    Ok(serde_json::json!({ "length": length, "key": key }))
}

pub(super) async fn handle_list_rpushx_cmd(
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

pub(super) async fn handle_list_lpop_cmd(
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

    // Check if there's an active transaction for this client_id
    let client_id = request
        .payload
        .get("client_id")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    if !client_id.is_empty() && count == Some(1) {
        // Only support single pop in transactions for now
        let was_queued = state.transaction_manager.queue_command_if_transaction(
            client_id,
            crate::core::transaction::TransactionCommand::ListLPop {
                key: key.to_string(),
            },
        )?;

        if was_queued {
            // Command queued in transaction, return success immediately
            return Ok(serde_json::json!({ "success": true, "queued": true }));
        }
    }

    // No active transaction, execute immediately
    let values = state.list_store.lpop(key, count)?;

    // Update key version for WATCH (optimistic locking) if values were popped
    if !values.is_empty() {
        state.transaction_manager.update_key_version(key);
    }

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

pub(super) async fn handle_list_rpop_cmd(
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

    // Check if there's an active transaction for this client_id
    let client_id = request
        .payload
        .get("client_id")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    if !client_id.is_empty() && count == Some(1) {
        // Only support single pop in transactions for now
        let was_queued = state.transaction_manager.queue_command_if_transaction(
            client_id,
            crate::core::transaction::TransactionCommand::ListRPop {
                key: key.to_string(),
            },
        )?;

        if was_queued {
            // Command queued in transaction, return success immediately
            return Ok(serde_json::json!({ "success": true, "queued": true }));
        }
    }

    // No active transaction, execute immediately
    let values = state.list_store.rpop(key, count)?;

    // Update key version for WATCH (optimistic locking) if values were popped
    if !values.is_empty() {
        state.transaction_manager.update_key_version(key);
    }

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

pub(super) async fn handle_list_lrange_cmd(
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

pub(super) async fn handle_list_llen_cmd(
    state: &AppState,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let key = request
        .payload
        .get("key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'key' field".to_string()))?;

    match state.list_store.llen(key) {
        Ok(length) => Ok(serde_json::json!({ "length": length, "key": key })),
        Err(SynapError::NotFound) => Ok(serde_json::json!({ "length": 0, "key": key })),
        Err(e) => Err(e),
    }
}

pub(super) async fn handle_list_lindex_cmd(
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

pub(super) async fn handle_list_lset_cmd(
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

pub(super) async fn handle_list_ltrim_cmd(
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

pub(super) async fn handle_list_lrem_cmd(
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

pub(super) async fn handle_list_linsert_cmd(
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

pub(super) async fn handle_list_lpos_cmd(
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

pub(super) async fn handle_list_rpoplpush_cmd(
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

pub(super) async fn handle_list_stats_cmd(
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

#[derive(Debug, Deserialize)]
pub struct HyperLogLogAddRequest {
    pub elements: Vec<String>,
    pub ttl_secs: Option<u64>,
}

#[derive(Debug, Serialize)]
pub struct HyperLogLogAddResponse {
    pub key: String,
    pub added: usize,
}

#[derive(Debug, Serialize)]
pub struct HyperLogLogCountResponse {
    pub key: String,
    pub count: u64,
}

#[derive(Debug, Deserialize)]
pub struct HyperLogLogMergeRequest {
    pub sources: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct HyperLogLogMergeResponse {
    pub destination: String,
    pub count: u64,
}

/// POST /list/:key/lpush - Push element(s) to left (front)
pub async fn list_lpush(
    State(state): State<AppState>,
    AuthContextExtractor(ctx): AuthContextExtractor,

    crate::hub::HubContextExtractor(hub_ctx): crate::hub::HubContextExtractor,
    Path(key): Path<String>,
    Json(req): Json<ListPushRequest>,
) -> Result<Json<ListPushResponse>, SynapError> {
    debug!("REST LPUSH key={} count={}", key, req.values.len());

    // Check permission
    require_permission(&ctx, &format!("list:{}", key), Action::Write)?;

    // Apply multi-tenant scoping if Hub mode is active

    let scoped_key =
        crate::hub::MultiTenant::scope_kv_key(hub_ctx.as_ref().map(|c| c.user_id()), &key);

    let values: Result<Vec<Vec<u8>>, _> = req
        .values
        .into_iter()
        .map(|v| serde_json::to_vec(&v).map_err(|e| SynapError::InvalidValue(e.to_string())))
        .collect();

    let length = state.list_store.lpush(&scoped_key, values?, false)?;

    Ok(Json(ListPushResponse { length, key }))
}

/// POST /list/:key/lpushx - Push element(s) to left only if key exists
pub async fn list_lpushx(
    State(state): State<AppState>,
    AuthContextExtractor(ctx): AuthContextExtractor,

    crate::hub::HubContextExtractor(hub_ctx): crate::hub::HubContextExtractor,
    Path(key): Path<String>,
    Json(req): Json<ListPushRequest>,
) -> Result<Json<ListPushResponse>, SynapError> {
    debug!("REST LPUSHX key={} count={}", key, req.values.len());

    // Check permission
    require_permission(&ctx, &format!("list:{}", key), Action::Write)?;

    // Apply multi-tenant scoping if Hub mode is active

    let scoped_key =
        crate::hub::MultiTenant::scope_kv_key(hub_ctx.as_ref().map(|c| c.user_id()), &key);

    let values: Result<Vec<Vec<u8>>, _> = req
        .values
        .into_iter()
        .map(|v| serde_json::to_vec(&v).map_err(|e| SynapError::InvalidValue(e.to_string())))
        .collect();

    let length = state.list_store.lpush(&scoped_key, values?, true)?;

    Ok(Json(ListPushResponse { length, key }))
}

/// POST /list/:key/rpush - Push element(s) to right (back)
pub async fn list_rpush(
    State(state): State<AppState>,
    AuthContextExtractor(ctx): AuthContextExtractor,

    crate::hub::HubContextExtractor(hub_ctx): crate::hub::HubContextExtractor,
    Path(key): Path<String>,
    Json(req): Json<ListPushRequest>,
) -> Result<Json<ListPushResponse>, SynapError> {
    debug!("REST RPUSH key={} count={}", key, req.values.len());

    // Check permission
    require_permission(&ctx, &format!("list:{}", key), Action::Write)?;

    // Apply multi-tenant scoping if Hub mode is active

    let scoped_key =
        crate::hub::MultiTenant::scope_kv_key(hub_ctx.as_ref().map(|c| c.user_id()), &key);

    let values: Result<Vec<Vec<u8>>, _> = req
        .values
        .into_iter()
        .map(|v| serde_json::to_vec(&v).map_err(|e| SynapError::InvalidValue(e.to_string())))
        .collect();

    let length = state.list_store.rpush(&scoped_key, values?, false)?;

    Ok(Json(ListPushResponse { length, key }))
}

/// POST /list/:key/rpushx - Push element(s) to right only if key exists
pub async fn list_rpushx(
    State(state): State<AppState>,
    AuthContextExtractor(ctx): AuthContextExtractor,

    crate::hub::HubContextExtractor(hub_ctx): crate::hub::HubContextExtractor,
    Path(key): Path<String>,
    Json(req): Json<ListPushRequest>,
) -> Result<Json<ListPushResponse>, SynapError> {
    debug!("REST RPUSHX key={} count={}", key, req.values.len());

    // Check permission
    require_permission(&ctx, &format!("list:{}", key), Action::Write)?;

    // Apply multi-tenant scoping if Hub mode is active

    let scoped_key =
        crate::hub::MultiTenant::scope_kv_key(hub_ctx.as_ref().map(|c| c.user_id()), &key);

    let values: Result<Vec<Vec<u8>>, _> = req
        .values
        .into_iter()
        .map(|v| serde_json::to_vec(&v).map_err(|e| SynapError::InvalidValue(e.to_string())))
        .collect();

    let length = state.list_store.rpush(&scoped_key, values?, true)?;

    Ok(Json(ListPushResponse { length, key }))
}

/// POST /list/:key/lpop - Pop element(s) from left (front)
pub async fn list_lpop(
    State(state): State<AppState>,
    AuthContextExtractor(ctx): AuthContextExtractor,

    crate::hub::HubContextExtractor(hub_ctx): crate::hub::HubContextExtractor,
    Path(key): Path<String>,
    Json(req): Json<ListPopRequest>,
) -> Result<Json<ListPopResponse>, SynapError> {
    let count = req.count.unwrap_or(1);
    debug!("REST LPOP key={} count={}", key, count);

    // Check permission
    require_permission(&ctx, &format!("list:{}", key), Action::Write)?;

    // Apply multi-tenant scoping if Hub mode is active

    let scoped_key =
        crate::hub::MultiTenant::scope_kv_key(hub_ctx.as_ref().map(|c| c.user_id()), &key);

    let values = state.list_store.lpop(&scoped_key, Some(count))?;

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
    AuthContextExtractor(ctx): AuthContextExtractor,

    crate::hub::HubContextExtractor(hub_ctx): crate::hub::HubContextExtractor,
    Path(key): Path<String>,
    Json(req): Json<ListPopRequest>,
) -> Result<Json<ListPopResponse>, SynapError> {
    let count = req.count.unwrap_or(1);
    debug!("REST RPOP key={} count={}", key, count);

    // Check permission
    require_permission(&ctx, &format!("list:{}", key), Action::Write)?;

    // Apply multi-tenant scoping if Hub mode is active

    let scoped_key =
        crate::hub::MultiTenant::scope_kv_key(hub_ctx.as_ref().map(|c| c.user_id()), &key);

    let values = state.list_store.rpop(&scoped_key, Some(count))?;

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
    AuthContextExtractor(ctx): AuthContextExtractor,

    crate::hub::HubContextExtractor(hub_ctx): crate::hub::HubContextExtractor,
    Path(key): Path<String>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Result<Json<ListRangeResponse>, SynapError> {
    // Check permission
    require_permission(&ctx, &format!("list:{}", key), Action::Read)?;

    // Apply multi-tenant scoping if Hub mode is active

    let scoped_key =
        crate::hub::MultiTenant::scope_kv_key(hub_ctx.as_ref().map(|c| c.user_id()), &key);

    let start: i64 = params
        .get("start")
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(0);
    let stop: i64 = params
        .get("stop")
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(-1);

    debug!("REST LRANGE key={} start={} stop={}", key, start, stop);

    let values = state.list_store.lrange(&scoped_key, start, stop)?;

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
    AuthContextExtractor(ctx): AuthContextExtractor,

    crate::hub::HubContextExtractor(hub_ctx): crate::hub::HubContextExtractor,
    Path(key): Path<String>,
) -> Result<Json<serde_json::Value>, SynapError> {
    debug!("REST LLEN key={}", key);

    // Check permission
    require_permission(&ctx, &format!("list:{}", key), Action::Read)?;

    // Apply multi-tenant scoping if Hub mode is active

    let scoped_key =
        crate::hub::MultiTenant::scope_kv_key(hub_ctx.as_ref().map(|c| c.user_id()), &key);

    let length = state.list_store.llen(&scoped_key)?;

    Ok(Json(json!({ "length": length, "key": key })))
}

/// GET /list/:key/index/:index - Get element at index
pub async fn list_index(
    State(state): State<AppState>,
    AuthContextExtractor(ctx): AuthContextExtractor,

    crate::hub::HubContextExtractor(hub_ctx): crate::hub::HubContextExtractor,
    Path((key, index)): Path<(String, i64)>,
) -> Result<Json<serde_json::Value>, SynapError> {
    debug!("REST LINDEX key={} index={}", key, index);

    // Check permission
    require_permission(&ctx, &format!("list:{}", key), Action::Read)?;

    // Apply multi-tenant scoping if Hub mode is active

    let scoped_key =
        crate::hub::MultiTenant::scope_kv_key(hub_ctx.as_ref().map(|c| c.user_id()), &key);

    let value = state.list_store.lindex(&scoped_key, index)?;

    let json_value: serde_json::Value = serde_json::from_slice(&value)
        .unwrap_or_else(|_| serde_json::Value::String(String::from_utf8_lossy(&value).to_string()));

    Ok(Json(
        json!({ "value": json_value, "key": key, "index": index }),
    ))
}

/// POST /list/:key/set - Set element at index
pub async fn list_set(
    State(state): State<AppState>,
    AuthContextExtractor(ctx): AuthContextExtractor,

    crate::hub::HubContextExtractor(hub_ctx): crate::hub::HubContextExtractor,
    Path(key): Path<String>,
    Json(req): Json<ListSetRequest>,
) -> Result<Json<serde_json::Value>, SynapError> {
    debug!("REST LSET key={} index={}", key, req.index);

    // Check permission
    require_permission(&ctx, &format!("list:{}", key), Action::Write)?;

    // Apply multi-tenant scoping if Hub mode is active

    let scoped_key =
        crate::hub::MultiTenant::scope_kv_key(hub_ctx.as_ref().map(|c| c.user_id()), &key);

    let value = serde_json::to_vec(&req.value)
        .map_err(|e| SynapError::InvalidValue(format!("Failed to serialize value: {}", e)))?;

    state.list_store.lset(&scoped_key, req.index, value)?;

    Ok(Json(
        json!({ "success": true, "key": key, "index": req.index }),
    ))
}

/// POST /list/:key/trim - Trim list to range
pub async fn list_trim(
    State(state): State<AppState>,
    AuthContextExtractor(ctx): AuthContextExtractor,

    crate::hub::HubContextExtractor(hub_ctx): crate::hub::HubContextExtractor,
    Path(key): Path<String>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Result<Json<serde_json::Value>, SynapError> {
    // Check permission
    require_permission(&ctx, &format!("list:{}", key), Action::Write)?;

    // Apply multi-tenant scoping if Hub mode is active

    let scoped_key =
        crate::hub::MultiTenant::scope_kv_key(hub_ctx.as_ref().map(|c| c.user_id()), &key);

    let start: i64 = params
        .get("start")
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(0);
    let stop: i64 = params
        .get("stop")
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(-1);

    debug!("REST LTRIM key={} start={} stop={}", key, start, stop);

    state.list_store.ltrim(&scoped_key, start, stop)?;

    Ok(Json(json!({ "success": true, "key": key })))
}

/// POST /list/:key/rem - Remove occurrences of value
pub async fn list_rem(
    State(state): State<AppState>,
    AuthContextExtractor(ctx): AuthContextExtractor,

    crate::hub::HubContextExtractor(hub_ctx): crate::hub::HubContextExtractor,
    Path(key): Path<String>,
    Json(req): Json<ListRemRequest>,
) -> Result<Json<serde_json::Value>, SynapError> {
    debug!("REST LREM key={} count={}", key, req.count);

    // Check permission
    require_permission(&ctx, &format!("list:{}", key), Action::Write)?;

    // Apply multi-tenant scoping if Hub mode is active

    let scoped_key =
        crate::hub::MultiTenant::scope_kv_key(hub_ctx.as_ref().map(|c| c.user_id()), &key);

    let value = serde_json::to_vec(&req.value)
        .map_err(|e| SynapError::InvalidValue(format!("Failed to serialize value: {}", e)))?;

    let removed = state.list_store.lrem(&scoped_key, req.count, value)?;

    Ok(Json(json!({ "removed": removed, "key": key })))
}

/// POST /list/:key/insert - Insert value before/after pivot
pub async fn list_insert(
    State(state): State<AppState>,
    AuthContextExtractor(ctx): AuthContextExtractor,

    crate::hub::HubContextExtractor(hub_ctx): crate::hub::HubContextExtractor,
    Path(key): Path<String>,
    Json(req): Json<ListInsertRequest>,
) -> Result<Json<serde_json::Value>, SynapError> {
    debug!("REST LINSERT key={} before={}", key, req.before);

    // Check permission
    require_permission(&ctx, &format!("list:{}", key), Action::Write)?;

    // Apply multi-tenant scoping if Hub mode is active

    let scoped_key =
        crate::hub::MultiTenant::scope_kv_key(hub_ctx.as_ref().map(|c| c.user_id()), &key);

    let pivot = serde_json::to_vec(&req.pivot)
        .map_err(|e| SynapError::InvalidValue(format!("Failed to serialize pivot: {}", e)))?;

    let value = serde_json::to_vec(&req.value)
        .map_err(|e| SynapError::InvalidValue(format!("Failed to serialize value: {}", e)))?;

    let length = state
        .list_store
        .linsert(&scoped_key, req.before, pivot, value)?;

    Ok(Json(json!({ "length": length, "key": key })))
}

/// POST /list/:source/rpoplpush/:destination - Atomically pop from source and push to destination
pub async fn list_rpoplpush(
    State(state): State<AppState>,
    AuthContextExtractor(ctx): AuthContextExtractor,

    crate::hub::HubContextExtractor(hub_ctx): crate::hub::HubContextExtractor,
    Path((source, destination)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>, SynapError> {
    debug!(
        "REST RPOPLPUSH source={} destination={}",
        source, destination
    );

    // Check permissions for both keys
    require_permission(&ctx, &format!("list:{}", source), Action::Write)?;
    require_permission(&ctx, &format!("list:{}", destination), Action::Write)?;

    // Apply multi-tenant scoping if Hub mode is active

    let scoped_source =
        crate::hub::MultiTenant::scope_kv_key(hub_ctx.as_ref().map(|c| c.user_id()), &source);

    let scoped_destination =
        crate::hub::MultiTenant::scope_kv_key(hub_ctx.as_ref().map(|c| c.user_id()), &destination);

    let value = state
        .list_store
        .rpoplpush(&scoped_source, &scoped_destination)?;

    let json_value: serde_json::Value = serde_json::from_slice(&value)
        .unwrap_or_else(|_| serde_json::Value::String(String::from_utf8_lossy(&value).to_string()));

    Ok(Json(
        json!({ "value": json_value, "source": source, "destination": destination }),
    ))
}

/// GET /list/stats - Get list statistics
pub async fn list_stats(
    State(state): State<AppState>,
    AuthContextExtractor(ctx): AuthContextExtractor,
) -> Result<Json<ListStatsResponse>, SynapError> {
    // Check permission (read access to any list)
    require_permission(&ctx, "list:*", Action::Read)?;
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
