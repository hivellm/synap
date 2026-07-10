use super::*;

// ==================== Set Request/Response Types ====================

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

// ==================== Set Command Handlers ====================

pub(super) async fn handle_set_add_cmd(
    state: &AppState,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let key = request
        .payload
        .get("key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'key' field".to_string()))?;

    let members: Vec<Vec<u8>> = request
        .payload
        .get("members")
        .and_then(|v| v.as_array())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'members' array".to_string()))?
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
            crate::core::transaction::TransactionCommand::SetAdd {
                key: key.to_string(),
                members: members.clone(),
            },
        )?;

        if was_queued {
            // Command queued in transaction, return success immediately
            return Ok(serde_json::json!({ "success": true, "queued": true }));
        }
    }

    // No active transaction, execute immediately
    let added = state.set_store.sadd(key, members)?;

    // Update key version for WATCH (optimistic locking)
    state.transaction_manager.update_key_version(key);

    Ok(serde_json::json!({ "added": added }))
}

pub(super) async fn handle_set_rem_cmd(
    state: &AppState,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let key = request
        .payload
        .get("key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'key' field".to_string()))?;

    let members: Vec<Vec<u8>> = request
        .payload
        .get("members")
        .and_then(|v| v.as_array())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'members' array".to_string()))?
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
            crate::core::transaction::TransactionCommand::SetRem {
                key: key.to_string(),
                members: members.clone(),
            },
        )?;

        if was_queued {
            // Command queued in transaction, return success immediately
            return Ok(serde_json::json!({ "success": true, "queued": true }));
        }
    }

    // No active transaction, execute immediately
    let removed = state.set_store.srem(key, members)?;

    // Update key version for WATCH (optimistic locking) if removed
    if removed > 0 {
        state.transaction_manager.update_key_version(key);
    }

    Ok(serde_json::json!({ "removed": removed }))
}

pub(super) async fn handle_set_ismember_cmd(
    state: &AppState,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let key = request
        .payload
        .get("key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'key' field".to_string()))?;

    let member = request
        .payload
        .get("member")
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'member' field".to_string()))?;

    let member_bytes =
        serde_json::to_vec(member).map_err(|e| SynapError::SerializationError(e.to_string()))?;

    let is_member = state.set_store.sismember(key, member_bytes)?;

    Ok(serde_json::json!({ "is_member": is_member }))
}

pub(super) async fn handle_set_members_cmd(
    state: &AppState,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let key = request
        .payload
        .get("key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'key' field".to_string()))?;

    let members = state.set_store.smembers(key)?;

    let json_members: Vec<serde_json::Value> = members
        .into_iter()
        .map(|m| {
            serde_json::from_slice(&m).unwrap_or_else(|_| {
                serde_json::Value::String(String::from_utf8_lossy(&m).to_string())
            })
        })
        .collect();

    Ok(serde_json::json!({ "members": json_members }))
}

pub(super) async fn handle_set_size_cmd(
    state: &AppState,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let key = request
        .payload
        .get("key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'key' field".to_string()))?;

    let count = state.set_store.scard(key)?;

    Ok(serde_json::json!({ "size": count }))
}

pub(super) async fn handle_set_pop_cmd(
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

    let members = state.set_store.spop(key, count)?;

    // Update key version for WATCH (optimistic locking) if members were popped
    if !members.is_empty() {
        state.transaction_manager.update_key_version(key);
    }

    let json_members: Vec<serde_json::Value> = members
        .into_iter()
        .map(|m| {
            serde_json::from_slice(&m).unwrap_or_else(|_| {
                serde_json::Value::String(String::from_utf8_lossy(&m).to_string())
            })
        })
        .collect();

    Ok(serde_json::json!({ "members": json_members }))
}

pub(super) async fn handle_set_randmember_cmd(
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

    let members = state.set_store.srandmember(key, count)?;

    let json_members: Vec<serde_json::Value> = members
        .into_iter()
        .map(|m| {
            serde_json::from_slice(&m).unwrap_or_else(|_| {
                serde_json::Value::String(String::from_utf8_lossy(&m).to_string())
            })
        })
        .collect();

    Ok(serde_json::json!({ "members": json_members }))
}

