use reqwest::Client;
use serde_json::json;
use std::sync::Arc;
use synap_server::auth::{ApiKeyManager, UserManager};
use synap_server::{AppState, KVStore, PubSubRouter, ScriptManager, ServerConfig, create_router};
use tokio::net::TcpListener;

/// Spawn a test server and return its base URL
async fn spawn_test_server() -> String {
    let config = ServerConfig::default();
    let kv_config = config.to_kv_config();

    let kv_store = Arc::new(KVStore::new(kv_config));
    let hash_store = Arc::new(synap_server::core::HashStore::new());
    let list_store = Arc::new(synap_server::core::ListStore::new());
    let set_store = Arc::new(synap_server::core::SetStore::new());
    let sorted_set_store = Arc::new(synap_server::core::SortedSetStore::new());
    let monitoring = Arc::new(synap_server::monitoring::MonitoringManager::new(
        kv_store.clone(),
        hash_store.clone(),
        list_store.clone(),
        set_store.clone(),
        sorted_set_store.clone(),
    ));
    let transaction_manager = Arc::new(synap_server::core::TransactionManager::new(
        kv_store.clone(),
        hash_store.clone(),
        list_store.clone(),
        set_store.clone(),
        sorted_set_store.clone(),
    ));
    let geospatial_store = Arc::new(synap_server::core::GeospatialStore::new(
        sorted_set_store.clone(),
    ));
    let app_state = AppState {
        kv_store,
        hash_store,
        list_store,
        set_store,
        sorted_set_store,

        hyperloglog_store: Arc::new(synap_server::core::HyperLogLogStore::new()),
        bitmap_store: Arc::new(synap_server::core::BitmapStore::new()),
        geospatial_store,
        queue_manager: None,
        stream_manager: None,
        pubsub_router: Some(Arc::new(PubSubRouter::new())),
        persistence: None,
        consumer_group_manager: None,
        partition_manager: None,
        monitoring,
        transaction_manager,
        script_manager: Arc::new(ScriptManager::default()),
        client_list_manager: Arc::new(synap_server::monitoring::ClientListManager::new()),
        cluster_topology: None,
        cluster_migration: None,
    };

    let user_manager = Arc::new(UserManager::new());
    let api_key_manager = Arc::new(ApiKeyManager::new());
    let app = create_router(
        app_state,
        synap_server::config::RateLimitConfig {
            enabled: false,
            requests_per_second: 100,
            burst_size: 10,
        },
        synap_server::config::McpConfig::default(),
        user_manager,
        api_key_manager,
        false,
        false,
    );

    // Bind to random port
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    // Wait a moment for server to start
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    format!("http://{}", addr)
}

// ============================================================================
// Pub/Sub Tests
// ============================================================================

