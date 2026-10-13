use super::apply::StoreRefs;
use super::types::{
    PersistenceError, QueueImage, QueueMessageRecord, Result, Snapshot, SnapshotConfig, StreamEvent,
};
use super::wal_frame;
use crate::core::queue::{QueueConfig, QueueMessage, QueueState};
use crate::core::{BitmapValue, HyperLogLogValue, ListValue, SetValue, StreamManager};
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::sync::Mutex;
use tracing::{debug, info, warn};

/// v2 = kv + queue + stream. v3 adds hash/list/set/sorted-set. v4 adds each
/// KV key's absolute expiry and the HyperLogLog and bitmap sections. v5
/// (current) stores every queue whole: its configuration and its ready,
/// in-flight and dead-lettered messages with their full metadata (creation
/// time included); v2-v4 stored the ready messages only, without it.
const SNAPSHOT_VERSION: u8 = 5;
const SNAPSHOT_MAGIC: &[u8; 8] = b"SYNAP005";
const SNAPSHOT_MAGIC_V4: &[u8; 8] = b"SYNAP004";
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
    pub(crate) queues: HashMap<String, QueueState>,
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
            queues: stores
                .queue_manager
                .map(|qm| qm.dump_state())
                .unwrap_or_default(),
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

/// Digits of the sequence number in a snapshot file name.
const SEQUENCE_DIGITS: usize = 20;

/// Appended to a snapshot's final name for the file it is written to before
/// the rename that publishes it.
const TEMP_SUFFIX: &str = ".tmp";

/// Final file name of the snapshot with sequence number `sequence`, named
/// after the format version it is written in. Sequenced names of every
/// version share one sequence, so v4 and v5 files order by sequence alone.
fn snapshot_file_name(sequence: u64) -> String {
    format!(
        "snapshot-v{}-{:0width$}.bin",
        SNAPSHOT_VERSION,
        sequence,
        width = SEQUENCE_DIGITS
    )
}

/// The temporary file a snapshot is written to before it is renamed to `path`.
fn temp_path(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(TEMP_SUFFIX);
    path.with_file_name(name)
}

/// Position of a snapshot file in the load order.
///
/// Files named `snapshot-v<version>-<20-digit sequence>.bin` (1.4.0 and
/// later) are ordered by their sequence. Any other `snapshot-*.bin` file was
/// named by an earlier release (`snapshot-v<version>-<unix seconds>.bin`) or by
/// hand: it is older than every sequenced file, and such files are ordered
/// among themselves by name, as earlier releases did.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum SnapshotOrder {
    Named(String),
    Sequenced(u64),
}

impl SnapshotOrder {
    /// Order of the file called `name`, or `None` when it is not a snapshot
    /// (temporary files included).
    fn of(name: &str) -> Option<Self> {
        let stem = name.strip_prefix("snapshot-")?.strip_suffix(".bin")?;
        Some(match parse_sequence(stem) {
            Some(sequence) => SnapshotOrder::Sequenced(sequence),
            None => SnapshotOrder::Named(name.to_string()),
        })
    }
}

/// The sequence number in a `v<version>-<20 digits>` file name stem.
fn parse_sequence(stem: &str) -> Option<u64> {
    let (version, sequence) = stem.strip_prefix('v')?.split_once('-')?;
    let all_digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    if !all_digits(version) || sequence.len() != SEQUENCE_DIGITS || !all_digits(sequence) {
        return None;
    }
    sequence.parse().ok()
}

/// Snapshot manager for periodic state dumps with streaming support
pub struct SnapshotManager {
    config: SnapshotConfig,
    /// Serializes snapshot writes, so each takes the next sequence number and
    /// no two of them write the same temporary file.
    write_lock: Mutex<()>,
}

