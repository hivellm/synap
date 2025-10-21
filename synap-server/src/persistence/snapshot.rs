use super::types::{PersistenceError, Result, Snapshot, SnapshotConfig};
use crate::core::kv_store::KVStore;
use crate::core::queue::QueueManager;
use std::path::PathBuf;
use std::time::SystemTime;
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tracing::{debug, info, warn};

/// Snapshot manager for periodic state dumps
pub struct SnapshotManager {
    config: SnapshotConfig,
}

impl SnapshotManager {
    /// Create a new snapshot manager
    pub fn new(config: SnapshotConfig) -> Self {
        Self { config }
    }

    /// Create a snapshot of the current state
    pub async fn create_snapshot(
        &self,
        kv_store: &KVStore,
        queue_manager: Option<&QueueManager>,
        wal_offset: u64,
    ) -> Result<PathBuf> {
        // Create directory if it doesn't exist
        tokio::fs::create_dir_all(&self.config.directory).await?;

        let timestamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let filename = format!("snapshot-{}.bin", timestamp);
        let path = self.config.directory.join(&filename);

        info!("Creating snapshot at {:?}", path);

        // Collect KV data
        let kv_data = kv_store.dump().await?;

        // Collect queue data (if available)
        let queue_data = if let Some(qm) = queue_manager {
            qm.dump().await?
        } else {
            std::collections::HashMap::new()
        };

        let snapshot = Snapshot {
            version: 1,
            timestamp,
            wal_offset,
            kv_data,
            queue_data,
        };

        // Serialize snapshot
        let data = bincode::serialize(&snapshot)?;
        let checksum = crc64fast::digest(&data);

        debug!(
            "Snapshot size: {} bytes, checksum: {}",
            data.len(),
            checksum
        );

        // Write to file: checksum (u64) + data
        let mut file = File::create(&path).await?;
        file.write_u64(checksum).await?;
        file.write_all(&data).await?;
        file.sync_all().await?;

        info!("Snapshot created successfully: {:?}", path);

        // Cleanup old snapshots
        self.cleanup_old_snapshots().await?;

        Ok(path)
    }

    /// Load the latest snapshot
    pub async fn load_latest(&self) -> Result<Option<(Snapshot, PathBuf)>> {
        let snapshots = self.list_snapshots().await?;

        if snapshots.is_empty() {
            info!("No snapshots found");
            return Ok(None);
        }

        // Get the most recent snapshot
        let latest = &snapshots[snapshots.len() - 1];
        info!("Loading snapshot from {:?}", latest);

        let mut file = File::open(latest).await?;

        // Read checksum
        let checksum_expected = file.read_u64().await?;

        // Read data
        let mut data = Vec::new();
        file.read_to_end(&mut data).await?;

        // Verify checksum
        let checksum_actual = crc64fast::digest(&data);
        if checksum_actual != checksum_expected {
            warn!(
                "Snapshot checksum mismatch: expected {}, got {}",
                checksum_expected, checksum_actual
            );
            return Err(PersistenceError::SnapshotCorrupted(latest.clone()));
        }

        // Deserialize
        let snapshot: Snapshot = bincode::deserialize(&data)?;

        info!(
            "Snapshot loaded successfully: version={}, timestamp={}, wal_offset={}",
            snapshot.version, snapshot.timestamp, snapshot.wal_offset
        );

        Ok(Some((snapshot, latest.clone())))
    }

    /// List all snapshots in directory (sorted by timestamp)
    async fn list_snapshots(&self) -> Result<Vec<PathBuf>> {
        if !self.config.directory.exists() {
            return Ok(Vec::new());
        }

        let mut snapshots = Vec::new();
        let mut dir = tokio::fs::read_dir(&self.config.directory).await?;

        while let Some(entry) = dir.next_entry().await? {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("bin") {
                if let Some(filename) = path.file_name() {
                    if filename.to_string_lossy().starts_with("snapshot-") {
                        snapshots.push(path);
                    }
                }
            }
        }

        // Sort by filename (which includes timestamp)
        snapshots.sort();

        Ok(snapshots)
    }

    /// Cleanup old snapshots, keeping only the configured number
    async fn cleanup_old_snapshots(&self) -> Result<()> {
        let mut snapshots = self.list_snapshots().await?;

        if snapshots.len() <= self.config.max_snapshots {
            return Ok(());
        }

        // Remove oldest snapshots
        snapshots.sort();
        let to_remove = snapshots.len() - self.config.max_snapshots;

        for snapshot in snapshots.iter().take(to_remove) {
            info!("Removing old snapshot: {:?}", snapshot);
            tokio::fs::remove_file(snapshot).await?;
        }

        Ok(())
    }

    /// Get snapshot statistics
    pub async fn stats(&self) -> Result<SnapshotStats> {
        let snapshots = self.list_snapshots().await?;

        let mut total_size = 0u64;
        for snapshot in &snapshots {
            if let Ok(metadata) = tokio::fs::metadata(snapshot).await {
                total_size += metadata.len();
            }
        }

        Ok(SnapshotStats {
            count: snapshots.len(),
            total_size_bytes: total_size,
            latest: snapshots.last().cloned(),
        })
    }
}

/// Snapshot statistics
#[derive(Debug)]
pub struct SnapshotStats {
    pub count: usize,
    pub total_size_bytes: u64,
    pub latest: Option<PathBuf>,
}

// CRC64 implementation (simple version)
mod crc64fast {
    pub fn digest(data: &[u8]) -> u64 {
        // Simple CRC64 using polynomial 0x42F0E1EBA9EA3693
        let mut crc = 0xFFFF_FFFF_FFFF_FFFFu64;
        for &byte in data {
            crc ^= byte as u64;
            for _ in 0..8 {
                if crc & 1 == 1 {
                    crc = (crc >> 1) ^ 0x42F0_E1EB_A9EA_3693;
                } else {
                    crc >>= 1;
                }
            }
        }
        !crc
    }
}

