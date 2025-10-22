//! Extended unit tests for replication system
//!
//! These tests provide comprehensive coverage of replication components

use std::sync::Arc;
use synap_server::persistence::types::Operation;
use synap_server::replication::{
    MasterNode, NodeRole, ReplicaNode, ReplicationConfig, ReplicationLog,
};
use synap_server::{KVConfig, KVStore};

#[tokio::test]
async fn test_replication_log_wraparound() {
    let log = ReplicationLog::new(100);

    // Fill buffer twice
    for i in 0..200 {
        log.append(Operation::KVSet {
            key: format!("key_{}", i),
            value: vec![i as u8],
            ttl: None,
        });
    }

    // Should have 200 operations but only last 100
    assert_eq!(log.current_offset(), 200);
    assert_eq!(log.oldest_offset(), 100);

    // Can get recent operations
    let ops = log.get_from_offset(150).unwrap();
    assert_eq!(ops.len(), 50);
}

#[tokio::test]
async fn test_replication_log_concurrent_append() {
    let log = Arc::new(ReplicationLog::new(10000));

    let mut handles = vec![];

    // Spawn 10 tasks appending 100 operations each
    for task_id in 0..10 {
        let log_clone = Arc::clone(&log);
        let handle = tokio::spawn(async move {
            for i in 0..100 {
                log_clone.append(Operation::KVSet {
                    key: format!("task_{}_{}", task_id, i),
                    value: vec![i as u8],
                    ttl: None,
                });
            }
        });
        handles.push(handle);
    }

    // Wait for all tasks
    for handle in handles {
        handle.await.unwrap();
    }

    // Should have exactly 1000 operations
    assert_eq!(log.current_offset(), 1000);
}

#[tokio::test]
async fn test_master_multiple_operations() {
    let mut config = ReplicationConfig::default();
    config.enabled = true;
    config.role = NodeRole::Master;
    config.replica_listen_address = Some("127.0.0.1:25000".parse().unwrap());

    let kv = Arc::new(KVStore::new(KVConfig::default()));
    let master = Arc::new(MasterNode::new(config, kv).await.unwrap());

    // Replicate 1000 operations
    for i in 0..1000 {
        master.replicate(Operation::KVSet {
            key: format!("key_{}", i),
            value: format!("value_{}", i).into_bytes(),
            ttl: None,
        });
    }

    let stats = master.stats();
    assert_eq!(stats.master_offset, 1000);
}

#[tokio::test]
async fn test_master_list_replicas_empty() {
    let mut config = ReplicationConfig::default();
    config.enabled = true;
    config.role = NodeRole::Master;
    config.replica_listen_address = Some("127.0.0.1:25001".parse().unwrap());

    let kv = Arc::new(KVStore::new(KVConfig::default()));
    let master = Arc::new(MasterNode::new(config, kv).await.unwrap());

    let replicas = master.list_replicas();
    assert!(replicas.is_empty());
}

#[tokio::test]
async fn test_replica_stats() {
    let mut config = ReplicationConfig::default();
    config.enabled = true;
    config.role = NodeRole::Replica;
    config.master_address = Some("127.0.0.1:25002".parse().unwrap());
    config.auto_reconnect = false;

    let kv = Arc::new(KVStore::new(KVConfig::default()));
    let replica = ReplicaNode::new(config, kv).await.unwrap();

    let stats = replica.stats().await;
    assert_eq!(stats.replica_offset, 0);
    assert_eq!(stats.master_offset, 0);
    assert_eq!(stats.lag_operations, 0);
    assert!(!stats.connected);
}

#[tokio::test]
async fn test_replica_lag_calculation() {
    let mut config = ReplicationConfig::default();
    config.enabled = true;
    config.role = NodeRole::Replica;
    config.master_address = Some("127.0.0.1:25003".parse().unwrap());
    config.auto_reconnect = false;

    let kv = Arc::new(KVStore::new(KVConfig::default()));
    let replica = ReplicaNode::new(config, kv).await.unwrap();

    // Initially no lag
    assert_eq!(replica.lag(), 0);
    assert_eq!(replica.current_offset(), 0);
}

#[tokio::test]
async fn test_replication_log_lag_various_offsets() {
    let log = ReplicationLog::new(1000);

    // Add 500 operations
    for i in 0..500 {
        log.append(Operation::KVSet {
            key: format!("key_{}", i),
            value: vec![i as u8],
            ttl: None,
        });
    }

    // Test various lag calculations
    assert_eq!(log.calculate_lag(500), 0); // Caught up
    assert_eq!(log.calculate_lag(450), 50); // 50 behind
    assert_eq!(log.calculate_lag(0), 500); // Very behind
    assert_eq!(log.calculate_lag(250), 250); // Halfway
}

