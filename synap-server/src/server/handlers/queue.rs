use super::*;

// ==================== Queue REST Endpoints ====================

/// Create queue endpoint
pub async fn queue_create(
    State(state): State<AppState>,
    AuthContextExtractor(ctx): AuthContextExtractor,

    crate::hub::HubContextExtractor(hub_ctx): crate::hub::HubContextExtractor,
    Path(queue_name): Path<String>,
    Json(req): Json<CreateQueueRequest>,
) -> Result<Json<serde_json::Value>, SynapError> {
    debug!("REST CREATE QUEUE: {}", queue_name);

    // Check permission
    require_permission(&ctx, &format!("queue:{}", queue_name), Action::Configure)?;

    let queue_manager = state
        .queue_manager
        .as_ref()
        .ok_or_else(|| SynapError::InvalidRequest("Queue system disabled".to_string()))?;

    // Apply multi-tenant scoping if Hub mode is active

    let scoped_name = crate::hub::MultiTenant::scope_queue_name(
        hub_ctx.as_ref().map(|c| c.user_id()),
        &queue_name,
    );

    let config = if req.max_depth.is_some() || req.ack_deadline_secs.is_some() {
        Some(crate::core::QueueConfig {
            max_depth: req.max_depth.unwrap_or(100_000),
            ack_deadline_secs: req.ack_deadline_secs.unwrap_or(30),
            default_max_retries: req.default_max_retries.unwrap_or(3),
            default_priority: req.default_priority.unwrap_or(5),
        })
    } else {
        None
    };

    queue_manager.create_queue(&scoped_name, config).await?;

    Ok(Json(serde_json::json!({
        "success": true,
        "queue": queue_name
    })))
}

/// Publish message endpoint
pub async fn queue_publish(
    State(state): State<AppState>,
    AuthContextExtractor(ctx): AuthContextExtractor,

    crate::hub::HubContextExtractor(hub_ctx): crate::hub::HubContextExtractor,
    Path(queue_name): Path<String>,
    Json(req): Json<PublishRequest>,
) -> Result<Json<PublishResponse>, SynapError> {
    debug!("REST PUBLISH to queue: {}", queue_name);

    // Check permission
    require_permission(&ctx, &format!("queue:{}", queue_name), Action::Write)?;

    let queue_manager = state
        .queue_manager
        .as_ref()
        .ok_or_else(|| SynapError::InvalidRequest("Queue system disabled".to_string()))?;

    // Apply multi-tenant scoping if Hub mode is active

    let scoped_name = crate::hub::MultiTenant::scope_queue_name(
        hub_ctx.as_ref().map(|c| c.user_id()),
        &queue_name,
    );

    let message = queue_manager
        .publish_with_message(&scoped_name, req.payload, req.priority, req.max_retries)
        .await?;

    let message_id = message.id.clone();

    // Log to WAL if persistence is enabled
    if let Some(ref persistence) = state.persistence {
        if let Err(e) = persistence
            .log_queue_publish(scoped_name.clone(), message)
            .await
        {
            error!("Failed to log queue publish to WAL: {}", e);
            // Don't fail the request, just log the error
        }
    }

    Ok(Json(PublishResponse { message_id }))
}

/// Consume message endpoint
pub async fn queue_consume(
    State(state): State<AppState>,
    AuthContextExtractor(ctx): AuthContextExtractor,

    crate::hub::HubContextExtractor(hub_ctx): crate::hub::HubContextExtractor,
    Path((queue_name, consumer_id)): Path<(String, String)>,
) -> Result<Json<ConsumeResponse>, SynapError> {
    debug!("REST CONSUME from queue: {} by {}", queue_name, consumer_id);

    // Check permission
    require_permission(&ctx, &format!("queue:{}", queue_name), Action::Read)?;

    let queue_manager = state
        .queue_manager
        .as_ref()
        .ok_or_else(|| SynapError::InvalidRequest("Queue system disabled".to_string()))?;

    // Apply multi-tenant scoping if Hub mode is active

    let scoped_name = crate::hub::MultiTenant::scope_queue_name(
        hub_ctx.as_ref().map(|c| c.user_id()),
        &queue_name,
    );

    let message = queue_manager.consume(&scoped_name, &consumer_id).await?;

    if let Some(msg) = message {
        Ok(Json(ConsumeResponse {
            message_id: Some(msg.id),
            payload: Some((*msg.payload).clone()), // Convert Arc<Vec<u8>> to Vec<u8>
            priority: Some(msg.priority),
            retry_count: Some(msg.retry_count),
            headers: Some(msg.headers),
        }))
    } else {
        Ok(Json(ConsumeResponse {
            message_id: None,
            payload: None,
            priority: None,
            retry_count: None,
            headers: None,
        }))
    }
}

