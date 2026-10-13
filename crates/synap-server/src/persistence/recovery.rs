use super::types::{PersistenceConfig, PersistenceError, Result, Snapshot, WALEntry};
use super::wal_frame::{self, ScanEnd};
use super::{SnapshotManager, WriteAheadLog};
use crate::core::QueueConfig;
use crate::core::hash::HashStore;
use crate::core::kv_store::KVStore;
use crate::core::list::ListStore;
use crate::core::queue::QueueManager;
use crate::core::set::SetStore;
use crate::core::sorted_set::{SortedSetStore, ZAddOptions};
use crate::core::types::KVConfig;
use crate::core::{BitmapStore, Expiry, HyperLogLogStore, SetOptions};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{info, warn};

/// Every store rebuilt by [`recover_all`], plus the offset the next WAL record
/// must get.
pub struct RecoveredStores {
    pub kv_store: KVStore,
    pub hash_store: Option<HashStore>,
    pub list_store: Option<ListStore>,
    pub set_store: Option<SetStore>,
    pub sorted_set_store: Option<SortedSetStore>,
    pub queue_manager: Option<QueueManager>,
    pub hyperloglog_store: HyperLogLogStore,
    pub bitmap_store: BitmapStore,
    /// Offset the next WAL record must get: past every record in the WAL and
    /// never below the loaded snapshot's cutoff. Hand it to
    /// [`PersistenceLayer::continue_wal_at`](super::PersistenceLayer::continue_wal_at).
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
/// Snapshots are tried newest first. A damaged file (truncated, failing its
/// checksum, undecodable) is skipped only when the WAL provably still holds
/// every record after the older snapshot's cutoff (the whole history when no
/// valid snapshot is left); otherwise recovery fails with
/// [`PersistenceError::SnapshotUnrecoverable`], which refuses startup. A
/// snapshot that cannot be read for another reason (permissions, device
/// errors) also fails recovery: it may be intact.
///
/// Expiry is absolute (phase23): snapshot keys keep the deadline they had
/// (v4 files; v3/v2 keys load without expiry), WAL records written before
/// phase23 have their relative TTL anchored at the entry's own timestamp, and
/// any key whose deadline has already passed is left out. HyperLogLog and
/// bitmap data come from the v4 snapshot sections plus the WAL.
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
    // Opening the WAL repairs a torn tail and upgrades a 1.3.x file before it
    // is read below.
    WriteAheadLog::open(config.wal.clone()).await?;
    let hyperloglog_store = HyperLogLogStore::new();
    let bitmap_store = BitmapStore::new();

    // Step 1: pick the newest valid snapshot and check that skipping any
    // damaged newer file loses nothing.
    let choice = load_newest_valid(&snapshot_mgr, &config.snapshot.directory).await?;
    let cutoff = choice.loaded.as_ref().map_or(0, |(s, _)| s.wal_offset);
    let wal = read_wal(config.wal.path.clone(), cutoff).await?;
    check_fallback(
        config,
        &choice.damaged,
        choice.loaded.as_ref().map(|(_, path)| path.as_path()),
        cutoff,
        &wal,
    )?;
    let highest = wal.highest.unwrap_or(0);
    if cutoff > highest.saturating_add(1) {
        warn!(
            "The WAL ends before the snapshot cutoff {} (last record {}): records the snapshot holds were lost from the WAL, likely by a power loss right after a snapshot taken by an earlier release. New records continue from offset {}",
            cutoff, highest, cutoff
        );
    }

