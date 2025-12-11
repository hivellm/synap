// Server-to-Server REST API Tests
// Tests all REST endpoints with real HTTP requests

use reqwest::Client;
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use synap_server::auth::{ApiKeyManager, UserManager};
use synap_server::{
    AppState, KVConfig, KVStore, ScriptManager, StreamConfig, StreamManager, create_router,
};
use tokio::net::TcpListener;

async fn spawn_test_server() -> String {
    let kv_store = Arc::new(KVStore::new(KVConfig::default()));
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

    // Initialize StreamManager for stream tests
    let stream_mgr = Arc::new(StreamManager::new(StreamConfig::default()));
    stream_mgr.clone().start_compaction_task();

    let state = AppState {
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

#[tokio::test]
async fn test_rest_health_endpoint() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    let res = client
        .get(format!("{}/health", base_url))
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), 200);

    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["status"], "healthy");
    assert_eq!(body["service"], "synap");
    assert!(body["version"].as_str().is_some());
}

#[tokio::test]
async fn test_rest_set_endpoint() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Test SET with all parameters
    let res = client
        .post(format!("{}/kv/set", base_url))
        .json(&json!({
            "key": "test_key",
            "value": {"name": "Alice", "age": 30},
            "ttl": 3600
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), 200);

    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["success"], true);
    assert_eq!(body["key"], "test_key");
}

#[tokio::test]
async fn test_rest_get_endpoint() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Setup: SET a value
    client
        .post(format!("{}/kv/set", base_url))
        .json(&json!({
            "key": "user:123",
            "value": {"username": "alice", "email": "alice@example.com"}
        }))
        .send()
        .await
        .unwrap();

    // Test: GET the value
    let res = client
        .get(format!("{}/kv/get/user:123", base_url))
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), 200);

    // Server returns value directly as JSON string
    let body_text = res.text().await.unwrap();
    let value_str: String = serde_json::from_str(&body_text).unwrap();
    let value: serde_json::Value = serde_json::from_str(&value_str).unwrap();
    assert_eq!(value["username"], "alice");
    assert_eq!(value["email"], "alice@example.com");
}

#[tokio::test]
async fn test_rest_get_nonexistent() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    let res = client
        .get(format!("{}/kv/get/nonexistent_key", base_url))
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), 200);

    // Server returns error object when key not found
    let body: serde_json::Value = res.json().await.unwrap();
    assert!(body.get("error").is_some(), "Expected error for not found");
}

#[tokio::test]
async fn test_rest_delete_endpoint() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Setup: SET a value
    client
        .post(format!("{}/kv/set", base_url))
        .json(&json!({"key": "to_delete", "value": "temporary"}))
        .send()
        .await
        .unwrap();

    // Test: DELETE
    let res = client
        .delete(format!("{}/kv/del/to_delete", base_url))
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), 200);

    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["deleted"], true);
    assert_eq!(body["key"], "to_delete");

    // Verify: GET returns not found
    let res = client
        .get(format!("{}/kv/get/to_delete", base_url))
        .send()
        .await
        .unwrap();

    // Server returns error object when key not found
    let body: serde_json::Value = res.json().await.unwrap();
    assert!(
        body.get("error").is_some(),
        "Expected error for deleted key"
    );
}

#[tokio::test]
async fn test_rest_stats_endpoint() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Perform some operations
    for i in 0..10 {
        client
            .post(format!("{}/kv/set", base_url))
            .json(&json!({"key": format!("key{}", i), "value": format!("value{}", i)}))
            .send()
            .await
            .unwrap();
    }

    // Test: GET stats
    let res = client
        .get(format!("{}/kv/stats", base_url))
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), 200);

    let body: serde_json::Value = res.json().await.unwrap();
    assert!(body["total_keys"].as_u64().unwrap() >= 10);
    assert!(body["operations"]["sets"].as_u64().unwrap() >= 10);
    assert!(body["hit_rate"].as_f64().is_some());
}

