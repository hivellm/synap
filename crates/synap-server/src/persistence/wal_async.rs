use super::types::{FsyncMode, Operation, PersistenceError, Result, WALConfig, WALEntry};
use super::wal_frame;
use std::future::Future;
use std::io;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant, SystemTime};
use tokio::fs::{File, OpenOptions};
use tokio::io::{AsyncWriteExt, BufWriter};
use tokio::sync::{mpsc, oneshot};
use tokio::time::MissedTickBehavior;
use tracing::{debug, error, info};

/// Byte sink behind the WAL writer.
///
/// Production uses [`FileSink`]; the trait is the seam that lets tests wrap it
/// and inject write, flush and fsync failures.
pub trait WalSink: Send + 'static {
    /// Append `buf` to the sink. Bytes may stay in an application buffer until
    /// [`WalSink::flush`].
    fn write_all(&mut self, buf: &[u8]) -> impl Future<Output = io::Result<()>> + Send;

    /// Hand every buffered byte to the operating system, without fsync.
    fn flush(&mut self) -> impl Future<Output = io::Result<()>> + Send;

    /// Force every byte already handed to the OS onto stable storage (fsync).
    fn sync(&mut self) -> impl Future<Output = io::Result<()>> + Send;
}

/// Production [`WalSink`]: the WAL file behind a `BufWriter`.
pub struct FileSink {
    writer: BufWriter<File>,
}

impl FileSink {
    fn new(file: File, capacity: usize) -> Self {
        Self {
            writer: BufWriter::with_capacity(capacity, file),
        }
    }
}

impl WalSink for FileSink {
    async fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        self.writer.write_all(buf).await
    }

    async fn flush(&mut self) -> io::Result<()> {
        self.writer.flush().await
    }

    async fn sync(&mut self) -> io::Result<()> {
        self.writer.get_ref().sync_all().await
    }
}

/// A request submitted to the background writer.
enum WriteRequest {
    /// A single operation with its completion notification.
    Single {
        operation: Operation,
        response_tx: oneshot::Sender<Result<u64>>,
    },
    /// A group of operations that MUST be written contiguously and confirmed as a
    /// unit — used to log a MULTI/EXEC transaction atomically (audit M-010). The
    /// writer emits every op back-to-back within one drain, so no other write
    /// interleaves between them, and confirms once with all assigned offsets.
    Batch {
        operations: Vec<Operation>,
        response_tx: oneshot::Sender<Result<Vec<u64>>>,
    },
    /// Barrier: every earlier entry reaches the OS (and stable storage when
    /// `sync` is set) before the response is sent.
    Flush {
        sync: bool,
        response_tx: oneshot::Sender<Result<()>>,
    },
    /// Flush and fsync every earlier entry, respond, then stop the writer so no
    /// later append can be acknowledged.
    Shutdown {
        response_tx: oneshot::Sender<Result<()>>,
    },
}

/// Asynchronous Write-Ahead Log with group commit optimization
/// Batches multiple operations before fsyncing for 10-100x better throughput
///
/// Durability per [`FsyncMode`]: every group is handed to the OS before its
/// acknowledgements are sent. `Always` also fsyncs each group; `Periodic`
/// fsyncs on its own interval whether or not more writes arrive; `Never` does
/// not fsync. A failed write, flush or fsync fails every acknowledgement in its
/// group and poisons the writer: every later append fails until restart.
#[derive(Clone)]
pub struct AsyncWAL {
    writer_tx: mpsc::UnboundedSender<WriteRequest>,
    current_offset: Arc<AtomicU64>,
    poison: Arc<OnceLock<String>>,
    fsync_mode: FsyncMode,
}

impl AsyncWAL {
    /// Create or open an async WAL file
    pub async fn open(config: WALConfig) -> Result<Self> {
        Self::open_with_sink(config, |sink| sink).await
    }

