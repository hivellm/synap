use super::*;

// =====================================
// Partitioned Stream Handlers (Kafka-style)
// =====================================

#[derive(Debug, Deserialize)]
pub struct CreateTopicRequest {
    pub num_partitions: Option<usize>,
    pub replication_factor: Option<usize>,
    pub retention_policy: Option<RetentionPolicyRequest>,
    pub segment_bytes: Option<u64>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum RetentionPolicyRequest {
    Time {
        retention_secs: u64,
    },
    Size {
        max_bytes: u64,
    },
    Messages {
        max_messages: u64,
    },
    Combined {
        retention_secs: Option<u64>,
        max_bytes: Option<u64>,
        max_messages: Option<u64>,
    },
    Infinite,
}

impl From<RetentionPolicyRequest> for crate::core::RetentionPolicy {
    fn from(req: RetentionPolicyRequest) -> Self {
        match req {
            RetentionPolicyRequest::Time { retention_secs } => {
                crate::core::RetentionPolicy::Time { retention_secs }
            }
            RetentionPolicyRequest::Size { max_bytes } => {
                crate::core::RetentionPolicy::Size { max_bytes }
            }
            RetentionPolicyRequest::Messages { max_messages } => {
                crate::core::RetentionPolicy::Messages { max_messages }
            }
            RetentionPolicyRequest::Combined {
                retention_secs,
                max_bytes,
                max_messages,
            } => crate::core::RetentionPolicy::Combined {
                retention_secs,
                max_bytes,
                max_messages,
            },
            RetentionPolicyRequest::Infinite => crate::core::RetentionPolicy::Infinite,
        }
    }
}

pub async fn create_partitioned_topic(
    State(state): State<AppState>,
    Path(topic): Path<String>,
    Json(req): Json<CreateTopicRequest>,
) -> Result<Json<serde_json::Value>, SynapError> {
    let partition_manager = state
        .partition_manager
        .as_ref()
        .ok_or_else(|| SynapError::InvalidRequest("Partition system disabled".to_string()))?;

    let mut config = crate::core::PartitionConfig::default();
    if let Some(num) = req.num_partitions {
        config.num_partitions = num;
    }
    if let Some(rep) = req.replication_factor {
        config.replication_factor = rep;
    }
    if let Some(retention) = req.retention_policy {
        config.retention = retention.into();
    }
    if let Some(seg) = req.segment_bytes {
        config.segment_bytes = seg;
    }

    partition_manager
        .create_topic(&topic, Some(config.clone()))
        .await
        .map_err(SynapError::InvalidRequest)?;

    Ok(Json(json!({
        "success": true,
        "topic": topic,
        "num_partitions": config.num_partitions,
        "replication_factor": config.replication_factor
    })))
}

#[derive(Debug, Deserialize)]
pub struct PartitionPublishRequest {
    pub event_type: String,
    pub key: Option<String>,
    pub data: serde_json::Value,
}

/// Publish to partitioned topic
pub async fn publish_to_partition(
    State(state): State<AppState>,
    Path(topic): Path<String>,
    Json(req): Json<PartitionPublishRequest>,
) -> Result<Json<serde_json::Value>, SynapError> {
    let partition_manager = state
        .partition_manager
        .as_ref()
        .ok_or_else(|| SynapError::InvalidRequest("Partition system disabled".to_string()))?;

    let data =
        serde_json::to_vec(&req.data).map_err(|e| SynapError::SerializationError(e.to_string()))?;

    let key = req.key.map(|k| k.into_bytes());

    let (partition_id, offset) = partition_manager
        .publish(&topic, &req.event_type, key, data)
        .await
        .map_err(SynapError::InvalidRequest)?;

    Ok(Json(json!({
        "partition_id": partition_id,
        "offset": offset,
        "topic": topic
    })))
}

#[derive(Debug, Deserialize)]
pub struct ConsumePartitionRequest {
    pub from_offset: Option<u64>,
    pub limit: Option<usize>,
}

/// Consume from specific partition
pub async fn consume_from_partition(
    State(state): State<AppState>,
    Path((topic, partition_id)): Path<(String, usize)>,
    Json(req): Json<ConsumePartitionRequest>,
) -> Result<Json<serde_json::Value>, SynapError> {
    let partition_manager = state
        .partition_manager
        .as_ref()
        .ok_or_else(|| SynapError::InvalidRequest("Partition system disabled".to_string()))?;

    let from_offset = req.from_offset.unwrap_or(0);
    let limit = req.limit.unwrap_or(100).min(1000);

    let events = partition_manager
        .consume_partition(&topic, partition_id, from_offset, limit)
        .await
        .map_err(SynapError::InvalidRequest)?;

    let next_offset = events.last().map(|e| e.offset + 1).unwrap_or(from_offset);

    Ok(Json(json!({
        "topic": topic,
        "partition_id": partition_id,
        "events": events,
        "next_offset": next_offset,
        "count": events.len()
    })))
}

/// Get topic stats
pub async fn get_topic_stats(
    State(state): State<AppState>,
    Path(topic): Path<String>,
) -> Result<Json<serde_json::Value>, SynapError> {
    let partition_manager = state
        .partition_manager
        .as_ref()
        .ok_or_else(|| SynapError::InvalidRequest("Partition system disabled".to_string()))?;

    let stats = partition_manager
        .topic_stats(&topic)
        .await
        .map_err(SynapError::InvalidRequest)?;

    Ok(Json(json!({
        "topic": topic,
        "partitions": stats
    })))
}

/// List all topics
pub async fn list_topics(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, SynapError> {
    let partition_manager = state
        .partition_manager
        .as_ref()
        .ok_or_else(|| SynapError::InvalidRequest("Partition system disabled".to_string()))?;

    let topics = partition_manager.list_topics().await;

    Ok(Json(json!({
        "topics": topics,
        "count": topics.len()
    })))
}

/// Delete topic
pub async fn delete_topic(
    State(state): State<AppState>,
    Path(topic): Path<String>,
) -> Result<Json<serde_json::Value>, SynapError> {
    let partition_manager = state
        .partition_manager
        .as_ref()
        .ok_or_else(|| SynapError::InvalidRequest("Partition system disabled".to_string()))?;

    partition_manager
        .delete_topic(&topic)
        .await
        .map_err(SynapError::InvalidRequest)?;

    Ok(Json(json!({
        "success": true,
        "deleted": topic
    })))
}

// =====================================
// Consumer Group Handlers
// =====================================

#[derive(Debug, Deserialize)]
pub struct CreateConsumerGroupRequest {
    pub topic: String,
    pub partition_count: usize,
    pub strategy: Option<String>,
    pub session_timeout_secs: Option<u64>,
}

/// Create consumer group
pub async fn create_consumer_group(
    State(state): State<AppState>,
    Path(group_id): Path<String>,
    Json(req): Json<CreateConsumerGroupRequest>,
) -> Result<Json<serde_json::Value>, SynapError> {
    let consumer_group_manager = state
        .consumer_group_manager
        .as_ref()
        .ok_or_else(|| SynapError::InvalidRequest("Consumer group system disabled".to_string()))?;

    let mut config = crate::core::ConsumerGroupConfig::default();

    if let Some(strategy) = req.strategy {
        config.strategy = match strategy.as_str() {
            "round_robin" => crate::core::AssignmentStrategy::RoundRobin,
            "range" => crate::core::AssignmentStrategy::Range,
            "sticky" => crate::core::AssignmentStrategy::Sticky,
            _ => {
                return Err(SynapError::InvalidRequest(
                    "Invalid assignment strategy".to_string(),
                ));
            }
        };
    }

    if let Some(timeout) = req.session_timeout_secs {
        config.session_timeout_secs = timeout;
    }

    consumer_group_manager
        .create_group(&group_id, &req.topic, req.partition_count, Some(config))
        .await
        .map_err(SynapError::InvalidRequest)?;

    Ok(Json(json!({
        "success": true,
        "group_id": group_id,
        "topic": req.topic
    })))
}

#[derive(Debug, Deserialize)]
pub struct JoinGroupRequest {
    pub session_timeout_secs: Option<u64>,
}

/// Join consumer group
pub async fn join_consumer_group(
    State(state): State<AppState>,
    Path(group_id): Path<String>,
    Json(req): Json<JoinGroupRequest>,
) -> Result<Json<serde_json::Value>, SynapError> {
    let consumer_group_manager = state
        .consumer_group_manager
        .as_ref()
        .ok_or_else(|| SynapError::InvalidRequest("Consumer group system disabled".to_string()))?;

    let timeout = req.session_timeout_secs.unwrap_or(30);

    let member = consumer_group_manager
        .join_group(&group_id, timeout)
        .await
        .map_err(SynapError::InvalidRequest)?;

    // Trigger rebalance
    let _ = consumer_group_manager.rebalance_group(&group_id).await;

    Ok(Json(json!({
        "member_id": member.id,
        "group_id": member.group_id
    })))
}

/// Leave consumer group
pub async fn leave_consumer_group(
    State(state): State<AppState>,
    Path((group_id, member_id)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>, SynapError> {
    let consumer_group_manager = state
        .consumer_group_manager
        .as_ref()
        .ok_or_else(|| SynapError::InvalidRequest("Consumer group system disabled".to_string()))?;

    consumer_group_manager
        .leave_group(&group_id, &member_id)
        .await
        .map_err(SynapError::InvalidRequest)?;

    // Trigger rebalance
    let _ = consumer_group_manager.rebalance_group(&group_id).await;

    Ok(Json(json!({
        "success": true,
        "member_id": member_id
    })))
}

/// Get partition assignment
pub async fn get_partition_assignment(
    State(state): State<AppState>,
    Path((group_id, member_id)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>, SynapError> {
    let consumer_group_manager = state
        .consumer_group_manager
        .as_ref()
        .ok_or_else(|| SynapError::InvalidRequest("Consumer group system disabled".to_string()))?;

    let assignment = consumer_group_manager
        .get_assignment(&group_id, &member_id)
        .await
        .map_err(SynapError::InvalidRequest)?;

    Ok(Json(json!({
        "member_id": member_id,
        "group_id": group_id,
        "partitions": assignment
    })))
}

#[derive(Debug, Deserialize)]
pub struct CommitOffsetRequest {
    pub partition_id: usize,
    pub offset: u64,
}

/// Commit offset
pub async fn commit_offset(
    State(state): State<AppState>,
    Path(group_id): Path<String>,
    Json(req): Json<CommitOffsetRequest>,
) -> Result<Json<serde_json::Value>, SynapError> {
    let consumer_group_manager = state
        .consumer_group_manager
        .as_ref()
        .ok_or_else(|| SynapError::InvalidRequest("Consumer group system disabled".to_string()))?;

    consumer_group_manager
        .commit_offset(&group_id, req.partition_id, req.offset)
        .await
        .map_err(SynapError::InvalidRequest)?;

    Ok(Json(json!({
        "success": true,
        "partition_id": req.partition_id,
        "offset": req.offset
    })))
}

/// Get committed offset
pub async fn get_committed_offset(
    State(state): State<AppState>,
    Path((group_id, partition_id)): Path<(String, usize)>,
) -> Result<Json<serde_json::Value>, SynapError> {
    let consumer_group_manager = state
        .consumer_group_manager
        .as_ref()
        .ok_or_else(|| SynapError::InvalidRequest("Consumer group system disabled".to_string()))?;

    let offset = consumer_group_manager
        .get_offset(&group_id, partition_id)
        .await
        .map_err(SynapError::InvalidRequest)?;

    Ok(Json(json!({
        "group_id": group_id,
        "partition_id": partition_id,
        "offset": offset
    })))
}

/// Get consumer group stats
pub async fn get_consumer_group_stats(
    State(state): State<AppState>,
    Path(group_id): Path<String>,
) -> Result<Json<serde_json::Value>, SynapError> {
    let consumer_group_manager = state
        .consumer_group_manager
        .as_ref()
        .ok_or_else(|| SynapError::InvalidRequest("Consumer group system disabled".to_string()))?;

    let stats = consumer_group_manager
        .group_stats(&group_id)
        .await
        .map_err(SynapError::InvalidRequest)?;

    Ok(Json(serde_json::to_value(stats).map_err(|e| {
        SynapError::SerializationError(e.to_string())
    })?))
}

/// List consumer groups
pub async fn list_consumer_groups(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, SynapError> {
    let consumer_group_manager = state
        .consumer_group_manager
        .as_ref()
        .ok_or_else(|| SynapError::InvalidRequest("Consumer group system disabled".to_string()))?;

    let groups = consumer_group_manager.list_groups().await;

    Ok(Json(json!({
        "groups": groups,
        "count": groups.len()
    })))
}

/// Heartbeat from consumer
pub async fn consumer_heartbeat(
    State(state): State<AppState>,
    Path((group_id, member_id)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>, SynapError> {
    let consumer_group_manager = state
        .consumer_group_manager
        .as_ref()
        .ok_or_else(|| SynapError::InvalidRequest("Consumer group system disabled".to_string()))?;

    consumer_group_manager
        .heartbeat(&group_id, &member_id)
        .await
        .map_err(SynapError::InvalidRequest)?;

    Ok(Json(json!({
        "success": true
    })))
}
