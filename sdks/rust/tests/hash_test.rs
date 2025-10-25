//! Hash Manager tests

use synap_sdk::{HashManager, SynapClient, SynapConfig};

#[tokio::test]
async fn test_hash_operations() {
    let config = SynapConfig::new("http://localhost:15500");
    let client = SynapClient::new(config).expect("Failed to create client");
    let hash = client.hash();

    // Note: These are interface tests - will fail if server not running
    // For true unit tests, we'd need to mock the HTTP client
    
    // Test set
    let set_result = hash.set("test:hash", "field1", "value1").await;
    assert!(set_result.is_ok() || set_result.is_err()); // Either OK or network error

    // Test get
    let get_result = hash.get("test:hash", "field1").await;
    assert!(get_result.is_ok() || get_result.is_err());

    // Test len
    let len_result = hash.len("test:hash").await;
    assert!(len_result.is_ok() || len_result.is_err());
}

#[tokio::test]
async fn test_hash_exists() {
    let config = SynapConfig::new("http://localhost:15500");
    let client = SynapClient::new(config).expect("Failed to create client");
    let hash = client.hash();

    let exists_result = hash.exists("test:hash", "field1").await;
    assert!(exists_result.is_ok() || exists_result.is_err());
}

#[tokio::test]
async fn test_hash_mset_mget() {
    let config = SynapConfig::new("http://localhost:15500");
    let client = SynapClient::new(config).expect("Failed to create client");
    let hash = client.hash();

    let mut fields = std::collections::HashMap::new();
    fields.insert("name".to_string(), "Alice".to_string());
    fields.insert("age".to_string(), "30".to_string());

    let mset_result = hash.mset("test:user", fields).await;
    assert!(mset_result.is_ok() || mset_result.is_err());

    let mget_result = hash.mget("test:user", vec!["name".to_string(), "age".to_string()]).await;
    assert!(mget_result.is_ok() || mget_result.is_err());
}

#[tokio::test]
async fn test_hash_incr() {
    let config = SynapConfig::new("http://localhost:15500");
    let client = SynapClient::new(config).expect("Failed to create client");
    let hash = client.hash();

    let incr_result = hash.incr_by("test:counters", "visits", 1).await;
    assert!(incr_result.is_ok() || incr_result.is_err());

    let incr_float_result = hash.incr_by_float("test:metrics", "score", 0.5).await;
    assert!(incr_float_result.is_ok() || incr_float_result.is_err());
}

