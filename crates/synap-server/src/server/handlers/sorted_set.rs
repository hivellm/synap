use super::*;

// ==================== Sorted Set Handlers ====================

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum ZAddRequest {
    Single {
        member: serde_json::Value,
        score: f64,
        #[serde(default)]
        nx: bool,
        #[serde(default)]
        xx: bool,
        #[serde(default)]
        gt: bool,
        #[serde(default)]
        lt: bool,
    },
    Multiple {
        members: Vec<serde_json::Value>,
        scores: Vec<f64>,
        #[serde(default)]
        nx: bool,
        #[serde(default)]
        xx: bool,
        #[serde(default)]
        gt: bool,
        #[serde(default)]
        lt: bool,
    },
}

#[derive(Debug, Deserialize)]
pub struct ZRemRequest {
    pub members: Vec<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct ZInterstoreRequest {
    pub destination: String,
    pub keys: Vec<String>,
    #[serde(default)]
    pub weights: Option<Vec<f64>>,
    #[serde(default)]
    pub aggregate: String, // "sum", "min", "max"
}

fn serialize_scored_members(members: Vec<crate::core::ScoredMember>) -> Vec<serde_json::Value> {
    members
        .into_iter()
        .map(|m| {
            let member_str = String::from_utf8(m.member.clone())
                .unwrap_or_else(|_| format!("<binary data: {} bytes>", m.member.len()));

            json!({
                "member": member_str,
                "score": m.score,
            })
        })
        .collect()
}

pub(super) async fn handle_sortedset_zadd_cmd(
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
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'member' field".to_string()))?;

    let score = request
        .payload
        .get("score")
        .and_then(|v| v.as_f64())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'score' field".to_string()))?;

    let member_bytes = member.as_bytes().to_vec();

    let opts = crate::core::ZAddOptions {
        nx: request
            .payload
            .get("nx")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        xx: request
            .payload
            .get("xx")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        gt: request
            .payload
            .get("gt")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        lt: request
            .payload
            .get("lt")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        ch: request
            .payload
            .get("ch")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        incr: request
            .payload
            .get("incr")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
    };

    let (added, changed) = state.sorted_set_store.zadd(key, member_bytes, score, &opts);

    Ok(serde_json::json!({ "added": added, "changed": changed, "key": key }))
}

pub(super) async fn handle_sortedset_zrem_cmd(
    state: &AppState,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let key = request
        .payload
        .get("key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'key' field".to_string()))?;

    let members = request
        .payload
        .get("members")
        .and_then(|v| v.as_array())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'members' array".to_string()))?;

    let member_bytes: Result<Vec<Vec<u8>>, SynapError> = members
        .iter()
        .map(|m| {
            m.as_str().map(|s| s.as_bytes().to_vec()).ok_or_else(|| {
                SynapError::InvalidRequest("All 'members' entries must be strings".to_string())
            })
        })
        .collect();
    let member_bytes = member_bytes?;

    let removed = state.sorted_set_store.zrem(key, &member_bytes);

    Ok(serde_json::json!({ "removed": removed, "key": key }))
}

pub(super) async fn handle_sortedset_zscore_cmd(
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
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'member' field".to_string()))?;

    let member_bytes = member.as_bytes();
    let score = state.sorted_set_store.zscore(key, member_bytes);

    Ok(serde_json::json!({ "score": score, "key": key, "member": member }))
}

pub(super) async fn handle_sortedset_zcard_cmd(
    state: &AppState,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let key = request
        .payload
        .get("key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'key' field".to_string()))?;

    let count = state.sorted_set_store.zcard(key);

    Ok(serde_json::json!({ "count": count, "key": key }))
}

pub(super) async fn handle_sortedset_zincrby_cmd(
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
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'member' field".to_string()))?;

    let increment = request
        .payload
        .get("increment")
        .and_then(|v| v.as_f64())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'increment' field".to_string()))?;

    let member_bytes = member.as_bytes().to_vec();

    let new_score = state.sorted_set_store.zincrby(key, member_bytes, increment);

    Ok(serde_json::json!({ "score": new_score, "key": key }))
}

