//! Synchronization utilities for replication
//!
//! This module provides helpers for:
//! - Snapshot creation and transfer
//! - Incremental sync
//! - Checksum verification

use crate::core::{KVStore, StreamManager};
use crate::persistence::types::Operation;
use serde::{Deserialize, Serialize};
use tracing::{debug, info};

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

/// Create a snapshot of KV store and streams for full sync
pub async fn create_snapshot(kv_store: &KVStore, offset: u64) -> Result<Vec<u8>, String> {
    create_snapshot_with_streams(kv_store, None, offset).await
}

/// Create a snapshot with optional stream manager
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

/// Apply snapshot with optional stream manager
pub async fn apply_snapshot_with_streams(
    kv_store: &KVStore,
    stream_manager: Option<&StreamManager>,
    snapshot: &[u8],
) -> Result<u64, String> {
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
        "Applying snapshot: {} operations ({} keys, {} streams), offset: {}",
        operations.len(),
        metadata.total_keys,
        metadata.total_streams,
        metadata.offset
    );

    // Apply operations
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

    info!("Snapshot applied successfully");
    Ok(metadata.offset)
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