pub(super) async fn handle_set_move_cmd(
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

    let member = request
        .payload
        .get("member")
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'member' field".to_string()))?;

    let member_bytes =
        serde_json::to_vec(member).map_err(|e| SynapError::SerializationError(e.to_string()))?;

    let moved = state.set_store.smove(source, destination, member_bytes)?;

    // Update key versions for WATCH (optimistic locking) if moved
    if moved {
        state.transaction_manager.update_key_version(source);
        state.transaction_manager.update_key_version(destination);
    }

    Ok(serde_json::json!({ "moved": moved }))
}

pub(super) async fn handle_set_inter_cmd(
    state: &AppState,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let keys: Vec<String> = request
        .payload
        .get("keys")
        .and_then(|v| v.as_array())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'keys' array".to_string()))?
        .iter()
        .filter_map(|v| v.as_str().map(String::from))
        .collect();

    let members = state.set_store.sinter(&keys)?;

    let json_members: Vec<serde_json::Value> = members
        .into_iter()
        .map(|m| {
            serde_json::from_slice(&m).unwrap_or_else(|_| {
                serde_json::Value::String(String::from_utf8_lossy(&m).to_string())
            })
        })
        .collect();

    Ok(serde_json::json!({ "members": json_members }))
}

pub(super) async fn handle_set_union_cmd(
    state: &AppState,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let keys: Vec<String> = request
        .payload
        .get("keys")
        .and_then(|v| v.as_array())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'keys' array".to_string()))?
        .iter()
        .filter_map(|v| v.as_str().map(String::from))
        .collect();

    let members = state.set_store.sunion(&keys)?;

    let json_members: Vec<serde_json::Value> = members
        .into_iter()
        .map(|m| {
            serde_json::from_slice(&m).unwrap_or_else(|_| {
                serde_json::Value::String(String::from_utf8_lossy(&m).to_string())
            })
        })
        .collect();

    Ok(serde_json::json!({ "members": json_members }))
}

pub(super) async fn handle_set_diff_cmd(
    state: &AppState,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let keys: Vec<String> = request
        .payload
        .get("keys")
        .and_then(|v| v.as_array())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'keys' array".to_string()))?
        .iter()
        .filter_map(|v| v.as_str().map(String::from))
        .collect();

    let members = state.set_store.sdiff(&keys)?;

    let json_members: Vec<serde_json::Value> = members
        .into_iter()
        .map(|m| {
            serde_json::from_slice(&m).unwrap_or_else(|_| {
                serde_json::Value::String(String::from_utf8_lossy(&m).to_string())
            })
        })
        .collect();

    Ok(serde_json::json!({ "members": json_members }))
}

pub(super) async fn handle_set_stats_cmd(
    state: &AppState,
    _request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let stats = state.set_store.stats();

    Ok(serde_json::json!({
        "total_sets": stats.total_sets,
        "total_members": stats.total_members,
        "operations": {
            "sadd_count": stats.sadd_count,
            "srem_count": stats.srem_count,
            "sismember_count": stats.sismember_count,
            "smembers_count": stats.smembers_count,
            "scard_count": stats.scard_count,
            "spop_count": stats.spop_count,
            "srandmember_count": stats.srandmember_count,
            "smove_count": stats.smove_count,
            "sinter_count": stats.sinter_count,
            "sunion_count": stats.sunion_count,
            "sdiff_count": stats.sdiff_count,
        }
    }))
}

// ==================== Lua Scripting Handlers ====================

pub async fn set_add(
    State(state): State<AppState>,
    AuthContextExtractor(ctx): AuthContextExtractor,

    crate::hub::HubContextExtractor(hub_ctx): crate::hub::HubContextExtractor,
    Path(key): Path<String>,
    Json(req): Json<SetAddRequest>,
) -> Result<Json<SetAddResponse>, SynapError> {
    debug!("REST SADD key={} count={}", key, req.members.len());

    // Check permission
    require_permission(&ctx, &format!("set:{}", key), Action::Write)?;

    // Apply multi-tenant scoping if Hub mode is active

    let scoped_key =
        crate::hub::MultiTenant::scope_kv_key(hub_ctx.as_ref().map(|c| c.user_id()), &key);

    let members: Result<Vec<Vec<u8>>, _> = req
        .members
        .into_iter()
        .map(|v| serde_json::to_vec(&v).map_err(|e| SynapError::InvalidValue(e.to_string())))
        .collect();

    let added = state.set_store.sadd(&scoped_key, members?)?;

    Ok(Json(SetAddResponse { added, key }))
}

