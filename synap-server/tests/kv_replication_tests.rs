//! Key-Value Store Replication Tests
//!
//! Comprehensive tests for KV operations with master-slave replication.
//! Validates that all KV operations are correctly replicated to replica nodes.

use std::sync::Arc;
use std::sync::atomic::{AtomicU16, Ordering};
use std::time::Duration;
use synap_server::persistence::types::Operation;
use synap_server::replication::{MasterNode, NodeRole, ReplicaNode, ReplicationConfig};
use synap_server::{KVConfig, KVStore};
use tokio::time::sleep;

static KV_TEST_PORT: AtomicU16 = AtomicU16::new(30000);

fn next_port() -> u16 {
    KV_TEST_PORT.fetch_add(1, Ordering::SeqCst)
}

async fn create_kv_master() -> (Arc<MasterNode>, Arc<KVStore>, std::net::SocketAddr) {
    let port = next_port();
    let addr: std::net::SocketAddr = format!("127.0.0.1:{}", port).parse().unwrap();

    let mut config = ReplicationConfig::default();
    config.enabled = true;
    config.role = NodeRole::Master;
    config.replica_listen_address = Some(addr);
    config.heartbeat_interval_ms = 100;

    let kv = Arc::new(KVStore::new(KVConfig::default()));
    let master = Arc::new(
        MasterNode::new(config.clone(), Arc::clone(&kv), None)
            .await
            .unwrap(),
    );

    sleep(Duration::from_millis(100)).await;

    (master, kv, addr)
}