pub(super) async fn handle_sortedset_zrange_cmd(
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

    let with_scores = request
        .payload
        .get("withscores")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let members = state.sorted_set_store.zrange(key, start, stop, with_scores);
    let members = serialize_scored_members(members);

    Ok(serde_json::json!({ "members": members, "key": key }))
}

pub(super) async fn handle_sortedset_zrevrange_cmd(
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

    let with_scores = request
        .payload
        .get("withscores")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let members = state
        .sorted_set_store
        .zrevrange(key, start, stop, with_scores);
    let members = serialize_scored_members(members);

    Ok(serde_json::json!({ "members": members, "key": key }))
}

pub(super) async fn handle_sortedset_zrank_cmd(
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
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'member' field".to_string()))?;

    let member_bytes = member.as_bytes();
    let rank = state.sorted_set_store.zrank(key, member_bytes);

    Ok(serde_json::json!({ "rank": rank, "key": key, "member": member }))
}

pub(super) async fn handle_sortedset_zrevrank_cmd(
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
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'member' field".to_string()))?;

    let member_bytes = member.as_bytes();
    let rank = state.sorted_set_store.zrevrank(key, member_bytes);

    Ok(serde_json::json!({ "rank": rank, "key": key, "member": member }))
}

pub(super) async fn handle_sortedset_zcount_cmd(
    state: &AppState,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let key = request
        .payload
        .get("key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'key' field".to_string()))?;

    let min = request
        .payload
        .get("min")
        .and_then(|v| v.as_f64())
        .unwrap_or(f64::NEG_INFINITY);

    let max = request
        .payload
        .get("max")
        .and_then(|v| v.as_f64())
        .unwrap_or(f64::INFINITY);

    let count = state.sorted_set_store.zcount(key, min, max);

    Ok(serde_json::json!({ "count": count, "key": key }))
}

pub(super) async fn handle_sortedset_zpopmin_cmd(
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
        .unwrap_or(1) as usize;

    let members = state.sorted_set_store.zpopmin(key, count);
    let result_count = members.len();
    let serialized = serialize_scored_members(members);

    Ok(serde_json::json!({ "members": serialized, "count": result_count, "key": key }))
}

pub(super) async fn handle_sortedset_zpopmax_cmd(
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
        .unwrap_or(1) as usize;

    let members = state.sorted_set_store.zpopmax(key, count);
    let result_count = members.len();
    let serialized = serialize_scored_members(members);

    Ok(serde_json::json!({ "members": serialized, "count": result_count, "key": key }))
}

pub(super) async fn handle_sortedset_zrangebyscore_cmd(
    state: &AppState,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let key = request
        .payload
        .get("key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'key' field".to_string()))?;

    let min = request
        .payload
        .get("min")
        .and_then(|v| v.as_f64())
        .unwrap_or(f64::NEG_INFINITY);

    let max = request
        .payload
        .get("max")
        .and_then(|v| v.as_f64())
        .unwrap_or(f64::INFINITY);

    let with_scores = request
        .payload
        .get("withscores")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let members = state
        .sorted_set_store
        .zrangebyscore(key, min, max, with_scores);
    let members = serialize_scored_members(members);

    Ok(serde_json::json!({ "members": members, "key": key }))
}

pub(super) async fn handle_sortedset_zremrangebyrank_cmd(
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

    let removed = state.sorted_set_store.zremrangebyrank(key, start, stop);

    Ok(serde_json::json!({ "removed": removed, "key": key }))
}

pub(super) async fn handle_sortedset_zremrangebyscore_cmd(
    state: &AppState,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let key = request
        .payload
        .get("key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'key' field".to_string()))?;

    let min = request
        .payload
        .get("min")
        .and_then(|v| v.as_f64())
        .unwrap_or(f64::NEG_INFINITY);

    let max = request
        .payload
        .get("max")
        .and_then(|v| v.as_f64())
        .unwrap_or(f64::INFINITY);

    let removed = state.sorted_set_store.zremrangebyscore(key, min, max);

    Ok(serde_json::json!({ "removed": removed, "key": key }))
}

