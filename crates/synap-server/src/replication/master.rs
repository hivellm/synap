use super::config::ReplicationConfig;
use super::replication_log::ReplicationLog;
use super::types::{
    ReplicaInfo, ReplicationCommand, ReplicationError, ReplicationOperation, ReplicationResult,
    ReplicationStats,
};
use crate::core::{KVStore, StreamManager};
use crate::persistence::order::WriteOrder;
use crate::persistence::types::Operation;
use crate::persistence::{SnapshotImage, StoreArcs};
use parking_lot::{Mutex, RwLock};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, oneshot};
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

    /// Stores, write barrier and test pause the full sync reads from.
    full_sync: Arc<FullSyncSource>,

    /// Connected replicas
    replicas: Arc<RwLock<HashMap<String, ReplicaConnection>>>,

    /// Channel to send operations to replication task
    replication_tx: mpsc::UnboundedSender<ReplicationMessage>,

    /// Total bytes replicated (accumulated)
    total_bytes: Arc<AtomicU64>,
}

struct ReplicaConnection {
    id: String,
    address: SocketAddr,
    offset: u64,
    connected_at: u64,
    last_heartbeat: u64,
    sender: mpsc::UnboundedSender<ReplicationCommand>,
}

/// What a full sync reads (phase31): every replicated datatype store and the
/// write barrier shared with the persistence layer.
struct FullSyncSource {
    stores: StoreArcs,
    /// Per-key ordering stripes. The persistence layer built with this master
    /// uses the same lock ([`MasterNode::write_order`]), so holding every
    /// stripe means no write is between being applied and being replicated.
    order: Arc<WriteOrder>,
    /// Test seam set by [`MasterNode::pause_next_full_sync`].
    pause: Mutex<Option<FullSyncPause>>,
}

/// Pause of the next full sync while it holds the write barrier.
struct FullSyncPause {
    reached: oneshot::Sender<u64>,
    release: oneshot::Receiver<()>,
}

enum ReplicationMessage {
    Operation {
        operation: Operation,
        offset: u64,
    },
    #[allow(dead_code)]
    Heartbeat,
}

impl MasterNode {
    /// Create a master node whose full sync ships KV and, when given, streams.
    /// Use [`with_stores`](Self::with_stores) to ship every datatype.
    pub async fn new(
        config: ReplicationConfig,
        kv_store: Arc<KVStore>,
        stream_manager: Option<Arc<StreamManager>>,
    ) -> ReplicationResult<Self> {
        Self::with_stores(
            config,
            StoreArcs {
                stream_manager,
                ..StoreArcs::kv_only(kv_store)
            },
        )
        .await
    }

    /// Create a master node whose full sync ships every store in `stores`.
    pub async fn with_stores(
        config: ReplicationConfig,
        stores: StoreArcs,
    ) -> ReplicationResult<Self> {
        if !config.is_master() {
            return Err(ReplicationError::NotMaster);
        }

        info!(
            "Initializing master node with stream support: {}",
            stores.stream_manager.is_some()
        );
        let full_sync = Arc::new(FullSyncSource {
            stores,
            order: Arc::new(WriteOrder::new()),
            pause: Mutex::new(None),
        });

        // Create replication log (1M operations buffer, like Redis)
        let replication_log = Arc::new(ReplicationLog::new(1_000_000));

        let (replication_tx, replication_rx) = mpsc::unbounded_channel();
        let replicas = Arc::new(RwLock::new(HashMap::new()));

        // Spawn listener for replica connections
        let listen_addr = config
            .replica_listen_address
            .expect("master node is only started with a configured replica_listen_address");
        let replicas_clone = Arc::clone(&replicas);
        let log_clone = Arc::clone(&replication_log);

        tokio::spawn(Self::listen_for_replicas(
            listen_addr,
            replicas_clone,
            log_clone,
            Arc::clone(&full_sync),
        ));

        // Spawn heartbeat task
        let replicas_clone = Arc::clone(&replicas);
        let heartbeat_interval = config.heartbeat_interval_ms;
        tokio::spawn(Self::heartbeat_task(replicas_clone, heartbeat_interval));

        // Spawn replication task (process operations)
        let replicas_clone = Arc::clone(&replicas);
        let log_clone = Arc::clone(&replication_log);
        let total_bytes = Arc::new(AtomicU64::new(0));
        let total_bytes_clone = Arc::clone(&total_bytes);
        tokio::spawn(Self::replication_task(
            replication_rx,
            replicas_clone,
            log_clone,
            total_bytes_clone,
        ));

        Ok(Self {
            config,
            replication_log,
            full_sync,
            replicas,
            replication_tx,
            total_bytes,
        })
    }

