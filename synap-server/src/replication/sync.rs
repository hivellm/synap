//! Synchronization utilities for replication
//!
//! This module provides helpers for:
//! - Snapshot creation and transfer
//! - Incremental sync
//! - Checksum verification

use crate::core::KVStore;
use crate::persistence::types::Operation;
use serde::{Deserialize, Serialize};
use tracing::{debug, info};

/// Snapshot metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotMetadata {
    pub offset: u64,
    pub timestamp: u64,
    pub total_keys: usize,
    pub compressed: bool,
    pub checksum: u32,
}

/// Create a snapshot of KV store for full sync
pub async fn create_snapshot(kv_store: &KVStore, offset: u64) -> Result<Vec<u8>, String> {
    info!("Creating snapshot at offset {}", offset);

    // Get all keys
    let keys = kv_store.keys().await.map_err(|e| e.to_string())?;
    let total_keys = keys.len();

    // Serialize all key-value pairs
    let mut operations = Vec::new();
    for key in keys {
        if let Ok(Some(value)) = kv_store.get(&key).await {
            operations.push(Operation::KVSet {
                key: key.clone(),
                value,
                ttl: None, // TODO: Include TTL
            });
        }
    }

    // Serialize operations
    let data = bincode::serialize(&operations).map_err(|e| e.to_string())?;

    // Calculate checksum
    let checksum = crc32fast::hash(&data);

    // Create metadata
    let metadata = SnapshotMetadata {
        offset,
        timestamp: current_timestamp(),
        total_keys,
        compressed: false,
        checksum,
    };

    info!(
        "Snapshot created: {} keys, {} bytes, checksum: {}",
        total_keys,
        data.len(),
        checksum
    );

    // Combine metadata + data
    let mut result = bincode::serialize(&metadata).map_err(|e| e.to_string())?;
    result.extend_from_slice(&data);

    Ok(result)
}

/// Apply snapshot to KV store
pub async fn apply_snapshot(kv_store: &KVStore, snapshot: &[u8]) -> Result<u64, String> {
    // Deserialize metadata
    let metadata: SnapshotMetadata = bincode::deserialize(snapshot).map_err(|e| e.to_string())?;

    let metadata_size = bincode::serialized_size(&metadata).map_err(|e| e.to_string())? as usize;
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
    let operations: Vec<Operation> = bincode::deserialize(data).map_err(|e| e.to_string())?;

    info!(
        "Applying snapshot: {} operations, offset: {}",
        operations.len(),
        metadata.offset
    );

    // Apply operations
    for op in operations {
        match op {
            Operation::KVSet { key, value, ttl } => {
                let _ = kv_store.set(&key, value, ttl).await;
            }
            _ => {
                debug!("Skipping non-SET operation in snapshot");
            }
        }
    }

    info!("Snapshot applied successfully");
    Ok(metadata.offset)
}

fn current_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
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
}
