use super::*;

// ==================== HyperLogLog Command Handlers ====================

fn parse_hyperloglog_element(value: &serde_json::Value) -> Result<Vec<u8>, SynapError> {
    if let Some(s) = value.as_str() {
        return Ok(s.as_bytes().to_vec());
    }

    if let Some(array) = value.as_array() {
        let mut bytes = Vec::with_capacity(array.len());
        for item in array {
            let byte = item.as_u64().ok_or_else(|| {
                SynapError::InvalidValue(
                    "HyperLogLog element arrays must contain integers".to_string(),
                )
            })?;

            if byte > u8::MAX as u64 {
                return Err(SynapError::InvalidValue(
                    "HyperLogLog element byte out of range (0-255)".to_string(),
                ));
            }

            bytes.push(byte as u8);
        }

        return Ok(bytes);
    }

    if value.is_null() {
        return Err(SynapError::InvalidValue(
            "HyperLogLog elements cannot be null".to_string(),
        ));
    }

    serde_json::to_vec(value).map_err(|e| SynapError::SerializationError(e.to_string()))
}

pub(super) async fn handle_hyperloglog_pfadd_cmd(
    state: &AppState,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let key = request
        .payload
        .get("key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'key' field".to_string()))?;

    let elements = request
        .payload
        .get("elements")
        .and_then(|v| v.as_array())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'elements' array".to_string()))?
        .iter()
        .map(parse_hyperloglog_element)
        .collect::<Result<Vec<_>, _>>()?;

    let ttl_secs = request.payload.get("ttl_secs").and_then(|v| v.as_u64());

    let added = state.hyperloglog_store.pfadd(key, elements, ttl_secs)?;

    Ok(serde_json::json!({ "key": key, "added": added }))
}

pub(super) async fn handle_hyperloglog_pfcount_cmd(
    state: &AppState,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let key = request
        .payload
        .get("key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'key' field".to_string()))?;

    let count = state.hyperloglog_store.pfcount(key)?;

    Ok(serde_json::json!({ "key": key, "count": count }))
}

pub(super) async fn handle_hyperloglog_pfmerge_cmd(
    state: &AppState,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let destination = request
        .payload
        .get("destination")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'destination' field".to_string()))?;

    let sources = request
        .payload
        .get("sources")
        .and_then(|v| v.as_array())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'sources' array".to_string()))?
        .iter()
        .map(|v| {
            v.as_str()
                .map(|s| s.to_string())
                .ok_or_else(|| SynapError::InvalidValue("Source keys must be strings".to_string()))
        })
        .collect::<Result<Vec<_>, _>>()?;

    if sources.is_empty() {
        return Err(SynapError::InvalidRequest(
            "HyperLogLog pfmerge requires at least one source key".to_string(),
        ));
    }

    let count = state.hyperloglog_store.pfmerge(destination, sources)?;

    Ok(serde_json::json!({ "destination": destination, "count": count }))
}

pub(super) async fn handle_hyperloglog_stats_cmd(
    state: &AppState,
    _request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let stats = state.hyperloglog_store.stats();

    Ok(serde_json::json!({
        "total_hlls": stats.total_hlls,
        "total_cardinality": stats.total_cardinality,
        "pfadd_count": stats.pfadd_count,
        "pfcount_count": stats.pfcount_count,
        "pfmerge_count": stats.pfmerge_count,
    }))
}

// ==================== HyperLogLog REST Handlers ====================

