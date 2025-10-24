use reqwest::Client;
use serde_json::json;
use std::sync::Arc;
use synap_server::{AppState, KVStore, PubSubRouter, ServerConfig, create_router};
use tokio::net::TcpListener;

/// Spawn a test server and return its base URL
async fn spawn_test_server() -> String {
    let config = ServerConfig::default();
    let kv_config = config.to_kv_config();

    let hash_store = Arc::new(synap_server::core::HashStore::new());
    let app_state = AppState {
        kv_store: Arc::new(KVStore::new(kv_config)),
        hash_store,
        list_store: Arc::new(synap_server::core::ListStore::new()),
        queue_manager: None,
        stream_manager: None,
        pubsub_router: Some(Arc::new(PubSubRouter::new())),
        persistence: None,
        consumer_group_manager: None,
        partition_manager: None,
    };

    let app = create_router(
        app_state,
        synap_server::config::RateLimitConfig {
            enabled: false,
            requests_per_second: 100,
            burst_size: 10,
        },
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

/// Helper function to send StreamableHTTP command
async fn send_command(
    client: &Client,
    base_url: &str,
    command: &str,
    payload: serde_json::Value,
) -> serde_json::Value {
    let request = json!({
        "command": command,
        "request_id": uuid::Uuid::new_v4().to_string(),
        "payload": payload
    });

    let res = client
        .post(format!("{}/api/v1/command", base_url))
        .json(&request)
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), 200);
    res.json().await.unwrap()
}

// ============================================================================
// Pub/Sub StreamableHTTP Tests
// ============================================================================

#[tokio::test]
async fn test_pubsub_subscribe_command() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    let res = send_command(
        &client,
        &base_url,
        "pubsub.subscribe",
        json!({
            "topics": ["test.topic", "notifications.*"]
        }),
    )
    .await;

    assert_eq!(res["success"], true);
    assert!(res["payload"]["subscriber_id"].as_str().is_some());
    assert_eq!(res["payload"]["subscription_count"], 2);
}

#[tokio::test]
async fn test_pubsub_publish_command() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Subscribe first
    let sub_res = send_command(
        &client,
        &base_url,
        "pubsub.subscribe",
        json!({
            "topics": ["events.test"]
        }),
    )
    .await;

    assert_eq!(sub_res["success"], true);

    // Publish
    let res = send_command(
        &client,
        &base_url,
        "pubsub.publish",
        json!({
            "topic": "events.test",
            "payload": {"message": "Hello Pub/Sub"},
            "metadata": {"source": "test"}
        }),
    )
    .await;

    assert_eq!(res["success"], true);
    assert!(res["payload"]["message_id"].as_str().is_some());
    assert_eq!(res["payload"]["topic"], "events.test");
    assert_eq!(res["payload"]["subscribers_matched"], 1);
}

#[tokio::test]
async fn test_pubsub_wildcard_single_level_command() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Subscribe with wildcard
    send_command(
        &client,
        &base_url,
        "pubsub.subscribe",
        json!({
            "topics": ["alerts.*"]
        }),
    )
    .await;

    // Publish to matching topic
    let res = send_command(
        &client,
        &base_url,
        "pubsub.publish",
        json!({
            "topic": "alerts.critical",
            "payload": {"level": "critical"}
        }),
    )
    .await;

    assert_eq!(res["payload"]["subscribers_matched"], 1);

    // Publish to non-matching topic
    let res = send_command(
        &client,
        &base_url,
        "pubsub.publish",
        json!({
            "topic": "alerts.critical.database",
            "payload": {"level": "critical"}
        }),
    )
    .await;

    assert_eq!(res["payload"]["subscribers_matched"], 0);
}

#[tokio::test]
async fn test_pubsub_wildcard_multi_level_command() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Subscribe with multi-level wildcard
    send_command(
        &client,
        &base_url,
        "pubsub.subscribe",
        json!({
            "topics": ["system.#"]
        }),
    )
    .await;

    // All these should match
    let topics = vec!["system", "system.cpu", "system.cpu.usage"];

    for topic in topics {
        let res = send_command(
            &client,
            &base_url,
            "pubsub.publish",
            json!({
                "topic": topic,
                "payload": {"value": 75}
            }),
        )
        .await;

        assert_eq!(
            res["payload"]["subscribers_matched"], 1,
            "Topic {} should match system.#",
            topic
        );
    }

    // This should not match
    let res = send_command(
        &client,
        &base_url,
        "pubsub.publish",
        json!({
            "topic": "application.start",
            "payload": {"test": true}
        }),
    )
    .await;

    assert_eq!(res["payload"]["subscribers_matched"], 0);
}

