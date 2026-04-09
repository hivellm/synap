//! Common types for Synap SDK

use serde::{Deserialize, Deserializer, Serialize};

/// Deserialize a value that may arrive as either a number or a string
/// (RESP3 returns all scalars as strings).
fn from_str_or_num<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
{
    use serde::de::Error;
    let value = serde_json::Value::deserialize(deserializer)?;
    match &value {
        serde_json::Value::Number(n) => {
            let s = n.to_string();
            s.parse::<T>().map_err(Error::custom)
        }
        serde_json::Value::String(s) => s.parse::<T>().map_err(Error::custom),
        other => Err(Error::custom(format!(
            "expected number or string, got {other}"
        ))),
    }
}

/// Queue message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: String,
    pub payload: Vec<u8>,
    #[serde(default)]
    pub priority: u8,
    #[serde(default)]
    pub retry_count: u32,
    #[serde(default)]
    pub max_retries: u32,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub deadline: Option<u64>,
}

/// Queue statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueStats {
    #[serde(deserialize_with = "from_str_or_num")]
    pub depth: usize,
    #[serde(deserialize_with = "from_str_or_num")]
    pub consumers: usize,
    #[serde(deserialize_with = "from_str_or_num")]
    pub published: u64,
    #[serde(deserialize_with = "from_str_or_num")]
    pub consumed: u64,
    #[serde(deserialize_with = "from_str_or_num")]
    pub acked: u64,
    #[serde(deserialize_with = "from_str_or_num")]
    pub nacked: u64,
    #[serde(deserialize_with = "from_str_or_num")]
    pub dead_lettered: usize,
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