/// POST /hyperloglog/:key/pfadd - Add elements to HyperLogLog structure
pub async fn hyperloglog_pfadd(
    State(state): State<AppState>,
    AuthContextExtractor(ctx): AuthContextExtractor,

    crate::hub::HubContextExtractor(hub_ctx): crate::hub::HubContextExtractor,
    Path(key): Path<String>,
    Json(req): Json<HyperLogLogAddRequest>,
) -> Result<Json<HyperLogLogAddResponse>, SynapError> {
    debug!(
        "REST PFADD key={} count={} ttl_secs={:?}",
        key,
        req.elements.len(),
        req.ttl_secs
    );

    // Check permission
    require_permission(&ctx, &format!("hyperloglog:{}", key), Action::Write)?;

    if req.elements.is_empty() {
        return Ok(Json(HyperLogLogAddResponse { key, added: 0 }));
    }

    // Apply multi-tenant scoping if Hub mode is active

    let scoped_key =
        crate::hub::MultiTenant::scope_kv_key(hub_ctx.as_ref().map(|c| c.user_id()), &key);

    let elements: Vec<Vec<u8>> = req
        .elements
        .into_iter()
        .map(|element| element.into_bytes())
        .collect();

    let added = state
        .hyperloglog_store
        .pfadd(&scoped_key, elements, req.ttl_secs)?;

    Ok(Json(HyperLogLogAddResponse { key, added }))
}

/// GET /hyperloglog/:key/pfcount - Estimate cardinality of HyperLogLog structure
pub async fn hyperloglog_pfcount(
    State(state): State<AppState>,
    AuthContextExtractor(ctx): AuthContextExtractor,

    crate::hub::HubContextExtractor(hub_ctx): crate::hub::HubContextExtractor,
    Path(key): Path<String>,
) -> Result<Json<HyperLogLogCountResponse>, SynapError> {
    debug!("REST PFCOUNT key={}", key);

    // Check permission
    require_permission(&ctx, &format!("hyperloglog:{}", key), Action::Read)?;

    // Apply multi-tenant scoping if Hub mode is active

    let scoped_key =
        crate::hub::MultiTenant::scope_kv_key(hub_ctx.as_ref().map(|c| c.user_id()), &key);

    let count = state.hyperloglog_store.pfcount(&scoped_key)?;

    Ok(Json(HyperLogLogCountResponse { key, count }))
}

/// POST /hyperloglog/:destination/pfmerge - Merge multiple HyperLogLog structures
pub async fn hyperloglog_pfmerge(
    State(state): State<AppState>,
    AuthContextExtractor(ctx): AuthContextExtractor,

    crate::hub::HubContextExtractor(hub_ctx): crate::hub::HubContextExtractor,
    Path(destination): Path<String>,
    Json(req): Json<HyperLogLogMergeRequest>,
) -> Result<Json<HyperLogLogMergeResponse>, SynapError> {
    debug!(
        "REST PFMERGE destination={} sources={:?}",
        destination, req.sources
    );

    if req.sources.is_empty() {
        return Err(SynapError::InvalidRequest(
            "HyperLogLog merge requires at least one source key".to_string(),
        ));
    }

    // Check permissions for destination and all source keys
    require_permission(&ctx, &format!("hyperloglog:{}", destination), Action::Write)?;
    for source in &req.sources {
        require_permission(&ctx, &format!("hyperloglog:{}", source), Action::Read)?;
    }

    // Apply multi-tenant scoping if Hub mode is active

    let scoped_destination =
        crate::hub::MultiTenant::scope_kv_key(hub_ctx.as_ref().map(|c| c.user_id()), &destination);

    let scoped_sources: Vec<String> = req
        .sources
        .iter()
        .map(|key| {
            crate::hub::MultiTenant::scope_kv_key(hub_ctx.as_ref().map(|c| c.user_id()), key)
                .into_owned()
        })
        .collect();

    let count = state
        .hyperloglog_store
        .pfmerge(&scoped_destination, scoped_sources)?;

    Ok(Json(HyperLogLogMergeResponse { destination, count }))
}

/// GET /hyperloglog/stats - Retrieve HyperLogLog statistics
pub async fn hyperloglog_stats(
    State(state): State<AppState>,
    AuthContextExtractor(ctx): AuthContextExtractor,
) -> Result<Json<crate::core::HyperLogLogStats>, SynapError> {
    debug!("REST HYPERLOGLOG STATS");

    // Check permission (read access to any hyperloglog)
    require_permission(&ctx, "hyperloglog:*", Action::Read)?;

    let stats = state.hyperloglog_store.stats();

    Ok(Json(stats))
}
