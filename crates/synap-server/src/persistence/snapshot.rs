use super::types::{PersistenceError, Result, Snapshot, SnapshotConfig, StreamEvent};
use crate::core::kv_store::KVStore;
use crate::core::queue::{QueueManager, QueueMessage};
use crate::core::stream::StreamManager;
use crate::core::{HashStore, ListStore, SetStore, SortedSetStore};
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::SystemTime;
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader, BufWriter};
use tracing::{debug, info, warn};

/// v2 = kv + queue + stream. v3 (current) also persists hash/list/set/sorted-set.
const SNAPSHOT_VERSION: u8 = 3;
const SNAPSHOT_MAGIC: &[u8; 8] = b"SYNAP003";
const SNAPSHOT_MAGIC_V2: &[u8; 8] = b"SYNAP002";

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
    #[allow(clippy::too_many_arguments)]
    pub async fn create_snapshot(
        &self,
        kv_store: &KVStore,
        hash_store: Option<&HashStore>,
        list_store: Option<&ListStore>,
        set_store: Option<&SetStore>,
        sorted_set_store: Option<&SortedSetStore>,
        queue_manager: Option<&QueueManager>,
        stream_manager: Option<&StreamManager>,
        wal_offset: u64,
    ) -> Result<PathBuf> {
        // Create directory if it doesn't exist
        tokio::fs::create_dir_all(&self.config.directory).await?;

        let timestamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let filename = format!("snapshot-v{}-{}.bin", SNAPSHOT_VERSION, timestamp);
        let path = self.config.directory.join(&filename);

        info!("Creating streaming snapshot at {:?}", path);

        let file = File::create(&path).await?;
        let mut writer = BufWriter::new(file);
        let mut checksum = CRC64::new();

        // Write header: magic + version + timestamp + wal_offset
        writer.write_all(SNAPSHOT_MAGIC).await?;
        checksum.update(SNAPSHOT_MAGIC);

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
                let msg_data = bincode::serde::encode_to_vec(&message, bincode::config::legacy())
                    .map_err(std::io::Error::other)?;
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

                let event_data =
                    bincode::serde::encode_to_vec(&snapshot_event, bincode::config::legacy())
                        .map_err(std::io::Error::other)?;
                let event_len = event_data.len() as u32;

                writer.write_u32(event_len).await?;
                checksum.update(&event_len.to_le_bytes());

                writer.write_all(&event_data).await?;
                checksum.update(&event_data);
            }
        }

        // Stream hash / list / set / sorted-set data (v3+).
        let hash_data = match hash_store {
            Some(hs) => hs.dump(),
            None => HashMap::new(),
        };
        debug!("Streaming {} hashes", hash_data.len());
        write_map_section(&mut writer, &mut checksum, &hash_data).await?;

        let list_data = match list_store {
            Some(ls) => ls.dump(),
            None => HashMap::new(),
        };
        debug!("Streaming {} lists", list_data.len());
        write_map_section(&mut writer, &mut checksum, &list_data).await?;

        let set_data = match set_store {
            Some(ss) => ss.dump(),
            None => HashMap::new(),
        };
        debug!("Streaming {} sets", set_data.len());
        write_map_section(&mut writer, &mut checksum, &set_data).await?;

        let sorted_set_data = match sorted_set_store {
            Some(zs) => zs.dump(),
            None => HashMap::new(),
        };
        debug!("Streaming {} sorted sets", sorted_set_data.len());
        write_map_section(&mut writer, &mut checksum, &sorted_set_data).await?;

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

        // Running digest, updated with the exact same byte sequence the writer
        // fed into its CRC64 (LE for numeric fields, raw bytes for data), so we
        // can verify integrity against the trailing checksum at the end.
        let mut checksum = CRC64::new();

        // Read header: magic (8 bytes) + version (1 byte)
        let mut magic = [0u8; 8];
        reader.read_exact(&mut magic).await?;
        checksum.update(&magic);

        // v3 (SNAPSHOT_MAGIC) carries the hash/list/set/sorted-set sections;
        // v2 (SNAPSHOT_MAGIC_V2) does not. Any other magic is unreadable.
        let has_collections = if &magic == SNAPSHOT_MAGIC {
            true
        } else if &magic == SNAPSHOT_MAGIC_V2 {
            false
        } else {
            return Err(PersistenceError::SnapshotCorrupted(latest.clone()));
        };

        let version = reader.read_u8().await?;
        checksum.update(&[version]);
        if version != 2 && version != SNAPSHOT_VERSION {
            warn!(
                "Unsupported snapshot version: expected 2 or {}, got {}",
                SNAPSHOT_VERSION, version
            );
            return Err(PersistenceError::SnapshotCorrupted(latest.clone()));
        }

        // Read metadata
        let timestamp = reader.read_u64().await?;
        checksum.update(&timestamp.to_le_bytes());
        let wal_offset = reader.read_u64().await?;
        checksum.update(&wal_offset.to_le_bytes());

        // Read KV data
        let kv_count = reader.read_u64().await?;
        checksum.update(&kv_count.to_le_bytes());
        let mut kv_data = HashMap::new();

        for _ in 0..kv_count {
            let key_len = reader.read_u32().await?;
            checksum.update(&key_len.to_le_bytes());
            let mut key_bytes = vec![0u8; key_len as usize];
            reader.read_exact(&mut key_bytes).await?;
            checksum.update(&key_bytes);
            let key = String::from_utf8(key_bytes)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

            let value_len = reader.read_u32().await?;
            checksum.update(&value_len.to_le_bytes());
            let mut value = vec![0u8; value_len as usize];
            reader.read_exact(&mut value).await?;
            checksum.update(&value);

            kv_data.insert(key, value);
        }

        // Read Queue data
        let queue_count = reader.read_u64().await?;
        checksum.update(&queue_count.to_le_bytes());
        let mut queue_data = HashMap::new();

        for _ in 0..queue_count {
            let queue_len = reader.read_u32().await?;
            checksum.update(&queue_len.to_le_bytes());
            let mut queue_bytes = vec![0u8; queue_len as usize];
            reader.read_exact(&mut queue_bytes).await?;
            checksum.update(&queue_bytes);
            let queue_name = String::from_utf8(queue_bytes)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

            let msg_count = reader.read_u64().await?;
            checksum.update(&msg_count.to_le_bytes());
            let mut messages = Vec::new();

            for _ in 0..msg_count {
                let msg_len = reader.read_u32().await?;
                checksum.update(&msg_len.to_le_bytes());
                let mut msg_bytes = vec![0u8; msg_len as usize];
                reader.read_exact(&mut msg_bytes).await?;
                checksum.update(&msg_bytes);

                let (message, _): (QueueMessage, _) =
                    bincode::serde::decode_from_slice(&msg_bytes, bincode::config::legacy())
                        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
                messages.push(message);
            }

            queue_data.insert(queue_name, messages);
        }

        // Read Stream data (always present in the v2 format, count may be 0)
        let stream_count = reader.read_u64().await?;
        checksum.update(&stream_count.to_le_bytes());
        let mut stream_data = HashMap::new();
        for _ in 0..stream_count {
            let room_len = reader.read_u32().await?;
            checksum.update(&room_len.to_le_bytes());
            let mut room_bytes = vec![0u8; room_len as usize];
            reader.read_exact(&mut room_bytes).await?;
            checksum.update(&room_bytes);
            let room_name = String::from_utf8(room_bytes)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

            let event_count = reader.read_u64().await?;
            checksum.update(&event_count.to_le_bytes());
            let mut events = Vec::new();

            for _ in 0..event_count {
                let event_len = reader.read_u32().await?;
                checksum.update(&event_len.to_le_bytes());
                let mut event_bytes = vec![0u8; event_len as usize];
                reader.read_exact(&mut event_bytes).await?;
                checksum.update(&event_bytes);

                let (event, _): (StreamEvent, _) =
                    bincode::serde::decode_from_slice(&event_bytes, bincode::config::legacy())
                        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
                events.push(event);
            }

            stream_data.insert(room_name, events);
        }

        // Read hash / list / set / sorted-set sections (v3+; absent in v2).
        let (hash_data, list_data, set_data, sorted_set_data) = if has_collections {
            let hash_data = read_map_section(&mut reader, &mut checksum).await?;
            let list_data = read_map_section(&mut reader, &mut checksum).await?;
            let set_data = read_map_section(&mut reader, &mut checksum).await?;
            let sorted_set_data = read_map_section(&mut reader, &mut checksum).await?;
            (hash_data, list_data, set_data, sorted_set_data)
        } else {
            (
                HashMap::new(),
                HashMap::new(),
                HashMap::new(),
                HashMap::new(),
            )
        };

        // Verify integrity: the trailing CRC64 must match the running digest.
        let stored_checksum = reader.read_u64().await?;
        let computed_checksum = checksum.finalize();
        if stored_checksum != computed_checksum {
            warn!(
                "Snapshot checksum mismatch at {:?} (stored={:#x}, computed={:#x})",
                latest, stored_checksum, computed_checksum
            );
            return Err(PersistenceError::SnapshotCorrupted(latest.clone()));
        }

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
            list_data,
            set_data,
            sorted_set_data,
            hash_data,
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

