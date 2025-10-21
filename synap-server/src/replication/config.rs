use super::types::NodeRole;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

/// Replication configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplicationConfig {
    /// Enable replication
    pub enabled: bool,

    /// Node role (master, replica, standalone)
    pub role: NodeRole,

    /// Master address (for replica nodes)
    pub master_address: Option<SocketAddr>,

    /// Replica listen address (for master to send data)
    pub replica_listen_address: Option<SocketAddr>,

    /// Heartbeat interval in milliseconds
    pub heartbeat_interval_ms: u64,

    /// Maximum lag threshold (ms) before alerting
    pub max_lag_ms: u64,

    /// Replication buffer size (KB)
    pub buffer_size_kb: usize,

    /// Automatic reconnect on disconnect
    pub auto_reconnect: bool,

    /// Reconnect delay in milliseconds
    pub reconnect_delay_ms: u64,

    /// Replica timeout in seconds (master marks replica dead)
    pub replica_timeout_secs: u64,
}

impl Default for ReplicationConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            role: NodeRole::Standalone,
            master_address: None,
            replica_listen_address: None,
            heartbeat_interval_ms: 1000, // 1 second heartbeat
            max_lag_ms: 10000,           // 10 seconds max lag
            buffer_size_kb: 256,         // 256KB buffer
            auto_reconnect: true,
            reconnect_delay_ms: 5000, // 5 seconds
            replica_timeout_secs: 30, // 30 seconds timeout
        }
    }
}

impl ReplicationConfig {
    /// Validate configuration
    pub fn validate(&self) -> Result<(), String> {
        if !self.enabled {
            return Ok(());
        }

        match self.role {
            NodeRole::Replica => {
                if self.master_address.is_none() {
                    return Err("Replica node requires master_address".to_string());
                }
            }
            NodeRole::Master => {
                if self.replica_listen_address.is_none() {
                    return Err("Master node requires replica_listen_address".to_string());
                }
            }
            NodeRole::Standalone => {
                if self.enabled {
                    return Err("Standalone node cannot have replication enabled".to_string());
                }
            }
        }

        Ok(())
    }

    /// Check if this node is a master
    pub fn is_master(&self) -> bool {
        self.enabled && self.role == NodeRole::Master
    }

    /// Check if this node is a replica
    pub fn is_replica(&self) -> bool {
        self.enabled && self.role == NodeRole::Replica
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_default_config() {
        let config = ReplicationConfig::default();
        assert!(!config.enabled);
        assert_eq!(config.role, NodeRole::Standalone);
    }

    #[test]
    fn test_master_validation() {
        let mut config = ReplicationConfig::default();
        config.enabled = true;
        config.role = NodeRole::Master;

        // Should fail without listen address
        assert!(config.validate().is_err());

        // Should succeed with listen address
        config.replica_listen_address = Some(SocketAddr::from_str("0.0.0.0:15501").unwrap());
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_replica_validation() {
        let mut config = ReplicationConfig::default();
        config.enabled = true;
        config.role = NodeRole::Replica;

        // Should fail without master address
        assert!(config.validate().is_err());

        // Should succeed with master address
        config.master_address = Some(SocketAddr::from_str("127.0.0.1:15501").unwrap());
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_role_checks() {
        let mut config = ReplicationConfig::default();
        config.enabled = true;

        config.role = NodeRole::Master;
        assert!(config.is_master());
        assert!(!config.is_replica());

        config.role = NodeRole::Replica;
        assert!(!config.is_master());
        assert!(config.is_replica());
    }
}
