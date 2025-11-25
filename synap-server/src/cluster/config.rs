use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::time::Duration;

/// Cluster configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterConfig {
    /// Enable cluster mode
    pub enabled: bool,

    /// This node's ID (auto-generated if not set)
    pub node_id: Option<String>,

    /// This node's address
    pub node_address: SocketAddr,

    /// Cluster seed nodes (for discovery)
    pub seed_nodes: Vec<SocketAddr>,

    /// Cluster communication port
    pub cluster_port: u16,

    /// Node timeout (milliseconds)
    pub node_timeout_ms: u64,

    /// Cluster require full coverage
    /// If true, cluster will not accept writes if < 16384 slots are covered
    pub require_full_coverage: bool,

    /// Migration batch size (keys per batch)
    pub migration_batch_size: usize,

    /// Migration timeout (seconds)
    pub migration_timeout_secs: u64,

    /// Raft election timeout (milliseconds)
    pub raft_election_timeout_ms: u64,

    /// Raft heartbeat interval (milliseconds)
    pub raft_heartbeat_interval_ms: u64,
}

impl Default for ClusterConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            node_id: None,
            node_address: "127.0.0.1:15502".parse().unwrap(),
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
        // TODO: Load from environment variables or config file
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
