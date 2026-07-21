use super::types::{FsyncMode, Operation, PersistenceError, Result, WALConfig, WALEntry};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime};
use tokio::fs::{File, OpenOptions};
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufWriter};
use tokio::sync::{mpsc, oneshot};
use tracing::{debug, info, warn};

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
}

/// Asynchronous Write-Ahead Log with group commit optimization
/// Batches multiple operations before fsyncing for 10-100x better throughput
#[derive(Clone)]
pub struct AsyncWAL {
    writer_tx: mpsc::UnboundedSender<WriteRequest>,
    current_offset: Arc<AtomicU64>,
}

impl AsyncWAL {
    /// Create or open an async WAL file
    pub async fn open(config: WALConfig) -> Result<Self> {
        // Create directory if it doesn't exist
        if let Some(parent) = config.path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        // Open or create file (append mode)
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .read(true)
            .open(&config.path)
            .await?;

        let writer = BufWriter::with_capacity(config.buffer_size_kb * 1024, file);

        // Scan for last offset
        let current_offset = Arc::new(AtomicU64::new(0));
        let last_offset = Self::scan_for_last_offset(&config.path).await?;
        current_offset.store(last_offset, Ordering::SeqCst);

        info!(
            "Async WAL opened at {:?}, current offset: {}",
            config.path, last_offset
        );

        // Create channel for write operations
        let (writer_tx, writer_rx) = mpsc::unbounded_channel();

        // Spawn background writer task
        let offset_clone = Arc::clone(&current_offset);
        tokio::spawn(Self::writer_loop(
            writer,
            writer_rx,
            offset_clone,
            config.clone(),
        ));

        Ok(Self {
            writer_tx,
            current_offset,
        })
    }

    /// Scan WAL file to find the last valid offset
    async fn scan_for_last_offset(path: &PathBuf) -> Result<u64> {
        let mut file = File::open(path).await?;
        let mut max_offset = 0u64;

        // Read until EOF (read_u64 errors at end of file)
        while let Ok(size) = file.read_u64().await {
            // Read checksum
            let _checksum = match file.read_u32().await {
                Ok(c) => c,
                Err(_) => break,
            };

            // Read entry data
            let mut data = vec![0u8; size as usize];
            if file.read_exact(&mut data).await.is_err() {
                warn!("Incomplete WAL entry detected, truncating");
                break;
            }

            // Try to deserialize
            match bincode::serde::decode_from_slice::<WALEntry, _>(&data, bincode::config::legacy())
            {
                Ok((entry, _)) => {
                    max_offset = max_offset.max(entry.offset);
                }
                Err(_) => {
                    warn!("Corrupted WAL entry detected, stopping scan");
                    break;
                }
            }
        }

        Ok(max_offset + 1)
    }

    /// Background writer loop with group commit optimization
    async fn writer_loop(
        mut writer: BufWriter<File>,
        mut rx: mpsc::UnboundedReceiver<WriteRequest>,
        current_offset: Arc<AtomicU64>,
        config: WALConfig,
    ) {
        const MAX_BATCH_SIZE: usize = 1000;
        let batch_timeout = Duration::from_millis(10);

        let mut batch: Vec<WriteRequest> = Vec::with_capacity(MAX_BATCH_SIZE);
        let mut last_fsync = std::time::Instant::now();

        loop {
            // Collect requests for group commit
            tokio::select! {
                // Receive first request (blocking)
                Some(req) = rx.recv() => {
                    batch.push(req);

                    // Try to fill batch (non-blocking)
                    while batch.len() < MAX_BATCH_SIZE {
                        match rx.try_recv() {
                            Ok(req) => batch.push(req),
                            Err(_) => break,
                        }
                    }
                }

                // Timeout for small batches
                _ = tokio::time::sleep(batch_timeout), if !batch.is_empty() => {
                    // Process current batch
                }

                else => break,  // Channel closed
            }

            if !batch.is_empty() {
                // Each request produces exactly one response (a single offset, or
                // the vector of offsets for a batch). A `Batch` request's ops are
                // written back-to-back here, so they land contiguously in the WAL.
                enum Pending {
                    Single(oneshot::Sender<Result<u64>>, Result<u64>),
                    Batch(oneshot::Sender<Result<Vec<u64>>>, Result<Vec<u64>>),
                }
                let mut responses: Vec<Pending> = Vec::with_capacity(batch.len());

                for request in batch.drain(..) {
                    match request {
                        WriteRequest::Single {
                            operation,
                            response_tx,
                        } => {
                            let result = Self::write_one(&mut writer, &current_offset, operation)
                                .await
                                .map(|(offset, _)| offset);
                            responses.push(Pending::Single(response_tx, result));
                        }
                        WriteRequest::Batch {
                            operations,
                            response_tx,
                        } => {
                            let mut offsets = Vec::with_capacity(operations.len());
                            let mut batch_result = Ok(());
                            for operation in operations {
                                match Self::write_one(&mut writer, &current_offset, operation).await
                                {
                                    Ok((offset, _)) => offsets.push(offset),
                                    Err(e) => {
                                        batch_result = Err(e);
                                        break;
                                    }
                                }
                            }
                            let result = batch_result.map(|_| offsets);
                            responses.push(Pending::Batch(response_tx, result));
                        }
                    }
                }

                // Group commit: single fsync for entire batch
                let should_fsync = match config.fsync_mode {
                    FsyncMode::Always => true,
                    FsyncMode::Periodic => {
                        let elapsed = last_fsync.elapsed();
                        elapsed.as_millis() >= config.fsync_interval_ms as u128
                    }
                    FsyncMode::Never => false,
                };

                if should_fsync {
                    if let Err(e) = writer.flush().await {
                        warn!("WAL flush failed: {}", e);
                    }
                    if let Err(e) = writer.get_ref().sync_all().await {
                        warn!("WAL fsync failed: {}", e);
                    }
                    last_fsync = std::time::Instant::now();
                    debug!("Group commit: {} requests fsynced", responses.len());
                }

                // Send responses back
                for pending in responses {
                    match pending {
                        Pending::Single(tx, result) => {
                            let _ = tx.send(result);
                        }
                        Pending::Batch(tx, result) => {
                            let _ = tx.send(result);
                        }
                    }
                }
            }
        }

        info!("Async WAL writer loop terminated");
    }

