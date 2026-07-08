use super::types::{FsyncMode, Operation, PersistenceConfig};
use super::{AsyncWAL, SnapshotManager};
use crate::core::{
    HashStore, KVStore, ListStore, QueueManager, SetStore, SortedSetStore, StreamManager,
};
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
    /// When set (master role), every logged operation is also propagated to
    /// connected replicas (audit M-005). Replication piggybacks on the
    /// persistence log, so it currently requires persistence to be enabled.
    replication_master: Option<Arc<crate::replication::MasterNode>>,
}

impl PersistenceLayer {
    /// Create a new persistence layer with AsyncWAL (no replication).
    pub async fn new(config: PersistenceConfig) -> super::types::Result<Self> {
        Self::new_with_replication(config, None).await
    }

    /// Create a persistence layer that also propagates each logged operation to
    /// replicas through the given master node.
    pub async fn new_with_replication(
        config: PersistenceConfig,
        replication_master: Option<Arc<crate::replication::MasterNode>>,
    ) -> super::types::Result<Self> {
        let wal = AsyncWAL::open(config.wal.clone()).await?;
        let snapshot_mgr = SnapshotManager::new(config.snapshot.clone());

        Ok(Self {
            wal: Arc::new(wal),
            snapshot_mgr: Arc::new(snapshot_mgr),
            config,
            last_snapshot: Arc::new(RwLock::new(Instant::now())),
            operations_since_snapshot: Arc::new(RwLock::new(0)),
            replication_master,
        })
    }

    /// Propagate an operation to connected replicas when running as master.
    fn maybe_replicate(&self, operation: &Operation) {
        if let Some(master) = &self.replication_master {
            master.replicate(operation.clone());
        }
    }

    /// Returns true when the WAL fsync mode is Always (sync durability).
    ///
    /// When true, the SET handler must log to WAL BEFORE writing to memory.
    /// When false, WAL is written asynchronously after the memory write (current
    /// default behavior, labeled `Periodic` or `Never`).
    pub fn is_sync_durability(&self) -> bool {
        self.config.enabled
            && self.config.wal.enabled
            && self.config.wal.fsync_mode == FsyncMode::Always
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
        self.maybe_replicate(&operation);
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

        self.maybe_replicate(&operation);
        self.wal.append(operation).await?;

        let mut ops = self.operations_since_snapshot.write();
        *ops += 1;

        Ok(())
    }

    /// Log a KV RENAME operation
    pub async fn log_kv_rename(
        &self,
        source: String,
        destination: String,
    ) -> super::types::Result<()> {
        if !self.config.enabled || !self.config.wal.enabled {
            return Ok(());
        }

        let operation = Operation::KVRename {
            source,
            destination,
        };

        self.maybe_replicate(&operation);
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

        self.maybe_replicate(&operation);
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

        self.maybe_replicate(&operation);
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

        self.maybe_replicate(&operation);
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

        self.maybe_replicate(&operation);
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

        self.maybe_replicate(&operation);
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

        self.maybe_replicate(&operation);
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

        self.maybe_replicate(&operation);
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

        self.maybe_replicate(&operation);
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

        self.maybe_replicate(&operation);
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

        self.maybe_replicate(&operation);
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

        self.maybe_replicate(&operation);
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

        self.maybe_replicate(&operation);
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

        self.maybe_replicate(&operation);
        self.wal.append(operation).await?;

        Ok(())
    }

    /// Log a Queue NACK operation
    pub async fn log_queue_nack(
        &self,
        queue: String,
        message_id: String,
        requeue: bool,
    ) -> super::types::Result<()> {
        if !self.config.enabled || !self.config.wal.enabled {
            return Ok(());
        }

        let operation = Operation::QueueNack {
            queue,
            message_id,
            requeue,
        };

        self.maybe_replicate(&operation);
        self.wal.append(operation).await?;

        Ok(())
    }

    /// Stream publishes are intentionally NOT written to the KV WAL.
    ///
    /// Streams are durable through their dedicated `StreamPersistence` and are
    /// captured in periodic snapshots (the v3 stream section). Recovery never
    /// replayed `Operation::StreamPublish` from the KV WAL — logging it here was
    /// dead weight that could diverge from the real stream state (audit M-014).
    /// Kept as a no-op so existing call sites need no change; the WAL variant is
    /// retained only so recovery can skip entries in pre-existing WAL files.
    pub async fn log_stream_publish(
        &self,
        _room: String,
        _event_type: String,
        _payload: Vec<u8>,
    ) -> super::types::Result<()> {
        Ok(())
    }

    /// Create a snapshot if conditions are met
    #[allow(clippy::too_many_arguments)]
    pub async fn maybe_snapshot(
        &self,
        kv_store: &KVStore,
        hash_store: Option<&HashStore>,
        list_store: Option<&ListStore>,
        set_store: Option<&SetStore>,
        sorted_set_store: Option<&SortedSetStore>,
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
                .create_snapshot(
                    kv_store,
                    hash_store,
                    list_store,
                    set_store,
                    sorted_set_store,
                    queue_manager,
                    stream_manager,
                    wal_offset,
                )
                .await?;

            // Reset counters
            *self.last_snapshot.write() = Instant::now();
            *self.operations_since_snapshot.write() = 0;
        }

