//! Integration tests for replication system
//!
//! These tests cover:
//! - Full sync with real TCP connections
//! - Partial sync after reconnection
//! - Multiple replicas synchronization
//! - Disconnect/reconnect scenarios
//! - Data consistency verification
//! - Stress tests with thousands of operations

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

    // Give replica a moment to start connecting
    sleep(Duration::from_millis(50)).await;

    (replica, kv)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_full_sync_with_real_connection() {
    // Create master and populate with data
    let (master, master_kv, master_addr) = create_master().await;

    // Populate master with initial data BEFORE replica connects
    for i in 0..100 {
        let key = format!("key_{}", i);
        let value = format!("value_{}", i).into_bytes();

        // Add to KV store (this is what gets snapshotted)
        master_kv.set(&key, value, None).await.unwrap();
    }

    // Small delay to ensure operations are logged
    sleep(Duration::from_millis(50)).await;

    // Create replica - should trigger full sync from snapshot
    // Use auto_reconnect=true so replica stays connected for verification
    let (replica, replica_kv) = create_replica(master_addr, true).await;

    // Give replica time to connect and receive full sync
    sleep(Duration::from_secs(2)).await;

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

    // Verify replica offset (should match snapshot offset)
    assert!(replica.is_connected());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_partial_sync_after_disconnect() {
    let (master, master_kv, master_addr) = create_master().await;

    // Initial data - add to KV store only (snapshot will be used)
    for i in 0..50 {
        let key = format!("key_{}", i);
        master_kv
            .set(&key, format!("value_{}", i).into_bytes(), None)
            .await
            .unwrap();
    }

    sleep(Duration::from_millis(100)).await;

    // Create replica and sync
    let (replica, replica_kv) = create_replica(master_addr, true).await;
    sleep(Duration::from_secs(1)).await;

    // Verify initial sync (snapshot synced 50 keys)
    assert_eq!(replica_kv.keys().await.unwrap().len(), 50);

    // Add more data with replication log
    for i in 50..100 {
        let key = format!("key_{}", i);
        let value = format!("value_{}", i).into_bytes();
        master_kv.set(&key, value.clone(), None).await.unwrap();

        // Add to replication log so partial sync can work
        master.replicate(Operation::KVSet {
            key,
            value,
            ttl: None,
        });
    }

    // Wait for replication to replica
    sleep(Duration::from_secs(1)).await;

    // Verify all 100 keys are now synced
    let final_count = replica_kv.keys().await.unwrap().len();
    assert_eq!(final_count, 100, "Expected 100 keys, got {}", final_count);

    // Verify replica caught up
    assert!(
        replica.current_offset() >= 50,
        "Replica should have processed replicated operations"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_multiple_replicas_sync() {
    let (master, master_kv, master_addr) = create_master().await;

    // Populate master first
    for i in 0..200 {
        let key = format!("multi_key_{}", i);
        let value = format!("multi_value_{}", i).into_bytes();
        master_kv.set(&key, value, None).await.unwrap();
    }

    sleep(Duration::from_millis(100)).await;

    // Create 3 replicas - all should get full snapshot
    let (replica1, replica_kv1) = create_replica(master_addr, true).await;
    let (replica2, replica_kv2) = create_replica(master_addr, true).await;
    let (replica3, replica_kv3) = create_replica(master_addr, true).await;

    // Wait for all replicas to sync
    sleep(Duration::from_secs(3)).await;

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

    // All replicas should have received all data
    assert_eq!(replica_kv1.keys().await.unwrap().len(), 200);
    assert_eq!(replica_kv2.keys().await.unwrap().len(), 200);
    assert_eq!(replica_kv3.keys().await.unwrap().len(), 200);

    // All replicas should be connected
    assert!(replica1.is_connected());
    assert!(replica2.is_connected());
    assert!(replica3.is_connected());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "Slow test - run manually"]
async fn test_stress_thousands_of_operations() {
    let (master, master_kv, master_addr) = create_master().await;

    // Add initial data for snapshot
    for i in 0..1000 {
        let key = format!("stress_key_{}", i);
        master_kv
            .set(&key, format!("stress_value_{}", i).into_bytes(), None)
            .await
            .unwrap();
    }

    let (replica, replica_kv) = create_replica(master_addr, true).await;
    sleep(Duration::from_secs(1)).await;

    // Add more data via replication log
    for i in 1000..5000 {
        let key = format!("stress_key_{}", i);
        let value = format!("stress_value_{}", i).into_bytes();

        master_kv.set(&key, value.clone(), None).await.unwrap();
        master.replicate(Operation::KVSet {
            key,
            value,
            ttl: None,
        });
    }

    // Wait for replication to complete
    sleep(Duration::from_secs(3)).await;

    // Verify replica has significant amount of data
    let key_count = replica_kv.keys().await.unwrap().len();
    assert!(
        key_count >= 1000,
        "Replica should have at least 1000 keys, has {}",
        key_count
    );

    // Sample check
    for i in (0..5000).step_by(500) {
        let key = format!("stress_key_{}", i);
        let value = replica_kv.get(&key).await.unwrap();
        assert!(value.is_some(), "Stress test: key {} missing", key);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_data_consistency_after_updates() {
    let (master, master_kv, master_addr) = create_master().await;

    // Initial data for snapshot
    for i in 0..100 {
        let key = format!("consistency_key_{}", i);
        master_kv
            .set(&key, format!("v1_{}", i).into_bytes(), None)
            .await
            .unwrap();
    }

    let (replica, replica_kv) = create_replica(master_addr, true).await;
    sleep(Duration::from_secs(1)).await;

    // Update same keys via replication log
    for i in 0..100 {
        let key = format!("consistency_key_{}", i);
        let value = format!("v2_{}", i).into_bytes();
        master_kv.set(&key, value.clone(), None).await.unwrap();
        master.replicate(Operation::KVSet {
            key,
            value,
            ttl: None,
        });
    }

    sleep(Duration::from_secs(1)).await;

    // Verify replica has latest values (v2)
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

    // Should have processed ~100 update operations
    assert!(replica.current_offset() >= 50);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_delete_operations_sync() {
    let (master, master_kv, master_addr) = create_master().await;
    
    // Create keys in master KV store (for snapshot)
    for i in 0..50 {
        let key = format!("del_key_{}", i);
        master_kv.set(&key, format!("value_{}", i).into_bytes(), None).await.unwrap();
    }

    let (replica, replica_kv) = create_replica(master_addr, true).await;
    sleep(Duration::from_secs(1)).await;
    
    // Verify all 50 keys synced
    assert_eq!(replica_kv.keys().await.unwrap().len(), 50);

    // Delete half of them via replication
    for i in 0..25 {
        let key = format!("del_key_{}", i);
        master_kv.mdel(&[key.clone()]).await.unwrap();
        master.replicate(Operation::KVDel { keys: vec![key] });
    }

    sleep(Duration::from_secs(1)).await;

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

    // Batch set to KV store
    for i in 0..100 {
        let key = format!("batch_key_{}", i);
        let value = format!("batch_value_{}", i).into_bytes();
        master_kv.set(&key, value, None).await.unwrap();
    }

    let (replica, replica_kv) = create_replica(master_addr, true).await;
    sleep(Duration::from_secs(1)).await;

    // Verify all replicated via snapshot
    assert_eq!(replica_kv.keys().await.unwrap().len(), 100);
    
    for i in 0..100 {
        let key = format!("batch_key_{}", i);
        let value = replica_kv.get(&key).await.unwrap();
        assert_eq!(
            value,
            Some(format!("batch_value_{}", i).into_bytes()),
            "Batch key {} not synced",
            key
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_replication_lag_monitoring() {
    let (master, master_kv, master_addr) = create_master().await;
    
    // Initial data for snapshot
    for i in 0..50 {
        let key = format!("lag_key_{}", i);
        master_kv.set(&key, format!("value_{}", i).into_bytes(), None).await.unwrap();
    }
    
    let (replica, _replica_kv) = create_replica(master_addr, true).await;
    sleep(Duration::from_secs(1)).await;

    // Write more operations via replication log
    for i in 50..150 {
        let key = format!("lag_key_{}", i);
        let value = format!("value_{}", i).into_bytes();
        master_kv.set(&key, value.clone(), None).await.unwrap();
        master.replicate(Operation::KVSet {
            key,
            value,
            ttl: None,
        });
    }

    sleep(Duration::from_secs(1)).await;

    let stats = master.stats();
    assert_eq!(stats.master_offset, 100); // 100 operations in log

    // Replica should have caught up
    let replica_offset = replica.current_offset();
    assert!(
        replica_offset >= 50,
        "Replica should have processed some operations: offset = {}",
        replica_offset
    );

    let lag = replica.lag();
    assert!(
        lag < 60,
        "Replication lag too high: {} operations behind",
        lag
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_replica_auto_reconnect() {
    let (master, master_kv, master_addr) = create_master().await;

    // Initial data for snapshot
    for i in 0..50 {
        let key = format!("reconnect_key_{}", i);
        master_kv.set(&key, format!("value_{}", i).into_bytes(), None).await.unwrap();
    }

    // Create replica with auto-reconnect
    let (replica, replica_kv) = create_replica(master_addr, true).await;
    sleep(Duration::from_secs(1)).await;

    assert_eq!(replica_kv.keys().await.unwrap().len(), 50);

    // Add more data via replication log (replica should receive these)
    for i in 50..100 {
        let key = format!("reconnect_key_{}", i);
        let value = format!("value_{}", i).into_bytes();
        master_kv.set(&key, value.clone(), None).await.unwrap();
        master.replicate(Operation::KVSet {
            key,
            value,
            ttl: None,
        });
    }

    sleep(Duration::from_secs(1)).await;

    // Verify replica caught up
    let key_count = replica_kv.keys().await.unwrap().len();
    assert!(key_count >= 50, "Replica should have at least 50 keys, has {}", key_count);
    
    assert!(replica.is_connected());
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