    /// Open the WAL and route its writes through `wrap(FileSink)`.
    ///
    /// Before the file is opened for append it is scanned: every frame's length
    /// and checksum are verified, a torn tail is truncated to the last valid
    /// frame, and interior corruption fails with
    /// [`PersistenceError::WALInteriorCorruption`].
    pub async fn open_with_sink<S, F>(config: WALConfig, wrap: F) -> Result<Self>
    where
        S: WalSink,
        F: FnOnce(FileSink) -> S,
    {
        // Create directory if it doesn't exist
        if let Some(parent) = config.path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let report = wal_frame::repair_async(config.path.clone()).await?;
        let next_offset = report.next_offset();

        // Open or create file (append mode), now ending on a frame boundary
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .read(true)
            .open(&config.path)
            .await?;
        let sink = wrap(FileSink::new(file, config.buffer_size_kb * 1024));

        info!(
            "Async WAL opened at {:?}, current offset: {}",
            config.path, next_offset
        );

        let current_offset = Arc::new(AtomicU64::new(next_offset));
        let poison = Arc::new(OnceLock::new());
        let (writer_tx, writer_rx) = mpsc::unbounded_channel();

        let writer = Writer {
            sink,
            mode: config.fsync_mode,
            interval: Duration::from_millis(config.fsync_interval_ms),
            current_offset: Arc::clone(&current_offset),
            poison: Arc::clone(&poison),
            unsynced: false,
            last_sync: Instant::now(),
        };
        tokio::spawn(writer.run(writer_rx));

        Ok(Self {
            writer_tx,
            current_offset,
            poison,
            fsync_mode: config.fsync_mode,
        })
    }

    /// Send a request to the writer and await its response.
    async fn request<T>(
        &self,
        make: impl FnOnce(oneshot::Sender<Result<T>>) -> WriteRequest,
    ) -> Result<T> {
        let (tx, rx) = oneshot::channel();
        self.writer_tx
            .send(make(tx))
            .map_err(|_| broken_pipe("WAL writer channel closed"))?;
        rx.await
            .map_err(|_| broken_pipe("WAL writer response channel closed"))?
    }

    /// Fail fast when the writer is poisoned.
    fn check_poison(&self) -> Result<()> {
        match self.poison.get() {
            Some(reason) => Err(PersistenceError::WALPoisoned(reason.clone())),
            None => Ok(()),
        }
    }

    /// Append an operation to the WAL (returns immediately, actual write is batched)
    pub async fn append(&self, operation: Operation) -> Result<u64> {
        self.check_poison()?;
        self.request(|response_tx| WriteRequest::Single {
            operation,
            response_tx,
        })
        .await
    }

    /// Append a group of operations as one atomic unit (audit M-010).
    ///
    /// All operations are written contiguously by the background writer and a
    /// single confirmation is returned once they are durably recorded (subject to
    /// the configured fsync mode), so a MULTI/EXEC is logged as a unit rather than
    /// as interleavable single appends. Returns the assigned offsets in order. An
    /// empty batch is a no-op.
    pub async fn append_batch(&self, operations: Vec<Operation>) -> Result<Vec<u64>> {
        if operations.is_empty() {
            return Ok(Vec::new());
        }
        self.check_poison()?;
        self.request(|response_tx| WriteRequest::Batch {
            operations,
            response_tx,
        })
        .await
    }

    /// Wait until every entry appended before this call has reached the OS,
    /// and stable storage too unless the fsync mode is `Never`.
    pub async fn flush(&self) -> Result<()> {
        self.check_poison()?;
        let sync = self.fsync_mode != FsyncMode::Never;
        self.request(|response_tx| WriteRequest::Flush { sync, response_tx })
            .await
    }

    /// Flush and fsync every entry appended before this call, then stop the
    /// writer. Later appends fail instead of being acknowledged after the
    /// final fsync. Used by graceful shutdown.
    pub async fn shutdown(&self) -> Result<()> {
        self.check_poison()?;
        self.request(|response_tx| WriteRequest::Shutdown { response_tx })
            .await
    }

    /// Why the writer is poisoned, or `None` while it is healthy.
    pub fn poison_reason(&self) -> Option<String> {
        self.poison.get().cloned()
    }

    /// Get current offset
    pub fn current_offset(&self) -> u64 {
        self.current_offset.load(Ordering::SeqCst)
    }

