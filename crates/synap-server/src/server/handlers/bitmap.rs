use super::*;

// ==================== Bitmap Request/Response Types ====================

#[derive(Debug, Deserialize)]
pub struct BitmapSetBitRequest {
    pub offset: usize,
    pub value: u8,
}

#[derive(Debug, Serialize)]
pub struct BitmapSetBitResponse {
    pub key: String,
    pub old_value: u8,
}

#[derive(Debug, Serialize)]
pub struct BitmapGetBitResponse {
    pub key: String,
    pub offset: usize,
    pub value: u8,
}

#[derive(Debug, Deserialize)]
pub struct BitmapCountRequest {
    pub start: Option<usize>,
    pub end: Option<usize>,
}

#[derive(Debug, Serialize)]
pub struct BitmapCountResponse {
    pub key: String,
    pub count: usize,
}

#[derive(Debug, Deserialize)]
pub struct BitmapPosRequest {
    pub value: u8,
    pub start: Option<usize>,
    pub end: Option<usize>,
}

#[derive(Debug, Serialize)]
pub struct BitmapPosResponse {
    pub key: String,
    pub position: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub struct BitmapOpRequest {
    pub operation: String, // "AND", "OR", "XOR", "NOT"
    pub source_keys: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct BitmapOpResponse {
    pub destination: String,
    pub length: usize,
}

#[derive(Debug, Deserialize)]
pub struct BitmapFieldOperation {
    pub operation: String, // "GET", "SET", "INCRBY"
    pub offset: usize,
    pub width: usize,
    pub signed: Option<bool>,
    pub value: Option<i64>,       // For SET
    pub increment: Option<i64>,   // For INCRBY
    pub overflow: Option<String>, // "WRAP", "SAT", "FAIL"
}

#[derive(Debug, Serialize)]
pub struct BitmapFieldResponse {
    pub key: String,
    pub results: Vec<i64>,
}

pub async fn bitmap_setbit(
    State(state): State<AppState>,
    AuthContextExtractor(ctx): AuthContextExtractor,

    crate::hub::HubContextExtractor(hub_ctx): crate::hub::HubContextExtractor,
    Path(key): Path<String>,
    Json(req): Json<BitmapSetBitRequest>,
) -> Result<Json<BitmapSetBitResponse>, SynapError> {
    debug!(
        "REST SETBIT key={} offset={} value={}",
        key, req.offset, req.value
    );

    // Check permission
    require_permission(&ctx, &format!("bitmap:{}", key), Action::Write)?;

    // Apply multi-tenant scoping if Hub mode is active

    let scoped_key =
        crate::hub::MultiTenant::scope_kv_key(hub_ctx.as_ref().map(|c| c.user_id()), &key);

    let old_value = state
        .bitmap_store
        .setbit(&scoped_key, req.offset, req.value)?;

    Ok(Json(BitmapSetBitResponse { key, old_value }))
}

/// GET /bitmap/:key/getbit/:offset - Get bit at offset
pub async fn bitmap_getbit(
    State(state): State<AppState>,
    AuthContextExtractor(ctx): AuthContextExtractor,

    crate::hub::HubContextExtractor(hub_ctx): crate::hub::HubContextExtractor,
    Path((key, offset)): Path<(String, usize)>,
) -> Result<Json<BitmapGetBitResponse>, SynapError> {
    debug!("REST GETBIT key={} offset={}", key, offset);

    // Check permission
    require_permission(&ctx, &format!("bitmap:{}", key), Action::Read)?;

    // Apply multi-tenant scoping if Hub mode is active

    let scoped_key =
        crate::hub::MultiTenant::scope_kv_key(hub_ctx.as_ref().map(|c| c.user_id()), &key);

    let value = state.bitmap_store.getbit(&scoped_key, offset)?;

    Ok(Json(BitmapGetBitResponse { key, offset, value }))
}

/// GET /bitmap/:key/bitcount - Count set bits in bitmap
pub async fn bitmap_bitcount(
    State(state): State<AppState>,
    AuthContextExtractor(ctx): AuthContextExtractor,

    crate::hub::HubContextExtractor(hub_ctx): crate::hub::HubContextExtractor,
    Path(key): Path<String>,
    Query(params): Query<BitmapCountRequest>,
) -> Result<Json<BitmapCountResponse>, SynapError> {
    debug!(
        "REST BITCOUNT key={} start={:?} end={:?}",
        key, params.start, params.end
    );

    // Check permission
    require_permission(&ctx, &format!("bitmap:{}", key), Action::Read)?;

    // Apply multi-tenant scoping if Hub mode is active

    let scoped_key =
        crate::hub::MultiTenant::scope_kv_key(hub_ctx.as_ref().map(|c| c.user_id()), &key);

    let count = state
        .bitmap_store
        .bitcount(&scoped_key, params.start, params.end)?;

    Ok(Json(BitmapCountResponse { key, count }))
}

/// GET /bitmap/:key/bitpos - Find first bit set to value
pub async fn bitmap_bitpos(
    State(state): State<AppState>,
    AuthContextExtractor(ctx): AuthContextExtractor,

    crate::hub::HubContextExtractor(hub_ctx): crate::hub::HubContextExtractor,
    Path(key): Path<String>,
    Query(params): Query<BitmapPosRequest>,
) -> Result<Json<BitmapPosResponse>, SynapError> {
    debug!(
        "REST BITPOS key={} value={} start={:?} end={:?}",
        key, params.value, params.start, params.end
    );

    // Check permission
    require_permission(&ctx, &format!("bitmap:{}", key), Action::Read)?;

    // Apply multi-tenant scoping if Hub mode is active

    let scoped_key =
        crate::hub::MultiTenant::scope_kv_key(hub_ctx.as_ref().map(|c| c.user_id()), &key);

    let position =
        state
            .bitmap_store
            .bitpos(&scoped_key, params.value, params.start, params.end)?;

    Ok(Json(BitmapPosResponse { key, position }))
}

/// POST /bitmap/:destination/bitop - Perform bitwise operation
pub async fn bitmap_bitop(
    State(state): State<AppState>,
    AuthContextExtractor(ctx): AuthContextExtractor,

    crate::hub::HubContextExtractor(hub_ctx): crate::hub::HubContextExtractor,
    Path(destination): Path<String>,
    Json(req): Json<BitmapOpRequest>,
) -> Result<Json<BitmapOpResponse>, SynapError> {
    debug!(
        "REST BITOP destination={} operation={} sources={:?}",
        destination, req.operation, req.source_keys
    );

    // Check permissions for destination and all source keys
    require_permission(&ctx, &format!("bitmap:{}", destination), Action::Write)?;
    for source in &req.source_keys {
        require_permission(&ctx, &format!("bitmap:{}", source), Action::Read)?;
    }

    // Apply multi-tenant scoping if Hub mode is active

    let scoped_destination =
        crate::hub::MultiTenant::scope_kv_key(hub_ctx.as_ref().map(|c| c.user_id()), &destination);

    let scoped_sources: Vec<String> = req
        .source_keys
        .iter()
        .map(|key| {
            crate::hub::MultiTenant::scope_kv_key(hub_ctx.as_ref().map(|c| c.user_id()), key)
                .into_owned()
        })
        .collect();

    let operation = req.operation.parse::<crate::core::BitmapOperation>()?;
    let length = state
        .bitmap_store
        .bitop(operation, &scoped_destination, &scoped_sources)?;

    Ok(Json(BitmapOpResponse {
        destination,
        length,
    }))
}

/// POST /bitmap/:key/bitfield - Execute bitfield operations
pub async fn bitmap_bitfield(
    State(state): State<AppState>,
    AuthContextExtractor(ctx): AuthContextExtractor,

    crate::hub::HubContextExtractor(hub_ctx): crate::hub::HubContextExtractor,
    Path(key): Path<String>,
    Json(req): Json<Vec<BitmapFieldOperation>>,
) -> Result<Json<BitmapFieldResponse>, SynapError> {
    debug!("REST BITFIELD key={} operations={}", key, req.len());

    // Check permission (write for any modifying operations)
    require_permission(&ctx, &format!("bitmap:{}", key), Action::Write)?;

    // Apply multi-tenant scoping if Hub mode is active

    let scoped_key =
        crate::hub::MultiTenant::scope_kv_key(hub_ctx.as_ref().map(|c| c.user_id()), &key);

    use crate::core::{BitfieldOperation as CoreOp, BitfieldOverflow};

    let mut operations = Vec::new();

    for op in req {
        let core_op = match op.operation.to_uppercase().as_str() {
            "GET" => CoreOp::Get {
                offset: op.offset,
                width: op.width,
                signed: op.signed.unwrap_or(false),
            },
            "SET" => {
                let value = op.value.ok_or_else(|| {
                    SynapError::InvalidRequest("SET operation requires 'value' field".to_string())
                })?;
                CoreOp::Set {
                    offset: op.offset,
                    width: op.width,
                    signed: op.signed.unwrap_or(false),
                    value,
                }
            }
            "INCRBY" => {
                let increment = op.increment.ok_or_else(|| {
                    SynapError::InvalidRequest(
                        "INCRBY operation requires 'increment' field".to_string(),
                    )
                })?;
                let overflow = op
                    .overflow
                    .as_deref()
                    .map(|s| s.parse())
                    .transpose()?
                    .unwrap_or(BitfieldOverflow::Wrap);
                CoreOp::IncrBy {
                    offset: op.offset,
                    width: op.width,
                    signed: op.signed.unwrap_or(false),
                    increment,
                    overflow,
                }
            }
            _ => {
                return Err(SynapError::InvalidRequest(format!(
                    "Invalid operation: {}",
                    op.operation
                )));
            }
        };
        operations.push(core_op);
    }

    let results = state.bitmap_store.bitfield(&scoped_key, &operations)?;
    let values: Vec<i64> = results.iter().map(|r| r.value).collect();

    Ok(Json(BitmapFieldResponse {
        key,
        results: values,
    }))
}

/// GET /bitmap/stats - Retrieve bitmap statistics
pub async fn bitmap_stats(
    State(state): State<AppState>,
    AuthContextExtractor(ctx): AuthContextExtractor,
) -> Result<Json<crate::core::BitmapStats>, SynapError> {
    debug!("REST BITMAP STATS");

    // Check permission (read access to any bitmap)
    require_permission(&ctx, "bitmap:*", Action::Read)?;

    let stats = state.bitmap_store.stats();

    Ok(Json(stats))
}

// ==================== Bitmap StreamableHTTP Command Handlers ====================

pub(super) async fn handle_bitmap_setbit_cmd(
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
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'offset' field".to_string()))?
        as usize;

    let value = request
        .payload
        .get("value")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'value' field".to_string()))?
        as u8;

    let old_value = state.bitmap_store.setbit(key, offset, value)?;

    Ok(serde_json::json!({ "key": key, "offset": offset, "old_value": old_value }))
}

pub(super) async fn handle_bitmap_getbit_cmd(
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
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'offset' field".to_string()))?
        as usize;

    let value = state.bitmap_store.getbit(key, offset)?;

    Ok(serde_json::json!({ "key": key, "offset": offset, "value": value }))
}

pub(super) async fn handle_bitmap_bitcount_cmd(
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
        .and_then(|v| v.as_u64())
        .map(|v| v as usize);
    let end = request
        .payload
        .get("end")
        .and_then(|v| v.as_u64())
        .map(|v| v as usize);

    let count = state.bitmap_store.bitcount(key, start, end)?;

    Ok(serde_json::json!({ "key": key, "count": count }))
}

pub(super) async fn handle_bitmap_bitpos_cmd(
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
        .and_then(|v| v.as_u64())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'value' field".to_string()))?
        as u8;

    let start = request
        .payload
        .get("start")
        .and_then(|v| v.as_u64())
        .map(|v| v as usize);
    let end = request
        .payload
        .get("end")
        .and_then(|v| v.as_u64())
        .map(|v| v as usize);

    let position = state.bitmap_store.bitpos(key, value, start, end)?;

    Ok(serde_json::json!({ "key": key, "position": position }))
}

pub(super) async fn handle_bitmap_bitop_cmd(
    state: &AppState,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let destination = request
        .payload
        .get("destination")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'destination' field".to_string()))?;

    let operation_str = request
        .payload
        .get("operation")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'operation' field".to_string()))?;

    let source_keys = request
        .payload
        .get("source_keys")
        .and_then(|v| v.as_array())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'source_keys' array".to_string()))?
        .iter()
        .map(|v| {
            v.as_str()
                .map(|s| s.to_string())
                .ok_or_else(|| SynapError::InvalidValue("Source keys must be strings".to_string()))
        })
        .collect::<Result<Vec<_>, _>>()?;

    let operation = operation_str.parse::<crate::core::BitmapOperation>()?;
    let length = state
        .bitmap_store
        .bitop(operation, destination, &source_keys)?;

    Ok(serde_json::json!({ "destination": destination, "length": length }))
}

pub(super) async fn handle_bitmap_bitfield_cmd(
    state: &AppState,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let key = request
        .payload
        .get("key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'key' field".to_string()))?;

    let operations_json = request
        .payload
        .get("operations")
        .and_then(|v| v.as_array())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'operations' array".to_string()))?;

    use crate::core::{BitfieldOperation as CoreOp, BitfieldOverflow};

    let mut operations = Vec::new();

    for op_json in operations_json {
        let op_type = op_json
            .get("operation")
            .and_then(|v| v.as_str())
            .ok_or_else(|| SynapError::InvalidRequest("Missing 'operation' field".to_string()))?;

        let offset = op_json
            .get("offset")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| SynapError::InvalidRequest("Missing 'offset' field".to_string()))?
            as usize;

        let width = op_json
            .get("width")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| SynapError::InvalidRequest("Missing 'width' field".to_string()))?
            as usize;

