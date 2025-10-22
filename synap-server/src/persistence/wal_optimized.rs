use super::types::{FsyncMode, Operation, PersistenceError, Result, WALConfig, WALEntry};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::fs::{File, OpenOptions};
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufWriter};
use tokio::sync::{Mutex, mpsc, oneshot};
use tracing::{debug, info, warn};

/// Redis-style optimized WAL with advanced batching and pipelining
///
/// Optimizations inspired by Redis AOF:
/// - Pipelined writes (batch multiple operations)
/// - Delayed fsync (group commit)
/// - Buffer reuse (avoid allocations)
/// - Background fsync thread (non-blocking)
#[derive(Clone)]
pub struct OptimizedWAL {
    writer_tx: mpsc::UnboundedSender<WriteCommand>,
    current_offset: Arc<AtomicU64>,
    stats: Arc<Mutex<WALStats>>,
}

#[derive(Debug, Default)]
pub struct WALStats {
    pub total_writes: u64,
    pub total_bytes: u64,
    pub batches_written: u64,
    pub fsyncs_performed: u64,
}

enum WriteCommand {
    Append {
        operation: Operation,
        response_tx: oneshot::Sender<Result<u64>>,
    },
    Flush {
        response_tx: oneshot::Sender<Result<()>>,
    },
    #[allow(dead_code)]
    Shutdown,
}

impl OptimizedWAL {
    /// Create or open an optimized WAL file
    pub async fn open(config: WALConfig) -> Result<Self> {
        // Create directory if needed
        if let Some(parent) = config.path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        // Open file in append mode
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .read(true)
            .open(&config.path)
            .await?;

        // Large buffer for better throughput (Redis uses 32MB)
        let buffer_size = (config.buffer_size_kb * 1024).max(32 * 1024); // Min 32KB
        let writer = BufWriter::with_capacity(buffer_size, file);

        // Scan for last offset
        let current_offset = Arc::new(AtomicU64::new(0));
        let last_offset = Self::scan_for_last_offset(&config.path).await?;
        current_offset.store(last_offset, Ordering::SeqCst);

        info!(
            "Optimized WAL opened at {:?}, offset: {}, buffer: {}KB",
            config.path,
            last_offset,
            buffer_size / 1024
        );

        let (writer_tx, writer_rx) = mpsc::unbounded_channel();
        let stats = Arc::new(Mutex::new(WALStats::default()));

        // Spawn background writer with Redis-style batching
        let offset_clone = Arc::clone(&current_offset);
        let stats_clone = Arc::clone(&stats);
        tokio::spawn(Self::redis_style_writer_loop(
            writer,
            writer_rx,
            offset_clone,
            stats_clone,
            config.clone(),
        ));

        Ok(Self {
            writer_tx,
            current_offset,
            stats,
        })
    }

