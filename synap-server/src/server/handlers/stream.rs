use super::*;

// ==================== Event Stream REST Endpoints ====================

/// Create stream room
pub async fn stream_create_room(
    State(state): State<AppState>,
    AuthContextExtractor(ctx): AuthContextExtractor,

    crate::hub::HubContextExtractor(hub_ctx): crate::hub::HubContextExtractor,
    Path(room_name): Path<String>,
) -> Result<Json<serde_json::Value>, SynapError> {
    debug!("REST CREATE STREAM ROOM: {}", room_name);

    // Check permission
    require_permission(&ctx, &format!("stream:{}", room_name), Action::Configure)?;

    let stream_manager = state
        .stream_manager
        .as_ref()
        .ok_or_else(|| SynapError::InvalidRequest("Stream system disabled".to_string()))?;

    // Apply multi-tenant scoping if Hub mode is active

    let scoped_name = crate::hub::MultiTenant::scope_stream_name(
        hub_ctx.as_ref().map(|c| c.user_id()),
        &room_name,
    );

    stream_manager
        .create_room(&scoped_name)
        .await
        .map_err(SynapError::InvalidRequest)?;

    Ok(Json(serde_json::json!({
        "success": true,
        "room": room_name
    })))
}

/// Publish event to stream room
pub async fn stream_publish(
    State(state): State<AppState>,
    AuthContextExtractor(ctx): AuthContextExtractor,

    crate::hub::HubContextExtractor(hub_ctx): crate::hub::HubContextExtractor,
    Path(room_name): Path<String>,
    Json(req): Json<StreamPublishRequest>,
) -> Result<Json<StreamPublishResponse>, SynapError> {
    debug!("REST STREAM PUBLISH to room: {}", room_name);

    // Check permission
    require_permission(&ctx, &format!("stream:{}", room_name), Action::Write)?;

    let stream_manager = state
        .stream_manager
        .as_ref()
        .ok_or_else(|| SynapError::InvalidRequest("Stream system disabled".to_string()))?;

    // Apply multi-tenant scoping if Hub mode is active

    let scoped_name = crate::hub::MultiTenant::scope_stream_name(
        hub_ctx.as_ref().map(|c| c.user_id()),
        &room_name,
    );

    let data_bytes =
        serde_json::to_vec(&req.data).map_err(|e| SynapError::SerializationError(e.to_string()))?;

    let offset = stream_manager
        .publish(&scoped_name, &req.event, data_bytes)
        .await
        .map_err(SynapError::InvalidRequest)?;

    Ok(Json(StreamPublishResponse {
        offset,
        room: room_name,
    }))
}

/// Consume events from stream room
pub async fn stream_consume(
    State(state): State<AppState>,
    AuthContextExtractor(ctx): AuthContextExtractor,

    crate::hub::HubContextExtractor(hub_ctx): crate::hub::HubContextExtractor,
    Path((room_name, subscriber_id)): Path<(String, String)>,
    axum::extract::Query(params): axum::extract::Query<HashMap<String, String>>,
) -> Result<Json<StreamConsumeResponse>, SynapError> {
    debug!(
        "REST STREAM CONSUME from room: {}, subscriber: {}",
        room_name, subscriber_id
    );

    // Check permission
    require_permission(&ctx, &format!("stream:{}", room_name), Action::Read)?;

    let stream_manager = state
        .stream_manager
        .as_ref()
        .ok_or_else(|| SynapError::InvalidRequest("Stream system disabled".to_string()))?;

    // Apply multi-tenant scoping if Hub mode is active

    let scoped_name = crate::hub::MultiTenant::scope_stream_name(
        hub_ctx.as_ref().map(|c| c.user_id()),
        &room_name,
    );

    let from_offset = params
        .get("from_offset")
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0);

    let limit = params
        .get("limit")
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(100);

    let events = stream_manager
        .consume(&scoped_name, &subscriber_id, from_offset, limit)
        .await
        .map_err(SynapError::InvalidRequest)?;

    let next_offset = events.last().map(|e| e.offset + 1).unwrap_or(from_offset);

    Ok(Json(StreamConsumeResponse {
        events,
        next_offset,
    }))
}

/// Get stream room statistics
pub async fn stream_room_stats(
    State(state): State<AppState>,
    AuthContextExtractor(ctx): AuthContextExtractor,

    crate::hub::HubContextExtractor(hub_ctx): crate::hub::HubContextExtractor,
    Path(room_name): Path<String>,
) -> Result<Json<crate::core::RoomStats>, SynapError> {
    debug!("REST STREAM STATS for room: {}", room_name);

    // Check permission
    require_permission(&ctx, &format!("stream:{}", room_name), Action::Read)?;

    let stream_manager = state
        .stream_manager
        .as_ref()
        .ok_or_else(|| SynapError::InvalidRequest("Stream system disabled".to_string()))?;

    // Apply multi-tenant scoping if Hub mode is active

    let scoped_name = crate::hub::MultiTenant::scope_stream_name(
        hub_ctx.as_ref().map(|c| c.user_id()),
        &room_name,
    );

    let stats = stream_manager
        .room_stats(&scoped_name)
        .await
        .map_err(SynapError::InvalidRequest)?;

    Ok(Json(stats))
}

