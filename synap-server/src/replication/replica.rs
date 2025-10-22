use super::config::ReplicationConfig;
use super::types::{
    ReplicationCommand, ReplicationError, ReplicationOperation, ReplicationResult, ReplicationStats,
};
use crate::core::KVStore;
use crate::persistence::types::Operation;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

/// Replica Node - Read-only node that receives operations from master
///
/// Features:
/// - Connects to master on startup
/// - Receives full/partial sync
/// - Applies operations to local state
/// - Tracks replication lag
/// - Auto-reconnects on disconnect
pub struct ReplicaNode {
    config: ReplicationConfig,
    kv_store: Arc<KVStore>,

    /// Current offset (last applied operation)
    current_offset: Arc<AtomicU64>,

    /// Master offset (last known master offset from heartbeat)
    master_offset: Arc<AtomicU64>,

    /// Last heartbeat timestamp
    last_heartbeat: Arc<AtomicU64>,

    /// Connection status
    connected: Arc<AtomicBool>,

    /// Replication stats
    stats: Arc<RwLock<ReplicationStats>>,
}

impl ReplicaNode {
    /// Create a new replica node
    pub async fn new(
        config: ReplicationConfig,
        kv_store: Arc<KVStore>,
    ) -> ReplicationResult<Arc<Self>> {
        if !config.is_replica() {
            return Err(ReplicationError::NotReplica);
        }

        info!("Initializing replica node");

        let replica = Arc::new(Self {
            config,
            kv_store,
            current_offset: Arc::new(AtomicU64::new(0)),
            master_offset: Arc::new(AtomicU64::new(0)),
            last_heartbeat: Arc::new(AtomicU64::new(0)),
            connected: Arc::new(AtomicBool::new(false)),
            stats: Arc::new(RwLock::new(ReplicationStats::default())),
        });

        // Start replication loop in background task
        let replica_clone = Arc::clone(&replica);
        tokio::spawn(async move {
            replica_clone.replication_loop().await;
        });

        // Small delay to let task start
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        Ok(replica)
    }

    /// Main replication loop - connect, sync, and receive updates
    async fn replication_loop(self: Arc<Self>) {
        let master_addr = self.config.master_address.unwrap();
        let reconnect_delay = Duration::from_millis(self.config.reconnect_delay_ms);

        eprintln!(
            "[REPLICA] Replication loop starting, master: {}, auto_reconnect: {}",
            master_addr, self.config.auto_reconnect
        );

        loop {
            eprintln!("[REPLICA] Connecting to master at {}", master_addr);

            match self.connect_and_sync(master_addr).await {
                Ok(_) => {
                    eprintln!("[REPLICA] Replication connection closed normally");
                }
                Err(e) => {
                    eprintln!("[REPLICA] Replication error: {}", e);
                    self.connected.store(false, Ordering::SeqCst);
                }
            }

            if !self.config.auto_reconnect {
                eprintln!("[REPLICA] Auto-reconnect disabled, stopping replication loop");
                break;
            }

            eprintln!("[REPLICA] Reconnecting in {:?}", reconnect_delay);
            tokio::time::sleep(reconnect_delay).await;
        }

        eprintln!("[REPLICA] Replication loop ended");
    }

    /// Connect to master and start synchronization
    async fn connect_and_sync(&self, master_addr: SocketAddr) -> ReplicationResult<()> {
        eprintln!("[REPLICA] TCP connecting to {}...", master_addr);

        // Connect to master
        let mut stream = TcpStream::connect(master_addr).await.map_err(|e| {
            eprintln!("[REPLICA] TCP connection failed: {}", e);
            ReplicationError::ConnectionFailed(e.to_string())
        })?;

        eprintln!("[REPLICA] TCP connected to master");

        // Send handshake with current offset
        let current_offset = self.current_offset.load(Ordering::SeqCst);
        let handshake = bincode::serialize(&current_offset)?;
        eprintln!("[REPLICA] Sending handshake, offset: {}", current_offset);
        stream.write_all(&handshake).await?;
        stream.flush().await?;

        eprintln!("[REPLICA] Handshake sent");

        // Mark as connected
        self.connected.store(true, Ordering::SeqCst);
        eprintln!("[REPLICA] Marked as connected");

        // Receive sync (full or partial)
        eprintln!("[REPLICA] Calling receive_sync...");
        self.receive_sync(&mut stream).await?;
        eprintln!("[REPLICA] receive_sync completed");

        // Receive ongoing replication commands
        eprintln!("[REPLICA] Starting to receive commands...");
        self.receive_commands(&mut stream).await?;

        Ok(())
    }

