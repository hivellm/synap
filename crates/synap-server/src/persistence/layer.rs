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
    /// WAL sink — present only when persistence + WAL are enabled. A layer built
    /// purely to propagate to replicas (persistence disabled) carries `None`, so
    /// no WAL file is created (phase6j decoupling).
    wal: Option<Arc<AsyncWAL>>,
    snapshot_mgr: Arc<SnapshotManager>,
    config: PersistenceConfig,
    last_snapshot: Arc<RwLock<Instant>>,
    operations_since_snapshot: Arc<RwLock<usize>>,
    /// When set (master role), every recorded operation is also propagated to
    /// connected replicas (audit M-005). Propagation is decoupled from the WAL
    /// so a master replicates even when persistence is disabled (phase6j).
    replication_master: Option<Arc<crate::replication::MasterNode>>,
}

impl PersistenceLayer {
    /// Create a new persistence layer with AsyncWAL (no replication).
    pub async fn new(config: PersistenceConfig) -> super::types::Result<Self> {
        Self::new_with_replication(config, None).await
    }

    /// Create a persistence layer that also propagates each recorded operation to
    /// replicas through the given master node.
    ///
    /// The WAL is opened only when persistence and WAL are both enabled. When the
    /// layer exists solely to feed a replication master (persistence disabled),
    /// no WAL file is created — operations are still propagated to replicas.
    pub async fn new_with_replication(
        config: PersistenceConfig,
        replication_master: Option<Arc<crate::replication::MasterNode>>,
    ) -> super::types::Result<Self> {
        let wal = if config.enabled && config.wal.enabled {
            Some(Arc::new(AsyncWAL::open(config.wal.clone()).await?))
        } else {
            None
        };
        let snapshot_mgr = SnapshotManager::new(config.snapshot.clone());

        Ok(Self {
            wal,
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

    /// Record an operation: propagate to replicas (always) and append to the WAL
    /// (only when a WAL sink is present).
    ///
    /// This is the single shared hook behind every `log_*` method. Propagation
    /// happens first and unconditionally so replication does not depend on
    /// persistence being enabled (phase6j); WAL logging follows when durable
    /// persistence is active.
    async fn record(&self, operation: Operation) -> super::types::Result<()> {
        self.maybe_replicate(&operation);

        if let Some(wal) = &self.wal {
            wal.append(operation).await?;
            *self.operations_since_snapshot.write() += 1;
        }

        Ok(())
    }

    /// Log an entire committed transaction as one atomic unit (audit M-010).
    ///
    /// Each [`CommittedWrite`](crate::core::CommittedWrite) is mapped to its
    /// persistence [`Operation`], every op is propagated to replicas (so the
    /// transaction is replicated, decoupled from the WAL), and the whole set is
    /// appended to the WAL as a single atomic batch — so a MULTI/EXEC survives a
    /// crash all-or-nothing rather than as interleavable single appends. With
    /// persistence disabled the ops are still replicated (no WAL append).
    pub async fn log_transaction(
        &self,
        writes: &[crate::core::CommittedWrite],
    ) -> super::types::Result<()> {
        if writes.is_empty() {
            return Ok(());
        }

        let ops: Vec<Operation> = writes
            .iter()
            .map(Self::committed_write_to_operation)
            .collect();

        // Propagate to replicas first (decoupled from the WAL, phase6j).
        for op in &ops {
            self.maybe_replicate(op);
        }

        if let Some(wal) = &self.wal {
            wal.append_batch(ops).await?;
            *self.operations_since_snapshot.write() += writes.len();
        }

        Ok(())
    }

    /// Map a committed transaction effect to its durable persistence operation.
    fn committed_write_to_operation(write: &crate::core::CommittedWrite) -> Operation {
        use crate::core::CommittedWrite as Cw;
        match write {
            Cw::KvSet { key, value, ttl } => Operation::KVSet {
                key: key.clone(),
                value: value.clone(),
                ttl: *ttl,
            },
            Cw::KvDel { keys } => Operation::KVDel { keys: keys.clone() },
            Cw::HashSet { key, field, value } => Operation::HashSet {
                key: key.clone(),
                field: field.clone(),
                value: value.clone(),
            },
            Cw::HashDel { key, fields } => Operation::HashDel {
                key: key.clone(),
                fields: fields.clone(),
            },
            Cw::HashIncrBy { key, field, delta } => Operation::HashIncrBy {
                key: key.clone(),
                field: field.clone(),
                increment: *delta,
            },
            Cw::ListPush { key, values, left } => Operation::ListPush {
                key: key.clone(),
                values: values.clone(),
                left: *left,
            },
            Cw::ListPop { key, left } => Operation::ListPop {
                key: key.clone(),
                count: 1,
                left: *left,
            },
            Cw::SetAdd { key, members } => Operation::SetAdd {
                key: key.clone(),
                members: members.clone(),
            },
            Cw::SetRem { key, members } => Operation::SetRem {
                key: key.clone(),
                members: members.clone(),
            },
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
        self.record(Operation::KVSet { key, value, ttl }).await
    }

    /// Log a KV DELETE operation
    pub async fn log_kv_del(&self, keys: Vec<String>) -> super::types::Result<()> {
        self.record(Operation::KVDel { keys }).await
    }

    /// Log a KV RENAME operation
    pub async fn log_kv_rename(
        &self,
        source: String,
        destination: String,
    ) -> super::types::Result<()> {
        self.record(Operation::KVRename {
            source,
            destination,
        })
        .await
    }

    /// Log a Hash SET operation
    pub async fn log_hash_set(
        &self,
        key: String,
        field: String,
        value: Vec<u8>,
    ) -> super::types::Result<()> {
        self.record(Operation::HashSet { key, field, value }).await
    }

    /// Log a Hash DELETE operation
    pub async fn log_hash_del(&self, key: String, fields: Vec<String>) -> super::types::Result<()> {
        self.record(Operation::HashDel { key, fields }).await
    }

    /// Log a Hash INCREMENT operation
    pub async fn log_hash_incrby(
        &self,
        key: String,
        field: String,
        increment: i64,
    ) -> super::types::Result<()> {
        self.record(Operation::HashIncrBy {
            key,
            field,
            increment,
        })
        .await
    }

    /// Log a Hash INCREMENT BY FLOAT operation
    pub async fn log_hash_incrbyfloat(
        &self,
        key: String,
        field: String,
        increment: f64,
    ) -> super::types::Result<()> {
        self.record(Operation::HashIncrByFloat {
            key,
            field,
            increment,
        })
        .await
    }

    /// Log a List PUSH operation
    pub async fn log_list_push(
        &self,
        key: String,
        values: Vec<Vec<u8>>,
        left: bool,
    ) -> super::types::Result<()> {
        self.record(Operation::ListPush { key, values, left }).await
    }

    /// Log a List POP operation
    pub async fn log_list_pop(
        &self,
        key: String,
        count: usize,
        left: bool,
    ) -> super::types::Result<()> {
        self.record(Operation::ListPop { key, count, left }).await
    }

    /// Log a List SET operation
    pub async fn log_list_set(
        &self,
        key: String,
        index: i64,
        value: Vec<u8>,
    ) -> super::types::Result<()> {
        self.record(Operation::ListSet { key, index, value }).await
    }

    /// Log a List TRIM operation
    pub async fn log_list_trim(
        &self,
        key: String,
        start: i64,
        stop: i64,
    ) -> super::types::Result<()> {
        self.record(Operation::ListTrim { key, start, stop }).await
    }

    /// Log a List REMOVE operation
    pub async fn log_list_rem(
        &self,
        key: String,
        count: i64,
        value: Vec<u8>,
    ) -> super::types::Result<()> {
        self.record(Operation::ListRem { key, count, value }).await
    }

    /// Log a List INSERT operation
    pub async fn log_list_insert(
        &self,
        key: String,
        before: bool,
        pivot: Vec<u8>,
        value: Vec<u8>,
    ) -> super::types::Result<()> {
        self.record(Operation::ListInsert {
            key,
            before,
            pivot,
            value,
        })
        .await
    }

    /// Log a List RPOPLPUSH operation
    pub async fn log_list_rpoplpush(
        &self,
        source: String,
        destination: String,
    ) -> super::types::Result<()> {
        self.record(Operation::ListRpoplpush {
            source,
            destination,
        })
        .await
    }

    /// Log a Queue PUBLISH operation
    pub async fn log_queue_publish(
        &self,
        queue: String,
        message: crate::core::queue::QueueMessage,
    ) -> super::types::Result<()> {
        self.record(Operation::QueuePublish { queue, message })
            .await
    }

    /// Log a Queue ACK operation
    pub async fn log_queue_ack(
        &self,
        queue: String,
        message_id: String,
    ) -> super::types::Result<()> {
        self.record(Operation::QueueAck { queue, message_id }).await
    }

    /// Log a Queue NACK operation
    pub async fn log_queue_nack(
        &self,
        queue: String,
        message_id: String,
        requeue: bool,
    ) -> super::types::Result<()> {
        self.record(Operation::QueueNack {
            queue,
            message_id,
            requeue,
        })
        .await
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

            // A WAL-less layer (persistence disabled) has no offset to record.
            let wal_offset = self.wal.as_ref().map(|w| w.current_offset()).unwrap_or(0);

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
        self.record(Operation::SetAdd { key, members }).await
    }

    /// Log a Set REMOVE operation (SREM)
    pub async fn log_set_rem(
        &self,
        key: String,
        members: Vec<Vec<u8>>,
    ) -> super::types::Result<()> {
        self.record(Operation::SetRem { key, members }).await
    }

    /// Log a Set MOVE operation (SMOVE)
    pub async fn log_set_move(
        &self,
        source: String,
        destination: String,
        member: Vec<u8>,
    ) -> super::types::Result<()> {
        self.record(Operation::SetMove {
            source,
            destination,
            member,
        })
        .await
    }

    /// Log a Set INTER STORE operation (SINTERSTORE)
    pub async fn log_set_interstore(
        &self,
        destination: String,
        keys: Vec<String>,
    ) -> super::types::Result<()> {
        self.record(Operation::SetInterStore { destination, keys })
            .await
    }

    /// Log a Set UNION STORE operation (SUNIONSTORE)
    pub async fn log_set_unionstore(
        &self,
        destination: String,
        keys: Vec<String>,
    ) -> super::types::Result<()> {
        self.record(Operation::SetUnionStore { destination, keys })
            .await
    }

    /// Log a Set DIFF STORE operation (SDIFFSTORE)
    pub async fn log_set_diffstore(
        &self,
        destination: String,
        keys: Vec<String>,
    ) -> super::types::Result<()> {
        self.record(Operation::SetDiffStore { destination, keys })
            .await
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
        self.record(Operation::ZAdd {
            key,
            member,
            score,
            nx,
            xx,
            gt,
            lt,
        })
        .await
    }

    /// Log a Sorted Set REMOVE operation (ZREM)
    pub async fn log_zrem(&self, key: String, members: Vec<Vec<u8>>) -> super::types::Result<()> {
        self.record(Operation::ZRem { key, members }).await
    }

    /// Log a Sorted Set INCREMENT BY operation (ZINCRBY)
    pub async fn log_zincrby(
        &self,
        key: String,
        member: Vec<u8>,
        increment: f64,
    ) -> super::types::Result<()> {
        self.record(Operation::ZIncrBy {
            key,
            member,
            increment,
        })
        .await
    }

    /// Log a Sorted Set REMOVE RANGE BY RANK operation (ZREMRANGEBYRANK)
    pub async fn log_zremrangebyrank(
        &self,
        key: String,
        start: i64,
        stop: i64,
    ) -> super::types::Result<()> {
        self.record(Operation::ZRemRangeByRank { key, start, stop })
            .await
    }

    /// Log a Sorted Set REMOVE RANGE BY SCORE operation (ZREMRANGEBYSCORE)
    pub async fn log_zremrangebyscore(
        &self,
        key: String,
        min: f64,
        max: f64,
    ) -> super::types::Result<()> {
        self.record(Operation::ZRemRangeByScore { key, min, max })
            .await
    }

    /// Log a Sorted Set INTER STORE operation (ZINTERSTORE)
    pub async fn log_zinterstore(
        &self,
        destination: String,
        keys: Vec<String>,
        weights: Option<Vec<f64>>,
        aggregate: String,
    ) -> super::types::Result<()> {
        self.record(Operation::ZInterStore {
            destination,
            keys,
            weights,
            aggregate,
        })
        .await
    }

    /// Log a Sorted Set UNION STORE operation (ZUNIONSTORE)
    pub async fn log_zunionstore(
        &self,
        destination: String,
        keys: Vec<String>,
        weights: Option<Vec<f64>>,
        aggregate: String,
    ) -> super::types::Result<()> {
        self.record(Operation::ZUnionStore {
            destination,
            keys,
            weights,
            aggregate,
        })
        .await
    }

    /// Log a Sorted Set DIFF STORE operation (ZDIFFSTORE)
    pub async fn log_zdiffstore(
        &self,
        destination: String,
        keys: Vec<String>,
    ) -> super::types::Result<()> {
        self.record(Operation::ZDiffStore { destination, keys })
            .await
    }

    /// No explicit flush needed with AsyncWAL (group commit handles it)
    pub async fn flush(&self) -> super::types::Result<()> {
        // AsyncWAL handles batching and flushing automatically
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{CommittedWrite, KVConfig, QueueConfig};
    use std::path::PathBuf;

    fn all_committed_writes() -> Vec<CommittedWrite> {
        vec![
            CommittedWrite::KvSet {
                key: "k".into(),
                value: b"v".to_vec(),
                ttl: Some(60),
            },
            CommittedWrite::KvDel {
                keys: vec!["k2".into()],
            },
            CommittedWrite::HashSet {
                key: "h".into(),
                field: "f".into(),
                value: b"hv".to_vec(),
            },
            CommittedWrite::HashDel {
                key: "h".into(),
                fields: vec!["f2".into()],
            },
            CommittedWrite::HashIncrBy {
                key: "h".into(),
                field: "n".into(),
                delta: 2,
            },
            CommittedWrite::ListPush {
                key: "l".into(),
                values: vec![b"a".to_vec()],
                left: true,
            },
            CommittedWrite::ListPop {
                key: "l".into(),
                left: false,
            },
            CommittedWrite::SetAdd {
                key: "s".into(),
                members: vec![b"m".to_vec()],
            },
            CommittedWrite::SetRem {
                key: "s".into(),
                members: vec![b"m".to_vec()],
            },
        ]
    }

    /// Every `log_*` method routes through `record()` (propagate + WAL append).
    /// Exercises the full delegating surface refactored in phase6j.
    #[tokio::test]
    async fn all_log_methods_route_through_record() {
        let dir = "./target/layer_logall_test";
        let _ = std::fs::remove_dir_all(dir);
        std::fs::create_dir_all(format!("{dir}/snap")).unwrap();

        let mut config = PersistenceConfig::default();
        config.enabled = true;
        config.wal.enabled = true;
        config.wal.path = PathBuf::from(format!("{dir}/wal.log"));
        config.snapshot.enabled = false;
        config.snapshot.directory = PathBuf::from(format!("{dir}/snap"));
        let layer = PersistenceLayer::new(config).await.unwrap();

        assert!(!layer.is_sync_durability()); // default Periodic fsync

        layer
            .log_kv_set("k".into(), b"v".to_vec(), None)
            .await
            .unwrap();
        layer.log_kv_del(vec!["k".into()]).await.unwrap();
        layer.log_kv_rename("a".into(), "b".into()).await.unwrap();
        layer
            .log_hash_set("h".into(), "f".into(), b"v".to_vec())
            .await
            .unwrap();
        layer
            .log_hash_del("h".into(), vec!["f".into()])
            .await
            .unwrap();
        layer
            .log_hash_incrby("h".into(), "n".into(), 1)
            .await
            .unwrap();
        layer
            .log_hash_incrbyfloat("h".into(), "n".into(), 1.5)
            .await
            .unwrap();
        layer
            .log_list_push("l".into(), vec![b"a".to_vec()], true)
            .await
            .unwrap();
        layer.log_list_pop("l".into(), 1, false).await.unwrap();
        layer
            .log_list_set("l".into(), 0, b"x".to_vec())
            .await
            .unwrap();
        layer.log_list_trim("l".into(), 0, 1).await.unwrap();
        layer
            .log_list_rem("l".into(), 1, b"x".to_vec())
            .await
            .unwrap();
        layer
            .log_list_insert("l".into(), true, b"p".to_vec(), b"v".to_vec())
            .await
            .unwrap();
        layer
            .log_list_rpoplpush("l".into(), "l2".into())
            .await
            .unwrap();
        let msg = crate::core::queue::QueueMessage::new(b"p".to_vec(), 0, 3);
        layer.log_queue_publish("q".into(), msg).await.unwrap();
        layer.log_queue_ack("q".into(), "id".into()).await.unwrap();
        layer
            .log_queue_nack("q".into(), "id".into(), true)
            .await
            .unwrap();
        layer
            .log_stream_publish("r".into(), "e".into(), b"d".to_vec())
            .await
            .unwrap();
        layer
            .log_set_add("s".into(), vec![b"m".to_vec()])
            .await
            .unwrap();
        layer
            .log_set_rem("s".into(), vec![b"m".to_vec()])
            .await
            .unwrap();
        layer
            .log_set_move("s".into(), "s2".into(), b"m".to_vec())
            .await
            .unwrap();
        layer
            .log_set_interstore("d".into(), vec!["s".into()])
            .await
            .unwrap();
        layer
            .log_set_unionstore("d".into(), vec!["s".into()])
            .await
            .unwrap();
        layer
            .log_set_diffstore("d".into(), vec!["s".into()])
            .await
            .unwrap();
        layer
            .log_zadd("z".into(), b"m".to_vec(), 1.0, false, false, false, false)
            .await
            .unwrap();
        layer
            .log_zrem("z".into(), vec![b"m".to_vec()])
            .await
            .unwrap();
        layer
            .log_zincrby("z".into(), b"m".to_vec(), 1.0)
            .await
            .unwrap();
        layer.log_zremrangebyrank("z".into(), 0, 1).await.unwrap();
        layer
            .log_zremrangebyscore("z".into(), 0.0, 1.0)
            .await
            .unwrap();
        layer
            .log_zinterstore("d".into(), vec!["z".into()], None, "sum".into())
            .await
            .unwrap();
        layer
            .log_zunionstore("d".into(), vec!["z".into()], Some(vec![1.0]), "max".into())
            .await
            .unwrap();
        layer
            .log_zdiffstore("d".into(), vec!["z".into()])
            .await
            .unwrap();
        layer.flush().await.unwrap();

        let _ = std::fs::remove_dir_all(dir);
    }

    /// `log_transaction` maps every `CommittedWrite` variant to an operation. With
    /// persistence disabled it takes the no-WAL path (still succeeds).
    #[tokio::test]
    async fn log_transaction_maps_all_variants_without_wal() {
        let mut config = PersistenceConfig::default();
        config.enabled = false;
        let layer = PersistenceLayer::new(config).await.unwrap();

        // Empty batch is a no-op.
        layer.log_transaction(&[]).await.unwrap();
        // All variants map cleanly.
        layer
            .log_transaction(&all_committed_writes())
            .await
            .unwrap();
    }

    /// A transaction logged with WAL enabled is recovered as its effects.
    #[tokio::test]
    async fn log_transaction_is_durable_and_recovers() {
        let dir = "./target/layer_txn_test";
        let _ = std::fs::remove_dir_all(dir);
        std::fs::create_dir_all(format!("{dir}/snap")).unwrap();

        let mut config = PersistenceConfig::default();
        config.enabled = true;
        config.wal.enabled = true;
        config.wal.fsync_mode = FsyncMode::Always;
        config.wal.path = PathBuf::from(format!("{dir}/wal.log"));
        config.snapshot.enabled = false;
        config.snapshot.directory = PathBuf::from(format!("{dir}/snap"));

        {
            let layer = PersistenceLayer::new(config.clone()).await.unwrap();
            layer
                .log_transaction(&[
                    CommittedWrite::KvSet {
                        key: "tk".into(),
                        value: b"tv".to_vec(),
                        ttl: None,
                    },
                    CommittedWrite::HashSet {
                        key: "th".into(),
                        field: "f".into(),
                        value: b"hv".to_vec(),
                    },
                ])
                .await
                .unwrap();
            drop(layer);
            tokio::time::sleep(std::time::Duration::from_millis(150)).await;
        }

        let (kv, hash, _l, _s, _z, _q, _off) =
            crate::persistence::recover(&config, KVConfig::default(), QueueConfig::default())
                .await
                .unwrap();
        assert_eq!(kv.get("tk").await.unwrap(), Some(b"tv".to_vec()));
        assert_eq!(hash.unwrap().hget("th", "f").unwrap(), Some(b"hv".to_vec()));

        let _ = std::fs::remove_dir_all(dir);
    }
}
