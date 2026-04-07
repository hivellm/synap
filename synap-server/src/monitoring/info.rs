//! INFO command implementation
//!
//! Redis-style INFO command with multiple sections

use crate::core::{HashStore, KVStore, ListStore, SetStore, SortedSetStore};
use serde::Serialize;
use std::str::FromStr;
use std::sync::Arc;

/// INFO command output sections
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InfoSection {
    All,
    Server,
    Memory,
    Stats,
    Replication,
    Cluster,
    Keyspace,
}

impl FromStr for InfoSection {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s.to_lowercase().as_str() {
            "server" => Self::Server,
            "memory" => Self::Memory,
            "stats" => Self::Stats,
            "replication" => Self::Replication,
            "cluster" => Self::Cluster,
            "keyspace" => Self::Keyspace,
            _ => Self::All,
        })
    }
}

impl InfoSection {
    /// Parse section from string (Redis-style, convenience method)
    #[allow(clippy::should_implement_trait)] // Kept for convenience, FromStr also exists
    pub fn from_str(s: &str) -> Self {
        <Self as FromStr>::from_str(s).unwrap_or(Self::All)
    }
}

/// Server information
#[derive(Debug, Serialize)]
pub struct ServerInfo {
    #[serde(rename = "redis_version")]
    pub version: String,
    #[serde(rename = "redis_git_sha1")]
    pub git_sha1: String,
    #[serde(rename = "redis_git_dirty")]
    pub git_dirty: String,
    #[serde(rename = "redis_build_id")]
    pub build_id: String,
    #[serde(rename = "redis_mode")]
    pub mode: String,
    #[serde(rename = "os")]
    pub os: String,
    #[serde(rename = "arch_bits")]
    pub arch_bits: String,
    #[serde(rename = "multiplexing_api")]
    pub multiplexing_api: String,
    #[serde(rename = "process_id")]
    pub process_id: u32,
    #[serde(rename = "run_id")]
    pub run_id: String,
    #[serde(rename = "tcp_port")]
    pub tcp_port: u16,
    #[serde(rename = "uptime_in_seconds")]
    pub uptime_in_seconds: u64,
    #[serde(rename = "uptime_in_days")]
    pub uptime_in_days: u64,
    #[serde(rename = "hz")]
    pub hz: u32,
    #[serde(rename = "executable")]
    pub executable: String,
    #[serde(rename = "config_file")]
    pub config_file: String,
}

impl ServerInfo {
    /// Create ServerInfo from current state
    pub async fn collect(uptime_secs: u64, port: u16) -> Self {
        use std::env;

        let process_id = std::process::id();
        let executable = env::current_exe()
            .ok()
            .and_then(|p| p.to_str().map(|s| s.to_string()))
            .unwrap_or_else(|| "synap-server".to_string());

        Self {
            version: env!("CARGO_PKG_VERSION").to_string(),
            git_sha1: String::new(), // Could use git_version crate
            git_dirty: "0".to_string(),
            build_id: format!("{:x}", process_id),
            mode: "standalone".to_string(),
            os: std::env::consts::OS.to_string(),
            arch_bits: if cfg!(target_pointer_width = "64") {
                "64"
            } else {
                "32"
            }
            .to_string(),
            multiplexing_api: "tokio".to_string(),
            process_id,
            run_id: format!("{:016x}", process_id),
            tcp_port: port,
            uptime_in_seconds: uptime_secs,
            uptime_in_days: uptime_secs / 86400,
            hz: 10, // Background task frequency
            executable,
            config_file: "config.yml".to_string(),
        }
    }
}

/// Memory information
#[derive(Debug, Serialize)]
pub struct MemoryInfo {
    #[serde(rename = "used_memory")]
    pub used_memory: usize,
    #[serde(rename = "used_memory_human")]
    pub used_memory_human: String,
    #[serde(rename = "used_memory_rss")]
    pub used_memory_rss: usize,
    #[serde(rename = "used_memory_peak")]
    pub used_memory_peak: usize,
    #[serde(rename = "used_memory_peak_human")]
    pub used_memory_peak_human: String,
    #[serde(rename = "mem_fragmentation_ratio")]
    pub mem_fragmentation_ratio: f64,
    #[serde(rename = "mem_allocator")]
    pub mem_allocator: String,
}