/// ACK message endpoint
pub async fn queue_ack(
    State(state): State<AppState>,

    crate::hub::HubContextExtractor(hub_ctx): crate::hub::HubContextExtractor,
    Path(queue_name): Path<String>,
    Json(req): Json<AckRequest>,
) -> Result<Json<serde_json::Value>, SynapError> {
    debug!(
        "REST ACK message: {} in queue: {}",
        req.message_id, queue_name
    );

    let queue_manager = state
        .queue_manager
        .as_ref()
        .ok_or_else(|| SynapError::InvalidRequest("Queue system disabled".to_string()))?;

    // Apply multi-tenant scoping if Hub mode is active

    let scoped_name = crate::hub::MultiTenant::scope_queue_name(
        hub_ctx.as_ref().map(|c| c.user_id()),
        &queue_name,
    );

    queue_manager.ack(&scoped_name, &req.message_id).await?;

    // Log to WAL if persistence is enabled
    if let Some(ref persistence) = state.persistence {
        if let Err(e) = persistence
            .log_queue_ack(scoped_name.clone(), req.message_id.clone())
            .await
        {
            error!("Failed to log queue ACK to WAL: {}", e);
            // Don't fail the request, just log the error
        }
    }

    Ok(Json(serde_json::json!({ "success": true })))
}

/// NACK message endpoint
pub async fn queue_nack(
    State(state): State<AppState>,

    crate::hub::HubContextExtractor(hub_ctx): crate::hub::HubContextExtractor,
    Path(queue_name): Path<String>,
    Json(req): Json<NackRequest>,
) -> Result<Json<serde_json::Value>, SynapError> {
    debug!(
        "REST NACK message: {} in queue: {}",
        req.message_id, queue_name
    );

    let queue_manager = state
        .queue_manager
        .as_ref()
        .ok_or_else(|| SynapError::InvalidRequest("Queue system disabled".to_string()))?;

    // Apply multi-tenant scoping if Hub mode is active

    let scoped_name = crate::hub::MultiTenant::scope_queue_name(
        hub_ctx.as_ref().map(|c| c.user_id()),
        &queue_name,
    );

    queue_manager
        .nack(&scoped_name, &req.message_id, req.requeue)
        .await?;

    // Log to WAL if persistence is enabled
    if let Some(ref persistence) = state.persistence {
        if let Err(e) = persistence
            .log_queue_nack(scoped_name.clone(), req.message_id.clone(), req.requeue)
            .await
        {
            error!("Failed to log queue NACK to WAL: {}", e);
            // Don't fail the request, just log the error
        }
    }

    Ok(Json(serde_json::json!({ "success": true })))
}

/// Queue stats endpoint
pub async fn queue_stats(
    State(state): State<AppState>,
    AuthContextExtractor(ctx): AuthContextExtractor,
    Path(queue_name): Path<String>,
) -> Result<Json<serde_json::Value>, SynapError> {
    debug!("REST QUEUE STATS: {}", queue_name);

    // Check permission
    require_permission(&ctx, &format!("queue:{}", queue_name), Action::Read)?;

    let queue_manager = state
        .queue_manager
        .as_ref()
        .ok_or_else(|| SynapError::InvalidRequest("Queue system disabled".to_string()))?;

    let stats = queue_manager.stats(&queue_name).await?;

    Ok(Json(serde_json::to_value(stats).map_err(|e| {
        SynapError::SerializationError(e.to_string())
    })?))
}

/// List queues endpoint
pub async fn queue_list(
    State(state): State<AppState>,
    AuthContextExtractor(ctx): AuthContextExtractor,

    crate::hub::HubContextExtractor(hub_ctx): crate::hub::HubContextExtractor,
) -> Result<Json<serde_json::Value>, SynapError> {
    debug!("REST LIST QUEUES");

    // Check permission (read access to any queue)
    require_permission(&ctx, "queue:*", Action::Read)?;

    let queue_manager = state
        .queue_manager
        .as_ref()
        .ok_or_else(|| SynapError::InvalidRequest("Queue system disabled".to_string()))?;

    let queues = queue_manager.list_queues().await?;

    // Apply multi-tenant filtering if Hub mode is active

    let filtered_queues = crate::hub::MultiTenant::filter_user_resources(
        queues,
        hub_ctx.as_ref().map(|c| c.user_id()),
    );

    // Remove user prefixes from queue names in response

    let clean_names = crate::hub::MultiTenant::unscope_names(filtered_queues);

    Ok(Json(serde_json::json!({ "queues": clean_names })))
}

