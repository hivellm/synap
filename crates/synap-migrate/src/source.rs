//! Reading a Synap data directory without changing it.
//!
//! The directory is read the way the server reads it at startup, with the
//! server's own code: the newest snapshot (every format and file name the
//! server reads: v2 to v5, sequenced and legacy names), then the WAL records
//! written after that snapshot's cutoff (headerless 1.3.x files and versioned
//! ones), replayed into the stores by [`recovery::rebuild`]. Startup also
//! repairs a torn WAL tail and upgrades a 1.3.x WAL in place; this reader does
//! neither and writes nothing.
//!
//! The read is stricter than startup: every snapshot file must load, not only
//! the newest (startup may fall back past a damaged one), and the whole WAL
//! must scan. Corrupt or unsupported input fails the read with an error that
//! names the file and the reason, before the caller writes anything.

use anyhow::{Context, Result, bail};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use synap_server::persistence::recovery::{self, RecoveredStores};
use synap_server::persistence::types::{PersistenceError, StreamEvent};
use synap_server::persistence::wal_frame::{self, ScanEnd, WalFormat};
use synap_server::persistence::{Operation, Snapshot, StoreRefs, WALEntry};
use synap_server::{KVConfig, QueueConfig};

use crate::snapshot;

/// Where a data directory keeps its files: the server's default layout,
/// `<data dir>/snapshots/` and `<data dir>/wal/synap.wal`.
#[derive(Debug, Clone)]
pub struct Layout {
    pub snapshot_dir: PathBuf,
    pub wal_path: PathBuf,
}

impl Layout {
    /// The layout of the data directory `data_dir`.
    pub fn of(data_dir: &Path) -> Self {
        Self {
            snapshot_dir: data_dir.join("snapshots"),
            wal_path: data_dir.join("wal").join("synap.wal"),
        }
    }
}

/// The snapshot a read started from.
#[derive(Debug, Clone)]
pub struct SnapshotRead {
    pub path: PathBuf,
    /// Format version (2 to 5).
    pub version: u32,
    /// The WAL offset its replay starts at.
    pub wal_offset: u64,
}

/// What the WAL held.
#[derive(Debug, Clone)]
pub struct WalRead {
    pub path: PathBuf,
    pub format: WalFormat,
    /// Valid records in the file.
    pub records: u64,
    /// Records at or after the snapshot cutoff, replayed into the stores.
    pub replayed: usize,
    /// Stream publish records among them, which replay skips as startup
    /// does (streams come from the snapshot).
    pub stream_records: usize,
    /// A torn tail (bytes a crash left in the middle of an append), which the
    /// server discards at startup and this read skips the same way.
    pub torn_tail: Option<String>,
}

/// A data directory as the server would recover it.
pub struct Source {
    pub stores: RecoveredStores,
    /// The snapshot's stream section (streams are not in the WAL).
    pub streams: HashMap<String, Vec<StreamEvent>>,
    pub snapshot: Option<SnapshotRead>,
    pub wal: WalRead,
}

impl Source {
    /// Every rebuilt store, for [`SnapshotImage::capture`](synap_server::persistence::SnapshotImage::capture).
    pub fn store_refs(&self) -> StoreRefs<'_> {
        StoreRefs {
            kv_store: &self.stores.kv_store,
            hash_store: self.stores.hash_store.as_ref(),
            list_store: self.stores.list_store.as_ref(),
            set_store: self.stores.set_store.as_ref(),
            sorted_set_store: self.stores.sorted_set_store.as_ref(),
            queue_manager: self.stores.queue_manager.as_ref(),
            stream_manager: None,
            hyperloglog_store: Some(&self.stores.hyperloglog_store),
            bitmap_store: Some(&self.stores.bitmap_store),
        }
    }
}

/// The KV configuration a read rebuilds with: no memory cap and no value size
/// limit, so rebuilding never evicts or refuses a key the directory holds.
fn unbounded_kv_config() -> KVConfig {
    KVConfig {
        // The store compares `max_memory_mb << 20` as an i64.
        max_memory_mb: usize::try_from(i64::MAX >> 20).unwrap_or(usize::MAX >> 20),
        max_value_size_bytes: None,
        ..KVConfig::default()
    }
}

