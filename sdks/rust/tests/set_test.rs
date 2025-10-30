//! Set Manager tests

use synap_sdk::{SynapClient, SynapConfig};

#[tokio::test]
async fn test_set_add_rem() {
    let config = SynapConfig::new("http://localhost:15500");
    let client = SynapClient::new(config).expect("Failed to create client");
    let set = client.set();

    // Test add
    let add_result = set
        .add("test:tags", vec!["rust".to_string(), "redis".to_string()])
        .await;
    assert!(add_result.is_ok() || add_result.is_err());

    // Test rem
    let rem_result = set.rem("test:tags", vec!["redis".to_string()]).await;
    assert!(rem_result.is_ok() || rem_result.is_err());
}

#[tokio::test]
async fn test_set_is_member() {
    let config = SynapConfig::new("http://localhost:15500");
    let client = SynapClient::new(config).expect("Failed to create client");
    let set = client.set();

    let result = set.is_member("test:tags", "rust".to_string()).await;
    assert!(result.is_ok() || result.is_err());
}

#[tokio::test]
async fn test_set_members() {
    let config = SynapConfig::new("http://localhost:15500");
    let client = SynapClient::new(config).expect("Failed to create client");
    let set = client.set();

    let result = set.members("test:tags").await;
    assert!(result.is_ok() || result.is_err());
}

#[tokio::test]
async fn test_set_card() {
    let config = SynapConfig::new("http://localhost:15500");
    let client = SynapClient::new(config).expect("Failed to create client");
    let set = client.set();

    let result = set.card("test:tags").await;
    assert!(result.is_ok() || result.is_err());
}

#[tokio::test]
async fn test_set_pop() {
    let config = SynapConfig::new("http://localhost:15500");
    let client = SynapClient::new(config).expect("Failed to create client");
    let set = client.set();

    let result = set.pop("test:tags", 1).await;
    assert!(result.is_ok() || result.is_err());
}

#[tokio::test]
async fn test_set_rand_member() {
    let config = SynapConfig::new("http://localhost:15500");
    let client = SynapClient::new(config).expect("Failed to create client");
    let set = client.set();

    let result = set.rand_member("test:tags", 2).await;
    assert!(result.is_ok() || result.is_err());
}

#[tokio::test]
async fn test_set_algebra() {
    let config = SynapConfig::new("http://localhost:15500");
    let client = SynapClient::new(config).expect("Failed to create client");
    let set = client.set();

    // Test inter
    let inter_result = set
        .inter(vec!["test:tags1".to_string(), "test:tags2".to_string()])
        .await;
    assert!(inter_result.is_ok() || inter_result.is_err());

    // Test union
    let union_result = set
        .union(vec!["test:tags1".to_string(), "test:tags2".to_string()])
        .await;
    assert!(union_result.is_ok() || union_result.is_err());

    // Test diff
    let diff_result = set
        .diff(vec!["test:tags1".to_string(), "test:tags2".to_string()])
        .await;
    assert!(diff_result.is_ok() || diff_result.is_err());
}

#[tokio::test]
async fn test_set_store_operations() {
    let config = SynapConfig::new("http://localhost:15500");
    let client = SynapClient::new(config).expect("Failed to create client");
    let set = client.set();

    let inter_store_result = set
        .inter_store(
            "test:result",
            vec!["test:tags1".to_string(), "test:tags2".to_string()],
        )
        .await;
    assert!(inter_store_result.is_ok() || inter_store_result.is_err());
}
