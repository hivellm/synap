use super::*;
use crate::core::{KVConfig, KVStore};
use std::sync::Arc;

#[tokio::test]
async fn test_replication_log_basic() {
    let log = ReplicationLog::new(100);

    let op = crate::persistence::types::Operation::KVSet {
        key: "test".to_string(),
        value: b"value".to_vec(),
        ttl: None,
    };

    let offset = log.append(op);
    assert_eq!(offset, 0);
    assert_eq!(log.current_offset(), 1);
}

#[tokio::test]
async fn test_master_replica_sync() {
    // Setup master
    let mut master_config = ReplicationConfig::default();
    master_config.enabled = true;
    master_config.role = NodeRole::Master;
    master_config.replica_listen_address = Some("127.0.0.1:0".parse().unwrap());

    let master_kv = Arc::new(KVStore::new(KVConfig::default()));
    let master = MasterNode::new(master_config.clone(), Arc::clone(&master_kv))
        .await
        .unwrap();

    // Write some data to master
    master_kv
        .set("key1", b"value1".to_vec(), None)
        .await
        .unwrap();
    master_kv
        .set("key2", b"value2".to_vec(), None)
        .await
        .unwrap();

    // Replicate operations
    master.replicate(crate::persistence::types::Operation::KVSet {
        key: "key1".to_string(),
        value: b"value1".to_vec(),
        ttl: None,
    });

    master.replicate(crate::persistence::types::Operation::KVSet {
        key: "key2".to_string(),
        value: b"value2".to_vec(),
        ttl: None,
    });

    // Verify master stats
    let stats = master.stats();
    assert_eq!(stats.master_offset, 2);
}

#[tokio::test]
async fn test_replica_initialization() {
    let mut replica_config = ReplicationConfig::default();
    replica_config.enabled = true;
    replica_config.role = NodeRole::Replica;
    replica_config.master_address = Some("127.0.0.1:15501".parse().unwrap());
    replica_config.auto_reconnect = false; // Don't actually connect

    let replica_kv = Arc::new(KVStore::new(KVConfig::default()));
    let replica = ReplicaNode::new(replica_config, replica_kv).await.unwrap();

    assert!(!replica.is_connected());
    assert_eq!(replica.current_offset(), 0);
}

#[tokio::test]
async fn test_snapshot_sync() {
    let kv = KVStore::new(KVConfig::default());

    // Populate data
    kv.set("test1", b"data1".to_vec(), None).await.unwrap();
    kv.set("test2", b"data2".to_vec(), None).await.unwrap();
    kv.set("test3", b"data3".to_vec(), None).await.unwrap();

    // Create snapshot
    let snapshot = sync::create_snapshot(&kv, 100).await.unwrap();
    assert!(!snapshot.is_empty());

    // Apply to new store
    let kv2 = KVStore::new(KVConfig::default());
    let offset = sync::apply_snapshot(&kv2, &snapshot).await.unwrap();

    assert_eq!(offset, 100);
    assert_eq!(kv2.get("test1").await.unwrap(), Some(b"data1".to_vec()));
    assert_eq!(kv2.get("test2").await.unwrap(), Some(b"data2".to_vec()));
    assert_eq!(kv2.get("test3").await.unwrap(), Some(b"data3".to_vec()));
}

#[tokio::test]
async fn test_replication_lag_calculation() {
    let log = ReplicationLog::new(1000);

    // Add 100 operations
    for i in 0..100 {
        log.append(crate::persistence::types::Operation::KVSet {
            key: format!("key_{}", i),
            value: vec![i as u8],
            ttl: None,
        });
    }

    // Replica is at offset 60
    let lag = log.calculate_lag(60);
    assert_eq!(lag, 40); // 100 - 60 = 40 operations behind
}

#[tokio::test]
async fn test_partial_resync() {
    let log = ReplicationLog::new(1000);

    // Add operations
    for i in 0..50 {
        log.append(crate::persistence::types::Operation::KVSet {
            key: format!("key_{}", i),
            value: vec![i as u8],
            ttl: None,
        });
    }

    // Get from offset 30
    let ops = log.get_from_offset(30).unwrap();
    assert_eq!(ops.len(), 20); // Operations 30-49
    assert_eq!(ops[0].offset, 30);
    assert_eq!(ops.last().unwrap().offset, 49);
}

#[tokio::test]
async fn test_full_sync_required() {
    let log = ReplicationLog::new(10); // Small buffer

    // Fill buffer and overflow
    for i in 0..50 {
        log.append(crate::persistence::types::Operation::KVSet {
            key: format!("key_{}", i),
            value: vec![i as u8],
            ttl: None,
        });
    }

    // Oldest should be 40 (50 - 10)
    assert_eq!(log.oldest_offset(), 40);

    // Requesting from old offset should fail
    let result = log.get_from_offset(10);
    assert!(matches!(result, Err(ReplicationError::FullSyncRequired)));
}

#[tokio::test]
async fn test_config_validation() {
    // Valid master config
    let mut master_cfg = ReplicationConfig::default();
    master_cfg.enabled = true;
    master_cfg.role = NodeRole::Master;
    master_cfg.replica_listen_address = Some("0.0.0.0:15501".parse().unwrap());
    assert!(master_cfg.validate().is_ok());

    // Invalid master (missing address)
    let mut bad_master = ReplicationConfig::default();
    bad_master.enabled = true;
    bad_master.role = NodeRole::Master;
    assert!(bad_master.validate().is_err());

    // Valid replica config
    let mut replica_cfg = ReplicationConfig::default();
    replica_cfg.enabled = true;
    replica_cfg.role = NodeRole::Replica;
    replica_cfg.master_address = Some("127.0.0.1:15501".parse().unwrap());
    assert!(replica_cfg.validate().is_ok());

    // Invalid replica (missing master)
    let mut bad_replica = ReplicationConfig::default();
    bad_replica.enabled = true;
    bad_replica.role = NodeRole::Replica;
    assert!(bad_replica.validate().is_err());
}
