//! S2S (Server-to-Server) integration tests for Monitoring operations
//! These tests require a running Synap server

use reqwest::Client;
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use synap_server::auth::{ApiKeyManager, UserManager};
use synap_server::core::{
    HashStore, HyperLogLogStore, KVStore, ListStore, SetStore, SortedSetStore, TransactionManager,
};
use synap_server::monitoring::MonitoringManager;
use synap_server::server::router::create_router;
use synap_server::{AppState, KVConfig, ScriptManager};
use tokio::net::TcpListener;

async fn spawn_test_server() -> String {
    let kv_store = Arc::new(KVStore::new(KVConfig::default()));
    let hash_store = Arc::new(HashStore::new());
    let list_store = Arc::new(ListStore::new());
    let set_store = Arc::new(SetStore::new());
    let sorted_set_store = Arc::new(SortedSetStore::new());

    let monitoring = Arc::new(MonitoringManager::new(
        kv_store.clone(),
        hash_store.clone(),
        list_store.clone(),
        set_store.clone(),
        sorted_set_store.clone(),
    ));

    let transaction_manager = Arc::new(TransactionManager::new(
        kv_store.clone(),
        hash_store.clone(),
        list_store.clone(),
        set_store.clone(),
        sorted_set_store.clone(),
    ));

    let script_manager = Arc::new(ScriptManager::default());
    let hyperloglog_store = Arc::new(HyperLogLogStore::new());
    let bitmap_store = Arc::new(synap_server::core::BitmapStore::new());
    let geospatial_store = Arc::new(synap_server::core::GeospatialStore::new(
        sorted_set_store.clone(),
    ));

    let state = AppState {
        kv_store,
        hash_store,
        list_store,
        set_store,
        sorted_set_store,
        hyperloglog_store,
        bitmap_store,
        geospatial_store,
        queue_manager: None,
        stream_manager: None,
        partition_manager: None,
        consumer_group_manager: None,
        pubsub_router: None,
        persistence: None,
        monitoring,
        transaction_manager,
        script_manager,
    };

    let user_manager = Arc::new(UserManager::new());
    let api_key_manager = Arc::new(ApiKeyManager::new());
    let app = create_router(
        state,
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
    let url = format!("http://{}", addr);

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    tokio::time::sleep(Duration::from_millis(100)).await;

    url
}

async fn send_command(
    client: &Client,
    base_url: &str,
    command: &str,
    payload: serde_json::Value,
) -> serde_json::Value {
    client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": command,
            "request_id": uuid::Uuid::new_v4().to_string(),
            "payload": payload
        }))
        .send()
        .await
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap()
}

#[tokio::test]
#[cfg(feature = "s2s-tests")]
async fn test_info_all() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    let res = send_command(&client, &base_url, "info", json!({})).await;

    assert_eq!(res["success"], true);
    assert!(res["payload"]["server"].is_object());
    assert!(res["payload"]["memory"].is_object());
    assert!(res["payload"]["stats"].is_object());
    assert!(res["payload"]["replication"].is_object());
    assert!(res["payload"]["keyspace"].is_object());
}

#[tokio::test]
#[cfg(feature = "s2s-tests")]
async fn test_info_server_section() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    let res = send_command(&client, &base_url, "info", json!({"section": "server"})).await;

    assert_eq!(res["success"], true);
    assert!(res["payload"]["server"].is_object());
    assert!(res["payload"]["server"]["redis_version"].is_string());
    assert!(res["payload"]["server"]["uptime_in_seconds"].is_number());
}

#[tokio::test]
#[cfg(feature = "s2s-tests")]
async fn test_info_memory_section() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    let res = send_command(&client, &base_url, "info", json!({"section": "memory"})).await;

    assert_eq!(res["success"], true);
    assert!(res["payload"]["memory"].is_object());
}

#[tokio::test]
#[cfg(feature = "s2s-tests")]
async fn test_info_stats_section() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    let res = send_command(&client, &base_url, "info", json!({"section": "stats"})).await;

    assert_eq!(res["success"], true);
    assert!(res["payload"]["stats"].is_object());
}

