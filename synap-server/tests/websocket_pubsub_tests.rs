use futures_util::{SinkExt, StreamExt};
use serde_json::json;
use std::sync::Arc;
use synap_server::{AppState, KVStore, PubSubRouter, ScriptManager, ServerConfig, create_router};
use tokio::net::TcpListener;
use tokio_tungstenite::{connect_async, tungstenite::Message};

/// Spawn a test server and return its base URL
async fn spawn_test_server() -> String {
    let config = ServerConfig::default();
    let kv_config = config.to_kv_config();

    let hash_store = Arc::new(synap_server::core::HashStore::new());
    let kv_store = Arc::new(KVStore::new(kv_config));
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
    let app_state = AppState {
        kv_store,
        hash_store,
        list_store,
        set_store,
        sorted_set_store,
        hyperloglog_store: Arc::new(synap_server::core::HyperLogLogStore::new()),
        queue_manager: None,
        stream_manager: None,
        pubsub_router: Some(Arc::new(PubSubRouter::new())),
        persistence: None,
        consumer_group_manager: None,
        partition_manager: None,
        monitoring,
        transaction_manager,
        script_manager: Arc::new(ScriptManager::default()),
    };

    let app = create_router(
        app_state,
        synap_server::config::RateLimitConfig {
            enabled: false,
            requests_per_second: 100,
            burst_size: 10,
        },
        synap_server::config::McpConfig::default(),
    );

    // Bind to random port
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    // Wait for server to start
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    format!("http://{}", addr)
}

#[tokio::test]
async fn test_pubsub_websocket_connection() {
    let base_url = spawn_test_server().await;
    let ws_url = base_url.replace("http://", "ws://");

    // Connect via WebSocket
    let (ws_stream, _) = connect_async(format!("{}/pubsub/ws?topics=test.topic", ws_url))
        .await
        .expect("Failed to connect");

    let (mut write, mut read) = ws_stream.split();

    // Should receive welcome message
    if let Some(Ok(Message::Text(text))) = read.next().await {
        let msg: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(msg["type"], "connected");
        assert!(msg["subscriber_id"].as_str().is_some());
        assert_eq!(msg["topics"][0], "test.topic");
        assert_eq!(msg["subscription_count"], 1);
    } else {
        panic!("Did not receive welcome message");
    }

    write.close().await.unwrap();
}

#[tokio::test]
async fn test_pubsub_websocket_instant_delivery() {
    let base_url = spawn_test_server().await;
    let ws_url = base_url.replace("http://", "ws://");

    // Connect subscriber via WebSocket
    let (ws_stream, _) = connect_async(format!("{}/pubsub/ws?topics=notifications.email", ws_url))
        .await
        .unwrap();

    let (mut write, mut read) = ws_stream.split();

    // Skip welcome
    read.next().await;

    // Publish message via REST
    let client = reqwest::Client::new();
    client
        .post(format!("{}/pubsub/notifications.email/publish", base_url))
        .json(&json!({
            "payload": {"to": "user@test.com", "subject": "Test"},
            "metadata": {"source": "test"}
        }))
        .send()
        .await
        .unwrap();

    // Should receive message INSTANTLY (sub-millisecond)
    tokio::time::timeout(tokio::time::Duration::from_millis(100), async {
        while let Some(Ok(Message::Text(text))) = read.next().await {
            let msg: serde_json::Value = serde_json::from_str(&text).unwrap();

            if msg["type"] == "message" {
                assert_eq!(msg["topic"], "notifications.email");
                assert_eq!(msg["payload"]["to"], "user@test.com");
                assert_eq!(msg["payload"]["subject"], "Test");
                assert_eq!(msg["metadata"]["source"], "test");
                assert!(msg["message_id"].as_str().is_some());
                assert!(msg["timestamp"].as_u64().is_some());
                break;
            }
        }
    })
    .await
    .expect("Timeout - message should arrive instantly");

    write.close().await.unwrap();
}

