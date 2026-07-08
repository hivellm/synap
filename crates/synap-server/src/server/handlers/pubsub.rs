use super::*;

// Pub/Sub REST API types
#[derive(Debug, Deserialize)]
pub struct SubscribeRequest {
    pub topics: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct PublishMessageRequest {
    #[serde(alias = "data")]
    pub payload: serde_json::Value,
    pub metadata: Option<HashMap<String, String>>,
}

#[derive(Debug, Deserialize)]
pub struct UnsubscribeRequest {
    pub subscriber_id: String,
    pub topics: Option<Vec<String>>,
}

/// POST /pubsub/subscribe - Subscribe to topics
pub async fn pubsub_subscribe(
    State(state): State<AppState>,
    AuthContextExtractor(ctx): AuthContextExtractor,

    crate::hub::HubContextExtractor(hub_ctx): crate::hub::HubContextExtractor,
    Json(req): Json<SubscribeRequest>,
) -> Result<Json<serde_json::Value>, Json<serde_json::Value>> {
    debug!("POST /pubsub/subscribe - topics: {:?}", req.topics);

    // Check permission for each topic
    for topic in &req.topics {
        if require_permission(&ctx, &format!("pubsub:{}", topic), Action::Read).is_err() {
            return Err(Json(serde_json::json!({
                "error": format!("Insufficient permissions for topic: {}", topic)
            })));
        }
    }

    let pubsub_router = state.pubsub_router.as_ref().ok_or_else(|| {
        Json(serde_json::json!({
            "error": "Pub/Sub system disabled"
        }))
    })?;

    // Apply multi-tenant scoping if Hub mode is active

    let scoped_topics: Vec<String> = req
        .topics
        .iter()
        .map(|topic| {
            crate::hub::MultiTenant::scope_topic(hub_ctx.as_ref().map(|c| c.user_id()), topic)
        })
        .collect();

    match pubsub_router.subscribe(scoped_topics) {
        Ok(result) => {
            // Unscope topics in response

            let response_topics = crate::hub::MultiTenant::unscope_names(result.topics);

            Ok(Json(serde_json::json!({
                "subscriber_id": result.subscriber_id,
                "topics": response_topics,
                "subscription_count": result.subscription_count,
            })))
        }
        Err(e) => {
            error!("Subscribe error: {}", e);
            Err(Json(serde_json::json!({
                "error": e.to_string()
            })))
        }
    }
}

/// POST /pubsub/:topic/publish - Publish message to topic
pub async fn pubsub_publish(
    State(state): State<AppState>,
    AuthContextExtractor(ctx): AuthContextExtractor,

    crate::hub::HubContextExtractor(hub_ctx): crate::hub::HubContextExtractor,
    Path(topic): Path<String>,
    Json(req): Json<PublishMessageRequest>,
) -> Result<Json<serde_json::Value>, Json<serde_json::Value>> {
    debug!("POST /pubsub/{}/publish", topic);

    // Check permission
    if require_permission(&ctx, &format!("pubsub:{}", topic), Action::Write).is_err() {
        return Err(Json(serde_json::json!({
            "error": format!("Insufficient permissions for topic: {}", topic)
        })));
    }

    let pubsub_router = state.pubsub_router.as_ref().ok_or_else(|| {
        Json(serde_json::json!({
            "error": "Pub/Sub system disabled"
        }))
    })?;

    // Apply multi-tenant scoping if Hub mode is active

    let scoped_topic =
        crate::hub::MultiTenant::scope_topic(hub_ctx.as_ref().map(|c| c.user_id()), &topic);

    match pubsub_router.publish(&scoped_topic, req.payload, req.metadata) {
        Ok(result) => {
            // Unscope topic in response

            let response_topic = crate::hub::MultiTenant::parse_scoped_name(&result.topic)
                .map(|(_, name)| name)
                .unwrap_or(result.topic.clone());

            Ok(Json(serde_json::json!({
                "message_id": result.message_id,
                "topic": response_topic,
                "subscribers_matched": result.subscribers_matched,
            })))
        }
        Err(e) => {
            error!("Publish error: {}", e);
            Err(Json(serde_json::json!({
                "error": e.to_string()
            })))
        }
    }
}

/// POST /pubsub/unsubscribe - Unsubscribe from topics
pub async fn pubsub_unsubscribe(
    State(state): State<AppState>,

    crate::hub::HubContextExtractor(hub_ctx): crate::hub::HubContextExtractor,
    Json(req): Json<UnsubscribeRequest>,
) -> Result<Json<serde_json::Value>, Json<serde_json::Value>> {
    debug!(
        "POST /pubsub/unsubscribe - subscriber_id: {}",
        req.subscriber_id
    );

    let pubsub_router = state.pubsub_router.as_ref().ok_or_else(|| {
        Json(serde_json::json!({
            "error": "Pub/Sub system disabled"
        }))
    })?;

    // Apply multi-tenant scoping if Hub mode is active and topics are provided

    let scoped_topics = req.topics.map(|topics| {
        topics
            .iter()
            .map(|topic| {
                crate::hub::MultiTenant::scope_topic(hub_ctx.as_ref().map(|c| c.user_id()), topic)
            })
            .collect()
    });

    match pubsub_router.unsubscribe(&req.subscriber_id, scoped_topics) {
        Ok(count) => Ok(Json(serde_json::json!({
            "unsubscribed": count
        }))),
        Err(e) => {
            error!("Unsubscribe error: {}", e);
            Err(Json(serde_json::json!({
                "error": e.to_string()
            })))
        }
    }
}

/// GET /pubsub/stats - Get Pub/Sub statistics
pub async fn pubsub_stats(
    State(state): State<AppState>,
    AuthContextExtractor(ctx): AuthContextExtractor,
) -> Result<Json<serde_json::Value>, Json<serde_json::Value>> {
    debug!("GET /pubsub/stats");

    // Check permission (read access to any pubsub topic)
    if require_permission(&ctx, "pubsub:*", Action::Read).is_err() {
        return Err(Json(serde_json::json!({
            "error": "Insufficient permissions"
        })));
    }

    let pubsub_router = state.pubsub_router.as_ref().ok_or_else(|| {
        Json(serde_json::json!({
            "error": "Pub/Sub system disabled"
        }))
    })?;

    let stats = pubsub_router.get_stats();
    Ok(Json(serde_json::to_value(stats).unwrap()))
}

/// GET /pubsub/topics - List all topics
pub async fn pubsub_list_topics(
    State(state): State<AppState>,
    AuthContextExtractor(ctx): AuthContextExtractor,

    crate::hub::HubContextExtractor(hub_ctx): crate::hub::HubContextExtractor,
) -> Result<Json<serde_json::Value>, Json<serde_json::Value>> {
    debug!("GET /pubsub/topics");

    // Check permission (read access to any pubsub topic)
    if require_permission(&ctx, "pubsub:*", Action::Read).is_err() {
        return Err(Json(serde_json::json!({
            "error": "Insufficient permissions"
        })));
    }

    let pubsub_router = state.pubsub_router.as_ref().ok_or_else(|| {
        Json(serde_json::json!({
            "error": "Pub/Sub system disabled"
        }))
    })?;

    let all_topics = pubsub_router.list_topics();

    // Filter topics by user in Hub mode

    let topics =
        crate::hub::MultiTenant::unscope_names(crate::hub::MultiTenant::filter_user_resources(
            all_topics,
            hub_ctx.as_ref().map(|c| c.user_id()),
        ));

    Ok(Json(serde_json::json!({
        "topics": topics,
        "count": topics.len()
    })))
}

/// GET /pubsub/:topic/info - Get topic information
pub async fn pubsub_topic_info(
    State(state): State<AppState>,
    AuthContextExtractor(ctx): AuthContextExtractor,

    crate::hub::HubContextExtractor(hub_ctx): crate::hub::HubContextExtractor,
    Path(topic): Path<String>,
) -> Result<Json<serde_json::Value>, Json<serde_json::Value>> {
    debug!("GET /pubsub/{}/info", topic);

    // Check permission
    if require_permission(&ctx, &format!("pubsub:{}", topic), Action::Read).is_err() {
        return Err(Json(serde_json::json!({
            "error": format!("Insufficient permissions for topic: {}", topic)
        })));
    }

    let pubsub_router = state.pubsub_router.as_ref().ok_or_else(|| {
        Json(serde_json::json!({
            "error": "Pub/Sub system disabled"
        }))
    })?;

    // Apply multi-tenant scoping if Hub mode is active

    let scoped_topic =
        crate::hub::MultiTenant::scope_topic(hub_ctx.as_ref().map(|c| c.user_id()), &topic);

    match pubsub_router.get_topic_info(&scoped_topic) {
        Some(info) => Ok(Json(serde_json::to_value(info).unwrap())),
        None => Err(Json(serde_json::json!({
            "error": "Topic not found"
        }))),
    }
}

// ============================================================================
// Pub/Sub StreamableHTTP Command Handlers
// ============================================================================

pub(super) async fn handle_pubsub_subscribe_cmd(
    state: &AppState,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let pubsub_router = state
        .pubsub_router
        .as_ref()
        .ok_or_else(|| SynapError::InvalidRequest("Pub/Sub system disabled".to_string()))?;

    let topics_val = request
        .payload
        .get("topics")
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'topics' field".to_string()))?;

    let topics_arr = topics_val
        .as_array()
        .ok_or_else(|| SynapError::InvalidRequest("'topics' must be an array".to_string()))?;

    let topics: Vec<String> = topics_arr
        .iter()
        .filter_map(|v| v.as_str().map(|s| s.to_string()))
        .collect();

    if topics.is_empty() {
        return Err(SynapError::InvalidRequest(
            "At least one topic required".to_string(),
        ));
    }

    let result = pubsub_router.subscribe(topics)?;

    Ok(serde_json::json!({
        "subscriber_id": result.subscriber_id,
        "topics": result.topics,
        "subscription_count": result.subscription_count
    }))
}

pub(super) async fn handle_pubsub_publish_cmd(
    state: &AppState,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let pubsub_router = state
        .pubsub_router
        .as_ref()
        .ok_or_else(|| SynapError::InvalidRequest("Pub/Sub system disabled".to_string()))?;

    let topic = request
        .payload
        .get("topic")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'topic' field".to_string()))?;

    let payload = request
        .payload
        .get("payload")
        .or_else(|| request.payload.get("data"))
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'payload' or 'data' field".to_string()))?
        .clone();

    let metadata = request.payload.get("metadata").and_then(|v| {
        if let serde_json::Value::Object(map) = v {
            Some(
                map.iter()
                    .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                    .collect(),
            )
        } else {
            None
        }
    });

    let result = pubsub_router.publish(topic, payload, metadata)?;

    Ok(serde_json::json!({
        "message_id": result.message_id,
        "topic": result.topic,
        "subscribers_matched": result.subscribers_matched
    }))
}

pub(super) async fn handle_pubsub_unsubscribe_cmd(
    state: &AppState,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let pubsub_router = state
        .pubsub_router
        .as_ref()
        .ok_or_else(|| SynapError::InvalidRequest("Pub/Sub system disabled".to_string()))?;

    let subscriber_id = request
        .payload
        .get("subscriber_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'subscriber_id' field".to_string()))?;

