use crate::persistence::durable;
use rmcp::model::{CallToolRequestParams, CallToolResult, ContentBlock, ErrorData};
use serde_json::json;
use std::sync::Arc;
use uuid::Uuid;

use crate::auth::OperationContext;
use crate::server::AppState;

/// The `/api/v1/command` command each tool performs. Tools are authorized
/// through the envelope classifier with their own arguments (which use the
/// envelope's field names), so a tool needs exactly the permissions its
/// envelope command needs on any other transport.
pub fn tool_command(tool: &str) -> Option<&'static str> {
    Some(match tool {
        "synap_kv_get" => "kv.get",
        "synap_kv_set" => "kv.set",
        "synap_kv_delete" => "kv.del",
        "synap_kv_append" => "kv.append",
        "synap_kv_getrange" => "kv.getrange",
        "synap_kv_strlen" => "kv.strlen",
        "synap_key_type" => "key.type",
        "synap_key_exists" => "key.exists",
        "synap_key_rename" => "key.rename",
        "synap_hash_set" => "hash.set",
        "synap_hash_get" => "hash.get",
        "synap_hash_getall" => "hash.getall",
        "synap_list_push" => "list.lpush",
        "synap_list_pop" => "list.lpop",
        "synap_list_range" => "list.lrange",
        "synap_set_add" => "set.add",
        "synap_set_members" => "set.members",
        "synap_set_inter" => "set.inter",
        "synap_queue_publish" => "queue.publish",
        "synap_sortedset_zadd" => "sortedset.zadd",
        "synap_sortedset_zrange" => "sortedset.zrange",
        "synap_sortedset_zrank" => "sortedset.zrank",
        "synap_transaction_multi" => "transaction.multi",
        "synap_transaction_exec" => "transaction.exec",
        _ => return None,
    })
}

/// Authorize a tool call for `caller`; a tool without a policy is denied.
fn authorize_tool(
    caller: &OperationContext,
    request: &CallToolRequestParams,
) -> Result<(), ErrorData> {
    if caller.is_unrestricted() {
        return Ok(());
    }
    let arguments = request
        .arguments
        .clone()
        .map(serde_json::Value::Object)
        .unwrap_or(serde_json::Value::Null);
    // An unknown tool maps to a name no classifier knows, which is denied.
    let command = tool_command(&request.name).unwrap_or("mcp.unknown-tool");
    caller
        .authorize_envelope(command, &arguments)
        .map_err(|denied| ErrorData::invalid_request(denied.to_string(), None))
}

/// Run one MCP tool for `caller`, whose identity the transport (MCP or UMICP)
/// carried with the request. The call is authorized before any tool code runs.
pub async fn handle_mcp_tool(
    request: CallToolRequestParams,
    state: Arc<AppState>,
    caller: &OperationContext,
) -> Result<CallToolResult, ErrorData> {
    authorize_tool(caller, &request)?;

    match request.name.as_ref() {
        // Essential KV tools (3)
        "synap_kv_get" => handle_kv_get(request, state).await,
        "synap_kv_set" => handle_kv_set(request, state).await,
        "synap_kv_delete" => handle_kv_delete(request, state).await,
        // String Extension tools (3)
        "synap_kv_append" => handle_kv_append(request, state).await,
        "synap_kv_getrange" => handle_kv_getrange(request, state).await,
        "synap_kv_strlen" => handle_kv_strlen(request, state).await,
        // Key Management tools (3)
        "synap_key_type" => handle_key_type(request, state).await,
        "synap_key_exists" => handle_key_exists(request, state).await,
        "synap_key_rename" => handle_key_rename(request, state).await,
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
        // Transaction tools (2)
        "synap_transaction_multi" => handle_transaction_multi(request, state).await,
        "synap_transaction_exec" => handle_transaction_exec(request, state).await,
        _ => Err(ErrorData::invalid_params("Unknown tool", None)),
    }
}

async fn handle_kv_get(
    request: CallToolRequestParams,
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

    Ok(CallToolResult::success(vec![ContentBlock::text(response)]))
}