    /// Replay WAL entries from a specific offset.
    ///
    /// Every frame's checksum and length are verified; replay stops at a torn
    /// tail and fails on interior corruption.
    pub async fn replay(&self, path: &Path, from_offset: u64) -> Result<Vec<WALEntry>> {
        debug!("Replaying WAL from offset {}", from_offset);
        let entries = wal_frame::read_entries(path.to_path_buf(), from_offset).await?;
        info!("Replayed {} WAL entries", entries.len());
        Ok(entries)
    }
}

fn broken_pipe(message: &str) -> PersistenceError {
    PersistenceError::IOError(io::Error::new(io::ErrorKind::BrokenPipe, message))
}

/// One acknowledgement waiting for its group to reach the OS (and disk).
enum Pending {
    Single(oneshot::Sender<Result<u64>>, u64),
    Batch(oneshot::Sender<Result<Vec<u64>>>, Vec<u64>),
}

impl Pending {
    /// Send the group's outcome: the assigned offsets, or the poison reason.
    fn complete(self, outcome: &std::result::Result<(), String>) {
        let failure = |reason: &String| PersistenceError::WALPoisoned(reason.clone());
        match self {
            Pending::Single(tx, offset) => {
                let _ = tx.send(outcome.as_ref().map(|()| offset).map_err(failure));
            }
            Pending::Batch(tx, offsets) => {
                let _ = tx.send(outcome.as_ref().map(|()| offsets).map_err(failure));
            }
        }
    }
}

/// Whether the writer keeps running after a group.
#[derive(PartialEq, Eq)]
enum Flow {
    Continue,
    Stop,
}

/// Background writer state, owned by the writer task.
struct Writer<S> {
    sink: S,
    mode: FsyncMode,
    interval: Duration,
    current_offset: Arc<AtomicU64>,
    poison: Arc<OnceLock<String>>,
    /// Bytes have been written since the last successful fsync.
    unsynced: bool,
    last_sync: Instant,
}

impl<S: WalSink> Writer<S> {
    /// Background writer loop with group commit optimization
    async fn run(mut self, mut rx: mpsc::UnboundedReceiver<WriteRequest>) {
        const MAX_BATCH_SIZE: usize = 1000;
        let mut batch: Vec<WriteRequest> = Vec::with_capacity(MAX_BATCH_SIZE);

        // Periodic mode fsyncs on its own clock, so entries acknowledged just
        // before the server goes idle still reach the disk within the interval.
        let periodic = self.mode == FsyncMode::Periodic;
        let mut ticker = tokio::time::interval(self.interval.max(Duration::from_millis(1)));
        ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);

        loop {
            tokio::select! {
                received = rx.recv() => match received {
                    Some(request) => {
                        batch.push(request);
                        // Fill the group with whatever else is already queued
                        while batch.len() < MAX_BATCH_SIZE {
                            match rx.try_recv() {
                                Ok(request) => batch.push(request),
                                Err(_) => break,
                            }
                        }
                    }
                    None => break, // every handle dropped
                },
                _ = ticker.tick(), if periodic => {
                    if self.unsynced && self.poison.get().is_none() {
                        let _ = self.commit(&mut Vec::new(), true).await;
                    }
                    continue;
                }
            }

            if self.process(&mut batch).await == Flow::Stop {
                info!("Async WAL writer stopped after shutdown drain");
                return;
            }
        }

