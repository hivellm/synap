use super::apply::{StoreArcs, StoreRefs};
use super::order::{StripeGuards, WriteOrder};
use super::snapshot::SnapshotImage;
use super::types::{FsyncMode, Operation, PersistenceConfig, PersistenceError};
use super::wal_async::WalAck;
use super::{AsyncWAL, SnapshotManager};
use crate::core::sorted_set::ZAddOptions;
use parking_lot::RwLock;
use std::future::Future;
use std::path::PathBuf;
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
    /// Per-key ordering lock and snapshot barrier (phase23): writers hold
    /// their keys' stripes from applying a write until its record is queued,
    /// a snapshot holds every stripe while it records its cutoff and copies
    /// the stores.
    order: WriteOrder,
}

/// A record handed to replication and to the WAL writer queue, not yet
/// durable. [`Submitted::wait`] waits for the WAL group commit.
#[must_use = "a submitted record is durable only once `wait` succeeds"]
pub struct Submitted {
    ack: Option<WalAck>,
}

impl Submitted {
    /// Wait until the record reached the WAL (and the disk, per the fsync
    /// mode). Immediate without a WAL.
    pub async fn wait(self) -> super::types::Result<()> {
        match self.ack {
            Some(ack) => ack.wait().await.map(|_| ()),
            None => Ok(()),
        }
    }
}

/// The ordering stripes of one write's keys, held from before the write is
/// applied until its record is queued (see [`PersistenceLayer::order`]).
pub struct Ordered<'a> {
    layer: &'a PersistenceLayer,
    _stripes: StripeGuards<'a>,
}

