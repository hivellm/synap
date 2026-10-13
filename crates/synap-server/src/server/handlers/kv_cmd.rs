use super::*;
use crate::persistence::durable;

pub(super) async fn handle_kv_set_cmd(
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

    // Store strings as raw UTF-8 so round-trips return the original string,
    // not a JSON-encoded form. Non-string values are JSON-encoded as before.
    let value_bytes = if let Some(s) = value.as_str() {
        s.as_bytes().to_vec()
    } else {
        serde_json::to_vec(value).map_err(|e| SynapError::SerializationError(e.to_string()))?
    };

    // Check if there's an active transaction for this client_id
    let client_id = request
        .payload
        .get("client_id")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    if !client_id.is_empty() {
        let was_queued = state.transaction_manager.queue_command_if_transaction(
            client_id,
            crate::core::transaction::TransactionCommand::KVSet {
                key: key.to_string(),
                value: value_bytes.clone(),
                ttl,
            },
        )?;

        if was_queued {
            // Command queued in transaction, return success immediately
            return Ok(serde_json::json!({ "success": true, "queued": true }));
        }
    }

    // No active transaction, execute immediately
    durable::kv_set(
        state.persistence.as_deref(),
        &state.kv_store,
        key,
        value_bytes,
        ttl,
    )
    .await?;

    // Update key version for WATCH (optimistic locking)
    state.transaction_manager.update_key_version(key);

    Ok(serde_json::json!({ "success": true }))
}

pub(super) async fn handle_kv_get_cmd(
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

pub(super) async fn handle_kv_del_cmd(
    state: &AppState,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let key = request
        .payload
        .get("key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'key' field".to_string()))?;

    // Check if there's an active transaction for this client_id
    let client_id = request
        .payload
        .get("client_id")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    if !client_id.is_empty() {
        let was_queued = state.transaction_manager.queue_command_if_transaction(
            client_id,
            crate::core::transaction::TransactionCommand::KVDel {
                keys: vec![key.to_string()],
            },
        )?;

        if was_queued {
            // Command queued in transaction, return success immediately
            return Ok(serde_json::json!({ "success": true, "queued": true }));
        }
    }

    // No active transaction, execute immediately
    let deleted = durable::kv_delete(state.persistence.as_deref(), &state.kv_store, key).await?;

    // Update key version for WATCH (optimistic locking) if deleted
    if deleted {
        state.transaction_manager.update_key_version(key);
    }

    Ok(serde_json::json!({ "deleted": deleted }))
}