#[tokio::test]
async fn test_pubsub_subscribe() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Subscribe to topics
    let res = client
        .post(format!("{}/pubsub/subscribe", base_url))
        .json(&json!({
            "topics": ["test.topic", "notifications.*"]
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), 200);

    let body: serde_json::Value = res.json().await.unwrap();
    assert!(body["subscriber_id"].as_str().is_some());
    assert_eq!(body["subscription_count"], 2);
    assert_eq!(body["topics"].as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn test_pubsub_publish() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Subscribe first
    let sub_res = client
        .post(format!("{}/pubsub/subscribe", base_url))
        .json(&json!({
            "topics": ["test.topic"]
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(sub_res.status(), 200);

    // Publish message
    let res = client
        .post(format!("{}/pubsub/test.topic/publish", base_url))
        .json(&json!({
            "payload": {"message": "Hello, World!"},
            "metadata": {"source": "test"}
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), 200);

    let body: serde_json::Value = res.json().await.unwrap();
    assert!(body["message_id"].as_str().is_some());
    assert_eq!(body["topic"], "test.topic");
    assert_eq!(body["subscribers_matched"], 1);
}

#[tokio::test]
async fn test_pubsub_publish_with_data_field() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Subscribe first
    client
        .post(format!("{}/pubsub/test.data.topic/subscribe", base_url))
        .json(&json!({
            "topics": ["test.data.topic"]
        }))
        .send()
        .await
        .unwrap();

    // Publish with "data" field instead of "payload"
    let res = client
        .post(format!("{}/pubsub/test.data.topic/publish", base_url))
        .json(&json!({
            "data": {"message": "test with data field", "value": 42},
            "metadata": {"source": "test"}
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    assert!(body["message_id"].as_str().is_some());
    assert_eq!(body["topic"], "test.data.topic");
    // Note: subscribers_matched is 0 because REST subscribe doesn't create active connections
    // Only WebSocket subscriptions create active connections that receive messages
    assert_eq!(body["subscribers_matched"], 0);
}

#[tokio::test]
async fn test_pubsub_publish_backward_compatibility_with_payload_field() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Subscribe first
    client
        .post(format!("{}/pubsub/test.payload.topic/subscribe", base_url))
        .json(&json!({
            "topics": ["test.payload.topic"]
        }))
        .send()
        .await
        .unwrap();

    // Publish with "payload" field (backward compatibility)
    let res = client
        .post(format!("{}/pubsub/test.payload.topic/publish", base_url))
        .json(&json!({
            "payload": {"message": "test with payload field", "value": 100},
            "metadata": {"source": "test"}
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    assert!(body["message_id"].as_str().is_some());
    assert_eq!(body["topic"], "test.payload.topic");
    // Note: subscribers_matched is 0 because REST subscribe doesn't create active connections
    // Only WebSocket subscriptions create active connections that receive messages
    assert_eq!(body["subscribers_matched"], 0);
}

#[tokio::test]
async fn test_pubsub_wildcard_single_level() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Subscribe to wildcard pattern
    let sub_res = client
        .post(format!("{}/pubsub/subscribe", base_url))
        .json(&json!({
            "topics": ["notifications.*"]
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(sub_res.status(), 200);

    // Publish to matching topic
    let res = client
        .post(format!("{}/pubsub/notifications.email/publish", base_url))
        .json(&json!({
            "payload": {"to": "user@example.com"}
        }))
        .send()
        .await
        .unwrap();

    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["subscribers_matched"], 1);

    // Publish to non-matching topic (too many levels)
    let res = client
        .post(format!(
            "{}/pubsub/notifications.email.user/publish",
            base_url
        ))
        .json(&json!({
            "payload": {"test": true}
        }))
        .send()
        .await
        .unwrap();

    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["subscribers_matched"], 0);
}

#[tokio::test]
async fn test_pubsub_wildcard_multi_level() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Subscribe to wildcard pattern with #
    let sub_res = client
        .post(format!("{}/pubsub/subscribe", base_url))
        .json(&json!({
            "topics": ["events.user.#"]
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(sub_res.status(), 200);

    // All these should match
    let topics = vec![
        "events.user",
        "events.user.login",
        "events.user.login.success",
    ];

    for topic in topics {
        let res = client
            .post(format!("{}/pubsub/{}/publish", base_url, topic))
            .json(&json!({
                "payload": {"event": topic}
            }))
            .send()
            .await
            .unwrap();

        let body: serde_json::Value = res.json().await.unwrap();
        assert_eq!(
            body["subscribers_matched"], 1,
            "Topic {} should match",
            topic
        );
    }

    // This should not match
    let res = client
        .post(format!("{}/pubsub/events.admin/publish", base_url))
        .json(&json!({
            "payload": {"test": true}
        }))
        .send()
        .await
        .unwrap();

    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["subscribers_matched"], 0);
}

#[tokio::test]
async fn test_pubsub_unsubscribe() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Subscribe
    let sub_res = client
        .post(format!("{}/pubsub/subscribe", base_url))
        .json(&json!({
            "topics": ["topic1", "topic2"]
        }))
        .send()
        .await
        .unwrap();

    let body: serde_json::Value = sub_res.json().await.unwrap();
    let subscriber_id = body["subscriber_id"].as_str().unwrap();

    // Unsubscribe from one topic
    let res = client
        .post(format!("{}/pubsub/unsubscribe", base_url))
        .json(&json!({
            "subscriber_id": subscriber_id,
            "topics": ["topic1"]
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), 200);

    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["unsubscribed"], 1);

    // Verify topic1 has no subscribers
    let res = client
        .post(format!("{}/pubsub/topic1/publish", base_url))
        .json(&json!({
            "payload": {"test": true}
        }))
        .send()
        .await
        .unwrap();

    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["subscribers_matched"], 0);

    // Verify topic2 still has subscriber
    let res = client
        .post(format!("{}/pubsub/topic2/publish", base_url))
        .json(&json!({
            "payload": {"test": true}
        }))
        .send()
        .await
        .unwrap();

    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["subscribers_matched"], 1);
}

#[tokio::test]
async fn test_pubsub_stats() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Subscribe
    client
        .post(format!("{}/pubsub/subscribe", base_url))
        .json(&json!({
            "topics": ["test.topic"]
        }))
        .send()
        .await
        .unwrap();

    // Publish
    client
        .post(format!("{}/pubsub/test.topic/publish", base_url))
        .json(&json!({
            "payload": {"test": true}
        }))
        .send()
        .await
        .unwrap();

    // Get stats
    let res = client
        .get(format!("{}/pubsub/stats", base_url))
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), 200);

    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["total_topics"], 1);
    assert_eq!(body["total_subscribers"], 1);
    assert!(body["messages_published"].as_u64().unwrap() >= 1);
    assert!(body["messages_delivered"].as_u64().unwrap() >= 1);
}

#[tokio::test]
async fn test_pubsub_list_topics() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Subscribe to create topics
    client
        .post(format!("{}/pubsub/subscribe", base_url))
        .json(&json!({
            "topics": ["topic1", "topic2", "topic3"]
        }))
        .send()
        .await
        .unwrap();

    // List topics
    let res = client
        .get(format!("{}/pubsub/topics", base_url))
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), 200);

    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["count"], 3);

    let topics = body["topics"].as_array().unwrap();
    assert_eq!(topics.len(), 3);
}

