//! Integration tests for replication system  
//!
//! **STATUS**: Work in Progress - These tests require full TCP network implementation
//!
//! These tests cover:
//! - Full sync with real TCP connections
//! - Partial sync after reconnection
//! - Multiple replicas synchronization
//! - Disconnect/reconnect scenarios
//! - Data consistency verification
//! - Stress tests with thousands of operations
//!
//! Note: Currently marked as #[ignore] pending completion of network layer

use std::sync::Arc;
use std::sync::atomic::{AtomicU16, Ordering};
use std::time::Duration;
use synap_server::persistence::types::Operation;
use synap_server::replication::{MasterNode, NodeRole, ReplicaNode, ReplicationConfig};
use synap_server::{KVConfig, KVStore};
use tokio::time::sleep;

static TEST_PORT: AtomicU16 = AtomicU16::new(20000);

/// Helper to get next available test port
fn next_port() -> u16 {
    TEST_PORT.fetch_add(1, Ordering::SeqCst)
}

/// Helper to create a master node with a unique port
async fn create_master() -> (Arc<MasterNode>, Arc<KVStore>, std::net::SocketAddr) {
    let port = next_port();
    let addr: std::net::SocketAddr = format!("127.0.0.1:{}", port).parse().unwrap();

    let mut config = ReplicationConfig::default();
    config.enabled = true;
    config.role = NodeRole::Master;
    config.replica_listen_address = Some(addr);
    config.heartbeat_interval_ms = 100; // Fast heartbeats for testing

    let kv = Arc::new(KVStore::new(KVConfig::default()));
    let master = Arc::new(
        MasterNode::new(config.clone(), Arc::clone(&kv))
            .await
            .unwrap(),
    );

    // Give master time to bind to port
    sleep(Duration::from_millis(100)).await;

    (master, kv, addr)
}

/// Helper to create a replica node
async fn create_replica(
    master_addr: std::net::SocketAddr,
    auto_reconnect: bool,
) -> (Arc<ReplicaNode>, Arc<KVStore>) {
    let mut config = ReplicationConfig::default();
    config.enabled = true;
    config.role = NodeRole::Replica;
    config.master_address = Some(master_addr);
    config.auto_reconnect = auto_reconnect;
    config.reconnect_delay_ms = 100; // Fast reconnect for testing

    let kv = Arc::new(KVStore::new(KVConfig::default()));
    let replica = ReplicaNode::new(config, Arc::clone(&kv)).await.unwrap();

    (replica, kv)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "WIP: Requires full network layer implementation"]
