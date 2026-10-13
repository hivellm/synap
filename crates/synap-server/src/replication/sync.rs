//! Synchronization utilities for replication
//!
//! This module provides helpers for:
//! - The full-sync image: capture, versioned encoding and loading on a replica
//! - The 1.3.x KV + stream snapshot, still accepted from older masters
//! - Checksum verification

use super::types::{ReplicationCommand, ReplicationError, ReplicationResult};
use crate::core::queue::QueueMessage;
use crate::core::{
    BitmapValue, HyperLogLogValue, KVStore, ListValue, SetValue, StreamEvent, StreamManager,
    SynapError, ZAddOptions,
};
use crate::persistence::apply::apply_operation;
use crate::persistence::types::Operation;
use crate::persistence::{SnapshotImage, StoreRefs};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{debug, info, warn};

/// Format version of the full-sync image this build writes, and the only
/// one its replicas read from a `ReplicationCommand::FullSyncImage`.
/// Version 1 is the 1.3.x KV + stream snapshot carried by
/// `ReplicationCommand::FullSync`.
pub const FULL_SYNC_VERSION: u32 = 2;

/// Every replicated datatype as the master held it at one replication
/// offset (format version [`FULL_SYNC_VERSION`]).
///
/// KV values carry their absolute expiry (Unix ms); HyperLogLog and bitmap
/// values carry their own (`created_at + ttl_secs`). Queues hold their
/// ready messages, as in snapshots.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct FullSyncData {
    pub kv: Vec<(String, Vec<u8>, Option<u64>)>,
    pub hashes: HashMap<String, HashMap<String, Vec<u8>>>,
    pub lists: HashMap<String, ListValue>,
    pub sets: HashMap<String, SetValue>,
    pub sorted_sets: HashMap<String, Vec<(Vec<u8>, f64)>>,
    pub hyperloglogs: HashMap<String, HyperLogLogValue>,
    pub bitmaps: HashMap<String, BitmapValue>,
    pub queues: HashMap<String, Vec<QueueMessage>>,
    pub streams: HashMap<String, Vec<StreamEvent>>,
}

impl FullSyncData {
    /// Build the image from the stores copied under the write barrier plus
    /// the stream events read after it.
    pub fn from_image(image: SnapshotImage, streams: HashMap<String, Vec<StreamEvent>>) -> Self {
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
        Self {
            kv: kv
                .into_iter()
                .map(|(key, value, expires_at_ms)| (key, value.to_vec(), expires_at_ms))
                .collect(),
            hashes,
            lists,
            sets,
            sorted_sets,
            hyperloglogs,
            bitmaps,
            queues,
            streams,
        }
    }

    /// Key count per datatype, for logs.
    pub fn summary(&self) -> String {
        format!(
            "{} KV keys, {} hashes, {} lists, {} sets, {} sorted sets, {} HyperLogLogs, {} bitmaps, {} queues, {} stream rooms",
            self.kv.len(),
            self.hashes.len(),
            self.lists.len(),
            self.sets.len(),
            self.sorted_sets.len(),
            self.hyperloglogs.len(),
            self.bitmaps.len(),
            self.queues.len(),
            self.streams.len()
        )
    }
}

/// Encode `data` as the current full-sync command for `offset`.
pub fn encode_full_sync(data: &FullSyncData, offset: u64) -> ReplicationResult<ReplicationCommand> {
    let image = bincode::serde::encode_to_vec(data, bincode::config::legacy())?;
    Ok(ReplicationCommand::FullSyncImage {
        version: FULL_SYNC_VERSION,
        offset,
        checksum: crc32fast::hash(&image),
        image,
    })
}