        Ok(())
    }

    /// Start background snapshot task
    #[allow(clippy::too_many_arguments)]
    pub fn start_snapshot_task(
        self: Arc<Self>,
        kv_store: Arc<KVStore>,
        hash_store: Option<Arc<HashStore>>,
        list_store: Option<Arc<ListStore>>,
        set_store: Option<Arc<SetStore>>,
        sorted_set_store: Option<Arc<SortedSetStore>>,
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
                        hash_store.as_deref(),
                        list_store.as_deref(),
                        set_store.as_deref(),
                        sorted_set_store.as_deref(),
                        queue_manager.as_deref(),
                        stream_manager.as_deref(),
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
        self.maybe_replicate(&operation);
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
        self.maybe_replicate(&operation);
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
        self.maybe_replicate(&operation);
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
        self.maybe_replicate(&operation);
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
        self.maybe_replicate(&operation);
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
        self.maybe_replicate(&operation);
        self.wal.append(operation).await?;

        let mut ops = self.operations_since_snapshot.write();
        *ops += 1;

        Ok(())
    }

    /// Log a Sorted Set ADD operation (ZADD)
    #[allow(clippy::too_many_arguments)]
    pub async fn log_zadd(
        &self,
        key: String,
        member: Vec<u8>,
        score: f64,
        nx: bool,
        xx: bool,
        gt: bool,
        lt: bool,
    ) -> super::types::Result<()> {
        if !self.config.enabled || !self.config.wal.enabled {
            return Ok(());
        }

        let operation = Operation::ZAdd {
            key,
            member,
            score,
            nx,
            xx,
            gt,
            lt,
        };
        self.maybe_replicate(&operation);
        self.wal.append(operation).await?;

        let mut ops = self.operations_since_snapshot.write();
        *ops += 1;

        Ok(())
    }

    /// Log a Sorted Set REMOVE operation (ZREM)
    pub async fn log_zrem(&self, key: String, members: Vec<Vec<u8>>) -> super::types::Result<()> {
        if !self.config.enabled || !self.config.wal.enabled {
            return Ok(());
        }

        let operation = Operation::ZRem { key, members };
        self.maybe_replicate(&operation);
        self.wal.append(operation).await?;

        let mut ops = self.operations_since_snapshot.write();
        *ops += 1;

        Ok(())
    }

    /// Log a Sorted Set INCREMENT BY operation (ZINCRBY)
    pub async fn log_zincrby(
        &self,
        key: String,
        member: Vec<u8>,
        increment: f64,
    ) -> super::types::Result<()> {
        if !self.config.enabled || !self.config.wal.enabled {
            return Ok(());
        }

        let operation = Operation::ZIncrBy {
            key,
            member,
            increment,
        };
        self.maybe_replicate(&operation);
        self.wal.append(operation).await?;

        let mut ops = self.operations_since_snapshot.write();
        *ops += 1;

        Ok(())
    }

    /// Log a Sorted Set REMOVE RANGE BY RANK operation (ZREMRANGEBYRANK)
    pub async fn log_zremrangebyrank(
        &self,
        key: String,
        start: i64,
        stop: i64,
    ) -> super::types::Result<()> {
        if !self.config.enabled || !self.config.wal.enabled {
            return Ok(());
        }

        let operation = Operation::ZRemRangeByRank { key, start, stop };
        self.maybe_replicate(&operation);
        self.wal.append(operation).await?;

        let mut ops = self.operations_since_snapshot.write();
        *ops += 1;

        Ok(())
    }

    /// Log a Sorted Set REMOVE RANGE BY SCORE operation (ZREMRANGEBYSCORE)
    pub async fn log_zremrangebyscore(
        &self,
        key: String,
        min: f64,
        max: f64,
    ) -> super::types::Result<()> {
        if !self.config.enabled || !self.config.wal.enabled {
            return Ok(());
        }

        let operation = Operation::ZRemRangeByScore { key, min, max };
        self.maybe_replicate(&operation);
        self.wal.append(operation).await?;

        let mut ops = self.operations_since_snapshot.write();
        *ops += 1;

        Ok(())
    }

    /// Log a Sorted Set INTER STORE operation (ZINTERSTORE)
    pub async fn log_zinterstore(
        &self,
        destination: String,
        keys: Vec<String>,
        weights: Option<Vec<f64>>,
        aggregate: String,
    ) -> super::types::Result<()> {
        if !self.config.enabled || !self.config.wal.enabled {
            return Ok(());
        }

        let operation = Operation::ZInterStore {
            destination,
            keys,
            weights,
            aggregate,
        };
        self.maybe_replicate(&operation);
        self.wal.append(operation).await?;

        let mut ops = self.operations_since_snapshot.write();
        *ops += 1;

        Ok(())
    }

    /// Log a Sorted Set UNION STORE operation (ZUNIONSTORE)
    pub async fn log_zunionstore(
        &self,
        destination: String,
        keys: Vec<String>,
        weights: Option<Vec<f64>>,
        aggregate: String,
    ) -> super::types::Result<()> {
        if !self.config.enabled || !self.config.wal.enabled {
            return Ok(());
        }

        let operation = Operation::ZUnionStore {
            destination,
            keys,
            weights,
            aggregate,
        };
        self.maybe_replicate(&operation);
        self.wal.append(operation).await?;

        let mut ops = self.operations_since_snapshot.write();
        *ops += 1;

        Ok(())
    }

    /// Log a Sorted Set DIFF STORE operation (ZDIFFSTORE)
    pub async fn log_zdiffstore(
        &self,
        destination: String,
        keys: Vec<String>,
    ) -> super::types::Result<()> {
        if !self.config.enabled || !self.config.wal.enabled {
            return Ok(());
        }

        let operation = Operation::ZDiffStore { destination, keys };
        self.maybe_replicate(&operation);
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