    /// Receive initial sync from master
    async fn receive_sync(&self, stream: &mut TcpStream) -> ReplicationResult<()> {
        info!("Waiting to receive sync command from master...");

        // Read sync command with length prefix
        let cmd = Self::read_command(stream).await.map_err(|e| {
            error!("Failed to read sync command: {}", e);
            e
        })?;

        info!("Received sync command");

        match cmd {
            ReplicationCommand::FullSync {
                snapshot_data,
                offset,
            } => {
                info!(
                    "Receiving full sync, offset: {}, {} bytes",
                    offset,
                    snapshot_data.len()
                );

                // Apply snapshot
                super::sync::apply_snapshot(&self.kv_store, &snapshot_data)
                    .await
                    .map_err(|e| {
                        error!("Failed to apply snapshot: {}", e);
                        ReplicationError::SerializationError(e)
                    })?;

                self.current_offset.store(offset, Ordering::SeqCst);
                info!("Full sync completed, data restored at offset {}", offset);
            }
            ReplicationCommand::PartialSync {
                from_offset,
                operations,
            } => {
                info!(
                    "Receiving partial sync from offset {}, {} operations",
                    from_offset,
                    operations.len()
                );

                // Apply operations
                for op in operations {
                    self.apply_operation(op).await?;
                }

                info!("Partial sync completed");
            }
            _ => {
                warn!("Unexpected command during sync: {:?}", cmd);
            }
        }

        Ok(())
    }

    /// Read command with length prefix
    async fn read_command(stream: &mut TcpStream) -> ReplicationResult<ReplicationCommand> {
        // Read length prefix (4 bytes)
        let mut len_buf = [0u8; 4];
        stream.read_exact(&mut len_buf).await?;
        let len = u32::from_be_bytes(len_buf) as usize;

        // Read data
        let mut data = vec![0u8; len];
        stream.read_exact(&mut data).await?;

        // Deserialize
        let cmd = bincode::deserialize(&data)?;
        Ok(cmd)
    }

    /// Receive ongoing replication commands
    async fn receive_commands(&self, stream: &mut TcpStream) -> ReplicationResult<()> {
        loop {
            // Read command with length prefix
            let cmd = match Self::read_command(stream).await {
                Ok(c) => c,
                Err(ReplicationError::IOError(e))
                    if e.kind() == std::io::ErrorKind::UnexpectedEof =>
                {
                    info!("Master closed connection");
                    return Ok(());
                }
                Err(e) => {
                    error!("Failed to read command: {}", e);
                    return Err(e);
                }
            };

            // Handle command
            match cmd {
                ReplicationCommand::Operation(op) => {
                    self.apply_operation(op).await?;
                }
                ReplicationCommand::Heartbeat {
                    master_offset,
                    timestamp,
                } => {
                    self.handle_heartbeat(master_offset, timestamp);
                }
                _ => {
                    debug!("Received unexpected command: {:?}", cmd);
                }
            }
        }
    }