async fn handle_kv_set(
    request: CallToolRequestParams,
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

    durable::kv_set(
        state.persistence.as_deref(),
        &state.kv_store,
        key,
        value_bytes,
        ttl,
    )
    .await
    .map_err(|e| ErrorData::internal_error(format!("Set failed: {}", e), None))?;

    Ok(CallToolResult::success(vec![ContentBlock::text(
        json!({"success": true}).to_string(),
    )]))
}

async fn handle_kv_delete(
    request: CallToolRequestParams,
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

    let deleted = durable::kv_delete(state.persistence.as_deref(), &state.kv_store, key)
        .await
        .map_err(|e| ErrorData::internal_error(format!("Delete failed: {}", e), None))?;

    Ok(CallToolResult::success(vec![ContentBlock::text(
        json!({"deleted": deleted}).to_string(),
    )]))
}

// ==================== String Extension MCP Handlers ====================

async fn handle_kv_append(
    request: CallToolRequestParams,
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

    let value_bytes = value_str.as_bytes().to_vec();

    // Recorded as the whole resulting value and its expiry, not the operand.
    let length = durable::kv_append(
        state.persistence.as_deref(),
        &state.kv_store,
        key,
        value_bytes,
    )
    .await
    .map_err(|e| ErrorData::internal_error(format!("APPEND failed: {}", e), None))?;

    Ok(CallToolResult::success(vec![ContentBlock::text(
        json!({"length": length, "key": key}).to_string(),
    )]))
}

async fn handle_kv_getrange(
    request: CallToolRequestParams,
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

    let start = args
        .get("start")
        .and_then(|v| v.as_i64())
        .ok_or_else(|| ErrorData::invalid_params("Missing start", None))? as isize;

    let end = args
        .get("end")
        .and_then(|v| v.as_i64())
        .ok_or_else(|| ErrorData::invalid_params("Missing end", None))? as isize;

    let range_bytes = state
        .kv_store
        .getrange(key, start, end)
        .await
        .map_err(|e| ErrorData::internal_error(format!("GETRANGE failed: {}", e), None))?;

    let response = String::from_utf8(range_bytes).unwrap_or_else(|_| "<binary data>".to_string());

    Ok(CallToolResult::success(vec![ContentBlock::text(
        json!({"range": response}).to_string(),
    )]))
}

async fn handle_kv_strlen(
    request: CallToolRequestParams,
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

    let length = state
        .kv_store
        .strlen(key)
        .await
        .map_err(|e| ErrorData::internal_error(format!("STRLEN failed: {}", e), None))?;

    Ok(CallToolResult::success(vec![ContentBlock::text(
        json!({"length": length, "key": key}).to_string(),
    )]))
}

// ==================== Key Management MCP Handlers ====================

async fn handle_key_type(
    request: CallToolRequestParams,
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

    let manager = crate::core::KeyManager::new(
        state.kv_store.clone(),
        state.hash_store.clone(),
        state.list_store.clone(),
        state.set_store.clone(),
        state.sorted_set_store.clone(),
    );

    let key_type = manager
        .key_type(key)
        .await
        .map_err(|e| ErrorData::internal_error(format!("TYPE failed: {}", e), None))?;

    Ok(CallToolResult::success(vec![ContentBlock::text(
        json!({"key": key, "type": key_type.as_str()}).to_string(),
    )]))
}

async fn handle_key_exists(
    request: CallToolRequestParams,
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

    let manager = crate::core::KeyManager::new(
        state.kv_store.clone(),
        state.hash_store.clone(),
        state.list_store.clone(),
        state.set_store.clone(),
        state.sorted_set_store.clone(),
    );

    let exists = manager
        .exists(key)
        .await
        .map_err(|e| ErrorData::internal_error(format!("EXISTS failed: {}", e), None))?;

    Ok(CallToolResult::success(vec![ContentBlock::text(
        json!({"key": key, "exists": exists}).to_string(),
    )]))
}