    let topics = request.payload.get("topics").and_then(|v| {
        v.as_array().map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
    });

    let count = pubsub_router.unsubscribe(subscriber_id, topics)?;

    Ok(serde_json::json!({ "unsubscribed": count }))
}

pub(super) async fn handle_pubsub_stats_cmd(
    state: &AppState,
    _request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let pubsub_router = state
        .pubsub_router
        .as_ref()
        .ok_or_else(|| SynapError::InvalidRequest("Pub/Sub system disabled".to_string()))?;

    let stats = pubsub_router.get_stats();
    serde_json::to_value(stats).map_err(|e| SynapError::SerializationError(e.to_string()))
}

pub(super) async fn handle_pubsub_topics_cmd(
    state: &AppState,
    _request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let pubsub_router = state
        .pubsub_router
        .as_ref()
        .ok_or_else(|| SynapError::InvalidRequest("Pub/Sub system disabled".to_string()))?;

    let topics = pubsub_router.list_topics();
    Ok(serde_json::json!({
        "topics": topics,
        "count": topics.len()
    }))
}

pub(super) async fn handle_pubsub_info_cmd(
    state: &AppState,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let pubsub_router = state
        .pubsub_router
        .as_ref()
        .ok_or_else(|| SynapError::InvalidRequest("Pub/Sub system disabled".to_string()))?;

    let topic = request
        .payload
        .get("topic")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'topic' field".to_string()))?;

    match pubsub_router.get_topic_info(topic) {
        Some(info) => Ok(serde_json::to_value(info)
            .map_err(|e| SynapError::SerializationError(e.to_string()))?),
        None => Err(SynapError::InvalidRequest(format!(
            "Topic '{}' not found",
            topic
        ))),
    }
}
