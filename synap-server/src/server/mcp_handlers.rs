use rmcp::model::{CallToolRequestParam, CallToolResult, Content, ErrorData};
use serde_json::json;
use std::sync::Arc;

use crate::server::AppState;

pub async fn handle_mcp_tool(
    request: CallToolRequestParam,
    state: Arc<AppState>,
) -> Result<CallToolResult, ErrorData> {
    match request.name.as_ref() {
        // Essential KV tools (3)
        "synap_kv_get" => handle_kv_get(request, state).await,
        "synap_kv_set" => handle_kv_set(request, state).await,
        "synap_kv_delete" => handle_kv_delete(request, state).await,
        // Essential Hash tools (3)
        "synap_hash_set" => handle_hash_set(request, state).await,
        "synap_hash_get" => handle_hash_get(request, state).await,
        "synap_hash_getall" => handle_hash_getall(request, state).await,
        // Essential List tools (3)
        "synap_list_push" => handle_list_push(request, state).await,
        "synap_list_pop" => handle_list_pop(request, state).await,
        "synap_list_range" => handle_list_range(request, state).await,
        // Essential Set tools (3)
        "synap_set_add" => handle_set_add(request, state).await,
        "synap_set_members" => handle_set_members(request, state).await,
        "synap_set_inter" => handle_set_inter(request, state).await,
        // Essential Queue tool (1)
        "synap_queue_publish" => handle_queue_publish(request, state).await,
        // Essential Sorted Set tools (3)
        "synap_sortedset_zadd" => handle_sortedset_zadd(request, state).await,
        "synap_sortedset_zrange" => handle_sortedset_zrange(request, state).await,
        "synap_sortedset_zrank" => handle_sortedset_zrank(request, state).await,
        _ => Err(ErrorData::invalid_params("Unknown tool", None)),
    }
}

