use super::types::{PersistenceConfig, Result};
use super::{SnapshotManager, WriteAheadLog};
use crate::core::QueueConfig;
use crate::core::hash::HashStore;
use crate::core::kv_store::KVStore;
use crate::core::list::ListStore;
use crate::core::queue::{QueueManager, QueueState};
use crate::core::set::SetStore;
use crate::core::sorted_set::{SortedSetStore, ZAddOptions};
use crate::core::types::KVConfig;
use crate::core::{BitmapStore, Expiry, HyperLogLogStore, SetOptions};
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::info;

/// Every store rebuilt by [`recover_all`], plus the WAL offset replay ended at.
pub struct RecoveredStores {
    pub kv_store: KVStore,
    pub hash_store: Option<HashStore>,
    pub list_store: Option<ListStore>,
    pub set_store: Option<SetStore>,
    pub sorted_set_store: Option<SortedSetStore>,
    pub queue_manager: Option<QueueManager>,
    pub hyperloglog_store: HyperLogLogStore,
    pub bitmap_store: BitmapStore,
    pub wal_offset: u64,
}

/// Recover system state from persistence.
///
/// Returns the KV, collection and queue stores as a tuple; use
/// [`recover_all`] to also get the HyperLogLog and bitmap stores the WAL
/// replay rebuilt.
pub async fn recover(
    config: &PersistenceConfig,
    kv_config: KVConfig,
    queue_config: QueueConfig,
) -> Result<(
    KVStore,
    Option<HashStore>,
    Option<ListStore>,
    Option<SetStore>,
    Option<SortedSetStore>,
    Option<QueueManager>,
    u64,
)> {
    let r = recover_all(config, kv_config, queue_config).await?;
    Ok((
        r.kv_store,
        r.hash_store,
        r.list_store,
        r.set_store,
        r.sorted_set_store,
        r.queue_manager,
        r.wal_offset,
    ))
}