/// Purge queue endpoint
pub async fn queue_purge(
    State(state): State<AppState>,
    AuthContextExtractor(ctx): AuthContextExtractor,

    crate::hub::HubContextExtractor(hub_ctx): crate::hub::HubContextExtractor,
    Path(queue_name): Path<String>,
) -> Result<Json<serde_json::Value>, SynapError> {
    debug!("REST PURGE QUEUE: {}", queue_name);

    // Check permission
    require_permission(&ctx, &format!("queue:{}", queue_name), Action::Delete)?;

    let queue_manager = state
        .queue_manager
        .as_ref()
        .ok_or_else(|| SynapError::InvalidRequest("Queue system disabled".to_string()))?;

    // Apply multi-tenant scoping if Hub mode is active

    let scoped_name = crate::hub::MultiTenant::scope_queue_name(
        hub_ctx.as_ref().map(|c| c.user_id()),
        &queue_name,
    );

    let count = queue_manager.purge(&scoped_name).await?;

    Ok(Json(serde_json::json!({
        "success": true,
        "purged": count
    })))
}

/// Delete queue endpoint
pub async fn queue_delete(
    State(state): State<AppState>,
    AuthContextExtractor(ctx): AuthContextExtractor,

    crate::hub::HubContextExtractor(hub_ctx): crate::hub::HubContextExtractor,
    Path(queue_name): Path<String>,
) -> Result<Json<serde_json::Value>, SynapError> {
    debug!("REST DELETE QUEUE: {}", queue_name);

    // Check permission
    require_permission(&ctx, &format!("queue:{}", queue_name), Action::Delete)?;

    let queue_manager = state
        .queue_manager
        .as_ref()
        .ok_or_else(|| SynapError::InvalidRequest("Queue system disabled".to_string()))?;

    // Apply multi-tenant scoping if Hub mode is active

    let scoped_name = crate::hub::MultiTenant::scope_queue_name(
        hub_ctx.as_ref().map(|c| c.user_id()),
        &queue_name,
    );

    let deleted = queue_manager.delete_queue(&scoped_name).await?;

    Ok(Json(serde_json::json!({
        "success": true,
        "deleted": deleted
    })))
}

pub(super) async fn handle_queue_create_cmd(
    state: &AppState,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let queue_manager = state
        .queue_manager
        .as_ref()
        .ok_or_else(|| SynapError::InvalidRequest("Queue system disabled".to_string()))?;

    let name = request
        .payload
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'name' field".to_string()))?;

    let config = request.payload.get("config").and_then(|v| {
        let max_depth = v
            .get("max_depth")
            .and_then(|d| d.as_u64())
            .map(|d| d as usize);
        let ack_deadline_secs = v.get("ack_deadline_secs").and_then(|d| d.as_u64());
        let default_max_retries = v
            .get("default_max_retries")
            .and_then(|d| d.as_u64())
            .map(|d| d as u32);
        let default_priority = v
            .get("default_priority")
            .and_then(|d| d.as_u64())
            .map(|d| d as u8);

        if max_depth.is_some()
            || ack_deadline_secs.is_some()
            || default_max_retries.is_some()
            || default_priority.is_some()
        {
            Some(crate::core::QueueConfig {
                max_depth: max_depth.unwrap_or(100_000),
                ack_deadline_secs: ack_deadline_secs.unwrap_or(30),
                default_max_retries: default_max_retries.unwrap_or(3),
                default_priority: default_priority.unwrap_or(5),
            })
        } else {
            None
        }
    });

    queue_manager.create_queue(name, config).await?;
    Ok(serde_json::json!({ "success": true }))
}

