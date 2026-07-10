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
    /// Create cluster config from environment/config file
    pub fn from_env() -> Self {
        // Load from environment variables or config file (tracked in hivellm/synap#233)
        Self::default()
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