impl Ordered<'_> {
    /// Queue `operation` while the stripes are still held, release them, then
    /// wait for the record to be durable.
    pub async fn commit(self, operation: Operation) -> super::types::Result<()> {
        let submitted = self.layer.submit(operation, 1)?;
        drop(self);
        submitted.wait().await
    }

    /// [`commit`](Self::commit) for a command that produced several effects:
    /// they are queued as ONE record (see [`PersistenceLayer::log_operations`]).
    pub async fn commit_all(self, operations: Vec<Operation>) -> super::types::Result<()> {
        let submitted = self.layer.submit_all(operations)?;
        drop(self);
        submitted.wait().await
    }

    /// [`commit`](Self::commit) for a committed MULTI/EXEC transaction.
    pub async fn commit_transaction(
        self,
        writes: &[crate::core::CommittedWrite],
    ) -> super::types::Result<()> {
        let submitted = self.layer.submit_transaction(writes)?;
        drop(self);
        submitted.wait().await
    }
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
            Some(AsyncWAL::open(config.wal.clone()).await?)
        } else {
            None
        };
        Ok(Self::with_wal(config, wal, replication_master))
    }

    /// Build a layer around an already-open WAL (or none). This is the seam for
    /// a WAL opened with [`AsyncWAL::open_with_sink`], e.g. to inject flush and
    /// fsync failures in tests.
    pub fn with_wal(
        config: PersistenceConfig,
        wal: Option<AsyncWAL>,
        replication_master: Option<Arc<crate::replication::MasterNode>>,
    ) -> Self {
        let snapshot_mgr = SnapshotManager::new(config.snapshot.clone());

        Self {
            wal: wal.map(Arc::new),
            snapshot_mgr: Arc::new(snapshot_mgr),
            config,
            last_snapshot: Arc::new(RwLock::new(Instant::now())),
            operations_since_snapshot: Arc::new(RwLock::new(0)),
            replication_master,
            order: WriteOrder::new(),
        }
    }

    /// Lock the ordering stripes of `keys` for one write.
    ///
    /// Take this BEFORE applying the write and finish with
    /// [`Ordered::commit`]: the record is then queued for replication and the
    /// WAL before any later write to the same keys, so both see writes to a
    /// key in the order memory applied them, and a snapshot can never capture
    /// the write's effect while its record lands after the snapshot's cutoff.
    pub async fn order<'k>(&self, keys: impl IntoIterator<Item = &'k str>) -> Ordered<'_> {
        Ordered {
            layer: self,
            _stripes: self.order.lock(keys).await,
        }
    }

    /// Lock every ordering stripe for a write that touches every key (FLUSHDB):
    /// it is ordered against all writers and against snapshots as a whole.
    pub async fn order_all(&self) -> Ordered<'_> {
        Ordered {
            layer: self,
            _stripes: self.order.lock_all().await,
        }
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
        self.submit(operation, 1)?.wait().await
    }

    /// Hand one record (standing for `count` operations) to replication and to
    /// the WAL writer queue, synchronously, without waiting for durability.
    fn submit(&self, operation: Operation, count: usize) -> super::types::Result<Submitted> {
        self.maybe_replicate(&operation);
        let ack = match &self.wal {
            Some(wal) => {
                let ack = wal.enqueue(operation)?;
                *self.operations_since_snapshot.write() += count;
                Some(ack)
            }
            None => None,
        };
        Ok(Submitted { ack })
    }

    /// Submit the effects of one command: several operations become ONE
    /// [`Operation::Transaction`] record; one operation is submitted as itself.
    fn submit_all(&self, mut operations: Vec<Operation>) -> super::types::Result<Submitted> {
        match operations.len() {
            0 => Ok(Submitted { ack: None }),
            1 => self.submit(operations.remove(0), 1),
            count => self.submit(Operation::Transaction { operations }, count),
        }
    }

    /// Submit a committed transaction as ONE [`Operation::Transaction`] record.
    fn submit_transaction(
        &self,
        writes: &[crate::core::CommittedWrite],
    ) -> super::types::Result<Submitted> {
        if writes.is_empty() {
            return Ok(Submitted { ack: None });
        }
        let operations: Vec<Operation> = writes
            .iter()
            .map(Self::committed_write_to_operation)
            .collect();
        self.submit(Operation::Transaction { operations }, writes.len())
    }

    /// Record the effect of one write that has already been applied in memory:
    /// propagate it to replicas and append it to the WAL. This is what the
    /// shared mutation path ([`super::durable`]) calls for every transport.
    pub async fn log_operation(&self, operation: Operation) -> super::types::Result<()> {
        self.record(operation).await
    }

    /// Record the effects of one command that produced several operations
    /// (MSET, HMSET, GEOADD, ...) as ONE [`Operation::Transaction`] record, so
    /// replicas and WAL replay apply the command whole or not at all. A single
    /// operation is recorded as itself.
    pub async fn log_operations(&self, operations: Vec<Operation>) -> super::types::Result<()> {
        self.submit_all(operations)?.wait().await
    }

    /// Log an entire committed transaction as one atomic unit (audit M-010,
    /// phase22).
    ///
    /// Each [`CommittedWrite`](crate::core::CommittedWrite) is mapped to its
    /// persistence [`Operation`] and the set is wrapped in ONE
    /// [`Operation::Transaction`] record. That record is propagated to replicas
    /// as a single replication command with a single offset (decoupled from the
    /// WAL), so a replica applies the transaction whole or not at all, and is
    /// appended to the WAL as a single frame under one checksum, so a crash
    /// recovers every effect or none. With persistence disabled the record is
    /// still replicated (no WAL append).
    pub async fn log_transaction(
        &self,
        writes: &[crate::core::CommittedWrite],
    ) -> super::types::Result<()> {
        self.submit_transaction(writes)?.wait().await
    }

    /// Map a committed transaction effect to its durable persistence operation.
    fn committed_write_to_operation(write: &crate::core::CommittedWrite) -> Operation {
        use crate::core::CommittedWrite as Cw;
        match write {
            Cw::KvSet {
                key,
                value,
                expires_at_ms: None,
            } => Operation::KVSet {
                key: key.clone(),
                value: value.clone(),
                ttl: None,
            },
            Cw::KvSet {
                key,
                value,
                expires_at_ms,
            } => Operation::KVSetAt {
                key: key.clone(),
                value: value.clone(),
                expires_at_ms: *expires_at_ms,
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

    /// Log a Queue PUBLISH operation with the whole message (creation time
    /// included), so recovery and replicas restore it with its original ID
    /// and metadata.
    pub async fn log_queue_publish(
        &self,
        queue: String,
        message: crate::core::queue::QueueMessage,
    ) -> super::types::Result<()> {
        self.record(Operation::QueuePublishFull {
            queue,
            message: super::types::QueueMessageRecord::from(&message),
        })
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
    pub async fn maybe_snapshot(&self, stores: StoreRefs<'_>) -> super::types::Result<()> {
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
            self.snapshot(stores).await?;
        }

        Ok(())
    }

    /// Write a snapshot now, paired exactly with its WAL cutoff.
    ///
    /// Under the write barrier (every ordering stripe held, so no write is
    /// between being applied and being queued) it places a mark in the WAL
    /// queue and copies the stores; then it releases the barrier, learns the
    /// cutoff from the mark, and serializes outside the barrier. Every write
    /// is either in the image (queued before the mark, offset below the
    /// cutoff, not replayed) or after the cutoff (not in the image, replayed),
    /// never both. Without a WAL the cutoff is 0.
    pub async fn snapshot(&self, stores: StoreRefs<'_>) -> super::types::Result<PathBuf> {
        self.snapshot_with_pause(stores, std::future::ready(()))
            .await
    }

    /// [`snapshot`](Self::snapshot) that awaits `at_cutoff` while the barrier
    /// holds, right after the cutoff is recorded and before the stores are
    /// copied. A test seam: writers issued meanwhile wait for the barrier, so
    /// they land after the cutoff and outside the image.
    pub async fn snapshot_with_pause(
        &self,
        stores: StoreRefs<'_>,
        at_cutoff: impl Future<Output = ()>,
    ) -> super::types::Result<PathBuf> {
        let started = Instant::now();
        let (image, mark) = {
            let _barrier = self.order.lock_all().await;
            let mark = self.wal.as_ref().map(|wal| wal.mark()).transpose()?;
            at_cutoff.await;
            let image = SnapshotImage::capture(stores).await?;
            *self.operations_since_snapshot.write() = 0;
            (image, mark)
        };
        let barrier_held = started.elapsed();
        let wal_offset = match mark {
            Some(mark) => mark.offset().await?,
            None => 0,
        };
        let path = self
            .snapshot_mgr
            .write_image(image, stores.stream_manager, wal_offset)
            .await?;
        *self.last_snapshot.write() = Instant::now();
        info!(
            "Snapshot {:?} at WAL offset {} (writes paused {:?}, total {:?})",
            path,
            wal_offset,
            barrier_held,
            started.elapsed()
        );
        Ok(path)
    }

    /// Start background snapshot task
    pub fn start_snapshot_task(self: Arc<Self>, stores: StoreArcs) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(60)); // Check every minute

            loop {
                interval.tick().await;

                if let Err(e) = self.maybe_snapshot(stores.as_refs()).await {
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

    /// Log a Sorted Set ADD operation (ZADD). Only the conditional flags
    /// (`nx`/`xx`/`gt`/`lt`) are persisted; `ch`/`incr` shape the reply, not
    /// the stored state.
    pub async fn log_zadd(
        &self,
        key: String,
        member: Vec<u8>,
        score: f64,
        opts: &ZAddOptions,
    ) -> super::types::Result<()> {
        self.record(Operation::ZAdd {
            key,
            member,
            score,
            nx: opts.nx,
            xx: opts.xx,
            gt: opts.gt,
            lt: opts.lt,
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

    /// Wait until every operation logged before this call has reached the OS,
    /// and stable storage too unless the fsync mode is `Never`. Fails when the
    /// WAL is poisoned. A no-op without a WAL.
    pub async fn flush(&self) -> super::types::Result<()> {
        match &self.wal {
            Some(wal) => wal.flush().await,
            None => Ok(()),
        }
    }

    /// Graceful-shutdown drain: flush and fsync every logged operation, then
    /// stop the WAL writer so nothing logged afterwards is acknowledged.
    /// A no-op without a WAL.
    pub async fn shutdown(&self) -> super::types::Result<()> {
        match &self.wal {
            Some(wal) => wal.shutdown().await,
            None => Ok(()),
        }
    }

    /// Why the WAL writer is poisoned (a write, flush or fsync failed), or
    /// `None` while it is healthy or there is no WAL. A poisoned WAL refuses
    /// every write until the server restarts.
    pub fn wal_poison_reason(&self) -> Option<String> {
        self.wal.as_ref().and_then(|wal| wal.poison_reason())
    }

    /// Whether this layer writes a WAL (persistence and WAL both enabled).
    pub fn has_wal(&self) -> bool {
        self.wal.is_some()
    }
}

/// Error text every EXEC entry point returns when the committed transaction
/// could not be logged. The effects are already applied in memory and are not
/// rolled back; the message says so, because the commit is not durable.
pub fn exec_not_durable_message(err: &PersistenceError) -> String {
    format!(
        "transaction applied in memory but could not be logged to the WAL, so the commit is not durable and the effects are not rolled back: {err}"
    )
}

/// Error text every transport returns when a single write could not be
/// recorded. The write is already applied in memory and is not rolled back;
/// the message says so, because it is not durable.
pub fn write_not_durable_message(err: &PersistenceError) -> String {
    format!(
        "write applied in memory but could not be logged to the WAL, so it is not durable and is not rolled back: {err}"
    )
}

/// Shared test helpers for code that needs a WAL that fails on demand.
#[cfg(test)]
pub(crate) mod test_support {
    use super::*;
    use crate::persistence::wal_async::fault::Faults;
    use std::path::PathBuf;

    /// An Always-mode, WAL-backed layer whose sink fails the next flush or
    /// fsync when told to through the returned [`Faults`].
    pub(crate) async fn faulty_layer(name: &str) -> (Arc<PersistenceLayer>, Faults) {
        let dir = format!("./target/faulty_layer_{name}");
        let _ = std::fs::remove_dir_all(&dir);
        let mut config = PersistenceConfig::default();
        config.wal.fsync_mode = FsyncMode::Always;
        config.wal.path = PathBuf::from(format!("{dir}/wal.log"));
        config.snapshot.enabled = false;
        config.snapshot.directory = PathBuf::from(format!("{dir}/snap"));
        let faults = Faults::default();
        let wrap = faults.clone();
        let wal = AsyncWAL::open_with_sink(config.wal.clone(), move |sink| wrap.wrap(sink))
            .await
            .unwrap();
        let layer = PersistenceLayer::with_wal(config, Some(wal), None);
        (Arc::new(layer), faults)
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
                expires_at_ms: Some(4_102_444_800_000),
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
            .log_zadd("z".into(), b"m".to_vec(), 1.0, &ZAddOptions::default())
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

    /// A transaction whose WAL flush fails returns an error, poisons the WAL,
    /// and every later log call fails too.
    #[tokio::test]
    async fn log_transaction_fails_when_the_wal_flush_fails() {
        let (layer, faults) = test_support::faulty_layer("txn_flush_fail").await;

        faults.fail_next_flush();
        let err = layer
            .log_transaction(&all_committed_writes())
            .await
            .unwrap_err();
        assert!(exec_not_durable_message(&err).contains("not durable"));
        assert!(layer.wal_poison_reason().unwrap().contains("flush failed"));
        assert!(
            layer
                .log_kv_set("k".into(), b"v".to_vec(), None)
                .await
                .is_err()
        );
        assert!(layer.flush().await.is_err());
    }

    /// A master ships a transaction as ONE replication unit: one record at one
    /// replication offset, holding every operation in order.
    #[tokio::test]
    async fn log_transaction_replicates_one_unit() {
        use crate::replication::{MasterNode, NodeRole, ReplicationConfig};

        let mut repl = ReplicationConfig::default();
        repl.enabled = true;
        repl.role = NodeRole::Master;
        repl.replica_listen_address = Some("127.0.0.1:0".parse().unwrap());
        let kv = Arc::new(crate::core::KVStore::new(KVConfig::default()));
        let master = Arc::new(MasterNode::new(repl, kv, None).await.unwrap());

        let mut config = PersistenceConfig::default();
        config.enabled = false;
        let layer = PersistenceLayer::new_with_replication(config, Some(Arc::clone(&master)))
            .await
            .unwrap();
        let writes = all_committed_writes();
        layer.log_transaction(&writes).await.unwrap();

        assert_eq!(master.replication_offset(), 1);
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
                        expires_at_ms: None,
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
