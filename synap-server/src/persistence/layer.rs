use super::types::{Operation, PersistenceConfig};
use super::{SnapshotManager, WriteAheadLog};
use crate::core::{KVStore, QueueManager};
use parking_lot::RwLock;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{debug, info};
use tokio::sync::Mutex;

/// Persistence layer that wraps operations with WAL logging
pub struct PersistenceLayer {
    wal: Arc<Mutex<WriteAheadLog>>,
    snapshot_mgr: Arc<SnapshotManager>,
    config: PersistenceConfig,
    last_snapshot: Arc<RwLock<Instant>>,
    operations_since_snapshot: Arc<RwLock<usize>>,
}

impl PersistenceLayer {
    /// Create a new persistence layer
    pub async fn new(config: PersistenceConfig) -> super::types::Result<Self> {
        let wal = WriteAheadLog::open(config.wal.clone()).await?;
        let snapshot_mgr = SnapshotManager::new(config.snapshot.clone());

        Ok(Self {
            wal: Arc::new(Mutex::new(wal)),
            snapshot_mgr: Arc::new(snapshot_mgr),
            config,
            last_snapshot: Arc::new(RwLock::new(Instant::now())),
            operations_since_snapshot: Arc::new(RwLock::new(0)),
        })
    }

    /// Log a KV SET operation
    pub async fn log_kv_set(
        &self,
        key: String,
        value: Vec<u8>,
        ttl: Option<u64>,
    ) -> super::types::Result<()> {
        if !self.config.enabled || !self.config.wal.enabled {
            return Ok(());
        }

        let operation = Operation::KVSet { key, value, ttl };
        
        let mut wal = self.wal.lock().await;
        wal.append(operation).await?;
        
        // Track operations for snapshot threshold
        let mut ops = self.operations_since_snapshot.write();
        *ops += 1;

        Ok(())
    }

    /// Log a KV DELETE operation
    pub async fn log_kv_del(&self, keys: Vec<String>) -> super::types::Result<()> {
        if !self.config.enabled || !self.config.wal.enabled {
            return Ok(());
        }

        let operation = Operation::KVDel { keys };
        
        let mut wal = self.wal.lock().await;
        wal.append(operation).await?;
        
        let mut ops = self.operations_since_snapshot.write();
        *ops += 1;

        Ok(())
    }

    /// Log a Queue PUBLISH operation
    pub async fn log_queue_publish(
        &self,
        queue: String,
        message: crate::core::queue::QueueMessage,
    ) -> super::types::Result<()> {
        if !self.config.enabled || !self.config.wal.enabled {
            return Ok(());
        }

        let operation = Operation::QueuePublish { queue, message };
        
        let mut wal = self.wal.lock().await;
        wal.append(operation).await?;
        
        let mut ops = self.operations_since_snapshot.write();
        *ops += 1;

        Ok(())
    }

    /// Log a Queue ACK operation
    pub async fn log_queue_ack(
        &self,
        queue: String,
        message_id: String,
    ) -> super::types::Result<()> {
        if !self.config.enabled || !self.config.wal.enabled {
            return Ok(());
        }

        let operation = Operation::QueueAck { queue, message_id };
        
        let mut wal = self.wal.lock().await;
        wal.append(operation).await?;

        Ok(())
    }

    /// Create a snapshot if conditions are met
    pub async fn maybe_snapshot(
        &self,
        kv_store: &KVStore,
        queue_manager: Option<&QueueManager>,
    ) -> super::types::Result<()> {
        if !self.config.enabled || !self.config.snapshot.enabled {
            return Ok(());
        }

        let should_snapshot = {
            let last = self.last_snapshot.read();
            let ops = self.operations_since_snapshot.read();
            
            let time_elapsed = last.elapsed().as_secs() >= self.config.snapshot.interval_secs;
            let ops_threshold = *ops >= self.config.snapshot.operation_threshold;
            
            time_elapsed || ops_threshold
        };

        if should_snapshot {
            info!("Creating periodic snapshot");
            
            let wal_offset = {
                let wal = self.wal.lock().await;
                wal.current_offset()
            };
            
            self.snapshot_mgr
                .create_snapshot(kv_store, queue_manager, wal_offset)
                .await?;
            
            // Reset counters
            *self.last_snapshot.write() = Instant::now();
            *self.operations_since_snapshot.write() = 0;
        }

        Ok(())
    }

    /// Start background snapshot task
    pub fn start_snapshot_task(
        self: Arc<Self>,
        kv_store: Arc<KVStore>,
        queue_manager: Option<Arc<QueueManager>>,
    ) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(60));  // Check every minute

            loop {
                interval.tick().await;

                if let Err(e) = self
                    .maybe_snapshot(&kv_store, queue_manager.as_ref().map(|q| q.as_ref()))
                    .await
                {
                    tracing::error!("Snapshot failed: {}", e);
                }
            }
        })
    }

    /// Flush WAL to disk
    pub async fn flush(&self) -> super::types::Result<()> {
        let mut wal = self.wal.lock().await;
        wal.flush().await
    }
}

