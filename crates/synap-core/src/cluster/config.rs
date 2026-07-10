use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::time::Duration;

/// Cluster configuration.
///
/// Every field carries a serde default so a partial or legacy `cluster:` block in
/// config.yml still deserializes (issue #232) rather than failing to load.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterConfig {
    /// Enable cluster mode
    #[serde(default)]
    pub enabled: bool,

    /// This node's ID (auto-generated if not set)
    #[serde(default)]
    pub node_id: Option<String>,

    /// This node's address
    #[serde(default = "default_node_address")]
    pub node_address: SocketAddr,

    /// Cluster seed nodes (for discovery)
    #[serde(default, alias = "seeds")]
    pub seed_nodes: Vec<SocketAddr>,

    /// Cluster communication port
    #[serde(default = "default_cluster_port")]
    pub cluster_port: u16,

    /// Node timeout (milliseconds)
    #[serde(default = "default_node_timeout_ms")]
    pub node_timeout_ms: u64,

    /// Cluster require full coverage
    /// If true, cluster will not accept writes if < 16384 slots are covered
    #[serde(default = "default_true")]
    pub require_full_coverage: bool,

    /// Migration batch size (keys per batch)
    #[serde(default = "default_migration_batch_size")]
    pub migration_batch_size: usize,

    /// Migration timeout (seconds)
    #[serde(default = "default_migration_timeout_secs")]
    pub migration_timeout_secs: u64,

    /// Raft election timeout (milliseconds)
    #[serde(default = "default_raft_election_timeout_ms")]
    pub raft_election_timeout_ms: u64,

    /// Raft heartbeat interval (milliseconds)
    #[serde(default = "default_raft_heartbeat_interval_ms")]
    pub raft_heartbeat_interval_ms: u64,
}

fn default_node_address() -> SocketAddr {
    "127.0.0.1:15502"
        .parse()
        .expect("hardcoded default socket address is valid")
}
fn default_cluster_port() -> u16 {
    15502
}
fn default_node_timeout_ms() -> u64 {
    5000
}
fn default_true() -> bool {
    true
}
fn default_migration_batch_size() -> usize {
    100
}
fn default_migration_timeout_secs() -> u64 {
    60
}
fn default_raft_election_timeout_ms() -> u64 {
    1000
}
fn default_raft_heartbeat_interval_ms() -> u64 {
    100
}

impl Default for ClusterConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            node_id: None,
            node_address: "127.0.0.1:15502"
                .parse()
                .expect("hardcoded default socket address is valid"),
            seed_nodes: Vec::new(),
            cluster_port: 15502,
            node_timeout_ms: 5000,
            require_full_coverage: true,
            migration_batch_size: 100,
            migration_timeout_secs: 60,
            raft_election_timeout_ms: 1000,
            raft_heartbeat_interval_ms: 100,
        }
    }
}

impl ClusterConfig {
    /// Load cluster config from environment variables (issue #233), overlaying a
    /// default. Recognized vars (all optional):
    /// - `SYNAP_CLUSTER_ENABLED` (bool)
    /// - `SYNAP_CLUSTER_NODE_ID` (string)
    /// - `SYNAP_CLUSTER_NODE_ADDRESS` (host:port)
    /// - `SYNAP_CLUSTER_SEEDS` (comma-separated host:port list)
    /// - `SYNAP_CLUSTER_PORT` (u16)
    /// - `SYNAP_CLUSTER_NODE_TIMEOUT_MS`, `SYNAP_CLUSTER_MIGRATION_BATCH_SIZE`,
    ///   `SYNAP_CLUSTER_MIGRATION_TIMEOUT_SECS`, `SYNAP_CLUSTER_RAFT_ELECTION_TIMEOUT_MS`,
    ///   `SYNAP_CLUSTER_RAFT_HEARTBEAT_INTERVAL_MS` (integers)
    /// - `SYNAP_CLUSTER_REQUIRE_FULL_COVERAGE` (bool)
    ///
    /// Invalid values fall back to the default for that field.
    pub fn from_env() -> Self {
        Self::from_getter(|k| std::env::var(k).ok())
    }

    /// Overlay cluster config from a variable getter (env-var backed in prod,
    /// map-backed in tests — avoids racy global env mutation).
    pub(crate) fn from_getter(get: impl Fn(&str) -> Option<String>) -> Self {
        let mut cfg = Self::default();

        if let Some(v) = get("SYNAP_CLUSTER_ENABLED") {
            cfg.enabled = v.parse().unwrap_or(cfg.enabled);
        }
        if let Some(v) = get("SYNAP_CLUSTER_NODE_ID") {
            if !v.is_empty() {
                cfg.node_id = Some(v);
            }
        }
        if let Some(v) = get("SYNAP_CLUSTER_NODE_ADDRESS") {
            if let Ok(addr) = v.parse() {
                cfg.node_address = addr;
            }
        }
        if let Some(v) = get("SYNAP_CLUSTER_SEEDS") {
            cfg.seed_nodes = v
                .split(',')
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .filter_map(|s| s.parse().ok())
                .collect();
        }
        if let Some(v) = get("SYNAP_CLUSTER_PORT") {
            cfg.cluster_port = v.parse().unwrap_or(cfg.cluster_port);
        }
        if let Some(v) = get("SYNAP_CLUSTER_NODE_TIMEOUT_MS") {
            cfg.node_timeout_ms = v.parse().unwrap_or(cfg.node_timeout_ms);
        }
        if let Some(v) = get("SYNAP_CLUSTER_REQUIRE_FULL_COVERAGE") {
            cfg.require_full_coverage = v.parse().unwrap_or(cfg.require_full_coverage);
        }
        if let Some(v) = get("SYNAP_CLUSTER_MIGRATION_BATCH_SIZE") {
            cfg.migration_batch_size = v.parse().unwrap_or(cfg.migration_batch_size);
        }
        if let Some(v) = get("SYNAP_CLUSTER_MIGRATION_TIMEOUT_SECS") {
            cfg.migration_timeout_secs = v.parse().unwrap_or(cfg.migration_timeout_secs);
        }
        if let Some(v) = get("SYNAP_CLUSTER_RAFT_ELECTION_TIMEOUT_MS") {
            cfg.raft_election_timeout_ms = v.parse().unwrap_or(cfg.raft_election_timeout_ms);
        }
        if let Some(v) = get("SYNAP_CLUSTER_RAFT_HEARTBEAT_INTERVAL_MS") {
            cfg.raft_heartbeat_interval_ms = v.parse().unwrap_or(cfg.raft_heartbeat_interval_ms);
        }

        cfg
    }

    /// Get node timeout as Duration
    pub fn node_timeout(&self) -> Duration {
        Duration::from_millis(self.node_timeout_ms)
    }

    /// Get migration timeout as Duration
    pub fn migration_timeout(&self) -> Duration {
        Duration::from_secs(self.migration_timeout_secs)
    }

    /// Get raft election timeout as Duration
    pub fn raft_election_timeout(&self) -> Duration {
        Duration::from_millis(self.raft_election_timeout_ms)
    }

    /// Get raft heartbeat interval as Duration
    pub fn raft_heartbeat_interval(&self) -> Duration {
        Duration::from_millis(self.raft_heartbeat_interval_ms)
    }
}
