//! HyperLogLog S2S Integration Tests
//!
//! These tests require a running Synap server.
//! Run with: SYNAP_URL=http://localhost:15500 cargo test --test hyperloglog_s2s_test

mod common;

#[tokio::test]
#[ignore = "requires running Synap server"]
async fn test_hyperloglog_pfadd_pfcount() {
    let client = common::setup_s2s_client();
    let key = format!(
        "test:hll:{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    );

    let added = client
        .hyperloglog()
        .pfadd(&key, ["user:1", "user:2", "user:3"])
        .await
        .unwrap();
    assert!((0..=3).contains(&added));

    let count = client.hyperloglog().pfcount(&key).await.unwrap();
    assert!((2..=4).contains(&count)); // Approximate
}

#[tokio::test]
#[ignore = "requires running Synap server"]
async fn test_hyperloglog_pfmerge() {
    let client = common::setup_s2s_client();
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let key1 = format!("test:hll:merge1:{}", timestamp);
    let key2 = format!("test:hll:merge2:{}", timestamp);
    let dest = format!("test:hll:merge_dest:{}", timestamp);

    client
        .hyperloglog()
        .pfadd(&key1, ["user:1", "user:2", "user:3"])
        .await
        .unwrap();
    client
        .hyperloglog()
        .pfadd(&key2, ["user:4", "user:5", "user:6"])
        .await
        .unwrap();

    let count = client
        .hyperloglog()
        .pfmerge(&dest, &[key1, key2])
        .await
        .unwrap();
    assert!((5..=7).contains(&count)); // Approximate
}

#[tokio::test]
#[ignore = "requires running Synap server"]
async fn test_hyperloglog_stats() {
    let client = common::setup_s2s_client();
    let key = format!(
        "test:hll:stats:{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    );

    client
        .hyperloglog()
        .pfadd(&key, ["user:1", "user:2"])
        .await
        .unwrap();
    client.hyperloglog().pfcount(&key).await.unwrap();

    let stats = client.hyperloglog().stats().await.unwrap();
    assert!(stats.pfadd_count >= 1);
    assert!(stats.pfcount_count >= 1);
}
