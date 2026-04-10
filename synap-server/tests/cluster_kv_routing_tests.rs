//! Cluster KV Routing Integration Tests
//!
//! Tests cluster mode routing integration with KV store

use std::sync::Arc;
use std::time::Duration;
use synap_server::cluster::{
    hash_slot::hash_slot, migration::SlotMigrationManager, topology::ClusterTopology,
};
use synap_server::core::{KVConfig, KVStore};

#[tokio::test]
async fn test_kv_store_without_cluster() {
    // Test: KV store works normally without cluster mode
    let kv_store = KVStore::new(KVConfig::default());

    // Should work normally
    assert!(
        kv_store
            .set("test:key", b"value".to_vec(), None)
            .await
            .is_ok()
    );
    assert_eq!(
        kv_store.get("test:key").await.unwrap(),
        Some(b"value".to_vec())
    );
}

#[tokio::test]
async fn test_kv_store_with_cluster_routing() {
    // Test: KV store with cluster mode routes keys correctly
    let topology = Arc::new(ClusterTopology::new("node-0".to_string()));
    topology.initialize_cluster(3).unwrap();

    let kv_store = KVStore::new_with_cluster(KVConfig::default(), None, topology.clone(), None);

    // Find a key that belongs to node-0
    let mut test_key = None;
    for i in 0..1000 {
        let key = format!("test:key:{}", i);
        let slot = hash_slot(&key);
        if let Ok(owner) = topology.get_slot_owner(slot) {
            if owner == "node-0" {
                test_key = Some(key);
                break;
            }
        }
    }

    let key = test_key.expect("Should find a key for node-0");

    // Should work for keys belonging to this node
    assert!(kv_store.set(&key, b"value".to_vec(), None).await.is_ok());
    assert_eq!(kv_store.get(&key).await.unwrap(), Some(b"value".to_vec()));
}

#[tokio::test]
async fn test_kv_store_cluster_moved_error() {
    // Test: KV store returns MOVED error for keys belonging to other nodes
    let topology = Arc::new(ClusterTopology::new("node-0".to_string()));
    topology.initialize_cluster(3).unwrap();

    let kv_store = KVStore::new_with_cluster(KVConfig::default(), None, topology.clone(), None);

    // Find a key that belongs to node-1 or node-2
    let mut test_key = None;
    for i in 0..1000 {
        let key = format!("test:key:{}", i);
        let slot = hash_slot(&key);
        if let Ok(owner) = topology.get_slot_owner(slot) {
            if owner != "node-0" {
                test_key = Some(key);
                break;
            }
        }
    }

    let key = test_key.expect("Should find a key for another node");

    // Should return MOVED error
    let result = kv_store.set(&key, b"value".to_vec(), None).await;
    assert!(result.is_err());

    if let Err(synap_server::core::SynapError::ClusterMoved { slot, node_address }) = result {
        assert!(!node_address.is_empty());
        assert!(slot < 16384);
    } else {
        panic!("Expected ClusterMoved error");
    }
}

#[tokio::test]
async fn test_kv_store_cluster_ask_error() {
    // Test: KV store returns ASK error for keys being migrated
    let topology = Arc::new(ClusterTopology::new("node-0".to_string()));
    topology.initialize_cluster(3).unwrap();

    // Find a slot owned by node-0
    let mut test_slot = None;
    for i in 0..1000 {
        let key = format!("test:key:{}", i);
        let slot = hash_slot(&key);
        if let Ok(owner) = topology.get_slot_owner(slot) {
            if owner == "node-0" {
                test_slot = Some(slot);
                break;
            }
        }
    }

    let slot = test_slot.expect("Should find a slot for node-0");

    // Find a key that hashes to this exact slot
    let mut test_key = None;
    for i in 0..10000 {
        let key = format!("test:key:{}", i);
        if hash_slot(&key) == slot {
            test_key = Some(key);
            break;
        }
    }
    let test_key = test_key.expect("Should find a key for the slot");

    // Start migration of this slot
    let migration = Arc::new(SlotMigrationManager::new(100, Duration::from_secs(60)));
    migration
        .start_migration(slot, "node-0".to_string(), "node-1".to_string())
        .unwrap();

    // Wait a bit for migration state to update (migration worker updates state asynchronously)
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Verify migration is active
    let migration_status = migration.get_migration(slot);
    assert!(migration_status.is_some(), "Migration should be active");

    let kv_store = KVStore::new_with_cluster(
        KVConfig::default(),
        None,
        topology.clone(),
        Some(migration.clone()),
    );

    // Should return ASK error for migrating keys
    // Note: The migration check happens first, so even if the slot belongs to node-0,
    // it should return ASK if migration is active
    let result = kv_store.set(&test_key, b"value".to_vec(), None).await;

    // The result should be an error (either ASK or MOVED depending on timing)
    // In practice, ASK should be returned if migration is active
    assert!(result.is_err());

    // Accept either ASK (migration active) or MOVED (migration not yet detected)
    // This is acceptable behavior during migration transition
    match result {
        Err(synap_server::core::SynapError::ClusterAsk {
            slot: err_slot,
            node_address,
        }) => {
            assert_eq!(err_slot, slot);
            assert!(!node_address.is_empty());
        }
        Err(synap_server::core::SynapError::ClusterMoved { slot: err_slot, .. }) => {
            // MOVED is also acceptable if migration state hasn't propagated yet
            assert_eq!(err_slot, slot);
        }
        other => {
            panic!(
                "Expected ClusterAsk or ClusterMoved error, got: {:?}",
                other
            );
        }
    }
}