/// Recover every store from the latest snapshot plus the WAL written after it.
///
/// Expiry is absolute (phase23): snapshot keys keep the deadline they had
/// (v4 files; v3/v2 keys load without expiry), WAL records written before
/// phase23 have their relative TTL anchored at the entry's own timestamp, and
/// any key whose deadline has already passed is left out. HyperLogLog and
/// bitmap data come from the v4 snapshot sections plus the WAL.
///
/// Queue messages keep their identity (phase24): snapshot queues are restored
/// whole (v5: configuration, ready, in-flight and dead-lettered messages with
/// their metadata; older files: the ready messages), WAL publishes are
/// restored with their original ID and metadata, and replayed ACK/NACK/purge
/// records act by message ID. A replay error other than a settled message
/// being already gone fails recovery.
pub async fn recover_all(
    config: &PersistenceConfig,
    kv_config: KVConfig,
    queue_config: QueueConfig,
) -> Result<RecoveredStores> {
    if !config.enabled {
        info!("Persistence disabled, starting with fresh state");
        return Ok(RecoveredStores {
            kv_store: KVStore::new(kv_config),
            hash_store: Some(HashStore::new()),
            list_store: Some(ListStore::new()),
            set_store: Some(SetStore::new()),
            sorted_set_store: Some(SortedSetStore::new()),
            queue_manager: Some(QueueManager::new(queue_config)),
            hyperloglog_store: HyperLogLogStore::new(),
            bitmap_store: BitmapStore::new(),
            wal_offset: 0,
        });
    }

    info!("Starting recovery process...");

    let snapshot_mgr = SnapshotManager::new(config.snapshot.clone());
    let wal = WriteAheadLog::open(config.wal.clone()).await?;
    let hyperloglog_store = HyperLogLogStore::new();
    let bitmap_store = BitmapStore::new();

    // Step 1: Load latest snapshot (if exists)
    let (kv_store, hash_store, list_store, set_store, sorted_set_store, queue_manager, last_offset) =
        if let Some((snapshot, path)) = snapshot_mgr.load_latest().await? {
            info!(
                "Loaded snapshot from {:?} at offset {}",
                path, snapshot.wal_offset
            );

            // Restore KV store, each key with its absolute expiry; keys that
            // expired while the server was down are left out.
            let kv = KVStore::new(kv_config);
            let now_ms = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64;
            let mut expired = 0usize;
            for (key, value) in snapshot.kv_data {
                match snapshot.kv_expires_at_ms.get(&key) {
                    None => kv.set(&key, value, None).await?,
                    Some(&deadline) if deadline <= now_ms => expired += 1,
                    Some(&deadline) => {
                        kv.set_with_opts(
                            &key,
                            value,
                            Some(Expiry::UnixMilliseconds(deadline)),
                            SetOptions::default(),
                        )
                        .await?;
                    }
                }
            }
            if expired > 0 {
                info!(
                    "Skipped {} snapshot keys that expired before restart",
                    expired
                );
            }

            // Restore HyperLogLog and bitmap values (v4); expired ones are skipped.
            for (key, value) in snapshot.hyperloglog_data {
                hyperloglog_store.restore(key, value);
            }
            for (key, value) in snapshot.bitmap_data {
                bitmap_store.restore(key, value);
            }

            // Restore every queue whole: configuration (v5; older files get
            // the server default) and every message with its original ID and
            // metadata. In-flight messages become ready again.
            let queues = QueueManager::new(queue_config.clone());
            for (queue_name, image) in snapshot.queue_data {
                let records = |list: Vec<_>| list.into_iter().map(Into::into).collect();
                queues.restore_queue(
                    &queue_name,
                    QueueState {
                        config: image.config.unwrap_or_else(|| queue_config.clone()),
                        ready: records(image.ready),
                        in_flight: records(image.in_flight),
                        dead_letter: records(image.dead_letter),
                    },
                );
            }

            // Restore List store
            let lists = ListStore::new();
            for (key, list_value) in snapshot.list_data {
                // Restore list by pushing all elements
                let elements: Vec<Vec<u8>> = list_value.into_elements();
                if !elements.is_empty() {
                    lists.rpush(&key, elements, false)?;
                }
            }

            // Restore Set store
            let sets = SetStore::new();
            for (key, set_value) in snapshot.set_data {
                // Restore set by adding all members
                let members: Vec<Vec<u8>> = set_value.into_members();
                if !members.is_empty() {
                    sets.sadd(&key, members)?;
                }
            }

            // Restore Sorted Set store
            let sorted_sets = SortedSetStore::new();
            for (key, members_scores) in snapshot.sorted_set_data {
                // Restore sorted set by adding all members with their scores
                let opts = ZAddOptions::default();
                for (member, score) in members_scores {
                    sorted_sets.zadd(&key, member, score, &opts);
                }
            }

            // Restore Hash store
            let hashes = HashStore::new();
            for (key, fields) in snapshot.hash_data {
                for (field, value) in fields {
                    hashes.hset(&key, &field, value)?;
                }
            }

            (
                kv,
                Some(hashes),
                Some(lists),
                Some(sets),
                Some(sorted_sets),
                Some(queues),
                snapshot.wal_offset,
            )
        } else {
            info!("No snapshot found, starting fresh");
            (
                KVStore::new(kv_config),
                Some(HashStore::new()),
                Some(ListStore::new()),
                Some(SetStore::new()),
                Some(SortedSetStore::new()),
                Some(QueueManager::new(queue_config)),
                0,
            )
        };

    // Step 2: Replay WAL from snapshot offset
    info!("Replaying WAL from offset {}...", last_offset);
    let entries = wal.replay(last_offset).await?;
    let mut replayed = 0;

    for entry in entries {
        // Records written before phase23 carry TTLs relative to when they were
        // written; anchor them at the entry's timestamp.
        crate::persistence::apply::apply_operation(
            entry.operation.with_absolute_expiry(entry.timestamp),
            crate::persistence::apply::StoreRefs {
                kv_store: &kv_store,
                hash_store: hash_store.as_ref(),
                list_store: list_store.as_ref(),
                set_store: set_store.as_ref(),
                sorted_set_store: sorted_set_store.as_ref(),
                queue_manager: queue_manager.as_ref(),
                stream_manager: None, // WAL recovery skips streams (StreamPersistence owns them)
                hyperloglog_store: Some(&hyperloglog_store),
                bitmap_store: Some(&bitmap_store),
            },
        )
        .await?;
        replayed += 1;
    }

    info!("Recovery complete. Replayed {} operations", replayed);

    let final_offset = last_offset + replayed;

    Ok(RecoveredStores {
        kv_store,
        hash_store,
        list_store,
        set_store,
        sorted_set_store,
        queue_manager,
        hyperloglog_store,
        bitmap_store,
        wal_offset: final_offset,
    })
}

/// Test recovery without actually loading data (validation only)
pub async fn validate_recovery(config: &PersistenceConfig) -> Result<RecoveryInfo> {
    let snapshot_mgr = SnapshotManager::new(config.snapshot.clone());
    let wal = WriteAheadLog::open(config.wal.clone()).await?;

    let snapshot_info = if let Some((snapshot, path)) = snapshot_mgr.load_latest().await? {
        Some(SnapshotInfo {
            path,
            offset: snapshot.wal_offset,
            timestamp: snapshot.timestamp,
            kv_count: snapshot.kv_data.len(),
            queue_count: snapshot.queue_data.len(),
        })
    } else {
        None
    };

    let wal_offset = wal.current_offset();
    let wal_entries = if let Some(ref si) = snapshot_info {
        wal.replay(si.offset).await?.len()
    } else {
        wal.replay(0).await?.len()
    };

    Ok(RecoveryInfo {
        snapshot: snapshot_info,
        wal_offset,
        wal_entries_to_replay: wal_entries,
    })
}

/// Information about recovery state
#[derive(Debug)]
pub struct RecoveryInfo {
    pub snapshot: Option<SnapshotInfo>,
    pub wal_offset: u64,
    pub wal_entries_to_replay: usize,
}

/// Information about a snapshot
#[derive(Debug)]
pub struct SnapshotInfo {
    pub path: std::path::PathBuf,
    pub offset: u64,
    pub timestamp: u64,
    pub kv_count: usize,
    pub queue_count: usize,
}
