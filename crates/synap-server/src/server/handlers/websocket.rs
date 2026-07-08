use super::*;

// ============================================================================
// Pub/Sub WebSocket Handler
// ============================================================================

pub async fn kv_websocket(
    State(_state): State<AppState>,
    _ws: WebSocketUpgrade,
    axum::extract::Query(params): axum::extract::Query<HashMap<String, String>>,
) -> AxumResponse {
    // Parse keys from query params
    let keys_str = params.get("keys").cloned().unwrap_or_default();
    let keys: Vec<String> = keys_str
        .split(',')
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect();

    if keys.is_empty() {
        return (
            axum::http::StatusCode::BAD_REQUEST,
            "At least one key required in query param: ?keys=key1,key2",
        )
            .into_response();
    }

    info!("KV WebSocket WATCH connection for keys: {:?}", keys);

    // Note: Full implementation would require KVStore to support change notifications
    // For now, return not implemented
    (
        axum::http::StatusCode::NOT_IMPLEMENTED,
        "KV WebSocket WATCH not yet implemented - use polling for now",
    )
        .into_response()
}

// ============================================================================
// Queue WebSocket Handler
// ============================================================================

/// WebSocket handler for Queue continuous consume (real-time message delivery)
/// GET /queue/:name/ws/:consumer_id
pub async fn queue_websocket(
    State(state): State<AppState>,
    Path((queue_name, consumer_id)): Path<(String, String)>,
    axum::extract::ConnectInfo(addr): axum::extract::ConnectInfo<SocketAddr>,
    ws: WebSocketUpgrade,
) -> AxumResponse {
    let queue_manager = match state.queue_manager.as_ref() {
        Some(qm) => qm.clone(),
        None => {
            return (
                axum::http::StatusCode::SERVICE_UNAVAILABLE,
                "Queue system disabled",
            )
                .into_response();
        }
    };

    let client_list_manager = state.client_list_manager.clone();
    let client_addr = addr.to_string();
    let client_id = format!("queue-{}-{}", queue_name, consumer_id);

    info!(
        "Queue WebSocket connection: queue={}, consumer={}, addr={}",
        queue_name, consumer_id, client_addr
    );

    ws.on_upgrade(move |socket| {
        handle_queue_socket(
            socket,
            queue_manager,
            queue_name,
            consumer_id,
            client_list_manager,
            client_id,
            client_addr,
        )
    })
}