async fn handle_key_rename(
    request: CallToolRequestParams,
    state: Arc<AppState>,
) -> Result<CallToolResult, ErrorData> {
    let args = request
        .arguments
        .as_ref()
        .ok_or_else(|| ErrorData::invalid_params("Missing arguments", None))?;

    let source = args
        .get("source")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ErrorData::invalid_params("Missing source", None))?;

    let destination = args
        .get("destination")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ErrorData::invalid_params("Missing destination", None))?;

    let manager = crate::core::KeyManager::new(
        state.kv_store.clone(),
        state.hash_store.clone(),
        state.list_store.clone(),
        state.set_store.clone(),
        state.sorted_set_store.clone(),
    );

    durable::key_rename(state.persistence.as_deref(), &manager, source, destination)
        .await
        .map_err(|e| ErrorData::internal_error(format!("RENAME failed: {}", e), None))?;

    Ok(CallToolResult::success(vec![ContentBlock::text(
        json!({"success": true, "source": source, "destination": destination}).to_string(),
    )]))
}

// ==================== Hash MCP Handlers ====================

async fn handle_hash_set(
    request: CallToolRequestParams,
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

    let created = durable::hset(
        state.persistence.as_deref(),
        &state.hash_store,
        key,
        field,
        value_bytes,
    )
    .await
    .map_err(|e| ErrorData::internal_error(format!("HSET failed: {}", e), None))?;

    Ok(CallToolResult::success(vec![ContentBlock::text(
        json!({"created": created, "key": key, "field": field}).to_string(),
    )]))
}

async fn handle_hash_get(
    request: CallToolRequestParams,
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

    Ok(CallToolResult::success(vec![ContentBlock::text(response)]))
}

async fn handle_hash_getall(
    request: CallToolRequestParams,
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

    Ok(CallToolResult::success(vec![ContentBlock::text(
        json!({"fields": result, "count": result.len()}).to_string(),
    )]))
}

async fn handle_list_push(
    request: CallToolRequestParams,
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
        durable::lpush(
            state.persistence.as_deref(),
            &state.list_store,
            key,
            values,
            only_if_exists,
        )
        .await
    } else {
        durable::rpush(
            state.persistence.as_deref(),
            &state.list_store,
            key,
            values,
            only_if_exists,
        )
        .await
    }
    .map_err(|e| ErrorData::internal_error(format!("Push failed: {}", e), None))?;

    Ok(CallToolResult::success(vec![ContentBlock::text(
        json!({"length": length, "key": key}).to_string(),
    )]))
}

async fn handle_list_pop(
    request: CallToolRequestParams,
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
        durable::lpop(state.persistence.as_deref(), &state.list_store, key, count).await
    } else {
        durable::rpop(state.persistence.as_deref(), &state.list_store, key, count).await
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

    Ok(CallToolResult::success(vec![ContentBlock::text(
        json!({"values": json_values, "key": key}).to_string(),
    )]))
}

async fn handle_list_range(
    request: CallToolRequestParams,
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

    Ok(CallToolResult::success(vec![ContentBlock::text(
        json!({"values": json_values, "key": key}).to_string(),
    )]))
}

async fn handle_queue_publish(
    request: CallToolRequestParams,
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

    let queue_name = args
        .get("queue")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ErrorData::invalid_params("Missing queue name", None))?;

    let queue = queue_name;
    let message = args
        .get("message")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ErrorData::invalid_params("Missing message", None))?;

    let priority = args
        .get("priority")
        .and_then(|v| v.as_u64())
        .map(|p| p as u8);

    let message_id = durable::queue_publish(
        state.persistence.as_deref(),
        queue_manager,
        queue,
        message.as_bytes().to_vec(),
        priority,
        None,
    )
    .await
    .map(|message| message.id)
    .map_err(|e| ErrorData::internal_error(format!("Publish failed: {}", e), None))?;

    Ok(CallToolResult::success(vec![ContentBlock::text(
        json!({"message_id": message_id}).to_string(),
    )]))
}

