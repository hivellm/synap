use super::types::{FsyncMode, Operation, PersistenceError, Result, WALEntry, WALConfig};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::SystemTime;
use tokio::fs::{File, OpenOptions};
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufWriter};
use tracing::{debug, info, warn};

/// Write-Ahead Log for durability
pub struct WriteAheadLog {
    file: BufWriter<File>,
    path: PathBuf,
    current_offset: Arc<AtomicU64>,
    config: WALConfig,
    last_fsync: std::time::Instant,
}

impl WriteAheadLog {
    /// Create or open a WAL file
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

        let mut wal = Self {
            file: BufWriter::with_capacity(config.buffer_size_kb * 1024, file),
            path: config.path.clone(),
            current_offset: Arc::new(AtomicU64::new(0)),
            config,
            last_fsync: std::time::Instant::now(),
        };

        // Read current offset from file
        wal.scan_for_last_offset().await?;

        info!(
            "WAL opened at {:?}, current offset: {}",
            wal.path,
            wal.current_offset.load(Ordering::SeqCst)
        );

        Ok(wal)
    }

    /// Scan WAL file to find the last valid offset
    async fn scan_for_last_offset(&mut self) -> Result<()> {
        let mut file = File::open(&self.path).await?;
        let mut max_offset = 0u64;

        loop {
            // Try to read entry size
            let size = match file.read_u64().await {
                Ok(s) => s,
                Err(_) => break,  // EOF
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

        self.current_offset.store(max_offset + 1, Ordering::SeqCst);
        Ok(())
    }

    /// Append an operation to the WAL
    pub async fn append(&mut self, operation: Operation) -> Result<u64> {
        let offset = self.current_offset.fetch_add(1, Ordering::SeqCst);

        let entry = WALEntry {
            offset,
            timestamp: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            operation,
        };

        // Serialize entry
        let data = bincode::serialize(&entry)?;
        let checksum = crc32fast::hash(&data);

        debug!("WAL append: offset={}, size={}", offset, data.len());

        // Write entry format:
        // - size (u64)
        // - checksum (u32)
        // - data (serialized entry)
        self.file.write_u64(data.len() as u64).await?;
        self.file.write_u32(checksum).await?;
        self.file.write_all(&data).await?;

        // Handle fsync based on mode
        match self.config.fsync_mode {
            FsyncMode::Always => {
                self.file.flush().await?;
                self.file.get_ref().sync_all().await?;
            }
            FsyncMode::Periodic => {
                let elapsed = self.last_fsync.elapsed();
                if elapsed.as_millis() >= self.config.fsync_interval_ms as u128 {
                    self.file.flush().await?;
                    self.file.get_ref().sync_all().await?;
                    self.last_fsync = std::time::Instant::now();
                }
            }
            FsyncMode::Never => {
                // No fsync, rely on OS buffer flush
            }
        }

        Ok(offset)
    }

    /// Replay WAL entries from a specific offset
    pub async fn replay(&self, from_offset: u64) -> Result<Vec<WALEntry>> {
        let mut file = File::open(&self.path).await?;
        let mut entries = Vec::new();

        debug!("Replaying WAL from offset {}", from_offset);

        loop {
            // Read entry size
            let size = match file.read_u64().await {
                Ok(s) => s,
                Err(_) => break,  // EOF
            };

            // Read checksum
            let checksum_expected = match file.read_u32().await {
                Ok(c) => c,
                Err(_) => {
                    warn!("Incomplete WAL entry header");
                    break;
                }
            };

            // Read entry data
            let mut data = vec![0u8; size as usize];
            if file.read_exact(&mut data).await.is_err() {
                warn!("Incomplete WAL entry data");
                break;
            }

            // Verify checksum
            let checksum_actual = crc32fast::hash(&data);
            if checksum_actual != checksum_expected {
                warn!(
                    "Checksum mismatch: expected {}, got {}",
                    checksum_expected, checksum_actual
                );
                return Err(PersistenceError::ChecksumMismatch {
                    expected: checksum_expected as u64,
                    actual: checksum_actual as u64,
                });
            }

            // Deserialize
            let entry: WALEntry = bincode::deserialize(&data).map_err(|_| {
                PersistenceError::InvalidEntry
            })?;

            if entry.offset >= from_offset {
                entries.push(entry);
            }
        }

        info!("Replayed {} WAL entries", entries.len());
        Ok(entries)
    }

    /// Get current offset
    pub fn current_offset(&self) -> u64 {
        self.current_offset.load(Ordering::SeqCst)
    }

    /// Truncate WAL (keep entries after specified offset)
    pub async fn truncate(&mut self, keep_after_offset: u64) -> Result<()> {
        info!("Truncating WAL, keeping entries after offset {}", keep_after_offset);

        // Create new WAL file
        let new_path = self.path.with_extension("wal.new");
        let new_file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&new_path)
            .await?;
        
        let mut new_writer = BufWriter::new(new_file);

        // Replay and copy entries
        let entries = self.replay(keep_after_offset).await?;
        
        for entry in entries {
            let data = bincode::serialize(&entry)?;
            let checksum = crc32fast::hash(&data);

            new_writer.write_u64(data.len() as u64).await?;
            new_writer.write_u32(checksum).await?;
            new_writer.write_all(&data).await?;
        }

        new_writer.flush().await?;
        new_writer.get_ref().sync_all().await?;
        drop(new_writer);

        // Atomic rename
        tokio::fs::rename(&new_path, &self.path).await?;

        // Reopen file
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .await?;

        self.file = BufWriter::with_capacity(self.config.buffer_size_kb * 1024, file);

        info!("WAL truncated successfully");
        Ok(())
    }

    /// Flush pending writes
    pub async fn flush(&mut self) -> Result<()> {
        self.file.flush().await?;
        self.file.get_ref().sync_all().await?;
        Ok(())
    }
}

impl Drop for WriteAheadLog {
    fn drop(&mut self) {
        // Note: Cannot flush asynchronously in Drop
        // Users should call flush() explicitly before dropping
    }
}

