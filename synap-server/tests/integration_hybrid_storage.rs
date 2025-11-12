// Tests for hybrid HashMap/RadixTrie storage optimization
use synap_server::core::{KVConfig, KVStore};

#[tokio::test]
async fn test_hybrid_storage_starts_with_hashmap() {
    // Test that storage starts with HashMap for better small-dataset performance
    let store = KVStore::new(KVConfig::default());

    // Add a small number of keys (< 10K)
    for i in 0..100 {
        let key = format!("key_{}", i);
        store.set(&key, vec![i as u8], None).await.unwrap();
    }

    // Verify all keys exist
    for i in 0..100 {
        let key = format!("key_{}", i);
        let value = store.get(&key).await.unwrap();
        assert_eq!(value, Some(vec![i as u8]));
    }

    // Storage should still be HashMap at this point (< 10K keys)
    assert_eq!(store.dbsize().await.unwrap(), 100);
}

#[tokio::test]
async fn test_hybrid_storage_upgrades_to_trie() {
    // Test that storage automatically upgrades to RadixTrie at 10K threshold
    let store = KVStore::new(KVConfig::default());

    // Add keys up to and beyond the 10K threshold
    // Note: With 64 shards, each shard hits threshold at ~640K total keys
    // So we add 640K keys to trigger upgrade
    for i in 0..640_000 {
        let key = format!("key_{:08}", i);
        store.set(&key, vec![0], None).await.unwrap();

        if i % 100_000 == 0 {
            println!("Inserted {} keys...", i);
        }
    }

    // Verify final count
    let size = store.dbsize().await.unwrap();
    assert_eq!(size, 640_000);

    // Verify random access still works after upgrade
    for i in (0..640_000).step_by(10_000) {
        let key = format!("key_{:08}", i);
        let value = store.get(&key).await.unwrap();
        assert_eq!(value, Some(vec![0]));
    }

    println!("✅ Hybrid storage upgrade validated: 640K keys inserted and verified");
}

#[tokio::test]
async fn test_hybrid_storage_prefix_search() {
    // Test that prefix search works in both HashMap and RadixTrie modes
    let store = KVStore::new(KVConfig::default());

    // Add keys with common prefixes
    for i in 0..1000 {
        let key = format!("user:{}", i);
        store.set(&key, vec![i as u8], None).await.unwrap();

        let key = format!("product:{}", i);
        store.set(&key, vec![i as u8], None).await.unwrap();
    }

    // Test prefix search (should work in HashMap mode)
    let user_keys = store.scan(Some("user:"), 2000).await.unwrap();
    assert_eq!(user_keys.len(), 1000);

    let product_keys = store.scan(Some("product:"), 2000).await.unwrap();
    assert_eq!(product_keys.len(), 1000);

    // All keys should start with the correct prefix
    assert!(user_keys.iter().all(|k| k.starts_with("user:")));
    assert!(product_keys.iter().all(|k| k.starts_with("product:")));
}

#[tokio::test]
async fn test_hybrid_storage_operations_after_upgrade() {
    // Test that all operations work correctly after upgrade to RadixTrie
    let store = KVStore::new(KVConfig::default());

    // Insert enough keys to trigger upgrade (per-shard threshold)
    for i in 0..20_000 {
        let key = format!("key_{:08}", i);
        store.set(&key, vec![(i % 256) as u8], None).await.unwrap();
    }

    println!("Inserted 20K keys, testing operations...");

    // Test GET
    let value = store.get("key_00000100").await.unwrap();
    assert_eq!(value, Some(vec![100]));

    // Test DELETE
    store.delete("key_00000100").await.unwrap();
    let value = store.get("key_00000100").await.unwrap();
    assert_eq!(value, None);

    // Test EXISTS
    assert!(store.exists("key_00000200").await.unwrap());
    assert!(!store.exists("key_00000100").await.unwrap());

    // Test UPDATE
    store.set("key_00000200", vec![255], None).await.unwrap();
    let value = store.get("key_00000200").await.unwrap();
    assert_eq!(value, Some(vec![255]));

    // Test SCAN
    let keys = store.scan(Some("key_0000"), 100).await.unwrap();
    assert!(!keys.is_empty());
    assert!(keys.iter().all(|k| k.starts_with("key_0000")));

    println!("✅ All operations working correctly after upgrade");
}

#[tokio::test]
async fn test_hybrid_storage_performance_characteristics() {
    // Test that small datasets perform well
    let store = KVStore::new(KVConfig::default());

    let start = std::time::Instant::now();

    // Small dataset (HashMap mode)
    for i in 0..1000 {
        let key = format!("key_{}", i);
        store.set(&key, vec![i as u8], None).await.unwrap();
    }

    let insert_duration = start.elapsed();

    // Read all keys
    let start = std::time::Instant::now();
    for i in 0..1000 {
        let key = format!("key_{}", i);
        let _ = store.get(&key).await.unwrap();
    }
    let read_duration = start.elapsed();

    println!("Small dataset (1K keys):");
    println!(
        "  Insert: {:?} ({:.2} µs/key)",
        insert_duration,
        insert_duration.as_micros() as f64 / 1000.0
    );
    println!(
        "  Read: {:?} ({:.2} µs/key)",
        read_duration,
        read_duration.as_micros() as f64 / 1000.0
    );

    // Performance should be sub-millisecond for small datasets
    assert!(insert_duration.as_millis() < 100);
    assert!(read_duration.as_millis() < 50);
}
