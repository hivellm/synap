use rmcp::model::{CallToolRequestParam, CallToolResult, Content, ErrorData};
use serde_json::json;
use std::sync::Arc;

use crate::server::AppState;

pub async fn handle_mcp_tool(
    request: CallToolRequestParam,
    state: Arc<AppState>,
) -> Result<CallToolResult, ErrorData> {
    match request.name.as_ref() {
        "synap_kv_get" => handle_kv_get(request, state).await,
        "synap_kv_set" => handle_kv_set(request, state).await,
        "synap_kv_delete" => handle_kv_delete(request, state).await,
        "synap_kv_scan" => handle_kv_scan(request, state).await,
        "synap_queue_publish" => handle_queue_publish(request, state).await,
        "synap_queue_consume" => handle_queue_consume(request, state).await,
        "synap_stream_publish" => handle_stream_publish(request, state).await,
        "synap_pubsub_publish" => handle_pubsub_publish(request, state).await,
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
                "bytes" => serde_json::to_string(&bytes)
                    .unwrap_or_else(|_| "[]".to_string()),
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

    state
        .kv_store
        .set(key, value_str.as_bytes().to_vec(), ttl)
        .await
        .map_err(|e| ErrorData::internal_error(format!("Set failed: {}", e), None))?;

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

    Ok(CallToolResult::success(vec![Content::text(
        json!({"deleted": deleted}).to_string(),
    )]))
}

async fn handle_kv_scan(
    request: CallToolRequestParam,
    state: Arc<AppState>,
) -> Result<CallToolResult, ErrorData> {
    let args = request
        .arguments
        .as_ref()
        .ok_or_else(|| ErrorData::invalid_params("Missing arguments", None))?;

    let prefix = args.get("prefix").and_then(|v| v.as_str());
    let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(100) as usize;

    let keys = state
        .kv_store
        .scan(prefix, limit)
        .await
        .map_err(|e| ErrorData::internal_error(format!("Scan failed: {}", e), None))?;

    Ok(CallToolResult::success(vec![Content::text(
        json!({"keys": keys}).to_string(),
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

async fn handle_queue_consume(
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

    let consumer_id = args
        .get("consumer_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ErrorData::invalid_params("Missing consumer_id", None))?;

    let message = queue_manager
        .consume(queue, consumer_id)
        .await
        .map_err(|e| ErrorData::internal_error(format!("Consume failed: {}", e), None))?;

    let response = if let Some(msg) = message {
        json!({
            "message_id": msg.id,
            "payload": (*msg.payload).clone(),
            "priority": msg.priority
        })
    } else {
        json!(null)
    };

    Ok(CallToolResult::success(vec![Content::text(
        response.to_string(),
    )]))
}

async fn handle_stream_publish(
    request: CallToolRequestParam,
    state: Arc<AppState>,
) -> Result<CallToolResult, ErrorData> {
    let stream_manager = state
        .stream_manager
        .as_ref()
        .ok_or_else(|| ErrorData::internal_error("Stream system disabled", None))?;

    let args = request
        .arguments
        .as_ref()
        .ok_or_else(|| ErrorData::invalid_params("Missing arguments", None))?;

    let room = args
        .get("room")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ErrorData::invalid_params("Missing room", None))?;

    let event = args
        .get("event")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ErrorData::invalid_params("Missing event", None))?;

    let data = args
        .get("data")
        .ok_or_else(|| ErrorData::invalid_params("Missing data", None))?;

    let data_bytes = serde_json::to_vec(data)
        .map_err(|e| ErrorData::internal_error(format!("Serialization failed: {}", e), None))?;

    let offset = stream_manager
        .publish(room, event, data_bytes)
        .await
        .map_err(|e| ErrorData::internal_error(format!("Publish failed: {}", e), None))?;

    Ok(CallToolResult::success(vec![Content::text(
        json!({"offset": offset}).to_string(),
    )]))
}

async fn handle_pubsub_publish(
    request: CallToolRequestParam,
    state: Arc<AppState>,
) -> Result<CallToolResult, ErrorData> {
    let pubsub_router = state
        .pubsub_router
        .as_ref()
        .ok_or_else(|| ErrorData::internal_error("Pub/Sub system disabled", None))?;

    let args = request
        .arguments
        .as_ref()
        .ok_or_else(|| ErrorData::invalid_params("Missing arguments", None))?;

    let topic = args
        .get("topic")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ErrorData::invalid_params("Missing topic", None))?;

    let message = args
        .get("message")
        .ok_or_else(|| ErrorData::invalid_params("Missing message", None))?
        .clone();

    let result = pubsub_router
        .publish(topic, message, None)
        .map_err(|e| ErrorData::internal_error(format!("Publish failed: {}", e), None))?;

    Ok(CallToolResult::success(vec![Content::text(
        json!({
            "message_id": result.message_id,
            "subscribers_matched": result.subscribers_matched
        })
        .to_string(),
    )]))
}