pub(super) async fn handle_sortedset_zinterstore_cmd(
    state: &AppState,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let destination = request
        .payload
        .get("destination")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'destination' field".to_string()))?;

    let keys = request
        .payload
        .get("keys")
        .and_then(|v| v.as_array())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'keys' array".to_string()))?;

    let key_strs: Vec<&str> = keys.iter().filter_map(|v| v.as_str()).collect();

    let weights = request
        .payload
        .get("weights")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_f64()).collect::<Vec<f64>>());

    let aggregate_str = request
        .payload
        .get("aggregate")
        .and_then(|v| v.as_str())
        .unwrap_or("sum");

    let aggregate = match aggregate_str.to_lowercase().as_str() {
        "min" => crate::core::Aggregate::Min,
        "max" => crate::core::Aggregate::Max,
        _ => crate::core::Aggregate::Sum,
    };

    let count =
        state
            .sorted_set_store
            .zinterstore(destination, &key_strs, weights.as_deref(), aggregate);

    Ok(serde_json::json!({ "count": count, "destination": destination }))
}

pub(super) async fn handle_sortedset_zunionstore_cmd(
    state: &AppState,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let destination = request
        .payload
        .get("destination")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'destination' field".to_string()))?;

    let keys = request
        .payload
        .get("keys")
        .and_then(|v| v.as_array())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'keys' array".to_string()))?;

    let key_strs: Vec<&str> = keys.iter().filter_map(|v| v.as_str()).collect();

    let weights = request
        .payload
        .get("weights")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_f64()).collect::<Vec<f64>>());

    let aggregate_str = request
        .payload
        .get("aggregate")
        .and_then(|v| v.as_str())
        .unwrap_or("sum");

    let aggregate = match aggregate_str.to_lowercase().as_str() {
        "min" => crate::core::Aggregate::Min,
        "max" => crate::core::Aggregate::Max,
        _ => crate::core::Aggregate::Sum,
    };

    let count =
        state
            .sorted_set_store
            .zunionstore(destination, &key_strs, weights.as_deref(), aggregate);

    Ok(serde_json::json!({ "count": count, "destination": destination }))
}

pub(super) async fn handle_sortedset_zdiffstore_cmd(
    state: &AppState,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let destination = request
        .payload
        .get("destination")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'destination' field".to_string()))?;

    let keys = request
        .payload
        .get("keys")
        .and_then(|v| v.as_array())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'keys' array".to_string()))?;

    let key_strs: Vec<&str> = keys.iter().filter_map(|v| v.as_str()).collect();

    let count = state.sorted_set_store.zdiffstore(destination, &key_strs);

    Ok(serde_json::json!({ "count": count, "destination": destination }))
}

pub(super) async fn handle_sortedset_zmscore_cmd(
    state: &AppState,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let key = request
        .payload
        .get("key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'key' field".to_string()))?;

    let members = request
        .payload
        .get("members")
        .and_then(|v| v.as_array())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'members' array".to_string()))?;

    let member_bytes: Result<Vec<Vec<u8>>, SynapError> = members
        .iter()
        .map(|m| {
            m.as_str().map(|s| s.as_bytes().to_vec()).ok_or_else(|| {
                SynapError::InvalidRequest("All 'members' entries must be strings".to_string())
            })
        })
        .collect();
    let member_bytes = member_bytes?;

    let scores = state.sorted_set_store.zmscore(key, &member_bytes);

    Ok(serde_json::json!({ "scores": scores, "key": key }))
}

pub(super) async fn handle_sortedset_stats_cmd(
    state: &AppState,
    _request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let stats = state.sorted_set_store.stats();

    Ok(serde_json::json!({
        "total_keys": stats.total_keys,
        "total_members": stats.total_members,
        "avg_members_per_key": stats.avg_members_per_key,
        "memory_bytes": stats.memory_bytes,
    }))
}