pub(super) async fn handle_queue_delete_cmd(
    state: &AppState,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let queue_manager = state
        .queue_manager
        .as_ref()
        .ok_or_else(|| SynapError::InvalidRequest("Queue system disabled".to_string()))?;

    let name = request
        .payload
        .get("queue")
        .or_else(|| request.payload.get("name"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'queue' or 'name' field".to_string()))?;

    let deleted = queue_manager.delete_queue(name).await?;
    Ok(serde_json::json!({ "deleted": deleted }))
}

pub(super) async fn handle_queue_publish_cmd(
    state: &AppState,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let queue_manager = state
        .queue_manager
        .as_ref()
        .ok_or_else(|| SynapError::InvalidRequest("Queue system disabled".to_string()))?;

    let queue = request
        .payload
        .get("queue")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'queue' field".to_string()))?;

    let payload_arr = request
        .payload
        .get("payload")
        .and_then(|v| v.as_array())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'payload' field".to_string()))?;

    let payload_bytes: Vec<u8> = payload_arr
        .iter()
        .filter_map(|v| v.as_u64().map(|n| n as u8))
        .collect();

    let priority = request
        .payload
        .get("priority")
        .and_then(|v| v.as_u64())
        .map(|p| p as u8);
    let max_retries = request
        .payload
        .get("max_retries")
        .and_then(|v| v.as_u64())
        .map(|r| r as u32);
    // Note: headers are ignored for now - not supported by the queue manager
    // let headers = request.payload.get("headers")...

    let message = queue_manager
        .publish_with_message(queue, payload_bytes, priority, max_retries)
        .await?;

    let message_id = message.id.clone();

    // Log to WAL if persistence is enabled
    if let Some(ref persistence) = state.persistence {
        if let Err(e) = persistence
            .log_queue_publish(queue.to_string(), message)
            .await
        {
            error!("Failed to log queue publish to WAL: {}", e);
            // Don't fail the request, just log the error
        }
    }

    Ok(serde_json::json!({ "message_id": message_id }))
}

pub(super) async fn handle_queue_consume_cmd(
    state: &AppState,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let queue_manager = state
        .queue_manager
        .as_ref()
        .ok_or_else(|| SynapError::InvalidRequest("Queue system disabled".to_string()))?;

    let queue = request
        .payload
        .get("queue")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'queue' field".to_string()))?;

    let consumer_id = request
        .payload
        .get("consumer_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'consumer_id' field".to_string()))?;

    let message = queue_manager.consume(queue, consumer_id).await?;

    if let Some(msg) = message {
        Ok(serde_json::json!({
            "message": {
                "id": msg.id,
                "payload": (*msg.payload).clone(), // Convert Arc<Vec<u8>> to Vec<u8>
                "priority": msg.priority,
                "retry_count": msg.retry_count,
                "headers": msg.headers,
            }
        }))
    } else {
        Ok(serde_json::json!({ "message": null }))
    }
}

pub(super) async fn handle_queue_ack_cmd(
    state: &AppState,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let queue_manager = state
        .queue_manager
        .as_ref()
        .ok_or_else(|| SynapError::InvalidRequest("Queue system disabled".to_string()))?;

    let queue = request
        .payload
        .get("queue")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'queue' field".to_string()))?;

    let message_id = request
        .payload
        .get("message_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'message_id' field".to_string()))?;

    queue_manager.ack(queue, message_id).await?;

    // Log to WAL if persistence is enabled
    if let Some(ref persistence) = state.persistence {
        if let Err(e) = persistence
            .log_queue_ack(queue.to_string(), message_id.to_string())
            .await
        {
            error!("Failed to log queue ACK to WAL: {}", e);
            // Don't fail the request, just log the error
        }
    }

    Ok(serde_json::json!({ "success": true }))
}

pub(super) async fn handle_queue_nack_cmd(
    state: &AppState,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let queue_manager = state
        .queue_manager
        .as_ref()
        .ok_or_else(|| SynapError::InvalidRequest("Queue system disabled".to_string()))?;

    let queue = request
        .payload
        .get("queue")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'queue' field".to_string()))?;

    let message_id = request
        .payload
        .get("message_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'message_id' field".to_string()))?;

    let requeue = request
        .payload
        .get("requeue")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    queue_manager.nack(queue, message_id, requeue).await?;

    // Log to WAL if persistence is enabled
    if let Some(ref persistence) = state.persistence {
        if let Err(e) = persistence
            .log_queue_nack(queue.to_string(), message_id.to_string(), requeue)
            .await
        {
            error!("Failed to log queue NACK to WAL: {}", e);
            // Don't fail the request, just log the error
        }
    }

    Ok(serde_json::json!({ "success": true }))
}

pub(super) async fn handle_queue_list_cmd(
    state: &AppState,
    _request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let queue_manager = state
        .queue_manager
        .as_ref()
        .ok_or_else(|| SynapError::InvalidRequest("Queue system disabled".to_string()))?;

    let queues = queue_manager.list_queues().await?;
    Ok(serde_json::json!({ "queues": queues }))
}

pub(super) async fn handle_queue_stats_cmd(
    state: &AppState,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let queue_manager = state
        .queue_manager
        .as_ref()
        .ok_or_else(|| SynapError::InvalidRequest("Queue system disabled".to_string()))?;

    let queue = request
        .payload
        .get("queue")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'queue' field".to_string()))?;

    let stats = queue_manager.stats(queue).await?;
    serde_json::to_value(stats).map_err(|e| SynapError::SerializationError(e.to_string()))
}

pub(super) async fn handle_queue_purge_cmd(
    state: &AppState,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let queue_manager = state
        .queue_manager
        .as_ref()
        .ok_or_else(|| SynapError::InvalidRequest("Queue system disabled".to_string()))?;

    let queue = request
        .payload
        .get("queue")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'queue' field".to_string()))?;

    let count = queue_manager.purge(queue).await?;
    Ok(serde_json::json!({ "purged": count }))
}
