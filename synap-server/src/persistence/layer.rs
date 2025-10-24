use super::types::{Operation, PersistenceConfig};
use super::{AsyncWAL, SnapshotManager};
use crate::core::{KVStore, QueueManager, StreamManager};
use parking_lot::RwLock;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::info;

/// Persistence layer that wraps operations with WAL logging
/// Uses AsyncWAL for high-throughput group commit optimization
pub struct PersistenceLayer {
    wal: Arc<AsyncWAL>,
    snapshot_mgr: Arc<SnapshotManager>,
    config: PersistenceConfig,
    last_snapshot: Arc<RwLock<Instant>>,
    operations_since_snapshot: Arc<RwLock<usize>>,
}

impl PersistenceLayer {
    /// Create a new persistence layer with AsyncWAL
    pub async fn new(config: PersistenceConfig) -> super::types::Result<Self> {
        let wal = AsyncWAL::open(config.wal.clone()).await?;
        let snapshot_mgr = SnapshotManager::new(config.snapshot.clone());

        Ok(Self {
            wal: Arc::new(wal),
            snapshot_mgr: Arc::new(snapshot_mgr),
            config,
            last_snapshot: Arc::new(RwLock::new(Instant::now())),
            operations_since_snapshot: Arc::new(RwLock::new(0)),
        })
    }

    /// Log a KV SET operation (non-blocking with group commit)
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

        // AsyncWAL batches this automatically
        self.wal.append(operation).await?;

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

        self.wal.append(operation).await?;

        let mut ops = self.operations_since_snapshot.write();
        *ops += 1;

