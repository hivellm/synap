//! WebSocket Queue Tests
//! These tests require a running Synap server

#[cfg(feature = "s2s-tests")]
use futures_util::{SinkExt, StreamExt};
#[cfg(feature = "s2s-tests")]
use serde_json::json;
#[cfg(feature = "s2s-tests")]
use std::sync::Arc;
#[cfg(feature = "s2s-tests")]
use synap_server::auth::{ApiKeyManager, UserManager};
#[cfg(feature = "s2s-tests")]
use synap_server::{
    AppState, KVStore, QueueConfig, QueueManager, ScriptManager, ServerConfig, create_router,
};
#[cfg(feature = "s2s-tests")]
use tokio::net::TcpListener;
#[cfg(feature = "s2s-tests")]
use tokio_tungstenite::{connect_async, tungstenite::Message};

/// Spawn a test server and return its base URL and shutdown handle
#[cfg(feature = "s2s-tests")]
#[cfg_attr(not(feature = "s2s-tests"), allow(dead_code))]
async fn spawn_test_server() -> (String, tokio::sync::oneshot::Sender<()>) {
    let config = ServerConfig::default();
    let kv_config = config.to_kv_config();
    let queue_config = QueueConfig::default();

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
        queue_manager: Some(Arc::new(QueueManager::new(queue_config))),
        stream_manager: None,
        pubsub_router: None,
        persistence: None,
        consumer_group_manager: None,
        partition_manager: None,
        monitoring,
        transaction_manager,
        script_manager: Arc::new(ScriptManager::default()),
        client_list_manager: Arc::new(synap_server::monitoring::ClientListManager::new()),
        cluster_topology: None,
        cluster_migration: None,
        hub_client: None,
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

    // Create shutdown signal
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
    let shutdown_signal = async {
        shutdown_rx.await.ok();
    };

    tokio::spawn(async move {
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
        )
        .with_graceful_shutdown(shutdown_signal)
        .await
        .unwrap();
    });

    // Wait for server to start
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    (format!("http://{}", addr), shutdown_tx)
}

#[cfg(feature = "s2s-tests")]
#[tokio::test]
async fn test_queue_websocket_connection() {
    let (base_url, shutdown) = spawn_test_server().await;
    let ws_url = base_url.replace("http://", "ws://");

    // Create queue first via REST
    let client = reqwest::Client::new();
    client
        .post(format!("{}/queue/test_queue", base_url))
        .json(&json!({}))
        .send()
        .await
        .unwrap();

    // Connect via WebSocket
    let (ws_stream, _) = connect_async(format!("{}/queue/test_queue/ws/consumer1", ws_url))
        .await
        .expect("Failed to connect");

    let (mut write, mut read) = ws_stream.split();

    // Should receive welcome message
    if let Some(Ok(Message::Text(text))) = read.next().await {
        let msg: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(msg["type"], "connected");
        assert_eq!(msg["queue"], "test_queue");
        assert_eq!(msg["consumer_id"], "consumer1");
    } else {
        panic!("Did not receive welcome message");
    }

    // Close connection
    write.close().await.unwrap();
    let _ = shutdown.send(());
}

#[cfg(feature = "s2s-tests")]
#[tokio::test]
async fn test_queue_websocket_consume_and_ack() {
    let (base_url, shutdown) = spawn_test_server().await;
    let ws_url = base_url.replace("http://", "ws://");
    let client = reqwest::Client::new();

    // Create queue
    client
        .post(format!("{}/queue/ack_queue", base_url))
        .json(&json!({}))
        .send()
        .await
        .unwrap();

    // Publish a message
    let pub_res = client
        .post(format!("{}/queue/ack_queue/publish", base_url))
        .json(&json!({
            "payload": vec![1, 2, 3, 4, 5],
            "priority": 5
        }))
        .send()
        .await
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap();

    let message_id = pub_res["message_id"].as_str().unwrap();

    // Connect consumer via WebSocket
    let (ws_stream, _) = connect_async(format!("{}/queue/ack_queue/ws/consumer2", ws_url))
        .await
        .unwrap();

    let (mut write, mut read) = ws_stream.split();

    // Skip welcome message
    read.next().await;

    // Wait for message (should arrive within 200ms)
    tokio::time::timeout(tokio::time::Duration::from_millis(500), async {
        while let Some(Ok(Message::Text(text))) = read.next().await {
            let msg: serde_json::Value = serde_json::from_str(&text).unwrap();

            if msg["type"] == "message" {
                assert_eq!(msg["message_id"], message_id);
                assert_eq!(msg["priority"], 5);

                // Send ACK
                let ack_cmd = json!({
                    "command": "ack",
                    "message_id": message_id
                });
                write
                    .send(Message::Text(ack_cmd.to_string().into()))
                    .await
                    .unwrap();
                break;
            }
        }
    })
    .await
    .expect("Timeout waiting for message");

    write.close().await.unwrap();
    let _ = shutdown.send(());
}

