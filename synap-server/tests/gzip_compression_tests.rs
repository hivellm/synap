// Gzip Compression Tests
// Tests that REST API supports gzip compression (tower-http CompressionLayer)

use reqwest::Client;
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use synap_server::auth::{ApiKeyManager, UserManager};
use synap_server::{
    AppState, KVConfig, KVStore, QueueConfig, QueueManager, ScriptManager, create_router,
};
use tokio::net::TcpListener;

async fn spawn_test_server() -> String {
    let kv_store = Arc::new(KVStore::new(KVConfig::default()));
    let queue_manager = Arc::new(QueueManager::new(QueueConfig::default()));

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
    let state = AppState {
        kv_store,
        hash_store,
        list_store,
        set_store,
        sorted_set_store,
        hyperloglog_store: Arc::new(synap_server::core::HyperLogLogStore::new()),
        bitmap_store: Arc::new(synap_server::core::BitmapStore::new()),
        geospatial_store,
        queue_manager: Some(queue_manager),
        stream_manager: None,
        pubsub_router: None,
        persistence: None,
        consumer_group_manager: None,
        partition_manager: None,
        monitoring,
        transaction_manager,
        script_manager: Arc::new(ScriptManager::default()),
        client_list_manager: Arc::new(synap_server::monitoring::ClientListManager::new()),
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

// Note: reqwest automatically handles gzip decompression when the server
// sends compressed responses. These tests verify the server works correctly
// with compression enabled (via tower-http CompressionLayer).

#[tokio::test]
async fn test_compression_layer_enabled() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Verify server handles requests normally with compression layer
    let response = client
        .get(format!("{}/health", base_url))
        .send()
        .await
        .unwrap();

    assert!(response.status().is_success());
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["status"], "healthy");
}

#[tokio::test]
async fn test_compression_with_large_response() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Create many keys
    for i in 0..100 {
        client
            .post(format!("{}/kv/set", base_url))
            .json(&json!({
                "key": format!("key_{}", i),
                "value": format!("Value {}", i)
            }))
            .send()
            .await
            .unwrap();
    }

    // Request stats (will be compressed if large enough)
    let response = client
        .get(format!("{}/kv/stats", base_url))
        .send()
        .await
        .unwrap();

    assert!(response.status().is_success());
    let body: serde_json::Value = response.json().await.unwrap();
    assert!(body["total_keys"].as_u64().unwrap() >= 100);
}

#[tokio::test]
async fn test_compression_transparent_to_client() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // reqwest automatically handles compression/decompression
    let response = client
        .get(format!("{}/health", base_url))
        .send()
        .await
        .unwrap();

    assert!(response.status().is_success());
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["service"], "synap");
}

#[tokio::test]
async fn test_compression_with_queue_operations() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Create multiple queues
    for i in 0..50 {
        client
            .post(format!("{}/queue/queue_{}", base_url, i))
            .json(&json!({}))
            .send()
            .await
            .unwrap();
    }

    // List queues (auto-compressed by tower-http if large)
    let response = client
        .get(format!("{}/queue/list", base_url))
        .send()
        .await
        .unwrap();

    assert!(response.status().is_success());
    let body: serde_json::Value = response.json().await.unwrap();
    let queues = body["queues"].as_array().unwrap();
    assert!(queues.len() >= 50);
}

#[tokio::test]
async fn test_compression_preserves_complex_json() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Set a complex value
    let complex_value = json!({
        "user": {
            "id": 123,
            "name": "Alice",
            "tags": ["admin", "developer", "reviewer"],
            "metadata": {
                "created_at": "2025-10-21",
                "last_seen": "2025-10-21T12:00:00Z"
            }
        }
    });

    client
        .post(format!("{}/kv/set", base_url))
        .json(&json!({
            "key": "complex_data",
            "value": complex_value
        }))
        .send()
        .await
        .unwrap();

    // Get (compression handled transparently)
    let response = client
        .get(format!("{}/kv/get/complex_data", base_url))
        .send()
        .await
        .unwrap();

    assert!(response.status().is_success());
    // Server returns the value as a JSON string (quoted)
    let body_text = response.text().await.unwrap();

    // Parse the JSON string response (it's double-encoded)
    let value_str: String = serde_json::from_str(&body_text).unwrap();
    let value: serde_json::Value = serde_json::from_str(&value_str).unwrap();

    // Verify the complex JSON structure was preserved
    assert_eq!(value["user"]["id"], 123);
    assert_eq!(value["user"]["name"], "Alice");
    assert_eq!(value["user"]["tags"].as_array().unwrap().len(), 3);
}

#[tokio::test]
async fn test_compression_with_concurrent_requests() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    let mut handles = vec![];

    // 20 concurrent requests (compression handled transparently)
    for i in 0..20 {
        let url = base_url.clone();
        let client = client.clone();

        let handle = tokio::spawn(async move {
            let response = client
                .post(format!("{}/kv/set", url))
                .json(&json!({
                    "key": format!("comp_key_{}", i),
                    "value": format!("Value {}", i)
                }))
                .send()
                .await
                .unwrap();

            assert!(response.status().is_success());
            response.json::<serde_json::Value>().await.unwrap()
        });

        handles.push(handle);
    }

    // All should succeed
    for handle in handles {
        let body = handle.await.unwrap();
        assert_eq!(body["success"], true);
    }
}

#[tokio::test]
async fn test_compression_with_large_payload() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Create a large payload (will benefit from compression)
    let large_value = "Large data content that will be compressed by gzip. ".repeat(200);

    let response = client
        .post(format!("{}/kv/set", base_url))
        .json(&json!({
            "key": "large_key",
            "value": large_value
        }))
        .send()
        .await
        .unwrap();

    assert!(response.status().is_success());
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["success"], true);

    // Retrieve the large value (compressed by server, decompressed by client)
    let response = client
        .get(format!("{}/kv/get/large_key", base_url))
        .send()
        .await
        .unwrap();

    assert!(response.status().is_success());
    // Server returns the value as a JSON string (quoted)
    let body_text = response.text().await.unwrap();

    // Parse the JSON string response (double-encoded string)
    let value_str: String = serde_json::from_str(&body_text).unwrap();

    // Verify data integrity after compression/decompression
    assert!(value_str.contains("Large data content"));
    assert!(value_str.len() > 1000);
}
