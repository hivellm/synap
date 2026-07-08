use super::*;

// ==================== Hash REST Endpoints ====================

// Hash request/response types
#[derive(Debug, Deserialize)]
pub struct HashSetRequest {
    pub field: String,
    pub value: serde_json::Value,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum HashMSetRequest {
    Object {
        fields: HashMap<String, serde_json::Value>,
    },
    Array(Vec<serde_json::Value>),
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
    AuthContextExtractor(ctx): AuthContextExtractor,

    crate::hub::HubContextExtractor(hub_ctx): crate::hub::HubContextExtractor,
    Path(key): Path<String>,
    Json(req): Json<HashSetRequest>,
) -> Result<Json<HashSetResponse>, SynapError> {
    debug!("REST HSET key={} field={}", key, req.field);

    // Check permission
    require_permission(&ctx, &format!("hash:{}", key), Action::Write)?;

    // Apply multi-tenant scoping if Hub mode is active

    let scoped_key =
        crate::hub::MultiTenant::scope_kv_key(hub_ctx.as_ref().map(|c| c.user_id()), &key);

    let value = serde_json::to_vec(&req.value)
        .map_err(|e| SynapError::InvalidValue(format!("Failed to serialize value: {}", e)))?;

    let created = state.hash_store.hset(&scoped_key, &req.field, value)?;

    Ok(Json(HashSetResponse {
        created,
        key,
        field: req.field,
    }))
}

/// GET /hash/:key/:field - Get a field from hash
pub async fn hash_get(
    State(state): State<AppState>,
    AuthContextExtractor(ctx): AuthContextExtractor,

    crate::hub::HubContextExtractor(hub_ctx): crate::hub::HubContextExtractor,
    Path((key, field)): Path<(String, String)>,
) -> Result<Json<HashGetResponse>, SynapError> {
    debug!("REST HGET key={} field={}", key, field);

    // Check permission
    require_permission(&ctx, &format!("hash:{}", key), Action::Read)?;

    // Apply multi-tenant scoping if Hub mode is active

    let scoped_key =
        crate::hub::MultiTenant::scope_kv_key(hub_ctx.as_ref().map(|c| c.user_id()), &key);

    match state.hash_store.hget(&scoped_key, &field)? {
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
    AuthContextExtractor(ctx): AuthContextExtractor,

    crate::hub::HubContextExtractor(hub_ctx): crate::hub::HubContextExtractor,
    Path(key): Path<String>,
) -> Result<Json<HashMap<String, serde_json::Value>>, SynapError> {
    debug!("REST HGETALL key={}", key);

    // Check permission
    require_permission(&ctx, &format!("hash:{}", key), Action::Read)?;

    // Apply multi-tenant scoping if Hub mode is active

    let scoped_key =
        crate::hub::MultiTenant::scope_kv_key(hub_ctx.as_ref().map(|c| c.user_id()), &key);

    let all = state.hash_store.hgetall(&scoped_key)?;

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
    AuthContextExtractor(ctx): AuthContextExtractor,

    crate::hub::HubContextExtractor(hub_ctx): crate::hub::HubContextExtractor,
    Path(key): Path<String>,
) -> Result<Json<Vec<String>>, SynapError> {
    debug!("REST HKEYS key={}", key);

    // Check permission
    require_permission(&ctx, &format!("hash:{}", key), Action::Read)?;

    // Apply multi-tenant scoping if Hub mode is active

    let scoped_key =
        crate::hub::MultiTenant::scope_kv_key(hub_ctx.as_ref().map(|c| c.user_id()), &key);

    let keys = state.hash_store.hkeys(&scoped_key)?;
    Ok(Json(keys))
}

/// GET /hash/:key/vals - Get all values from hash
pub async fn hash_vals(
    State(state): State<AppState>,
    AuthContextExtractor(ctx): AuthContextExtractor,

    crate::hub::HubContextExtractor(hub_ctx): crate::hub::HubContextExtractor,
    Path(key): Path<String>,
) -> Result<Json<Vec<serde_json::Value>>, SynapError> {
    debug!("REST HVALS key={}", key);

    // Check permission
    require_permission(&ctx, &format!("hash:{}", key), Action::Read)?;

    // Apply multi-tenant scoping if Hub mode is active

    let scoped_key =
        crate::hub::MultiTenant::scope_kv_key(hub_ctx.as_ref().map(|c| c.user_id()), &key);

    let values = state.hash_store.hvals(&scoped_key)?;
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
    AuthContextExtractor(ctx): AuthContextExtractor,

    crate::hub::HubContextExtractor(hub_ctx): crate::hub::HubContextExtractor,
    Path(key): Path<String>,
) -> Result<Json<serde_json::Value>, SynapError> {
    debug!("REST HLEN key={}", key);

    // Check permission
    require_permission(&ctx, &format!("hash:{}", key), Action::Read)?;

    // Apply multi-tenant scoping if Hub mode is active

    let scoped_key =
        crate::hub::MultiTenant::scope_kv_key(hub_ctx.as_ref().map(|c| c.user_id()), &key);

    let len = state.hash_store.hlen(&scoped_key)?;
    Ok(Json(json!({ "length": len })))
}

/// POST /hash/:key/mset - Set multiple fields
pub async fn hash_mset(
    State(state): State<AppState>,
    AuthContextExtractor(ctx): AuthContextExtractor,

    crate::hub::HubContextExtractor(hub_ctx): crate::hub::HubContextExtractor,
    Path(key): Path<String>,
    Json(req): Json<HashMSetRequest>,
) -> Result<Json<serde_json::Value>, SynapError> {
    // Check permission
    require_permission(&ctx, &format!("hash:{}", key), Action::Write)?;

    // Apply multi-tenant scoping if Hub mode is active

    let scoped_key =
        crate::hub::MultiTenant::scope_kv_key(hub_ctx.as_ref().map(|c| c.user_id()), &key);

    let fields_map: HashMap<String, Vec<u8>> = match req {
        HashMSetRequest::Object { fields } => {
            debug!(
                "REST HMSET key={} fields={} (object format)",
                key,
                fields.len()
            );
            fields
                .into_iter()
                .map(|(k, v)| {
                    let bytes = serde_json::to_vec(&v).map_err(|e| {
                        SynapError::InvalidValue(format!("Failed to serialize field {}: {}", k, e))
                    })?;
                    Ok((k, bytes))
                })
                .collect::<Result<HashMap<_, _>, SynapError>>()?
        }
        HashMSetRequest::Array(arr) => {
            debug!("REST HMSET key={} fields={} (array format)", key, arr.len());
            let mut fields_map = HashMap::new();
            for item in arr {
                let obj = item.as_object().ok_or_else(|| {
                    SynapError::InvalidRequest(
                        "Array items must be objects with 'field' and 'value'".to_string(),
                    )
                })?;
                let field_name = obj.get("field").and_then(|v| v.as_str()).ok_or_else(|| {
                    SynapError::InvalidRequest("Missing 'field' in array item".to_string())
                })?;
                let value = obj.get("value").ok_or_else(|| {
                    SynapError::InvalidRequest("Missing 'value' in array item".to_string())
                })?;
                let bytes = serde_json::to_vec(value).map_err(|e| {
                    SynapError::InvalidValue(format!(
                        "Failed to serialize field {}: {}",
                        field_name, e
                    ))
                })?;
                fields_map.insert(field_name.to_string(), bytes);
            }
            fields_map
        }
    };

    state.hash_store.hmset(&scoped_key, fields_map)?;

    Ok(Json(json!({ "success": true, "key": key })))
}

/// POST /hash/:key/mget - Get multiple fields
pub async fn hash_mget(
    State(state): State<AppState>,
    AuthContextExtractor(ctx): AuthContextExtractor,

    crate::hub::HubContextExtractor(hub_ctx): crate::hub::HubContextExtractor,
    Path(key): Path<String>,
    Json(req): Json<HashMGetRequest>,
) -> Result<Json<Vec<Option<serde_json::Value>>>, SynapError> {
    debug!("REST HMGET key={} fields={:?}", key, req.fields);

    // Check permission
    require_permission(&ctx, &format!("hash:{}", key), Action::Read)?;

    // Apply multi-tenant scoping if Hub mode is active

    let scoped_key =
        crate::hub::MultiTenant::scope_kv_key(hub_ctx.as_ref().map(|c| c.user_id()), &key);

    let values = state.hash_store.hmget(&scoped_key, &req.fields)?;

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
    AuthContextExtractor(ctx): AuthContextExtractor,

    crate::hub::HubContextExtractor(hub_ctx): crate::hub::HubContextExtractor,
    Path(key): Path<String>,
    Json(req): Json<HashDelRequest>,
) -> Result<Json<HashDelResponse>, SynapError> {
    debug!("REST HDEL key={} fields={:?}", key, req.fields);

    // Check permission
    require_permission(&ctx, &format!("hash:{}", key), Action::Delete)?;

    // Apply multi-tenant scoping if Hub mode is active

    let scoped_key =
        crate::hub::MultiTenant::scope_kv_key(hub_ctx.as_ref().map(|c| c.user_id()), &key);

    let deleted = state.hash_store.hdel(&scoped_key, &req.fields)?;

    Ok(Json(HashDelResponse { deleted, key }))
}

/// GET /hash/:key/:field/exists - Check if field exists
pub async fn hash_exists(
    State(state): State<AppState>,
    AuthContextExtractor(ctx): AuthContextExtractor,

    crate::hub::HubContextExtractor(hub_ctx): crate::hub::HubContextExtractor,
    Path((key, field)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>, SynapError> {
    debug!("REST HEXISTS key={} field={}", key, field);

    // Check permission
    require_permission(&ctx, &format!("hash:{}", key), Action::Read)?;

    // Apply multi-tenant scoping if Hub mode is active

    let scoped_key =
        crate::hub::MultiTenant::scope_kv_key(hub_ctx.as_ref().map(|c| c.user_id()), &key);

    let exists = state.hash_store.hexists(&scoped_key, &field)?;

    Ok(Json(json!({ "exists": exists })))
}

/// POST /hash/:key/incrby - Increment field by integer
pub async fn hash_incrby(
    State(state): State<AppState>,
    AuthContextExtractor(ctx): AuthContextExtractor,

    crate::hub::HubContextExtractor(hub_ctx): crate::hub::HubContextExtractor,
    Path(key): Path<String>,
    Json(req): Json<HashIncrByRequest>,
) -> Result<Json<serde_json::Value>, SynapError> {
    debug!(
        "REST HINCRBY key={} field={} increment={}",
        key, req.field, req.increment
    );

    // Check permission
    require_permission(&ctx, &format!("hash:{}", key), Action::Write)?;

    // Apply multi-tenant scoping if Hub mode is active

    let scoped_key =
        crate::hub::MultiTenant::scope_kv_key(hub_ctx.as_ref().map(|c| c.user_id()), &key);

    let new_value = state
        .hash_store
        .hincrby(&scoped_key, &req.field, req.increment)?;

    Ok(Json(json!({ "value": new_value })))
}

/// POST /hash/:key/incrbyfloat - Increment field by float
pub async fn hash_incrbyfloat(
    State(state): State<AppState>,
    AuthContextExtractor(ctx): AuthContextExtractor,

    crate::hub::HubContextExtractor(hub_ctx): crate::hub::HubContextExtractor,
    Path(key): Path<String>,
    Json(req): Json<HashIncrByFloatRequest>,
) -> Result<Json<serde_json::Value>, SynapError> {
    debug!(
        "REST HINCRBYFLOAT key={} field={} increment={}",
        key, req.field, req.increment
    );

    // Check permission
    require_permission(&ctx, &format!("hash:{}", key), Action::Write)?;

    // Apply multi-tenant scoping if Hub mode is active

    let scoped_key =
        crate::hub::MultiTenant::scope_kv_key(hub_ctx.as_ref().map(|c| c.user_id()), &key);

    let new_value = state
        .hash_store
        .hincrbyfloat(&scoped_key, &req.field, req.increment)?;

    Ok(Json(json!({ "value": new_value })))
}

/// POST /hash/:key/setnx - Set field only if it doesn't exist
pub async fn hash_setnx(
    State(state): State<AppState>,
    AuthContextExtractor(ctx): AuthContextExtractor,

    crate::hub::HubContextExtractor(hub_ctx): crate::hub::HubContextExtractor,
    Path(key): Path<String>,
    Json(req): Json<HashSetRequest>,
) -> Result<Json<serde_json::Value>, SynapError> {
    debug!("REST HSETNX key={} field={}", key, req.field);

    // Check permission
    require_permission(&ctx, &format!("hash:{}", key), Action::Write)?;

    // Apply multi-tenant scoping if Hub mode is active

    let scoped_key =
        crate::hub::MultiTenant::scope_kv_key(hub_ctx.as_ref().map(|c| c.user_id()), &key);

    let value = serde_json::to_vec(&req.value)
        .map_err(|e| SynapError::InvalidValue(format!("Failed to serialize value: {}", e)))?;

    let created = state.hash_store.hsetnx(&scoped_key, &req.field, value)?;

    Ok(Json(
        json!({ "created": created, "key": key, "field": req.field }),
    ))
}

/// GET /hash/stats - Get hash statistics
pub async fn hash_stats(
    State(state): State<AppState>,
    AuthContextExtractor(ctx): AuthContextExtractor,
) -> Result<Json<HashStatsResponse>, SynapError> {
    debug!("REST HASH STATS");

    // Check permission (read access to any hash)
    require_permission(&ctx, "hash:*", Action::Read)?;

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

pub(super) async fn handle_hash_set_cmd(
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

    // Check if there's an active transaction for this client_id
    let client_id = request
        .payload
        .get("client_id")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    if !client_id.is_empty() {
        let was_queued = state.transaction_manager.queue_command_if_transaction(
            client_id,
            crate::core::transaction::TransactionCommand::HashSet {
                key: key.to_string(),
                field: field.to_string(),
                value: value_bytes.clone(),
            },
        )?;

        if was_queued {
            // Command queued in transaction, return success immediately
            return Ok(serde_json::json!({ "success": true, "queued": true }));
        }
    }

    // No active transaction, execute immediately
    let created = state.hash_store.hset(key, field, value_bytes)?;

    // Update key version for WATCH (optimistic locking)
    state.transaction_manager.update_key_version(key);

    Ok(serde_json::json!({ "created": created, "key": key, "field": field }))
}

pub(super) async fn handle_hash_get_cmd(
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

pub(super) async fn handle_hash_getall_cmd(
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

pub(super) async fn handle_hash_del_cmd(
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

    // Check if there's an active transaction for this client_id
    let client_id = request
        .payload
        .get("client_id")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    if !client_id.is_empty() {
        let was_queued = state.transaction_manager.queue_command_if_transaction(
            client_id,
            crate::core::transaction::TransactionCommand::HashDel {
                key: key.to_string(),
                fields: fields.clone(),
            },
        )?;

        if was_queued {
            // Command queued in transaction, return success immediately
            return Ok(serde_json::json!({ "success": true, "queued": true }));
        }
    }

    // No active transaction, execute immediately
    let deleted = state.hash_store.hdel(key, &fields)?;

    // Update key version for WATCH (optimistic locking) if deleted
    if deleted > 0 {
        state.transaction_manager.update_key_version(key);
    }

    Ok(serde_json::json!({ "deleted": deleted }))
}

pub(super) async fn handle_hash_exists_cmd(
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

pub(super) async fn handle_hash_len_cmd(
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

pub(super) async fn handle_hash_keys_cmd(
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

pub(super) async fn handle_hash_vals_cmd(
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

pub(super) async fn handle_hash_mset_cmd(
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

pub(super) async fn handle_hash_mget_cmd(
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

pub(super) async fn handle_hash_incrby_cmd(
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

    // Check if there's an active transaction for this client_id
    let client_id = request
        .payload
        .get("client_id")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    if !client_id.is_empty() {
        let was_queued = state.transaction_manager.queue_command_if_transaction(
            client_id,
            crate::core::transaction::TransactionCommand::HashIncrBy {
                key: key.to_string(),
                field: field.to_string(),
                delta: increment,
            },
        )?;

        if was_queued {
            // Command queued in transaction, return success immediately
            return Ok(serde_json::json!({ "success": true, "queued": true }));
        }
    }

    // No active transaction, execute immediately
    let new_value = state.hash_store.hincrby(key, field, increment)?;

    // Update key version for WATCH (optimistic locking)
    state.transaction_manager.update_key_version(key);

    Ok(serde_json::json!({ "value": new_value }))
}

pub(super) async fn handle_hash_incrbyfloat_cmd(
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

pub(super) async fn handle_hash_setnx_cmd(
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

pub(super) async fn handle_hash_stats_cmd(
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
