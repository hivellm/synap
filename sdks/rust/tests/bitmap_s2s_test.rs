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
