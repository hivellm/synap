//! Bitmap S2S Integration Tests
//!
//! These tests require a running Synap server.
//! Run with: SYNAP_URL=http://localhost:15500 cargo test --test bitmap_s2s_test

mod common;

#[tokio::test]
#[ignore = "requires running Synap server"]
async fn test_bitmap_setbit_getbit() {
    let client = common::setup_s2s_client();
    let key = format!(
        "test:bitmap:{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    );

    // Set bit 5 to 1
    let old_value = client.bitmap().setbit(&key, 5, 1).await.unwrap();
    assert_eq!(old_value, 0); // Was unset before

    // Get bit 5
    let value = client.bitmap().getbit(&key, 5).await.unwrap();
    assert_eq!(value, 1);

    // Set bit 5 back to 0
    let old_value2 = client.bitmap().setbit(&key, 5, 0).await.unwrap();
    assert_eq!(old_value2, 1); // Was set before

    // Get bit 5 again
    let value2 = client.bitmap().getbit(&key, 5).await.unwrap();
    assert_eq!(value2, 0);
}

#[tokio::test]
#[ignore = "requires running Synap server"]
async fn test_bitmap_bitcount() {
    let client = common::setup_s2s_client();
    let key = format!(
        "test:bitmap:count:{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    );

    // Set multiple bits
    client.bitmap().setbit(&key, 0, 1).await.unwrap();
    client.bitmap().setbit(&key, 2, 1).await.unwrap();
    client.bitmap().setbit(&key, 4, 1).await.unwrap();
    client.bitmap().setbit(&key, 6, 1).await.unwrap();

    // Count all bits
    let count = client.bitmap().bitcount(&key, None, None).await.unwrap();
    assert_eq!(count, 4);
}

#[tokio::test]
#[ignore = "requires running Synap server"]
async fn test_bitmap_bitpos() {
    let client = common::setup_s2s_client();
    let key = format!(
        "test:bitmap:pos:{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    );

    // Set bit at position 7
    client.bitmap().setbit(&key, 7, 1).await.unwrap();

    // Find first set bit
    let pos = client.bitmap().bitpos(&key, 1, None, None).await.unwrap();
    assert_eq!(pos, Some(7));
}

#[tokio::test]
#[ignore = "requires running Synap server"]
async fn test_bitmap_bitop_and() {
    let client = common::setup_s2s_client();
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let key1 = format!("test:bitmap:and1:{}", timestamp);
    let key2 = format!("test:bitmap:and2:{}", timestamp);
    let dest = format!("test:bitmap:and_result:{}", timestamp);

    // Set bits in bitmap1 (bits 0, 1, 2)
    client.bitmap().setbit(&key1, 0, 1).await.unwrap();
    client.bitmap().setbit(&key1, 1, 1).await.unwrap();
    client.bitmap().setbit(&key1, 2, 1).await.unwrap();

    // Set bits in bitmap2 (bits 1, 2, 3)
    client.bitmap().setbit(&key2, 1, 1).await.unwrap();
    client.bitmap().setbit(&key2, 2, 1).await.unwrap();
    client.bitmap().setbit(&key2, 3, 1).await.unwrap();

    // AND operation
    let length = client
        .bitmap()
        .bitop(
            synap_sdk::BitmapOperation::And,
            &dest,
            &[key1.clone(), key2.clone()],
        )
        .await
        .unwrap();
    assert!(length > 0);

    // Check result: should have bits 1 and 2 set
    assert_eq!(client.bitmap().getbit(&dest, 0).await.unwrap(), 0);
    assert_eq!(client.bitmap().getbit(&dest, 1).await.unwrap(), 1);
    assert_eq!(client.bitmap().getbit(&dest, 2).await.unwrap(), 1);
    assert_eq!(client.bitmap().getbit(&dest, 3).await.unwrap(), 0);
}

#[tokio::test]
#[ignore = "requires running Synap server"]
async fn test_bitmap_bitfield_get_set() {
    let client = common::setup_s2s_client();
    let key = format!(
        "test:bitmap:bitfield:{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    );

    // SET operation: Set 8-bit unsigned value 42 at offset 0
    let operations = vec![serde_json::json!({
        "operation": "SET",
        "offset": 0,
        "width": 8,
        "signed": false,
        "value": 42
    })];

    let results = client.bitmap().bitfield(&key, &operations).await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0], 0); // Old value was 0

    // GET operation: Read back the value
    let operations = vec![serde_json::json!({
        "operation": "GET",
        "offset": 0,
        "width": 8,
        "signed": false
    })];

    let results = client.bitmap().bitfield(&key, &operations).await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0], 42);
}