        // The last handle is gone: hand the tail to the OS (and to the disk
        // unless fsync is disabled) before the file is closed.
        if self.poison.get().is_none() {
            let sync = self.mode != FsyncMode::Never;
            let _ = self.commit(&mut Vec::new(), sync).await;
        }
        info!("Async WAL writer loop terminated");
    }

    /// Write one group of requests, then flush it and answer every request.
    async fn process(&mut self, batch: &mut Vec<WriteRequest>) -> Flow {
        let mut pending: Vec<Pending> = Vec::with_capacity(batch.len());

        for request in batch.drain(..) {
            match request {
                WriteRequest::Single {
                    operation,
                    response_tx,
                } => match self.write_entries(vec![operation]).await {
                    Ok(offsets) => pending.push(Pending::Single(response_tx, offsets[0])),
                    Err(e) => {
                        let _ = response_tx.send(Err(e));
                    }
                },
                WriteRequest::Batch {
                    operations,
                    response_tx,
                } => match self.write_entries(operations).await {
                    Ok(offsets) => pending.push(Pending::Batch(response_tx, offsets)),
                    Err(e) => {
                        let _ = response_tx.send(Err(e));
                    }
                },
                WriteRequest::Flush { sync, response_tx } => {
                    let outcome = self.commit(&mut pending, sync).await;
                    let _ = response_tx.send(outcome.map_err(PersistenceError::WALPoisoned));
                }
                WriteRequest::Shutdown { response_tx } => {
                    let outcome = self.commit(&mut pending, true).await;
                    let _ = response_tx.send(outcome.map_err(PersistenceError::WALPoisoned));
                    // Requests queued behind the shutdown are dropped, so
                    // their callers get an error rather than an acknowledgement.
                    return Flow::Stop;
                }
            }
        }

        if !pending.is_empty() {
            let count = pending.len();
            if self.commit(&mut pending, false).await.is_ok() {
                debug!("Group commit: {} requests flushed", count);
            }
        }
        Flow::Continue
    }

    /// Encode `operations` as frames and write them back-to-back.
    ///
    /// Every frame is encoded before any byte is written, so a serialization
    /// error writes nothing. A sink error poisons the writer.
    async fn write_entries(&mut self, operations: Vec<Operation>) -> Result<Vec<u64>> {
        if let Some(reason) = self.poison.get() {
            return Err(PersistenceError::WALPoisoned(reason.clone()));
        }

        let timestamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let mut offsets = Vec::with_capacity(operations.len());
        let mut frames = Vec::with_capacity(operations.len());
        for operation in operations {
            let offset = self.current_offset.fetch_add(1, Ordering::SeqCst);
            let entry = WALEntry {
                offset,
                timestamp,
                operation,
            };
            frames.push(wal_frame::encode_frame(&entry)?);
            offsets.push(offset);
        }

        for frame in &frames {
            if let Err(e) = self.sink.write_all(frame).await {
                let reason = self.poison_with(format!("write failed: {e}"));
                return Err(PersistenceError::WALPoisoned(reason));
            }
        }
        self.unsynced = true;
        Ok(offsets)
    }

    /// Hand buffered bytes to the OS and fsync when `force_sync` is set or the
    /// fsync mode requires it now, then answer every pending request with the
    /// outcome. A failure poisons the writer and fails every pending request.
    async fn commit(
        &mut self,
        pending: &mut Vec<Pending>,
        force_sync: bool,
    ) -> std::result::Result<(), String> {
        let outcome = self.flush_and_sync(force_sync).await;
        for request in pending.drain(..) {
            request.complete(&outcome);
        }
        outcome
    }

    async fn flush_and_sync(&mut self, force_sync: bool) -> std::result::Result<(), String> {
        if let Some(reason) = self.poison.get() {
            return Err(reason.clone());
        }
        if let Err(e) = self.sink.flush().await {
            return Err(self.poison_with(format!("flush failed: {e}")));
        }
        if (force_sync || self.sync_due()) && self.unsynced {
            if let Err(e) = self.sink.sync().await {
                return Err(self.poison_with(format!("fsync failed: {e}")));
            }
            self.unsynced = false;
            self.last_sync = Instant::now();
        }
        Ok(())
    }

    /// Whether the fsync mode requires an fsync at the end of this group.
    fn sync_due(&self) -> bool {
        match self.mode {
            FsyncMode::Always => true,
            FsyncMode::Periodic => self.last_sync.elapsed() >= self.interval,
            FsyncMode::Never => false,
        }
    }

    /// Poison the writer: after a failed write, flush or fsync the file's
    /// contents are unknown (the kernel may have dropped dirty pages), so no
    /// later append may be acknowledged. Returns the recorded reason.
    fn poison_with(&self, reason: String) -> String {
        if self.poison.set(reason.clone()).is_ok() {
            error!(
                "WAL writer poisoned: {}; every write fails until the server restarts",
                reason
            );
        }
        self.poison.get().cloned().unwrap_or(reason)
    }
}