/// Check and decode a `FullSyncImage` payload without touching any store.
///
/// Fails on a version other than [`FULL_SYNC_VERSION`] (naming both), on a
/// checksum mismatch and on bytes that do not decode as one whole image.
pub fn decode_full_sync(
    version: u32,
    checksum: u32,
    image: &[u8],
) -> ReplicationResult<FullSyncData> {
    if version != FULL_SYNC_VERSION {
        return Err(ReplicationError::FullSync(format!(
            "the master sent full-sync image version {version}, this replica reads version {FULL_SYNC_VERSION}; upgrade replicas before the master"
        )));
    }
    let actual = crc32fast::hash(image);
    if actual != checksum {
        return Err(ReplicationError::FullSync(format!(
            "checksum mismatch: expected {checksum}, got {actual}"
        )));
    }
    let (data, read): (FullSyncData, usize) =
        bincode::serde::decode_from_slice(image, bincode::config::legacy())?;
    if read != image.len() {
        return Err(ReplicationError::FullSync(format!(
            "{} trailing bytes after the image",
            image.len() - read
        )));
    }
    Ok(data)
}

/// Replace the replica's state with `data`: clear every store present in
/// `stores`, then load the image into them.
///
/// The caller decodes and checks the whole image first
/// ([`decode_full_sync`]), so a bad image never reaches this point, and runs
/// this on the replication task before applying any streamed command. Data
/// for a store the replica does not have is skipped with a warning. KV keys
/// and HyperLogLogs/bitmaps whose deadline passed in transit are not created.
pub async fn load_full_sync(data: FullSyncData, stores: StoreRefs<'_>) -> Result<(), SynapError> {
    clear_all(stores).await;
    let summary = data.summary();
    let FullSyncData {
        kv,
        hashes,
        lists,
        sets,
        sorted_sets,
        hyperloglogs,
        bitmaps,
        queues,
        streams,
    } = data;

    let kv_only = StoreRefs::kv_only(stores.kv_store);
    for (key, value, expires_at_ms) in kv {
        apply_operation(
            Operation::KVSetAt {
                key,
                value,
                expires_at_ms,
            },
            kv_only,
        )
        .await?;
    }
    load_collections(stores, hashes, lists, sets, sorted_sets)?;
    match (stores.hyperloglog_store, hyperloglogs.is_empty()) {
        (Some(hll), _) => hyperloglogs
            .into_iter()
            .for_each(|(k, v)| hll.restore(k, v)),
        (None, false) => skipped("HyperLogLog", hyperloglogs.len()),
        (None, true) => {}
    }
    match (stores.bitmap_store, bitmaps.is_empty()) {
        (Some(bitmap), _) => bitmaps.into_iter().for_each(|(k, v)| bitmap.restore(k, v)),
        (None, false) => skipped("bitmap", bitmaps.len()),
        (None, true) => {}
    }
    load_queues(stores, queues).await?;
    match (stores.stream_manager, streams.is_empty()) {
        (Some(sm), _) => {
            for (room, events) in streams {
                sm.restore_room(&room, events)
                    .await
                    .map_err(SynapError::InternalError)?;
            }
        }
        (None, false) => skipped("stream room", streams.len()),
        (None, true) => {}
    }
    info!("Full sync loaded: {}", summary);
    Ok(())
}

/// Log that the image holds `count` values of a datatype the replica has no
/// store for.
fn skipped(datatype: &str, count: usize) {
    warn!(
        "Full sync: skipping {} {} value(s): this replica has no {} store",
        count, datatype, datatype
    );
}

/// Empty every store present in `stores` (queues and stream rooms are
/// deleted).
async fn clear_all(stores: StoreRefs<'_>) {
    stores.kv_store.clear().await;
    if let Some(h) = stores.hash_store {
        h.clear();
    }
    if let Some(l) = stores.list_store {
        l.clear();
    }
    if let Some(s) = stores.set_store {
        s.clear();
    }
    if let Some(z) = stores.sorted_set_store {
        z.clear();
    }
    if let Some(hll) = stores.hyperloglog_store {
        hll.clear();
    }
    if let Some(bitmap) = stores.bitmap_store {
        bitmap.clear();
    }
    if let Some(qm) = stores.queue_manager {
        for queue in qm.list_queues().await.unwrap_or_default() {
            let _ = qm.delete_queue(&queue).await;
        }
    }
    if let Some(sm) = stores.stream_manager {
        clear_streams(sm).await;
    }
}