/// Handle Queue WebSocket connection
pub(super) async fn handle_queue_socket(
    socket: WebSocket,
    queue_manager: Arc<crate::core::QueueManager>,
    queue_name: String,
    consumer_id: String,
    client_list_manager: Arc<crate::monitoring::ClientListManager>,
    client_id: String,
    client_addr: String,
) {
    let connected_at = std::time::SystemTime::now();
    let client_info =
        crate::monitoring::ClientInfo::new(client_id.clone(), client_addr, connected_at);
    client_list_manager.add(client_info).await;

    let (mut ws_sender, mut ws_receiver) = socket.split();

    // Send welcome message
    let welcome = json!({
        "type": "connected",
        "queue": queue_name,
        "consumer_id": consumer_id
    });

    if ws_sender
        .send(axum::extract::ws::Message::Text(welcome.to_string().into()))
        .await
        .is_err()
    {
        warn!("Failed to send welcome to consumer: {}", consumer_id);
        client_list_manager.remove(&client_id).await;
        return;
    }

    loop {
        tokio::select! {
            // Try to consume a message (non-blocking with timeout)
            _ = tokio::time::sleep(tokio::time::Duration::from_millis(100)) => {
                match queue_manager.consume(&queue_name, &consumer_id).await {
                    Ok(Some(msg)) => {
                        let msg_json = json!({
                            "type": "message",
                            "message_id": msg.id,
                            "payload": (*msg.payload).clone(),  // Clone Vec<u8> from Arc
                            "priority": msg.priority,
                            "retry_count": msg.retry_count,
                            "created_at": msg.created_at,
                            "headers": msg.headers
                        });

                        if ws_sender.send(axum::extract::ws::Message::Text(msg_json.to_string().into())).await.is_err() {
                            warn!("Failed to send message to consumer: {}", consumer_id);
                            break;
                        }
                    }
                    Ok(None) => {
                        // No messages available, continue waiting
                    }
                    Err(e) => {
                         error!("Queue consume error: {}", e);
                        let _ = ws_sender.send(axum::extract::ws::Message::Text(
                            json!({"type": "error", "error": e.to_string()}).to_string().into()
                        )).await;
                        break;
                    }
                }
            }

            // Handle incoming WebSocket messages (ACK/NACK commands)
            Some(msg) = ws_receiver.next() => {
                match msg {
                    Ok(axum::extract::ws::Message::Text(text)) => {
                        if let Ok(cmd) = serde_json::from_str::<serde_json::Value>(&text) {
                            match cmd["command"].as_str() {
                                Some("ack") => {
                                    if let Some(msg_id) = cmd["message_id"].as_str() {
                                        if let Err(e) = queue_manager.ack(&queue_name, msg_id).await {
                                            error!("ACK error: {}", e);
                                        }
                                    }
                                }
                                Some("nack") => {
                                    if let Some(msg_id) = cmd["message_id"].as_str() {
                                        let requeue = cmd["requeue"].as_bool().unwrap_or(true);
                                        if let Err(e) = queue_manager.nack(&queue_name, msg_id, requeue).await {
                                            error!("NACK error: {}", e);
                                        }
                                    }
                                }
                                _ => {
                                    warn!("Unknown command: {:?}", cmd["command"]);
                                }
                            }
                        }
                    }
                    Ok(axum::extract::ws::Message::Close(_)) => {
                        info!("Queue consumer {} closed connection", consumer_id);
                        break;
                    }
                    Ok(axum::extract::ws::Message::Ping(data)) => {
                        if ws_sender.send(axum::extract::ws::Message::Pong(data)).await.is_err() {
                            break;
                        }
                    }
                    Ok(_) => {}
                    Err(e) => {
                        warn!("WebSocket error for consumer {}: {}", consumer_id, e);
                        break;
                    }
                }
            }

            else => {
                break;
            }
        }
    }

    // Cleanup: remove client from tracking
    client_list_manager.remove(&client_id).await;
    info!("Queue consumer {} disconnected", consumer_id);
}

// ============================================================================
// Event Streams WebSocket Handler
// ============================================================================

/// WebSocket handler for Event Streams (real-time event push)
/// GET /stream/:room/ws/:subscriber_id?from_offset=0
pub async fn stream_websocket(
    State(state): State<AppState>,
    Path((room_name, subscriber_id)): Path<(String, String)>,
    axum::extract::Query(params): axum::extract::Query<HashMap<String, String>>,
    axum::extract::ConnectInfo(addr): axum::extract::ConnectInfo<SocketAddr>,
    ws: WebSocketUpgrade,
) -> AxumResponse {
    let stream_manager = match state.stream_manager.as_ref() {
        Some(sm) => sm.clone(),
        None => {
            return (
                axum::http::StatusCode::SERVICE_UNAVAILABLE,
                "Stream system disabled",
            )
                .into_response();
        }
    };

    let from_offset = params
        .get("from_offset")
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0);

    info!(
        "Stream WebSocket connection: room={}, subscriber={}, from_offset={}",
        room_name, subscriber_id, from_offset
    );

    let client_list_manager = state.client_list_manager.clone();
    let client_addr = addr.to_string();
    let client_id = format!("stream-{}-{}", room_name, subscriber_id);

    ws.on_upgrade(move |socket| {
        handle_stream_socket(
            socket,
            stream_manager,
            room_name,
            subscriber_id,
            from_offset,
            client_list_manager,
            client_id,
            client_addr,
        )
    })
}