    /// Redis-style writer loop with advanced batching
    async fn redis_style_writer_loop(
        mut writer: BufWriter<File>,
        mut rx: mpsc::UnboundedReceiver<WriteCommand>,
        current_offset: Arc<AtomicU64>,
        stats: Arc<Mutex<WALStats>>,
        config: WALConfig,
    ) {
        const MAX_BATCH_SIZE: usize = 10_000; // Redis can batch thousands
        let batch_timeout = Duration::from_micros(100); // 100µs micro-batching

        let mut batch: Vec<WriteCommand> = Vec::with_capacity(MAX_BATCH_SIZE);
        let mut last_fsync = std::time::Instant::now();
        let fsync_interval = Duration::from_millis(config.fsync_interval_ms);

        info!(
            "WAL writer loop started with {} fsync mode",
            match config.fsync_mode {
                FsyncMode::Always => "Always",
                FsyncMode::Periodic => "Periodic",
                FsyncMode::Never => "Never",
            }
        );

        loop {
            // Micro-batching: collect operations for 100µs or until batch full
            let deadline = tokio::time::sleep(batch_timeout);
            tokio::pin!(deadline);

            tokio::select! {
                Some(cmd) = rx.recv() => {
                    if matches!(cmd, WriteCommand::Shutdown) {
                        info!("WAL shutdown requested");
                        break;
                    }
                    batch.push(cmd);

                    // Opportunistic batching (non-blocking)
                    while batch.len() < MAX_BATCH_SIZE {
                        match rx.try_recv() {
                            Ok(cmd) => {
                                if matches!(cmd, WriteCommand::Shutdown) {
                                    break;
                                }
                                batch.push(cmd);
                            }
                            Err(_) => break,
                        }
                    }
                }

                _ = &mut deadline, if !batch.is_empty() => {
                    // Timeout - process accumulated batch
                }

                else => break,
            }

            if batch.is_empty() {
                continue;
            }

            // Process batch
            let batch_size = batch.len();
            let mut appends = Vec::new();
            let mut flushes = Vec::new();

            for cmd in batch.drain(..) {
                match cmd {
                    WriteCommand::Append {
                        operation,
                        response_tx,
                    } => {
                        appends.push((operation, response_tx));
                    }
                    WriteCommand::Flush { response_tx } => {
                        flushes.push(response_tx);
                    }
                    WriteCommand::Shutdown => break,
                }
            }

            // Write all append operations
            let mut write_results = Vec::new();
            for (operation, _) in &appends {
                let offset = current_offset.fetch_add(1, Ordering::SeqCst);
                let entry = WALEntry {
                    offset,
                    timestamp: Self::current_timestamp(),
                    operation: operation.clone(),
                };

                match Self::write_entry(&mut writer, &entry).await {
                    Ok(bytes_written) => {
                        write_results.push(Ok(offset));
                        let mut stats_guard = stats.lock().await;
                        stats_guard.total_writes += 1;
                        stats_guard.total_bytes += bytes_written;
                    }
                    Err(e) => {
                        write_results.push(Err(e));
                    }
                }
            }

            // Flush buffer to OS
            if let Err(e) = writer.flush().await {
                warn!("WAL buffer flush failed: {}", e);
            } else {
                let mut stats_guard = stats.lock().await;
                stats_guard.batches_written += 1;
                drop(stats_guard);
            }

            // Fsync based on mode (Redis-style)
            let should_fsync = match config.fsync_mode {
                FsyncMode::Always => true,
                FsyncMode::Periodic => last_fsync.elapsed() >= fsync_interval,
                FsyncMode::Never => false,
            };

            if should_fsync {
                if let Ok(_file) = writer.get_mut().sync_all().await {
                    last_fsync = std::time::Instant::now();
                    let mut stats_guard = stats.lock().await;
                    stats_guard.fsyncs_performed += 1;
                    debug!("WAL fsync performed, batch size: {}", batch_size);
                }
            }

            // Send responses
            for (result, (_, response_tx)) in write_results.into_iter().zip(appends.into_iter()) {
                let _ = response_tx.send(result);
            }

            // Respond to flush requests
            for response_tx in flushes {
                let _ = response_tx.send(Ok(()));
            }
        }

        // Final flush on shutdown
        if let Err(e) = writer.flush().await {
            warn!("Final WAL flush failed: {}", e);
        }
        if let Err(e) = writer.get_mut().sync_all().await {
            warn!("Final WAL fsync failed: {}", e);
        }

        info!("WAL writer loop terminated");
    }

    /// Write a single entry to the WAL
    async fn write_entry(writer: &mut BufWriter<File>, entry: &WALEntry) -> Result<u64> {
        let data = bincode::serialize(entry)?;
        let size = data.len() as u64;

        // Checksum (CRC32)
        let checksum = crc32fast::hash(&data);

        // Write: [size (8 bytes)][checksum (4 bytes)][data]
        writer.write_u64(size).await?;
        writer.write_u32(checksum).await?;
        writer.write_all(&data).await?;

        Ok(size + 12) // Total bytes written
    }

    /// Append operation to WAL (non-blocking)
    pub async fn append(&self, operation: Operation) -> Result<u64> {
        let (response_tx, response_rx) = oneshot::channel();

        self.writer_tx
            .send(WriteCommand::Append {
                operation,
                response_tx,
            })
            .map_err(|_| PersistenceError::IOError(std::io::Error::other("WAL writer closed")))?;

        response_rx
            .await
            .map_err(|_| PersistenceError::IOError(std::io::Error::other("WAL response lost")))?
    }