/// Read the data directory at `layout` as the server would recover it,
/// verifying every snapshot file and the whole WAL first.
/// `queue_defaults` is the configuration of queues the directory does not
/// record one for (snapshots older than v5, queues created implicitly by a
/// replayed publish), as the server's own queue settings would give them.
pub async fn read(layout: &Layout, queue_defaults: QueueConfig) -> Result<Source> {
    let (mut snapshot, snapshot_read) = match load_every_snapshot(&layout.snapshot_dir).await? {
        Some((snapshot, path)) => {
            let read = SnapshotRead {
                path,
                version: snapshot.version,
                wal_offset: snapshot.wal_offset,
            };
            (Some(snapshot), Some(read))
        }
        None => (None, None),
    };
    let cutoff = snapshot_read.as_ref().map_or(0, |s| s.wal_offset);
    let streams = snapshot
        .as_mut()
        .map(|s| std::mem::take(&mut s.stream_data))
        .unwrap_or_default();

    let (entries, wal, next_offset) = read_wal(layout.wal_path.clone(), cutoff).await?;
    let stores = recovery::rebuild(
        snapshot,
        entries,
        next_offset,
        unbounded_kv_config(),
        queue_defaults,
    )
    .await
    .with_context(|| {
        format!(
            "cannot replay the WAL {} onto the snapshot",
            layout.wal_path.display()
        )
    })?;
    Ok(Source {
        stores,
        streams,
        snapshot: snapshot_read,
        wal,
    })
}

/// Load and verify every snapshot file in `snapshot_dir`, oldest first, and
/// return the newest. Any file that does not load fails the read, naming the
/// file and the reason (a truncated or damaged file, a checksum mismatch, a
/// format this release does not read).
async fn load_every_snapshot(snapshot_dir: &Path) -> Result<Option<(Snapshot, PathBuf)>> {
    let manager = snapshot::manager(snapshot_dir);
    let files = manager.list_snapshots().await.with_context(|| {
        format!(
            "cannot list the snapshot directory {}",
            snapshot_dir.display()
        )
    })?;
    let newest = files.len().checked_sub(1);
    let mut loaded = None;
    for (index, path) in files.into_iter().enumerate() {
        let snapshot = match manager.load(&path).await {
            Ok(snapshot) => snapshot,
            // The error names the file and the reason.
            Err(e @ PersistenceError::SnapshotCorrupted { .. }) => return Err(e.into()),
            Err(e) => bail!("cannot read the snapshot {}: {e}", path.display()),
        };
        // Only the newest one is kept; the others are dropped once verified.
        if Some(index) == newest {
            loaded = Some((snapshot, path));
        }
    }
    Ok(loaded)
}

/// Stream publish records in `operation`, a transaction's included.
fn stream_records(operation: &Operation) -> usize {
    match operation {
        Operation::StreamPublish { .. } => 1,
        Operation::Transaction { operations } => operations.iter().map(stream_records).sum(),
        _ => 0,
    }
}

/// Scan the WAL at `path` without repairing it, keeping the records at or
/// after `cutoff` in file order. Also returns the offset the next record
/// would get: past every record and never below the cutoff, as startup
/// recovery computes it.
async fn read_wal(path: PathBuf, cutoff: u64) -> Result<(Vec<WALEntry>, WalRead, u64)> {
    tokio::task::spawn_blocking(move || {
        let mut entries = Vec::new();
        let report = wal_frame::scan(&path, |entry| {
            if entry.offset >= cutoff {
                entries.push(entry);
            }
        })
        .with_context(|| format!("cannot read the WAL {}", path.display()))?;
        let next_offset = report.next_offset().max(cutoff);
        let torn_tail = match report.end {
            ScanEnd::Clean => None,
            ScanEnd::TornTail {
                valid_len,
                file_len,
                reason,
            } => Some(format!(
                "bytes {valid_len} to {file_len} are a torn tail ({reason})"
            )),
        };
        let wal = WalRead {
            path,
            format: report.format,
            records: report.valid_frames,
            replayed: entries.len(),
            stream_records: entries.iter().map(|e| stream_records(&e.operation)).sum(),
            torn_tail,
        };
        Ok((entries, wal, next_offset))
    })
    .await
    .context("the WAL reader stopped")?
}

#[cfg(test)]
mod tests {
    use super::*;
    use synap_server::KVStore;

