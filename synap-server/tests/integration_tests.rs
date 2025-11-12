use reqwest::Client;
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use synap_server::auth::{ApiKeyManager, UserManager};
use synap_server::monitoring::MonitoringManager;
use synap_server::{AppState, KVConfig, KVStore, ScriptManager, create_router};
use tokio::net::TcpListener;

/// Helper to spawn a test server
async fn spawn_test_server() -> String {
    let kv_store = Arc::new(KVStore::new(KVConfig::default()));
    let hash_store = Arc::new(synap_server::core::HashStore::new());

    let monitoring = Arc::new(MonitoringManager::new(
        kv_store.clone(),
        hash_store.clone(),
        Arc::new(synap_server::core::ListStore::new()),
        Arc::new(synap_server::core::SetStore::new()),
        Arc::new(synap_server::core::SortedSetStore::new()),
    ));
    let transaction_manager = Arc::new(synap_server::core::TransactionManager::new(
        kv_store.clone(),
        hash_store.clone(),
        Arc::new(synap_server::core::ListStore::new()),
        Arc::new(synap_server::core::SetStore::new()),
        Arc::new(synap_server::core::SortedSetStore::new()),
    ));
    let sorted_set_store = Arc::new(synap_server::core::SortedSetStore::new());
    let geospatial_store = Arc::new(synap_server::core::GeospatialStore::new(
        sorted_set_store.clone(),
    ));
    let state = AppState {
        kv_store,
        hash_store,
        list_store: Arc::new(synap_server::core::ListStore::new()),
        set_store: Arc::new(synap_server::core::SetStore::new()),
        sorted_set_store,
        hyperloglog_store: Arc::new(synap_server::core::HyperLogLogStore::new()),
        bitmap_store: Arc::new(synap_server::core::BitmapStore::new()),
        geospatial_store,
        queue_manager: None,
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

    // Give server time to start
    tokio::time::sleep(Duration::from_millis(100)).await;

    url
}

#[tokio::test]
async fn test_health_check() {
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
}

#[tokio::test]
async fn test_kv_set_get_delete() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // SET
    let res = client
        .post(format!("{}/kv/set", base_url))
        .json(&json!({
            "key": "test_key",
            "value": "test_value"
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["success"], true);

    // GET
    let res = client
        .get(format!("{}/kv/get/test_key", base_url))
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), 200);
    // Server returns value directly as JSON string
    let value_str: String = res.json().await.unwrap();
    assert_eq!(value_str, "\"test_value\"");

    // DELETE
    let res = client
        .delete(format!("{}/kv/del/test_key", base_url))
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["deleted"], true);

    // GET after delete
    let res = client
        .get(format!("{}/kv/get/test_key", base_url))
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), 200);
    // Server returns error object when key not found
    let body: serde_json::Value = res.json().await.unwrap();
    assert!(
        body.get("error").is_some(),
        "Expected error for deleted key"
    );
}

#[tokio::test]
async fn test_kv_with_ttl() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // SET with 1 second TTL
    let res = client
        .post(format!("{}/kv/set", base_url))
        .json(&json!({
            "key": "ttl_key",
            "value": "temporary",
            "ttl": 1
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), 200);

    // GET immediately
    let res = client
        .get(format!("{}/kv/get/ttl_key", base_url))
        .send()
        .await
        .unwrap();

    // Server returns value directly
    let value_str: String = res.json().await.unwrap();
    assert_eq!(value_str, "\"temporary\"");

    // Wait for expiration
    tokio::time::sleep(Duration::from_secs(2)).await;

    // GET after expiration
    let res = client
        .get(format!("{}/kv/get/ttl_key", base_url))
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
async fn test_streamable_http_command() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // kv.set command
    let res = client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "kv.set",
            "request_id": "test-123",
            "payload": {
                "key": "cmd_key",
                "value": "cmd_value"
            }
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["success"], true);
    assert_eq!(body["request_id"], "test-123");

    // kv.get command
    let res = client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "kv.get",
            "request_id": "test-456",
            "payload": {
                "key": "cmd_key"
            }
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["success"], true);
    // Payload now returns value directly as JSON string
    assert_eq!(body["payload"], "\"cmd_value\"");
}