#[tokio::test]
async fn test_replication_config_defaults() {
    let config = ReplicationConfig::default();

    assert!(!config.enabled);
    assert_eq!(config.role, NodeRole::Standalone);
    assert!(config.master_address.is_none());
    assert!(config.replica_listen_address.is_none());
    assert_eq!(config.heartbeat_interval_ms, 1000);
    assert_eq!(config.max_lag_ms, 10000);
    assert!(config.auto_reconnect);
    assert_eq!(config.reconnect_delay_ms, 5000);
}

#[tokio::test]
async fn test_master_config_validation_fail_without_address() {
    let mut config = ReplicationConfig::default();
    config.enabled = true;
    config.role = NodeRole::Master;
    // Missing replica_listen_address

    assert!(config.validate().is_err());
}

#[tokio::test]
async fn test_replica_config_validation_fail_without_master() {
    let mut config = ReplicationConfig::default();
    config.enabled = true;
    config.role = NodeRole::Replica;
    // Missing master_address

    assert!(config.validate().is_err());
}

#[tokio::test]
async fn test_replication_log_empty() {
    let log = ReplicationLog::new(100);

    assert_eq!(log.current_offset(), 0);
    assert_eq!(log.oldest_offset(), 0);
    assert_eq!(log.calculate_lag(0), 0);
}

#[tokio::test]
async fn test_replication_log_get_all_from_start() {
    let log = ReplicationLog::new(100);

    for i in 0..50 {
        log.append(Operation::KVSet {
            key: format!("key_{}", i),
            value: vec![i as u8],
            ttl: None,
        });
    }

    let ops = log.get_from_offset(0).unwrap();
    assert_eq!(ops.len(), 50);
    assert_eq!(ops.first().unwrap().offset, 0);
    assert_eq!(ops.last().unwrap().offset, 49);
}

#[tokio::test]
async fn test_delete_operation_replication() {
    let log = ReplicationLog::new(100);

    // Set operation
    log.append(Operation::KVSet {
        key: "test_key".to_string(),
        value: b"test_value".to_vec(),
        ttl: None,
    });

    // Delete operation
    log.append(Operation::KVDel {
        keys: vec!["test_key".to_string()],
    });

    assert_eq!(log.current_offset(), 2);

    let ops = log.get_from_offset(0).unwrap();
    assert_eq!(ops.len(), 2);

    // Verify second is delete
    match &ops[1].operation {
        Operation::KVDel { keys } => {
            assert_eq!(keys.len(), 1);
            assert_eq!(keys[0], "test_key");
        }
        _ => panic!("Expected delete operation"),
    }
}

#[tokio::test]
async fn test_batch_delete_replication() {
    let log = ReplicationLog::new(100);

    log.append(Operation::KVDel {
        keys: vec!["key1".to_string(), "key2".to_string(), "key3".to_string()],
    });

    let ops = log.get_from_offset(0).unwrap();
    assert_eq!(ops.len(), 1);

    match &ops[0].operation {
        Operation::KVDel { keys } => {
            assert_eq!(keys.len(), 3);
        }
        _ => panic!("Expected delete operation"),
    }
}

#[tokio::test]
async fn test_master_replication_various_operations() {
    let mut config = ReplicationConfig::default();
    config.enabled = true;
    config.role = NodeRole::Master;
    config.replica_listen_address = Some("127.0.0.1:25004".parse().unwrap());

    let kv = Arc::new(KVStore::new(KVConfig::default()));
    let master = Arc::new(MasterNode::new(config, kv).await.unwrap());

    // SET operations
    for i in 0..10 {
        master.replicate(Operation::KVSet {
            key: format!("key_{}", i),
            value: vec![i as u8],
            ttl: None,
        });
    }

    // DELETE operations
    master.replicate(Operation::KVDel {
        keys: vec!["key_0".to_string(), "key_1".to_string()],
    });

    let stats = master.stats();
    assert_eq!(stats.master_offset, 11); // 10 sets + 1 delete
}

#[tokio::test]
async fn test_replication_with_ttl() {
    let log = ReplicationLog::new(100);

    log.append(Operation::KVSet {
        key: "expiring_key".to_string(),
        value: b"value".to_vec(),
        ttl: Some(60), // 60 seconds TTL
    });

    let ops = log.get_from_offset(0).unwrap();
    match &ops[0].operation {
        Operation::KVSet { key, value, ttl } => {
            assert_eq!(key, "expiring_key");
            assert_eq!(value, b"value");
            assert_eq!(*ttl, Some(60));
        }
        _ => panic!("Expected SET operation with TTL"),
    }
}