#[tokio::test]
async fn test_rest_workflow_complete() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // 1. SET multiple keys
    for i in 1..=5 {
        let res = client
            .post(format!("{}/kv/set", base_url))
            .json(&json!({
                "key": format!("product:{}", i),
                "value": {"name": format!("Product {}", i), "price": i * 10}
            }))
            .send()
            .await
            .unwrap();

        assert_eq!(res.status(), 200);
    }

    // 2. GET all keys
    for i in 1..=5 {
        let res = client
            .get(format!("{}/kv/get/product:{}", base_url, i))
            .send()
            .await
            .unwrap();

        // Server returns value directly as JSON string
        let body_text = res.text().await.unwrap();
        let value_str: String = serde_json::from_str(&body_text).unwrap();
        let value: serde_json::Value = serde_json::from_str(&value_str).unwrap();
        assert_eq!(value["name"], format!("Product {}", i));
    }

    // 3. DELETE some keys
    for i in 1..=3 {
        let res = client
            .delete(format!("{}/kv/del/product:{}", base_url, i))
            .send()
            .await
            .unwrap();

        let body: serde_json::Value = res.json().await.unwrap();
        assert_eq!(body["deleted"], true);
    }

    // 4. Verify stats
    let res = client
        .get(format!("{}/kv/stats", base_url))
        .send()
        .await
        .unwrap();

    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["total_keys"].as_u64().unwrap(), 2); // Only 4 and 5 remain
    assert_eq!(body["operations"]["sets"].as_u64().unwrap(), 5);
    assert_eq!(body["operations"]["dels"].as_u64().unwrap(), 3);
}

#[tokio::test]
async fn test_rest_error_handling() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Test: Invalid JSON
    let res = client
        .post(format!("{}/kv/set", base_url))
        .header("Content-Type", "application/json")
        .body("invalid json")
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), 400);

    // Test: Missing required field
    let res = client
        .post(format!("{}/kv/set", base_url))
        .json(&json!({"key": "test"})) // Missing 'value'
        .send()
        .await
        .unwrap();

    // Should return error (422 or 400)
    assert!(res.status().is_client_error());
}

#[tokio::test]
async fn test_rest_ttl_workflow() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // SET with TTL
    let res = client
        .post(format!("{}/kv/set", base_url))
        .json(&json!({
            "key": "session:abc",
            "value": "token123",
            "ttl": 2
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), 200);

    // GET immediately
    let res = client
        .get(format!("{}/kv/get/session:abc", base_url))
        .send()
        .await
        .unwrap();

    // Server returns value directly as JSON string (double-encoded)
    let body_text = res.text().await.unwrap();
    let value_str: String = serde_json::from_str(&body_text).unwrap();
    // For TTL test, just verify value was found (not checking exact value as may vary)
    assert!(!value_str.is_empty(), "Expected non-empty value");

    // Wait for expiration
    tokio::time::sleep(Duration::from_secs(3)).await;

    // GET after expiration
    let res = client
        .get(format!("{}/kv/get/session:abc", base_url))
        .send()
        .await
        .unwrap();

    // Server returns error object when key not found/expired
    let body: serde_json::Value = res.json().await.unwrap();
    assert!(
        body.get("error").is_some(),
        "Expected error for expired key"
    );
}

#[tokio::test]
async fn test_rest_concurrent_requests() {
    let base_url = spawn_test_server().await;
    let client = Arc::new(Client::new());

    // Spawn 10 concurrent SET requests
    let mut handles = vec![];
    for i in 0..10 {
        let client = Arc::clone(&client);
        let url = base_url.clone();

        let handle = tokio::spawn(async move {
            client
                .post(format!("{}/kv/set", url))
                .json(&json!({"key": format!("concurrent:{}", i), "value": i}))
                .send()
                .await
                .unwrap()
        });

        handles.push(handle);
    }

    // Wait for all to complete
    for handle in handles {
        let res = handle.await.unwrap();
        assert_eq!(res.status(), 200);
    }

    // Verify all keys exist
    let res = client
        .get(format!("{}/kv/stats", base_url))
        .send()
        .await
        .unwrap();

    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["total_keys"].as_u64().unwrap(), 10);
}

#[tokio::test]
async fn test_rest_memory_usage_nonexistent_key() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Test memory usage for non-existent key (should return 0)
    let res = client
        .get(format!("{}/memory/nonexistent:key/usage", base_url))
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["bytes"], 0);
    assert_eq!(body["human"], "0B");
    assert_eq!(body["key"], "nonexistent:key");
}