    /// Force flush (useful for testing)
    pub async fn flush(&self) -> Result<()> {
        let (response_tx, response_rx) = oneshot::channel();

        self.writer_tx
            .send(WriteCommand::Flush { response_tx })
            .map_err(|_| PersistenceError::IOError(std::io::Error::other("WAL writer closed")))?;

        response_rx
            .await
            .map_err(|_| PersistenceError::IOError(std::io::Error::other("WAL response lost")))?
    }

    /// Get current offset
    pub fn current_offset(&self) -> u64 {
        self.current_offset.load(Ordering::SeqCst)
    }

    /// Get statistics
    pub async fn stats(&self) -> WALStats {
        let guard = self.stats.lock().await;
        WALStats {
            total_writes: guard.total_writes,
            total_bytes: guard.total_bytes,
            batches_written: guard.batches_written,
            fsyncs_performed: guard.fsyncs_performed,
        }
    }

    fn current_timestamp() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }

    async fn scan_for_last_offset(path: &PathBuf) -> Result<u64> {
        if tokio::fs::metadata(path).await.is_err() {
            return Ok(0);
        }

        let mut file = File::open(path).await?;
        let mut max_offset = 0u64;

        loop {
            let size = match file.read_u64().await {
                Ok(s) => s,
                Err(_) => break,
            };

            let _checksum = match file.read_u32().await {
                Ok(c) => c,
                Err(_) => break,
            };

            let mut data = vec![0u8; size as usize];
            if file.read_exact(&mut data).await.is_err() {
                break;
            }

            if let Ok(entry) = bincode::deserialize::<WALEntry>(&data) {
                max_offset = max_offset.max(entry.offset);
            }
        }

        Ok(max_offset + 1)
    }

    /// Read all entries from WAL (for recovery)
    pub async fn read_all(&self, path: &PathBuf) -> Result<Vec<WALEntry>> {
        let mut file = File::open(path).await?;
        let mut entries = Vec::new();

        loop {
            let size = match file.read_u64().await {
                Ok(s) => s,
                Err(_) => break,
            };

            let expected_checksum = match file.read_u32().await {
                Ok(c) => c,
                Err(_) => break,
            };

            let mut data = vec![0u8; size as usize];
            if file.read_exact(&mut data).await.is_err() {
                warn!("Incomplete WAL entry during recovery");
                break;
            }

            let actual_checksum = crc32fast::hash(&data);
            if actual_checksum != expected_checksum {
                warn!("Checksum mismatch in WAL, stopping recovery");
                break;
            }

            match bincode::deserialize::<WALEntry>(&data) {
                Ok(entry) => entries.push(entry),
                Err(e) => {
                    warn!("Failed to deserialize WAL entry: {}", e);
                    break;
                }
            }
        }

        info!("Read {} entries from WAL", entries.len());
        Ok(entries)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_optimized_wal_batching() {
        let temp_dir = std::env::temp_dir().join("synap_wal_test");
        let _ = tokio::fs::remove_dir_all(&temp_dir).await;
        tokio::fs::create_dir_all(&temp_dir).await.unwrap();

        let config = WALConfig {
            enabled: true,
            path: temp_dir.join("test.wal"),
            buffer_size_kb: 64,
            fsync_mode: FsyncMode::Periodic,
            fsync_interval_ms: 100,
            max_size_mb: 10,
        };

        let wal = OptimizedWAL::open(config.clone()).await.unwrap();

        // Write multiple operations
        let mut handles = vec![];
        for i in 0..100 {
            let wal_clone = wal.clone();
            let handle = tokio::spawn(async move {
                wal_clone
                    .append(Operation::KVSet {
                        key: format!("key_{}", i),
                        value: vec![i as u8; 100],
                        ttl: None,
                    })
                    .await
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.await.unwrap().unwrap();
        }

        // Force flush
        wal.flush().await.unwrap();

        // Read back
        let entries = wal.read_all(&config.path).await.unwrap();
        assert_eq!(entries.len(), 100);

        // Cleanup
        let _ = tokio::fs::remove_dir_all(&temp_dir).await;
    }
}
