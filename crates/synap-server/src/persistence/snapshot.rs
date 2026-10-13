use super::apply::StoreRefs;
use super::types::{PersistenceError, Result, Snapshot, SnapshotConfig, StreamEvent};
use crate::core::queue::QueueMessage;
use crate::core::{BitmapValue, HyperLogLogValue, ListValue, SetValue, StreamManager};
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::SystemTime;
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader, BufWriter};
use tracing::{debug, info, warn};

/// v2 = kv + queue + stream. v3 adds hash/list/set/sorted-set. v4 (current)
/// adds each KV key's absolute expiry and the HyperLogLog and bitmap sections.
const SNAPSHOT_VERSION: u8 = 4;
const SNAPSHOT_MAGIC: &[u8; 8] = b"SYNAP004";
const SNAPSHOT_MAGIC_V3: &[u8; 8] = b"SYNAP003";
const SNAPSHOT_MAGIC_V2: &[u8; 8] = b"SYNAP002";

/// In-memory copy of every WAL-backed store, taken while the snapshot barrier
/// holds (see [`super::order`]) and serialized after it is released.
///
/// KV values are shared `Arc<[u8]>` buffers, so capturing them is a refcount
/// bump per key; the collections are cloned. Streams are not part of the
/// image: they are not in the WAL, so they need no barrier and are read when
/// the file is written.
///
/// The replication full sync captures the same image under the same barrier
/// (phase31) and reads its fields to build the replica's image.
pub struct SnapshotImage {
    pub(crate) kv: Vec<(String, Arc<[u8]>, Option<u64>)>,
    pub(crate) queues: HashMap<String, Vec<QueueMessage>>,
    pub(crate) hashes: HashMap<String, HashMap<String, Vec<u8>>>,
    pub(crate) lists: HashMap<String, ListValue>,
    pub(crate) sets: HashMap<String, SetValue>,
    pub(crate) sorted_sets: HashMap<String, Vec<(Vec<u8>, f64)>>,
    pub(crate) hyperloglogs: HashMap<String, HyperLogLogValue>,
    pub(crate) bitmaps: HashMap<String, BitmapValue>,
}

impl SnapshotImage {
    /// Copy every store present in `stores` (absent ones are empty).
    pub async fn capture(stores: StoreRefs<'_>) -> Result<Self> {
        Ok(Self {
            kv: stores.kv_store.dump_with_expiry().await?,
            queues: match stores.queue_manager {
                Some(qm) => qm.dump().await?,
                None => HashMap::new(),
            },
            hashes: stores.hash_store.map(|s| s.dump()).unwrap_or_default(),
            lists: stores.list_store.map(|s| s.dump()).unwrap_or_default(),
            sets: stores.set_store.map(|s| s.dump()).unwrap_or_default(),
            sorted_sets: stores
                .sorted_set_store
                .map(|s| s.dump())
                .unwrap_or_default(),
            hyperloglogs: stores
                .hyperloglog_store
                .map(|s| s.dump())
                .unwrap_or_default(),
            bitmaps: stores.bitmap_store.map(|s| s.dump()).unwrap_or_default(),
        })
    }
}

/// Snapshot manager for periodic state dumps with streaming support
pub struct SnapshotManager {
    config: SnapshotConfig,
}

impl SnapshotManager {
    /// Create a new snapshot manager
    pub fn new(config: SnapshotConfig) -> Self {
        Self { config }
    }

    /// Capture the stores and write them as a snapshot whose WAL replay starts
    /// at `wal_offset`.
    ///
    /// The capture is not synchronized with writers; the persistence layer's
    /// [`PersistenceLayer::snapshot`](super::PersistenceLayer::snapshot) pairs
    /// the image with its WAL cutoff under the write barrier instead.
    pub async fn create_snapshot(&self, stores: StoreRefs<'_>, wal_offset: u64) -> Result<PathBuf> {
        let image = SnapshotImage::capture(stores).await?;
        self.write_image(image, stores.stream_manager, wal_offset)
            .await
    }