/// Handle Event Stream WebSocket connection
#[allow(clippy::too_many_arguments)]
pub(super) async fn handle_stream_socket(
    socket: WebSocket,
    stream_manager: Arc<crate::core::StreamManager>,
    room_name: String,
    subscriber_id: String,
    mut current_offset: u64,
    client_list_manager: Arc<crate::monitoring::ClientListManager>,
    client_id: String,
    client_addr: String,
) {
    let connected_at = std::time::SystemTime::now();
    let client_info =
        crate::monitoring::ClientInfo::new(client_id.clone(), client_addr, connected_at);
    client_list_manager.add(client_info).await;

    let (mut ws_sender, mut ws_receiver) = socket.split();

    // Send welcome message
    let welcome = json!({
        "type": "connected",
        "room": room_name,
        "subscriber_id": subscriber_id,
        "from_offset": current_offset
    });

    if ws_sender
        .send(axum::extract::ws::Message::Text(welcome.to_string().into()))
        .await
        .is_err()
    {
        warn!(
            "Failed to send welcome to stream subscriber: {}",
            subscriber_id
        );
        return;
    }

    loop {
        tokio::select! {
            // Poll for new events (100ms interval)
            _ = tokio::time::sleep(tokio::time::Duration::from_millis(100)) => {
                match stream_manager.consume(&room_name, &subscriber_id, current_offset, 100).await {
                    Ok(events) => {
                        if !events.is_empty() {
                            for event in &events {
                                // Deserialize data from bytes to JSON
                                let data_json: serde_json::Value = serde_json::from_slice(&event.data)
                                    .unwrap_or(serde_json::Value::Null);

                                let event_json = json!({
                                    "type": "event",
                                    "offset": event.offset,
                                    "event": event.event,
                                    "data": data_json,
                                    "timestamp": event.timestamp
                                });

                                if ws_sender.send(axum::extract::ws::Message::Text(event_json.to_string().into())).await.is_err() {
                                    warn!("Failed to send event to subscriber: {}", subscriber_id);
                                    client_list_manager.remove(&client_id).await;
                                    return;
                                }
                            }

                            // Update offset to next expected event
                            current_offset = events.last().unwrap().offset + 1;
                        }
                    }
                    Err(e) => {
                        error!("Stream consume error: {}", e);
                        let _ = ws_sender.send(axum::extract::ws::Message::Text(
                            json!({"type": "error", "error": e}).to_string().into()
                        )).await;
                        break;
                    }
                }
            }

            // Handle incoming WebSocket messages (control messages)
            Some(msg) = ws_receiver.next() => {
                match msg {
                    Ok(axum::extract::ws::Message::Close(_)) => {
                        info!("Stream subscriber {} closed connection", subscriber_id);
                        break;
                    }
                    Ok(axum::extract::ws::Message::Ping(data)) => {
                        if ws_sender.send(axum::extract::ws::Message::Pong(data)).await.is_err() {
                            break;
                        }
                    }
                    Ok(_) => {}
                    Err(e) => {
                        warn!("WebSocket error for stream subscriber {}: {}", subscriber_id, e);
                        break;
                    }
                }
            }

            else => {
                break;
            }
        }
    }

    // Cleanup: remove client from tracking
    client_list_manager.remove(&client_id).await;
    info!(
        "Stream subscriber {} disconnected from room {}",
        subscriber_id, room_name
    );
}

// ============================================================================
// Pub/Sub WebSocket Handler
// ============================================================================

/// WebSocket handler for Pub/Sub subscriptions
/// GET /pubsub/ws?topics=topic1,topic2,*.wildcard
pub async fn pubsub_websocket(
    State(state): State<AppState>,
    ws: WebSocketUpgrade,
    axum::extract::Query(params): axum::extract::Query<HashMap<String, String>>,
    axum::extract::ConnectInfo(addr): axum::extract::ConnectInfo<SocketAddr>,
) -> AxumResponse {
    let pubsub_router = match state.pubsub_router.as_ref() {
        Some(router) => router.clone(),
        None => {
            return (
                axum::http::StatusCode::SERVICE_UNAVAILABLE,
                "Pub/Sub system disabled",
            )
                .into_response();
        }
    };

    // Parse topics from query params
    let topics_str = params.get("topics").cloned().unwrap_or_default();
    let topics: Vec<String> = topics_str
        .split(',')
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect();

    if topics.is_empty() {
        return (
            axum::http::StatusCode::BAD_REQUEST,
            "At least one topic required in query param: ?topics=topic1,topic2",
        )
            .into_response();
    }

    info!("WebSocket connection requested for topics: {:?}", topics);

    let client_list_manager = state.client_list_manager.clone();
    let client_addr = addr.to_string();

    ws.on_upgrade(move |socket| {
        handle_pubsub_socket(
            socket,
            pubsub_router,
            topics,
            client_list_manager,
            client_addr,
        )
    })
}