/// Fault-injecting [`WalSink`] for tests across the crate.
#[cfg(test)]
pub(crate) mod fault {
    use super::{FileSink, WalSink};
    use std::io;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    /// Shared switches and counters for a [`FaultSink`].
    #[derive(Clone, Default)]
    pub(crate) struct Faults {
        fail_next_flush: Arc<AtomicBool>,
        fail_next_sync: Arc<AtomicBool>,
        syncs: Arc<AtomicUsize>,
    }

    impl Faults {
        /// Make the next flush fail.
        pub(crate) fn fail_next_flush(&self) {
            self.fail_next_flush.store(true, Ordering::SeqCst);
        }

        /// Make the next fsync fail.
        pub(crate) fn fail_next_sync(&self) {
            self.fail_next_sync.store(true, Ordering::SeqCst);
        }

        /// Number of successful fsyncs.
        pub(crate) fn syncs(&self) -> usize {
            self.syncs.load(Ordering::SeqCst)
        }

        /// Wrap a production sink with these faults.
        pub(crate) fn wrap(&self, inner: FileSink) -> FaultSink {
            FaultSink {
                inner,
                faults: self.clone(),
            }
        }
    }

    /// [`FileSink`] wrapper that fails a flush or fsync on demand.
    pub(crate) struct FaultSink {
        inner: FileSink,
        faults: Faults,
    }

    impl WalSink for FaultSink {
        async fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
            self.inner.write_all(buf).await
        }

        async fn flush(&mut self) -> io::Result<()> {
            if self.faults.fail_next_flush.swap(false, Ordering::SeqCst) {
                return Err(io::Error::other("injected flush failure"));
            }
            self.inner.flush().await
        }