impl SnapshotManager {
    /// Create a new snapshot manager
    pub fn new(config: SnapshotConfig) -> Self {
        Self {
            config,
            write_lock: Mutex::new(()),
        }
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

    /// Serialize `image` (plus the current stream events) as a v5 snapshot
    /// whose WAL replay starts at `wal_offset`, publish it atomically and
    /// prune old files.
    ///
    /// The bytes go to `<final name>.tmp` in the snapshot directory. That file
    /// is fsynced, renamed to the final name, and the directory is fsynced
    /// (Unix; on Windows the NTFS rename is atomic and journaled in order). A
    /// crash at any byte leaves either no new snapshot — only the temporary
    /// file, which recovery ignores and the next write removes — or the
    /// complete one. Old snapshots are removed only after that, and never the
    /// one just written.
    pub async fn write_image(
        &self,
        image: SnapshotImage,
        stream_manager: Option<&StreamManager>,
        wal_offset: u64,
    ) -> Result<PathBuf> {
        let _writing = self.write_lock.lock().await;
        self.prepare_directory().await?;
        let sequence = self.next_sequence().await?;
        let path = self.config.directory.join(snapshot_file_name(sequence));
        let temp = temp_path(&path);

        info!("Creating streaming snapshot at {:?}", path);
        let checksum = match write_snapshot_file(&temp, image, stream_manager, wal_offset).await {
            Ok(checksum) => checksum,
            Err(e) => {
                remove_temp(&temp).await;
                return Err(e);
            }
        };
        tokio::fs::rename(&temp, &path).await?;
        sync_parent_dir(path.clone()).await?;
        info!(
            "Streaming snapshot created successfully: {:?} (checksum: {})",
            path, checksum
        );

        if let Err(e) = self.cleanup_old_snapshots(&path).await {
            warn!(
                "Snapshot {:?} is durable, but removing old snapshots failed: {}",
                path, e
            );
        }
        Ok(path)
    }

    /// Create the snapshot directory (making its entry durable when it is
    /// new) and remove temporary files left by a write that never finished.
    async fn prepare_directory(&self) -> Result<()> {
        let dir = &self.config.directory;
        if !tokio::fs::try_exists(dir).await? {
            tokio::fs::create_dir_all(dir).await?;
            sync_parent_dir(dir.clone()).await?;
        }
        let mut entries = tokio::fs::read_dir(dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.starts_with("snapshot-") && name.ends_with(TEMP_SUFFIX) {
                warn!(
                    "Removing {:?}, left by a snapshot write that did not finish",
                    entry.path()
                );
                tokio::fs::remove_file(entry.path()).await?;
            }
        }
        Ok(())
    }

    /// One past the highest sequence number in the directory (1 for the
    /// first sequenced snapshot).
    async fn next_sequence(&self) -> Result<u64> {
        let highest = self
            .list_ordered()
            .await?
            .into_iter()
            .filter_map(|(order, _)| match order {
                SnapshotOrder::Sequenced(sequence) => Some(sequence),
                SnapshotOrder::Named(_) => None,
            })
            .max()
            .unwrap_or(0);
        highest.checked_add(1).ok_or_else(|| {
            PersistenceError::IOError(std::io::Error::other(format!(
                "snapshot sequence numbers in {:?} are exhausted",
                self.config.directory
            )))
        })
    }

    /// Load the newest snapshot file. Fails when that file is damaged;
    /// recovery ([`super::recover_all`]) falls back to older files instead
    /// when that is safe.
    pub async fn load_latest(&self) -> Result<Option<(Snapshot, PathBuf)>> {
        let Some(latest) = self.list_snapshots().await?.pop() else {
            info!("No snapshots found");
            return Ok(None);
        };
        let snapshot = self.load(&latest).await?;
        Ok(Some((snapshot, latest)))
    }

    /// Load and verify the snapshot at `path`.
    ///
    /// A truncated file, an unknown magic, a version byte that does not match
    /// it, a checksum mismatch or a record that does not decode returns
    /// [`PersistenceError::SnapshotCorrupted`]. Other I/O errors (permissions,
    /// device errors) are returned as they are: the file may be intact.
    pub async fn load(&self, path: &Path) -> Result<Snapshot> {
        info!("Loading snapshot from {:?}", path);
        let file = File::open(path).await?;
        let len = file.metadata().await?.len();
        let mut reader = SnapshotReader::new(file, len);
        match read_snapshot(&mut reader, path).await {
            Err(PersistenceError::IOError(e))
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::UnexpectedEof | std::io::ErrorKind::InvalidData
                ) =>
            {
                warn!("Snapshot {:?} is damaged: {}", path, e);
                Err(PersistenceError::SnapshotCorrupted(path.to_path_buf()))
            }
            other => other,
        }
    }

