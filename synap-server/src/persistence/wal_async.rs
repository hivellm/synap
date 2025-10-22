use super::types::{FsyncMode, Operation, PersistenceError, Result, WALConfig, WALEntry};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime};
use tokio::fs::{File, OpenOptions};
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufWriter};
use tokio::sync::{mpsc, oneshot};
use tracing::{debug, info, warn};

/// Operation to be written with completion notification
struct WriteOperation {
    operation: Operation,
    response_tx: oneshot::Sender<Result<u64>>,
}

/// Asynchronous Write-Ahead Log with group commit optimization
/// Batches multiple operations before fsyncing for 10-100x better throughput
#[derive(Clone)]
pub struct AsyncWAL {
    writer_tx: mpsc::UnboundedSender<WriteOperation>,
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
    #[allow(clippy::while_let_loop)]
    async fn scan_for_last_offset(path: &PathBuf) -> Result<u64> {
        let mut file = File::open(path).await?;
        let mut max_offset = 0u64;

        loop {
            // Try to read entry size
            let size = match file.read_u64().await {
                Ok(s) => s,
                Err(_) => break, // EOF
            };

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
            match bincode::deserialize::<WALEntry>(&data) {
                Ok(entry) => {
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
        mut rx: mpsc::UnboundedReceiver<WriteOperation>,
        current_offset: Arc<AtomicU64>,
        config: WALConfig,
    ) {
        const MAX_BATCH_SIZE: usize = 1000;
        let batch_timeout = Duration::from_millis(10);

        let mut batch: Vec<WriteOperation> = Vec::with_capacity(MAX_BATCH_SIZE);
        let mut last_fsync = std::time::Instant::now();

        loop {
            // Collect operations for batching
            tokio::select! {
                // Receive first operation (blocking)
                Some(op) = rx.recv() => {
                    batch.push(op);

                    // Try to fill batch (non-blocking)
                    while batch.len() < MAX_BATCH_SIZE {
                        match rx.try_recv() {
                            Ok(op) => batch.push(op),
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
                // Write all entries in batch
                let mut responses = Vec::with_capacity(batch.len());

                for WriteOperation {
                    operation,
                    response_tx,
                } in batch.drain(..)
                {
                    let offset = current_offset.fetch_add(1, Ordering::SeqCst);

                    let entry = WALEntry {
                        offset,
                        timestamp: SystemTime::now()
                            .duration_since(SystemTime::UNIX_EPOCH)
                            .unwrap()
                            .as_secs(),
                        operation,
                    };

                    // Serialize and write
                    match Self::write_entry(&mut writer, &entry).await {
                        Ok(_) => {
                            responses.push((response_tx, Ok(offset)));
                        }
                        Err(e) => {
                            responses.push((response_tx, Err(e)));
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
                    debug!("Group commit: {} operations fsynced", responses.len());
                }

                // Send responses back
                for (tx, result) in responses {
                    let _ = tx.send(result);
                }
            }
        }

        info!("Async WAL writer loop terminated");
    }

    /// Write a single entry to the writer
    async fn write_entry(writer: &mut BufWriter<File>, entry: &WALEntry) -> Result<()> {
        // Serialize entry
        let data = bincode::serialize(entry)?;
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

        let write_op = WriteOperation {
            operation,
            response_tx: tx,
        };

        self.writer_tx.send(write_op).map_err(|_| {
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

    /// Get current offset
    pub fn current_offset(&self) -> u64 {
        self.current_offset.load(Ordering::SeqCst)
    }

    /// Replay WAL entries from a specific offset
    #[allow(clippy::while_let_loop)]
    pub async fn replay(&self, path: &PathBuf, from_offset: u64) -> Result<Vec<WALEntry>> {
        let mut file = File::open(path).await?;
        let mut entries = Vec::new();

        debug!("Replaying WAL from offset {}", from_offset);

        loop {
            // Read entry size
            let size = match file.read_u64().await {
                Ok(s) => s,
                Err(_) => break, // EOF
            };

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
            match bincode::deserialize::<WALEntry>(&data) {
                Ok(entry) => {
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