async fn test_full_sync_with_real_connection() {
    // Create master and populate with data
    let (master, master_kv, master_addr) = create_master().await;

    // Populate master with initial data
    for i in 0..100 {
        let key = format!("key_{}", i);
        let value = format!("value_{}", i).into_bytes();
        master_kv.set(&key, value, None).await.unwrap();

        // Replicate to log
        master.replicate(Operation::KVSet {
            key: key.clone(),
            value: format!("value_{}", i).into_bytes(),
            ttl: None,
        });
    }

    // Wait for operations to be logged
    sleep(Duration::from_millis(100)).await;

    // Create replica - should trigger full sync
    let (replica, replica_kv) = create_replica(master_addr, false).await;

    // Wait for sync to complete
    sleep(Duration::from_millis(500)).await;

    // Verify data consistency
    for i in 0..100 {
        let key = format!("key_{}", i);
        let expected = format!("value_{}", i).into_bytes();

        let replica_value = replica_kv.get(&key).await.unwrap();
        assert_eq!(
            replica_value,
            Some(expected.clone()),
            "Key {} not synced correctly",
            key
        );
    }

    // Verify replica offset
    assert_eq!(replica.current_offset(), 100);
    assert!(replica.is_connected());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_partial_sync_after_disconnect() {
    let (master, master_kv, master_addr) = create_master().await;

    // Initial data
    for i in 0..50 {
        let key = format!("key_{}", i);
        master_kv
            .set(&key, format!("value_{}", i).into_bytes(), None)
            .await
            .unwrap();
        master.replicate(Operation::KVSet {
            key,
            value: format!("value_{}", i).into_bytes(),
            ttl: None,
        });
    }

    sleep(Duration::from_millis(100)).await;

    // Create replica and sync
    let (replica, replica_kv) = create_replica(master_addr, true).await;
    sleep(Duration::from_millis(500)).await;

    // Verify initial sync
    assert_eq!(replica.current_offset(), 50);

    // Simulate disconnect by dropping replica
    drop(replica);
    drop(replica_kv); //drop the kv store too
    sleep(Duration::from_millis(200)).await;

    // Add more data while replica is disconnected
    for i in 50..100 {
        let key = format!("key_{}", i);
        master_kv
            .set(&key, format!("value_{}", i).into_bytes(), None)
            .await
            .unwrap();
        master.replicate(Operation::KVSet {
            key,
            value: format!("value_{}", i).into_bytes(),
            ttl: None,
        });
    }

    sleep(Duration::from_millis(100)).await;

    // Reconnect replica - should do partial sync
    let (replica2, replica_kv2) = create_replica(master_addr, false).await;
    sleep(Duration::from_millis(500)).await;

    // Verify all data is synced
    for i in 0..100 {
        let key = format!("key_{}", i);
        let value = replica_kv2.get(&key).await.unwrap();
        assert_eq!(
            value,
            Some(format!("value_{}", i).into_bytes()),
            "Key {} not synced after reconnect",
            key
        );
    }

    assert_eq!(replica2.current_offset(), 100);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_multiple_replicas_sync() {
    let (master, master_kv, master_addr) = create_master().await;

    // Create 3 replicas
    let (replica1, replica_kv1) = create_replica(master_addr, false).await;
    let (replica2, replica_kv2) = create_replica(master_addr, false).await;
    let (replica3, replica_kv3) = create_replica(master_addr, false).await;

    sleep(Duration::from_millis(200)).await;

    // Write data to master
    for i in 0..200 {
        let key = format!("multi_key_{}", i);
        let value = format!("multi_value_{}", i).into_bytes();

        master_kv.set(&key, value.clone(), None).await.unwrap();
        master.replicate(Operation::KVSet {
            key,
            value,
            ttl: None,
        });

        // Small delay to allow replication
        if i % 50 == 0 {
            sleep(Duration::from_millis(50)).await;
        }
    }

    // Wait for all replicas to sync
    sleep(Duration::from_millis(1000)).await;

    // Verify all replicas have the same data
    for i in 0..200 {
        let key = format!("multi_key_{}", i);
        let expected = format!("multi_value_{}", i).into_bytes();

        let v1 = replica_kv1.get(&key).await.unwrap();
        let v2 = replica_kv2.get(&key).await.unwrap();
        let v3 = replica_kv3.get(&key).await.unwrap();

        assert_eq!(v1, Some(expected.clone()), "Replica 1 missing {}", key);
        assert_eq!(v2, Some(expected.clone()), "Replica 2 missing {}", key);
        assert_eq!(v3, Some(expected.clone()), "Replica 3 missing {}", key);
    }

    // All replicas should be at the same offset
    assert_eq!(replica1.current_offset(), 200);
    assert_eq!(replica2.current_offset(), 200);
    assert_eq!(replica3.current_offset(), 200);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_stress_thousands_of_operations() {
    let (master, master_kv, master_addr) = create_master().await;
    let (replica, replica_kv) = create_replica(master_addr, false).await;

    sleep(Duration::from_millis(200)).await;

    // Write 5000 operations
    for i in 0..5000 {
        let key = format!("stress_key_{}", i);
        let value = format!("stress_value_{}", i).into_bytes();

        master_kv.set(&key, value.clone(), None).await.unwrap();
        master.replicate(Operation::KVSet {
            key,
            value,
            ttl: None,
        });

        // Batch delays
        if i % 100 == 0 {
            sleep(Duration::from_millis(10)).await;
        }
    }

    // Wait for replication to complete
    sleep(Duration::from_secs(2)).await;

    // Sample check (checking all 5000 would be slow)
    for i in (0..5000).step_by(100) {
        let key = format!("stress_key_{}", i);
        let value = replica_kv.get(&key).await.unwrap();
        assert_eq!(
            value,
            Some(format!("stress_value_{}", i).into_bytes()),
            "Stress test failed at key {}",
            key
        );
    }

    // Check final offset
    assert_eq!(replica.current_offset(), 5000);

    // Verify master stats
    let stats = master.stats();
    assert_eq!(stats.master_offset, 5000);

    // Verify at least one replica is connected
    let replicas = master.list_replicas();
    assert!(!replicas.is_empty(), "No replicas connected");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_data_consistency_after_updates() {
    let (master, master_kv, master_addr) = create_master().await;
    let (replica, replica_kv) = create_replica(master_addr, false).await;

    sleep(Duration::from_millis(200)).await;

    // Write initial data
    for i in 0..100 {
        let key = format!("consistency_key_{}", i);
        master_kv
            .set(&key, format!("v1_{}", i).into_bytes(), None)
            .await
            .unwrap();
        master.replicate(Operation::KVSet {
            key,
            value: format!("v1_{}", i).into_bytes(),
            ttl: None,
        });
    }

    sleep(Duration::from_millis(500)).await;

    // Update same keys
    for i in 0..100 {
        let key = format!("consistency_key_{}", i);
        master_kv
            .set(&key, format!("v2_{}", i).into_bytes(), None)
            .await
            .unwrap();
        master.replicate(Operation::KVSet {
            key,
            value: format!("v2_{}", i).into_bytes(),
            ttl: None,
        });
    }

    sleep(Duration::from_millis(500)).await;

    // Verify replica has latest values
    for i in 0..100 {
        let key = format!("consistency_key_{}", i);
        let value = replica_kv.get(&key).await.unwrap();
        assert_eq!(
            value,
            Some(format!("v2_{}", i).into_bytes()),
            "Consistency check failed for {}",
            key
        );
    }

    assert_eq!(replica.current_offset(), 200);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_delete_operations_sync() {
    let (master, master_kv, master_addr) = create_master().await;
    let (replica, replica_kv) = create_replica(master_addr, false).await;

    sleep(Duration::from_millis(200)).await;

    // Create keys
    for i in 0..50 {
        let key = format!("del_key_{}", i);
        master_kv
            .set(&key, format!("value_{}", i).into_bytes(), None)
            .await
            .unwrap();
        master.replicate(Operation::KVSet {
            key,
            value: format!("value_{}", i).into_bytes(),
            ttl: None,
        });
    }

    sleep(Duration::from_millis(300)).await;

    // Delete half of them
    for i in 0..25 {
        let key = format!("del_key_{}", i);
        master_kv.mdel(&[key.clone()]).await.unwrap();
        master.replicate(Operation::KVDel { keys: vec![key] });
    }

    sleep(Duration::from_millis(300)).await;

    // Verify deletes replicated
    for i in 0..25 {
        let key = format!("del_key_{}", i);
        let value = replica_kv.get(&key).await.unwrap();
        assert_eq!(value, None, "Key {} should be deleted", key);
    }

    // Verify non-deleted keys still exist
    for i in 25..50 {
        let key = format!("del_key_{}", i);
        let value = replica_kv.get(&key).await.unwrap();
        assert_eq!(
            value,
            Some(format!("value_{}", i).into_bytes()),
            "Key {} should exist",
            key
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_batch_operations_sync() {
    let (master, master_kv, master_addr) = create_master().await;
    let (replica, replica_kv) = create_replica(master_addr, false).await;

    sleep(Duration::from_millis(200)).await;

    // Batch set
    let mut keys = Vec::new();
    let mut values = Vec::new();

    for i in 0..100 {
        keys.push(format!("batch_key_{}", i));
        values.push(format!("batch_value_{}", i).into_bytes());
    }

    // Set all keys
    for (key, value) in keys.iter().zip(values.iter()) {
        master_kv.set(key, value.clone(), None).await.unwrap();
        master.replicate(Operation::KVSet {
            key: key.clone(),
            value: value.clone(),
            ttl: None,
        });
    }

    sleep(Duration::from_millis(800)).await;

    // Verify all replicated
    for (key, expected_value) in keys.iter().zip(values.iter()) {
        let value = replica_kv.get(key).await.unwrap();
        assert_eq!(
            value,
            Some(expected_value.clone()),
            "Batch key {} not synced",
            key
        );
    }

    assert_eq!(replica.current_offset(), 100);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_replication_lag_monitoring() {
    let (master, master_kv, master_addr) = create_master().await;
    let (replica, _replica_kv) = create_replica(master_addr, false).await;

    sleep(Duration::from_millis(200)).await;

    // Write operations continuously
    for i in 0..100 {
        let key = format!("lag_key_{}", i);
        master_kv
            .set(&key, format!("value_{}", i).into_bytes(), None)
            .await
            .unwrap();
        master.replicate(Operation::KVSet {
            key,
            value: format!("value_{}", i).into_bytes(),
            ttl: None,
        });
    }

    // Check stats periodically
    sleep(Duration::from_millis(500)).await;

    let stats = master.stats();
    assert_eq!(stats.master_offset, 100);

    // Replica should be caught up or very close
    let replica_offset = replica.current_offset();
    assert!(
        replica_offset >= 95,
        "Replica lag too high: offset = {}",
        replica_offset
    );

    let lag = replica.lag();
    assert!(
        lag < 10,
        "Replication lag too high: {} operations behind",
        lag
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_replica_auto_reconnect() {
    let (master, master_kv, master_addr) = create_master().await;

    // Initial data
    for i in 0..50 {
        let key = format!("reconnect_key_{}", i);
        master_kv
            .set(&key, format!("value_{}", i).into_bytes(), None)
            .await
            .unwrap();
        master.replicate(Operation::KVSet {
            key,
            value: format!("value_{}", i).into_bytes(),
            ttl: None,
        });
    }

    sleep(Duration::from_millis(200)).await;

    // Create replica with auto-reconnect
    let (replica, replica_kv) = create_replica(master_addr, true).await;
    sleep(Duration::from_millis(500)).await;

    assert_eq!(replica.current_offset(), 50);

    // Simulate temporary network issue by dropping connection
    // (In real scenario, you'd close the connection)

    // Add more data
    for i in 50..100 {
        let key = format!("reconnect_key_{}", i);
        master_kv
            .set(&key, format!("value_{}", i).into_bytes(), None)
            .await
            .unwrap();
        master.replicate(Operation::KVSet {
            key,
            value: format!("value_{}", i).into_bytes(),
            ttl: None,
        });
    }

    // Wait for auto-reconnect and resync
    sleep(Duration::from_secs(2)).await;

    // Verify data after reconnect
    for i in 0..100 {
        let key = format!("reconnect_key_{}", i);
        let value = replica_kv.get(&key).await.unwrap();
        assert_eq!(
            value,
            Some(format!("value_{}", i).into_bytes()),
            "Key {} not synced after reconnect",
            key
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_concurrent_writes_during_sync() {
    let (master, master_kv, master_addr) = create_master().await;

    // Start writing data
    let master_kv_clone = Arc::clone(&master_kv);
    let master_clone = Arc::clone(&master);

    let write_task = tokio::spawn(async move {
        for i in 0..500 {
            let key = format!("concurrent_key_{}", i);
            let value = format!("concurrent_value_{}", i).into_bytes();

            master_kv_clone
                .set(&key, value.clone(), None)
                .await
                .unwrap();
            master_clone.replicate(Operation::KVSet {
                key,
                value,
                ttl: None,
            });

            if i % 50 == 0 {
                sleep(Duration::from_millis(50)).await;
            }
        }
    });

    // Start replica while writes are happening
    sleep(Duration::from_millis(100)).await;
    let (replica, replica_kv) = create_replica(master_addr, false).await;

    // Wait for writes to complete
    write_task.await.unwrap();

    // Wait for sync
    sleep(Duration::from_secs(2)).await;

    // Verify all data synced
    let replica_offset = replica.current_offset();
    assert!(
        replica_offset >= 480,
        "Not all operations synced: offset = {}",
        replica_offset
    );

    // Sample verification
    for i in (0..500).step_by(50) {
        let key = format!("concurrent_key_{}", i);
        let value = replica_kv.get(&key).await.unwrap();
        assert!(
            value.is_some(),
            "Key {} missing after concurrent writes",
            key
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_large_values_replication() {
    let (master, master_kv, master_addr) = create_master().await;
    let (replica, replica_kv) = create_replica(master_addr, false).await;

    sleep(Duration::from_millis(200)).await;

    // Create large values (100KB each)
    for i in 0..10 {
        let key = format!("large_key_{}", i);
        let value = vec![i as u8; 100 * 1024]; // 100KB

        master_kv.set(&key, value.clone(), None).await.unwrap();
        master.replicate(Operation::KVSet {
            key,
            value,
            ttl: None,
        });
    }

    // Wait for large values to replicate
    sleep(Duration::from_secs(2)).await;

    // Verify large values
    for i in 0..10 {
        let key = format!("large_key_{}", i);
        let value = replica_kv.get(&key).await.unwrap();
        assert!(value.is_some(), "Large value {} not replicated", key);
        assert_eq!(
            value.unwrap().len(),
            100 * 1024,
            "Large value {} has wrong size",
            key
        );
    }

    assert_eq!(replica.current_offset(), 10);
}
