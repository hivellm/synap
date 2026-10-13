use super::types::{FsyncMode, Operation, Result, WALConfig, WALEntry};
use super::wal_frame;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::SystemTime;
use tokio::fs::{File, OpenOptions};
use tokio::io::{AsyncWriteExt, BufWriter};
use tracing::{debug, info};

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

        // Verify every frame, truncate a torn tail, and write or upgrade the
        // format header before opening for append; interior corruption and an
        // unsupported format version fail here.
        let report = wal_frame::prepare_async(config.path.clone()).await?;

        // Open or create file (append mode)
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .read(true)
            .open(&config.path)
            .await?;

        let wal = Self {
            file: BufWriter::with_capacity(config.buffer_size_kb * 1024, file),
            path: config.path.clone(),
            current_offset: Arc::new(AtomicU64::new(report.next_offset())),
            config,
            last_fsync: std::time::Instant::now(),
        };

        info!(
            "WAL opened at {:?}, current offset: {}",
            wal.path,
            wal.current_offset.load(Ordering::SeqCst)
        );

        Ok(wal)
    }

    /// Append an operation to the WAL
    pub async fn append(&mut self, operation: Operation) -> Result<u64> {
        let offset = self.current_offset.fetch_add(1, Ordering::SeqCst);

        let entry = WALEntry {
            offset,
            timestamp: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            operation,
        };

        let frame = wal_frame::encode_frame(&entry)?;
        debug!("WAL append: offset={}, frame size={}", offset, frame.len());
        self.file.write_all(&frame).await?;

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

    /// Replay WAL entries from a specific offset.
    ///
    /// Every frame's checksum and length are verified; replay stops at a torn
    /// tail and fails on interior corruption.
    pub async fn replay(&self, from_offset: u64) -> Result<Vec<WALEntry>> {
        debug!("Replaying WAL from offset {}", from_offset);
        let entries = wal_frame::read_entries(self.path.clone(), from_offset).await?;
        info!("Replayed {} WAL entries", entries.len());
        Ok(entries)
    }

    /// Get current offset
    pub fn current_offset(&self) -> u64 {
        self.current_offset.load(Ordering::SeqCst)
    }

    /// Truncate WAL (keep entries after specified offset)
    pub async fn truncate(&mut self, keep_after_offset: u64) -> Result<()> {
        info!(
            "Truncating WAL, keeping entries after offset {}",
            keep_after_offset
        );

        // Create new WAL file
        let new_path = self.path.with_extension("wal.new");
        let new_file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&new_path)
            .await?;

        let mut new_writer = BufWriter::new(new_file);
        new_writer
            .write_all(&wal_frame::encode_file_header())
            .await?;

        // Replay and copy entries
        let entries = self.replay(keep_after_offset).await?;

        for entry in entries {
            new_writer
                .write_all(&wal_frame::encode_frame(&entry)?)
                .await?;
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