// Set MCP handlers
async fn handle_set_add(
    request: CallToolRequestParams,
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

    let added = durable::sadd(
        state.persistence.as_deref(),
        &state.set_store,
        key,
        member_bytes,
    )
    .await
    .map_err(|e| ErrorData::internal_error(format!("SADD failed: {}", e), None))?;

    Ok(CallToolResult::success(vec![ContentBlock::text(
        json!({"added": added, "key": key}).to_string(),
    )]))
}

async fn handle_set_members(
    request: CallToolRequestParams,
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

    Ok(CallToolResult::success(vec![ContentBlock::text(
        json!({"members": members, "count": members.len()}).to_string(),
    )]))
}

async fn handle_set_inter(
    request: CallToolRequestParams,
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

    Ok(CallToolResult::success(vec![ContentBlock::text(
        json!({"intersection": intersection, "count": intersection.len()}).to_string(),
    )]))
}

async fn handle_sortedset_zadd(
    request: CallToolRequestParams,
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

    let (added, _) = durable::zadd(
        state.persistence.as_deref(),
        &state.sorted_set_store,
        key,
        member_bytes,
        score,
        &opts,
    )
    .await
    .map_err(|e| ErrorData::internal_error(format!("ZADD failed: {}", e), None))?;

    Ok(CallToolResult::success(vec![ContentBlock::text(
        json!({"added": added > 0, "key": key, "member": member, "score": score}).to_string(),
    )]))
}

async fn handle_sortedset_zrange(
    request: CallToolRequestParams,
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

    Ok(CallToolResult::success(vec![ContentBlock::text(
        json!({"members": result, "count": result.len()}).to_string(),
    )]))
}

async fn handle_sortedset_zrank(
    request: CallToolRequestParams,
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

    Ok(CallToolResult::success(vec![ContentBlock::text(
        json!({"rank": rank, "key": key, "member": member}).to_string(),
    )]))
}

// ==================== Transaction MCP Handlers ====================

async fn handle_transaction_multi(
    request: CallToolRequestParams,
    state: Arc<AppState>,
) -> Result<CallToolResult, ErrorData> {
    let args = request
        .arguments
        .as_ref()
        .ok_or_else(|| ErrorData::invalid_params("Missing arguments", None))?;

    // Get client_id from args or generate one
    let client_id = match args.get("client_id").and_then(|v| v.as_str()) {
        Some(id) => id.to_string(),
        None => Uuid::new_v4().to_string(),
    };

    state
        .transaction_manager
        .multi(client_id.clone())
        .map_err(|e| ErrorData::internal_error(format!("Transaction multi failed: {}", e), None))?;

    Ok(CallToolResult::success(vec![ContentBlock::text(
        json!({
            "success": true,
            "message": "Transaction started",
            "client_id": client_id
        })
        .to_string(),
    )]))
}

