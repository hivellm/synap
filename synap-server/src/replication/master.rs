use super::config::ReplicationConfig;
use super::replication_log::ReplicationLog;
use super::types::{
    ReplicaInfo, ReplicationCommand, ReplicationError, ReplicationOperation, ReplicationResult,
    ReplicationStats,
};
use crate::core::KVStore;
use crate::persistence::types::Operation;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Master Node - Accepts writes and replicates to replica nodes
///
/// Features:
/// - Maintains replication log
/// - Sends operations to all connected replicas
/// - Monitors replica lag
/// - Handles full and partial sync
/// - Heartbeat mechanism
pub struct MasterNode {
    #[allow(dead_code)]
    config: ReplicationConfig,
    replication_log: Arc<ReplicationLog>,

    /// Connected replicas
    replicas: Arc<RwLock<HashMap<String, ReplicaConnection>>>,

    /// Channel to send operations to replication task
    replication_tx: mpsc::UnboundedSender<ReplicationMessage>,
}

struct ReplicaConnection {
    id: String,
    address: SocketAddr,
    offset: u64,
    connected_at: u64,
    last_heartbeat: u64,
    sender: mpsc::UnboundedSender<ReplicationCommand>,
}

enum ReplicationMessage {
    Operation(Operation),
    #[allow(dead_code)]
    Heartbeat,
}

impl MasterNode {
    /// Create a new master node
    pub async fn new(config: ReplicationConfig, kv_store: Arc<KVStore>) -> ReplicationResult<Self> {
        if !config.is_master() {
            return Err(ReplicationError::NotMaster);
        }

        info!("Initializing master node");

        // Create replication log (1M operations buffer, like Redis)
        let replication_log = Arc::new(ReplicationLog::new(1_000_000));

        let (replication_tx, replication_rx) = mpsc::unbounded_channel();
        let replicas = Arc::new(RwLock::new(HashMap::new()));

        // Spawn listener for replica connections
        let listen_addr = config.replica_listen_address.unwrap();
        let replicas_clone = Arc::clone(&replicas);
        let log_clone = Arc::clone(&replication_log);
        let kv_clone = Arc::clone(&kv_store);

        tokio::spawn(Self::listen_for_replicas(
            listen_addr,
            replicas_clone,
            log_clone,
            kv_clone,
        ));

        // Spawn heartbeat task
        let replicas_clone = Arc::clone(&replicas);
        let heartbeat_interval = config.heartbeat_interval_ms;
        tokio::spawn(Self::heartbeat_task(replicas_clone, heartbeat_interval));

        // Spawn replication task (process operations)
        let replicas_clone = Arc::clone(&replicas);
        let log_clone = Arc::clone(&replication_log);
        tokio::spawn(Self::replication_task(
            replication_rx,
            replicas_clone,
            log_clone,
        ));

        Ok(Self {
            config,
            replication_log,
            replicas,
            replication_tx,
        })
    }

    /// Listen for replica connections
    async fn listen_for_replicas(
        listen_addr: SocketAddr,
        replicas: Arc<RwLock<HashMap<String, ReplicaConnection>>>,
        replication_log: Arc<ReplicationLog>,
        kv_store: Arc<KVStore>,
    ) {
        let listener = match TcpListener::bind(listen_addr).await {
            Ok(l) => l,
            Err(e) => {
                error!("Failed to bind replication listener: {}", e);
                return;
            }
        };

        info!("Master listening for replicas on {}", listen_addr);

        loop {
            match listener.accept().await {
                Ok((stream, addr)) => {
                    info!("New replica connection from {}", addr);

                    let replicas_clone = Arc::clone(&replicas);
                    let log_clone = Arc::clone(&replication_log);
                    let kv_clone = Arc::clone(&kv_store);

                    tokio::spawn(Self::handle_replica(
                        stream,
                        addr,
                        replicas_clone,
                        log_clone,
                        kv_clone,
                    ));
                }
                Err(e) => {
                    warn!("Failed to accept replica connection: {}", e);
                }
            }
        }
    }