/// POST /set/:key/rem - Remove member(s) from set
pub async fn set_rem(
    State(state): State<AppState>,
    AuthContextExtractor(ctx): AuthContextExtractor,

    crate::hub::HubContextExtractor(hub_ctx): crate::hub::HubContextExtractor,
    Path(key): Path<String>,
    Json(req): Json<SetRemRequest>,
) -> Result<Json<serde_json::Value>, SynapError> {
    debug!("REST SREM key={} count={}", key, req.members.len());

    // Check permission
    require_permission(&ctx, &format!("set:{}", key), Action::Delete)?;

    // Apply multi-tenant scoping if Hub mode is active

    let scoped_key =
        crate::hub::MultiTenant::scope_kv_key(hub_ctx.as_ref().map(|c| c.user_id()), &key);

    let members: Result<Vec<Vec<u8>>, _> = req
        .members
        .into_iter()
        .map(|v| serde_json::to_vec(&v).map_err(|e| SynapError::InvalidValue(e.to_string())))
        .collect();

    let removed = state.set_store.srem(&scoped_key, members?)?;

    Ok(Json(json!({ "removed": removed, "key": key })))
}

/// POST /set/:key/ismember - Check if member exists
pub async fn set_ismember(
    State(state): State<AppState>,
    AuthContextExtractor(ctx): AuthContextExtractor,

    crate::hub::HubContextExtractor(hub_ctx): crate::hub::HubContextExtractor,
    Path(key): Path<String>,
    Json(req): Json<SetMemberRequest>,
) -> Result<Json<serde_json::Value>, SynapError> {
    debug!("REST SISMEMBER key={}", key);

    // Check permission
    require_permission(&ctx, &format!("set:{}", key), Action::Read)?;

    // Apply multi-tenant scoping if Hub mode is active

    let scoped_key =
        crate::hub::MultiTenant::scope_kv_key(hub_ctx.as_ref().map(|c| c.user_id()), &key);

    let member = serde_json::to_vec(&req.member)
        .map_err(|e| SynapError::InvalidValue(format!("Failed to serialize member: {}", e)))?;

    let is_member = state.set_store.sismember(&scoped_key, member)?;

    Ok(Json(json!({ "is_member": is_member, "key": key })))
}

/// GET /set/:key/members - Get all members
pub async fn set_members(
    State(state): State<AppState>,
    AuthContextExtractor(ctx): AuthContextExtractor,

    crate::hub::HubContextExtractor(hub_ctx): crate::hub::HubContextExtractor,
    Path(key): Path<String>,
) -> Result<Json<serde_json::Value>, SynapError> {
    debug!("REST SMEMBERS key={}", key);

    // Check permission
    require_permission(&ctx, &format!("set:{}", key), Action::Read)?;

    // Apply multi-tenant scoping if Hub mode is active

    let scoped_key =
        crate::hub::MultiTenant::scope_kv_key(hub_ctx.as_ref().map(|c| c.user_id()), &key);

    let members = state.set_store.smembers(&scoped_key)?;

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
    AuthContextExtractor(ctx): AuthContextExtractor,

    crate::hub::HubContextExtractor(hub_ctx): crate::hub::HubContextExtractor,
    Path(key): Path<String>,
) -> Result<Json<serde_json::Value>, SynapError> {
    debug!("REST SCARD key={}", key);

    // Check permission
    require_permission(&ctx, &format!("set:{}", key), Action::Read)?;

    // Apply multi-tenant scoping if Hub mode is active

    let scoped_key =
        crate::hub::MultiTenant::scope_kv_key(hub_ctx.as_ref().map(|c| c.user_id()), &key);

    let count = state.set_store.scard(&scoped_key)?;

    Ok(Json(json!({ "count": count, "key": key })))
}