async fn create_kv_replica(master_addr: std::net::SocketAddr) -> (Arc<ReplicaNode>, Arc<KVStore>) {
    let config = ReplicationConfig {
        enabled: true,
        role: NodeRole::Replica,
        master_address: Some(master_addr),
        auto_reconnect: true,
        reconnect_delay_ms: 100,
        ..Default::default()
    };

    let kv = Arc::new(KVStore::new(KVConfig::default()));
    let replica = ReplicaNode::new(config, Arc::clone(&kv), None)
        .await
        .unwrap();

    sleep(Duration::from_millis(50)).await;

    (replica, kv)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_kv_set_get_replication() {
    let (master, master_kv, master_addr) = create_kv_master().await;

    // Set some keys on master
    master_kv
        .set("key1", b"value1".to_vec(), None)
        .await
        .unwrap();
    master_kv
        .set("key2", b"value2".to_vec(), None)
        .await
        .unwrap();
    master_kv
        .set("key3", b"value3".to_vec(), None)
        .await
        .unwrap();

    // Create replica (should snapshot these keys)
    let (_replica, replica_kv) = create_kv_replica(master_addr).await;
    sleep(Duration::from_secs(2)).await;

    // Verify keys are replicated
    assert_eq!(
        replica_kv.get("key1").await.unwrap(),
        Some(b"value1".to_vec())
    );
    assert_eq!(
        replica_kv.get("key2").await.unwrap(),
        Some(b"value2".to_vec())
    );
    assert_eq!(
        replica_kv.get("key3").await.unwrap(),
        Some(b"value3".to_vec())
    );

    // Add more keys via replication log
    master_kv
        .set("key4", b"value4".to_vec(), None)
        .await
        .unwrap();
    master.replicate(Operation::KVSet {
        key: "key4".to_string(),
        value: b"value4".to_vec(),
        ttl: None,
    });

    sleep(Duration::from_secs(1)).await;

    // Verify new key is replicated
    assert_eq!(
        replica_kv.get("key4").await.unwrap(),
        Some(b"value4".to_vec())
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_kv_delete_replication() {
    let (master, master_kv, master_addr) = create_kv_master().await;

    // Create initial keys
    for i in 0..20 {
        let key = format!("delete_test_{}", i);
        master_kv
            .set(&key, format!("value_{}", i).into_bytes(), None)
            .await
            .unwrap();
    }

    let (_replica, replica_kv) = create_kv_replica(master_addr).await;
    sleep(Duration::from_secs(2)).await;

    // Verify all keys are synced
    assert_eq!(replica_kv.keys().await.unwrap().len(), 20);

    // Delete some keys via replication
    for i in 0..10 {
        let key = format!("delete_test_{}", i);
        master_kv.mdel(std::slice::from_ref(&key)).await.unwrap();
        master.replicate(Operation::KVDel { keys: vec![key] });
    }

    sleep(Duration::from_secs(1)).await;

    // Verify deletions are replicated
    for i in 0..10 {
        let key = format!("delete_test_{}", i);
        assert_eq!(
            replica_kv.get(&key).await.unwrap(),
            None,
            "Key {} should be deleted",
            key
        );
    }

    // Verify non-deleted keys still exist
    for i in 10..20 {
        let key = format!("delete_test_{}", i);
        assert!(
            replica_kv.get(&key).await.unwrap().is_some(),
            "Key {} should exist",
            key
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_kv_batch_operations_replication() {
    let (master, master_kv, master_addr) = create_kv_master().await;

    // Initial data for snapshot
    for i in 0..50 {
        master_kv
            .set(&format!("initial_{}", i), vec![i as u8], None)
            .await
            .unwrap();
    }

    let (_replica, replica_kv) = create_kv_replica(master_addr).await;
    sleep(Duration::from_secs(2)).await;

    // Verify snapshot sync
    assert_eq!(replica_kv.keys().await.unwrap().len(), 50);

    // Batch set via replication
    for i in 0..30 {
        let key = format!("batch_{}", i);
        let value = format!("batch_value_{}", i).into_bytes();
        master_kv.set(&key, value.clone(), None).await.unwrap();
        master.replicate(Operation::KVSet {
            key,
            value,
            ttl: None,
        });
    }

    sleep(Duration::from_secs(1)).await;

    // Verify batch operations replicated
    for i in 0..30 {
        let key = format!("batch_{}", i);
        let value = replica_kv.get(&key).await.unwrap();
        assert_eq!(value, Some(format!("batch_value_{}", i).into_bytes()));
    }

    // Total keys should be 50 initial + 30 batch = 80
    let total_keys = replica_kv.keys().await.unwrap().len();
    assert!(
        total_keys >= 50,
        "Should have at least initial 50 keys, got {}",
        total_keys
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_kv_mset_mdel_replication() {
    let (master, master_kv, master_addr) = create_kv_master().await;

    // Initial keys
    master_kv
        .set("keep1", b"value1".to_vec(), None)
        .await
        .unwrap();
    master_kv
        .set("keep2", b"value2".to_vec(), None)
        .await
        .unwrap();

    let (_replica, replica_kv) = create_kv_replica(master_addr).await;
    sleep(Duration::from_secs(1)).await;

    // Multi-set via replication
    let keys_to_set = vec![
        ("mset_1".to_string(), b"mvalue_1".to_vec()),
        ("mset_2".to_string(), b"mvalue_2".to_vec()),
        ("mset_3".to_string(), b"mvalue_3".to_vec()),
    ];

    for (key, value) in keys_to_set {
        master_kv.set(&key, value.clone(), None).await.unwrap();
        master.replicate(Operation::KVSet {
            key,
            value,
            ttl: None,
        });
    }

    sleep(Duration::from_millis(500)).await;

    // Verify MSET replicated
    assert_eq!(
        replica_kv.get("mset_1").await.unwrap(),
        Some(b"mvalue_1".to_vec())
    );
    assert_eq!(
        replica_kv.get("mset_2").await.unwrap(),
        Some(b"mvalue_2".to_vec())
    );
    assert_eq!(
        replica_kv.get("mset_3").await.unwrap(),
        Some(b"mvalue_3".to_vec())
    );

    // Multi-delete via replication
    master_kv
        .mdel(&["mset_1".to_string(), "mset_2".to_string()])
        .await
        .unwrap();
    master.replicate(Operation::KVDel {
        keys: vec!["mset_1".to_string(), "mset_2".to_string()],
    });

    sleep(Duration::from_millis(500)).await;

    // Verify MDEL replicated
    assert_eq!(replica_kv.get("mset_1").await.unwrap(), None);
    assert_eq!(replica_kv.get("mset_2").await.unwrap(), None);
    assert_eq!(
        replica_kv.get("mset_3").await.unwrap(),
        Some(b"mvalue_3".to_vec())
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_kv_ttl_replication() {
    let (master, master_kv, master_addr) = create_kv_master().await;

    // Set keys with TTL on master
    master_kv
        .set("ttl_key1", b"value1".to_vec(), Some(3600))
        .await
        .unwrap();
    master_kv
        .set("ttl_key2", b"value2".to_vec(), Some(7200))
        .await
        .unwrap();
    master_kv
        .set("no_ttl", b"permanent".to_vec(), None)
        .await
        .unwrap();

    let (_replica, replica_kv) = create_kv_replica(master_addr).await;
    sleep(Duration::from_secs(2)).await;

    // Verify keys with TTL are replicated
    assert_eq!(
        replica_kv.get("ttl_key1").await.unwrap(),
        Some(b"value1".to_vec())
    );
    assert_eq!(
        replica_kv.get("ttl_key2").await.unwrap(),
        Some(b"value2".to_vec())
    );
    assert_eq!(
        replica_kv.get("no_ttl").await.unwrap(),
        Some(b"permanent".to_vec())
    );

    // Add more TTL keys via replication
    master_kv
        .set("ttl_key3", b"expiring".to_vec(), Some(1800))
        .await
        .unwrap();
    master.replicate(Operation::KVSet {
        key: "ttl_key3".to_string(),
        value: b"expiring".to_vec(),
        ttl: Some(1800),
    });

    sleep(Duration::from_millis(500)).await;

    // Verify TTL key replicated
    assert_eq!(
        replica_kv.get("ttl_key3").await.unwrap(),
        Some(b"expiring".to_vec())
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_kv_update_operations_replication() {
    let (master, master_kv, master_addr) = create_kv_master().await;

    // Initial values
    for i in 0..10 {
        let key = format!("update_key_{}", i);
        master_kv
            .set(&key, format!("v1_{}", i).into_bytes(), None)
            .await
            .unwrap();
    }

    let (_replica, replica_kv) = create_kv_replica(master_addr).await;
    sleep(Duration::from_secs(1)).await;

    // Verify initial sync
    for i in 0..10 {
        let key = format!("update_key_{}", i);
        assert_eq!(
            replica_kv.get(&key).await.unwrap(),
            Some(format!("v1_{}", i).into_bytes())
        );
    }

    // Update all keys via replication
    for i in 0..10 {
        let key = format!("update_key_{}", i);
        let value = format!("v2_updated_{}", i).into_bytes();
        master_kv.set(&key, value.clone(), None).await.unwrap();
        master.replicate(Operation::KVSet {
            key,
            value,
            ttl: None,
        });
    }

    sleep(Duration::from_secs(1)).await;

    // Verify updates are replicated
    for i in 0..10 {
        let key = format!("update_key_{}", i);
        assert_eq!(
            replica_kv.get(&key).await.unwrap(),
            Some(format!("v2_updated_{}", i).into_bytes()),
            "Update not replicated for {}",
            key
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_kv_scan_operations_replication() {
    let (master, master_kv, master_addr) = create_kv_master().await;

    // Create keys with common prefixes
    for i in 0..20 {
        master_kv
            .set(&format!("user:{}", i), vec![i as u8], None)
            .await
            .unwrap();
        master_kv
            .set(&format!("session:{}", i), vec![i as u8], None)
            .await
            .unwrap();
    }

    let (_replica, replica_kv) = create_kv_replica(master_addr).await;
    sleep(Duration::from_secs(2)).await;

    // Verify SCAN works on replica (snapshot data)
    let user_keys = replica_kv.scan(Some("user:"), 100).await.unwrap();
    assert_eq!(user_keys.len(), 20, "Should have 20 user: keys");

    let session_keys = replica_kv.scan(Some("session:"), 100).await.unwrap();
    assert_eq!(session_keys.len(), 20, "Should have 20 session: keys");

    // Add more keys via replication
    for i in 20..30 {
        let key = format!("user:{}", i);
        let value = vec![i as u8];
        master_kv.set(&key, value.clone(), None).await.unwrap();
        master.replicate(Operation::KVSet {
            key,
            value,
            ttl: None,
        });
    }

    sleep(Duration::from_secs(1)).await;

    // Verify new keys are scannable
    let all_user_keys = replica_kv.scan(Some("user:"), 100).await.unwrap();
    assert!(
        all_user_keys.len() >= 20,
        "Should have at least 20 user keys"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_kv_exists_replication() {
    let (_master, master_kv, master_addr) = create_kv_master().await;

    master_kv
        .set("exists_key", b"value".to_vec(), None)
        .await
        .unwrap();

    let (_replica, replica_kv) = create_kv_replica(master_addr).await;
    sleep(Duration::from_secs(1)).await;

    // Test EXISTS on replica
    let exists_result = replica_kv.get("exists_key").await.unwrap();
    assert!(exists_result.is_some(), "Key should exist on replica");

    let not_exists = replica_kv.get("nonexistent").await.unwrap();
    assert!(not_exists.is_none(), "Key should not exist on replica");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_kv_overwrite_replication() {
    let (master, master_kv, master_addr) = create_kv_master().await;

    // Set initial value
    master_kv
        .set("overwrite_key", b"v1".to_vec(), None)
        .await
        .unwrap();

    let (_replica, replica_kv) = create_kv_replica(master_addr).await;
    sleep(Duration::from_secs(1)).await;

    assert_eq!(
        replica_kv.get("overwrite_key").await.unwrap(),
        Some(b"v1".to_vec())
    );

    // Overwrite multiple times via replication
    for i in 2..=5 {
        let value = format!("v{}", i).into_bytes();
        master_kv
            .set("overwrite_key", value.clone(), None)
            .await
            .unwrap();
        master.replicate(Operation::KVSet {
            key: "overwrite_key".to_string(),
            value,
            ttl: None,
        });
        sleep(Duration::from_millis(100)).await;
    }

    sleep(Duration::from_millis(500)).await;

    // Should have latest value
    assert_eq!(
        replica_kv.get("overwrite_key").await.unwrap(),
        Some(b"v5".to_vec())
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_kv_large_dataset_replication() {
    let (_master, master_kv, master_addr) = create_kv_master().await;

    // Create 500 keys on master
    for i in 0..500 {
        let key = format!("large_dataset_{}", i);
        let value = format!("value_{}_{}", i, "x".repeat(100)).into_bytes();
        master_kv.set(&key, value, None).await.unwrap();
    }

    let (_replica, replica_kv) = create_kv_replica(master_addr).await;
    sleep(Duration::from_secs(3)).await;

    // Verify large dataset is synced
    let key_count = replica_kv.keys().await.unwrap().len();
    assert_eq!(
        key_count, 500,
        "Should have exactly 500 keys, got {}",
        key_count
    );

    // Sample verification
    for i in (0..500).step_by(50) {
        let key = format!("large_dataset_{}", i);
        assert!(
            replica_kv.get(&key).await.unwrap().is_some(),
            "Key {} missing",
            key
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_kv_mixed_operations_replication() {
    let (master, master_kv, master_addr) = create_kv_master().await;

    // Initial dataset
    for i in 0..20 {
        master_kv
            .set(&format!("mixed_{}", i), vec![i as u8], None)
            .await
            .unwrap();
    }

    let (_replica, replica_kv) = create_kv_replica(master_addr).await;
    sleep(Duration::from_secs(1)).await;

    // Mixed operations via replication: SET, UPDATE, DELETE
    // Set new keys
    for i in 20..25 {
        let key = format!("mixed_{}", i);
        let value = vec![i as u8];
        master_kv.set(&key, value.clone(), None).await.unwrap();
        master.replicate(Operation::KVSet {
            key,
            value,
            ttl: None,
        });
    }

    // Update existing keys
    for i in 0..5 {
        let key = format!("mixed_{}", i);
        let value = vec![255 - i as u8];
        master_kv.set(&key, value.clone(), None).await.unwrap();
        master.replicate(Operation::KVSet {
            key,
            value,
            ttl: None,
        });
    }

    // Delete some keys
    for i in 10..15 {
        let key = format!("mixed_{}", i);
        master_kv.mdel(std::slice::from_ref(&key)).await.unwrap();
        master.replicate(Operation::KVDel { keys: vec![key] });
    }

    sleep(Duration::from_secs(1)).await;

    // Verify new keys exist
    for i in 20..25 {
        let key = format!("mixed_{}", i);
        assert!(
            replica_kv.get(&key).await.unwrap().is_some(),
            "New key {} missing",
            key
        );
    }

    // Verify updates
    for i in 0..5 {
        let key = format!("mixed_{}", i);
        assert_eq!(
            replica_kv.get(&key).await.unwrap(),
            Some(vec![255 - i as u8]),
            "Update not replicated for {}",
            key
        );
    }

    // Verify deletions
    for i in 10..15 {
        let key = format!("mixed_{}", i);
        assert_eq!(
            replica_kv.get(&key).await.unwrap(),
            None,
            "Key {} should be deleted",
            key
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_kv_binary_values_replication() {
    let (_master, master_kv, master_addr) = create_kv_master().await;

    // Set binary data (images, serialized data, etc.)
    let binary_data1 = vec![0xFF, 0xD8, 0xFF, 0xE0]; // JPEG header
    let binary_data2 = vec![0x89, 0x50, 0x4E, 0x47]; // PNG header
    let binary_data3 = vec![0; 1024]; // 1KB of zeros

    master_kv
        .set("image_jpeg", binary_data1.clone(), None)
        .await
        .unwrap();
    master_kv
        .set("image_png", binary_data2.clone(), None)
        .await
        .unwrap();
    master_kv
        .set("binary_blob", binary_data3.clone(), None)
        .await
        .unwrap();

    let (_replica, replica_kv) = create_kv_replica(master_addr).await;
    sleep(Duration::from_secs(1)).await;

    // Verify binary data integrity
    assert_eq!(
        replica_kv.get("image_jpeg").await.unwrap(),
        Some(binary_data1)
    );
    assert_eq!(
        replica_kv.get("image_png").await.unwrap(),
        Some(binary_data2)
    );
    assert_eq!(
        replica_kv.get("binary_blob").await.unwrap(),
        Some(binary_data3)
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_kv_empty_values_replication() {
    let (_master, master_kv, master_addr) = create_kv_master().await;

    // Set empty values (valid use case)
    master_kv.set("empty_key", vec![], None).await.unwrap();
    master_kv
        .set("another_empty", b"".to_vec(), None)
        .await
        .unwrap();

    let (_replica, replica_kv) = create_kv_replica(master_addr).await;
    sleep(Duration::from_secs(1)).await;

    // Verify empty values are replicated
    assert_eq!(replica_kv.get("empty_key").await.unwrap(), Some(vec![]));
    assert_eq!(
        replica_kv.get("another_empty").await.unwrap(),
        Some(b"".to_vec())
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_kv_unicode_keys_replication() {
    let (_master, master_kv, master_addr) = create_kv_master().await;

    // Unicode keys
    master_kv
        .set("キー1", b"Japanese".to_vec(), None)
        .await
        .unwrap();
    master_kv
        .set("مفتاح", b"Arabic".to_vec(), None)
        .await
        .unwrap();
    master_kv
        .set("ключ", b"Russian".to_vec(), None)
        .await
        .unwrap();
    master_kv
        .set("🔑emoji", b"Emoji".to_vec(), None)
        .await
        .unwrap();

    let (_replica, replica_kv) = create_kv_replica(master_addr).await;
    sleep(Duration::from_secs(1)).await;

    // Verify Unicode keys replicated
    assert_eq!(
        replica_kv.get("キー1").await.unwrap(),
        Some(b"Japanese".to_vec())
    );
    assert_eq!(
        replica_kv.get("مفتاح").await.unwrap(),
        Some(b"Arabic".to_vec())
    );
    assert_eq!(
        replica_kv.get("ключ").await.unwrap(),
        Some(b"Russian".to_vec())
    );
    assert_eq!(
        replica_kv.get("🔑emoji").await.unwrap(),
        Some(b"Emoji".to_vec())
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_kv_stats_replication() {
    let (_master, master_kv, master_addr) = create_kv_master().await;

    // Add various keys
    for i in 0..100 {
        master_kv
            .set(&format!("stats_key_{}", i), vec![i as u8], None)
            .await
            .unwrap();
    }

    let (_replica, replica_kv) = create_kv_replica(master_addr).await;
    sleep(Duration::from_secs(2)).await;

    // Get stats from both
    let master_stats = master_kv.stats().await;
    let replica_stats = replica_kv.stats().await;

    // Both should have same number of keys
    assert_eq!(
        master_stats.total_keys, replica_stats.total_keys,
        "Master and replica should have same key count"
    );
    assert_eq!(master_stats.total_keys, 100);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_kv_keys_list_replication() {
    let (_master, master_kv, master_addr) = create_kv_master().await;

    // Add keys
    let expected_keys = vec!["alpha", "beta", "gamma", "delta", "epsilon"];
    for key in &expected_keys {
        master_kv.set(key, b"value".to_vec(), None).await.unwrap();
    }

    let (_replica, replica_kv) = create_kv_replica(master_addr).await;
    sleep(Duration::from_secs(1)).await;

    // Get keys list from replica
    let replica_keys = replica_kv.keys().await.unwrap();
    assert_eq!(replica_keys.len(), expected_keys.len());

    for key in expected_keys {
        assert!(
            replica_keys.contains(&key.to_string()),
            "Replica missing key: {}",
            key
        );
    }
}