/// Delete every stream room.
async fn clear_streams(sm: &StreamManager) {
    for room in sm.list_rooms().await {
        let _ = sm.delete_room(&room).await;
    }
}

/// Load the hash, list, set and sorted-set sections.
fn load_collections(
    stores: StoreRefs<'_>,
    hashes: HashMap<String, HashMap<String, Vec<u8>>>,
    lists: HashMap<String, ListValue>,
    sets: HashMap<String, SetValue>,
    sorted_sets: HashMap<String, Vec<(Vec<u8>, f64)>>,
) -> Result<(), SynapError> {
    match stores.hash_store {
        Some(h) => {
            for (key, fields) in hashes.into_iter().filter(|(_, f)| !f.is_empty()) {
                h.hmset(&key, fields)?;
            }
        }
        None if !hashes.is_empty() => skipped("hash", hashes.len()),
        None => {}
    }
    match stores.list_store {
        Some(l) => {
            for (key, list) in lists {
                let elements = list.into_elements();
                if !elements.is_empty() {
                    l.rpush(&key, elements, false)?;
                }
            }
        }
        None if !lists.is_empty() => skipped("list", lists.len()),
        None => {}
    }
    match stores.set_store {
        Some(s) => {
            for (key, set) in sets {
                let members = set.into_members();
                if !members.is_empty() {
                    s.sadd(&key, members)?;
                }
            }
        }
        None if !sets.is_empty() => skipped("set", sets.len()),
        None => {}
    }
    match stores.sorted_set_store {
        Some(z) => {
            let opts = ZAddOptions::default();
            for (key, members) in sorted_sets {
                for (member, score) in members {
                    z.zadd(&key, member, score, &opts);
                }
            }
        }
        None if !sorted_sets.is_empty() => skipped("sorted set", sorted_sets.len()),
        None => {}
    }
    Ok(())
}

/// Recreate every queue (empty ones included) and publish its messages
/// through the shared applier, the same path a streamed `QueuePublish` takes.
async fn load_queues(
    stores: StoreRefs<'_>,
    queues: HashMap<String, Vec<QueueMessage>>,
) -> Result<(), SynapError> {
    let Some(qm) = stores.queue_manager else {
        if !queues.is_empty() {
            skipped("queue", queues.len());
        }
        return Ok(());
    };
    let queue_only = StoreRefs {
        queue_manager: Some(qm),
        ..StoreRefs::kv_only(stores.kv_store)
    };
    for (queue, messages) in queues {
        qm.create_queue(&queue, None).await?;
        for message in messages {
            apply_operation(
                Operation::QueuePublish {
                    queue: queue.clone(),
                    message,
                },
                queue_only,
            )
            .await?;
        }
    }
    Ok(())
}

/// Replace the replica's KV keys and stream rooms with a 1.3.x full sync
/// (decoded with [`decode_legacy_snapshot`]). The other datatypes are not in
/// that image and are left as they are.
pub async fn load_legacy_full_sync(
    kv_store: &KVStore,
    stream_manager: Option<&StreamManager>,
    operations: Vec<Operation>,
) {
    kv_store.clear().await;
    if let Some(sm) = stream_manager {
        clear_streams(sm).await;
    }
    apply_legacy_operations(kv_store, stream_manager, operations).await;
}

/// Snapshot metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotMetadata {
    pub offset: u64,
    pub timestamp: u64,
    pub total_keys: usize,
    pub total_streams: usize,
    pub compressed: bool,
    pub checksum: u32,
}