/// Handle individual WebSocket connection for Pub/Sub
pub(super) async fn handle_pubsub_socket(
    socket: WebSocket,
    pubsub_router: Arc<crate::core::PubSubRouter>,
    topics: Vec<String>,
    client_list_manager: Arc<crate::monitoring::ClientListManager>,
    client_addr: String,
) {
    let (mut ws_sender, mut ws_receiver) = socket.split();

    // Subscribe to topics
    let subscribe_result = match pubsub_router.subscribe(topics.clone()) {
        Ok(result) => result,
        Err(e) => {
            error!("Failed to subscribe: {}", e);
            let _ = ws_sender
                .send(axum::extract::ws::Message::Text(
                    json!({
                        "error": e.to_string()
                    })
                    .to_string()
                    .into(),
                ))
                .await;
            return;
        }
    };

    let subscriber_id = subscribe_result.subscriber_id.clone();
    let client_id = format!("pubsub-{}", subscriber_id);
    let connected_at = std::time::SystemTime::now();

    info!(
        "Subscriber {} connected to topics: {:?}",
        subscriber_id, topics
    );

    // Track client connection
    let client_info =
        crate::monitoring::ClientInfo::new(client_id.clone(), client_addr, connected_at);
    client_list_manager.add(client_info).await;

    // Create channel for receiving messages
    let (tx, mut rx) = mpsc::unbounded_channel::<Message>();

    // Register connection
    pubsub_router.register_connection(subscriber_id.clone(), tx);

    // Send welcome message in the loop (first iteration will handle it)
    let welcome_msg = json!({
        "type": "connected",
        "subscriber_id": subscriber_id,
        "topics": topics,
        "subscription_count": subscribe_result.subscription_count
    });

    // Send welcome message
    if ws_sender
        .send(axum::extract::ws::Message::Text(
            welcome_msg.to_string().into(),
        ))
        .await
        .is_err()
    {
        warn!(
            "Failed to send welcome message to subscriber: {}",
            subscriber_id
        );
        pubsub_router.unregister_connection(&subscriber_id);
        return;
    }

    // Process both incoming WebSocket messages and outgoing Pub/Sub messages
    loop {
        tokio::select! {
            // Receive messages from Pub/Sub channel
            Some(message) = rx.recv() => {
                let msg_json = serde_json::to_string(&json!({
                    "type": "message",
                    "message_id": message.id,
                    "topic": message.topic,
                    "payload": message.payload,
                    "metadata": message.metadata,
                    "timestamp": message.timestamp
                }))
                .unwrap();

                if ws_sender
                    .send(axum::extract::ws::Message::Text(msg_json.into()))
                    .await
                    .is_err()
                {
                    warn!("Failed to send message to subscriber: {}", subscriber_id);
                    break;
                }
            }

            // Handle incoming WebSocket messages (keepalive/pings)
            Some(msg) = ws_receiver.next() => {
                match msg {
                    Ok(axum::extract::ws::Message::Close(_)) => {
                        info!("Subscriber {} closed connection", subscriber_id);
                        break;
                    }
                    Ok(axum::extract::ws::Message::Ping(data)) => {
                        if ws_sender.send(axum::extract::ws::Message::Pong(data)).await.is_err() {
                            break;
                        }
                    }
                    Ok(_) => {
                        // Ignore other message types
                    }
                    Err(e) => {
                        warn!("WebSocket error for subscriber {}: {}", subscriber_id, e);
                        break;
                    }
                }
            }

            else => {
                // Both channels closed
                break;
            }
        }
    }

    // Cleanup
    client_list_manager.remove(&client_id).await;
    pubsub_router.unregister_connection(&subscriber_id);
    let _ = pubsub_router.unsubscribe(&subscriber_id, None);
    info!("Subscriber {} disconnected and cleaned up", subscriber_id);
}

// ============================================================================
// Pub/Sub REST API Handlers
// ============================================================================