#[tokio::test]
#[cfg(feature = "s2s-tests")]
async fn test_info_keyspace_section() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Create some keys first
    send_command(
        &client,
        &base_url,
        "kv.set",
        json!({"key": "test_key", "value": "test_value"}),
    )
    .await;

    let res = send_command(&client, &base_url, "info", json!({"section": "keyspace"})).await;

    assert_eq!(res["success"], true);
    assert!(res["payload"]["keyspace"].is_object());
}

#[tokio::test]
#[cfg(feature = "s2s-tests")]
async fn test_slowlog_get_empty() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    let res = send_command(&client, &base_url, "slowlog.get", json!({})).await;

    assert_eq!(res["success"], true);
    assert!(res["payload"]["entries"].is_array());
    assert_eq!(res["payload"]["total"], 0);
}

#[tokio::test]
#[cfg(feature = "s2s-tests")]
async fn test_slowlog_get_with_count() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    let res = send_command(&client, &base_url, "slowlog.get", json!({"count": 5})).await;

    assert_eq!(res["success"], true);
    assert!(res["payload"]["entries"].is_array());
    assert!(res["payload"]["total"].is_number());
}

#[tokio::test]
#[cfg(feature = "s2s-tests")]
async fn test_slowlog_reset() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    let res = send_command(&client, &base_url, "slowlog.reset", json!({})).await;

    assert_eq!(res["success"], true);
    assert_eq!(res["payload"]["success"], true);
    assert!(res["payload"]["cleared"].is_number());
}

#[tokio::test]
#[cfg(feature = "s2s-tests")]
async fn test_memory_usage_kv() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Create a key
    send_command(
        &client,
        &base_url,
        "kv.set",
        json!({"key": "memory_test", "value": "test_value_12345"}),
    )
    .await;

    let res = send_command(
        &client,
        &base_url,
        "memory.usage",
        json!({"key": "memory_test"}),
    )
    .await;

    assert_eq!(res["success"], true);
    assert!(res["payload"]["key"].as_str().is_some());
    assert!(res["payload"]["bytes"].is_number());
    assert!(res["payload"]["human"].is_string());
    assert!(res["payload"]["bytes"].as_u64().unwrap() > 0);
}

#[tokio::test]
#[cfg(feature = "s2s-tests")]
async fn test_memory_usage_hash() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Create a hash
    send_command(
        &client,
        &base_url,
        "hash.set",
        json!({"key": "hash_memory", "field": "field1", "value": "value1"}),
    )
    .await;

    send_command(
        &client,
        &base_url,
        "hash.set",
        json!({"key": "hash_memory", "field": "field2", "value": "value2"}),
    )
    .await;

    let res = send_command(
        &client,
        &base_url,
        "memory.usage",
        json!({"key": "hash_memory"}),
    )
    .await;

    assert_eq!(res["success"], true);
    assert_eq!(res["payload"]["key"], "hash_memory");
    assert!(res["payload"]["bytes"].as_u64().unwrap() > 0);
}

#[tokio::test]
#[cfg(feature = "s2s-tests")]
async fn test_memory_usage_list() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Create a list
    send_command(
        &client,
        &base_url,
        "list.lpush",
        json!({"key": "list_memory", "values": ["value1", "value2", "value3"]}),
    )
    .await;

    let res = send_command(
        &client,
        &base_url,
        "memory.usage",
        json!({"key": "list_memory"}),
    )
    .await;

    assert_eq!(res["success"], true);
    assert_eq!(res["payload"]["key"], "list_memory");
    assert!(res["payload"]["bytes"].as_u64().unwrap() > 0);
}

#[tokio::test]
#[cfg(feature = "s2s-tests")]
async fn test_memory_usage_nonexistent() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    let res = send_command(
        &client,
        &base_url,
        "memory.usage",
        json!({"key": "nonexistent"}),
    )
    .await;

    assert_eq!(res["success"], false);
    assert!(res["error"].as_str().is_some());
}

#[tokio::test]
#[cfg(feature = "s2s-tests")]
async fn test_client_list() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    let res = send_command(&client, &base_url, "client.list", json!({})).await;

    assert_eq!(res["success"], true);
    assert!(res["payload"]["clients"].is_array());
    assert!(res["payload"]["count"].is_number());
}