async fn handle_transaction_exec(
    request: CallToolRequestParams,
    state: Arc<AppState>,
) -> Result<CallToolResult, ErrorData> {
    let args = request
        .arguments
        .as_ref()
        .ok_or_else(|| ErrorData::invalid_params("Missing arguments", None))?;

    let client_id = args
        .get("client_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ErrorData::invalid_params("Missing client_id", None))?
        .to_string();

    match state
        .transaction_manager
        .exec(&client_id)
        .await
        .map_err(|e| ErrorData::internal_error(format!("Transaction exec failed: {}", e), None))?
    {
        Some((results, writes)) => {
            // Persist + replicate the whole transaction atomically (audit M-010).
            // A commit that could not be logged is not durable: report it.
            if let Some(ref persistence) = state.persistence
                && let Err(e) = persistence.log_transaction(&writes).await
            {
                tracing::error!("Failed to log EXEC transaction to WAL: {}", e);
                return Err(ErrorData::internal_error(
                    crate::persistence::layer::exec_not_durable_message(&e),
                    None,
                ));
            }
            Ok(CallToolResult::success(vec![ContentBlock::text(
                json!({
                    "success": true,
                    "results": results
                })
                .to_string(),
            )]))
        }
        None => Ok(CallToolResult::success(vec![ContentBlock::text(
            json!({
                "aborted": true,
                "message": "Transaction aborted: watched keys changed"
            })
            .to_string(),
        )])),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::{Action, AuthContext, Permission};
    use crate::config::McpConfig;

    /// The MCP EXEC tool fails, saying the commit is not durable, when the WAL
    /// cannot log the transaction.
    #[tokio::test]
    async fn exec_tool_reports_a_transaction_the_wal_could_not_log() {
        let mut state = crate::server::handlers::test_app_state();
        let (layer, faults) =
            crate::persistence::layer::test_support::faulty_layer("mcp_exec").await;
        state.persistence = Some(layer);
        state
            .transaction_manager
            .multi("mcp_client".to_string())
            .unwrap();
        state
            .transaction_manager
            .queue_command_if_transaction(
                "mcp_client",
                crate::core::TransactionCommand::KVSet {
                    key: "mcp:k".into(),
                    value: b"v".to_vec(),
                    ttl: None,
                },
            )
            .unwrap();

        faults.fail_next_flush();
        let request = CallToolRequestParams::new("synap_transaction_exec").with_arguments(
            json!({"client_id": "mcp_client"})
                .as_object()
                .cloned()
                .unwrap_or_default(),
        );
        match handle_transaction_exec(request, Arc::new(state)).await {
            Err(error) => assert!(error.message.contains("not durable"), "{error:?}"),
            Ok(result) => panic!("EXEC must fail when the WAL rejects it, got {result:?}"),
        }
    }

    fn every_tool() -> Vec<String> {
        let config = McpConfig {
            enable_kv_tools: true,
            enable_hash_tools: true,
            enable_list_tools: true,
            enable_set_tools: true,
            enable_queue_tools: true,
            enable_sortedset_tools: true,
            enable_transaction_tools: true,
        };
        crate::server::get_mcp_tools(&config)
            .into_iter()
            .map(|tool| tool.name.to_string())
            .collect()
    }

    /// Every tool the server can expose maps to an envelope command the
    /// classifier knows, so no tool runs without an access policy.
    #[test]
    fn every_tool_has_an_access_policy() {
        let tools = every_tool();
        assert!(tools.len() >= 24, "only {} tools listed", tools.len());
        for tool in tools {
            let command = tool_command(&tool)
                .unwrap_or_else(|| panic!("tool `{tool}` has no envelope command"));
            assert!(
                crate::auth::classify_envelope(command, &serde_json::json!({})).is_some(),
                "tool `{tool}` maps to unclassified `{command}`"
            );
        }
    }

    #[test]
    fn tools_are_authorized_with_their_arguments() {
        let reader = OperationContext::from_auth_context(&AuthContext {
            user_id: Some("reader".into()),
            api_key_id: None,
            client_ip: std::net::IpAddr::from([127, 0, 0, 1]),
            permissions: vec![Permission::new("kv:*", Action::Read)],
            is_admin: false,
        });
        let call = |tool: &str, args: serde_json::Value| {
            CallToolRequestParams::new(tool.to_string())
                .with_arguments(args.as_object().cloned().unwrap_or_default())
        };

        assert!(
            authorize_tool(
                &reader,
                &call("synap_kv_get", serde_json::json!({"key": "a"}))
            )
            .is_ok()
        );
        let denied = authorize_tool(
            &reader,
            &call(
                "synap_kv_set",
                serde_json::json!({"key": "a", "value": "1"}),
            ),
        )
        .unwrap_err();
        assert!(denied.message.contains("kv:a"), "{}", denied.message);
        assert!(authorize_tool(&reader, &call("synap_unknown", serde_json::json!({}))).is_err());
        assert!(
            authorize_tool(
                &OperationContext::unrestricted(),
                &call(
                    "synap_kv_set",
                    serde_json::json!({"key": "a", "value": "1"})
                )
            )
            .is_ok()
        );
    }
}
