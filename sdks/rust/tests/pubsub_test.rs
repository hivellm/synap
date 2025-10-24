//! Pub/Sub integration tests

use serde_json::json;
use synap_sdk::{SynapClient, SynapConfig};
use std::time::{SystemTime, UNIX_EPOCH};

fn timestamp_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis()
}

#[tokio::test]
async fn test_pubsub_publish_correct_payload_field() {
    let config = SynapConfig::new("http://localhost:15500");
    let client = SynapClient::new(config).expect("Failed to create client");

    let topic = format!("test.topic.{}", timestamp_millis());
    let data = json!({"event": "test", "data": "test-data"});

    // This test will verify that the SDK sends the correct "payload" field
    // If it sends "data" instead, the server will reject it
    let result = client.pubsub().publish(&topic, data, None, None).await;

    // Should succeed even with 0 subscribers
    assert!(result.is_ok(), "Publish failed: {:?}", result.err());
}

#[tokio::test]
async fn test_pubsub_publish_different_types() {
    let config = SynapConfig::new("http://localhost:15500");
    let client = SynapClient::new(config).expect("Failed to create client");

    let topic = format!("test.types.{}", timestamp_millis());

    // String payload
    let result = client.pubsub().publish(&topic, json!("string message"), None, None).await;
    assert!(result.is_ok());

    // Number payload
    let result = client.pubsub().publish(&topic, json!(12345), None, None).await;
    assert!(result.is_ok());

    // Object payload
    let result = client.pubsub().publish(&topic, json!({"key": "value"}), None, None).await;
    assert!(result.is_ok());

    // Array payload
    let result = client.pubsub().publish(&topic, json!([1, 2, 3]), None, None).await;
    assert!(result.is_ok());

    // Null payload
    let result = client.pubsub().publish(&topic, json!(null), None, None).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_pubsub_publish_with_priority() {
    let config = SynapConfig::new("http://localhost:15500");
    let client = SynapClient::new(config).expect("Failed to create client");

    let topic = format!("test.priority.{}", timestamp_millis());
    let data = json!({"message": "high priority"});

    let result = client.pubsub().publish(&topic, data, Some(9), None).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_pubsub_rapid_publishing() {
    let config = SynapConfig::new("http://localhost:15500");
    let client = SynapClient::new(config).expect("Failed to create client");

    let topic = format!("test.rapid.{}", timestamp_millis());

    // Publish 50 messages rapidly
    let mut handles = vec![];
    for i in 0..50 {
        let client_clone = client.clone();
        let topic_clone = topic.clone();
        
        let handle = tokio::spawn(async move {
            client_clone.pubsub().publish(
                &topic_clone,
                json!({"id": i}),
                None,
                None
            ).await
        });
        
        handles.push(handle);
    }

    // Wait for all to complete
    for handle in handles {
        let result = handle.await.expect("Task failed");
        assert!(result.is_ok());
    }
}

#[tokio::test]
async fn test_pubsub_large_payload() {
    let config = SynapConfig::new("http://localhost:15500");
    let client = SynapClient::new(config).expect("Failed to create client");

    let topic = format!("test.large.{}", timestamp_millis());
    let large_data = "x".repeat(50000); // 50KB
    let message = json!({"data": large_data});

    let result = client.pubsub().publish(&topic, message, None, None).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_pubsub_nested_objects() {
    let config = SynapConfig::new("http://localhost:15500");
    let client = SynapClient::new(config).expect("Failed to create client");

    let topic = format!("test.nested.{}", timestamp_millis());
    let message = json!({
        "user": {
            "id": 123,
            "profile": {
                "name": "Alice",
                "settings": {
                    "theme": "dark",
                    "notifications": true
                }
            }
        },
        "timestamp": timestamp_millis()
    });

    let result = client.pubsub().publish(&topic, message, None, None).await;
    assert!(result.is_ok());
}