pub async fn sortedset_zadd(
    State(state): State<AppState>,
    AuthContextExtractor(ctx): AuthContextExtractor,
    Path(key): Path<String>,
    Json(req): Json<ZAddRequest>,
) -> Result<Json<serde_json::Value>, SynapError> {
    // Check permission
    require_permission(&ctx, &format!("sortedset:{}", key), Action::Write)?;

    let opts = match &req {
        ZAddRequest::Single { nx, xx, gt, lt, .. } => crate::core::ZAddOptions {
            nx: *nx,
            xx: *xx,
            gt: *gt,
            lt: *lt,
            ch: false,
            incr: false,
        },
        ZAddRequest::Multiple { nx, xx, gt, lt, .. } => crate::core::ZAddOptions {
            nx: *nx,
            xx: *xx,
            gt: *gt,
            lt: *lt,
            ch: false,
            incr: false,
        },
    };

    let total_added = match req {
        ZAddRequest::Single { member, score, .. } => {
            debug!(
                "REST ZADD key={} member={:?} score={} (single)",
                key, member, score
            );
            let member_bytes = serde_json::to_vec(&member).map_err(|e| {
                SynapError::InvalidValue(format!("Failed to serialize member: {}", e))
            })?;
            let (added, _) = state
                .sorted_set_store
                .zadd(&key, member_bytes, score, &opts);
            added
        }
        ZAddRequest::Multiple {
            members, scores, ..
        } => {
            debug!("REST ZADD key={} members={} (multiple)", key, members.len());
            if members.len() != scores.len() {
                return Err(SynapError::InvalidRequest(
                    "members and scores arrays must have the same length".to_string(),
                ));
            }
            let mut total_added = 0;
            for (member, score) in members.into_iter().zip(scores) {
                let member_bytes = serde_json::to_vec(&member).map_err(|e| {
                    SynapError::InvalidValue(format!("Failed to serialize member: {}", e))
                })?;
                let (added, _) = state
                    .sorted_set_store
                    .zadd(&key, member_bytes, score, &opts);
                total_added += added;
            }
            total_added
        }
    };

    Ok(Json(json!({ "added": total_added, "key": key })))
}

/// POST /sortedset/:key/zrem - Remove members
pub async fn sortedset_zrem(
    State(state): State<AppState>,
    AuthContextExtractor(ctx): AuthContextExtractor,
    Path(key): Path<String>,
    Json(req): Json<ZRemRequest>,
) -> Result<Json<serde_json::Value>, SynapError> {
    debug!("REST ZREM key={} members={:?}", key, req.members);

    // Check permission
    require_permission(&ctx, &format!("sortedset:{}", key), Action::Delete)?;

    let members: Result<Vec<Vec<u8>>, _> = req.members.iter().map(serde_json::to_vec).collect();

    let members = members
        .map_err(|e| SynapError::InvalidValue(format!("Failed to serialize members: {}", e)))?;

    let removed = state.sorted_set_store.zrem(&key, &members);

    Ok(Json(json!({ "removed": removed, "key": key })))
}