    /// Every snapshot file with its load order, oldest first.
    async fn list_ordered(&self) -> Result<Vec<(SnapshotOrder, PathBuf)>> {
        let mut snapshots = Vec::new();
        let mut dir = match tokio::fs::read_dir(&self.config.directory).await {
            Ok(dir) => dir,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(snapshots),
            Err(e) => return Err(e.into()),
        };
        while let Some(entry) = dir.next_entry().await? {
            if let Some(order) = SnapshotOrder::of(&entry.file_name().to_string_lossy()) {
                snapshots.push((order, entry.path()));
            }
        }
        snapshots.sort();
        Ok(snapshots)
    }

    /// Every snapshot file, oldest first: files named by earlier releases (by
    /// name), then sequenced files by sequence number. Temporary files are not
    /// listed.
    pub async fn list_snapshots(&self) -> Result<Vec<PathBuf>> {
        Ok(self
            .list_ordered()
            .await?
            .into_iter()
            .map(|(_, path)| path)
            .collect())
    }

    /// Remove the oldest snapshots beyond `max_snapshots` (at least one is
    /// kept), never `keep`, the snapshot just written.
    async fn cleanup_old_snapshots(&self, keep: &Path) -> Result<()> {
        let snapshots = self.list_snapshots().await?;
        let excess = snapshots
            .len()
            .saturating_sub(self.config.max_snapshots.max(1));
        for snapshot in snapshots.iter().take(excess) {
            if snapshot.as_path() == keep {
                continue;
            }
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

/// [`wal_frame::sync_parent_dir`] on the blocking pool: make a create or
/// rename in the directory holding `path` durable.
async fn sync_parent_dir(path: PathBuf) -> Result<()> {
    tokio::task::spawn_blocking(move || wal_frame::sync_parent_dir(&path))
        .await
        .map_err(std::io::Error::other)??;
    Ok(())
}

/// Remove the temporary file of a write that failed. A file that cannot be
/// removed now is removed by the next snapshot write.
async fn remove_temp(temp: &Path) {
    if let Err(e) = tokio::fs::remove_file(temp).await
        && e.kind() != std::io::ErrorKind::NotFound
    {
        warn!(
            "Could not remove {:?} after a failed snapshot write: {}; the next snapshot removes it",
            temp, e
        );
    }
}

/// Stream `image` (plus the current stream events) to `path` as a v5
/// snapshot and fsync it. Returns the checksum written at the end.
async fn write_snapshot_file(
    path: &Path,
    image: SnapshotImage,
    stream_manager: Option<&StreamManager>,
    wal_offset: u64,
) -> Result<u64> {
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
    let timestamp = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let file = File::create(path).await?;
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

    // Queue section (v5): per queue its name, its configuration and its
    // ready, in-flight and dead-lettered messages, each message one
    // length-prefixed `QueueMessageRecord`.
    let queue_count = queues.len() as u64;
    writer.write_u64(queue_count).await?;
    checksum.update(&queue_count.to_le_bytes());

    debug!("Streaming {} queue entries", queue_count);

    for (queue_name, state) in queues {
        write_blob(&mut writer, &mut checksum, queue_name.as_bytes()).await?;
        write_bincode(&mut writer, &mut checksum, &state.config).await?;
        for messages in [&state.ready, &state.in_flight, &state.dead_letter] {
            write_message_list(&mut writer, &mut checksum, messages).await?;
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
    Ok(final_checksum)
}

/// Decode a snapshot from `r`, verifying its trailing checksum.
async fn read_snapshot(r: &mut SnapshotReader, path: &Path) -> Result<Snapshot> {
    // Header: magic (8 bytes) + version (1 byte). The magic names the format:
    // v5 (current), v4 (queues hold their ready messages only), v3 (no KV
    // expiry, no HyperLogLog/bitmap sections) or v2 (no collection sections
    // either).
    let magic = r.bytes(8).await?;
    let expected_version: u8 = if magic[..] == SNAPSHOT_MAGIC[..] {
        SNAPSHOT_VERSION
    } else if magic[..] == SNAPSHOT_MAGIC_V4[..] {
        4
    } else if magic[..] == SNAPSHOT_MAGIC_V3[..] {
        3
    } else if magic[..] == SNAPSHOT_MAGIC_V2[..] {
        2
    } else {
        warn!("Snapshot {:?} does not start with a known magic", path);
        return Err(PersistenceError::SnapshotCorrupted(path.to_path_buf()));
    };

    let version = r.u8().await?;
    if version != expected_version {
        warn!(
            "Snapshot version byte {} does not match its magic (version {})",
            version, expected_version
        );
        return Err(PersistenceError::SnapshotCorrupted(path.to_path_buf()));
    }
    let has_collections = version >= 3;
    let has_expiry_and_hll_bitmap = version >= 4;
    let has_full_queues = version >= 5;

    let timestamp = r.u64().await?;
    let wal_offset = r.u64().await?;

    // KV section
    let kv_count = r.u64().await?;
    let mut kv_data = HashMap::new();
    let mut kv_expires_at_ms = HashMap::new();
    for _ in 0..kv_count {
        let key = r.string().await?;
        let value = r.blob().await?;
        if has_expiry_and_hll_bitmap {
            let expires_at_ms = r.u64().await?;
            if expires_at_ms != 0 {
                kv_expires_at_ms.insert(key.clone(), expires_at_ms);
            }
        }
        kv_data.insert(key, value);
    }

    // Queue section: v5 holds whole queues; v2-v4 the ready messages only,
    // in `QueueMessage`'s own encoding, which has no creation time (it
    // decodes as "now").
    let queue_count = r.u64().await?;
    let mut queue_data = HashMap::new();
    for _ in 0..queue_count {
        let queue_name = r.string().await?;
        let image = if has_full_queues {
            QueueImage {
                config: Some(r.decoded::<QueueConfig>().await?),
                ready: r.message_list().await?,
                in_flight: r.message_list().await?,
                dead_letter: r.message_list().await?,
            }
        } else {
            let msg_count = r.u64().await?;
            let mut ready = Vec::new();
            for _ in 0..msg_count {
                let message = r.decoded::<QueueMessage>().await?;
                ready.push(QueueMessageRecord::from(&message));
            }
            QueueImage {
                ready,
                ..QueueImage::default()
            }
        };
        queue_data.insert(queue_name, image);
    }

    // Stream section (always present since v2, count may be 0)
    let stream_count = r.u64().await?;
    let mut stream_data = HashMap::new();
    for _ in 0..stream_count {
        let room_name = r.string().await?;
        let event_count = r.u64().await?;
        let mut events = Vec::new();
        for _ in 0..event_count {
            events.push(r.decoded::<StreamEvent>().await?);
        }
        stream_data.insert(room_name, events);
    }

    // Hash / list / set / sorted-set sections (v3+; absent in v2).
    let (hash_data, list_data, set_data, sorted_set_data) = if has_collections {
        (
            r.map_section().await?,
            r.map_section().await?,
            r.map_section().await?,
            r.map_section().await?,
        )
    } else {
        (
            HashMap::new(),
            HashMap::new(),
            HashMap::new(),
            HashMap::new(),
        )
    };

    // HyperLogLog / bitmap sections (v4+).
    let (hyperloglog_data, bitmap_data) = if has_expiry_and_hll_bitmap {
        (r.map_section().await?, r.map_section().await?)
    } else {
        (HashMap::new(), HashMap::new())
    };

    // Verify integrity: the trailing CRC64 must match the running digest.
    let (stored_checksum, computed_checksum) = r.trailer().await?;
    if stored_checksum != computed_checksum {
        warn!(
            "Snapshot checksum mismatch at {:?} (stored={:#x}, computed={:#x})",
            path, stored_checksum, computed_checksum
        );
        return Err(PersistenceError::SnapshotCorrupted(path.to_path_buf()));
    }

    info!(
        "Snapshot loaded successfully: version={}, timestamp={}, wal_offset={}, streams={}",
        version,
        timestamp,
        wal_offset,
        stream_data.len()
    );

    Ok(Snapshot {
        version: u32::from(version),
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
    })
}

/// Write `bytes` prefixed with their length (u32), feeding the checksum the
/// same bytes [`SnapshotReader::blob`] accumulates.
async fn write_blob<W>(writer: &mut W, checksum: &mut CRC64, bytes: &[u8]) -> std::io::Result<()>
where
    W: AsyncWriteExt + Unpin,
{
    let len = u32::try_from(bytes.len()).map_err(invalid_data)?;
    writer.write_u32(len).await?;
    checksum.update(&len.to_le_bytes());
    writer.write_all(bytes).await?;
    checksum.update(bytes);
    Ok(())
}

/// Write `value` bincode-encoded (legacy config) as one blob, read back by
/// [`SnapshotReader::decoded`].
async fn write_bincode<W, V>(writer: &mut W, checksum: &mut CRC64, value: &V) -> std::io::Result<()>
where
    W: AsyncWriteExt + Unpin,
    V: Serialize,
{
    let data = bincode::serde::encode_to_vec(value, bincode::config::legacy())
        .map_err(std::io::Error::other)?;
    write_blob(writer, checksum, &data).await
}

/// Write a message list of the v5 queue section: count (u64), then each
/// message as one [`QueueMessageRecord`] blob.
async fn write_message_list<W>(
    writer: &mut W,
    checksum: &mut CRC64,
    messages: &[QueueMessage],
) -> std::io::Result<()>
where
    W: AsyncWriteExt + Unpin,
{
    let count = messages.len() as u64;
    writer.write_u64(count).await?;
    checksum.update(&count.to_le_bytes());
    for message in messages {
        write_bincode(writer, checksum, &QueueMessageRecord::from(message)).await?;
    }
    Ok(())
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

/// Reads a snapshot file the way the writer laid it out: numbers are
/// big-endian on disk and fed to the CRC64 little-endian, raw bytes are fed as
/// they are. Every length is checked against the bytes left in the file before
/// anything is allocated, so a damaged length fails like a truncated file
/// instead of allocating gigabytes.
struct SnapshotReader {
    inner: BufReader<File>,
    remaining: u64,
    checksum: CRC64,
}

impl SnapshotReader {
    fn new(file: File, len: u64) -> Self {
        Self {
            inner: BufReader::new(file),
            remaining: len,
            checksum: CRC64::new(),
        }
    }

    /// Account for `n` more bytes, failing when the file ends first.
    fn take(&mut self, n: u64) -> std::io::Result<()> {
        match self.remaining.checked_sub(n) {
            Some(rest) => {
                self.remaining = rest;
                Ok(())
            }
            None => Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                format!(
                    "{} bytes needed, only {} left in the file",
                    n, self.remaining
                ),
            )),
        }
    }

    async fn u8(&mut self) -> std::io::Result<u8> {
        self.take(1)?;
        let value = self.inner.read_u8().await?;
        self.checksum.update(&[value]);
        Ok(value)
    }

    async fn u32(&mut self) -> std::io::Result<u32> {
        self.take(4)?;
        let value = self.inner.read_u32().await?;
        self.checksum.update(&value.to_le_bytes());
        Ok(value)
    }

    async fn u64(&mut self) -> std::io::Result<u64> {
        self.take(8)?;
        let value = self.inner.read_u64().await?;
        self.checksum.update(&value.to_le_bytes());
        Ok(value)
    }

    /// `len` raw bytes.
    async fn bytes(&mut self, len: u64) -> std::io::Result<Vec<u8>> {
        self.take(len)?;
        let mut buf = vec![0u8; usize::try_from(len).map_err(invalid_data)?];
        self.inner.read_exact(&mut buf).await?;
        self.checksum.update(&buf);
        Ok(buf)
    }

    /// A u32 length, then that many bytes.
    async fn blob(&mut self) -> std::io::Result<Vec<u8>> {
        let len = self.u32().await?;
        self.bytes(u64::from(len)).await
    }

    /// A u32 length, then that many bytes of UTF-8.
    async fn string(&mut self) -> std::io::Result<String> {
        String::from_utf8(self.blob().await?).map_err(invalid_data)
    }

    /// A u32 length, then that many bytes of bincode (legacy config).
    async fn decoded<V: DeserializeOwned>(&mut self) -> std::io::Result<V> {
        let data = self.blob().await?;
        bincode::serde::decode_from_slice(&data, bincode::config::legacy())
            .map(|(value, _)| value)
            .map_err(invalid_data)
    }

    /// A message list written by [`write_message_list`]. Each record is a
    /// bounded blob, so a damaged count fails at the first missing record.
    async fn message_list(&mut self) -> std::io::Result<Vec<QueueMessageRecord>> {
        let count = self.u64().await?;
        let mut messages = Vec::new();
        for _ in 0..count {
            messages.push(self.decoded().await?);
        }
        Ok(messages)
    }

    /// A section written by [`write_map_section`].
    async fn map_section<V: DeserializeOwned>(&mut self) -> std::io::Result<HashMap<String, V>> {
        let count = self.u64().await?;
        let mut out = HashMap::new();
        for _ in 0..count {
            let key = self.string().await?;
            let value = self.decoded().await?;
            out.insert(key, value);
        }
        Ok(out)
    }

    /// Read the trailing checksum, which is not part of the digest. Returns
    /// the stored checksum and the digest of every byte before it.
    async fn trailer(&mut self) -> std::io::Result<(u64, u64)> {
        let computed = self.checksum.finalize();
        self.take(8)?;
        let stored = self.inner.read_u64().await?;
        Ok((stored, computed))
    }
}

/// An `InvalidData` I/O error: the bytes are there but do not decode.
fn invalid_data<E>(error: E) -> std::io::Error
where
    E: Into<Box<dyn std::error::Error + Send + Sync>>,
{
    std::io::Error::new(std::io::ErrorKind::InvalidData, error)
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

    fn finalize(&self) -> u64 {
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
        assert_eq!(snapshot.version, 5);
        assert_eq!(snapshot.kv_expires_at_ms.len(), 1);
        assert_eq!(snapshot.kv_expires_at_ms["t"], deadline);
        assert_eq!(snapshot.kv_data["p"], b"v".to_vec());
        assert_eq!(
            snapshot.hyperloglog_data["h"].expires_at_ms(),
            hll.dump()["h"].expires_at_ms()
        );
        assert_eq!(snapshot.bitmap_data["b"].data, bitmap.dump()["b"].data);
    }

    /// A v4 file, written byte by byte here, still loads its queue section:
    /// the ready messages keep their ID and metadata, with no configuration
    /// and no in-flight or dead-lettered messages (v4 did not store them).
    #[tokio::test]
    async fn v4_queue_section_still_loads() {
        let mgr = manager("v4_queue");
        let mut message = QueueMessage::new(b"body".to_vec(), 7, 2);
        message.retry_count = 1;
        let encoded = bincode::serde::encode_to_vec(&message, bincode::config::legacy()).unwrap();
        let mut f = Fixture::default();
        f.raw(b"SYNAP004");
        f.raw(&[4]);
        f.u64(1_700_000_000); // timestamp
        f.u64(3); // wal_offset
        f.u64(0); // kv count
        f.u64(1); // queue count
        f.u32(1);
        f.raw(b"q");
        f.u64(1); // message count
        f.u32(encoded.len() as u32);
        f.raw(&encoded);
        // stream, hash, list, set, sorted-set, HyperLogLog and bitmap: empty.
        for _ in 0..7 {
            f.u64(0);
        }
        let mut bytes = f.bytes;
        bytes.extend_from_slice(&f.crc.finalize().to_be_bytes());
        std::fs::write(
            mgr.config
                .directory
                .join("snapshot-v4-00000000000000000001.bin"),
            bytes,
        )
        .unwrap();

        let (snapshot, _) = mgr.load_latest().await.unwrap().unwrap();
        assert_eq!(snapshot.version, 4);
        let queue = &snapshot.queue_data["q"];
        assert!(queue.config.is_none());
        assert!(queue.in_flight.is_empty() && queue.dead_letter.is_empty());
        assert_eq!(queue.ready.len(), 1);
        assert_eq!(queue.ready[0].id, message.id);
        assert_eq!(queue.ready[0].payload, b"body".to_vec());
        assert_eq!(queue.ready[0].priority, 7);
        assert_eq!(queue.ready[0].retry_count, 1);
        assert_eq!(queue.ready[0].max_retries, 2);

        // The next write continues the sequence under the v5 name and is the
        // newest file.
        let kv = KVStore::new(KVConfig::default());
        let next = mgr
            .create_snapshot(StoreRefs::kv_only(&kv), 4)
            .await
            .unwrap();
        assert!(next.ends_with(snapshot_file_name(2)));
        assert_eq!(mgr.load_latest().await.unwrap().unwrap().1, next);
    }

    /// v5 stores each queue whole: configuration, ready, in-flight (in
    /// delivery order) and dead-lettered messages, creation time included.
    #[tokio::test]
    async fn v5_snapshot_keeps_whole_queues() {
        use crate::core::QueueManager;
        let mgr = manager("v5_queues");
        let kv = KVStore::new(KVConfig::default());
        let qm = QueueManager::new(QueueConfig::default());
        let config = QueueConfig {
            max_depth: 21,
            default_max_retries: 0,
            ..QueueConfig::default()
        };
        qm.create_queue("q", Some(config)).await.unwrap();
        for body in ["in-flight", "dead", "ready"] {
            qm.publish("q", body.as_bytes().to_vec(), None, None)
                .await
                .unwrap();
        }
        let in_flight = qm.consume("q", "c").await.unwrap().unwrap();
        let dead = qm.consume("q", "c").await.unwrap().unwrap();
        qm.nack("q", &dead.id, false).await.unwrap();
        let before = qm.dump_state().remove("q").unwrap();
        let stores = StoreRefs {
            queue_manager: Some(&qm),
            ..StoreRefs::kv_only(&kv)
        };
        let path = mgr.create_snapshot(stores, 0).await.unwrap();

        let (snapshot, _) = mgr.load_latest().await.unwrap().unwrap();
        assert_eq!(snapshot.version, 5);
        let queue = &snapshot.queue_data["q"];
        assert_eq!(queue.config.as_ref().unwrap().max_depth, 21);
        let records = |list: &[QueueMessage]| -> Vec<QueueMessageRecord> {
            list.iter().map(QueueMessageRecord::from).collect()
        };
        assert_eq!(queue.ready, records(&before.ready));
        assert_eq!(queue.in_flight, records(&before.in_flight));
        assert_eq!(queue.dead_letter, records(&before.dead_letter));
        assert_eq!(queue.in_flight[0].id, in_flight.id);
        assert_eq!(queue.in_flight[0].created_at, in_flight.created_at);
        assert_eq!(queue.dead_letter[0].id, dead.id);

        // Every cut of the file inside the queue section loads as corrupted.
        let bytes = std::fs::read(&path).unwrap();
        for len in 0..bytes.len() {
            std::fs::write(&path, &bytes[..len]).unwrap();
            assert!(
                matches!(
                    mgr.load(&path).await,
                    Err(PersistenceError::SnapshotCorrupted(_))
                ),
                "cut at {len}"
            );
        }
    }

    /// A manager over `./target/snapshot_format_<name>` keeping `max` files.
    fn manager_keeping(name: &str, max: usize) -> SnapshotManager {
        let mgr = manager(name);
        SnapshotManager::new(SnapshotConfig {
            max_snapshots: max,
            ..mgr.config.clone()
        })
    }

    fn file_names(mgr: &SnapshotManager) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(&mgr.config.directory)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    /// Sequenced names sort by sequence after every name an earlier release
    /// wrote; temporary files and other files are not snapshots.
    #[test]
    fn snapshot_order_ranks_sequenced_names_after_legacy_names() {
        let order = |name: &str| SnapshotOrder::of(name);
        assert_eq!(
            snapshot_file_name(7),
            "snapshot-v5-00000000000000000007.bin"
        );
        // v4 and v5 sequenced names share one sequence.
        assert!(order("snapshot-v4-00000000000000000006.bin") < order(&snapshot_file_name(7)));
        assert!(order(&snapshot_file_name(7)) < order("snapshot-v4-00000000000000000008.bin"));
        assert_eq!(
            order(&snapshot_file_name(7)),
            Some(SnapshotOrder::Sequenced(7))
        );
        assert!(order(&snapshot_file_name(2)) < order(&snapshot_file_name(10)));
        assert!(order("snapshot-v4-1999999999.bin") < order(&snapshot_file_name(1)));
        assert!(order("snapshot-v3-1700000000.bin") < order("snapshot-v4-1600000000.bin"));
        assert_eq!(order("snapshot-v4-00000000000000000007.bin.tmp"), None);
        assert_eq!(order("notes.bin"), None);
        assert!(matches!(
            order("snapshot-v4-0000000000000000000x.bin"),
            Some(SnapshotOrder::Named(_))
        ));
    }

    /// Two snapshots written within the same second get different files,
    /// both load, and the second one is the newest.
    #[tokio::test]
    async fn two_snapshots_in_the_same_second_both_survive() {
        let mgr = manager("same_second");
        let kv = KVStore::new(KVConfig::default());
        kv.set("k", b"first".to_vec(), None).await.unwrap();
        let first = mgr
            .create_snapshot(StoreRefs::kv_only(&kv), 1)
            .await
            .unwrap();
        kv.set("k", b"second".to_vec(), None).await.unwrap();
        let second = mgr
            .create_snapshot(StoreRefs::kv_only(&kv), 2)
            .await
            .unwrap();

        assert_ne!(first, second);
        assert_eq!(
            mgr.list_snapshots().await.unwrap(),
            vec![first.clone(), second]
        );
        assert_eq!(
            mgr.load(&first).await.unwrap().kv_data["k"],
            b"first".to_vec()
        );
        let (latest, _) = mgr.load_latest().await.unwrap().unwrap();
        assert_eq!(latest.wal_offset, 2);
        assert_eq!(latest.kv_data["k"], b"second".to_vec());
    }

    /// A temporary file left by a crashed write is never listed or loaded,
    /// and the next write removes it.
    #[tokio::test]
    async fn leftover_temporary_file_is_ignored_then_removed() {
        let mgr = manager("leftover_temp");
        let kv = KVStore::new(KVConfig::default());
        kv.set("k", b"v".to_vec(), None).await.unwrap();
        let first = mgr
            .create_snapshot(StoreRefs::kv_only(&kv), 1)
            .await
            .unwrap();
        let bytes = std::fs::read(&first).unwrap();
        let crashed = temp_path(&mgr.config.directory.join(snapshot_file_name(2)));
        std::fs::write(&crashed, &bytes[..bytes.len() / 2]).unwrap();

        assert_eq!(mgr.list_snapshots().await.unwrap(), vec![first.clone()]);
        assert_eq!(mgr.load_latest().await.unwrap().unwrap().1, first);

        let next = mgr
            .create_snapshot(StoreRefs::kv_only(&kv), 3)
            .await
            .unwrap();
        assert!(!crashed.exists());
        assert_eq!(mgr.list_snapshots().await.unwrap(), vec![first, next]);
    }

    /// Retention keeps the newest files and never removes the snapshot it
    /// just wrote, even when configured to keep none.
    #[tokio::test]
    async fn retention_never_removes_the_snapshot_just_written() {
        let kv = KVStore::new(KVConfig::default());
        kv.set("k", b"v".to_vec(), None).await.unwrap();
        for max in [0, 1] {
            let mgr = manager_keeping(&format!("retention_{max}"), max);
            for offset in 1..=3 {
                mgr.create_snapshot(StoreRefs::kv_only(&kv), offset)
                    .await
                    .unwrap();
            }
            assert_eq!(file_names(&mgr), vec![snapshot_file_name(3)], "max {max}");
            let (snapshot, _) = mgr.load_latest().await.unwrap().unwrap();
            assert_eq!(snapshot.wal_offset, 3, "max {max}");
        }

        let mgr = manager_keeping("retention_2", 2);
        for offset in 1..=4 {
            mgr.create_snapshot(StoreRefs::kv_only(&kv), offset)
                .await
                .unwrap();
        }
        assert_eq!(
            file_names(&mgr),
            vec![snapshot_file_name(3), snapshot_file_name(4)]
        );
    }

    /// A file cut at any byte loads as corrupted (never as data, never as a
    /// plain I/O error), and so does a length field that points past the end
    /// of the file, without allocating it.
    #[tokio::test]
    async fn truncated_or_overlong_files_load_as_corrupted() {
        let mgr = manager("truncated");
        let kv = KVStore::new(KVConfig::default());
        kv.set("key", b"value".to_vec(), Some(600)).await.unwrap();
        let hashes = crate::core::HashStore::new();
        hashes.hset("h", "f", b"v".to_vec()).unwrap();
        let stores = StoreRefs {
            hash_store: Some(&hashes),
            ..StoreRefs::kv_only(&kv)
        };
        let path = mgr.create_snapshot(stores, 9).await.unwrap();
        let bytes = std::fs::read(&path).unwrap();

        for len in 0..bytes.len() {
            std::fs::write(&path, &bytes[..len]).unwrap();
            let result = mgr.load(&path).await;
            assert!(
                matches!(result, Err(PersistenceError::SnapshotCorrupted(_))),
                "cut at {len}: {result:?}"
            );
        }

        // magic(8) version(1) timestamp(8) wal_offset(8) kv_count(8), then the
        // first key's u32 length: claim 4 GiB.
        let mut overlong = bytes.clone();
        overlong[33..37].copy_from_slice(&u32::MAX.to_be_bytes());
        std::fs::write(&path, &overlong).unwrap();
        assert!(matches!(
            mgr.load(&path).await,
            Err(PersistenceError::SnapshotCorrupted(_))
        ));

        std::fs::write(&path, &bytes).unwrap();
        assert_eq!(
            mgr.load(&path).await.unwrap().kv_data["key"],
            b"value".to_vec()
        );
    }
}
