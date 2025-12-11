//! Common types for Synap SDK

use serde::{Deserialize, Serialize};

/// Queue message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: String,
    pub payload: Vec<u8>,
    pub priority: u8,
    pub retry_count: u32,
    pub max_retries: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deadline: Option<u64>,
}

/// Queue statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueStats {
    pub depth: usize,
    pub pending: usize,
    pub max_depth: usize,
    pub total_published: u64,
    pub total_consumed: u64,
    pub total_acked: u64,
    pub total_nacked: u64,
    pub dlq_count: usize,
}

/// Stream event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub offset: u64,
    #[serde(rename = "event_type")]
    pub event: String,
    pub data: serde_json::Value,
    pub timestamp: Option<u64>,
}

/// Stream statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamStats {
    pub room: String,
    pub max_offset: u64,
    pub total_events: u64,
    pub created_at: u64,
    pub last_activity: u64,
}

/// Pub/Sub message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PubSubMessage {
    pub topic: String,
    pub data: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headers: Option<std::collections::HashMap<String, String>>,
}

/// KV Store statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KVStats {
    pub total_keys: usize,
    pub total_memory_bytes: usize,
    pub hit_rate: f64,
}

/// HyperLogLog statistics
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HyperLogLogStats {
    pub total_hlls: u64,
    pub pfadd_count: u64,
    pub pfcount_count: u64,
    pub pfmerge_count: u64,
    pub total_cardinality: u64,
}
