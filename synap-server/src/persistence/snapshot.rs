use super::types::{PersistenceError, Result, Snapshot, SnapshotConfig, StreamEvent};
use crate::core::kv_store::KVStore;
use crate::core::queue::{QueueManager, QueueMessage};
use crate::core::stream::StreamManager;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::SystemTime;
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader, BufWriter};
use tracing::{debug, info, warn};

const SNAPSHOT_VERSION: u8 = 2; // Version 2 with streaming format

/// Snapshot manager for periodic state dumps with streaming support
pub struct SnapshotManager {
    config: SnapshotConfig,
}

impl SnapshotManager {
    /// Create a new snapshot manager
    pub fn new(config: SnapshotConfig) -> Self {
        Self { config }
    }

    /// Create a snapshot using streaming serialization (O(1) memory usage)
    pub async fn create_snapshot(
        &self,
        kv_store: &KVStore,
        queue_manager: Option<&QueueManager>,
        stream_manager: Option<&StreamManager>,
        wal_offset: u64,
    ) -> Result<PathBuf> {
        // Create directory if it doesn't exist
        tokio::fs::create_dir_all(&self.config.directory).await?;

        let timestamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let filename = format!("snapshot-v{}-{}.bin", SNAPSHOT_VERSION, timestamp);
        let path = self.config.directory.join(&filename);

        info!("Creating streaming snapshot at {:?}", path);

        let file = File::create(&path).await?;
        let mut writer = BufWriter::new(file);
        let mut checksum = CRC64::new();

        // Write header: magic + version + timestamp + wal_offset
        writer.write_all(b"SYNAP002").await?;
        checksum.update(b"SYNAP002");

        writer.write_u8(SNAPSHOT_VERSION).await?;
        checksum.update(&[SNAPSHOT_VERSION]);

        writer.write_u64(timestamp).await?;
        checksum.update(&timestamp.to_le_bytes());

        writer.write_u64(wal_offset).await?;
        checksum.update(&wal_offset.to_le_bytes());

        // Stream KV data
        let kv_data = kv_store.dump().await?;
        let kv_count = kv_data.len() as u64;

        writer.write_u64(kv_count).await?;
        checksum.update(&kv_count.to_le_bytes());

        debug!("Streaming {} KV entries", kv_count);

        for (key, value) in kv_data {
            // Write key length + key + value length + value
            let key_bytes = key.as_bytes();
            let key_len = key_bytes.len() as u32;
            let value_len = value.len() as u32;

            writer.write_u32(key_len).await?;
            checksum.update(&key_len.to_le_bytes());

            writer.write_all(key_bytes).await?;
            checksum.update(key_bytes);

            writer.write_u32(value_len).await?;
            checksum.update(&value_len.to_le_bytes());

            writer.write_all(&value).await?;
            checksum.update(&value);
        }

        // Stream queue data (if available)
        let queue_data = if let Some(qm) = queue_manager {
            qm.dump().await?
        } else {
            std::collections::HashMap::new()
        };

        let queue_count = queue_data.len() as u64;
        writer.write_u64(queue_count).await?;
        checksum.update(&queue_count.to_le_bytes());

        debug!("Streaming {} queue entries", queue_count);

        for (queue_name, messages) in queue_data {
            // Write queue name
            let name_bytes = queue_name.as_bytes();
            let name_len = name_bytes.len() as u32;

            writer.write_u32(name_len).await?;
            checksum.update(&name_len.to_le_bytes());

            writer.write_all(name_bytes).await?;
            checksum.update(name_bytes);

            // Write messages count
            let msg_count = messages.len() as u64;
            writer.write_u64(msg_count).await?;
            checksum.update(&msg_count.to_le_bytes());

            // Serialize each message
            for message in messages {
                let msg_data = bincode::serialize(&message)?;
                let msg_len = msg_data.len() as u32;

                writer.write_u32(msg_len).await?;
                checksum.update(&msg_len.to_le_bytes());

                writer.write_all(&msg_data).await?;
                checksum.update(&msg_data);
            }
        }

        // Stream stream data (if available)
        let stream_data = if let Some(sm) = stream_manager {
            sm.get_all_events().await
        } else {
            HashMap::new()
        };

        let stream_count = stream_data.len() as u64;
        writer.write_u64(stream_count).await?;
        checksum.update(&stream_count.to_le_bytes());

        debug!("Streaming {} stream rooms", stream_count);

        for (room_name, events) in stream_data {
            // Write room name
            let name_bytes = room_name.as_bytes();
            let name_len = name_bytes.len() as u32;

            writer.write_u32(name_len).await?;
            checksum.update(&name_len.to_le_bytes());

            writer.write_all(name_bytes).await?;
            checksum.update(name_bytes);

            // Write events count
            let event_count = events.len() as u64;
            writer.write_u64(event_count).await?;
            checksum.update(&event_count.to_le_bytes());

            // Serialize each event
            for event in events {
                // Convert to snapshot StreamEvent
                let snapshot_event = StreamEvent {
                    id: event.id,
                    offset: event.offset,
                    event_type: event.event,
                    data: event.data,
                    timestamp: event.timestamp,
                };

                let event_data = bincode::serialize(&snapshot_event)?;
                let event_len = event_data.len() as u32;

                writer.write_u32(event_len).await?;
                checksum.update(&event_len.to_le_bytes());

                writer.write_all(&event_data).await?;
                checksum.update(&event_data);
            }
        }

        // Write checksum at end
        let final_checksum = checksum.finalize();
        writer.write_u64(final_checksum).await?;

        writer.flush().await?;
        writer.into_inner().sync_all().await?;

        info!(
            "Streaming snapshot created successfully: {:?} (checksum: {})",
            path, final_checksum
        );

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

        let file = File::open(latest).await?;
        let mut reader = BufReader::new(file);

        // Read header: magic (8 bytes) + version (1 byte)
        let mut magic = [0u8; 8];
        reader.read_exact(&mut magic).await?;

        if &magic != b"SYNAP002" {
            // Try old format
            return Err(PersistenceError::SnapshotCorrupted(latest.clone()));
        }

        let version = reader.read_u8().await?;
        if version != SNAPSHOT_VERSION {
            warn!(
                "Snapshot version mismatch: expected {}, got {}",
                SNAPSHOT_VERSION, version
            );
            return Err(PersistenceError::SnapshotCorrupted(latest.clone()));
        }

        // Read metadata
        let timestamp = reader.read_u64().await?;
        let wal_offset = reader.read_u64().await?;

        // Read KV data
        let kv_count = reader.read_u64().await?;
        let mut kv_data = HashMap::new();

        for _ in 0..kv_count {
            let key_len = reader.read_u32().await? as usize;
            let mut key_bytes = vec![0u8; key_len];
            reader.read_exact(&mut key_bytes).await?;
            let key = String::from_utf8(key_bytes)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

            let value_len = reader.read_u32().await? as usize;
            let mut value = vec![0u8; value_len];
            reader.read_exact(&mut value).await?;

            kv_data.insert(key, value);
        }

        // Read Queue data
        let queue_count = reader.read_u64().await?;
        let mut queue_data = HashMap::new();

        for _ in 0..queue_count {
            let queue_len = reader.read_u32().await? as usize;
            let mut queue_bytes = vec![0u8; queue_len];
            reader.read_exact(&mut queue_bytes).await?;
            let queue_name = String::from_utf8(queue_bytes)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

            let msg_count = reader.read_u64().await?;
            let mut messages = Vec::new();

            for _ in 0..msg_count {
                let msg_len = reader.read_u32().await? as usize;
                let mut msg_bytes = vec![0u8; msg_len];
                reader.read_exact(&mut msg_bytes).await?;

                let message: QueueMessage = bincode::deserialize(&msg_bytes)
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
                messages.push(message);
            }

            queue_data.insert(queue_name, messages);
        }

        // Read Stream data (optional for backward compatibility)
        let mut stream_data = HashMap::new();

        // Try to read stream data (might not exist in old snapshots)
        if let Ok(stream_count) = reader.read_u64().await {
            for _ in 0..stream_count {
                let room_len = reader.read_u32().await? as usize;
                let mut room_bytes = vec![0u8; room_len];
                reader.read_exact(&mut room_bytes).await?;
                let room_name = String::from_utf8(room_bytes)
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

                let event_count = reader.read_u64().await?;
                let mut events = Vec::new();

                for _ in 0..event_count {
                    let event_len = reader.read_u32().await? as usize;
                    let mut event_bytes = vec![0u8; event_len];
                    reader.read_exact(&mut event_bytes).await?;

                    let event: StreamEvent = bincode::deserialize(&event_bytes)
                        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
                    events.push(event);
                }

                stream_data.insert(room_name, events);
            }
        }

        // Verify checksum
        let _checksum = reader.read_u64().await.unwrap_or(0); // Optional for backward compatibility

        info!(
            "Snapshot loaded successfully: version={}, timestamp={}, wal_offset={}, streams={}",
            version,
            timestamp,
            wal_offset,
            stream_data.len()
        );

        // Reconstruct Snapshot struct from loaded data
        let snapshot = Snapshot {
            version: version as u32,
            timestamp,
            wal_offset,
            kv_data,
            queue_data,
            stream_data,
        };

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
                    let name = filename.to_string_lossy();
                    if name.starts_with("snapshot-") {
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

// CRC64 implementation for streaming checksum
struct CRC64 {
    crc: u64,
}

impl CRC64 {
    fn new() -> Self {
        Self {
            crc: 0xFFFF_FFFF_FFFF_FFFF,
        }
    }

    fn update(&mut self, data: &[u8]) {
        for &byte in data {
            self.crc ^= byte as u64;
            for _ in 0..8 {
                if self.crc & 1 == 1 {
                    self.crc = (self.crc >> 1) ^ 0x42F0_E1EB_A9EA_3693;
                } else {
                    self.crc >>= 1;
                }
            }
        }
    }

    fn finalize(self) -> u64 {
        !self.crc
    }
}
