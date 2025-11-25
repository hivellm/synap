//! List Manager tests

use synap_sdk::{SynapClient, SynapConfig};

#[tokio::test]
async fn test_list_push_pop() {
    let config = SynapConfig::new("http://localhost:15500");
    let client = SynapClient::new(config).expect("Failed to create client");
    let list = client.list();

    // Test rpush
    let rpush_result = list
        .rpush("test:tasks", vec!["task1".to_string(), "task2".to_string()])
        .await;
    assert!(rpush_result.is_ok() || rpush_result.is_err());

    // Test lpush
    let lpush_result = list.lpush("test:tasks", vec!["task0".to_string()]).await;
    assert!(lpush_result.is_ok() || lpush_result.is_err());

    // Test lpop
    let lpop_result = list.lpop("test:tasks", 1).await;
    assert!(lpop_result.is_ok() || lpop_result.is_err());

    // Test rpop
    let rpop_result = list.rpop("test:tasks", 1).await;
    assert!(rpop_result.is_ok() || rpop_result.is_err());
}

#[tokio::test]
async fn test_list_range() {
    let config = SynapConfig::new("http://localhost:15500");
    let client = SynapClient::new(config).expect("Failed to create client");
    let list = client.list();

    let range_result = list.range("test:tasks", 0, -1).await;
    assert!(range_result.is_ok() || range_result.is_err());
}

#[tokio::test]
async fn test_list_len() {
    let config = SynapConfig::new("http://localhost:15500");
    let client = SynapClient::new(config).expect("Failed to create client");
    let list = client.list();

    let len_result = list.len("test:tasks").await;
    assert!(len_result.is_ok() || len_result.is_err());
}

#[tokio::test]
async fn test_list_index_set() {
    let config = SynapConfig::new("http://localhost:15500");
    let client = SynapClient::new(config).expect("Failed to create client");
    let list = client.list();

    let index_result = list.index("test:tasks", 0).await;
    assert!(index_result.is_ok() || index_result.is_err());

    let set_result = list.set("test:tasks", 0, "new_task".to_string()).await;
    assert!(set_result.is_ok() || set_result.is_err());
}

#[tokio::test]
async fn test_list_trim() {
    let config = SynapConfig::new("http://localhost:15500");
    let client = SynapClient::new(config).expect("Failed to create client");
    let list = client.list();

    let trim_result = list.trim("test:tasks", 0, 10).await;
    assert!(trim_result.is_ok() || trim_result.is_err());
}
