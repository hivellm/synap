//! Pub/Sub integration tests

use serde_json::json;
use std::time::{SystemTime, UNIX_EPOCH};
use synap_sdk::{SynapClient, SynapConfig};

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
    let result = client
        .pubsub()
        .publish(&topic, json!("string message"), None, None)
        .await;
    assert!(result.is_ok());

    // Number payload
    let result = client
        .pubsub()
        .publish(&topic, json!(12345), None, None)
        .await;
    assert!(result.is_ok());

    // Object payload
    let result = client
        .pubsub()
        .publish(&topic, json!({"key": "value"}), None, None)
        .await;
    assert!(result.is_ok());

    // Array payload
    let result = client
        .pubsub()
        .publish(&topic, json!([1, 2, 3]), None, None)
        .await;
    assert!(result.is_ok());

    // Null payload
    let result = client
        .pubsub()
        .publish(&topic, json!(null), None, None)
        .await;
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
            client_clone
                .pubsub()
                .publish(&topic_clone, json!({"id": i}), None, None)
                .await
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

#[tokio::test]
async fn test_pubsub_subscribe_topics() {
    let config = SynapConfig::new("http://localhost:15500");
    let client = SynapClient::new(config).expect("Failed to create client");

    let subscriber_id = format!("sub-{}", timestamp_millis());
    let topics = vec!["user.*".to_string(), "order.#".to_string()];

    let result = client
        .pubsub()
        .subscribe_topics(&subscriber_id, topics)
        .await;

    assert!(result.is_ok(), "Subscribe failed: {:?}", result.err());
    // Subscription ID may be empty or present depending on server impl
}

#[tokio::test]
async fn test_pubsub_subscribe_single_topic() {
    let config = SynapConfig::new("http://localhost:15500");
    let client = SynapClient::new(config).expect("Failed to create client");

    let subscriber_id = format!("sub-single-{}", timestamp_millis());
    let topics = vec!["test.topic".to_string()];

    let result = client
        .pubsub()
        .subscribe_topics(&subscriber_id, topics)
        .await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_pubsub_unsubscribe() {
    let config = SynapConfig::new("http://localhost:15500");
    let client = SynapClient::new(config).expect("Failed to create client");

    let subscriber_id = format!("sub-unsub-{}", timestamp_millis());
    let topics = vec!["test.unsub".to_string()];

    // Subscribe first
    let _ = client
        .pubsub()
        .subscribe_topics(&subscriber_id, topics.clone())
        .await;

    // Then unsubscribe
    let result = client.pubsub().unsubscribe(&subscriber_id, topics).await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_pubsub_list_topics() {
    let config = SynapConfig::new("http://localhost:15500");
    let client = SynapClient::new(config).expect("Failed to create client");

    // Publish to ensure there's at least one topic
    let topic = format!("test.list.{}", timestamp_millis());
    let _ = client
        .pubsub()
        .publish(&topic, json!({"test": "data"}), None, None)
        .await;

    let result = client.pubsub().list_topics().await;

    assert!(result.is_ok(), "List topics failed: {:?}", result.err());
    let _topics = result.unwrap();
    // May or may not have topics - just verify the call succeeds
}

#[tokio::test]
async fn test_pubsub_wildcard_subscriptions() {
    let config = SynapConfig::new("http://localhost:15500");
    let client = SynapClient::new(config).expect("Failed to create client");

    let subscriber_id = format!("sub-wildcard-{}", timestamp_millis());

    // Single-level wildcard
    let topics1 = vec!["user.*".to_string()];
    let result1 = client
        .pubsub()
        .subscribe_topics(&subscriber_id, topics1)
        .await;
    assert!(result1.is_ok());

    // Multi-level wildcard
    let subscriber_id2 = format!("sub-wildcard2-{}", timestamp_millis());
    let topics2 = vec!["order.#".to_string()];
    let result2 = client
        .pubsub()
        .subscribe_topics(&subscriber_id2, topics2)
        .await;
    assert!(result2.is_ok());
}

#[tokio::test]
async fn test_pubsub_multiple_subscribers() {
    let config = SynapConfig::new("http://localhost:15500");
    let client = SynapClient::new(config).expect("Failed to create client");

    let topic = format!("test.multi.{}", timestamp_millis());

    // Create multiple subscribers
    for i in 0..3 {
        let subscriber_id = format!("sub-multi-{}-{}", i, timestamp_millis());
        let _ = client
            .pubsub()
            .subscribe_topics(&subscriber_id, vec![topic.clone()])
            .await;
    }

    // Publish to topic
    let result = client
        .pubsub()
        .publish(&topic, json!({"msg": "to all"}), None, None)
        .await;

    assert!(result.is_ok());
    // Note: subscribers_matched may be 0 because subscribers need to be active/polling
}

#[tokio::test]
async fn test_pubsub_publish_with_headers() {
    let config = SynapConfig::new("http://localhost:15500");
    let client = SynapClient::new(config).expect("Failed to create client");

    let topic = format!("test.headers.{}", timestamp_millis());
    let data = json!({"message": "with headers"});

    let mut headers = std::collections::HashMap::new();
    headers.insert("content-type".to_string(), "application/json".to_string());
    headers.insert("x-custom-header".to_string(), "custom-value".to_string());

    let result = client
        .pubsub()
        .publish(&topic, data, None, Some(headers))
        .await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_pubsub_publish_all_priorities() {
    let config = SynapConfig::new("http://localhost:15500");
    let client = SynapClient::new(config).expect("Failed to create client");

    let topic = format!("test.priorities.{}", timestamp_millis());

    // Test all priority levels
    for priority in 0..=9 {
        let result = client
            .pubsub()
            .publish(&topic, json!({"priority": priority}), Some(priority), None)
            .await;
        assert!(result.is_ok(), "Failed for priority {}", priority);
    }
}

#[tokio::test]
async fn test_pubsub_empty_topics_list() {
    let config = SynapConfig::new("http://localhost:15500");
    let client = SynapClient::new(config).expect("Failed to create client");

    let subscriber_id = format!("sub-empty-{}", timestamp_millis());
    let topics: Vec<String> = vec![];

    // Subscribe with empty topics list - should fail or handle gracefully
    let result = client
        .pubsub()
        .subscribe_topics(&subscriber_id, topics)
        .await;

    // Either succeeds with empty subscription or returns error
    // Both are acceptable behaviors
    let _ = result;
}