    /// Serialize `image` (plus the current stream events) as a v4 snapshot
    /// whose WAL replay starts at `wal_offset`, fsync it and prune old files.
    pub async fn write_image(
        &self,
        image: SnapshotImage,
        stream_manager: Option<&StreamManager>,
        wal_offset: u64,
    ) -> Result<PathBuf> {
        let SnapshotImage {
            kv,
            queues,
            hashes,
            lists,
            sets,
            sorted_sets,
            hyperloglogs,
            bitmaps,
        } = image;
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

        // KV section: key, value and absolute expiry (Unix ms, 0 = none).
        let kv_count = kv.len() as u64;
        writer.write_u64(kv_count).await?;
        checksum.update(&kv_count.to_le_bytes());

        debug!("Streaming {} KV entries", kv_count);

        for (key, value, expires_at_ms) in kv {
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

            let expiry = expires_at_ms.unwrap_or(0);
            writer.write_u64(expiry).await?;
            checksum.update(&expiry.to_le_bytes());
        }

        let queue_count = queues.len() as u64;
        writer.write_u64(queue_count).await?;
        checksum.update(&queue_count.to_le_bytes());

        debug!("Streaming {} queue entries", queue_count);

        for (queue_name, messages) in queues {
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

        // Streams are not in the WAL, so they are read here, outside the
        // snapshot barrier.
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

        // Hash / list / set / sorted-set sections (v3+).
        debug!("Streaming {} hashes", hashes.len());
        write_map_section(&mut writer, &mut checksum, &hashes).await?;
        debug!("Streaming {} lists", lists.len());
        write_map_section(&mut writer, &mut checksum, &lists).await?;
        debug!("Streaming {} sets", sets.len());
        write_map_section(&mut writer, &mut checksum, &sets).await?;
        debug!("Streaming {} sorted sets", sorted_sets.len());
        write_map_section(&mut writer, &mut checksum, &sorted_sets).await?;

        // HyperLogLog / bitmap sections (v4+). Each value carries its own
        // absolute expiry (`created_at + ttl_secs`).
        debug!("Streaming {} HyperLogLogs", hyperloglogs.len());
        write_map_section(&mut writer, &mut checksum, &hyperloglogs).await?;
        debug!("Streaming {} bitmaps", bitmaps.len());
        write_map_section(&mut writer, &mut checksum, &bitmaps).await?;

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

        // The magic names the format: v4 (current), v3 (no KV expiry, no
        // HyperLogLog/bitmap sections) or v2 (no collection sections either).
        let expected_version: u8 = if &magic == SNAPSHOT_MAGIC {
            SNAPSHOT_VERSION
        } else if &magic == SNAPSHOT_MAGIC_V3 {
            3
        } else if &magic == SNAPSHOT_MAGIC_V2 {
            2
        } else {
            return Err(PersistenceError::SnapshotCorrupted(latest.clone()));
        };

        let version = reader.read_u8().await?;
        checksum.update(&[version]);
        if version != expected_version {
            warn!(
                "Snapshot version byte {} does not match its magic (version {})",
                version, expected_version
            );
            return Err(PersistenceError::SnapshotCorrupted(latest.clone()));
        }
        let has_collections = version >= 3;
        let has_expiry_and_hll_bitmap = version >= 4;

        // Read metadata
        let timestamp = reader.read_u64().await?;
        checksum.update(&timestamp.to_le_bytes());
        let wal_offset = reader.read_u64().await?;
        checksum.update(&wal_offset.to_le_bytes());

        // Read KV data
        let kv_count = reader.read_u64().await?;
        checksum.update(&kv_count.to_le_bytes());
        let mut kv_data = HashMap::new();
        let mut kv_expires_at_ms = HashMap::new();

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

            if has_expiry_and_hll_bitmap {
                let expires_at_ms = reader.read_u64().await?;
                checksum.update(&expires_at_ms.to_le_bytes());
                if expires_at_ms != 0 {
                    kv_expires_at_ms.insert(key.clone(), expires_at_ms);
                }
            }

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

        // Read HyperLogLog / bitmap sections (v4+).
        let (hyperloglog_data, bitmap_data) = if has_expiry_and_hll_bitmap {
            let hyperloglog_data = read_map_section(&mut reader, &mut checksum).await?;
            let bitmap_data = read_map_section(&mut reader, &mut checksum).await?;
            (hyperloglog_data, bitmap_data)
        } else {
            (HashMap::new(), HashMap::new())
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
            kv_expires_at_ms,
            hyperloglog_data,
            bitmap_data,
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
            if path.extension().and_then(|s| s.to_str()) == Some("bin")
                && let Some(filename) = path.file_name()
            {
                let name = filename.to_string_lossy();
                if name.starts_with("snapshot-") {
                    snapshots.push(path);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{BitmapStore, Expiry, HyperLogLogStore, KVConfig, KVStore, SetOptions};

    /// Hand-written snapshot bytes: numbers are streamed big-endian and fed
    /// to the CRC64 little-endian, as the format specifies.
    struct Fixture {
        bytes: Vec<u8>,
        crc: CRC64,
    }

    impl Default for Fixture {
        fn default() -> Self {
            Self {
                bytes: Vec::new(),
                crc: CRC64::new(),
            }
        }
    }

    impl Fixture {
        fn raw(&mut self, data: &[u8]) {
            self.bytes.extend_from_slice(data);
            self.crc.update(data);
        }

        fn u64(&mut self, v: u64) {
            self.bytes.extend_from_slice(&v.to_be_bytes());
            self.crc.update(&v.to_le_bytes());
        }

        fn u32(&mut self, v: u32) {
            self.bytes.extend_from_slice(&v.to_be_bytes());
            self.crc.update(&v.to_le_bytes());
        }
    }

    fn manager(name: &str) -> SnapshotManager {
        let dir = format!("./target/snapshot_format_{name}");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        SnapshotManager::new(SnapshotConfig {
            directory: dir.into(),
            ..SnapshotConfig::default()
        })
    }

    /// A v3 file, written byte by byte here rather than by the v4 writer,
    /// still loads: its keys come back without expiry and the v4-only
    /// sections are empty.
    #[tokio::test]
    async fn v3_snapshot_still_loads() {
        let mgr = manager("v3");
        let mut f = Fixture::default();
        f.raw(b"SYNAP003");
        f.raw(&[3]);
        f.u64(1_700_000_000); // timestamp
        f.u64(9); // wal_offset
        f.u64(1); // kv count
        f.u32(1);
        f.raw(b"k");
        f.u32(1);
        f.raw(b"v");
        // queue, stream, hash, list, set and sorted-set sections: all empty.
        for _ in 0..6 {
            f.u64(0);
        }
        let mut bytes = f.bytes;
        bytes.extend_from_slice(&f.crc.finalize().to_be_bytes());
        std::fs::write(
            mgr.config.directory.join("snapshot-v3-1700000000.bin"),
            bytes,
        )
        .unwrap();

        let (snapshot, _) = mgr.load_latest().await.unwrap().unwrap();
        assert_eq!(snapshot.version, 3);
        assert_eq!(snapshot.wal_offset, 9);
        assert_eq!(snapshot.kv_data["k"], b"v".to_vec());
        assert!(snapshot.kv_expires_at_ms.is_empty());
        assert!(snapshot.hyperloglog_data.is_empty());
        assert!(snapshot.bitmap_data.is_empty());
    }

    /// v4 keeps every KV key's absolute deadline (persistent keys have none)
    /// and the HyperLogLog and bitmap values.
    #[tokio::test]
    async fn v4_snapshot_keeps_expiry_hyperloglog_and_bitmaps() {
        let mgr = manager("v4");
        let kv = KVStore::new(KVConfig::default());
        let deadline = 4_102_444_800_000; // 2100-01-01
        kv.set_with_opts(
            "t",
            b"v".to_vec(),
            Some(Expiry::UnixMilliseconds(deadline)),
            SetOptions::default(),
        )
        .await
        .unwrap();
        kv.set("p", b"v".to_vec(), None).await.unwrap();
        let hll = HyperLogLogStore::new();
        hll.pfadd("h", vec![b"a".to_vec()], Some(60)).unwrap();
        let bitmap = BitmapStore::new();
        bitmap.setbit("b", 3, 1).unwrap();
        let stores = StoreRefs {
            hyperloglog_store: Some(&hll),
            bitmap_store: Some(&bitmap),
            ..StoreRefs::kv_only(&kv)
        };
        mgr.create_snapshot(stores, 5).await.unwrap();

        let (snapshot, _) = mgr.load_latest().await.unwrap().unwrap();
        assert_eq!(snapshot.version, 4);
        assert_eq!(snapshot.kv_expires_at_ms.len(), 1);
        assert_eq!(snapshot.kv_expires_at_ms["t"], deadline);
        assert_eq!(snapshot.kv_data["p"], b"v".to_vec());
        assert_eq!(
            snapshot.hyperloglog_data["h"].expires_at_ms(),
            hll.dump()["h"].expires_at_ms()
        );
        assert_eq!(snapshot.bitmap_data["b"].data, bitmap.dump()["b"].data);
    }
}
