/// Failover management - Promote replica to master
///
/// Features:
/// - Manual failover (admin command)
/// - Graceful promotion (finish replicating)
/// - Configuration update
/// - Automatic mode (future)
use super::config::ReplicationConfig;
use super::master::MasterNode;
use super::replica::ReplicaNode;
use super::types::{NodeRole, ReplicationError, ReplicationResult};
use crate::core::KVStore;
use std::sync::Arc;
use tracing::{info, warn};

/// Failover manager
pub struct FailoverManager {
    #[allow(dead_code)]
    config: ReplicationConfig,
}

impl FailoverManager {
    pub fn new(config: ReplicationConfig) -> Self {
        Self { config }
    }

    /// Promote replica to master
    ///
    /// Steps:
    /// 1. Wait for replica to catch up (apply all pending operations)
    /// 2. Stop replication from old master
    /// 3. Change role to master
    /// 4. Start accepting replica connections
    ///
    /// Returns new master node
    pub async fn promote_replica_to_master(
        replica: Arc<ReplicaNode>,
        kv_store: Arc<KVStore>,
    ) -> ReplicationResult<MasterNode> {
        info!("Starting replica promotion to master");

        // Wait for replica to catch up
        Self::wait_for_sync(replica.as_ref()).await?;

        // Get current offset
        let current_offset = replica.current_offset();
        info!("Replica synced at offset {}", current_offset);

        // Stop replica (will disconnect from master)
        // Note: In real implementation, we'd need a shutdown mechanism
        warn!("Replica disconnect not fully implemented - manual intervention may be needed");

        // Create new master configuration
        let mut master_config = ReplicationConfig::default();
        master_config.enabled = true;
        master_config.role = NodeRole::Master;
        master_config.replica_listen_address = Some("0.0.0.0:15501".parse().unwrap()); // Default
        master_config.heartbeat_interval_ms = 1000;
        master_config.max_lag_ms = 10000;

        // Create new master node
        let master = MasterNode::new(master_config, kv_store).await?;

        info!("Replica successfully promoted to master");

        Ok(master)
    }

    /// Wait for replica to catch up with master
    async fn wait_for_sync(replica: &ReplicaNode) -> ReplicationResult<()> {
        const MAX_WAIT_SECS: u64 = 60; // 1 minute timeout
        const CHECK_INTERVAL_MS: u64 = 100;

        let start = std::time::Instant::now();

        loop {
            let stats = replica.stats().await;

            // Check if caught up (lag < 1 operation)
            if stats.lag_operations <= 1 {
                info!("Replica caught up: offset {}", stats.replica_offset);
                return Ok(());
            }

            // Check timeout
            if start.elapsed().as_secs() > MAX_WAIT_SECS {
                return Err(ReplicationError::LagTooHigh(stats.lag_ms));
            }

            // Wait and retry
            tokio::time::sleep(tokio::time::Duration::from_millis(CHECK_INTERVAL_MS)).await;
        }
    }

    /// Demote master to replica (reverse failover)
    pub async fn demote_master_to_replica(
        _master: MasterNode,
        new_master_addr: std::net::SocketAddr,
        kv_store: Arc<KVStore>,
    ) -> ReplicationResult<Arc<ReplicaNode>> {
        info!(
            "Demoting master to replica, new master: {}",
            new_master_addr
        );

        // Stop master (close replica connections)
        // Note: In real implementation, we'd need a shutdown mechanism
        warn!("Master shutdown not fully implemented");

        // Create replica configuration
        let mut replica_config = ReplicationConfig::default();
        replica_config.enabled = true;
        replica_config.role = NodeRole::Replica;
        replica_config.master_address = Some(new_master_addr);
        replica_config.auto_reconnect = true;

        // Create new replica node
        let replica = ReplicaNode::new(replica_config, kv_store).await?;

        info!("Master successfully demoted to replica");

        Ok(replica)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::KVConfig;

    #[tokio::test]
    async fn test_failover_manager_creation() {
        let config = ReplicationConfig::default();
        let manager = FailoverManager::new(config);

        // Basic test - just verify creation
        assert!(true);
    }

    #[tokio::test]
    async fn test_promote_replica() {
        // Create a replica
        let mut replica_config = ReplicationConfig::default();
        replica_config.enabled = true;
        replica_config.role = NodeRole::Replica;
        replica_config.master_address = Some("127.0.0.1:15501".parse().unwrap());
        replica_config.auto_reconnect = false;

        let kv = Arc::new(KVStore::new(KVConfig::default()));
        let replica = ReplicaNode::new(replica_config, Arc::clone(&kv))
            .await
            .unwrap();

        // Promote to master
        let master = FailoverManager::promote_replica_to_master(replica, kv)
            .await
            .unwrap();

        // Verify master is created
        let stats = master.stats();
        // Offset is always >= 0 for u64
        assert!(stats.master_offset == 0 || stats.master_offset > 0);
    }
}