    /// Handle a single replica connection
    async fn handle_replica(
        mut stream: TcpStream,
        addr: SocketAddr,
        replicas: Arc<RwLock<HashMap<String, ReplicaConnection>>>,
        replication_log: Arc<ReplicationLog>,
        kv_store: Arc<KVStore>,
    ) {
        let replica_id = Uuid::new_v4().to_string();

        // Read replica handshake (request offset)
        let mut buf = vec![0u8; 1024];
        let requested_offset = match stream.read(&mut buf).await {
            Ok(n) if n > 0 => match bincode::deserialize::<u64>(&buf[..n]) {
                Ok(offset) => offset,
                Err(_) => 0,
            },
            _ => 0,
        };

        info!(
            "Replica {} requesting sync from offset {}",
            replica_id, requested_offset
        );

        // Determine sync type
        let oldest_offset = replication_log.oldest_offset();
        let needs_full_sync = requested_offset < oldest_offset;

        if needs_full_sync {
            info!("Performing full sync for replica {}", replica_id);

            // Send full snapshot
            if let Err(e) = Self::send_full_sync(&mut stream, &kv_store, &replication_log).await {
                error!("Full sync failed for replica {}: {}", replica_id, e);
                return;
            }
        } else {
            info!("Performing partial sync for replica {}", replica_id);

            // Send incremental updates
            if let Err(e) =
                Self::send_partial_sync(&mut stream, requested_offset, &replication_log).await
            {
                error!("Partial sync failed for replica {}: {}", replica_id, e);
                return;
            }
        }

        // Register replica
        let (tx, mut rx) = mpsc::unbounded_channel();
        {
            let mut reps = replicas.write();
            reps.insert(
                replica_id.clone(),
                ReplicaConnection {
                    id: replica_id.clone(),
                    address: addr,
                    offset: replication_log.current_offset(),
                    connected_at: Self::current_timestamp(),
                    last_heartbeat: Self::current_timestamp(),
                    sender: tx,
                },
            );
        }

        info!("Replica {} connected and synced", replica_id);

        // Stream replication commands
        while let Some(cmd) = rx.recv().await {
            let data = match bincode::serialize(&cmd) {
                Ok(d) => d,
                Err(e) => {
                    error!("Failed to serialize command: {}", e);
                    break;
                }
            };

            // Send command
            if stream.write_all(&data).await.is_err() {
                warn!("Replica {} disconnected", replica_id);
                break;
            }
        }

        // Remove replica on disconnect
        replicas.write().remove(&replica_id);
        info!("Replica {} disconnected", replica_id);
    }

    /// Send full sync (snapshot) to replica
    async fn send_full_sync(
        stream: &mut TcpStream,
        _kv_store: &KVStore,
        replication_log: &ReplicationLog,
    ) -> ReplicationResult<()> {
        // Get snapshot data (simplified - in production, use streaming)
        let snapshot_data = vec![]; // TODO: Implement snapshot serialization
        let current_offset = replication_log.current_offset();

        let cmd = ReplicationCommand::FullSync {
            snapshot_data,
            offset: current_offset,
        };

        let data = bincode::serialize(&cmd)?;
        stream.write_all(&data).await?;
        stream.flush().await?;

        debug!("Full sync sent, offset: {}", current_offset);
        Ok(())
    }

    /// Send partial sync (incremental operations) to replica
    async fn send_partial_sync(
        stream: &mut TcpStream,
        from_offset: u64,
        replication_log: &ReplicationLog,
    ) -> ReplicationResult<()> {
        let operations = replication_log.get_from_offset(from_offset)?;
        let op_count = operations.len();

        let cmd = ReplicationCommand::PartialSync {
            from_offset,
            operations,
        };

        let data = bincode::serialize(&cmd)?;
        stream.write_all(&data).await?;
        stream.flush().await?;

        debug!("Partial sync sent, {} operations", op_count);
        Ok(())
    }