#[cfg(feature = "s2s-tests")]
#[tokio::test]
async fn test_queue_websocket_nack_and_requeue() {
    let (base_url, shutdown) = spawn_test_server().await;
    let ws_url = base_url.replace("http://", "ws://");
    let client = reqwest::Client::new();

    // Create queue
    client
        .post(format!("{}/queue/nack_queue", base_url))
        .json(&json!({}))
        .send()
        .await
        .unwrap();

    // Publish message
    let pub_res = client
        .post(format!("{}/queue/nack_queue/publish", base_url))
        .json(&json!({
            "payload": vec![10, 20, 30],
            "priority": 3
        }))
        .send()
        .await
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap();

    let message_id = pub_res["message_id"].as_str().unwrap().to_string();

    // Connect consumer
    let (ws_stream, _) = connect_async(format!("{}/queue/nack_queue/ws/consumer3", ws_url))
        .await
        .unwrap();

    let (mut write, mut read) = ws_stream.split();

    // Skip welcome
    read.next().await;

    // Receive message and NACK it
    tokio::time::timeout(tokio::time::Duration::from_millis(500), async {
        while let Some(Ok(Message::Text(text))) = read.next().await {
            let msg: serde_json::Value = serde_json::from_str(&text).unwrap();

            if msg["type"] == "message" {
                // Send NACK with requeue
                let nack_cmd = json!({
                    "command": "nack",
                    "message_id": message_id,
                    "requeue": true
                });
                write
                    .send(Message::Text(nack_cmd.to_string().into()))
                    .await
                    .unwrap();
                break;
            }
        }
    })
    .await
    .expect("Timeout");

    // Should receive the message again (requeued)
    tokio::time::timeout(tokio::time::Duration::from_secs(2), async {
        while let Some(Ok(Message::Text(text))) = read.next().await {
            let msg: serde_json::Value = serde_json::from_str(&text).unwrap();

            if msg["type"] == "message" {
                assert_eq!(msg["message_id"], message_id);
                // ACK this time
                let ack_cmd = json!({
                    "command": "ack",
                    "message_id": message_id
                });
                write
                    .send(Message::Text(ack_cmd.to_string().into()))
                    .await
                    .unwrap();
                break;
            }
        }
    })
    .await
    .expect("Should receive requeued message");

    write.close().await.unwrap();
    let _ = shutdown.send(());
}

#[cfg(feature = "s2s-tests")]
#[tokio::test]
async fn test_queue_websocket_multiple_messages() {
    let (base_url, shutdown) = spawn_test_server().await;
    let ws_url = base_url.replace("http://", "ws://");
    let client = reqwest::Client::new();

    // Create queue
    client
        .post(format!("{}/queue/multi_queue", base_url))
        .json(&json!({}))
        .send()
        .await
        .unwrap();

    // Publish 5 messages
    for i in 1..=5 {
        client
            .post(format!("{}/queue/multi_queue/publish", base_url))
            .json(&json!({
                "payload": vec![i],
                "priority": i
            }))
            .send()
            .await
            .unwrap();
    }

    // Connect consumer
    let (ws_stream, _) = connect_async(format!("{}/queue/multi_queue/ws/consumer4", ws_url))
        .await
        .unwrap();

    let (mut write, mut read) = ws_stream.split();

    // Skip welcome
    read.next().await;

    // Receive and ACK all 5 messages
    let mut received_count = 0;
    tokio::time::timeout(tokio::time::Duration::from_secs(2), async {
        while let Some(Ok(Message::Text(text))) = read.next().await {
            let msg: serde_json::Value = serde_json::from_str(&text).unwrap();

            if msg["type"] == "message" {
                received_count += 1;

                // Send ACK
                let ack_cmd = json!({
                    "command": "ack",
                    "message_id": msg["message_id"]
                });
                write
                    .send(Message::Text(ack_cmd.to_string().into()))
                    .await
                    .unwrap();

                if received_count == 5 {
                    break;
                }
            }
        }
    })
    .await
    .expect("Timeout waiting for messages");

    assert_eq!(received_count, 5);
    write.close().await.unwrap();
    let _ = shutdown.send(());
}