        Ok(())
    }

    /// Log a Hash SET operation
    pub async fn log_hash_set(
        &self,
        key: String,
        field: String,
        value: Vec<u8>,
    ) -> super::types::Result<()> {
        if !self.config.enabled || !self.config.wal.enabled {
            return Ok(());
        }

        let operation = Operation::HashSet { key, field, value };

        self.wal.append(operation).await?;

        let mut ops = self.operations_since_snapshot.write();
        *ops += 1;

        Ok(())
    }

    /// Log a Hash DELETE operation
    pub async fn log_hash_del(&self, key: String, fields: Vec<String>) -> super::types::Result<()> {
        if !self.config.enabled || !self.config.wal.enabled {
            return Ok(());
        }

        let operation = Operation::HashDel { key, fields };

        self.wal.append(operation).await?;

        let mut ops = self.operations_since_snapshot.write();
        *ops += 1;

        Ok(())
    }

    /// Log a Hash INCREMENT operation
    pub async fn log_hash_incrby(
        &self,
        key: String,
        field: String,
        increment: i64,
    ) -> super::types::Result<()> {
        if !self.config.enabled || !self.config.wal.enabled {
            return Ok(());
        }

        let operation = Operation::HashIncrBy {
            key,
            field,
            increment,
        };

        self.wal.append(operation).await?;

        let mut ops = self.operations_since_snapshot.write();
        *ops += 1;

        Ok(())
    }

    /// Log a Hash INCREMENT BY FLOAT operation
    pub async fn log_hash_incrbyfloat(
        &self,
        key: String,
        field: String,
        increment: f64,
    ) -> super::types::Result<()> {
        if !self.config.enabled || !self.config.wal.enabled {
            return Ok(());
        }

        let operation = Operation::HashIncrByFloat {
            key,
            field,
            increment,
        };

        self.wal.append(operation).await?;

        let mut ops = self.operations_since_snapshot.write();
        *ops += 1;

        Ok(())
    }

    /// Log a List PUSH operation
    pub async fn log_list_push(
        &self,
        key: String,
        values: Vec<Vec<u8>>,
        left: bool,
    ) -> super::types::Result<()> {
        if !self.config.enabled || !self.config.wal.enabled {
            return Ok(());
        }

        let operation = Operation::ListPush { key, values, left };

        self.wal.append(operation).await?;

        let mut ops = self.operations_since_snapshot.write();
        *ops += 1;

        Ok(())
    }

    /// Log a List POP operation
    pub async fn log_list_pop(
        &self,
        key: String,
        count: usize,
        left: bool,
    ) -> super::types::Result<()> {
        if !self.config.enabled || !self.config.wal.enabled {
            return Ok(());
        }

        let operation = Operation::ListPop { key, count, left };

        self.wal.append(operation).await?;

        let mut ops = self.operations_since_snapshot.write();
        *ops += 1;

        Ok(())
    }

    /// Log a List SET operation
    pub async fn log_list_set(
        &self,
        key: String,
        index: i64,
        value: Vec<u8>,
    ) -> super::types::Result<()> {
        if !self.config.enabled || !self.config.wal.enabled {
            return Ok(());
        }

        let operation = Operation::ListSet { key, index, value };

        self.wal.append(operation).await?;

        let mut ops = self.operations_since_snapshot.write();
        *ops += 1;

        Ok(())
    }

    /// Log a List TRIM operation
    pub async fn log_list_trim(
        &self,
        key: String,
        start: i64,
        stop: i64,
    ) -> super::types::Result<()> {
        if !self.config.enabled || !self.config.wal.enabled {
            return Ok(());
        }

        let operation = Operation::ListTrim { key, start, stop };

        self.wal.append(operation).await?;

        let mut ops = self.operations_since_snapshot.write();
        *ops += 1;

        Ok(())
    }

    /// Log a List REMOVE operation
    pub async fn log_list_rem(
        &self,
        key: String,
        count: i64,
        value: Vec<u8>,
    ) -> super::types::Result<()> {
        if !self.config.enabled || !self.config.wal.enabled {
            return Ok(());
        }

        let operation = Operation::ListRem { key, count, value };

        self.wal.append(operation).await?;

        let mut ops = self.operations_since_snapshot.write();
        *ops += 1;

        Ok(())
    }

    /// Log a List INSERT operation
    pub async fn log_list_insert(
        &self,
        key: String,
        before: bool,
        pivot: Vec<u8>,
        value: Vec<u8>,
    ) -> super::types::Result<()> {
        if !self.config.enabled || !self.config.wal.enabled {
            return Ok(());
        }

        let operation = Operation::ListInsert {
            key,
            before,
            pivot,
            value,
        };

        self.wal.append(operation).await?;

        let mut ops = self.operations_since_snapshot.write();
        *ops += 1;

        Ok(())
    }

    /// Log a List RPOPLPUSH operation
    pub async fn log_list_rpoplpush(
        &self,
        source: String,
        destination: String,
    ) -> super::types::Result<()> {
        if !self.config.enabled || !self.config.wal.enabled {
            return Ok(());
        }

        let operation = Operation::ListRpoplpush {
            source,
            destination,
        };

        self.wal.append(operation).await?;

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

        self.wal.append(operation).await?;

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

        self.wal.append(operation).await?;

        Ok(())
    }

    /// Log a Stream PUBLISH operation
    pub async fn log_stream_publish(
        &self,
        room: String,
        event_type: String,
        payload: Vec<u8>,
    ) -> super::types::Result<()> {
        if !self.config.enabled || !self.config.wal.enabled {
            return Ok(());
        }

        let operation = Operation::StreamPublish {
            room,
            event_type,
            payload,
        };

        self.wal.append(operation).await?;

        let mut ops = self.operations_since_snapshot.write();
        *ops += 1;

        Ok(())
    }

    /// Create a snapshot if conditions are met
    pub async fn maybe_snapshot(
        &self,
        kv_store: &KVStore,
        queue_manager: Option<&QueueManager>,
        stream_manager: Option<&StreamManager>,
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

            let wal_offset = self.wal.current_offset();

            self.snapshot_mgr
                .create_snapshot(kv_store, queue_manager, stream_manager, wal_offset)
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
        stream_manager: Option<Arc<StreamManager>>,
    ) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(60)); // Check every minute

            loop {
                interval.tick().await;

                if let Err(e) = self
                    .maybe_snapshot(
                        &kv_store,
                        queue_manager.as_ref().map(|q| q.as_ref()),
                        stream_manager.as_ref().map(|s| s.as_ref()),
                    )
                    .await
                {
                    tracing::error!("Snapshot failed: {}", e);
                }
            }
        })
    }

    /// Log a Set ADD operation (SADD)
    pub async fn log_set_add(
        &self,
        key: String,
        members: Vec<Vec<u8>>,
    ) -> super::types::Result<()> {
        if !self.config.enabled || !self.config.wal.enabled {
            return Ok(());
        }

        let operation = Operation::SetAdd { key, members };
        self.wal.append(operation).await?;

        let mut ops = self.operations_since_snapshot.write();
        *ops += 1;

        Ok(())
    }

    /// Log a Set REMOVE operation (SREM)
    pub async fn log_set_rem(
        &self,
        key: String,
        members: Vec<Vec<u8>>,
    ) -> super::types::Result<()> {
        if !self.config.enabled || !self.config.wal.enabled {
            return Ok(());
        }

        let operation = Operation::SetRem { key, members };
        self.wal.append(operation).await?;

        let mut ops = self.operations_since_snapshot.write();
        *ops += 1;

        Ok(())
    }

    /// Log a Set MOVE operation (SMOVE)
    pub async fn log_set_move(
        &self,
        source: String,
        destination: String,
        member: Vec<u8>,
    ) -> super::types::Result<()> {
        if !self.config.enabled || !self.config.wal.enabled {
            return Ok(());
        }

        let operation = Operation::SetMove {
            source,
            destination,
            member,
        };
        self.wal.append(operation).await?;

        let mut ops = self.operations_since_snapshot.write();
        *ops += 1;

        Ok(())
    }

    /// Log a Set INTER STORE operation (SINTERSTORE)
    pub async fn log_set_interstore(
        &self,
        destination: String,
        keys: Vec<String>,
    ) -> super::types::Result<()> {
        if !self.config.enabled || !self.config.wal.enabled {
            return Ok(());
        }

        let operation = Operation::SetInterStore { destination, keys };
        self.wal.append(operation).await?;

        let mut ops = self.operations_since_snapshot.write();
        *ops += 1;

        Ok(())
    }

    /// Log a Set UNION STORE operation (SUNIONSTORE)
    pub async fn log_set_unionstore(
        &self,
        destination: String,
        keys: Vec<String>,
    ) -> super::types::Result<()> {
        if !self.config.enabled || !self.config.wal.enabled {
            return Ok(());
        }

        let operation = Operation::SetUnionStore { destination, keys };
        self.wal.append(operation).await?;

        let mut ops = self.operations_since_snapshot.write();
        *ops += 1;

        Ok(())
    }

    /// Log a Set DIFF STORE operation (SDIFFSTORE)
    pub async fn log_set_diffstore(
        &self,
        destination: String,
        keys: Vec<String>,
    ) -> super::types::Result<()> {
        if !self.config.enabled || !self.config.wal.enabled {
            return Ok(());
        }

        let operation = Operation::SetDiffStore { destination, keys };
        self.wal.append(operation).await?;

        let mut ops = self.operations_since_snapshot.write();
        *ops += 1;

        Ok(())
    }

    /// No explicit flush needed with AsyncWAL (group commit handles it)
    pub async fn flush(&self) -> super::types::Result<()> {
        // AsyncWAL handles batching and flushing automatically
        Ok(())
    }
}