    fn set(offset: u64, key: &str, value: &str, ttl: Option<u64>) -> WALEntry {
        WALEntry {
            offset,
            timestamp: 1_700_000_000,
            operation: Operation::KVSet {
                key: key.into(),
                value: value.as_bytes().to_vec(),
                ttl,
            },
        }
    }

    /// Write `entries` as a WAL file: versioned (header first) or legacy
    /// (1.3.x, frames from byte 0).
    fn write_wal(path: &Path, legacy: bool, entries: &[WALEntry]) -> Vec<u8> {
        let mut bytes = Vec::new();
        if !legacy {
            bytes.extend_from_slice(&wal_frame::encode_file_header());
        }
        for entry in entries {
            bytes.extend_from_slice(&wal_frame::encode_frame(entry).unwrap());
        }
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, &bytes).unwrap();
        bytes
    }

    /// A snapshot at WAL cutoff 3 holding `a`, written by the server.
    async fn snapshot_at_cutoff_3(layout: &Layout) {
        let kv = KVStore::new(KVConfig::default());
        kv.set("a", b"snap".to_vec(), None).await.unwrap();
        snapshot::manager(&layout.snapshot_dir)
            .create_snapshot(StoreRefs::kv_only(&kv), 3)
            .await
            .unwrap();
    }

    async fn get(source: &Source, key: &str) -> Option<Vec<u8>> {
        source.stores.kv_store.get(key).await.unwrap()
    }

    /// Records from the snapshot's cutoff on are replayed onto it, in legacy
    /// and versioned WALs alike; earlier records are not, a relative TTL is
    /// anchored at its record's time, and the files are left as they were.
    #[tokio::test]
    async fn the_wal_tail_after_the_snapshot_is_replayed_in_both_wal_formats() {
        for legacy in [true, false] {
            let dir = tempfile::tempdir().unwrap();
            let layout = Layout::of(dir.path());
            snapshot_at_cutoff_3(&layout).await;
            let wal_bytes = write_wal(
                &layout.wal_path,
                legacy,
                &[
                    set(1, "a", "old", None),
                    set(2, "before", "cut", None),
                    set(3, "tail", "t", None),
                    WALEntry {
                        offset: 4,
                        timestamp: 1_700_000_000,
                        operation: Operation::KVDel {
                            keys: vec!["a".into()],
                        },
                    },
                    set(5, "expired", "x", Some(60)),
                ],
            );

            let source = read(&layout, QueueConfig::default()).await.unwrap();
            assert_eq!(source.snapshot.as_ref().unwrap().wal_offset, 3);
            assert_eq!(get(&source, "tail").await, Some(b"t".to_vec()));
            assert_eq!(get(&source, "a").await, None, "the tail deleted it");
            assert_eq!(get(&source, "before").await, None, "below the cutoff");
            assert_eq!(get(&source, "expired").await, None, "expired in 2023");
            let expected_format = if legacy {
                WalFormat::Legacy
            } else {
                WalFormat::Versioned(2)
            };
            assert_eq!(source.wal.format, expected_format);
            assert_eq!(source.wal.records, 5);
            assert_eq!(source.wal.replayed, 3);
            assert_eq!(source.stores.wal_offset, 6);
            assert!(source.wal.torn_tail.is_none());
            assert_eq!(std::fs::read(&layout.wal_path).unwrap(), wal_bytes);
        }
    }

    /// A torn tail is skipped and reported, as startup does, but the file is
    /// not repaired.
    #[tokio::test]
    async fn a_torn_tail_is_skipped_and_reported_without_repair() {
        let dir = tempfile::tempdir().unwrap();
        let layout = Layout::of(dir.path());
        let mut bytes = write_wal(
            &layout.wal_path,
            false,
            &[set(1, "kept", "k", None), set(2, "torn", "t", None)],
        );
        bytes.truncate(bytes.len() - 3);
        std::fs::write(&layout.wal_path, &bytes).unwrap();

        let source = read(&layout, QueueConfig::default()).await.unwrap();
        assert!(source.snapshot.is_none());
        assert_eq!(get(&source, "kept").await, Some(b"k".to_vec()));
        assert_eq!(get(&source, "torn").await, None);
        assert_eq!(source.wal.records, 1);
        let torn = source.wal.torn_tail.as_deref().unwrap();
        assert!(torn.contains("torn tail"), "{torn}");
        assert_eq!(std::fs::read(&layout.wal_path).unwrap(), bytes);
    }
}