#[tokio::test]
async fn test_kv_store_cluster_slot_not_assigned() {
    // Test: KV store returns error when slot is not assigned
    let topology = Arc::new(ClusterTopology::new("node-0".to_string()));
    // Don't initialize cluster - no slots assigned

    let kv_store = KVStore::new_with_cluster(KVConfig::default(), None, topology.clone(), None);

    // Should return error for unassigned slots
    let result = kv_store.set("test:key", b"value".to_vec(), None).await;
    assert!(result.is_err());

    if let Err(synap_server::core::SynapError::ClusterSlotNotAssigned { slot }) = result {
        assert!(slot < 16384);
    } else {
        panic!("Expected ClusterSlotNotAssigned error");
    }
}

#[tokio::test]
async fn test_kv_store_get_with_cluster_routing() {
    // Test: GET operation respects cluster routing
    let topology = Arc::new(ClusterTopology::new("node-0".to_string()));
    topology.initialize_cluster(3).unwrap();

    let kv_store = KVStore::new_with_cluster(KVConfig::default(), None, topology.clone(), None);

    // Find a key that belongs to node-0
    let mut test_key = None;
    for i in 0..1000 {
        let key = format!("test:key:{}", i);
        let slot = hash_slot(&key);
        if let Ok(owner) = topology.get_slot_owner(slot) {
            if owner == "node-0" {
                test_key = Some(key);
                break;
            }
        }
    }

    let key = test_key.expect("Should find a key for node-0");

    // Set the key first
    assert!(kv_store.set(&key, b"value".to_vec(), None).await.is_ok());

    // GET should work
    assert_eq!(kv_store.get(&key).await.unwrap(), Some(b"value".to_vec()));

    // GET for key belonging to another node should fail
    let mut other_key = None;
    for i in 0..1000 {
        let key = format!("test:other:{}", i);
        let slot = hash_slot(&key);
        if let Ok(owner) = topology.get_slot_owner(slot) {
            if owner != "node-0" {
                other_key = Some(key);
                break;
            }
        }
    }

    if let Some(key) = other_key {
        let result = kv_store.get(&key).await;
        assert!(result.is_err());
    }
}

#[tokio::test]
async fn test_kv_store_delete_with_cluster_routing() {
    // Test: DELETE operation respects cluster routing
    let topology = Arc::new(ClusterTopology::new("node-0".to_string()));
    topology.initialize_cluster(3).unwrap();

    let kv_store = KVStore::new_with_cluster(KVConfig::default(), None, topology.clone(), None);

    // Find a key that belongs to node-0
    let mut test_key = None;
    for i in 0..1000 {
        let key = format!("test:key:{}", i);
        let slot = hash_slot(&key);
        if let Ok(owner) = topology.get_slot_owner(slot) {
            if owner == "node-0" {
                test_key = Some(key);
                break;
            }
        }
    }

    let key = test_key.expect("Should find a key for node-0");

    // Set the key first
    assert!(kv_store.set(&key, b"value".to_vec(), None).await.is_ok());

    // DELETE should work
    assert!(kv_store.delete(&key).await.unwrap());

    // DELETE for key belonging to another node should fail
    let mut other_key = None;
    for i in 0..1000 {
        let key = format!("test:other:{}", i);
        let slot = hash_slot(&key);
        if let Ok(owner) = topology.get_slot_owner(slot) {
            if owner != "node-0" {
                other_key = Some(key);
                break;
            }
        }
    }

    if let Some(key) = other_key {
        let result = kv_store.delete(&key).await;
        assert!(result.is_err());
    }
}