#[tokio::test]
async fn test_incr_decr() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // INCR
    let res = client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "kv.incr",
            "request_id": "test-incr",
            "payload": {
                "key": "counter",
                "amount": 5
            }
        }))
        .send()
        .await
        .unwrap();

    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["payload"]["value"], 5);

    // DECR
    let res = client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "kv.decr",
            "request_id": "test-decr",
            "payload": {
                "key": "counter",
                "amount": 2
            }
        }))
        .send()
        .await
        .unwrap();

    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["payload"]["value"], 3);
}

#[tokio::test]
async fn test_mset_mget() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // MSET
    let res = client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "kv.mset",
            "request_id": "test-mset",
            "payload": {
                "pairs": [
                    {"key": "key1", "value": "value1"},
                    {"key": "key2", "value": "value2"},
                    {"key": "key3", "value": "value3"}
                ]
            }
        }))
        .send()
        .await
        .unwrap();

    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["success"], true);

    // MGET
    let res = client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "kv.mget",
            "request_id": "test-mget",
            "payload": {
                "keys": ["key1", "key2", "key3", "key4"]
            }
        }))
        .send()
        .await
        .unwrap();

    let body: serde_json::Value = res.json().await.unwrap();
    let values = body["payload"]["values"].as_array().unwrap();
    assert_eq!(values[0], "value1");
    assert_eq!(values[1], "value2");
    assert_eq!(values[2], "value3");
    assert_eq!(values[3], serde_json::Value::Null);
}

#[tokio::test]
async fn test_scan() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Set multiple keys with common prefix
    for i in 1..=5 {
        client
            .post(format!("{}/kv/set", base_url))
            .json(&json!({
                "key": format!("user:{}", i),
                "value": format!("User {}", i)
            }))
            .send()
            .await
            .unwrap();
    }

    // SCAN with prefix
    let res = client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "kv.scan",
            "request_id": "test-scan",
            "payload": {
                "prefix": "user:",
                "limit": 10
            }
        }))
        .send()
        .await
        .unwrap();

    let body: serde_json::Value = res.json().await.unwrap();
    let keys = body["payload"]["keys"].as_array().unwrap();
    assert_eq!(keys.len(), 5);
}

#[tokio::test]
async fn test_stats() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Perform some operations
    client
        .post(format!("{}/kv/set", base_url))
        .json(&json!({"key": "k1", "value": "v1"}))
        .send()
        .await
        .unwrap();

    client
        .get(format!("{}/kv/get/k1", base_url))
        .send()
        .await
        .unwrap();

    // Get stats
    let res = client
        .get(format!("{}/kv/stats", base_url))
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    assert!(body["total_keys"].as_u64().unwrap() >= 1);
    assert!(body["operations"]["sets"].as_u64().unwrap() >= 1);
    assert!(body["operations"]["gets"].as_u64().unwrap() >= 1);
}

// ==================== Transaction Integration Tests ====================

#[tokio::test]
async fn test_transaction_multi_exec() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Start transaction
    let multi_res = client
        .post(format!("{}/transaction/multi", base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(multi_res.status(), 200);

    // Execute transaction (empty - should work)
    let exec_res = client
        .post(format!("{}/transaction/exec", base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(exec_res.status(), 200);
    let exec_body: serde_json::Value = exec_res.json().await.unwrap();
    // EXEC returns either {success: true, results: [...]} or {aborted: true}
    assert!(exec_body["success"].as_bool().is_some() || exec_body["results"].is_array());
}

#[tokio::test]
async fn test_transaction_discard() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Start transaction
    let multi_res = client
        .post(format!("{}/transaction/multi", base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(multi_res.status(), 200);

    // Discard transaction
    let discard_res = client
        .post(format!("{}/transaction/discard", base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(discard_res.status(), 200);
    let discard_body: serde_json::Value = discard_res.json().await.unwrap();
    assert!(discard_body["success"].as_bool().unwrap_or(false));
}

#[tokio::test]
async fn test_transaction_watch_unwatch() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Start transaction
    let multi_res = client
        .post(format!("{}/transaction/multi", base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(multi_res.status(), 200);

    // Watch keys
    let watch_res = client
        .post(format!("{}/transaction/watch", base_url))
        .json(&json!({
            "keys": ["key1", "key2"]
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(watch_res.status(), 200);

    // Unwatch
    let unwatch_res = client
        .post(format!("{}/transaction/unwatch", base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(unwatch_res.status(), 200);
    let unwatch_body: serde_json::Value = unwatch_res.json().await.unwrap();
    assert!(unwatch_body["success"].as_bool().unwrap_or(false));
}