/// POST /set/:key/pop - Remove and return random member(s)
pub async fn set_pop(
    State(state): State<AppState>,
    AuthContextExtractor(ctx): AuthContextExtractor,

    crate::hub::HubContextExtractor(hub_ctx): crate::hub::HubContextExtractor,
    Path(key): Path<String>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Result<Json<serde_json::Value>, SynapError> {
    let count = params.get("count").and_then(|s| s.parse::<usize>().ok());

    debug!("REST SPOP key={} count={:?}", key, count);

    // Check permission
    require_permission(&ctx, &format!("set:{}", key), Action::Write)?;

    // Apply multi-tenant scoping if Hub mode is active

    let scoped_key =
        crate::hub::MultiTenant::scope_kv_key(hub_ctx.as_ref().map(|c| c.user_id()), &key);

    let members = state.set_store.spop(&scoped_key, count)?;

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
    AuthContextExtractor(ctx): AuthContextExtractor,

    crate::hub::HubContextExtractor(hub_ctx): crate::hub::HubContextExtractor,
    Path(key): Path<String>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Result<Json<serde_json::Value>, SynapError> {
    let count = params.get("count").and_then(|s| s.parse::<usize>().ok());

    debug!("REST SRANDMEMBER key={} count={:?}", key, count);

    // Check permission
    require_permission(&ctx, &format!("set:{}", key), Action::Read)?;

    // Apply multi-tenant scoping if Hub mode is active

    let scoped_key =
        crate::hub::MultiTenant::scope_kv_key(hub_ctx.as_ref().map(|c| c.user_id()), &key);

    let members = state.set_store.srandmember(&scoped_key, count)?;

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
    AuthContextExtractor(ctx): AuthContextExtractor,

    crate::hub::HubContextExtractor(hub_ctx): crate::hub::HubContextExtractor,
    Path((source, destination)): Path<(String, String)>,
    Json(req): Json<SetMemberRequest>,
) -> Result<Json<serde_json::Value>, SynapError> {
    debug!("REST SMOVE source={} destination={}", source, destination);

    // Check permissions for both keys
    require_permission(&ctx, &format!("set:{}", source), Action::Write)?;
    require_permission(&ctx, &format!("set:{}", destination), Action::Write)?;

    // Apply multi-tenant scoping if Hub mode is active

    let scoped_source =
        crate::hub::MultiTenant::scope_kv_key(hub_ctx.as_ref().map(|c| c.user_id()), &source);

    let scoped_destination =
        crate::hub::MultiTenant::scope_kv_key(hub_ctx.as_ref().map(|c| c.user_id()), &destination);

    let member = serde_json::to_vec(&req.member)
        .map_err(|e| SynapError::InvalidValue(format!("Failed to serialize member: {}", e)))?;

    let moved = state
        .set_store
        .smove(&scoped_source, &scoped_destination, member)?;

    Ok(Json(
        json!({ "moved": moved, "source": source, "destination": destination }),
    ))
}

/// POST /set/inter - Intersection of multiple sets
pub async fn set_inter(
    State(state): State<AppState>,
    AuthContextExtractor(ctx): AuthContextExtractor,

    crate::hub::HubContextExtractor(hub_ctx): crate::hub::HubContextExtractor,
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

    // Check permissions for all keys
    for key in &keys {
        require_permission(&ctx, &format!("set:{}", key), Action::Read)?;
    }

    // Apply multi-tenant scoping if Hub mode is active

    let scoped_keys: Vec<String> = keys
        .iter()
        .map(|key| {
            crate::hub::MultiTenant::scope_kv_key(hub_ctx.as_ref().map(|c| c.user_id()), key)
                .into_owned()
        })
        .collect();

    let members = state.set_store.sinter(&scoped_keys)?;

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
    AuthContextExtractor(ctx): AuthContextExtractor,

    crate::hub::HubContextExtractor(hub_ctx): crate::hub::HubContextExtractor,
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

    // Check permissions for all keys
    for key in &keys {
        require_permission(&ctx, &format!("set:{}", key), Action::Read)?;
    }

    // Apply multi-tenant scoping if Hub mode is active

    let scoped_keys: Vec<String> = keys
        .iter()
        .map(|key| {
            crate::hub::MultiTenant::scope_kv_key(hub_ctx.as_ref().map(|c| c.user_id()), key)
                .into_owned()
        })
        .collect();

    let members = state.set_store.sunion(&scoped_keys)?;

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
    AuthContextExtractor(ctx): AuthContextExtractor,

    crate::hub::HubContextExtractor(hub_ctx): crate::hub::HubContextExtractor,
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

    // Check permissions for all keys
    for key in &keys {
        require_permission(&ctx, &format!("set:{}", key), Action::Read)?;
    }

    // Apply multi-tenant scoping if Hub mode is active

    let scoped_keys: Vec<String> = keys
        .iter()
        .map(|key| {
            crate::hub::MultiTenant::scope_kv_key(hub_ctx.as_ref().map(|c| c.user_id()), key)
                .into_owned()
        })
        .collect();

    let members = state.set_store.sdiff(&scoped_keys)?;

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
    AuthContextExtractor(ctx): AuthContextExtractor,
) -> Result<Json<SetStatsResponse>, SynapError> {
    // Check permission (read access to any set)
    require_permission(&ctx, "set:*", Action::Read)?;
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
