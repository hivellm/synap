use super::*;

/// SET endpoint - store a key-value pair
pub async fn kv_set(
    State(state): State<AppState>,
    AuthContextExtractor(ctx): AuthContextExtractor,

    crate::hub::HubContextExtractor(hub_ctx): crate::hub::HubContextExtractor,
    Json(req): Json<SetRequest>,
) -> Result<Json<SetResponse>, SynapError> {
    debug!("REST SET key={}", req.key);

    // Check permission (S-10: zero-alloc when auth is disabled)
    require_resource_permission(&ctx, "kv:", &req.key, Action::Write)?;

    // Apply multi-tenant scoping if Hub mode is active

    let scoped_key =
        crate::hub::MultiTenant::scope_kv_key(hub_ctx.as_ref().map(|c| c.user_id()), &req.key);

    // Store strings as raw UTF-8 so round-trips return the original string,
    // not a JSON-encoded form. Non-string values are JSON-encoded as before.
    let value_bytes = if let Some(s) = req.value.as_str() {
        s.as_bytes().to_vec()
    } else {
        serde_json::to_vec(&req.value).map_err(|e| SynapError::SerializationError(e.to_string()))?
    };

    // Reject oversized values before any allocation in the store
    if let Some(max_bytes) = state.kv_store.config().max_value_size_bytes {
        if value_bytes.len() > max_bytes {
            return Err(SynapError::InvalidRequest(format!(
                "Value size {} bytes exceeds max_value_size_bytes {}",
                value_bytes.len(),
                max_bytes
            )));
        }
    }

    // WAL write-ahead: when durability mode is Sync (fsync_mode=Always),
    // log to WAL BEFORE writing to memory. If WAL fails the request fails
    // and no data is written — true write-ahead semantics.
    // Otherwise WAL is written after the memory write (Async/Periodic mode).
    let is_sync = state
        .persistence
        .as_ref()
        .map(|p| p.is_sync_durability())
        .unwrap_or(false);

    // Read the clock ONCE here (S-07) and convert relative expiries to absolute ms.
    // Expiry::to_unix_ms() inside set_with_opts will see UnixMilliseconds and skip
    // an additional clock read, giving us a single SystemTime::now() call per SET.
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    // Resolve expiry: `expiry` field takes precedence over legacy `ttl`.
    // Convert to absolute-ms form using the pre-read clock.
    let expiry = req
        .expiry
        .or_else(|| req.ttl.map(Expiry::Seconds))
        .map(|e| match e {
            Expiry::Seconds(s) => {
                Expiry::UnixMilliseconds(now_ms.saturating_add(s.saturating_mul(1_000)))
            }
            Expiry::Milliseconds(ms) => Expiry::UnixMilliseconds(now_ms.saturating_add(ms)),
            already_absolute => already_absolute,
        });

    // Clone value only when WAL persistence is active; move the original into set_with_opts.
    let wal_value = if state.persistence.is_some() {
        Some(value_bytes.clone())
    } else {
        None
    };

    // WAL write-ahead (sync mode): log BEFORE writing to memory
    if is_sync {
        if let Some(ref persistence) = state.persistence {
            let ttl_secs = expiry.and_then(|e| match e {
                Expiry::Seconds(s) => Some(s),
                Expiry::Milliseconds(ms) => Some(ms / 1_000),
                _ => None,
            });
            persistence
                .log_kv_set(
                    scoped_key.clone().into_owned(),
                    value_bytes.clone(),
                    ttl_secs,
                )
                .await
                .map_err(|e| {
                    error!("WAL write failed (sync mode), aborting SET: {}", e);
                    SynapError::InternalError(format!("WAL write failed: {e}"))
                })?;
        }
    }

    let opts = SetOptions {
        if_absent: req.nx,
        if_present: req.xx,
        keep_ttl: req.keepttl,
        return_old: req.get,
    };

    // Set in KV store — move value_bytes (no extra clone when WAL is disabled).
    let result = state
        .kv_store
        .set_with_opts(&scoped_key, value_bytes, expiry, opts)
        .await?;

    // Async WAL: log after memory write (default, Periodic/Never modes)
    // Only log when the write actually happened (NX/XX condition was met).
    if result.written && !is_sync {
        if let Some(ref persistence) = state.persistence {
            let ttl_secs = expiry.and_then(|e| match e {
                Expiry::Seconds(s) => Some(s),
                Expiry::Milliseconds(ms) => Some(ms / 1_000),
                _ => None,
            });
            // wal_value is Some when persistence is active (cloned above).
            if let Some(wv) = wal_value {
                if let Err(e) = persistence
                    .log_kv_set(scoped_key.into_owned(), wv, ttl_secs)
                    .await
                {
                    error!(
                        "Failed to log KV SET to WAL (async mode, data in memory): {}",
                        e
                    );
                }
            }
        }
    }

    // Convert old_value bytes → JSON for the response
    let old_value_json = result
        .old_value
        .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok());

    Ok(Json(SetResponse {
        success: true,
        key: req.key,
        written: result.written,
        old_value: old_value_json,
    }))
}

