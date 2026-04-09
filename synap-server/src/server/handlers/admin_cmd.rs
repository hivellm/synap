use super::*;

// ============================================================================
// Monitoring StreamableHTTP Command Handlers
// ============================================================================

pub(super) async fn handle_info_cmd(
    state: AppState,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let section = request
        .payload
        .get("section")
        .and_then(|v| v.as_str())
        .unwrap_or("all");
    let section = InfoSection::from_str(section);

    let mut response = serde_json::json!({});

    if section == InfoSection::All || section == InfoSection::Server {
        let server_info = ServerInfo::collect(state.monitoring.uptime_secs(), 15500).await;
        response["server"] = serde_json::to_value(server_info)
            .map_err(|e| SynapError::SerializationError(e.to_string()))?;
    }

    if section == InfoSection::All || section == InfoSection::Memory {
        let stores = state.monitoring.stores();
        let memory_info =
            MemoryInfo::collect(stores.0, stores.1, stores.2, stores.3, stores.4).await;
        response["memory"] = serde_json::to_value(memory_info)
            .map_err(|e| SynapError::SerializationError(e.to_string()))?;
    }

    if section == InfoSection::All || section == InfoSection::Stats {
        let stores = state.monitoring.stores();
        let stats_info = StatsInfo::collect(stores.0, stores.1, stores.2, stores.3, stores.4).await;
        response["stats"] = serde_json::to_value(stats_info)
            .map_err(|e| SynapError::SerializationError(e.to_string()))?;
    }

    if section == InfoSection::All || section == InfoSection::Replication {
        let repl_info = ReplicationInfo::collect().await;
        response["replication"] = serde_json::to_value(repl_info)
            .map_err(|e| SynapError::SerializationError(e.to_string()))?;
    }

    if section == InfoSection::All || section == InfoSection::Keyspace {
        let stores = state.monitoring.stores();
        let keyspace_info =
            KeyspaceInfo::collect(stores.0, stores.1, stores.2, stores.3, stores.4).await;
        response["keyspace"] = serde_json::to_value(keyspace_info)
            .map_err(|e| SynapError::SerializationError(e.to_string()))?;
    }

    Ok(response)
}

pub(super) async fn handle_slowlog_get_cmd(
    state: AppState,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let count = request
        .payload
        .get("count")
        .and_then(|v| v.as_u64())
        .map(|v| v as usize);

    let entries = state.monitoring.slow_log().get(count).await;
    let total = state.monitoring.slow_log().len().await;

    Ok(serde_json::json!({
        "entries": entries,
        "total": total
    }))
}

pub(super) async fn handle_slowlog_reset_cmd(
    state: AppState,
    _request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let count = state.monitoring.slow_log().reset().await;

    Ok(serde_json::json!({
        "success": true,
        "cleared": count
    }))
}

pub(super) async fn handle_memory_usage_cmd(
    state: AppState,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let key = request
        .payload
        .get("key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'key' field".to_string()))?;

    let key_manager = KeyManager::new(
        state.kv_store.clone(),
        state.hash_store.clone(),
        state.list_store.clone(),
        state.set_store.clone(),
        state.sorted_set_store.clone(),
    );

    let stores = state.monitoring.stores();
    let key_type = key_manager.key_type(key).await?;

    // Return error if key doesn't exist
    if key_type == crate::core::KeyType::None {
        return Err(SynapError::KeyNotFound(format!(
            "Key '{}' does not exist",
            key
        )));
    }

    let usage = MemoryUsage::calculate_with_stores(
        key_type, key, &stores.0, &stores.1, &stores.2, &stores.3, &stores.4,
    )
    .await
    .unwrap_or_else(|| MemoryUsage {
        key: key.to_string(),
        bytes: 0,
        human: "0B".to_string(),
    });

    serde_json::to_value(usage).map_err(|e| SynapError::SerializationError(e.to_string()))
}

pub(super) async fn handle_client_list_cmd(
    state: AppState,
    _request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let clients = state.client_list_manager.list().await;
    let count = clients.len();

    let clients_json: Vec<serde_json::Value> = clients
        .into_iter()
        .map(|c| {
            serde_json::json!({
                "id": c.id,
                "addr": c.addr,
                "age": c.age,
                "idle": c.idle,
                "flags": c.flags,
                "db": c.db,
                "sub": c.sub,
                "psub": c.psub,
                "multi": c.multi,
                "qbuf": c.qbuf,
                "qbuf_free": c.qbuf_free,
                "obl": c.obl,
                "oll": c.oll,
                "omem": c.omem,
                "events": c.events,
                "cmd": c.cmd
            })
        })
        .collect();

    Ok(serde_json::json!({
        "clients": clients_json,
        "count": count
    }))
}

// ============================================================================
// Transaction StreamableHTTP Command Handlers
// ============================================================================

pub(super) async fn handle_transaction_multi_cmd(
    state: AppState,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    // Get client_id from request payload, default to request_id
    let client_id = request
        .payload
        .get("client_id")
        .and_then(|v| v.as_str())
        .unwrap_or(&request.request_id);

    debug!("StreamableHTTP MULTI client_id={}", client_id);
    state.transaction_manager.multi(client_id.to_string())?;

    Ok(serde_json::json!({
        "success": true,
        "message": "Transaction started"
    }))
}

pub(super) async fn handle_transaction_discard_cmd(
    state: AppState,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let client_id = request
        .payload
        .get("client_id")
        .and_then(|v| v.as_str())
        .unwrap_or(&request.request_id);

    debug!("StreamableHTTP DISCARD client_id={}", client_id);
    state.transaction_manager.discard(client_id)?;

    Ok(serde_json::json!({
        "success": true,
        "message": "Transaction discarded"
    }))
}

pub(super) async fn handle_transaction_watch_cmd(
    state: AppState,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let client_id = request
        .payload
        .get("client_id")
        .and_then(|v| v.as_str())
        .unwrap_or(&request.request_id);

    let keys = request
        .payload
        .get("keys")
        .and_then(|v| v.as_array())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'keys' field".to_string()))?
        .iter()
        .filter_map(|v| v.as_str().map(|s| s.to_string()))
        .collect();

    debug!(
        "StreamableHTTP WATCH client_id={}, keys={:?}",
        client_id, keys
    );
    state.transaction_manager.watch(client_id, keys)?;

    Ok(serde_json::json!({
        "success": true,
        "message": "Keys watched"
    }))
}

pub(super) async fn handle_transaction_unwatch_cmd(
    state: AppState,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let client_id = request
        .payload
        .get("client_id")
        .and_then(|v| v.as_str())
        .unwrap_or(&request.request_id);

    debug!("StreamableHTTP UNWATCH client_id={}", client_id);
    state.transaction_manager.unwatch(client_id)?;

    Ok(serde_json::json!({
        "success": true,
        "message": "Keys unwatched"
    }))
}

pub(super) async fn handle_transaction_exec_cmd(
    state: AppState,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let client_id = request
        .payload
        .get("client_id")
        .and_then(|v| v.as_str())
        .unwrap_or(&request.request_id);

    debug!("StreamableHTTP EXEC client_id={}", client_id);
    match state.transaction_manager.exec(client_id).await? {
        Some(results) => Ok(serde_json::json!({
            "success": true,
            "results": results
        })),
        None => Ok(serde_json::json!({
            "aborted": true,
            "message": "Transaction aborted: watched keys changed"
        })),
    }
}