#[tokio::test]
async fn test_pubsub_websocket_wildcard_single_level() {
    let base_url = spawn_test_server().await;
    let ws_url = base_url.replace("http://", "ws://");

    // Subscribe with wildcard
    let (ws_stream, _) = connect_async(format!("{}/pubsub/ws?topics=alerts.*", ws_url))
        .await
        .unwrap();

    let (mut write, mut read) = ws_stream.split();

    // Skip welcome
    read.next().await;

    let client = reqwest::Client::new();

    // Publish to matching topic
    client
        .post(format!("{}/pubsub/alerts.critical/publish", base_url))
        .json(&json!({
            "payload": {"level": "critical", "message": "System down"}
        }))
        .send()
        .await
        .unwrap();

    // Should receive
    tokio::time::timeout(tokio::time::Duration::from_millis(100), async {
        while let Some(Ok(Message::Text(text))) = read.next().await {
            let msg: serde_json::Value = serde_json::from_str(&text).unwrap();

            if msg["type"] == "message" {
                assert_eq!(msg["topic"], "alerts.critical");
                assert_eq!(msg["payload"]["level"], "critical");
                break;
            }
        }
    })
    .await
    .expect("Should receive matching message");

    // Publish to non-matching topic (too many levels)
    client
        .post(format!(
            "{}/pubsub/alerts.critical.database/publish",
            base_url
        ))
        .json(&json!({
            "payload": {"test": true}
        }))
        .send()
        .await
        .unwrap();

    // Should NOT receive (timeout is OK)
    let result = tokio::time::timeout(tokio::time::Duration::from_millis(200), async {
        while let Some(Ok(Message::Text(text))) = read.next().await {
            let msg: serde_json::Value = serde_json::from_str(&text).unwrap();
            if msg["type"] == "message" && msg["topic"] == "alerts.critical.database" {
                panic!("Should not receive non-matching message");
            }
        }
    })
    .await;

    // Timeout is expected (no message should arrive)
    assert!(
        result.is_err(),
        "Should timeout - no message for non-matching topic"
    );

    write.close().await.unwrap();
}

#[tokio::test]
async fn test_pubsub_websocket_multiple_topics() {
    let base_url = spawn_test_server().await;
    let ws_url = base_url.replace("http://", "ws://");

    // Subscribe to multiple topics at once
    let (ws_stream, _) = connect_async(format!(
        "{}/pubsub/ws?topics=topic1,topic2,wildcard.*",
        ws_url
    ))
    .await
    .unwrap();

    let (mut write, mut read) = ws_stream.split();

    // Skip welcome
    let welcome = read.next().await.unwrap().unwrap();
    let welcome_msg: serde_json::Value = serde_json::from_str(welcome.to_text().unwrap()).unwrap();
    assert_eq!(welcome_msg["subscription_count"], 3);

    let client = reqwest::Client::new();

    // Publish to each topic
    client
        .post(format!("{}/pubsub/topic1/publish", base_url))
        .json(&json!({"payload": {"msg": "t1"}}))
        .send()
        .await
        .unwrap();

    client
        .post(format!("{}/pubsub/wildcard.test/publish", base_url))
        .json(&json!({"payload": {"msg": "wildcard"}}))
        .send()
        .await
        .unwrap();

    // Should receive both
    let mut received = 0;
    tokio::time::timeout(tokio::time::Duration::from_millis(200), async {
        while let Some(Ok(Message::Text(text))) = read.next().await {
            let msg: serde_json::Value = serde_json::from_str(&text).unwrap();

            if msg["type"] == "message" {
                received += 1;
                if received == 2 {
                    break;
                }
            }
        }
    })
    .await
    .expect("Should receive messages");

    assert_eq!(received, 2);
    write.close().await.unwrap();
}

#[tokio::test]
async fn test_pubsub_websocket_multiple_subscribers() {
    let base_url = spawn_test_server().await;
    let ws_url = base_url.replace("http://", "ws://");

    // Connect 3 subscribers to same topic
    let mut subscribers = vec![];
    for _ in 0..3 {
        let (ws_stream, _) = connect_async(format!("{}/pubsub/ws?topics=broadcast.topic", ws_url))
            .await
            .unwrap();

        let (write, mut read) = ws_stream.split();

        // Skip welcome
        read.next().await;

        subscribers.push((write, read));
    }

    // Publish one message
    let client = reqwest::Client::new();
    client
        .post(format!("{}/pubsub/broadcast.topic/publish", base_url))
        .json(&json!({
            "payload": {"broadcast": "to all"}
        }))
        .send()
        .await
        .unwrap();

    // All 3 subscribers should receive it
    for (mut write, mut read) in subscribers {
        tokio::time::timeout(tokio::time::Duration::from_millis(100), async {
            while let Some(Ok(Message::Text(text))) = read.next().await {
                let msg: serde_json::Value = serde_json::from_str(&text).unwrap();

                if msg["type"] == "message" {
                    assert_eq!(msg["topic"], "broadcast.topic");
                    assert_eq!(msg["payload"]["broadcast"], "to all");
                    break;
                }
            }
        })
        .await
        .expect("Each subscriber should receive message");

        write.close().await.unwrap();
    }
}
