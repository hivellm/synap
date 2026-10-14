//! Snapshot access through the server's own codec.
//!
//! Every byte the migrator reads or writes goes through
//! `synap_server::persistence`: [`SnapshotManager`] finds snapshot files by the
//! server's naming rules, decodes every format the server reads and writes the
//! format it writes. There is no second implementation of the format here.

use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use synap_server::QueueConfig;
use synap_server::hub::MultiTenant;
use synap_server::persistence::types::SnapshotConfig;
use synap_server::persistence::{Snapshot, SnapshotImage, SnapshotManager};
use uuid::Uuid;

/// The snapshot manager of the data directory `data_dir`, whose snapshots
/// live in `<data_dir>/snapshots` (the server's default layout).
pub fn manager(data_dir: &Path) -> SnapshotManager {
    SnapshotManager::new(SnapshotConfig {
        directory: data_dir.join("snapshots"),
        ..SnapshotConfig::default()
    })
}

/// Load the newest snapshot of `data_dir`, in any format the server reads.
pub async fn load_latest(data_dir: &Path) -> Result<Option<(Snapshot, PathBuf)>> {
    manager(data_dir)
        .load_latest()
        .await
        .context("Failed to load the latest snapshot")
}

/// Write `snapshot` into `data_dir` in the format and under the name the
/// server writes, keeping its WAL cutoff. Queues the snapshot stores without
/// a configuration (v4 and older) get the server's default.
pub async fn write(data_dir: &Path, snapshot: Snapshot) -> Result<PathBuf> {
    let Snapshot {
        wal_offset,
        kv_data,
        kv_expires_at_ms,
        queue_data,
        stream_data,
        list_data,
        set_data,
        sorted_set_data,
        hash_data,
        hyperloglog_data,
        bitmap_data,
        ..
    } = snapshot;
    let kv = kv_data
        .into_iter()
        .map(|(key, value)| {
            let expires_at_ms = kv_expires_at_ms.get(&key).copied();
            (key, Arc::<[u8]>::from(value), expires_at_ms)
        })
        .collect();
    let queues = queue_data
        .into_iter()
        .map(|(name, image)| (name, image.into_state(QueueConfig::default())))
        .collect::<HashMap<_, _>>();
    let image = SnapshotImage {
        kv,
        queues,
        hashes: hash_data,
        lists: list_data,
        sets: set_data,
        sorted_sets: sorted_set_data,
        hyperloglogs: hyperloglog_data,
        bitmaps: bitmap_data,
    };
    manager(data_dir)
        .write_snapshot(image, stream_data, wal_offset)
        .await
        .context("Failed to write the snapshot")
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
    use synap_server::core::{Expiry, HashStore, HyperLogLogStore, SetOptions};
    use synap_server::persistence::StoreRefs;
    use synap_server::{KVConfig, KVStore, QueueManager};

    /// A snapshot the server wrote reads back through this module and is
    /// written again in the server's format, which the server loads with every
    /// section intact.
    #[tokio::test]
    async fn server_snapshots_round_trip_through_the_shared_codec() {
        let source = tempfile::tempdir().unwrap();
        let output = tempfile::tempdir().unwrap();
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
        manager(source.path())
            .create_snapshot(stores, 12)
            .await
            .unwrap();

        let (snapshot, _) = load_latest(source.path()).await.unwrap().unwrap();
        let written = write(output.path(), snapshot).await.unwrap();
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