/// GET endpoint - retrieve a value by key
pub async fn kv_get(
    State(state): State<AppState>,
    AuthContextExtractor(ctx): AuthContextExtractor,

    crate::hub::HubContextExtractor(hub_ctx): crate::hub::HubContextExtractor,
    Path(key): Path<String>,
    axum::extract::Query(params): axum::extract::Query<HashMap<String, String>>,
) -> Result<Json<GetResponse>, SynapError> {
    let return_type = params.get("type").map(|s| s.as_str()).unwrap_or("string");
    debug!("REST GET key={}, type={}", key, return_type);

    // Check permission
    require_resource_permission(&ctx, "kv:", &key, Action::Read)?;

    // Apply multi-tenant scoping if Hub mode is active

    let scoped_key =
        crate::hub::MultiTenant::scope_kv_key(hub_ctx.as_ref().map(|c| c.user_id()), &key);

    let value_bytes = state.kv_store.get(&scoped_key).await?;

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
    AuthContextExtractor(ctx): AuthContextExtractor,

    crate::hub::HubContextExtractor(hub_ctx): crate::hub::HubContextExtractor,
    Path(key): Path<String>,
) -> Result<Json<DeleteResponse>, SynapError> {
    debug!("REST DELETE key={}", key);

    // Check permission
    require_resource_permission(&ctx, "kv:", &key, Action::Delete)?;

    // Apply multi-tenant scoping if Hub mode is active

    let scoped_key =
        crate::hub::MultiTenant::scope_kv_key(hub_ctx.as_ref().map(|c| c.user_id()), &key);

    let deleted = state.kv_store.delete(&scoped_key).await?;

    // Log to WAL if persistence is enabled
    if deleted {
        if let Some(ref persistence) = state.persistence {
            if let Err(e) = persistence.log_kv_del(vec![scoped_key.into_owned()]).await {
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
pub async fn kv_stats(
    State(state): State<AppState>,
    AuthContextExtractor(ctx): AuthContextExtractor,
) -> Result<Json<StatsResponse>, SynapError> {
    // Check permission (read access to any KV key)
    require_permission(&ctx, "kv:*", Action::Read)?;
    debug!("REST STATS");

    let stats = state.kv_store.stats().await;

    Ok(Json(StatsResponse {
        total_keys: stats.total_keys.max(0) as usize,
        total_memory_bytes: stats.total_memory_bytes.max(0) as usize,
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

// ==================== String Extension REST Endpoints ====================

// String extension request/response types
#[derive(Debug, Deserialize)]
pub struct AppendRequest {
    pub value: serde_json::Value,
}

#[derive(Debug, Serialize)]
pub struct AppendResponse {
    pub length: usize,
}

#[derive(Debug, Deserialize)]
pub struct GetRangeRequest {
    pub start: isize,
    pub end: isize,
}

#[derive(Debug, Deserialize)]
pub struct SetRangeRequest {
    pub offset: usize,
    pub value: serde_json::Value,
}

#[derive(Debug, Serialize)]
pub struct SetRangeResponse {
    pub length: usize,
}

#[derive(Debug, Serialize)]
pub struct StrlenResponse {
    pub length: usize,
}

#[derive(Debug, Deserialize)]
pub struct GetSetRequest {
    pub value: serde_json::Value,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum GetSetResponse {
    Value(String),
    Null,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum MSetNxPair {
    Tuple((String, serde_json::Value)),
    Object {
        key: String,
        value: serde_json::Value,
    },
}

#[derive(Debug, Deserialize)]
pub struct MSetNxRequest {
    pub pairs: Vec<MSetNxPair>,
}

#[derive(Debug, Serialize)]
pub struct MSetNxResponse {
    pub success: bool,
}

/// APPEND endpoint - append bytes to existing value or create new key
pub async fn kv_append(
    State(state): State<AppState>,
    AuthContextExtractor(ctx): AuthContextExtractor,

    crate::hub::HubContextExtractor(hub_ctx): crate::hub::HubContextExtractor,
    Path(key): Path<String>,
    Json(req): Json<AppendRequest>,
) -> Result<Json<AppendResponse>, SynapError> {
    debug!("REST APPEND key={}", key);

    // Check permission
    require_resource_permission(&ctx, "kv:", &key, Action::Write)?;

    // Apply multi-tenant scoping if Hub mode is active

    let scoped_key =
        crate::hub::MultiTenant::scope_kv_key(hub_ctx.as_ref().map(|c| c.user_id()), &key);

    let value_bytes = serde_json::to_vec(&req.value)
        .map_err(|e| SynapError::SerializationError(e.to_string()))?;

    let length = state.kv_store.append(&scoped_key, value_bytes).await?;

    // Log to WAL (async, non-blocking)
    if let Some(ref persistence) = state.persistence {
        // APPEND is logged as SET since we reconstruct full value
        if let Err(e) = persistence
            .log_kv_set(scoped_key.into_owned(), vec![], None)
            .await
        {
            error!("Failed to log KV APPEND to WAL: {}", e);
        }
    }

    Ok(Json(AppendResponse { length }))
}

/// GETRANGE endpoint - get substring by range with negative indices
pub async fn kv_getrange(
    State(state): State<AppState>,
    AuthContextExtractor(ctx): AuthContextExtractor,

    crate::hub::HubContextExtractor(hub_ctx): crate::hub::HubContextExtractor,
    Path(key): Path<String>,
    axum::extract::Query(params): axum::extract::Query<HashMap<String, String>>,
) -> Result<Json<GetResponse>, SynapError> {
    let start = params
        .get("start")
        .and_then(|s| s.parse::<isize>().ok())
        .ok_or_else(|| SynapError::InvalidRequest("start parameter required".to_string()))?;
    let end = params
        .get("end")
        .and_then(|s| s.parse::<isize>().ok())
        .ok_or_else(|| SynapError::InvalidRequest("end parameter required".to_string()))?;

    debug!("REST GETRANGE key={}, start={}, end={}", key, start, end);

    // Check permission
    require_resource_permission(&ctx, "kv:", &key, Action::Read)?;

    // Apply multi-tenant scoping if Hub mode is active

    let scoped_key =
        crate::hub::MultiTenant::scope_kv_key(hub_ctx.as_ref().map(|c| c.user_id()), &key);

    let range_bytes = state.kv_store.getrange(&scoped_key, start, end).await?;

    if range_bytes.is_empty() {
        Ok(Json(GetResponse::NotFound(
            serde_json::json!({"error": "Key not found or range empty"}),
        )))
    } else {
        let value_str = String::from_utf8(range_bytes.clone())
            .unwrap_or_else(|_| format!("<binary data: {} bytes>", range_bytes.len()));
        Ok(Json(GetResponse::String(value_str)))
    }
}

/// SETRANGE endpoint - overwrite substring at offset
pub async fn kv_setrange(
    State(state): State<AppState>,
    AuthContextExtractor(ctx): AuthContextExtractor,

    crate::hub::HubContextExtractor(hub_ctx): crate::hub::HubContextExtractor,
    Path(key): Path<String>,
    Json(req): Json<SetRangeRequest>,
) -> Result<Json<SetRangeResponse>, SynapError> {
    debug!("REST SETRANGE key={}, offset={}", key, req.offset);

    // Check permission
    require_resource_permission(&ctx, "kv:", &key, Action::Write)?;

    // Apply multi-tenant scoping if Hub mode is active

    let scoped_key =
        crate::hub::MultiTenant::scope_kv_key(hub_ctx.as_ref().map(|c| c.user_id()), &key);

    let value_bytes = serde_json::to_vec(&req.value)
        .map_err(|e| SynapError::SerializationError(e.to_string()))?;

    let length = state
        .kv_store
        .setrange(&scoped_key, req.offset, value_bytes)
        .await?;

    // Log to WAL (async, non-blocking)
    if let Some(ref persistence) = state.persistence {
        // SETRANGE is logged as SET since we reconstruct full value
        if let Err(e) = persistence
            .log_kv_set(scoped_key.into_owned(), vec![], None)
            .await
        {
            error!("Failed to log KV SETRANGE to WAL: {}", e);
        }
    }

    Ok(Json(SetRangeResponse { length }))
}

/// STRLEN endpoint - get length of string value in bytes
pub async fn kv_strlen(
    State(state): State<AppState>,
    AuthContextExtractor(ctx): AuthContextExtractor,

    crate::hub::HubContextExtractor(hub_ctx): crate::hub::HubContextExtractor,
    Path(key): Path<String>,
) -> Result<Json<StrlenResponse>, SynapError> {
    debug!("REST STRLEN key={}", key);

    // Check permission
    require_resource_permission(&ctx, "kv:", &key, Action::Read)?;

    // Apply multi-tenant scoping if Hub mode is active

    let scoped_key =
        crate::hub::MultiTenant::scope_kv_key(hub_ctx.as_ref().map(|c| c.user_id()), &key);

    let length = state.kv_store.strlen(&scoped_key).await?;

    Ok(Json(StrlenResponse { length }))
}

/// GETSET endpoint - atomically get current value and set new one
pub async fn kv_getset(
    State(state): State<AppState>,
    AuthContextExtractor(ctx): AuthContextExtractor,

    crate::hub::HubContextExtractor(hub_ctx): crate::hub::HubContextExtractor,
    Path(key): Path<String>,
    Json(req): Json<GetSetRequest>,
) -> Result<Json<GetSetResponse>, SynapError> {
    debug!("REST GETSET key={}", key);

    // Check permission
    require_resource_permission(&ctx, "kv:", &key, Action::Write)?;

    // Apply multi-tenant scoping if Hub mode is active

    let scoped_key =
        crate::hub::MultiTenant::scope_kv_key(hub_ctx.as_ref().map(|c| c.user_id()), &key);

    let value_bytes = serde_json::to_vec(&req.value)
        .map_err(|e| SynapError::SerializationError(e.to_string()))?;

    let old_value = state
        .kv_store
        .getset(&scoped_key, value_bytes.clone())
        .await?;

    // Log to WAL (async, non-blocking)
    if let Some(ref persistence) = state.persistence {
        if let Err(e) = persistence
            .log_kv_set(scoped_key.into_owned(), value_bytes, None)
            .await
        {
            error!("Failed to log KV GETSET to WAL: {}", e);
        }
    }

    if let Some(old_bytes) = old_value {
        let old_str = String::from_utf8(old_bytes.clone())
            .unwrap_or_else(|_| format!("<binary data: {} bytes>", old_bytes.len()));
        Ok(Json(GetSetResponse::Value(old_str)))
    } else {
        Ok(Json(GetSetResponse::Null))
    }
}

/// MSETNX endpoint - multi-set only if ALL keys don't exist (atomic)
pub async fn kv_msetnx(
    State(state): State<AppState>,
    AuthContextExtractor(ctx): AuthContextExtractor,

    crate::hub::HubContextExtractor(hub_ctx): crate::hub::HubContextExtractor,
    Json(req): Json<MSetNxRequest>,
) -> Result<Json<MSetNxResponse>, SynapError> {
    debug!("REST MSETNX count={}", req.pairs.len());

    let pairs: Vec<(String, Vec<u8>)> = req
        .pairs
        .into_iter()
        .map(|pair| {
            let (key, value) = match pair {
                MSetNxPair::Tuple((k, v)) => (k, v),
                MSetNxPair::Object { key: k, value: v } => (k, v),
            };
            // Check permission
            require_resource_permission(&ctx, "kv:", &key, Action::Write)?;
            let value_bytes = serde_json::to_vec(&value)
                .map_err(|e| SynapError::SerializationError(e.to_string()))?;
            Ok((key, value_bytes))
        })
        .collect::<Result<Vec<_>, SynapError>>()?;

    // Apply multi-tenant scoping if Hub mode is active

    let scoped_pairs: Vec<(String, Vec<u8>)> = pairs
        .iter()
        .map(|(key, value)| {
            let scoped_key =
                crate::hub::MultiTenant::scope_kv_key(hub_ctx.as_ref().map(|c| c.user_id()), key);
            (scoped_key.into_owned(), value.clone())
        })
        .collect();

    let success = state.kv_store.msetnx(scoped_pairs.clone()).await?;

    // Log to WAL if all keys were set
    if success {
        if let Some(ref persistence) = state.persistence {
            for (key, value_bytes) in scoped_pairs {
                if let Err(e) = persistence.log_kv_set(key, value_bytes, None).await {
                    error!("Failed to log KV MSETNX to WAL: {}", e);
                }
            }
        }
    }

    Ok(Json(MSetNxResponse { success }))
}

// ==================== Key Management REST Endpoints ====================

/// Helper to create KeyManager from AppState
pub(crate) fn create_key_manager(state: &AppState) -> KeyManager {
    KeyManager::new(
        state.kv_store.clone(),
        state.hash_store.clone(),
        state.list_store.clone(),
        state.set_store.clone(),
        state.sorted_set_store.clone(),
    )
}

#[derive(Debug, Serialize)]
pub struct TypeResponse {
    pub key: String,
    pub r#type: String,
}

#[derive(Debug, Deserialize)]
pub struct RenameRequest {
    pub destination: String,
}

#[derive(Debug, Serialize)]
pub struct RenameResponse {
    pub success: bool,
    pub source: String,
    pub destination: String,
}

#[derive(Debug, Deserialize)]
pub struct CopyRequest {
    pub destination: String,
    pub replace: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct CopyResponse {
    pub success: bool,
    pub source: String,
    pub destination: String,
}

#[derive(Debug, Serialize)]
pub struct RandomKeyResponse {
    pub key: Option<String>,
}

/// TYPE endpoint - get the type of a key
pub async fn key_type(
    State(state): State<AppState>,
    AuthContextExtractor(ctx): AuthContextExtractor,

    crate::hub::HubContextExtractor(hub_ctx): crate::hub::HubContextExtractor,
    Path(key): Path<String>,
) -> Result<Json<TypeResponse>, SynapError> {
    debug!("REST TYPE key={}", key);

    // Check permission
    require_resource_permission(&ctx, "kv:", &key, Action::Read)?;

    // Apply multi-tenant scoping if Hub mode is active

    let scoped_key =
        crate::hub::MultiTenant::scope_kv_key(hub_ctx.as_ref().map(|c| c.user_id()), &key);

    let manager = create_key_manager(&state);
    let key_type = manager.key_type(&scoped_key).await?;

    Ok(Json(TypeResponse {
        key,
        r#type: key_type.as_str().to_string(),
    }))
}

/// EXISTS endpoint - check if key exists (cross-store)
pub async fn key_exists(
    State(state): State<AppState>,
    AuthContextExtractor(ctx): AuthContextExtractor,

    crate::hub::HubContextExtractor(hub_ctx): crate::hub::HubContextExtractor,
    Path(key): Path<String>,
) -> Result<Json<serde_json::Value>, SynapError> {
    debug!("REST EXISTS key={}", key);

    // Check permission
    require_resource_permission(&ctx, "kv:", &key, Action::Read)?;

    // Apply multi-tenant scoping if Hub mode is active

    let scoped_key =
        crate::hub::MultiTenant::scope_kv_key(hub_ctx.as_ref().map(|c| c.user_id()), &key);

    let manager = create_key_manager(&state);
    let exists = manager.exists(&scoped_key).await?;

    Ok(Json(serde_json::json!({
        "key": key,
        "exists": exists
    })))
}

/// RENAME endpoint - rename a key atomically
pub async fn key_rename(
    State(state): State<AppState>,
    AuthContextExtractor(ctx): AuthContextExtractor,

    crate::hub::HubContextExtractor(hub_ctx): crate::hub::HubContextExtractor,
    Path(source): Path<String>,
    Json(req): Json<RenameRequest>,
) -> Result<Json<RenameResponse>, SynapError> {
    debug!(
        "REST RENAME source={}, destination={}",
        source, req.destination
    );

    // Check permissions for both source and destination
    require_resource_permission(&ctx, "kv:", &source, Action::Delete)?;
    require_resource_permission(&ctx, "kv:", &req.destination, Action::Write)?;

    // Apply multi-tenant scoping if Hub mode is active

    let (scoped_source, scoped_dest) = {
        let user_id = hub_ctx.as_ref().map(|c| c.user_id());
        (
            crate::hub::MultiTenant::scope_kv_key(user_id, &source),
            crate::hub::MultiTenant::scope_kv_key(user_id, &req.destination),
        )
    };

    let manager = create_key_manager(&state);
    manager.rename(&scoped_source, &scoped_dest).await?;

    // Log to WAL if persistence is enabled
    if let Some(ref persistence) = state.persistence {
        if let Err(e) = persistence
            .log_kv_rename(scoped_source.into_owned(), scoped_dest.into_owned())
            .await
        {
            error!("Failed to log RENAME to WAL: {}", e);
            // Don't fail the request, just log the error
        }
    }

    Ok(Json(RenameResponse {
        success: true,
        source,
        destination: req.destination,
    }))
}

/// RENAMENX endpoint - rename key only if destination doesn't exist
pub async fn key_renamenx(
    State(state): State<AppState>,
    AuthContextExtractor(ctx): AuthContextExtractor,

    crate::hub::HubContextExtractor(hub_ctx): crate::hub::HubContextExtractor,
    Path(source): Path<String>,
    Json(req): Json<RenameRequest>,
) -> Result<Json<RenameResponse>, SynapError> {
    debug!(
        "REST RENAMENX source={}, destination={}",
        source, req.destination
    );

    // Check permissions for both source and destination
    require_resource_permission(&ctx, "kv:", &source, Action::Delete)?;
    require_resource_permission(&ctx, "kv:", &req.destination, Action::Write)?;

    // Apply multi-tenant scoping if Hub mode is active

    let (scoped_source, scoped_dest) = {
        let user_id = hub_ctx.as_ref().map(|c| c.user_id());
        (
            crate::hub::MultiTenant::scope_kv_key(user_id, &source),
            crate::hub::MultiTenant::scope_kv_key(user_id, &req.destination),
        )
    };

    let manager = create_key_manager(&state);
    let success = manager.renamenx(&scoped_source, &scoped_dest).await?;

    if !success {
        return Err(SynapError::InvalidRequest(
            "Destination key already exists".to_string(),
        ));
    }

    Ok(Json(RenameResponse {
        success: true,
        source,
        destination: req.destination,
    }))
}

/// COPY endpoint - copy key to destination
pub async fn key_copy(
    State(state): State<AppState>,
    AuthContextExtractor(ctx): AuthContextExtractor,

    crate::hub::HubContextExtractor(hub_ctx): crate::hub::HubContextExtractor,
    Path(source): Path<String>,
    Json(req): Json<CopyRequest>,
) -> Result<Json<CopyResponse>, SynapError> {
    debug!(
        "REST COPY source={}, destination={}, replace={:?}",
        source, req.destination, req.replace
    );

    // Check permissions for both source and destination
    require_resource_permission(&ctx, "kv:", &source, Action::Read)?;
    require_resource_permission(&ctx, "kv:", &req.destination, Action::Write)?;

    // Apply multi-tenant scoping if Hub mode is active

    let (scoped_source, scoped_dest) = {
        let user_id = hub_ctx.as_ref().map(|c| c.user_id());
        (
            crate::hub::MultiTenant::scope_kv_key(user_id, &source),
            crate::hub::MultiTenant::scope_kv_key(user_id, &req.destination),
        )
    };

    let manager = create_key_manager(&state);
    let replace = req.replace.unwrap_or(false);
    let success = manager.copy(&scoped_source, &scoped_dest, replace).await?;

    if !success {
        // Destination exists and replace=false - return 409 Conflict instead of error
        return Ok(Json(CopyResponse {
            success: false,
            source,
            destination: req.destination,
        }));
    }

    Ok(Json(CopyResponse {
        success: true,
        source,
        destination: req.destination,
    }))
}

/// RANDOMKEY endpoint - get a random key
pub async fn key_randomkey(
    State(state): State<AppState>,
    AuthContextExtractor(ctx): AuthContextExtractor,

    crate::hub::HubContextExtractor(hub_ctx): crate::hub::HubContextExtractor,
) -> Result<Json<RandomKeyResponse>, SynapError> {
    debug!("REST RANDOMKEY");

    // Check permission
    require_permission(&ctx, "kv:*", Action::Read)?;

    let manager = create_key_manager(&state);

    // In Hub mode, try to find a random key owned by the user

    let random_key = {
        if let Some(ref hub_ctx) = hub_ctx {
            // Try up to 10 times to find a key owned by this user
            let mut found_key: Option<String> = None;
            for _ in 0..10 {
                if let Some(key) = manager.randomkey().await? {
                    if crate::hub::MultiTenant::check_ownership(&key, hub_ctx.user_id()) {
                        found_key = Some(key);
                        break;
                    }
                } else {
                    // No keys at all
                    break;
                }
            }
            found_key
        } else {
            manager.randomkey().await?
        }
    };

    // Unscope the key before returning

    let unscoped_key = random_key.and_then(|k| {
        crate::hub::MultiTenant::parse_scoped_name(&k)
            .map(|(_, name)| name)
            .or(Some(k))
    });

    Ok(Json(RandomKeyResponse { key: unscoped_key }))
}

// ==================== Monitoring REST Endpoints ====================

/// INFO endpoint - get server information
pub async fn info(
    State(state): State<AppState>,
    AuthContextExtractor(ctx): AuthContextExtractor,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<serde_json::Value>, SynapError> {
    // Check admin permission for server info
    require_permission(&ctx, "admin:*", Action::Admin)?;
    let section = params.get("section").map(|s| s.as_str()).unwrap_or("all");
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

    Ok(Json(response))
}

/// SLOWLOG endpoint - get slow query log
pub async fn slowlog(
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<serde_json::Value>, SynapError> {
    let count = params.get("count").and_then(|s| s.parse::<usize>().ok());

    let entries = state.monitoring.slow_log().get(count).await;
    let total = state.monitoring.slow_log().len().await;

    Ok(Json(serde_json::json!({
        "entries": entries,
        "total": total
    })))
}

/// MEMORY USAGE endpoint - get memory usage for a key
pub async fn memory_usage(
    State(state): State<AppState>,
    Path(key): Path<String>,
) -> Result<Json<serde_json::Value>, SynapError> {
    let key_manager = KeyManager::new(
        state.kv_store.clone(),
        state.hash_store.clone(),
        state.list_store.clone(),
        state.set_store.clone(),
        state.sorted_set_store.clone(),
    );

    let stores = state.monitoring.stores();
    let key_type = key_manager.key_type(&key).await?;

    // Return 0 usage if key doesn't exist (REST endpoint behavior)
    if key_type == crate::core::KeyType::None {
        return Ok(Json(serde_json::json!({
            "key": key,
            "bytes": 0,
            "human": "0B"
        })));
    }

    let usage = MemoryUsage::calculate_with_stores(
        key_type, &key, &stores.0, &stores.1, &stores.2, &stores.3, &stores.4,
    )
    .await
    .unwrap_or_else(|| MemoryUsage {
        key: key.clone(),
        bytes: 0,
        human: "0B".to_string(),
    });

    Ok(Json(serde_json::to_value(usage).map_err(|e| {
        SynapError::SerializationError(e.to_string())
    })?))
}

/// CLIENT LIST endpoint - get active connections
pub async fn client_list(
    State(state): State<AppState>,

    crate::hub::HubContextExtractor(hub_ctx): crate::hub::HubContextExtractor,
) -> Result<Json<serde_json::Value>, SynapError> {
    // Block in Hub mode - would expose all users' connections

    crate::hub::require_standalone_mode(&hub_ctx)?;

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

    Ok(Json(serde_json::json!({
        "clients": clients_json,
        "count": count
    })))
}

// ==================== Transaction REST Endpoints ====================

#[derive(Debug, Deserialize)]
pub struct WatchRequest {
    pub keys: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct MultiResponse {
    pub success: bool,
    pub message: String,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum ExecResponse {
    Success { results: Vec<serde_json::Value> },
    Aborted { aborted: bool },
}

/// MULTI endpoint - start a transaction
pub async fn transaction_multi(
    State(state): State<AppState>,
) -> Result<Json<MultiResponse>, SynapError> {
    // For REST API, we'll use a client_id based on request context
    // In production, this should come from authentication/session
    let client_id = "rest_client".to_string();

    debug!("REST MULTI client_id={}", client_id);
    state.transaction_manager.multi(client_id)?;

    Ok(Json(MultiResponse {
        success: true,
        message: "Transaction started".to_string(),
    }))
}

/// DISCARD endpoint - discard current transaction
pub async fn transaction_discard(
    State(state): State<AppState>,
) -> Result<Json<MultiResponse>, SynapError> {
    let client_id = "rest_client";

    debug!("REST DISCARD client_id={}", client_id);
    state.transaction_manager.discard(client_id)?;

    Ok(Json(MultiResponse {
        success: true,
        message: "Transaction discarded".to_string(),
    }))
}

/// WATCH endpoint - watch keys for changes
pub async fn transaction_watch(
    State(state): State<AppState>,
    Json(req): Json<WatchRequest>,
) -> Result<Json<MultiResponse>, SynapError> {
    let client_id = "rest_client";

    if req.keys.is_empty() {
        return Err(SynapError::InvalidRequest(
            "Keys list cannot be empty".to_string(),
        ));
    }

    debug!("REST WATCH client_id={}, keys={:?}", client_id, req.keys);
    state.transaction_manager.watch(client_id, req.keys)?;

    Ok(Json(MultiResponse {
        success: true,
        message: "Keys watched".to_string(),
    }))
}

/// UNWATCH endpoint - unwatch all keys
pub async fn transaction_unwatch(
    State(state): State<AppState>,
) -> Result<Json<MultiResponse>, SynapError> {
    let client_id = "rest_client";

    debug!("REST UNWATCH client_id={}", client_id);
    state.transaction_manager.unwatch(client_id)?;

    Ok(Json(MultiResponse {
        success: true,
        message: "Keys unwatched".to_string(),
    }))
}

/// EXEC endpoint - execute transaction
pub async fn transaction_exec(
    State(state): State<AppState>,
) -> Result<Json<ExecResponse>, SynapError> {
    let client_id = "rest_client";

    debug!("REST EXEC client_id={}", client_id);
    match state.transaction_manager.exec(client_id).await? {
        Some(results) => Ok(Json(ExecResponse::Success { results })),
        None => Ok(Json(ExecResponse::Aborted { aborted: true })),
    }
}