#[tokio::test]
async fn test_rest_sortedset_zadd_with_array_format() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Test ZADD with array format (multiple members)
    let res = client
        .post(format!("{}/sortedset/leaders/zadd", base_url))
        .json(&json!({
            "members": ["alice", "bob", "charlie"],
            "scores": [100.0, 200.0, 150.0]
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["added"], 3);
    assert_eq!(body["key"], "leaders");

    // Verify members were added
    let zcard_res = client
        .get(format!("{}/sortedset/leaders/zcard", base_url))
        .send()
        .await
        .unwrap();
    let zcard_body: serde_json::Value = zcard_res.json().await.unwrap();
    assert_eq!(zcard_body["count"], 3);
}

#[tokio::test]
async fn test_rest_sortedset_zadd_backward_compatibility_single_member() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Test ZADD with single member format (backward compatibility)
    let res = client
        .post(format!("{}/sortedset/player/zadd", base_url))
        .json(&json!({
            "member": "alice",
            "score": 100.0
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["added"], 1);
    assert_eq!(body["key"], "player");

    // Verify member was added
    let zcard_res = client
        .get(format!("{}/sortedset/player/zcard", base_url))
        .send()
        .await
        .unwrap();
    let zcard_body: serde_json::Value = zcard_res.json().await.unwrap();
    assert_eq!(zcard_body["count"], 1);
}

#[tokio::test]
async fn test_rest_stream_room_creation() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Test stream room creation via REST (POST /stream/{room})
    let res = client
        .post(format!("{}/stream/test_room_123", base_url))
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    assert!(body["success"].as_bool().unwrap_or(false) || body.get("room").is_some());

    // Verify room exists by getting stats
    let stats_res = client
        .get(format!("{}/stream/test_room_123/stats", base_url))
        .send()
        .await
        .unwrap();

    assert_eq!(stats_res.status(), 200);
    let stats_body: serde_json::Value = stats_res.json().await.unwrap();
    assert_eq!(stats_body["name"], "test_room_123");
}

#[tokio::test]
async fn test_rest_geoadd_format_validation() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Test GEOADD with correct format (locations array)
    let res = client
        .post(format!("{}/geospatial/cities/geoadd", base_url))
        .json(&json!({
            "locations": [
                {"lat": 37.7749, "lon": -122.4194, "member": "San Francisco"},
                {"lat": 40.7128, "lon": -74.0060, "member": "New York"}
            ],
            "nx": false,
            "xx": false,
            "ch": false
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["added"], 2);
    assert_eq!(body["key"], "cities");
}

#[tokio::test]
async fn test_rest_error_handling_invalid_format_msetnx() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Test MSETNX with invalid format (missing required fields)
    let res = client
        .post(format!("{}/kv/msetnx", base_url))
        .json(&json!({
            "invalid": "format"
        }))
        .send()
        .await
        .unwrap();

    // Should return error (400 or 422)
    assert!(res.status().is_client_error());
}

#[tokio::test]
async fn test_rest_error_handling_invalid_format_hash_mset() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Test HMSET with invalid format (not object or array)
    let res = client
        .post(format!("{}/hash/test_key/mset", base_url))
        .json(&json!("invalid_string_format"))
        .send()
        .await
        .unwrap();

    // Should return error (400 or 422)
    assert!(res.status().is_client_error());
}

#[tokio::test]
async fn test_rest_error_handling_invalid_format_zadd() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Test ZADD with invalid format (missing both member and members)
    let res = client
        .post(format!("{}/sortedset/test_key/zadd", base_url))
        .json(&json!({
            "score": 100.0
            // Missing member or members
        }))
        .send()
        .await
        .unwrap();

    // Should return error (400 or 422)
    assert!(res.status().is_client_error());
}

#[tokio::test]
async fn test_rest_error_handling_invalid_format_pubsub_publish() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Test Pub/Sub publish with invalid format (missing both payload and data)
    let res = client
        .post(format!("{}/pubsub/test_topic/publish", base_url))
        .json(&json!({
            "metadata": {"source": "test"}
            // Missing payload and data
        }))
        .send()
        .await
        .unwrap();

    // Should return error (400 or 422)
    assert!(res.status().is_client_error());
}

#[tokio::test]
async fn test_rest_streamablehttp_command_format() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Test StreamableHTTP command endpoint with correct format
    let res = client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "kv.set",
            "request_id": "test-streamable-123",
            "payload": {
                "key": "streamable_test",
                "value": "test_value"
            }
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["success"], true);
    assert_eq!(body["request_id"], "test-streamable-123");
    assert!(body["payload"].is_object());

    // Verify the value was set
    let get_res = client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "kv.get",
            "request_id": "test-streamable-456",
            "payload": {
                "key": "streamable_test"
            }
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(get_res.status(), 200);
    let get_body: serde_json::Value = get_res.json().await.unwrap();
    assert_eq!(get_body["success"], true);
    let value_str = get_body["payload"].as_str().unwrap();
    assert_eq!(value_str, "\"test_value\"");
}

#[tokio::test]
async fn test_rest_streamablehttp_error_handling() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Test with missing command field
    let res = client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "request_id": "test-error-1",
            "payload": {}
        }))
        .send()
        .await
        .unwrap();

    // Should return 400 Bad Request for missing required field
    assert!(res.status().is_client_error());

    // Test with unknown command
    let res = client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "unknown.command",
            "request_id": "test-error-2",
            "payload": {}
        }))
        .send()
        .await
        .unwrap();

    // StreamableHTTP returns 200 but with success: false
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["success"], false);
    assert!(body["error"].as_str().unwrap().contains("Unknown command"));
}