    /// Assign the next offset, build the entry, and write it to the buffer.
    /// Returns the assigned offset (and unit) or the write error.
    async fn write_one(
        writer: &mut BufWriter<File>,
        current_offset: &Arc<AtomicU64>,
        operation: Operation,
    ) -> Result<(u64, ())> {
        let offset = current_offset.fetch_add(1, Ordering::SeqCst);
        let entry = WALEntry {
            offset,
            timestamp: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            operation,
        };
        Self::write_entry(writer, &entry)
            .await
            .map(|_| (offset, ()))
    }

    /// Write a single entry to the writer
    async fn write_entry(writer: &mut BufWriter<File>, entry: &WALEntry) -> Result<()> {
        // Serialize entry
        let data = bincode::serde::encode_to_vec(entry, bincode::config::legacy())?;
        let checksum = crc32fast::hash(&data);

        // Write entry format: size (u64) + checksum (u32) + data
        writer.write_u64(data.len() as u64).await?;
        writer.write_u32(checksum).await?;
        writer.write_all(&data).await?;

        Ok(())
    }

    /// Append an operation to the WAL (returns immediately, actual write is batched)
    pub async fn append(&self, operation: Operation) -> Result<u64> {
        let (tx, rx) = oneshot::channel();

        let request = WriteRequest::Single {
            operation,
            response_tx: tx,
        };

        self.writer_tx.send(request).map_err(|_| {
            PersistenceError::IOError(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "WAL writer channel closed",
            ))
        })?;

        // Wait for write confirmation
        rx.await.map_err(|_| {
            PersistenceError::IOError(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "WAL writer response channel closed",
            ))
        })?
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

        let (tx, rx) = oneshot::channel();

        let request = WriteRequest::Batch {
            operations,
            response_tx: tx,
        };

        self.writer_tx.send(request).map_err(|_| {
            PersistenceError::IOError(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "WAL writer channel closed",
            ))
        })?;

        rx.await.map_err(|_| {
            PersistenceError::IOError(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "WAL writer response channel closed",
            ))
        })?
    }

    /// Get current offset
    pub fn current_offset(&self) -> u64 {
        self.current_offset.load(Ordering::SeqCst)
    }

    /// Replay WAL entries from a specific offset
    pub async fn replay(&self, path: &PathBuf, from_offset: u64) -> Result<Vec<WALEntry>> {
        let mut file = File::open(path).await?;
        let mut entries = Vec::new();

        debug!("Replaying WAL from offset {}", from_offset);

        // Read until EOF (read_u64 errors at end of file)
        while let Ok(size) = file.read_u64().await {
            // Read checksum
            let checksum_expected = match file.read_u32().await {
                Ok(c) => c,
                Err(_) => break,
            };

            // Read entry data
            let mut data = vec![0u8; size as usize];
            if file.read_exact(&mut data).await.is_err() {
                warn!("Incomplete WAL entry during replay");
                break;
            }

            // Verify checksum
            let checksum_actual = crc32fast::hash(&data);
            if checksum_actual != checksum_expected {
                warn!("WAL checksum mismatch, stopping replay");
                break;
            }

            // Deserialize
            match bincode::serde::decode_from_slice::<WALEntry, _>(&data, bincode::config::legacy())
            {
                Ok((entry, _)) => {
                    if entry.offset >= from_offset {
                        entries.push(entry);
                    }
                }
                Err(e) => {
                    warn!("WAL entry deserialization failed: {}", e);
                    break;
                }
            }
        }

        info!("Replayed {} WAL entries", entries.len());
        Ok(entries)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wal_config(dir: &str) -> WALConfig {
        WALConfig {
            enabled: true,
            path: PathBuf::from(format!("{dir}/test.wal")),
            fsync_mode: FsyncMode::Always,
            ..Default::default()
        }
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
            // Both the single and batch paths run through the writer's should_fsync
            // match for this mode and return their assigned offsets. (Never does not
            // flush to disk, so we don't assert a replay here.)
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
            let _ = std::fs::remove_dir_all(&dir);
        }
    }
}