        let signed = op_json
            .get("signed")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let core_op = match op_type.to_uppercase().as_str() {
            "GET" => CoreOp::Get {
                offset,
                width,
                signed,
            },
            "SET" => {
                let value = op_json
                    .get("value")
                    .and_then(|v| v.as_i64())
                    .ok_or_else(|| {
                        SynapError::InvalidRequest(
                            "SET operation requires 'value' field".to_string(),
                        )
                    })?;
                CoreOp::Set {
                    offset,
                    width,
                    signed,
                    value,
                }
            }
            "INCRBY" => {
                let increment = op_json
                    .get("increment")
                    .and_then(|v| v.as_i64())
                    .ok_or_else(|| {
                        SynapError::InvalidRequest(
                            "INCRBY operation requires 'increment' field".to_string(),
                        )
                    })?;
                let overflow_str = op_json
                    .get("overflow")
                    .and_then(|v| v.as_str())
                    .unwrap_or("WRAP");
                let overflow = overflow_str.parse().unwrap_or(BitfieldOverflow::Wrap);
                CoreOp::IncrBy {
                    offset,
                    width,
                    signed,
                    increment,
                    overflow,
                }
            }
            _ => {
                return Err(SynapError::InvalidRequest(format!(
                    "Invalid operation: {}",
                    op_type
                )));
            }
        };
        operations.push(core_op);
    }

    let results = state.bitmap_store.bitfield(key, &operations)?;
    let values: Vec<i64> = results.iter().map(|r| r.value).collect();

    Ok(serde_json::json!({ "key": key, "results": values }))
}

pub(super) async fn handle_bitmap_stats_cmd(
    state: &AppState,
    _request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let stats = state.bitmap_store.stats();

    Ok(serde_json::json!({
        "total_bitmaps": stats.total_bitmaps,
        "total_bits": stats.total_bits,
        "setbit_count": stats.setbit_count,
        "getbit_count": stats.getbit_count,
        "bitcount_count": stats.bitcount_count,
        "bitop_count": stats.bitop_count,
        "bitpos_count": stats.bitpos_count,
        "bitfield_count": stats.bitfield_count,
    }))
}
