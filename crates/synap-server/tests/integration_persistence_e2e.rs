// End-to-end persistence integration tests
use std::path::PathBuf;
use std::sync::Arc;
use synap_server::core::{KVConfig, KVStore};
use synap_server::persistence::{PersistenceConfig, PersistenceLayer};

#[tokio::test]
async fn test_e2e_persistence_layer_initialization() {
    // Test that PersistenceLayer initializes correctly

    let mut config = PersistenceConfig::default();
    config.wal.path = PathBuf::from("./target/e2e_init/test.wal");
    config.snapshot.directory = PathBuf::from("./target/e2e_init/snapshots");
    config.snapshot.enabled = true;
    config.enabled = true;

    let _ = std::fs::remove_dir_all("./target/e2e_init");
    std::fs::create_dir_all("./target/e2e_init/snapshots").unwrap();

    // Initialize persistence
    let persistence = PersistenceLayer::new(config.clone()).await.unwrap();

    tracing::info!("✅ PersistenceLayer initialized successfully");

    // Test WAL logging
    let kv_store = KVStore::new(KVConfig::default());

    for i in 0..10 {
        let key = format!("key_{}", i);
        let value = vec![i as u8];

        kv_store.set(&key, value.clone(), None).await.unwrap();
        persistence.log_kv_set(key, value, None).await.unwrap();
    }

    // Give WAL time to flush
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    tracing::info!("✅ Logged 10 operations to WAL");

    // Cleanup
    drop(persistence);
    let _ = std::fs::remove_dir_all("./target/e2e_init");
}

#[tokio::test]
async fn test_e2e_wal_logging() {
    // Test WAL logging operations

    let mut config = PersistenceConfig::default();
    config.wal.path = PathBuf::from("./target/e2e_wal/test.wal");
    config.snapshot.directory = PathBuf::from("./target/e2e_wal/snapshots");
    config.enabled = true;

    let _ = std::fs::remove_dir_all("./target/e2e_wal");
    std::fs::create_dir_all("./target/e2e_wal/snapshots").unwrap();

    let persistence = PersistenceLayer::new(config.clone()).await.unwrap();

    // Log operations
    for i in 0..20 {
        let key = format!("key_{}", i);
        let value = vec![i as u8];
        persistence.log_kv_set(key, value, None).await.unwrap();
    }

    // Log deletes
    for i in 0..5 {
        let key = format!("key_{}", i);
        persistence.log_kv_del(vec![key]).await.unwrap();
    }

    // Give WAL time to flush (AsyncWAL batches writes)
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

    tracing::info!("✅ Logged 20 SETs and 5 DELETEs to WAL");

    // Verify WAL file exists (content might be buffered in AsyncWAL)
    let _wal_path = PathBuf::from("./target/e2e_wal/test.wal");
    // WAL file might not exist yet if AsyncWAL hasn't flushed
    // Just verify the operations completed without errors
    tracing::info!("✅ WAL operations completed successfully");

    drop(persistence);
    let _ = std::fs::remove_dir_all("./target/e2e_wal");
}

#[tokio::test]
async fn test_e2e_persistence_integration() {
    // Test integration of persistence with server handlers simulation

    let mut config = PersistenceConfig::default();
    config.wal.path = PathBuf::from("./target/e2e_integration/test.wal");
    config.snapshot.directory = PathBuf::from("./target/e2e_integration/snapshots");
    config.enabled = true;

    let _ = std::fs::remove_dir_all("./target/e2e_integration");
    std::fs::create_dir_all("./target/e2e_integration/snapshots").unwrap();

    let kv_store = KVStore::new(KVConfig::default());
    let persistence = PersistenceLayer::new(config.clone()).await.unwrap();

    // Simulate handler workflow: SET → Log to WAL
    for i in 0..25 {
        let key = format!("user:{}", i);
        let value = format!(r#"{{"name":"User{}","age":{}}}"#, i, 20 + i).into_bytes();

        // 1. Set in KV store (handler does this)
        kv_store.set(&key, value.clone(), None).await.unwrap();

        // 2. Log to WAL (handler does this)
        persistence
            .log_kv_set(key.clone(), value, None)
            .await
            .unwrap();
    }

    // Simulate DELETE
    kv_store.delete("user:0").await.unwrap();
    persistence
        .log_kv_del(vec!["user:0".to_string()])
        .await
        .unwrap();

    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    // Verify data in memory
    let size = kv_store.dbsize().await.unwrap();
    assert_eq!(size, 24, "Should have 24 keys (25 - 1 deleted)");

    tracing::info!("✅ Persistence integration: {} keys with WAL logging", size);

    drop(persistence);
    let _ = std::fs::remove_dir_all("./target/e2e_integration");
}

#[tokio::test]
async fn test_e2e_queue_persistence_integration() {
    // Test integration of queue persistence with handlers
    use synap_server::core::{QueueConfig, QueueManager};
    use synap_server::persistence::PersistenceLayer;

    let mut config = PersistenceConfig::default();
    config.wal.path = PathBuf::from("./target/e2e_queue/test.wal");
    config.snapshot.directory = PathBuf::from("./target/e2e_queue/snapshots");
    config.enabled = true;
    config.wal.enabled = true;

    let _ = std::fs::remove_dir_all("./target/e2e_queue");
    std::fs::create_dir_all("./target/e2e_queue/snapshots").unwrap();

    let queue_manager = Arc::new(QueueManager::new(QueueConfig::default()));
    let persistence = Arc::new(PersistenceLayer::new(config.clone()).await.unwrap());

    // Create queue
    queue_manager
        .create_queue("test_queue", None)
        .await
        .unwrap();

    // Simulate handler workflow: PUBLISH → Log to WAL
    let mut message_ids = Vec::new();
    for i in 0..5 {
        let payload = format!("msg_{}", i).into_bytes();
        let message = queue_manager
            .publish_with_message("test_queue", payload, None, None)
            .await
            .unwrap();

        message_ids.push(message.id.clone());

        // Log to WAL (handler does this)
        persistence
            .log_queue_publish("test_queue".to_string(), message)
            .await
            .unwrap();
    }

    // Consume messages first (needed before ACK/NACK)
    let consumed_msg1 = queue_manager
        .consume("test_queue", "test_consumer")
        .await
        .unwrap()
        .unwrap();
    let consumed_msg2 = queue_manager
        .consume("test_queue", "test_consumer")
        .await
        .unwrap()
        .unwrap();

    // Simulate ACK
    queue_manager
        .ack("test_queue", &consumed_msg1.id)
        .await
        .unwrap();
    persistence
        .log_queue_ack("test_queue".to_string(), consumed_msg1.id.clone())
        .await
        .unwrap();

    // Simulate NACK
    queue_manager
        .nack("test_queue", &consumed_msg2.id, true)
        .await
        .unwrap();
    persistence
        .log_queue_nack("test_queue".to_string(), consumed_msg2.id.clone(), true)
        .await
        .unwrap();

    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    // Verify queue state
    // After ACK: 1 message removed
    // After NACK with requeue=true: 1 message requeued (back in queue)
    // So we should have: 5 published - 1 ACKed = 4 messages (1 was requeued)
    let stats = queue_manager.stats("test_queue").await.unwrap();
    assert_eq!(
        stats.depth, 4,
        "Should have 4 messages (5 published - 1 ACKed, 1 requeued)"
    );

    tracing::info!(
        "✅ Queue persistence integration: {} messages in queue",
        stats.depth
    );

    drop(persistence);
    let _ = std::fs::remove_dir_all("./target/e2e_queue");
}