async fn handle_kv_get(
    request: CallToolRequestParam,
    state: Arc<AppState>,
) -> Result<CallToolResult, ErrorData> {
    let args = request
        .arguments
        .as_ref()
        .ok_or_else(|| ErrorData::invalid_params("Missing arguments", None))?;

    let key = args
        .get("key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ErrorData::invalid_params("Missing key", None))?;

    let return_type = args
        .get("type")
        .and_then(|v| v.as_str())
        .unwrap_or("string");

    let value_bytes = state
        .kv_store
        .get(key)
        .await
        .map_err(|e| ErrorData::internal_error(format!("Get failed: {}", e), None))?;

    let response = match value_bytes {
        Some(bytes) => {
            match return_type {
                "bytes" => serde_json::to_string(&bytes).unwrap_or_else(|_| "[]".to_string()),
                _ => {
                    // Default: return as string
                    String::from_utf8(bytes)
                        .unwrap_or_else(|e| format!("<binary data: {} bytes>", e.as_bytes().len()))
                }
            }
        }
        None => "null".to_string(),
    };

    Ok(CallToolResult::success(vec![Content::text(response)]))
}

async fn handle_kv_set(
    request: CallToolRequestParam,
    state: Arc<AppState>,
) -> Result<CallToolResult, ErrorData> {
    let args = request
        .arguments
        .as_ref()
        .ok_or_else(|| ErrorData::invalid_params("Missing arguments", None))?;

    let key = args
        .get("key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ErrorData::invalid_params("Missing key", None))?;

    let value_str = args
        .get("value")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ErrorData::invalid_params("Missing value", None))?;

    let ttl = args.get("ttl").and_then(|v| v.as_u64());

    let value_bytes = value_str.as_bytes().to_vec();

    state
        .kv_store
        .set(key, value_bytes.clone(), ttl)
        .await
        .map_err(|e| ErrorData::internal_error(format!("Set failed: {}", e), None))?;

    // Log to WAL (async, non-blocking)
    if let Some(ref persistence) = state.persistence {
        if let Err(e) = persistence
            .log_kv_set(key.to_string(), value_bytes, ttl)
            .await
        {
            tracing::error!("Failed to log KV SET to WAL (MCP): {}", e);
        }
    }

    Ok(CallToolResult::success(vec![Content::text(
        json!({"success": true}).to_string(),
    )]))
}

async fn handle_kv_delete(
    request: CallToolRequestParam,
    state: Arc<AppState>,
) -> Result<CallToolResult, ErrorData> {
    let args = request
        .arguments
        .as_ref()
        .ok_or_else(|| ErrorData::invalid_params("Missing arguments", None))?;

    let key = args
        .get("key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ErrorData::invalid_params("Missing key", None))?;

    let deleted = state
        .kv_store
        .delete(key)
        .await
        .map_err(|e| ErrorData::internal_error(format!("Delete failed: {}", e), None))?;

    // Log to WAL if deleted
    if deleted {
        if let Some(ref persistence) = state.persistence {
            if let Err(e) = persistence.log_kv_del(vec![key.to_string()]).await {
                tracing::error!("Failed to log KV DELETE to WAL (MCP): {}", e);
            }
        }
    }

    Ok(CallToolResult::success(vec![Content::text(
        json!({"deleted": deleted}).to_string(),
    )]))
}

// ==================== Hash MCP Handlers ====================

async fn handle_hash_set(
    request: CallToolRequestParam,
    state: Arc<AppState>,
) -> Result<CallToolResult, ErrorData> {
    let args = request
        .arguments
        .as_ref()
        .ok_or_else(|| ErrorData::invalid_params("Missing arguments", None))?;

    let key = args
        .get("key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ErrorData::invalid_params("Missing key", None))?;

    let field = args
        .get("field")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ErrorData::invalid_params("Missing field", None))?;

    let value = args
        .get("value")
        .ok_or_else(|| ErrorData::invalid_params("Missing value", None))?;

    let value_bytes = serde_json::to_vec(value)
        .map_err(|e| ErrorData::internal_error(format!("Serialization failed: {}", e), None))?;

    let created = state
        .hash_store
        .hset(key, field, value_bytes)
        .map_err(|e| ErrorData::internal_error(format!("HSET failed: {}", e), None))?;

    Ok(CallToolResult::success(vec![Content::text(
        json!({"created": created, "key": key, "field": field}).to_string(),
    )]))
}

async fn handle_hash_get(
    request: CallToolRequestParam,
    state: Arc<AppState>,
) -> Result<CallToolResult, ErrorData> {
    let args = request
        .arguments
        .as_ref()
        .ok_or_else(|| ErrorData::invalid_params("Missing arguments", None))?;

    let key = args
        .get("key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ErrorData::invalid_params("Missing key", None))?;

    let field = args
        .get("field")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ErrorData::invalid_params("Missing field", None))?;

    let value_bytes = state
        .hash_store
        .hget(key, field)
        .map_err(|e| ErrorData::internal_error(format!("HGET failed: {}", e), None))?;

    let response = match value_bytes {
        Some(bytes) => {
            let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap_or_else(|_| {
                serde_json::Value::String(String::from_utf8_lossy(&bytes).to_string())
            });
            json!({"found": true, "value": value}).to_string()
        }
        None => json!({"found": false}).to_string(),
    };

    Ok(CallToolResult::success(vec![Content::text(response)]))
}

async fn handle_hash_getall(
    request: CallToolRequestParam,
    state: Arc<AppState>,
) -> Result<CallToolResult, ErrorData> {
    let args = request
        .arguments
        .as_ref()
        .ok_or_else(|| ErrorData::invalid_params("Missing arguments", None))?;

    let key = args
        .get("key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ErrorData::invalid_params("Missing key", None))?;

    let all = state
        .hash_store
        .hgetall(key)
        .map_err(|e| ErrorData::internal_error(format!("HGETALL failed: {}", e), None))?;

    let result: std::collections::HashMap<String, serde_json::Value> = all
        .into_iter()
        .map(|(k, v)| {
            let json_value: serde_json::Value = serde_json::from_slice(&v).unwrap_or_else(|_| {
                serde_json::Value::String(String::from_utf8_lossy(&v).to_string())
            });
            (k, json_value)
        })
        .collect();

    Ok(CallToolResult::success(vec![Content::text(
        json!({"fields": result, "count": result.len()}).to_string(),
    )]))
}

async fn handle_list_push(
    request: CallToolRequestParam,
    state: Arc<AppState>,
) -> Result<CallToolResult, ErrorData> {
    let args = request
        .arguments
        .as_ref()
        .ok_or_else(|| ErrorData::invalid_params("Missing arguments", None))?;

    let key = args
        .get("key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ErrorData::invalid_params("Missing key", None))?;

    let values_array = args
        .get("values")
        .and_then(|v| v.as_array())
        .ok_or_else(|| ErrorData::invalid_params("Missing values array", None))?;

    let direction = args
        .get("direction")
        .and_then(|v| v.as_str())
        .unwrap_or("right");

    let only_if_exists = args
        .get("only_if_exists")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let values: Vec<Vec<u8>> = values_array
        .iter()
        .map(|v| serde_json::to_vec(v).map_err(|e| ErrorData::internal_error(e.to_string(), None)))
        .collect::<Result<Vec<_>, _>>()?;

    let length = if direction == "left" {
        state.list_store.lpush(key, values, only_if_exists)
    } else {
        state.list_store.rpush(key, values, only_if_exists)
    }
    .map_err(|e| ErrorData::internal_error(format!("Push failed: {}", e), None))?;

    Ok(CallToolResult::success(vec![Content::text(
        json!({"length": length, "key": key}).to_string(),
    )]))
}

async fn handle_list_pop(
    request: CallToolRequestParam,
    state: Arc<AppState>,
) -> Result<CallToolResult, ErrorData> {
    let args = request
        .arguments
        .as_ref()
        .ok_or_else(|| ErrorData::invalid_params("Missing arguments", None))?;

    let key = args
        .get("key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ErrorData::invalid_params("Missing key", None))?;

    let direction = args
        .get("direction")
        .and_then(|v| v.as_str())
        .unwrap_or("left");

    let count = args
        .get("count")
        .and_then(|v| v.as_u64())
        .map(|c| c as usize);

    let values = if direction == "left" {
        state.list_store.lpop(key, count)
    } else {
        state.list_store.rpop(key, count)
    }
    .map_err(|e| ErrorData::internal_error(format!("Pop failed: {}", e), None))?;

    let json_values: Vec<serde_json::Value> = values
        .into_iter()
        .map(|v| {
            serde_json::from_slice(&v).unwrap_or_else(|_| {
                serde_json::Value::String(String::from_utf8_lossy(&v).to_string())
            })
        })
        .collect();

    Ok(CallToolResult::success(vec![Content::text(
        json!({"values": json_values, "key": key}).to_string(),
    )]))
}

async fn handle_list_range(
    request: CallToolRequestParam,
    state: Arc<AppState>,
) -> Result<CallToolResult, ErrorData> {
    let args = request
        .arguments
        .as_ref()
        .ok_or_else(|| ErrorData::invalid_params("Missing arguments", None))?;

    let key = args
        .get("key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ErrorData::invalid_params("Missing key", None))?;

    let start = args.get("start").and_then(|v| v.as_i64()).unwrap_or(0);

    let stop = args.get("stop").and_then(|v| v.as_i64()).unwrap_or(-1);

    let values = state
        .list_store
        .lrange(key, start, stop)
        .map_err(|e| ErrorData::internal_error(format!("LRANGE failed: {}", e), None))?;

    let json_values: Vec<serde_json::Value> = values
        .into_iter()
        .map(|v| {
            serde_json::from_slice(&v).unwrap_or_else(|_| {
                serde_json::Value::String(String::from_utf8_lossy(&v).to_string())
            })
        })
        .collect();

    Ok(CallToolResult::success(vec![Content::text(
        json!({"values": json_values, "key": key}).to_string(),
    )]))
}

async fn handle_queue_publish(
    request: CallToolRequestParam,
    state: Arc<AppState>,
) -> Result<CallToolResult, ErrorData> {
    let queue_manager = state
        .queue_manager
        .as_ref()
        .ok_or_else(|| ErrorData::internal_error("Queue system disabled", None))?;

    let args = request
        .arguments
        .as_ref()
        .ok_or_else(|| ErrorData::invalid_params("Missing arguments", None))?;

    let queue = args
        .get("queue")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ErrorData::invalid_params("Missing queue", None))?;

    let message = args
        .get("message")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ErrorData::invalid_params("Missing message", None))?;

    let priority = args
        .get("priority")
        .and_then(|v| v.as_u64())
        .map(|p| p as u8);

    let message_id = queue_manager
        .publish(queue, message.as_bytes().to_vec(), priority, None)
        .await
        .map_err(|e| ErrorData::internal_error(format!("Publish failed: {}", e), None))?;

    Ok(CallToolResult::success(vec![Content::text(
        json!({"message_id": message_id}).to_string(),
    )]))
}

// Set MCP handlers
async fn handle_set_add(
    request: CallToolRequestParam,
    state: Arc<AppState>,
) -> Result<CallToolResult, ErrorData> {
    let args = request
        .arguments
        .as_ref()
        .ok_or_else(|| ErrorData::invalid_params("Missing arguments", None))?;

    let key = args
        .get("key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ErrorData::invalid_params("Missing key", None))?;

    let members = args
        .get("members")
        .and_then(|v| v.as_array())
        .ok_or_else(|| ErrorData::invalid_params("Missing members array", None))?;

    let member_bytes: Vec<Vec<u8>> = members
        .iter()
        .map(|v| serde_json::to_vec(v).unwrap_or_default())
        .collect();

    let added = state
        .set_store
        .sadd(key, member_bytes)
        .map_err(|e| ErrorData::internal_error(format!("SADD failed: {}", e), None))?;

    Ok(CallToolResult::success(vec![Content::text(
        json!({"added": added, "key": key}).to_string(),
    )]))
}

async fn handle_set_members(
    request: CallToolRequestParam,
    state: Arc<AppState>,
) -> Result<CallToolResult, ErrorData> {
    let args = request
        .arguments
        .as_ref()
        .ok_or_else(|| ErrorData::invalid_params("Missing arguments", None))?;

    let key = args
        .get("key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ErrorData::invalid_params("Missing key", None))?;

    let members_bytes = state
        .set_store
        .smembers(key)
        .map_err(|e| ErrorData::internal_error(format!("SMEMBERS failed: {}", e), None))?;

    let members: Vec<serde_json::Value> = members_bytes
        .iter()
        .map(|b| serde_json::from_slice(b).unwrap_or(json!(null)))
        .collect();

    Ok(CallToolResult::success(vec![Content::text(
        json!({"members": members, "count": members.len()}).to_string(),
    )]))
}

async fn handle_set_inter(
    request: CallToolRequestParam,
    state: Arc<AppState>,
) -> Result<CallToolResult, ErrorData> {
    let args = request
        .arguments
        .as_ref()
        .ok_or_else(|| ErrorData::invalid_params("Missing arguments", None))?;

    let keys = args
        .get("keys")
        .and_then(|v| v.as_array())
        .ok_or_else(|| ErrorData::invalid_params("Missing keys array", None))?;

    let key_strs: Vec<String> = keys
        .iter()
        .filter_map(|v| v.as_str().map(|s| s.to_string()))
        .collect();

    if key_strs.is_empty() {
        return Err(ErrorData::invalid_params("No valid keys provided", None));
    }

    let intersection_bytes = state
        .set_store
        .sinter(&key_strs)
        .map_err(|e| ErrorData::internal_error(format!("SINTER failed: {}", e), None))?;

    let intersection: Vec<serde_json::Value> = intersection_bytes
        .iter()
        .map(|b| serde_json::from_slice(b).unwrap_or(json!(null)))
        .collect();

    Ok(CallToolResult::success(vec![Content::text(
        json!({"intersection": intersection, "count": intersection.len()}).to_string(),
    )]))
}

async fn handle_sortedset_zadd(
    request: CallToolRequestParam,
    state: Arc<AppState>,
) -> Result<CallToolResult, ErrorData> {
    let args = request
        .arguments
        .as_ref()
        .ok_or_else(|| ErrorData::invalid_params("Missing arguments", None))?;

    let key = args
        .get("key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ErrorData::invalid_params("Missing key", None))?;

    let member = args
        .get("member")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ErrorData::invalid_params("Missing member", None))?;

    let score = args
        .get("score")
        .and_then(|v| v.as_f64())
        .ok_or_else(|| ErrorData::invalid_params("Missing or invalid score", None))?;

    let member_bytes = member.as_bytes().to_vec();
    let opts = crate::core::ZAddOptions::default();

    let (added, _) = state.sorted_set_store.zadd(key, member_bytes, score, &opts);

    Ok(CallToolResult::success(vec![Content::text(
        json!({"added": added > 0, "key": key, "member": member, "score": score}).to_string(),
    )]))
}

async fn handle_sortedset_zrange(
    request: CallToolRequestParam,
    state: Arc<AppState>,
) -> Result<CallToolResult, ErrorData> {
    let args = request
        .arguments
        .as_ref()
        .ok_or_else(|| ErrorData::invalid_params("Missing arguments", None))?;

    let key = args
        .get("key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ErrorData::invalid_params("Missing key", None))?;

    let start = args.get("start").and_then(|v| v.as_i64()).unwrap_or(0);

    let stop = args.get("stop").and_then(|v| v.as_i64()).unwrap_or(-1);

    let with_scores = args
        .get("withscores")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    let members = state.sorted_set_store.zrange(key, start, stop, with_scores);

    let result: Vec<serde_json::Value> = members
        .iter()
        .map(|sm| {
            if with_scores {
                json!({
                    "member": String::from_utf8_lossy(&sm.member),
                    "score": sm.score
                })
            } else {
                json!(String::from_utf8_lossy(&sm.member))
            }
        })
        .collect();

    Ok(CallToolResult::success(vec![Content::text(
        json!({"members": result, "count": result.len()}).to_string(),
    )]))
}

async fn handle_sortedset_zrank(
    request: CallToolRequestParam,
    state: Arc<AppState>,
) -> Result<CallToolResult, ErrorData> {
    let args = request
        .arguments
        .as_ref()
        .ok_or_else(|| ErrorData::invalid_params("Missing arguments", None))?;

    let key = args
        .get("key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ErrorData::invalid_params("Missing key", None))?;

    let member = args
        .get("member")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ErrorData::invalid_params("Missing member", None))?;

    let member_bytes = member.as_bytes();
    let rank = state.sorted_set_store.zrank(key, member_bytes);

    Ok(CallToolResult::success(vec![Content::text(
        json!({"rank": rank, "key": key, "member": member}).to_string(),
    )]))
}