/// Write a `HashMap<String, V>` section: count (u64), then per entry the UTF-8
/// key (u32 length-prefixed) and the bincode-encoded value (u32 length-prefixed).
/// The running CRC64 is fed the same bytes the reader will accumulate.
async fn write_map_section<W, V>(
    writer: &mut W,
    checksum: &mut CRC64,
    map: &HashMap<String, V>,
) -> std::io::Result<()>
where
    W: AsyncWriteExt + Unpin,
    V: Serialize,
{
    let count = map.len() as u64;
    writer.write_u64(count).await?;
    checksum.update(&count.to_le_bytes());
    for (key, value) in map {
        let key_bytes = key.as_bytes();
        let key_len = key_bytes.len() as u32;
        writer.write_u32(key_len).await?;
        checksum.update(&key_len.to_le_bytes());
        writer.write_all(key_bytes).await?;
        checksum.update(key_bytes);

        let data = bincode::serde::encode_to_vec(value, bincode::config::legacy())
            .map_err(std::io::Error::other)?;
        let data_len = data.len() as u32;
        writer.write_u32(data_len).await?;
        checksum.update(&data_len.to_le_bytes());
        writer.write_all(&data).await?;
        checksum.update(&data);
    }
    Ok(())
}

/// Read a section written by [`write_map_section`], updating `checksum` with the
/// same byte sequence so the trailing digest can be verified.
async fn read_map_section<R, V>(reader: &mut R, checksum: &mut CRC64) -> Result<HashMap<String, V>>
where
    R: AsyncReadExt + Unpin,
    V: DeserializeOwned,
{
    let count = reader.read_u64().await?;
    checksum.update(&count.to_le_bytes());
    let mut out = HashMap::new();
    for _ in 0..count {
        let key_len = reader.read_u32().await?;
        checksum.update(&key_len.to_le_bytes());
        let mut key_bytes = vec![0u8; key_len as usize];
        reader.read_exact(&mut key_bytes).await?;
        checksum.update(&key_bytes);
        let key = String::from_utf8(key_bytes)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

        let data_len = reader.read_u32().await?;
        checksum.update(&data_len.to_le_bytes());
        let mut data = vec![0u8; data_len as usize];
        reader.read_exact(&mut data).await?;
        checksum.update(&data);
        let (value, _) = bincode::serde::decode_from_slice(&data, bincode::config::legacy())
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        out.insert(key, value);
    }
    Ok(out)
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