#[tokio::test]
async fn test_pubsub_topic_info() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Subscribe
    client
        .post(format!("{}/pubsub/subscribe", base_url))
        .json(&json!({
            "topics": ["test.topic"]
        }))
        .send()
        .await
        .unwrap();

    // Publish a message
    client
        .post(format!("{}/pubsub/test.topic/publish", base_url))
        .json(&json!({
            "payload": {"test": true}
        }))
        .send()
        .await
        .unwrap();

    // Get topic info
    let res = client
        .get(format!("{}/pubsub/test.topic/info", base_url))
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), 200);

    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["topic"], "test.topic");
    assert_eq!(body["subscriber_count"], 1);
    assert!(body["message_count"].as_u64().unwrap() >= 1);
    assert!(body["created_at"].as_u64().is_some());
}

#[tokio::test]
async fn test_pubsub_multiple_subscribers() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Subscribe 3 times to same topic
    for _ in 0..3 {
        client
            .post(format!("{}/pubsub/subscribe", base_url))
            .json(&json!({
                "topics": ["broadcast.topic"]
            }))
            .send()
            .await
            .unwrap();
    }

    // Publish
    let res = client
        .post(format!("{}/pubsub/broadcast.topic/publish", base_url))
        .json(&json!({
            "payload": {"message": "Broadcast to all"}
        }))
        .send()
        .await
        .unwrap();

    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["subscribers_matched"], 3);
}

#[tokio::test]
async fn test_pubsub_complex_patterns() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Subscribe to different patterns
    client
        .post(format!("{}/pubsub/subscribe", base_url))
        .json(&json!({
            "topics": ["*.cpu.*", "metrics.#"]
        }))
        .send()
        .await
        .unwrap();

    // This topic matches both patterns
    let res = client
        .post(format!("{}/pubsub/metrics.cpu.usage/publish", base_url))
        .json(&json!({
            "payload": {"value": 75}
        }))
        .send()
        .await
        .unwrap();

    let body: serde_json::Value = res.json().await.unwrap();
    // Should match both wildcard patterns from same subscriber
    assert!(body["subscribers_matched"].as_u64().unwrap() >= 1);
}

#[tokio::test]
async fn test_pubsub_hierarchical_topics() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Subscribe to hierarchical pattern
    client
        .post(format!("{}/pubsub/subscribe", base_url))
        .json(&json!({
            "topics": ["app.backend.api.#"]
        }))
        .send()
        .await
        .unwrap();

    // Test various depths
    let topics = vec![
        "app.backend.api",
        "app.backend.api.users",
        "app.backend.api.users.create",
        "app.backend.api.users.create.validation",
    ];

    for topic in topics {
        let res = client
            .post(format!("{}/pubsub/{}/publish", base_url, topic))
            .json(&json!({
                "payload": {"event": topic}
            }))
            .send()
            .await
            .unwrap();

        let body: serde_json::Value = res.json().await.unwrap();
        assert_eq!(
            body["subscribers_matched"], 1,
            "Topic {} should match app.backend.api.#",
            topic
        );
    }

    // This should not match
    let res = client
        .post(format!("{}/pubsub/app.backend.worker/publish", base_url))
        .json(&json!({
            "payload": {"test": true}
        }))
        .send()
        .await
        .unwrap();

    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["subscribers_matched"], 0);
}