pub(super) async fn handle_kv_exists_cmd(
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

pub(super) async fn handle_kv_incr_cmd(
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

    // Check if there's an active transaction for this client_id
    let client_id = request
        .payload
        .get("client_id")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    if !client_id.is_empty() {
        let was_queued = state.transaction_manager.queue_command_if_transaction(
            client_id,
            crate::core::transaction::TransactionCommand::KVIncr {
                key: key.to_string(),
                delta: amount,
            },
        )?;

        if was_queued {
            // Command queued in transaction, return success immediately
            return Ok(serde_json::json!({ "success": true, "queued": true }));
        }
    }

    // No active transaction, execute immediately
    let value =
        durable::kv_incr(state.persistence.as_deref(), &state.kv_store, key, amount).await?;

    // Update key version for WATCH (optimistic locking)
    state.transaction_manager.update_key_version(key);

    Ok(serde_json::json!({ "value": value }))
}

pub(super) async fn handle_kv_decr_cmd(
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

    // Check if there's an active transaction for this client_id
    let client_id = request
        .payload
        .get("client_id")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    if !client_id.is_empty() {
        let was_queued = state.transaction_manager.queue_command_if_transaction(
            client_id,
            crate::core::transaction::TransactionCommand::KVIncr {
                key: key.to_string(),
                delta: -amount, // DECR is INCR with negative delta
            },
        )?;

        if was_queued {
            // Command queued in transaction, return success immediately
            return Ok(serde_json::json!({ "success": true, "queued": true }));
        }
    }

    // No active transaction, execute immediately
    let value =
        durable::kv_decr(state.persistence.as_deref(), &state.kv_store, key, amount).await?;

    // Update key version for WATCH (optimistic locking)
    state.transaction_manager.update_key_version(key);

    Ok(serde_json::json!({ "value": value }))
}

pub(super) async fn handle_kv_mset_cmd(
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

    durable::kv_mset(state.persistence.as_deref(), &state.kv_store, pairs).await?;

    Ok(serde_json::json!({ "success": true }))
}

pub(super) async fn handle_kv_mget_cmd(
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

pub(super) async fn handle_kv_mdel_cmd(
    state: &AppState,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let keys_val = request
        .payload
        .get("keys")
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'keys' field".to_string()))?;

    let keys: Vec<String> = serde_json::from_value(keys_val.clone())
        .map_err(|e| SynapError::InvalidRequest(format!("Invalid keys array: {}", e)))?;

    let count = durable::kv_mdel(state.persistence.as_deref(), &state.kv_store, &keys).await?;

    Ok(serde_json::json!({ "deleted": count }))
}

pub(super) async fn handle_kv_scan_cmd(
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

pub(super) async fn handle_kv_stats_cmd(
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

// ==================== String Extension Command Handlers ====================

pub(super) async fn handle_kv_append_cmd(
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

    // Same encoding as `kv.set`: strings as raw UTF-8, everything else as
    // JSON, so a partial write extends the bytes `kv.set` stored.
    let value_bytes = super::kv::encode_value_bytes(value)?;

    // Recorded as the whole resulting value and its expiry (D-08).
    let length = durable::kv_append(
        state.persistence.as_deref(),
        &state.kv_store,
        key,
        value_bytes,
    )
    .await?;

    Ok(serde_json::json!({ "length": length }))
}

pub(super) async fn handle_kv_getrange_cmd(
    store: Arc<KVStore>,
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
        .map(|v| v as isize)
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'start' field".to_string()))?;

    let end = request
        .payload
        .get("end")
        .and_then(|v| v.as_i64())
        .map(|v| v as isize)
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'end' field".to_string()))?;

    let range_bytes = store.getrange(key, start, end).await?;

    if range_bytes.is_empty() {
        Ok(serde_json::json!(null))
    } else {
        let value_str = String::from_utf8(range_bytes.clone())
            .unwrap_or_else(|_| format!("<binary data: {} bytes>", range_bytes.len()));
        Ok(serde_json::json!(value_str))
    }
}

pub(super) async fn handle_kv_setrange_cmd(
    state: &AppState,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let key = request
        .payload
        .get("key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'key' field".to_string()))?;

    let offset = request
        .payload
        .get("offset")
        .and_then(|v| v.as_u64())
        .map(|v| v as usize)
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'offset' field".to_string()))?;

    let value = request
        .payload
        .get("value")
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'value' field".to_string()))?;

    // Same encoding as `kv.set`: strings as raw UTF-8, everything else as
    // JSON, so a partial write extends the bytes `kv.set` stored.
    let value_bytes = super::kv::encode_value_bytes(value)?;

    // Recorded as the whole resulting value and its expiry (D-08).
    let length = durable::kv_setrange(
        state.persistence.as_deref(),
        &state.kv_store,
        key,
        offset,
        value_bytes,
    )
    .await?;

    Ok(serde_json::json!({ "length": length }))
}

pub(super) async fn handle_kv_strlen_cmd(
    store: Arc<KVStore>,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let key = request
        .payload
        .get("key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'key' field".to_string()))?;

    let length = store.strlen(key).await?;
    Ok(serde_json::json!({ "length": length }))
}

pub(super) async fn handle_kv_getset_cmd(
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

    // Same encoding as `kv.set`: strings as raw UTF-8, everything else as
    // JSON, so a partial write extends the bytes `kv.set` stored.
    let value_bytes = super::kv::encode_value_bytes(value)?;

    let old_value = durable::kv_getset(
        state.persistence.as_deref(),
        &state.kv_store,
        key,
        value_bytes,
    )
    .await?;

    if let Some(old_bytes) = old_value {
        let old_str = String::from_utf8(old_bytes.clone())
            .unwrap_or_else(|_| format!("<binary data: {} bytes>", old_bytes.len()));
        Ok(serde_json::json!(old_str))
    } else {
        Ok(serde_json::json!(null))
    }
}

pub(super) async fn handle_kv_msetnx_cmd(
    state: &AppState,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let pairs = request
        .payload
        .get("pairs")
        .and_then(|v| v.as_array())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'pairs' array".to_string()))?;

    let kv_pairs: Result<Vec<(String, Vec<u8>)>, SynapError> = pairs
        .iter()
        .map(|pair| {
            let pair_obj = pair
                .as_object()
                .ok_or_else(|| SynapError::InvalidRequest("Pair must be an object".to_string()))?;
            let key = pair_obj
                .get("key")
                .and_then(|v| v.as_str())
                .ok_or_else(|| SynapError::InvalidRequest("Missing 'key' in pair".to_string()))?;
            let value = pair_obj
                .get("value")
                .ok_or_else(|| SynapError::InvalidRequest("Missing 'value' in pair".to_string()))?;
            let value_bytes = serde_json::to_vec(value)
                .map_err(|e| SynapError::SerializationError(e.to_string()))?;
            Ok((key.to_string(), value_bytes))
        })
        .collect();

    let kv_pairs = kv_pairs?;
    let success =
        durable::kv_msetnx(state.persistence.as_deref(), &state.kv_store, kv_pairs).await?;

    Ok(serde_json::json!({ "success": success }))
}

// ==================== Key Management StreamableHTTP Handlers ====================

pub(super) async fn handle_key_type_cmd(
    state: AppState,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let key = request
        .payload
        .get("key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'key' field".to_string()))?;

    let manager = create_key_manager(&state);
    let key_type = manager.key_type(key).await?;

    Ok(serde_json::json!({
        "key": key,
        "type": key_type.as_str()
    }))
}

pub(super) async fn handle_key_exists_cmd(
    state: AppState,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let key = request
        .payload
        .get("key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'key' field".to_string()))?;

    let manager = create_key_manager(&state);
    let exists = manager.exists(key).await?;

    Ok(serde_json::json!({
        "key": key,
        "exists": exists
    }))
}

