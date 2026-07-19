//! WebSocket KV watch tests (`GET /kv/ws?keys=...`).
//!
//! Fully in-process: each test spawns its own server with a KV store wired to
//! a `KeyWatchNotifier`, the way `main.rs` does, so no external server or
//! feature flag is needed.

use futures_util::{SinkExt, StreamExt};
use serde_json::json;
use std::sync::Arc;
use synap_server::auth::{ApiKeyManager, UserManager};
use synap_server::core::KeyWatchNotifier;
use synap_server::{AppState, KVStore, PubSubRouter, ServerConfig, create_router};
use tokio::net::TcpListener;
use tokio_tungstenite::{connect_async, tungstenite::Message};

/// Spawn a test server whose KV store publishes watch events, and return its
/// base URL and shutdown handle.
async fn spawn_watch_server() -> (String, tokio::sync::oneshot::Sender<()>) {
    let config = ServerConfig::default();
    let kv_config = config.to_kv_config();

    let pubsub_router = Arc::new(PubSubRouter::new());
    let watch_notifier = Arc::new(KeyWatchNotifier::new(pubsub_router.clone(), 0));
    let kv_store = Arc::new(KVStore::new(kv_config).with_watch_notifier(Some(watch_notifier)));

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
        pubsub_router: Some(pubsub_router),
        persistence: None,
        consumer_group_manager: None,
        partition_manager: None,
        monitoring,
        transaction_manager,
        script_manager: Arc::new(synap_server::ScriptManager::default()),
        client_list_manager: Arc::new(synap_server::monitoring::ClientListManager::new()),
        cluster_topology: None,
        cluster_migration: None,
        hub_client: None,
        user_manager: None,
        require_auth: false,
        replication: None,
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

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

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

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    (format!("http://{}", addr), shutdown_tx)
}

/// Read frames until one of `type: "message"` arrives, and return it.
async fn next_watch_message(
    read: &mut (impl StreamExt<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + Unpin),
) -> serde_json::Value {
    tokio::time::timeout(tokio::time::Duration::from_secs(2), async {
        while let Some(Ok(Message::Text(text))) = read.next().await {
            let msg: serde_json::Value = serde_json::from_str(&text).unwrap();
            if msg["type"] == "message" {
                return msg;
            }
        }
        panic!("the socket closed before a watch message arrived");
    })
    .await
    .expect("a watch message arrives within the timeout")
}

#[tokio::test]
async fn ws_watch_receives_the_set_value() {
    let (base_url, shutdown) = spawn_watch_server().await;
    let ws_url = base_url.replace("http://", "ws://");

    let (ws_stream, _) = connect_async(format!("{}/kv/ws?keys=user:1", ws_url))
        .await
        .expect("the watch socket connects");
    let (mut write, mut read) = ws_stream.split();

    // Welcome frame names the watch channel.
    if let Some(Ok(Message::Text(text))) = read.next().await {
        let msg: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(msg["type"], "connected");
        assert_eq!(msg["topics"][0], "__watch@0__:user:1");
    } else {
        panic!("did not receive the welcome frame");
    }

    let client = reqwest::Client::new();
    client
        .post(format!("{}/kv/set", base_url))
        .json(&json!({ "key": "user:1", "value": "alice" }))
        .send()
        .await
        .unwrap();

    let msg = next_watch_message(&mut read).await;
    assert_eq!(msg["topic"], "__watch@0__:user:1");
    assert_eq!(msg["payload"]["event"], "set");
    assert_eq!(msg["payload"]["value"], "alice");
    assert_eq!(msg["payload"]["version"], 1);

    write.close().await.unwrap();
    let _ = shutdown.send(());
}

#[tokio::test]
async fn ws_watch_wildcard_covers_the_prefix() {
    let (base_url, shutdown) = spawn_watch_server().await;
    let ws_url = base_url.replace("http://", "ws://");

    let (ws_stream, _) = connect_async(format!("{}/kv/ws?keys=user:*", ws_url))
        .await
        .expect("the wildcard watch socket connects");
    let (mut write, mut read) = ws_stream.split();
    read.next().await; // welcome

    let client = reqwest::Client::new();
    for (key, value) in [("order:1", "ignored"), ("user:7", "seen")] {
        client
            .post(format!("{}/kv/set", base_url))
            .json(&json!({ "key": key, "value": value }))
            .send()
            .await
            .unwrap();
    }

    let msg = next_watch_message(&mut read).await;
    assert_eq!(
        msg["payload"]["key"], "user:7",
        "only the matching key reaches the wildcard watcher"
    );

    write.close().await.unwrap();
    let _ = shutdown.send(());
}

#[tokio::test]
async fn ws_watch_without_keys_is_rejected() {
    let (base_url, shutdown) = spawn_watch_server().await;
    let ws_url = base_url.replace("http://", "ws://");

    let result = connect_async(format!("{}/kv/ws", ws_url)).await;

    assert!(result.is_err(), "the handshake must fail without ?keys=");
    let _ = shutdown.send(());
}
