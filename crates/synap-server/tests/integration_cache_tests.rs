use synap_server::{KVConfig, KVStore};

#[tokio::test]
async fn test_kv_with_cache_hit() {
    // Create KV store with cache enabled (1000 entries)
    let config = KVConfig {
        max_memory_mb: 100,
        ..Default::default()
    };
    let store = KVStore::new_with_cache(config, Some(1000));

    // Set a value
    store
        .set("cached_key", vec![1, 2, 3, 4], None)
        .await
        .unwrap();

    // First get - should populate cache
    let value1 = store.get("cached_key").await.unwrap();
    assert_eq!(value1, Some(vec![1, 2, 3, 4]));

    // Second get - should hit cache
    let value2 = store.get("cached_key").await.unwrap();
    assert_eq!(value2, Some(vec![1, 2, 3, 4]));

    // Verify stats show hits
    let stats = store.stats().await;
    assert!(stats.hits >= 2);
}

#[tokio::test]
async fn test_kv_with_cache_miss() {
    let config = KVConfig {
        max_memory_mb: 100,
        ..Default::default()
    };
    let store = KVStore::new_with_cache(config, Some(1000));

    // Get non-existent key
    let value = store.get("nonexistent").await.unwrap();
    assert_eq!(value, None);

    let stats = store.get("nonexistent").await.unwrap();
    assert!(stats.is_none());
}

#[tokio::test]
async fn test_kv_cache_invalidation_on_delete() {
    let config = KVConfig {
        max_memory_mb: 100,
        ..Default::default()
    };
    let store = KVStore::new_with_cache(config, Some(1000));

    // Set and cache
    store.set("to_delete", vec![5, 6, 7], None).await.unwrap();

    // Get - populates cache
    store.get("to_delete").await.unwrap();

    // Delete - should invalidate cache
    let deleted = store.delete("to_delete").await.unwrap();
    assert!(deleted);

    // Get again - should not find in cache or storage
    let value = store.get("to_delete").await.unwrap();
    assert_eq!(value, None);
}

#[tokio::test]
async fn test_kv_cache_ttl_expiration() {
    let config = KVConfig {
        max_memory_mb: 100,
        ..Default::default()
    };
    let store = KVStore::new_with_cache(config, Some(1000));

    // Set with very short TTL (1 second)
    store.set("expiring", vec![1, 2, 3], Some(1)).await.unwrap();

    // Get immediately - should work
    let value1 = store.get("expiring").await.unwrap();
    assert!(value1.is_some());

    // Wait for expiration
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    // Get after expiration - should return None
    let value2 = store.get("expiring").await.unwrap();
    assert_eq!(value2, None);
}

#[tokio::test]
async fn test_kv_cache_flushdb_invalidation() {
    let config = KVConfig {
        max_memory_mb: 100,
        allow_flush_commands: true, // Enable FLUSHDB
        ..Default::default()
    };
    let store = KVStore::new_with_cache(config, Some(1000));

    // Set multiple keys
    for i in 1..=10 {
        store
            .set(&format!("key{}", i), vec![i as u8], None)
            .await
            .unwrap();
        store.get(&format!("key{}", i)).await.unwrap(); // Populate cache
    }

    // Flush all
    let flushed = store.flushdb().await.unwrap();
    assert_eq!(flushed, 10);

    // All keys should be gone from cache and storage
    for i in 1..=10 {
        let value = store.get(&format!("key{}", i)).await.unwrap();
        assert_eq!(value, None, "key{} should be flushed", i);
    }
}

#[tokio::test]
async fn test_kv_without_cache() {
    // KV store without cache should work normally
    let config = KVConfig {
        max_memory_mb: 100,
        ..Default::default()
    };
    let store = KVStore::new(config); // No cache

    store.set("key1", vec![1, 2, 3], None).await.unwrap();

    let value = store.get("key1").await.unwrap();
    assert_eq!(value, Some(vec![1, 2, 3]));

    let deleted = store.delete("key1").await.unwrap();
    assert!(deleted);
}