#[tokio::test]
#[ignore = "requires running Synap server"]
async fn test_bitmap_bitfield_incrby_wrap() {
    let client = common::setup_s2s_client();
    let key = format!(
        "test:bitmap:bitfield_wrap:{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    );

    // Set initial value
    let operations = vec![serde_json::json!({
        "operation": "SET",
        "offset": 0,
        "width": 8,
        "signed": false,
        "value": 250
    })];
    client.bitmap().bitfield(&key, &operations).await.unwrap();

    // INCRBY with wrap: 250 + 10 = 260 wraps to 4
    let operations = vec![serde_json::json!({
        "operation": "INCRBY",
        "offset": 0,
        "width": 8,
        "signed": false,
        "increment": 10,
        "overflow": "WRAP"
    })];

    let results = client.bitmap().bitfield(&key, &operations).await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0], 4); // 250 + 10 = 260 wraps to 4 (260 - 256)
}

#[tokio::test]
#[ignore = "requires running Synap server"]
async fn test_bitmap_bitfield_incrby_sat() {
    let client = common::setup_s2s_client();
    let key = format!(
        "test:bitmap:bitfield_sat:{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    );

    // Set 4-bit unsigned value to 14
    let operations = vec![serde_json::json!({
        "operation": "SET",
        "offset": 0,
        "width": 4,
        "signed": false,
        "value": 14
    })];
    client.bitmap().bitfield(&key, &operations).await.unwrap();

    // INCRBY with saturate: 14 + 1 = 15 (max), then stays at 15
    let operations = vec![serde_json::json!({
        "operation": "INCRBY",
        "offset": 0,
        "width": 4,
        "signed": false,
        "increment": 1,
        "overflow": "SAT"
    })];

    let results1 = client.bitmap().bitfield(&key, &operations).await.unwrap();
    assert_eq!(results1[0], 15);

    // Try to increment again (should saturate at 15)
    let results2 = client.bitmap().bitfield(&key, &operations).await.unwrap();
    assert_eq!(results2[0], 15);
}

#[tokio::test]
#[ignore = "requires running Synap server"]
async fn test_bitmap_bitfield_multiple_operations() {
    let client = common::setup_s2s_client();
    let key = format!(
        "test:bitmap:bitfield_multi:{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    );

    // Execute multiple operations in sequence
    let operations = vec![
        serde_json::json!({
            "operation": "SET",
            "offset": 0,
            "width": 8,
            "signed": false,
            "value": 100
        }),
        serde_json::json!({
            "operation": "SET",
            "offset": 8,
            "width": 8,
            "signed": false,
            "value": 200
        }),
        serde_json::json!({
            "operation": "GET",
            "offset": 0,
            "width": 8,
            "signed": false
        }),
        serde_json::json!({
            "operation": "GET",
            "offset": 8,
            "width": 8,
            "signed": false
        }),
        serde_json::json!({
            "operation": "INCRBY",
            "offset": 0,
            "width": 8,
            "signed": false,
            "increment": 50,
            "overflow": "WRAP"
        }),
    ];

    let results = client.bitmap().bitfield(&key, &operations).await.unwrap();
    assert_eq!(results.len(), 5);
    assert_eq!(results[0], 0); // Old value at offset 0
    assert_eq!(results[1], 0); // Old value at offset 8
    assert_eq!(results[2], 100); // Read back offset 0
    assert_eq!(results[3], 200); // Read back offset 8
    assert_eq!(results[4], 150); // Incremented offset 0
}

#[tokio::test]
#[ignore = "requires running Synap server"]
async fn test_bitmap_bitfield_signed_values() {
    let client = common::setup_s2s_client();
    let key = format!(
        "test:bitmap:bitfield_signed:{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    );

    // Set signed 8-bit negative value
    let operations = vec![serde_json::json!({
        "operation": "SET",
        "offset": 0,
        "width": 8,
        "signed": true,
        "value": -10
    })];
    client.bitmap().bitfield(&key, &operations).await.unwrap();

    // Read back as signed
    let operations = vec![serde_json::json!({
        "operation": "GET",
        "offset": 0,
        "width": 8,
        "signed": true
    })];

    let results = client.bitmap().bitfield(&key, &operations).await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0], -10);
}

#[tokio::test]
#[ignore = "requires running Synap server"]
async fn test_bitmap_stats() {
    let client = common::setup_s2s_client();
    let key = format!(
        "test:bitmap:stats:{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    );

    // Perform some operations
    client.bitmap().setbit(&key, 0, 1).await.unwrap();
    client.bitmap().getbit(&key, 0).await.unwrap();
    client.bitmap().bitcount(&key, None, None).await.unwrap();

    let stats = client.bitmap().stats().await.unwrap();
    assert!(stats.setbit_count >= 1);
    assert!(stats.getbit_count >= 1);
    assert!(stats.bitcount_count >= 1);
}