#[tokio::test]
async fn test_pubsub_unsubscribe_command() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Subscribe
    let sub_res = send_command(
        &client,
        &base_url,
        "pubsub.subscribe",
        json!({
            "topics": ["topic1", "topic2"]
        }),
    )
    .await;

    let subscriber_id = sub_res["payload"]["subscriber_id"].as_str().unwrap();

    // Unsubscribe from one topic
    let res = send_command(
        &client,
        &base_url,
        "pubsub.unsubscribe",
        json!({
            "subscriber_id": subscriber_id,
            "topics": ["topic1"]
        }),
    )
    .await;

    assert_eq!(res["success"], true);
    assert_eq!(res["payload"]["unsubscribed"], 1);

    // Verify topic1 has no subscribers
    let res = send_command(
        &client,
        &base_url,
        "pubsub.publish",
        json!({
            "topic": "topic1",
            "payload": {"test": true}
        }),
    )
    .await;

    assert_eq!(res["payload"]["subscribers_matched"], 0);

    // Verify topic2 still has subscriber
    let res = send_command(
        &client,
        &base_url,
        "pubsub.publish",
        json!({
            "topic": "topic2",
            "payload": {"test": true}
        }),
    )
    .await;

    assert_eq!(res["payload"]["subscribers_matched"], 1);
}

#[tokio::test]
async fn test_pubsub_stats_command() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Subscribe
    send_command(
        &client,
        &base_url,
        "pubsub.subscribe",
        json!({
            "topics": ["stats.test"]
        }),
    )
    .await;

    // Publish
    send_command(
        &client,
        &base_url,
        "pubsub.publish",
        json!({
            "topic": "stats.test",
            "payload": {"value": 123}
        }),
    )
    .await;

    // Get stats
    let res = send_command(&client, &base_url, "pubsub.stats", json!({})).await;

    assert_eq!(res["success"], true);
    assert_eq!(res["payload"]["total_topics"], 1);
    assert_eq!(res["payload"]["total_subscribers"], 1);
    assert!(res["payload"]["messages_published"].as_u64().unwrap() >= 1);
}

#[tokio::test]
async fn test_pubsub_topics_command() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Subscribe to create topics
    send_command(
        &client,
        &base_url,
        "pubsub.subscribe",
        json!({
            "topics": ["alpha", "beta", "gamma"]
        }),
    )
    .await;

    // List topics
    let res = send_command(&client, &base_url, "pubsub.topics", json!({})).await;

    assert_eq!(res["success"], true);
    assert_eq!(res["payload"]["count"], 3);

    let topics = res["payload"]["topics"].as_array().unwrap();
    assert_eq!(topics.len(), 3);
}

#[tokio::test]
async fn test_pubsub_info_command() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Subscribe
    send_command(
        &client,
        &base_url,
        "pubsub.subscribe",
        json!({
            "topics": ["info.test"]
        }),
    )
    .await;

    // Publish a message
    send_command(
        &client,
        &base_url,
        "pubsub.publish",
        json!({
            "topic": "info.test",
            "payload": {"data": "test"}
        }),
    )
    .await;

    // Get topic info
    let res = send_command(
        &client,
        &base_url,
        "pubsub.info",
        json!({
            "topic": "info.test"
        }),
    )
    .await;

    assert_eq!(res["success"], true);
    assert_eq!(res["payload"]["topic"], "info.test");
    assert_eq!(res["payload"]["subscriber_count"], 1);
    assert!(res["payload"]["message_count"].as_u64().unwrap() >= 1);
    assert!(res["payload"]["created_at"].as_u64().is_some());
}

#[tokio::test]
async fn test_pubsub_info_not_found() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Try to get info for non-existent topic
    let res = send_command(
        &client,
        &base_url,
        "pubsub.info",
        json!({
            "topic": "nonexistent.topic"
        }),
    )
    .await;

    assert_eq!(res["success"], false);
    assert!(res["error"].as_str().is_some());
}

#[tokio::test]
async fn test_pubsub_multiple_subscribers_command() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Subscribe 3 times
    for _ in 0..3 {
        send_command(
            &client,
            &base_url,
            "pubsub.subscribe",
            json!({
                "topics": ["broadcast"]
            }),
        )
        .await;
    }

    // Publish
    let res = send_command(
        &client,
        &base_url,
        "pubsub.publish",
        json!({
            "topic": "broadcast",
            "payload": {"message": "To all"}
        }),
    )
    .await;

    assert_eq!(res["payload"]["subscribers_matched"], 3);
}

#[tokio::test]
async fn test_pubsub_complex_wildcards_command() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Subscribe with different patterns
    send_command(
        &client,
        &base_url,
        "pubsub.subscribe",
        json!({
            "topics": ["app.*.error", "app.#"]
        }),
    )
    .await;

    // This matches both patterns from same subscriber
    let res = send_command(
        &client,
        &base_url,
        "pubsub.publish",
        json!({
            "topic": "app.backend.error",
            "payload": {"error": "Database connection failed"}
        }),
    )
    .await;

    // Should match both patterns
    assert!(res["payload"]["subscribers_matched"].as_u64().unwrap() >= 1);
}

#[tokio::test]
async fn test_pubsub_error_missing_topics() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    let res = send_command(
        &client,
        &base_url,
        "pubsub.subscribe",
        json!({
            // Missing topics field
        }),
    )
    .await;

    assert_eq!(res["success"], false);
    assert!(res["error"].as_str().unwrap().contains("topics"));
}

#[tokio::test]
async fn test_pubsub_error_empty_topics() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    let res = send_command(
        &client,
        &base_url,
        "pubsub.subscribe",
        json!({
            "topics": []
        }),
    )
    .await;

    assert_eq!(res["success"], false);
    assert!(res["error"].as_str().unwrap().contains("topic"));
}