/// Create a 1.3.x (version 1) snapshot of the KV store, as older masters
/// sent it.
pub async fn create_snapshot(kv_store: &KVStore, offset: u64) -> Result<Vec<u8>, String> {
    create_snapshot_with_streams(kv_store, None, offset).await
}

/// Create a 1.3.x (version 1) snapshot of the KV store and streams, as
/// older masters sent it. Current masters send [`encode_full_sync`].
pub async fn create_snapshot_with_streams(
    kv_store: &KVStore,
    stream_manager: Option<&StreamManager>,
    offset: u64,
) -> Result<Vec<u8>, String> {
    info!("Creating snapshot at offset {}", offset);

    // Every live key with its absolute expiry (Unix ms), so the replica keeps
    // the master's deadline instead of restarting the TTL on arrival.
    let entries = kv_store
        .dump_with_expiry()
        .await
        .map_err(|e| e.to_string())?;
    let total_keys = entries.len();

    let mut operations = Vec::with_capacity(total_keys);
    for (key, value, expires_at_ms) in entries {
        operations.push(Operation::KVSetAt {
            key,
            value: value.to_vec(),
            expires_at_ms,
        });
    }

    // Add stream operations
    let mut total_streams = 0;
    if let Some(sm) = stream_manager {
        let stream_data = sm.get_all_events().await;
        total_streams = stream_data.len();

        for (room, events) in stream_data {
            for event in events {
                operations.push(Operation::StreamPublish {
                    room: room.clone(),
                    event_type: event.event,
                    payload: event.data,
                });
            }
        }
    }

    // Serialize operations
    let data = bincode::serde::encode_to_vec(&operations, bincode::config::legacy())
        .map_err(|e| e.to_string())?;

    // Calculate checksum
    let checksum = crc32fast::hash(&data);

    // Create metadata
    let metadata = SnapshotMetadata {
        offset,
        timestamp: current_timestamp(),
        total_keys,
        total_streams,
        compressed: false,
        checksum,
    };

    info!(
        "Snapshot created: {} keys, {} streams, {} bytes, checksum: {}",
        total_keys,
        total_streams,
        data.len(),
        checksum
    );

    // Combine metadata + data
    let mut result = bincode::serde::encode_to_vec(&metadata, bincode::config::legacy())
        .map_err(|e| e.to_string())?;
    result.extend_from_slice(&data);

    Ok(result)
}

/// Apply snapshot to KV store (without streams)
pub async fn apply_snapshot(kv_store: &KVStore, snapshot: &[u8]) -> Result<u64, String> {
    apply_snapshot_with_streams(kv_store, None, snapshot).await
}

/// Apply a 1.3.x snapshot with optional stream manager, merging it into the
/// stores (nothing is cleared).
pub async fn apply_snapshot_with_streams(
    kv_store: &KVStore,
    stream_manager: Option<&StreamManager>,
    snapshot: &[u8],
) -> Result<u64, String> {
    let (metadata, operations) = decode_legacy_snapshot(snapshot)?;
    apply_legacy_operations(kv_store, stream_manager, operations).await;
    info!("Snapshot applied successfully");
    Ok(metadata.offset)
}

/// Check and decode a 1.3.x snapshot without touching any store.
pub fn decode_legacy_snapshot(
    snapshot: &[u8],
) -> Result<(SnapshotMetadata, Vec<Operation>), String> {
    // Deserialize metadata
    let (metadata, metadata_size): (SnapshotMetadata, usize) =
        bincode::serde::decode_from_slice(snapshot, bincode::config::legacy())
            .map_err(|e| e.to_string())?;
    let data = &snapshot[metadata_size..];

    // Verify checksum
    let checksum = crc32fast::hash(data);
    if checksum != metadata.checksum {
        return Err(format!(
            "Checksum mismatch: expected {}, got {}",
            metadata.checksum, checksum
        ));
    }

    // Deserialize operations
    let (operations, _): (Vec<Operation>, _) =
        bincode::serde::decode_from_slice(data, bincode::config::legacy())
            .map_err(|e| e.to_string())?;

    info!(
        "Decoded 1.3.x snapshot: {} operations ({} keys, {} streams), offset: {}",
        operations.len(),
        metadata.total_keys,
        metadata.total_streams,
        metadata.offset
    );
    Ok((metadata, operations))
}

