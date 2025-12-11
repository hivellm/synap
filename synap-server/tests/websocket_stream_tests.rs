//! WebSocket Stream Tests
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
    AppState, KVStore, ScriptManager, ServerConfig, StreamConfig, StreamManager, create_router,
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

    let stream_mgr = Arc::new(StreamManager::new(StreamConfig::default()));
    stream_mgr.clone().start_compaction_task();

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
        queue_manager: None,
        stream_manager: Some(stream_mgr),
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
        #[cfg(feature = "hub-integration")]
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
async fn test_stream_websocket_connection() {
    let (base_url, shutdown) = spawn_test_server().await;
    let ws_url = base_url.replace("http://", "ws://");
    let client = reqwest::Client::new();

    // Create room
    client
        .post(format!("{}/stream/ws_room", base_url))
        .send()
        .await
        .unwrap();

    // Connect via WebSocket
    let (ws_stream, _) = connect_async(format!("{}/stream/ws_room/ws/sub1", ws_url))
        .await
        .expect("Failed to connect");

    let (mut write, mut read) = ws_stream.split();

    // Should receive welcome message
    if let Some(Ok(Message::Text(text))) = read.next().await {
        let msg: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(msg["type"], "connected");
        assert_eq!(msg["room"], "ws_room");
        assert_eq!(msg["subscriber_id"], "sub1");
        assert_eq!(msg["from_offset"], 0);
    } else {
        panic!("Did not receive welcome message");
    }

    write.close().await.unwrap();
    let _ = shutdown.send(());
}

#[cfg(feature = "s2s-tests")]
#[tokio::test]
async fn test_stream_websocket_real_time_push() {
    let (base_url, shutdown) = spawn_test_server().await;
    let ws_url = base_url.replace("http://", "ws://");
    let client = reqwest::Client::new();

    // Create room
    client
        .post(format!("{}/stream/push_room", base_url))
        .send()
        .await
        .unwrap();

    // Connect subscriber via WebSocket
    let (ws_stream, _) = connect_async(format!("{}/stream/push_room/ws/sub2", ws_url))
        .await
        .unwrap();

    let (mut write, mut read) = ws_stream.split();

    // Skip welcome
    read.next().await;

    // Publish event via REST
    client
        .post(format!("{}/stream/push_room/publish", base_url))
        .json(&json!({
            "event": "test.event",
            "data": {"message": "Hello WebSocket"}
        }))
        .send()
        .await
        .unwrap();

    // Should receive event via WebSocket within 200ms
    tokio::time::timeout(tokio::time::Duration::from_millis(500), async {
        while let Some(Ok(Message::Text(text))) = read.next().await {
            let msg: serde_json::Value = serde_json::from_str(&text).unwrap();

            if msg["type"] == "event" {
                assert_eq!(msg["event"], "test.event");
                assert_eq!(msg["data"]["message"], "Hello WebSocket");
                assert!(msg["offset"].as_u64().is_some());
                break;
            }
        }
    })
    .await
    .expect("Timeout waiting for event");

    write.close().await.unwrap();
    let _ = shutdown.send(());
}

#[cfg(feature = "s2s-tests")]
#[tokio::test]
async fn test_stream_websocket_multiple_events() {
    let (base_url, shutdown) = spawn_test_server().await;
    let ws_url = base_url.replace("http://", "ws://");
    let client = reqwest::Client::new();

    // Create room
    client
        .post(format!("{}/stream/multi_room", base_url))
        .send()
        .await
        .unwrap();

    // Connect subscriber
    let (ws_stream, _) = connect_async(format!("{}/stream/multi_room/ws/sub3", ws_url))
        .await
        .unwrap();

    let (mut write, mut read) = ws_stream.split();

    // Skip welcome
    read.next().await;

    // Publish 3 events
    for i in 1..=3 {
        client
            .post(format!("{}/stream/multi_room/publish", base_url))
            .json(&json!({
                "event": "event",
                "data": {"number": i}
            }))
            .send()
            .await
            .unwrap();
    }

    // Receive all 3 events
    let mut received = 0;
    tokio::time::timeout(tokio::time::Duration::from_secs(1), async {
        while let Some(Ok(Message::Text(text))) = read.next().await {
            let msg: serde_json::Value = serde_json::from_str(&text).unwrap();

            if msg["type"] == "event" {
                received += 1;
                if received == 3 {
                    break;
                }
            }
        }
    })
    .await
    .expect("Timeout waiting for events");

    assert_eq!(received, 3);
    write.close().await.unwrap();
    let _ = shutdown.send(());
}

#[cfg(feature = "s2s-tests")]
#[tokio::test]
async fn test_stream_websocket_from_offset() {
    let (base_url, shutdown) = spawn_test_server().await;
    let ws_url = base_url.replace("http://", "ws://");
    let client = reqwest::Client::new();

    // Create room
    client
        .post(format!("{}/stream/offset_room", base_url))
        .send()
        .await
        .unwrap();

    // Publish 10 events first
    for i in 1..=10 {
        client
            .post(format!("{}/stream/offset_room/publish", base_url))
            .json(&json!({
                "event": "event",
                "data": {"num": i}
            }))
            .send()
            .await
            .unwrap();
    }

    // Connect from offset 5
    let (ws_stream, _) = connect_async(format!(
        "{}/stream/offset_room/ws/sub4?from_offset=5",
        ws_url
    ))
    .await
    .unwrap();

    let (mut write, mut read) = ws_stream.split();

    // Skip welcome
    read.next().await;

    // Should receive events from offset 5 onwards
    let mut first_offset = None;
    tokio::time::timeout(tokio::time::Duration::from_millis(500), async {
        while let Some(Ok(Message::Text(text))) = read.next().await {
            let msg: serde_json::Value = serde_json::from_str(&text).unwrap();

            if msg["type"] == "event" {
                let offset = msg["offset"].as_u64().unwrap();
                if first_offset.is_none() {
                    first_offset = Some(offset);
                }
                break;
            }
        }
    })
    .await
    .expect("Timeout");

    assert!(
        first_offset.unwrap() >= 5,
        "Should start from offset 5 or later"
    );
    write.close().await.unwrap();
    let _ = shutdown.send(());
}