    /// Listen for replica connections
    async fn listen_for_replicas(
        listen_addr: SocketAddr,
        replicas: Arc<RwLock<HashMap<String, ReplicaConnection>>>,
        replication_log: Arc<ReplicationLog>,
        full_sync: Arc<FullSyncSource>,
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

                    tokio::spawn(Self::handle_replica(
                        stream,
                        addr,
                        replicas_clone,
                        log_clone,
                        Arc::clone(&full_sync),
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
        full_sync: Arc<FullSyncSource>,
    ) {
        let replica_id = Uuid::new_v4().to_string();
        info!(
            replica_id = %replica_id,
            addr = %addr,
            "New replica connection from master"
        );

        // Read replica handshake (request offset)
        let mut buf = vec![0u8; 1024];
        debug!("Waiting for handshake from replica");
        let requested_offset = match stream.read(&mut buf).await {
            Ok(n) => {
                if n == 0 {
                    warn!("Connection closed before handshake");
                    return;
                }
                debug!(bytes = n, "Received handshake bytes");
                bincode::serde::decode_from_slice::<u64, _>(&buf[..n], bincode::config::legacy())
                    .map(|(v, _)| v)
                    .unwrap_or_default()
            }
            Err(e) => {
                error!(error = %e, "Error reading handshake");
                return;
            }
        };

        info!(
            replica_id = %replica_id,
            requested_offset = requested_offset,
            "Replica requesting sync from offset"
        );

        // Determine sync type
        let oldest_offset = replication_log.oldest_offset();
        let current_offset = replication_log.current_offset();

        // Full sync needed if:
        // 1. The replica has nothing from this master (requested 0). The log
        //    only holds writes made since this master started, not the data
        //    it recovered at boot, so replaying it from 0 is not enough.
        // 2. Requested offset is older than what we have in log
        // 3. Requested offset is beyond the log: the replica followed an
        //    earlier run of this master (its log restarted at 0).
        let needs_full_sync = requested_offset == 0
            || requested_offset < oldest_offset
            || requested_offset > current_offset;

        debug!(
            oldest_offset = oldest_offset,
            current_offset = current_offset,
            requested_offset = requested_offset,
            needs_full_sync = needs_full_sync,
            "Determining sync type"
        );

        // Register the replica BEFORE syncing (issue #234): once it is in the
        // replicas map, the replication fan-out buffers every live operation into
        // this replica's channel while the (possibly large) snapshot is still
        // transferring — so a write that lands between the snapshot point and
        // registration is no longer lost. Any overlap (operations the snapshot
        // already covers) is deduplicated by offset on the replica.
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

        let sync_result = if needs_full_sync {
            info!(replica_id = %replica_id, "Performing full sync for replica");
            Self::send_full_sync(&mut stream, &full_sync, &replication_log).await
        } else {
            info!(replica_id = %replica_id, "Performing partial sync for replica");
            Self::send_partial_sync(&mut stream, requested_offset, &replication_log).await
        };

        if let Err(e) = sync_result {
            error!(replica_id = %replica_id, error = %e, "Sync failed for replica");
            replicas.write().remove(&replica_id);
            return;
        }
        info!(replica_id = %replica_id, "Sync sent successfully");

        // Stream buffered + live replication commands. The buffer already holds
        // any operations that arrived during snapshot transfer; they are written
        // after the FullSync frame, so wire order stays snapshot-then-ops.
        while let Some(cmd) = rx.recv().await {
            if Self::send_command(&mut stream, &cmd).await.is_err() {
                warn!("Replica {} disconnected", replica_id);
                break;
            }
        }

        // Remove replica on disconnect
        replicas.write().remove(&replica_id);
        info!("Replica {} disconnected", replica_id);
    }

    /// Send a full sync: every datatype as of one replication offset.
    ///
    /// The stores are copied under the write barrier (see
    /// [`capture_full_sync`](Self::capture_full_sync)); stream events, which
    /// are not part of the replication log, and serialization happen after
    /// it is released.
    async fn send_full_sync(
        stream: &mut TcpStream,
        source: &FullSyncSource,
        replication_log: &ReplicationLog,
    ) -> ReplicationResult<()> {
        let (image, offset) = Self::capture_full_sync(source, replication_log).await?;
        let streams = match &source.stores.stream_manager {
            Some(sm) => sm.get_all_events().await,
            None => HashMap::new(),
        };
        let data = super::sync::FullSyncData::from_image(image, streams);
        let summary = data.summary();
        let cmd = super::sync::encode_full_sync(&data, offset)?;
        drop(data);

        info!("Full sync at offset {}: {}", offset, summary);
        Self::send_command(stream, &cmd).await?;

        debug!("Full sync sent, offset: {}", offset);
        Ok(())
    }

    /// Copy every replicated store and read the replication offset while
    /// holding every write-order stripe.
    ///
    /// Writers hold their keys' stripes from applying a write until its
    /// record is in the replication log, so under the barrier every write is
    /// either applied and logged below the offset (in the image) or not
    /// applied yet (logged at or after the offset and streamed). The replica
    /// skips streamed records below the offset, so each write takes effect
    /// exactly once.
    async fn capture_full_sync(
        source: &FullSyncSource,
        replication_log: &ReplicationLog,
    ) -> ReplicationResult<(SnapshotImage, u64)> {
        let pause = source.pause.lock().take();
        let _barrier = source.order.lock_all().await;
        let offset = replication_log.current_offset();
        if let Some(pause) = pause {
            let _ = pause.reached.send(offset);
            let _ = pause.release.await;
        }
        let image = SnapshotImage::capture(source.stores.as_refs())
            .await
            .map_err(|e| ReplicationError::FullSync(format!("capturing the image: {e}")))?;
        Ok((image, offset))
    }

    /// Send command with length prefix
    async fn send_command(
        stream: &mut TcpStream,
        cmd: &ReplicationCommand,
    ) -> ReplicationResult<()> {
        let data = bincode::serde::encode_to_vec(cmd, bincode::config::legacy())?;
        let len = data.len() as u32;

        // Send length prefix
        stream.write_all(&len.to_be_bytes()).await?;
        // Send data
        stream.write_all(&data).await?;
        stream.flush().await?;

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

        // Use the length-prefixed framing the replica's read_command expects —
        // a raw write_all here desynchronized the frame boundary so the replica
        // misparsed the partial sync and applied nothing (issue #234).
        Self::send_command(stream, &cmd).await?;

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

            let current_time = Self::current_timestamp();
            let mut reps = replicas.write();
            for replica in reps.values_mut() {
                // Update last_heartbeat timestamp when sending heartbeat
                replica.last_heartbeat = current_time;

                let heartbeat = ReplicationCommand::Heartbeat {
                    master_offset: 0, // Will be filled by replication task
                    timestamp: current_time,
                };

                let _ = replica.sender.send(heartbeat);
            }
        }
    }

    /// Background task to replicate operations
    async fn replication_task(
        mut rx: mpsc::UnboundedReceiver<ReplicationMessage>,
        replicas: Arc<RwLock<HashMap<String, ReplicaConnection>>>,
        _replication_log: Arc<ReplicationLog>,
        total_bytes: Arc<AtomicU64>,
    ) {
        while let Some(msg) = rx.recv().await {
            match msg {
                ReplicationMessage::Operation {
                    operation,
                    offset: _offset,
                } => {
                    // Operation already added to log by replicate() method
                    // Calculate size of serialized operation for byte tracking
                    let operation_size = match bincode::serde::encode_to_vec(
                        &ReplicationOperation {
                            offset: _offset,
                            timestamp: Self::current_timestamp(),
                            operation: operation.clone(),
                        },
                        bincode::config::legacy(),
                    ) {
                        Ok(data) => data.len() as u64,
                        Err(_) => 0, // Skip tracking if serialization fails
                    };

                    // Send to all replicas
                    let reps = replicas.read();
                    let replica_count = reps.len() as u64;
                    for replica in reps.values() {
                        let cmd = ReplicationCommand::Operation(ReplicationOperation {
                            offset: _offset,
                            timestamp: Self::current_timestamp(),
                            operation: operation.clone(),
                        });

                        let _ = replica.sender.send(cmd);
                    }

                    // Track total bytes: operation_size * number of replicas
                    total_bytes.fetch_add(operation_size * replica_count, Ordering::Relaxed);
                }
                ReplicationMessage::Heartbeat => {
                    // Handled by heartbeat_task
                }
            }
        }
    }

    /// Current replication-log offset — the number of operations propagated so
    /// far. Useful for INFO/status and for verifying that writes are flowing.
    pub fn replication_offset(&self) -> u64 {
        self.replication_log.current_offset()
    }

    /// The per-key write-order lock the full sync uses as its barrier. A
    /// persistence layer that replicates through this master must order its
    /// writes with this same lock (`PersistenceLayer::with_wal` does).
    pub fn write_order(&self) -> Arc<WriteOrder> {
        Arc::clone(&self.full_sync.order)
    }

    /// Test seam: the next full sync, once it holds the write barrier and
    /// has read its replication offset, sends that offset on the returned
    /// receiver and waits for the returned sender (or its drop) before it
    /// copies the stores. Writers issued meanwhile wait for the barrier.
    pub fn pause_next_full_sync(&self) -> (oneshot::Receiver<u64>, oneshot::Sender<()>) {
        let (reached_tx, reached_rx) = oneshot::channel();
        let (release_tx, release_rx) = oneshot::channel();
        *self.full_sync.pause.lock() = Some(FullSyncPause {
            reached: reached_tx,
            release: release_rx,
        });
        (reached_rx, release_tx)
    }

    /// Replicate an operation to all replicas
    pub fn replicate(&self, operation: Operation) -> u64 {
        // Queue for fan-out inside the log's critical section, so replicas
        // receive records in offset order.
        self.replication_log.append_with(operation, |record| {
            let _ = self.replication_tx.send(ReplicationMessage::Operation {
                operation: record.operation.clone(),
                offset: record.offset,
            });
        })
    }

    /// Get list of connected replicas
    pub fn list_replicas(&self) -> Vec<ReplicaInfo> {
        let current_time = Self::current_timestamp();
        let reps = self.replicas.read();
        reps.values()
            .map(|r| {
                // Calculate lag in milliseconds: (current_time - last_heartbeat) * 1000
                // last_heartbeat is updated when we send heartbeats, so this measures
                // how long since we last communicated with the replica
                let lag_ms = if r.last_heartbeat > 0 {
                    (current_time.saturating_sub(r.last_heartbeat)) * 1000
                } else {
                    0
                };

                ReplicaInfo {
                    id: r.id.clone(),
                    address: r.address,
                    role: super::types::NodeRole::Replica,
                    offset: r.offset,
                    connected_at: r.connected_at,
                    last_sync: r.last_heartbeat,
                    lag_ms,
                }
            })
            .collect()
    }

    /// Get replication statistics
    pub fn stats(&self) -> ReplicationStats {
        let current_offset = self.replication_log.current_offset();
        let current_time = Self::current_timestamp();
        let reps = self.replicas.read();

        // Calculate min replica offset
        let min_replica_offset = reps
            .values()
            .map(|r| r.offset)
            .min()
            .unwrap_or(current_offset);

        // Calculate lag in milliseconds
        // Strategy 1: Use operation timestamps (most accurate)
        // Strategy 2: Fallback to heartbeat timestamps
        let lag_ms = if min_replica_offset < current_offset && min_replica_offset > 0 {
            // Get the timestamp of the last operation the slowest replica received
            // min_replica_offset is the next offset needed, so last received is min_replica_offset - 1
            let last_received_offset = min_replica_offset - 1;

            if let Ok(ops) = self.replication_log.get_from_offset(last_received_offset) {
                // Find the operation at the last received offset
                if let Some(last_op) = ops.iter().find(|op| op.offset == last_received_offset) {
                    // Calculate lag: time since last operation was sent
                    (current_time.saturating_sub(last_op.timestamp)) * 1000
                } else {
                    // Fallback to heartbeat-based calculation
                    reps.values()
                        .map(|r| {
                            if r.last_heartbeat > 0 {
                                (current_time.saturating_sub(r.last_heartbeat)) * 1000
                            } else {
                                0
                            }
                        })
                        .max()
                        .unwrap_or(0)
                }
            } else {
                // Fallback to heartbeat-based calculation
                reps.values()
                    .map(|r| {
                        if r.last_heartbeat > 0 {
                            (current_time.saturating_sub(r.last_heartbeat)) * 1000
                        } else {
                            0
                        }
                    })
                    .max()
                    .unwrap_or(0)
            }
        } else if !reps.is_empty() {
            // Replicas are up to date, but calculate heartbeat lag for monitoring
            reps.values()
                .map(|r| {
                    if r.last_heartbeat > 0 {
                        (current_time.saturating_sub(r.last_heartbeat)) * 1000
                    } else {
                        0
                    }
                })
                .max()
                .unwrap_or(0)
        } else {
            // No replicas connected
            0
        };

        ReplicationStats {
            master_offset: current_offset,
            replica_offset: min_replica_offset,
            lag_operations: current_offset.saturating_sub(min_replica_offset),
            lag_ms,
            total_replicated: current_offset,
            total_bytes: self.total_bytes.load(Ordering::Relaxed),
            last_heartbeat: Self::current_timestamp(),
            connected: !reps.is_empty(),
        }
    }

    fn current_timestamp() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
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
        let master = MasterNode::new(config, kv, None).await;

        assert!(master.is_ok());
    }

    #[tokio::test]
    async fn test_master_replication_log() {
        let mut config = ReplicationConfig::default();
        config.enabled = true;
        config.role = super::super::types::NodeRole::Master;
        config.replica_listen_address = Some("127.0.0.1:0".parse().unwrap());

        let kv = Arc::new(KVStore::new(KVConfig::default()));
        let master = MasterNode::new(config, kv, None).await.unwrap();

        // Replicate operation
        let op = Operation::KVSet {
            key: "test_key".to_string(),
            value: b"test_value".to_vec(),
            ttl: None,
        };

        let offset = master.replicate(op);
        assert_eq!(offset, 0);

        // Test lag calculation
        let stats = master.stats();
        // With no replicas, lag should be 0
        assert_eq!(stats.lag_ms, 0);
        assert_eq!(stats.lag_operations, 0);
        assert_eq!(stats.master_offset, 1);
    }
}