/// Apply the KV and stream operations of a 1.3.x snapshot.
async fn apply_legacy_operations(
    kv_store: &KVStore,
    stream_manager: Option<&StreamManager>,
    operations: Vec<Operation>,
) {
    for op in operations {
        match op {
            Operation::KVSet { key, value, ttl } => {
                let _ = kv_store.set(key, value, ttl).await; // S-12: move owned key
            }
            // Images from phase23 masters carry absolute expiry; a key whose
            // deadline has passed in transit is not created.
            op @ Operation::KVSetAt { .. } => {
                let _ = crate::persistence::apply::apply_operation(
                    op,
                    crate::persistence::StoreRefs::kv_only(kv_store),
                )
                .await;
            }
            Operation::StreamPublish {
                room,
                event_type,
                payload,
            } => {
                if let Some(sm) = stream_manager {
                    // Idempotent room creation (synap#165).
                    let _ = sm.get_or_create_room(&room).await;
                    let _ = sm.publish(&room, &event_type, payload).await;
                } else {
                    debug!("Skipping stream operation (no stream manager)");
                }
            }
            _ => {
                debug!("Skipping non-SET/StreamPublish operation in snapshot");
            }
        }
    }
}

fn current_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::KVConfig;

    /// Every datatype store, as a master or a replica holds them.
    struct AllStores {
        kv: KVStore,
        hash: crate::core::HashStore,
        list: crate::core::ListStore,
        set: crate::core::SetStore,
        zset: crate::core::SortedSetStore,
        hll: crate::core::HyperLogLogStore,
        bitmap: crate::core::BitmapStore,
        queues: crate::core::QueueManager,
        streams: StreamManager,
    }

    impl AllStores {
        fn new() -> Self {
            Self {
                kv: KVStore::new(KVConfig::default()),
                hash: crate::core::HashStore::new(),
                list: crate::core::ListStore::new(),
                set: crate::core::SetStore::new(),
                zset: crate::core::SortedSetStore::new(),
                hll: crate::core::HyperLogLogStore::new(),
                bitmap: crate::core::BitmapStore::new(),
                queues: crate::core::QueueManager::new(crate::core::QueueConfig::default()),
                streams: StreamManager::new(crate::core::StreamConfig::default()),
            }
        }

        fn refs(&self) -> StoreRefs<'_> {
            StoreRefs {
                kv_store: &self.kv,
                hash_store: Some(&self.hash),
                list_store: Some(&self.list),
                set_store: Some(&self.set),
                sorted_set_store: Some(&self.zset),
                queue_manager: Some(&self.queues),
                stream_manager: Some(&self.streams),
                hyperloglog_store: Some(&self.hll),
                bitmap_store: Some(&self.bitmap),
            }
        }

        /// One value of every datatype under `prefix`.
        async fn fill(&self, prefix: &str, deadline_ms: u64) {
            use crate::core::{Expiry, SetOptions};
            let k = |name: &str| format!("{prefix}{name}");
            self.kv
                .set_with_opts(
                    &k("kv"),
                    b"v".to_vec(),
                    Some(Expiry::UnixMilliseconds(deadline_ms)),
                    SetOptions::default(),
                )
                .await
                .unwrap();
            self.kv.set(&k("plain"), b"p".to_vec(), None).await.unwrap();
            self.hash.hset(&k("hash"), "f", b"hv".to_vec()).unwrap();
            self.list
                .rpush(&k("list"), vec![b"a".to_vec(), b"b".to_vec()], false)
                .unwrap();
            self.set.sadd(&k("set"), vec![b"m".to_vec()]).unwrap();
            self.zset
                .zadd(&k("zset"), b"z".to_vec(), 2.5, &ZAddOptions::default());
            self.hll
                .pfadd(&k("hll"), vec![b"x".to_vec(), b"y".to_vec()], None)
                .unwrap();
            self.bitmap.setbit(&k("bitmap"), 7, 1).unwrap();
            self.queues.create_queue(&k("queue"), None).await.unwrap();
            self.queues
                .publish(&k("queue"), b"msg".to_vec(), Some(3), Some(2))
                .await
                .unwrap();
            self.queues
                .create_queue(&k("empty_queue"), None)
                .await
                .unwrap();
            self.streams.create_room(&k("room")).await.unwrap();
            self.streams
                .publish(&k("room"), "evt", b"data".to_vec())
                .await
                .unwrap();
        }

        /// A comparable description of every value (queue message ids and
        /// stream event ids are assigned anew on the replica, so payloads
        /// are compared).
        async fn fingerprint(&self) -> Vec<String> {
            let mut out = Vec::new();
            for (key, value, expiry) in self.kv.dump_with_expiry().await.unwrap() {
                out.push(format!("kv:{key}={:?}@{expiry:?}", value.to_vec()));
            }
            for (key, fields) in self.hash.dump() {
                let fields: std::collections::BTreeMap<_, _> = fields.into_iter().collect();
                out.push(format!("hash:{key}={fields:?}"));
            }
            for (key, list) in self.list.dump() {
                out.push(format!("list:{key}={:?}", list.into_elements()));
            }
            for (key, set) in self.set.dump() {
                let mut members = set.into_members();
                members.sort();
                out.push(format!("set:{key}={members:?}"));
            }
            for (key, members) in self.zset.dump() {
                out.push(format!("zset:{key}={members:?}"));
            }
            for (key, hll) in self.hll.dump() {
                out.push(format!("hll:{key}={:?}", hll.registers));
            }
            for (key, bitmap) in self.bitmap.dump() {
                out.push(format!("bitmap:{key}={:?}", bitmap.data));
            }
            for (queue, messages) in self.queues.dump().await.unwrap() {
                let payloads: Vec<_> = messages
                    .iter()
                    .map(|m| (m.payload.to_vec(), m.priority, m.max_retries))
                    .collect();
                out.push(format!("queue:{queue}={payloads:?}"));
            }
            for (room, events) in self.streams.get_all_events().await {
                let events: Vec<_> = events.iter().map(|e| (&e.event, &e.data)).collect();
                out.push(format!("room:{room}={events:?}"));
            }
            out.sort();
            out
        }

        /// The current full-sync command at `offset`.
        async fn full_sync(&self, offset: u64) -> ReplicationCommand {
            let image = SnapshotImage::capture(self.refs()).await.unwrap();
            let data = FullSyncData::from_image(image, self.streams.get_all_events().await);
            encode_full_sync(&data, offset).unwrap()
        }
    }

    fn far_deadline() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64
            + 3_600_000
    }

    fn image_parts(cmd: ReplicationCommand) -> (u32, u64, u32, Vec<u8>) {
        match cmd {
            ReplicationCommand::FullSyncImage {
                version,
                offset,
                checksum,
                image,
            } => (version, offset, checksum, image),
            other => panic!("expected FullSyncImage, got {other:?}"),
        }
    }

    /// phase31: the image carries every datatype, and loading it replaces
    /// the replica's state: stale values of every datatype are gone.
    #[tokio::test]
    async fn full_sync_image_replaces_every_datatype() {
        let master = AllStores::new();
        master.fill("m_", far_deadline()).await;
        let (version, offset, checksum, image) = image_parts(master.full_sync(42).await);
        assert_eq!((version, offset), (FULL_SYNC_VERSION, 42));

        let replica = AllStores::new();
        replica.fill("stale_", far_deadline()).await;
        let data = decode_full_sync(version, checksum, &image).unwrap();
        load_full_sync(data, replica.refs()).await.unwrap();

        let expected = master.fingerprint().await;
        assert_eq!(expected.len(), 11, "fixture covers every datatype");
        assert_eq!(replica.fingerprint().await, expected);
    }

    /// An image version this replica does not read is refused before any
    /// store changes, naming both versions.
    #[tokio::test]
    async fn unsupported_full_sync_version_is_refused() {
        let master = AllStores::new();
        master.fill("m_", far_deadline()).await;
        let (_, _, checksum, image) = image_parts(master.full_sync(1).await);

        let err = decode_full_sync(FULL_SYNC_VERSION + 1, checksum, &image)
            .unwrap_err()
            .to_string();
        assert!(err.contains("version 3"), "{err}");
        assert!(err.contains("reads version 2"), "{err}");
        assert!(err.contains("upgrade replicas before the master"), "{err}");
    }

    /// A corrupted image fails its checksum; one with trailing bytes is
    /// refused too.
    #[tokio::test]
    async fn corrupt_full_sync_image_is_refused() {
        let master = AllStores::new();
        master.fill("m_", far_deadline()).await;
        let (version, _, checksum, mut image) = image_parts(master.full_sync(1).await);

        let last = image.len() - 1;
        image[last] ^= 0xFF;
        let err = decode_full_sync(version, checksum, &image)
            .unwrap_err()
            .to_string();
        assert!(err.contains("checksum mismatch"), "{err}");

        image[last] ^= 0xFF;
        image.push(0);
        let err = decode_full_sync(version, crc32fast::hash(&image), &image)
            .unwrap_err()
            .to_string();
        assert!(err.contains("1 trailing bytes"), "{err}");
    }

    /// A 1.3.x full sync replaces the replica's KV keys and stream rooms and
    /// leaves the datatypes it does not carry alone.
    #[tokio::test]
    async fn legacy_full_sync_replaces_kv_and_streams() {
        let master = AllStores::new();
        master.kv.set("new", b"n".to_vec(), None).await.unwrap();
        master.streams.create_room("room").await.unwrap();
        master
            .streams
            .publish("room", "evt", b"d".to_vec())
            .await
            .unwrap();
        let snapshot = create_snapshot_with_streams(&master.kv, Some(&master.streams), 9)
            .await
            .unwrap();

        let replica = AllStores::new();
        replica.fill("stale_", far_deadline()).await;
        let (metadata, operations) = decode_legacy_snapshot(&snapshot).unwrap();
        assert_eq!(metadata.offset, 9);
        load_legacy_full_sync(&replica.kv, Some(&replica.streams), operations).await;

        let keys: Vec<String> = replica
            .kv
            .dump_with_expiry()
            .await
            .unwrap()
            .into_iter()
            .map(|(k, _, _)| k)
            .collect();
        assert_eq!(keys, vec!["new".to_string()]);
        assert_eq!(replica.streams.list_rooms().await, vec!["room".to_string()]);
        assert_eq!(
            replica.hash.hget("stale_hash", "f").unwrap(),
            Some(b"hv".to_vec()),
            "a 1.3.x image does not cover hashes"
        );
    }

    #[tokio::test]
    async fn test_snapshot_creation_and_application() {
        let kv1 = KVStore::new(KVConfig::default());

        // Populate KV store
        kv1.set("key1", b"value1".to_vec(), None).await.unwrap();
        kv1.set("key2", b"value2".to_vec(), None).await.unwrap();
        kv1.set("key3", b"value3".to_vec(), None).await.unwrap();

        // Create snapshot
        let snapshot = create_snapshot(&kv1, 100).await.unwrap();
        assert!(!snapshot.is_empty());

        // Apply to new KV store
        let kv2 = KVStore::new(KVConfig::default());
        let offset = apply_snapshot(&kv2, &snapshot).await.unwrap();

        assert_eq!(offset, 100);

        // Verify data
        assert_eq!(kv2.get("key1").await.unwrap(), Some(b"value1".to_vec()));
        assert_eq!(kv2.get("key2").await.unwrap(), Some(b"value2".to_vec()));
        assert_eq!(kv2.get("key3").await.unwrap(), Some(b"value3".to_vec()));
    }

    #[tokio::test]
    async fn test_snapshot_checksum_verification() {
        let kv = KVStore::new(KVConfig::default());
        kv.set("test", b"data".to_vec(), None).await.unwrap();

        let mut snapshot = create_snapshot(&kv, 0).await.unwrap();

        // Corrupt data
        if let Some(last) = snapshot.last_mut() {
            *last = !*last;
        }

        // Should fail checksum
        let kv2 = KVStore::new(KVConfig::default());
        let result = apply_snapshot(&kv2, &snapshot).await;

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Checksum mismatch"));
    }

    #[tokio::test]
    async fn test_snapshot_preserves_ttl() {
        let kv1 = KVStore::new(KVConfig::default());

        // Set keys with different TTLs
        kv1.set("key_no_ttl", b"value1".to_vec(), None)
            .await
            .unwrap();
        kv1.set("key_with_ttl", b"value2".to_vec(), Some(3600))
            .await
            .unwrap();

        // Create snapshot
        let snapshot = create_snapshot(&kv1, 0).await.unwrap();
        assert!(!snapshot.is_empty());

        // Apply to new KV store
        let kv2 = KVStore::new(KVConfig::default());
        let _offset = apply_snapshot(&kv2, &snapshot).await.unwrap();

        // Verify values
        assert_eq!(
            kv2.get("key_no_ttl").await.unwrap(),
            Some(b"value1".to_vec())
        );
        assert_eq!(
            kv2.get("key_with_ttl").await.unwrap(),
            Some(b"value2".to_vec())
        );

        // Verify TTLs
        // Key without TTL should return None or error
        let ttl1 = kv2.ttl("key_no_ttl").await;
        assert!(ttl1.is_err() || ttl1.unwrap().is_none());

        // Key with TTL should have remaining TTL (should be close to 3600 seconds)
        let ttl2 = kv2.ttl("key_with_ttl").await.unwrap();
        assert!(ttl2.is_some());
        // TTL should be close to 3600 (allow some small difference due to processing time)
        let remaining = ttl2.unwrap();
        assert!(
            remaining > 3500 && remaining <= 3600,
            "TTL should be preserved, got: {}",
            remaining
        );
    }

    /// phase23: the full-sync image carries each key's absolute deadline, so
    /// a replica keeps the master's deadline and does not create a key whose
    /// deadline passed while the image was in transit.
    #[tokio::test]
    async fn full_sync_carries_absolute_expiry() {
        use crate::core::{Expiry, SetOptions};
        let master = KVStore::new(KVConfig::default());
        let deadline = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64
            + 50_000;
        master
            .set_with_opts(
                "long",
                b"v".to_vec(),
                Some(Expiry::UnixMilliseconds(deadline)),
                SetOptions::default(),
            )
            .await
            .unwrap();
        master
            .set_with_opts(
                "short",
                b"v".to_vec(),
                Some(Expiry::Milliseconds(300)),
                SetOptions::default(),
            )
            .await
            .unwrap();
        let image = create_snapshot(&master, 7).await.unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(400)).await;

        let replica = KVStore::new(KVConfig::default());
        assert_eq!(apply_snapshot(&replica, &image).await.unwrap(), 7);
        let dump = replica.dump_with_expiry().await.unwrap();
        assert_eq!(dump.len(), 1, "the expired key was created on the replica");
        assert_eq!(dump[0].0, "long");
        assert_eq!(dump[0].2, Some(deadline));
    }
}