    // Step 2: restore the snapshot
    let (kv_store, hash_store, list_store, set_store, sorted_set_store, queue_manager, last_offset) =
        if let Some((snapshot, path)) = choice.loaded {
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

            // Restore Queue manager
            let queues = QueueManager::new(queue_config);
            for (queue_name, messages) in snapshot.queue_data {
                queues.create_queue(&queue_name, None).await?;
                for message in messages {
                    queues
                        .publish(
                            &queue_name,
                            (*message.payload).clone(), // Convert Arc<Vec<u8>> to Vec<u8>
                            Some(message.priority),
                            Some(message.max_retries),
                        )
                        .await?;
                }
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

    // Step 3: replay the WAL from the snapshot's cutoff
    info!("Replaying WAL from offset {}...", last_offset);
    let next_offset = wal.next_offset(last_offset);
    let mut replayed = 0;

    for entry in wal.entries {
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

    Ok(RecoveredStores {
        kv_store,
        hash_store,
        list_store,
        set_store,
        sorted_set_store,
        queue_manager,
        hyperloglog_store,
        bitmap_store,
        wal_offset: next_offset,
    })
}

/// The snapshot recovery starts from, and the damaged newer files it skipped.
struct SnapshotChoice {
    loaded: Option<(Snapshot, PathBuf)>,
    damaged: Vec<PathBuf>,
}

/// Load the newest snapshot that verifies, trying files newest first.
///
/// Damaged files are skipped and returned. Any other failure stops recovery:
/// a file that cannot be read may be intact, and skipping it could lose the
/// writes it holds.
async fn load_newest_valid(mgr: &SnapshotManager, dir: &Path) -> Result<SnapshotChoice> {
    let files = mgr.list_snapshots().await.map_err(|e| {
        PersistenceError::SnapshotUnrecoverable(format!(
            "cannot list the snapshot directory {dir:?}: {e}"
        ))
    })?;
    let mut damaged = Vec::new();
    for path in files.into_iter().rev() {
        match mgr.load(&path).await {
            Ok(snapshot) => {
                return Ok(SnapshotChoice {
                    loaded: Some((snapshot, path)),
                    damaged,
                });
            }
            Err(PersistenceError::SnapshotCorrupted(_)) => damaged.push(path),
            Err(e) => {
                return Err(PersistenceError::SnapshotUnrecoverable(format!(
                    "cannot read snapshot {path:?}: {e}"
                )));
            }
        }
    }
    Ok(SnapshotChoice {
        loaded: None,
        damaged,
    })
}

/// The WAL records to replay, plus what the whole file says about which
/// offsets it still holds.
struct WalReplay {
    /// Records at or after the cutoff, in file order.
    entries: Vec<WALEntry>,
    lowest: Option<u64>,
    highest: Option<u64>,
    /// First record after the cutoff whose offset does not follow the record
    /// before it: `(previous, found)`.
    break_at: Option<(u64, u64)>,
}

impl WalReplay {
    /// Offset the next record must get: past every record in the WAL, and
    /// never below `cutoff`, since the snapshot already holds every record
    /// below it.
    fn next_offset(&self, cutoff: u64) -> u64 {
        self.highest
            .map_or(1, |highest| highest.saturating_add(1))
            .max(cutoff)
    }
}

/// Read the WAL at `path` once: keep the records at or after `cutoff` and
/// note its lowest and highest offsets and the first break in the offsets
/// that reaches past the cutoff.
async fn read_wal(path: PathBuf, cutoff: u64) -> Result<WalReplay> {
    tokio::task::spawn_blocking(move || {
        let mut wal = WalReplay {
            entries: Vec::new(),
            lowest: None,
            highest: None,
            break_at: None,
        };
        let mut previous: Option<u64> = None;
        let report = wal_frame::scan(&path, |entry| {
            let offset = entry.offset;
            if let Some(prev) = previous
                && offset > cutoff
                && prev.checked_add(1) != Some(offset)
                && wal.break_at.is_none()
            {
                wal.break_at = Some((prev, offset));
            }
            previous = Some(offset);
            wal.lowest = Some(wal.lowest.map_or(offset, |lowest| lowest.min(offset)));
            wal.highest = Some(wal.highest.map_or(offset, |highest| highest.max(offset)));
            if offset >= cutoff {
                wal.entries.push(entry);
            }
        })?;
        if let ScanEnd::TornTail {
            valid_len, reason, ..
        } = &report.end
        {
            warn!(
                "WAL {:?}: replay stops at an unrepaired torn tail at byte {} ({})",
                path, valid_len, reason
            );
        }
        info!("Replayed {} WAL entries", wal.entries.len());
        Ok(wal)
    })
    .await
    .map_err(|e| PersistenceError::IOError(std::io::Error::other(e)))?
}

/// Check that skipping the `damaged` snapshot files loses nothing.
///
/// Recovery falls back to `fallback` (cutoff `cutoff`), or to an empty state
/// plus the whole WAL when no valid snapshot is left. That is complete only if
/// the WAL still holds every record from the cutoff on: the WAL is enabled,
/// reaches back to the cutoff (offsets start at 1, so an uncut WAL starts
/// there), has no break in its offsets after the cutoff, and holds records if
/// the snapshot was taken after some were written. The server never cuts the
/// WAL, so this fails only after the WAL was removed, truncated or replaced,
/// or with the WAL disabled.
fn check_fallback(
    config: &PersistenceConfig,
    damaged: &[PathBuf],
    fallback: Option<&Path>,
    cutoff: u64,
    wal: &WalReplay,
) -> Result<()> {
    if damaged.is_empty() {
        return Ok(());
    }
    let target = match fallback {
        Some(path) => format!("the older snapshot {path:?} (WAL cutoff {cutoff})"),
        None => "an empty state plus the whole WAL (no valid snapshot is left)".to_string(),
    };
    let first_needed = cutoff.max(1);
    let gap = if !config.wal.enabled {
        Some(
            "the WAL is disabled, so nothing else records the writes the damaged files hold"
                .to_string(),
        )
    } else if let Some((previous, found)) = wal.break_at {
        Some(format!(
            "the WAL offsets jump from {previous} to {found}, so records after the cutoff are missing"
        ))
    } else {
        match wal.lowest {
            Some(lowest) if lowest > first_needed => Some(format!(
                "the WAL starts at offset {lowest}, so the records from {first_needed} to {} are gone",
                lowest - 1
            )),
            None if cutoff > 1 => Some(format!(
                "the WAL holds no records although that snapshot was taken at offset {cutoff}, so the WAL was removed or replaced"
            )),
            _ => None,
        }
    };
    match gap {
        None => {
            warn!(
                "Snapshot file(s) {:?} are damaged; recovering from {} instead, which loses nothing: the WAL still holds every record after it",
                damaged, target
            );
            Ok(())
        }
        Some(gap) => Err(PersistenceError::SnapshotUnrecoverable(format!(
            "the newest snapshot file(s) {damaged:?} are damaged, and falling back to {target} would lose acknowledged writes: {gap}. Restore the damaged files from a backup, or move them out of the snapshot directory to accept that loss"
        ))),
    }
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
