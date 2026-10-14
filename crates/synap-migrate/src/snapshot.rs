//! Snapshot access through the server's own codec.
//!
//! Every byte the migrator reads or writes goes through
//! `synap_server::persistence`: [`SnapshotManager`] finds snapshot files by the
//! server's naming rules, decodes every format the server reads and writes the
//! format it writes. There is no second implementation of the format here.

use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use synap_server::hub::MultiTenant;
use synap_server::persistence::types::{SnapshotConfig, StreamEvent};
use synap_server::persistence::{SnapshotImage, SnapshotManager};
use uuid::Uuid;

/// The server's snapshot manager over `snapshot_dir`.
pub fn manager(snapshot_dir: &Path) -> SnapshotManager {
    SnapshotManager::new(SnapshotConfig {
        directory: snapshot_dir.to_path_buf(),
        ..SnapshotConfig::default()
    })
}

/// Write `image` and the stream section `streams` into `snapshot_dir` in the
/// format and under the name the server writes, with WAL replay starting at
/// `wal_offset`.
pub async fn write(
    snapshot_dir: &Path,
    image: SnapshotImage,
    streams: HashMap<String, Vec<StreamEvent>>,
    wal_offset: u64,
) -> Result<PathBuf> {
    manager(snapshot_dir)
        .write_snapshot(image, streams, wal_offset)
        .await
        .with_context(|| format!("cannot write a snapshot into {}", snapshot_dir.display()))
}

/// Apply user namespace prefix to resource names
pub fn scope_resource_name(user_id: &Uuid, resource_name: &str) -> String {
    MultiTenant::scope_kv_key(Some(user_id), resource_name).into_owned()
}

/// Check if a resource name is already scoped
pub fn is_scoped(resource_name: &str) -> bool {
    resource_name.starts_with("user_") && resource_name.contains(':')
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::{self, Layout};
    use synap_server::QueueConfig;
    use synap_server::core::{Expiry, HashStore, HyperLogLogStore, SetOptions};
    use synap_server::persistence::StoreRefs;
    use synap_server::{KVConfig, KVStore, QueueManager};

    /// A snapshot the server wrote is read through the server's codec and
    /// written again in the server's format, which the server loads with every
    /// section intact.
    #[tokio::test]
    async fn server_snapshots_round_trip_through_the_shared_codec() {
        let source_dir = tempfile::tempdir().unwrap();
        let output = tempfile::tempdir().unwrap();
        let layout = Layout::of(source_dir.path());
        let kv = KVStore::new(KVConfig::default());
        let deadline = 4_102_444_800_000; // 2100-01-01
        kv.set_with_opts(
            "expiring",
            b"v".to_vec(),
            Some(Expiry::UnixMilliseconds(deadline)),
            SetOptions::default(),
        )
        .await
        .unwrap();
        kv.set("plain", b"p".to_vec(), None).await.unwrap();
        let hashes = HashStore::new();
        hashes.hset("h", "f", b"1".to_vec()).unwrap();
        let hll = HyperLogLogStore::new();
        hll.pfadd("visitors", vec![b"a".to_vec()], None).unwrap();
        let queues = QueueManager::new(QueueConfig::default());
        queues.create_queue("jobs", None).await.unwrap();
        let id = queues
            .publish("jobs", b"job".to_vec(), Some(7), None)
            .await
            .unwrap();
        let stores = StoreRefs {
            hash_store: Some(&hashes),
            hyperloglog_store: Some(&hll),
            queue_manager: Some(&queues),
            ..StoreRefs::kv_only(&kv)
        };
        manager(&layout.snapshot_dir)
            .create_snapshot(stores, 12)
            .await
            .unwrap();

        let read = source::read(&layout, QueueConfig::default()).await.unwrap();
        assert_eq!(read.snapshot.as_ref().unwrap().version, 5);
        let image = SnapshotImage::capture(read.store_refs()).await.unwrap();
        let written = write(output.path(), image, read.streams, 12).await.unwrap();
        assert_eq!(
            written.file_name().unwrap(),
            "snapshot-v5-00000000000000000001.bin"
        );

        let reloaded = manager(output.path()).load(&written).await.unwrap();
        assert_eq!(reloaded.wal_offset, 12);
        assert_eq!(reloaded.kv_data["plain"], b"p".to_vec());
        assert_eq!(reloaded.kv_expires_at_ms["expiring"], deadline);
        assert!(!reloaded.kv_expires_at_ms.contains_key("plain"));
        assert_eq!(reloaded.hash_data["h"]["f"], b"1".to_vec());
        assert!(reloaded.hyperloglog_data.contains_key("visitors"));
        let jobs = &reloaded.queue_data["jobs"];
        assert_eq!(jobs.ready.len(), 1);
        assert_eq!(jobs.ready[0].id, id);
        assert_eq!(jobs.ready[0].priority, 7);
    }
}