impl MemoryInfo {
    /// Collect memory information
    pub async fn collect(
        kv_store: Arc<KVStore>,
        hash_store: Arc<HashStore>,
        list_store: Arc<ListStore>,
        set_store: Arc<SetStore>,
        sorted_set_store: Arc<SortedSetStore>,
    ) -> Self {
        let kv_stats = kv_store.stats().await;
        let hash_stats = hash_store.stats();
        let list_stats = list_store.stats();
        let set_stats = set_store.stats();
        let sorted_set_stats = sorted_set_store.stats();

        let used_memory = kv_stats.total_memory_bytes
            + hash_stats.total_memory_bytes
            + (list_stats.total_elements * 64) // Estimate for lists
            + (set_stats.total_members * 48) // Estimate for sets
            + sorted_set_stats.memory_bytes;

        let used_memory_human = format_bytes(used_memory);

        // Get RSS memory (approximate)
        let used_memory_rss = if let Ok(mem_info) = sys_info::mem_info() {
            (mem_info.total - mem_info.avail) as usize * 1024
        } else {
            used_memory * 2 // Fallback: assume 2x overhead
        };

        // Peak memory (for now, same as current)
        let used_memory_peak = used_memory_rss;
        let used_memory_peak_human = format_bytes(used_memory_peak);

        // Fragmentation ratio
        let mem_fragmentation_ratio = if used_memory > 0 {
            used_memory_rss as f64 / used_memory as f64
        } else {
            1.0
        };

        Self {
            used_memory,
            used_memory_human,
            used_memory_rss,
            used_memory_peak,
            used_memory_peak_human,
            mem_fragmentation_ratio,
            mem_allocator: "jemalloc".to_string(), // Rust default
        }
    }
}

/// Statistics information
#[derive(Debug, Serialize)]
pub struct StatsInfo {
    #[serde(rename = "total_commands_processed")]
    pub total_commands_processed: u64,
    #[serde(rename = "instantaneous_ops_per_sec")]
    pub instantaneous_ops_per_sec: u64,
    #[serde(rename = "total_connections_received")]
    pub total_connections_received: u64,
    #[serde(rename = "keyspace_hits")]
    pub keyspace_hits: u64,
    #[serde(rename = "keyspace_misses")]
    pub keyspace_misses: u64,
    #[serde(rename = "pubsub_channels")]
    pub pubsub_channels: usize,
    #[serde(rename = "pubsub_patterns")]
    pub pubsub_patterns: usize,
}

impl StatsInfo {
    /// Collect statistics
    pub async fn collect(
        kv_store: Arc<KVStore>,
        _hash_store: Arc<HashStore>,
        _list_store: Arc<ListStore>,
        _set_store: Arc<SetStore>,
        _sorted_set_store: Arc<SortedSetStore>,
    ) -> Self {
        let kv_stats = kv_store.stats().await;

        // Sum all operations
        let total_commands_processed = kv_stats.gets + kv_stats.sets + kv_stats.dels;

        Self {
            total_commands_processed,
            instantaneous_ops_per_sec: 0, // Would need to track over time
            total_connections_received: 0, // Would need connection counter
            keyspace_hits: kv_stats.hits,
            keyspace_misses: kv_stats.misses,
            pubsub_channels: 0, // Would need pubsub stats
            pubsub_patterns: 0,
        }
    }
}

/// Replication information
#[derive(Debug, Serialize)]
pub struct ReplicationInfo {
    #[serde(rename = "role")]
    pub role: String,
    #[serde(rename = "connected_slaves")]
    pub connected_slaves: u32,
    #[serde(rename = "master_repl_offset")]
    pub master_repl_offset: u64,
    #[serde(rename = "repl_backlog_active")]
    pub repl_backlog_active: u32,
    #[serde(rename = "repl_backlog_size")]
    pub repl_backlog_size: usize,
}

impl ReplicationInfo {
    /// Collect replication information (horizontally scaled in future)
    pub async fn collect() -> Self {
        Self {
            role: "master".to_string(),
            connected_slaves: 0,
            master_repl_offset: 0,
            repl_backlog_active: 0,
            repl_backlog_size: 0,
        }
    }
}

/// Keyspace information
#[derive(Debug, Serialize)]
pub struct KeyspaceInfo {
    pub db0: String,
}

impl KeyspaceInfo {
    /// Collect keyspace information
    pub async fn collect(
        kv_store: Arc<KVStore>,
        hash_store: Arc<HashStore>,
        list_store: Arc<ListStore>,
        set_store: Arc<SetStore>,
        sorted_set_store: Arc<SortedSetStore>,
    ) -> Self {
        let kv_stats = kv_store.stats().await;
        let hash_stats = hash_store.stats();
        let list_stats = list_store.stats();
        let set_stats = set_store.stats();
        let sorted_set_stats = sorted_set_store.stats();

        // Format: keys=total,expires=expired_keys,avg_ttl=avg_ttl
        let db0 = format!(
            "keys={},expires=0,avg_ttl=0",
            kv_stats.total_keys
                + hash_stats.total_hashes
                + list_stats.total_lists
                + set_stats.total_sets
                + sorted_set_stats.total_keys
        );

        Self { db0 }
    }
}

/// Format bytes to human-readable format
fn format_bytes(bytes: usize) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut unit_idx = 0;

    while size >= 1024.0 && unit_idx < UNITS.len() - 1 {
        size /= 1024.0;
        unit_idx += 1;
    }

    if size.fract() < 0.01 {
        format!("{:.0}{}", size, UNITS[unit_idx])
    } else {
        format!("{:.2}{}", size, UNITS[unit_idx])
    }
}