/// List all stream rooms
pub async fn stream_list_rooms(
    State(state): State<AppState>,
    AuthContextExtractor(ctx): AuthContextExtractor,

    crate::hub::HubContextExtractor(hub_ctx): crate::hub::HubContextExtractor,
) -> Result<Json<serde_json::Value>, SynapError> {
    debug!("REST STREAM LIST ROOMS");

    // Check permission (read access to any stream)
    require_permission(&ctx, "stream:*", Action::Read)?;

    let stream_manager = state
        .stream_manager
        .as_ref()
        .ok_or_else(|| SynapError::InvalidRequest("Stream system disabled".to_string()))?;

    let all_rooms = stream_manager.list_rooms().await;

    // Filter rooms by user in Hub mode

    let rooms =
        crate::hub::MultiTenant::unscope_names(crate::hub::MultiTenant::filter_user_resources(
            all_rooms,
            hub_ctx.as_ref().map(|c| c.user_id()),
        ));

    Ok(Json(serde_json::json!({
        "rooms": rooms,
        "count": rooms.len()
    })))
}

/// Delete stream room
pub async fn stream_delete_room(
    State(state): State<AppState>,
    AuthContextExtractor(ctx): AuthContextExtractor,

    crate::hub::HubContextExtractor(hub_ctx): crate::hub::HubContextExtractor,
    Path(room_name): Path<String>,
) -> Result<Json<serde_json::Value>, SynapError> {
    debug!("REST DELETE STREAM ROOM: {}", room_name);

    // Check permission
    require_permission(&ctx, &format!("stream:{}", room_name), Action::Delete)?;

    let stream_manager = state
        .stream_manager
        .as_ref()
        .ok_or_else(|| SynapError::InvalidRequest("Stream system disabled".to_string()))?;

    // Apply multi-tenant scoping if Hub mode is active

    let scoped_name = crate::hub::MultiTenant::scope_stream_name(
        hub_ctx.as_ref().map(|c| c.user_id()),
        &room_name,
    );

    stream_manager
        .delete_room(&scoped_name)
        .await
        .map_err(SynapError::InvalidRequest)?;

    Ok(Json(serde_json::json!({
        "success": true,
        "room": room_name
    })))
}

// ============================================================================
// Event Streams StreamableHTTP Command Handlers
// ============================================================================

pub(super) async fn handle_stream_create_cmd(
    state: &AppState,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let stream_manager = state
        .stream_manager
        .as_ref()
        .ok_or_else(|| SynapError::InvalidRequest("Stream system disabled".to_string()))?;

    let room = request
        .payload
        .get("room")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'room' field".to_string()))?;

    stream_manager
        .create_room(room)
        .await
        .map_err(SynapError::InvalidRequest)?;

    Ok(serde_json::json!({
        "success": true,
        "room": room
    }))
}

pub(super) async fn handle_stream_publish_cmd(
    state: &AppState,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let stream_manager = state
        .stream_manager
        .as_ref()
        .ok_or_else(|| SynapError::InvalidRequest("Stream system disabled".to_string()))?;

    let room = request
        .payload
        .get("room")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'room' field".to_string()))?;

    let event = request
        .payload
        .get("event")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'event' field".to_string()))?;

    let data = request
        .payload
        .get("data")
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'data' field".to_string()))?;

    let data_bytes =
        serde_json::to_vec(data).map_err(|e| SynapError::SerializationError(e.to_string()))?;

    let offset = stream_manager
        .publish(room, event, data_bytes)
        .await
        .map_err(SynapError::InvalidRequest)?;

    Ok(serde_json::json!({
        "offset": offset,
        "room": room
    }))
}

pub(super) async fn handle_stream_consume_cmd(
    state: &AppState,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let stream_manager = state
        .stream_manager
        .as_ref()
        .ok_or_else(|| SynapError::InvalidRequest("Stream system disabled".to_string()))?;

    let room = request
        .payload
        .get("room")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'room' field".to_string()))?;

    let subscriber_id = request
        .payload
        .get("subscriber_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'subscriber_id' field".to_string()))?;

    let from_offset = request
        .payload
        .get("from_offset")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);

    let limit = request
        .payload
        .get("limit")
        .and_then(|v| v.as_u64())
        .unwrap_or(100) as usize;

    let events = stream_manager
        .consume(room, subscriber_id, from_offset, limit)
        .await
        .map_err(SynapError::InvalidRequest)?;

    let next_offset = events.last().map(|e| e.offset + 1).unwrap_or(from_offset);

    Ok(serde_json::json!({
        "events": events,
        "next_offset": next_offset
    }))
}

pub(super) async fn handle_stream_stats_cmd(
    state: &AppState,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let stream_manager = state
        .stream_manager
        .as_ref()
        .ok_or_else(|| SynapError::InvalidRequest("Stream system disabled".to_string()))?;

    let room = request
        .payload
        .get("room")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'room' field".to_string()))?;

    let stats = stream_manager
        .room_stats(room)
        .await
        .map_err(SynapError::InvalidRequest)?;

    serde_json::to_value(stats).map_err(|e| SynapError::SerializationError(e.to_string()))
}

pub(super) async fn handle_stream_list_cmd(
    state: &AppState,
    _request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let stream_manager = state
        .stream_manager
        .as_ref()
        .ok_or_else(|| SynapError::InvalidRequest("Stream system disabled".to_string()))?;

    let rooms = stream_manager.list_rooms().await;

    Ok(serde_json::json!({
        "rooms": rooms,
        "count": rooms.len()
    }))
}

pub(super) async fn handle_stream_delete_cmd(
    state: &AppState,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let stream_manager = state
        .stream_manager
        .as_ref()
        .ok_or_else(|| SynapError::InvalidRequest("Stream system disabled".to_string()))?;

    let room = request
        .payload
        .get("room")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'room' field".to_string()))?;

    stream_manager
        .delete_room(room)
        .await
        .map_err(SynapError::InvalidRequest)?;

    Ok(serde_json::json!({
        "success": true,
        "deleted": room
    }))
}