        async fn sync(&mut self) -> io::Result<()> {
            if self.faults.fail_next_sync.swap(false, Ordering::SeqCst) {
                return Err(io::Error::other("injected fsync failure"));
            }
            self.inner.sync().await?;
            self.faults.syncs.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::fault::Faults;
    use super::*;
    use std::path::PathBuf;

    fn wal_config(dir: &str) -> WALConfig {
        WALConfig {
            enabled: true,
            path: PathBuf::from(format!("{dir}/test.wal")),
            fsync_mode: FsyncMode::Always,
            ..Default::default()
        }
    }

    fn fresh_config(name: &str, mode: FsyncMode, interval_ms: u64) -> WALConfig {
        let dir = format!("./target/wal_async_{name}");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        WALConfig {
            enabled: true,
            path: PathBuf::from(format!("{dir}/test.wal")),
            fsync_mode: mode,
            fsync_interval_ms: interval_ms,
            ..Default::default()
        }
    }

    fn set(key: &str) -> Operation {
        Operation::KVSet {
            key: key.into(),
            value: key.as_bytes().to_vec(),
            ttl: None,
        }
    }

    async fn open_faulty(config: &WALConfig) -> (AsyncWAL, Faults) {
        let faults = Faults::default();
        let wrap = faults.clone();
        let wal = AsyncWAL::open_with_sink(config.clone(), move |sink| wrap.wrap(sink))
            .await
            .unwrap();
        (wal, faults)
    }

    /// Entries a fresh reader sees in the file right now (what the OS has).
    fn entries_on_disk(config: &WALConfig) -> Vec<WALEntry> {
        let mut entries = Vec::new();
        wal_frame::scan(&config.path, |e| entries.push(e)).unwrap();
        entries
    }

    /// `append_batch` writes every op contiguously and returns their offsets; a
    /// later replay sees exactly those entries (audit M-010 transaction WAL unit).
    #[tokio::test]
    async fn append_batch_writes_all_and_replays() {
        let dir = "./target/wal_batch_test";
        let _ = std::fs::remove_dir_all(dir);
        std::fs::create_dir_all(dir).unwrap();
        let config = wal_config(dir);

        let wal = AsyncWAL::open(config.clone()).await.unwrap();

        // Empty batch is a no-op.
        assert!(wal.append_batch(vec![]).await.unwrap().is_empty());

        let ops = vec![
            Operation::KVSet {
                key: "a".into(),
                value: b"1".to_vec(),
                ttl: None,
            },
            Operation::KVSet {
                key: "b".into(),
                value: b"2".to_vec(),
                ttl: None,
            },
            Operation::KVDel {
                keys: vec!["a".into()],
            },
        ];
        let offsets = wal.append_batch(ops).await.unwrap();
        assert_eq!(offsets.len(), 3);
        // Offsets are contiguous and ascending.
        assert_eq!(offsets[1], offsets[0] + 1);
        assert_eq!(offsets[2], offsets[1] + 1);

        // A single append after the batch continues the offset sequence.
        let single = wal
            .append(Operation::KVSet {
                key: "c".into(),
                value: b"3".to_vec(),
                ttl: None,
            })
            .await
            .unwrap();
        assert_eq!(single, offsets[2] + 1);

        let entries = wal.replay(&config.path, 0).await.unwrap();
        assert_eq!(entries.len(), 4);

        let _ = std::fs::remove_dir_all(dir);
    }

    /// The writer's group-commit path handles every fsync mode (Periodic/Never
    /// branches, refactored for the batch API in phase6k).
    #[tokio::test]
    async fn append_honors_periodic_and_never_fsync_modes() {
        for (name, mode) in [
            ("periodic", FsyncMode::Periodic),
            ("never", FsyncMode::Never),
        ] {
            let dir = format!("./target/wal_fsync_{name}");
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            let config = WALConfig {
                enabled: true,
                path: PathBuf::from(format!("{dir}/test.wal")),
                fsync_mode: mode,
                fsync_interval_ms: 0, // force the Periodic fsync branch to fire
                ..Default::default()
            };
            let wal = AsyncWAL::open(config.clone()).await.unwrap();
            let o1 = wal
                .append(Operation::KVSet {
                    key: "k".into(),
                    value: b"v".to_vec(),
                    ttl: None,
                })
                .await
                .unwrap();
            let o2 = wal
                .append_batch(vec![Operation::KVDel {
                    keys: vec!["k".into()],
                }])
                .await
                .unwrap();
            assert_eq!(o2, vec![o1 + 1]);
            // Every group reaches the OS before it is acknowledged, in both modes.
            assert_eq!(entries_on_disk(&config).len(), 2);
            let _ = std::fs::remove_dir_all(&dir);
        }
    }

    /// Periodic mode fsyncs on its own interval while idle: one acknowledged
    /// entry and no further writes still reaches the disk within the interval.
    #[tokio::test]
    async fn periodic_mode_fsyncs_while_idle() {
        let config = fresh_config("idle", FsyncMode::Periodic, 1000);
        let (wal, faults) = open_faulty(&config).await;

        wal.append(set("idle")).await.unwrap();
        assert_eq!(faults.syncs(), 0, "fsync ran before the interval elapsed");

        tokio::time::sleep(Duration::from_millis(2000)).await;
        assert_eq!(faults.syncs(), 1, "idle writer never fsynced");
        assert_eq!(entries_on_disk(&config).len(), 1);
        assert!(wal.poison_reason().is_none());
    }

    /// Shutdown 10 ms after an acknowledgement, well before the interval,
    /// still flushes and fsyncs it, and later appends are refused.
    #[tokio::test]
    async fn shutdown_before_the_interval_drains_and_fsyncs() {
        let config = fresh_config("shutdown", FsyncMode::Periodic, 1000);
        let (wal, faults) = open_faulty(&config).await;

        wal.append(set("before-shutdown")).await.unwrap();
        tokio::time::sleep(Duration::from_millis(10)).await;
        wal.shutdown().await.unwrap();

        assert_eq!(faults.syncs(), 1);
        assert!(wal.append(set("after-shutdown")).await.is_err());
        let reopened = AsyncWAL::open(config.clone()).await.unwrap();
        let entries = reopened.replay(&config.path, 0).await.unwrap();
        assert_eq!(entries.len(), 1);
    }

    /// Never mode hands each group to the OS without filling the buffer and
    /// never fsyncs, even on a flush barrier or when the WAL is dropped.
    #[tokio::test]
    async fn never_mode_writes_without_fsync() {
        let config = fresh_config("never", FsyncMode::Never, 1000);
        let (wal, faults) = open_faulty(&config).await;

        wal.append(set("small")).await.unwrap();
        assert_eq!(entries_on_disk(&config).len(), 1);
        wal.flush().await.unwrap();
        drop(wal);
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert_eq!(faults.syncs(), 0);
    }

    /// The flush barrier fsyncs pending entries in Periodic mode.
    #[tokio::test]
    async fn flush_barrier_fsyncs_in_periodic_mode() {
        let config = fresh_config("barrier", FsyncMode::Periodic, 60_000);
        let (wal, faults) = open_faulty(&config).await;
        wal.append(set("a")).await.unwrap();
        wal.flush().await.unwrap();
        assert_eq!(faults.syncs(), 1);
        // Nothing new: the barrier does not fsync again.
        wal.flush().await.unwrap();
        assert_eq!(faults.syncs(), 1);
    }

    /// A failed fsync fails the write, poisons the writer, and every later
    /// append and flush fails too.
    #[tokio::test]
    async fn injected_fsync_failure_poisons_the_writer() {
        let config = fresh_config("fsync_fail", FsyncMode::Always, 1000);
        let (wal, faults) = open_faulty(&config).await;
        wal.append(set("ok")).await.unwrap();

        faults.fail_next_sync();
        let err = wal.append(set("lost")).await.unwrap_err();
        assert!(matches!(err, PersistenceError::WALPoisoned(_)), "{err}");
        assert!(wal.poison_reason().unwrap().contains("fsync failed"));
        assert!(matches!(
            wal.append(set("after")).await,
            Err(PersistenceError::WALPoisoned(_))
        ));
        assert!(wal.flush().await.is_err());
        assert!(wal.shutdown().await.is_err());
    }

    /// A failed flush fails a transaction batch and poisons the writer.
    #[tokio::test]
    async fn injected_flush_failure_fails_a_batch() {
        let config = fresh_config("flush_fail", FsyncMode::Always, 1000);
        let (wal, faults) = open_faulty(&config).await;

        faults.fail_next_flush();
        let err = wal
            .append_batch(vec![set("t1"), set("t2")])
            .await
            .unwrap_err();
        assert!(matches!(err, PersistenceError::WALPoisoned(_)), "{err}");
        assert!(wal.poison_reason().unwrap().contains("flush failed"));
        assert!(wal.append(set("after")).await.is_err());
    }

    /// Every acknowledgement in a group whose fsync fails is an error; none
    /// of the concurrent writers sees success.
    #[tokio::test]
    async fn every_acknowledgement_in_a_failed_group_fails() {
        let config = fresh_config("group_fail", FsyncMode::Always, 1000);
        let (wal, faults) = open_faulty(&config).await;
        faults.fail_next_sync();

        let writers: Vec<_> = (0..20)
            .map(|i| {
                let wal = wal.clone();
                tokio::spawn(async move { wal.append(set(&format!("k{i}"))).await })
            })
            .collect();
        for writer in writers {
            assert!(writer.await.unwrap().is_err());
        }
    }

    /// A failed fsync on the Periodic interval poisons the writer even though
    /// no append is waiting on it.
    #[tokio::test]
    async fn periodic_interval_fsync_failure_poisons() {
        let config = fresh_config("periodic_fail", FsyncMode::Periodic, 300);
        let (wal, faults) = open_faulty(&config).await;
        faults.fail_next_sync();
        // Acknowledged once handed to the OS, before the interval's fsync.
        wal.append(set("a")).await.unwrap();
        tokio::time::sleep(Duration::from_millis(1000)).await;
        assert!(wal.poison_reason().unwrap().contains("fsync failed"));
        assert!(wal.append(set("b")).await.is_err());
    }
}