/// GET /sortedset/:key/:member/zscore - Get score of member
pub async fn sortedset_zscore(
    State(state): State<AppState>,
    Path((key, member)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>, SynapError> {
    debug!("REST ZSCORE key={} member={}", key, member);

    let member_bytes = member.as_bytes();
    let score = state.sorted_set_store.zscore(&key, member_bytes);

    Ok(Json(
        json!({ "score": score, "key": key, "member": member }),
    ))
}

/// GET /sortedset/:key/zcard - Get cardinality
pub async fn sortedset_zcard(
    State(state): State<AppState>,
    Path(key): Path<String>,
) -> Result<Json<serde_json::Value>, SynapError> {
    debug!("REST ZCARD key={}", key);

    let count = state.sorted_set_store.zcard(&key);

    Ok(Json(json!({ "count": count, "key": key })))
}

/// POST /sortedset/:key/zincrby - Increment score
pub async fn sortedset_zincrby(
    State(state): State<AppState>,
    Path(key): Path<String>,
    Json(req): Json<ZAddRequest>,
) -> Result<Json<serde_json::Value>, SynapError> {
    let (member, increment) = match req {
        ZAddRequest::Single { member, score, .. } => (member, score),
        ZAddRequest::Multiple { .. } => {
            return Err(SynapError::InvalidRequest(
                "ZINCRBY only supports single member".to_string(),
            ));
        }
    };

    debug!(
        "REST ZINCRBY key={} member={:?} increment={}",
        key, member, increment
    );

    let member_bytes = serde_json::to_vec(&member)
        .map_err(|e| SynapError::InvalidValue(format!("Failed to serialize member: {}", e)))?;

    let new_score = state
        .sorted_set_store
        .zincrby(&key, member_bytes, increment);

    Ok(Json(json!({ "score": new_score, "key": key })))
}

/// GET /sortedset/:key/zrange - Get range by rank
pub async fn sortedset_zrange(
    State(state): State<AppState>,
    AuthContextExtractor(ctx): AuthContextExtractor,
    Path(key): Path<String>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<serde_json::Value>, SynapError> {
    // Check permission
    require_permission(&ctx, &format!("sortedset:{}", key), Action::Read)?;
    let start: i64 = params
        .get("start")
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let stop: i64 = params
        .get("stop")
        .and_then(|s| s.parse().ok())
        .unwrap_or(-1);
    let with_scores = params
        .get("withscores")
        .map(|s| s == "true")
        .unwrap_or(false);

    debug!(
        "REST ZRANGE key={} start={} stop={} withscores={}",
        key, start, stop, with_scores
    );

    let members = state
        .sorted_set_store
        .zrange(&key, start, stop, with_scores);

    Ok(Json(json!({ "members": members, "key": key })))
}

/// GET /sortedset/:key/zrevrange - Get reverse range by rank
pub async fn sortedset_zrevrange(
    State(state): State<AppState>,
    Path(key): Path<String>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<serde_json::Value>, SynapError> {
    let start: i64 = params
        .get("start")
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let stop: i64 = params
        .get("stop")
        .and_then(|s| s.parse().ok())
        .unwrap_or(-1);
    let with_scores = params
        .get("withscores")
        .map(|s| s == "true")
        .unwrap_or(false);

    debug!(
        "REST ZREVRANGE key={} start={} stop={} withscores={}",
        key, start, stop, with_scores
    );

    let members = state
        .sorted_set_store
        .zrevrange(&key, start, stop, with_scores);

    Ok(Json(json!({ "members": members, "key": key })))
}

/// GET /sortedset/:key/:member/zrank - Get rank of member
pub async fn sortedset_zrank(
    State(state): State<AppState>,
    Path((key, member)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>, SynapError> {
    debug!("REST ZRANK key={} member={}", key, member);

    let member_bytes = member.as_bytes();
    let rank = state.sorted_set_store.zrank(&key, member_bytes);

    Ok(Json(json!({ "rank": rank, "key": key, "member": member })))
}

/// POST /sortedset/zinterstore - Intersection
pub async fn sortedset_zinterstore(
    State(state): State<AppState>,
    Json(req): Json<ZInterstoreRequest>,
) -> Result<Json<serde_json::Value>, SynapError> {
    debug!(
        "REST ZINTERSTORE dest={} keys={:?}",
        req.destination, req.keys
    );

    let keys: Vec<&str> = req.keys.iter().map(|s| s.as_str()).collect();
    let weights = req.weights.as_deref();

    let aggregate = match req.aggregate.to_lowercase().as_str() {
        "min" => crate::core::Aggregate::Min,
        "max" => crate::core::Aggregate::Max,
        _ => crate::core::Aggregate::Sum,
    };

    let count = state
        .sorted_set_store
        .zinterstore(&req.destination, &keys, weights, aggregate);

    Ok(Json(
        json!({ "count": count, "destination": req.destination }),
    ))
}

/// POST /sortedset/zunionstore - Union
pub async fn sortedset_zunionstore(
    State(state): State<AppState>,
    Json(req): Json<ZInterstoreRequest>,
) -> Result<Json<serde_json::Value>, SynapError> {
    debug!(
        "REST ZUNIONSTORE dest={} keys={:?}",
        req.destination, req.keys
    );

    let keys: Vec<&str> = req.keys.iter().map(|s| s.as_str()).collect();
    let weights = req.weights.as_deref();

    let aggregate = match req.aggregate.to_lowercase().as_str() {
        "min" => crate::core::Aggregate::Min,
        "max" => crate::core::Aggregate::Max,
        _ => crate::core::Aggregate::Sum,
    };

    let count = state
        .sorted_set_store
        .zunionstore(&req.destination, &keys, weights, aggregate);

    Ok(Json(
        json!({ "count": count, "destination": req.destination }),
    ))
}

/// GET /sortedset/stats - Get sorted set statistics
pub async fn sortedset_stats(
    State(state): State<AppState>,
    AuthContextExtractor(ctx): AuthContextExtractor,
) -> Result<Json<serde_json::Value>, SynapError> {
    debug!("REST SORTEDSET STATS");

    // Check permission (read access to any sortedset)
    require_permission(&ctx, "sortedset:*", Action::Read)?;

    let stats = state.sorted_set_store.stats();

    Ok(Json(json!({
        "total_keys": stats.total_keys,
        "total_members": stats.total_members,
        "avg_members_per_key": stats.avg_members_per_key,
        "memory_bytes": stats.memory_bytes,
    })))
}

/// GET /sortedset/:key/:member/zrevrank - Get reverse rank of member
pub async fn sortedset_zrevrank(
    State(state): State<AppState>,
    Path((key, member)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>, SynapError> {
    debug!("REST ZREVRANK key={} member={}", key, member);

    let member_bytes = member.as_bytes();
    let rank = state.sorted_set_store.zrevrank(&key, member_bytes);

    Ok(Json(json!({ "rank": rank, "key": key, "member": member })))
}

/// GET /sortedset/:key/zcount - Count members in score range
pub async fn sortedset_zcount(
    State(state): State<AppState>,
    Path(key): Path<String>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<serde_json::Value>, SynapError> {
    let min: f64 = params
        .get("min")
        .and_then(|s| s.parse().ok())
        .unwrap_or(f64::NEG_INFINITY);
    let max: f64 = params
        .get("max")
        .and_then(|s| s.parse().ok())
        .unwrap_or(f64::INFINITY);

    debug!("REST ZCOUNT key={} min={} max={}", key, min, max);

    let count = state.sorted_set_store.zcount(&key, min, max);

    Ok(Json(json!({ "count": count, "key": key })))
}

/// POST /sortedset/:key/zmscore - Get multiple scores
pub async fn sortedset_zmscore(
    State(state): State<AppState>,
    Path(key): Path<String>,
    Json(req): Json<ZRemRequest>,
) -> Result<Json<serde_json::Value>, SynapError> {
    debug!("REST ZMSCORE key={} members={:?}", key, req.members);

    let members: Result<Vec<Vec<u8>>, _> = req.members.iter().map(serde_json::to_vec).collect();
    let members = members
        .map_err(|e| SynapError::InvalidValue(format!("Failed to serialize members: {}", e)))?;

    let scores = state.sorted_set_store.zmscore(&key, &members);

    Ok(Json(json!({ "scores": scores, "key": key })))
}

/// GET /sortedset/:key/zrangebyscore - Get range by score
pub async fn sortedset_zrangebyscore(
    State(state): State<AppState>,
    Path(key): Path<String>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<serde_json::Value>, SynapError> {
    let min: f64 = params
        .get("min")
        .and_then(|s| s.parse().ok())
        .unwrap_or(f64::NEG_INFINITY);
    let max: f64 = params
        .get("max")
        .and_then(|s| s.parse().ok())
        .unwrap_or(f64::INFINITY);
    let with_scores = params
        .get("withscores")
        .map(|s| s == "true")
        .unwrap_or(false);

    debug!(
        "REST ZRANGEBYSCORE key={} min={} max={} withscores={}",
        key, min, max, with_scores
    );

    let members = state
        .sorted_set_store
        .zrangebyscore(&key, min, max, with_scores);

    Ok(Json(json!({ "members": members, "key": key })))
}

/// POST /sortedset/:key/zpopmin - Pop minimum scored members
pub async fn sortedset_zpopmin(
    State(state): State<AppState>,
    Path(key): Path<String>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<serde_json::Value>, SynapError> {
    let count: usize = params
        .get("count")
        .and_then(|s| s.parse().ok())
        .unwrap_or(1);

    debug!("REST ZPOPMIN key={} count={}", key, count);

    let members = state.sorted_set_store.zpopmin(&key, count);

    Ok(Json(
        json!({ "members": members, "count": members.len(), "key": key }),
    ))
}

/// POST /sortedset/:key/zpopmax - Pop maximum scored members
pub async fn sortedset_zpopmax(
    State(state): State<AppState>,
    Path(key): Path<String>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<serde_json::Value>, SynapError> {
    let count: usize = params
        .get("count")
        .and_then(|s| s.parse().ok())
        .unwrap_or(1);

    debug!("REST ZPOPMAX key={} count={}", key, count);

    let members = state.sorted_set_store.zpopmax(&key, count);

    Ok(Json(
        json!({ "members": members, "count": members.len(), "key": key }),
    ))
}

/// POST /sortedset/:key/zremrangebyrank - Remove members by rank range
pub async fn sortedset_zremrangebyrank(
    State(state): State<AppState>,
    Path(key): Path<String>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<serde_json::Value>, SynapError> {
    let start: i64 = params
        .get("start")
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let stop: i64 = params
        .get("stop")
        .and_then(|s| s.parse().ok())
        .unwrap_or(-1);

    debug!(
        "REST ZREMRANGEBYRANK key={} start={} stop={}",
        key, start, stop
    );

    let removed = state.sorted_set_store.zremrangebyrank(&key, start, stop);

    Ok(Json(json!({ "removed": removed, "key": key })))
}

/// POST /sortedset/:key/zremrangebyscore - Remove members by score range
pub async fn sortedset_zremrangebyscore(
    State(state): State<AppState>,
    Path(key): Path<String>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<serde_json::Value>, SynapError> {
    let min: f64 = params
        .get("min")
        .and_then(|s| s.parse().ok())
        .unwrap_or(f64::NEG_INFINITY);
    let max: f64 = params
        .get("max")
        .and_then(|s| s.parse().ok())
        .unwrap_or(f64::INFINITY);

    debug!("REST ZREMRANGEBYSCORE key={} min={} max={}", key, min, max);

    let removed = state.sorted_set_store.zremrangebyscore(&key, min, max);

    Ok(Json(json!({ "removed": removed, "key": key })))
}

/// POST /sortedset/zdiffstore - Difference store
pub async fn sortedset_zdiffstore(
    State(state): State<AppState>,
    Json(req): Json<ZInterstoreRequest>,
) -> Result<Json<serde_json::Value>, SynapError> {
    debug!(
        "REST ZDIFFSTORE dest={} keys={:?}",
        req.destination, req.keys
    );

    let keys: Vec<&str> = req.keys.iter().map(|s| s.as_str()).collect();
    let count = state.sorted_set_store.zdiffstore(&req.destination, &keys);

    Ok(Json(
        json!({ "count": count, "destination": req.destination }),
    ))
}

// ==================== Geospatial Request/Response Types ====================

#[derive(Debug, Deserialize)]
pub struct GeospatialAddLocation {
    pub lat: f64,
    pub lon: f64,
    pub member: String,
}

#[derive(Debug, Deserialize)]
pub struct GeospatialAddRequest {
    pub locations: Vec<GeospatialAddLocation>,
    #[serde(default)]
    pub nx: bool,
    #[serde(default)]
    pub xx: bool,
    #[serde(default)]
    pub ch: bool,
}

#[derive(Debug, Serialize)]
pub struct GeospatialAddResponse {
    pub key: String,
    pub added: usize,
}

#[derive(Debug, Deserialize)]
pub struct GeospatialDistRequest {
    pub member1: String,
    pub member2: String,
    #[serde(default = "default_unit")]
    pub unit: String,
}