pub(super) async fn handle_key_rename_cmd(
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

    let manager = create_key_manager(state);
    durable::key_rename(state.persistence.as_deref(), &manager, source, destination).await?;

    Ok(serde_json::json!({
        "success": true,
        "source": source,
        "destination": destination
    }))
}

pub(super) async fn handle_key_renamenx_cmd(
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

    let manager = create_key_manager(state);
    let success =
        durable::key_renamenx(state.persistence.as_deref(), &manager, source, destination).await?;

    if !success {
        return Err(SynapError::InvalidRequest(
            "Destination key already exists".to_string(),
        ));
    }

    Ok(serde_json::json!({
        "success": true,
        "source": source,
        "destination": destination
    }))
}

pub(super) async fn handle_key_copy_cmd(
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

    let replace = request
        .payload
        .get("replace")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let manager = create_key_manager(state);
    let success = durable::key_copy(
        state.persistence.as_deref(),
        &manager,
        source,
        destination,
        replace,
    )
    .await?;

    if !success {
        return Err(SynapError::InvalidRequest(
            "Destination key already exists and replace=false".to_string(),
        ));
    }

    Ok(serde_json::json!({
        "success": true,
        "source": source,
        "destination": destination
    }))
}

pub(super) async fn handle_key_randomkey_cmd(
    state: AppState,
    _request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let manager = create_key_manager(&state);
    let random_key = manager.randomkey().await?;

    Ok(serde_json::json!({
        "key": random_key
    }))
}

pub(super) async fn handle_kv_keys_cmd(
    store: Arc<KVStore>,
    _request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let keys = store.keys().await?;
    Ok(serde_json::json!({ "keys": keys, "count": keys.len() }))
}

pub(super) async fn handle_kv_dbsize_cmd(
    store: Arc<KVStore>,
    _request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let size = store.dbsize().await?;
    Ok(serde_json::json!({ "size": size }))
}

pub(super) async fn handle_kv_flushdb_cmd(
    state: &AppState,
    _request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let count = durable::kv_flushdb(state.persistence.as_deref(), &state.kv_store).await?;
    Ok(serde_json::json!({ "flushed": count }))
}

pub(super) async fn handle_kv_flushall_cmd(
    state: &AppState,
    _request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let count = durable::kv_flushall(state.persistence.as_deref(), &state.kv_store).await?;
    Ok(serde_json::json!({ "flushed": count }))
}

pub(super) async fn handle_kv_expire_cmd(
    state: &AppState,
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

    let result =
        durable::kv_expire(state.persistence.as_deref(), &state.kv_store, key, ttl).await?;
    Ok(serde_json::json!({ "result": result }))
}

pub(super) async fn handle_kv_ttl_cmd(
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

pub(super) async fn handle_kv_persist_cmd(
    state: &AppState,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let key = request
        .payload
        .get("key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'key' field".to_string()))?;

    let result = durable::kv_persist(state.persistence.as_deref(), &state.kv_store, key).await?;
    Ok(serde_json::json!({ "result": result }))
}