    /// Apply a single replication operation
    async fn apply_operation(&self, op: ReplicationOperation) -> ReplicationResult<()> {
        debug!("Applying operation at offset {}", op.offset);

        // Verify offset sequence
        let expected_offset = self.current_offset.load(Ordering::SeqCst);
        if op.offset != expected_offset {
            warn!(
                "Offset mismatch: expected {}, got {}",
                expected_offset, op.offset
            );
            // Continue anyway (idempotent operations)
        }

        // Apply to KV store
        match &op.operation {
            Operation::KVSet { key, value, ttl } => {
                let _ = self.kv_store.set(key.as_str(), value.clone(), *ttl).await;
            }
            Operation::KVDel { keys } => {
                let _ = self.kv_store.mdel(keys).await;
            }
            _ => {
                // Other operations (Queue, Stream, etc.) would be handled here
                debug!("Skipping non-KV operation");
            }
        }

        // Update offset
        self.current_offset.store(op.offset + 1, Ordering::SeqCst);

        // Update stats
        let mut stats = self.stats.write().await;
        stats.replica_offset = op.offset;
        stats.total_replicated += 1;

        Ok(())
    }

    /// Handle heartbeat from master
    fn handle_heartbeat(&self, master_offset: u64, timestamp: u64) {
        self.master_offset.store(master_offset, Ordering::SeqCst);
        self.last_heartbeat.store(timestamp, Ordering::SeqCst);

        debug!("Heartbeat received, master offset: {}", master_offset);
    }

    /// Get replication statistics
    pub async fn stats(&self) -> ReplicationStats {
        let mut stats = self.stats.read().await.clone();

        let current_offset = self.current_offset.load(Ordering::SeqCst);
        let master_offset = self.master_offset.load(Ordering::SeqCst);

        stats.replica_offset = current_offset;
        stats.master_offset = master_offset;
        stats.lag_operations = master_offset.saturating_sub(current_offset);
        stats.connected = self.connected.load(Ordering::SeqCst);
        stats.last_heartbeat = self.last_heartbeat.load(Ordering::SeqCst);

        // Calculate lag in milliseconds
        let now = Self::current_timestamp();
        let last_hb = stats.last_heartbeat;
        stats.lag_ms = if last_hb > 0 {
            (now.saturating_sub(last_hb)) * 1000 // Convert to ms
        } else {
            0
        };

        stats
    }

    /// Check if connected to master
    pub fn is_connected(&self) -> bool {
        self.connected.load(Ordering::SeqCst)
    }

    /// Get current offset
    pub fn current_offset(&self) -> u64 {
        self.current_offset.load(Ordering::SeqCst)
    }

    /// Get replication lag (operations behind master)
    pub fn lag(&self) -> u64 {
        let current = self.current_offset.load(Ordering::SeqCst);
        let master = self.master_offset.load(Ordering::SeqCst);

        master.saturating_sub(current)
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
    async fn test_replica_initialization() {
        let mut config = ReplicationConfig::default();
        config.enabled = true;
        config.role = super::super::types::NodeRole::Replica;
        config.master_address = Some("127.0.0.1:15501".parse().unwrap());
        config.auto_reconnect = false; // Don't actually connect in test

        let kv = Arc::new(KVStore::new(KVConfig::default()));
        let replica = ReplicaNode::new(config, kv).await;

        assert!(replica.is_ok());
    }

    #[tokio::test]
    async fn test_replica_apply_operation() {
        let mut config = ReplicationConfig::default();
        config.enabled = true;
        config.role = super::super::types::NodeRole::Replica;
        config.master_address = Some("127.0.0.1:15501".parse().unwrap());
        config.auto_reconnect = false;

        let kv = Arc::new(KVStore::new(KVConfig::default()));
        let replica = ReplicaNode::new(config, kv.clone()).await.unwrap();

        // Apply SET operation
        let op = ReplicationOperation {
            offset: 0,
            timestamp: 0,
            operation: Operation::KVSet {
                key: "test_key".to_string(),
                value: b"test_value".to_vec(),
                ttl: None,
            },
        };

        replica.apply_operation(op).await.unwrap();

        // Verify
        let value = kv.get("test_key").await.unwrap();
        assert_eq!(value, Some(b"test_value".to_vec()));
        assert_eq!(replica.current_offset(), 1);
    }
}