    /// Background task to send heartbeats
    async fn heartbeat_task(
        replicas: Arc<RwLock<HashMap<String, ReplicaConnection>>>,
        interval_ms: u64,
    ) {
        let mut interval = tokio::time::interval(Duration::from_millis(interval_ms));

        loop {
            interval.tick().await;

            let reps = replicas.read();
            for (_, replica) in reps.iter() {
                let heartbeat = ReplicationCommand::Heartbeat {
                    master_offset: 0, // Will be filled by replication task
                    timestamp: Self::current_timestamp(),
                };

                let _ = replica.sender.send(heartbeat);
            }
        }
    }

    /// Background task to replicate operations
    async fn replication_task(
        mut rx: mpsc::UnboundedReceiver<ReplicationMessage>,
        replicas: Arc<RwLock<HashMap<String, ReplicaConnection>>>,
        replication_log: Arc<ReplicationLog>,
    ) {
        while let Some(msg) = rx.recv().await {
            match msg {
                ReplicationMessage::Operation(operation) => {
                    // Append to replication log
                    let offset = replication_log.append(operation.clone());

                    // Send to all replicas
                    let reps = replicas.read();
                    for (_, replica) in reps.iter() {
                        let cmd = ReplicationCommand::Operation(ReplicationOperation {
                            offset,
                            timestamp: Self::current_timestamp(),
                            operation: operation.clone(),
                        });

                        let _ = replica.sender.send(cmd);
                    }
                }
                ReplicationMessage::Heartbeat => {
                    // Handled by heartbeat_task
                }
            }
        }
    }

    /// Replicate an operation to all replicas
    pub fn replicate(&self, operation: Operation) -> u64 {
        let offset = self.replication_log.append(operation.clone());

        // Send to replication task
        let _ = self
            .replication_tx
            .send(ReplicationMessage::Operation(operation));

        offset
    }

    /// Get list of connected replicas
    pub fn list_replicas(&self) -> Vec<ReplicaInfo> {
        let reps = self.replicas.read();
        reps.values()
            .map(|r| ReplicaInfo {
                id: r.id.clone(),
                address: r.address,
                role: super::types::NodeRole::Replica,
                offset: r.offset,
                connected_at: r.connected_at,
                last_sync: r.last_heartbeat,
                lag_ms: 0, // TODO: Calculate from heartbeat
            })
            .collect()
    }

    /// Get replication statistics
    pub fn stats(&self) -> ReplicationStats {
        let current_offset = self.replication_log.current_offset();
        let reps = self.replicas.read();

        // Calculate min replica offset
        let min_replica_offset = reps
            .values()
            .map(|r| r.offset)
            .min()
            .unwrap_or(current_offset);

        ReplicationStats {
            master_offset: current_offset,
            replica_offset: min_replica_offset,
            lag_operations: current_offset.saturating_sub(min_replica_offset),
            lag_ms: 0, // TODO: Calculate from timestamps
            total_replicated: current_offset,
            total_bytes: 0, // TODO: Track bytes
            last_heartbeat: Self::current_timestamp(),
            connected: !reps.is_empty(),
        }
    }

    fn current_timestamp() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::KVConfig;

    #[tokio::test]
    async fn test_master_initialization() {
        let mut config = ReplicationConfig::default();
        config.enabled = true;
        config.role = super::super::types::NodeRole::Master;
        config.replica_listen_address = Some("127.0.0.1:0".parse().unwrap());

        let kv = Arc::new(KVStore::new(KVConfig::default()));
        let master = MasterNode::new(config, kv).await;

        assert!(master.is_ok());
    }

    #[tokio::test]
    async fn test_master_replication_log() {
        let mut config = ReplicationConfig::default();
        config.enabled = true;
        config.role = super::super::types::NodeRole::Master;
        config.replica_listen_address = Some("127.0.0.1:0".parse().unwrap());

        let kv = Arc::new(KVStore::new(KVConfig::default()));
        let master = MasterNode::new(config, kv).await.unwrap();

        // Replicate operation
        let op = Operation::KVSet {
            key: "test_key".to_string(),
            value: b"test_value".to_vec(),
            ttl: None,
        };

        let offset = master.replicate(op);
        assert_eq!(offset, 0);

        // Check stats
        let stats = master.stats();
        assert_eq!(stats.master_offset, 1);
    }
}
