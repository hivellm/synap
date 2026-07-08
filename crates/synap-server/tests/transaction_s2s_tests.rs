//! S2S (Server-to-Server) integration tests for Transaction operations
//! These tests require a running Synap server

#[cfg(feature = "s2s-tests")]
use reqwest::Client;
#[cfg(feature = "s2s-tests")]
use serde_json::json;
#[cfg(feature = "s2s-tests")]
use std::sync::Arc;
#[cfg(feature = "s2s-tests")]
use std::time::Duration;
#[cfg(feature = "s2s-tests")]
use synap_server::auth::{ApiKeyManager, UserManager};
#[cfg(feature = "s2s-tests")]
use synap_server::core::{
    HashStore, HyperLogLogStore, KVStore, ListStore, SetStore, SortedSetStore, TransactionManager,
};
#[cfg(feature = "s2s-tests")]
use synap_server::monitoring::{ClientListManager, MonitoringManager};
#[cfg(feature = "s2s-tests")]
use synap_server::server::router::create_router;
#[cfg(feature = "s2s-tests")]
use synap_server::{AppState, KVConfig, ScriptManager};
#[cfg(feature = "s2s-tests")]
use tokio::net::TcpListener;

#[cfg(feature = "s2s-tests")]
#[cfg_attr(not(feature = "s2s-tests"), allow(dead_code))]
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
    let client_list_manager = Arc::new(ClientListManager::new());
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
        client_list_manager,
        cluster_topology: None,
        cluster_migration: None,
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
#[cfg(feature = "s2s-tests")]
async fn test_transaction_kv_auto_queue() {
    let base_url = spawn_test_server().await;
    let client = Client::new();
    let client_id = format!(
        "test_client_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );

    // Start transaction
    let multi_res = client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "transaction.multi",
            "request_id": "test-1",
            "payload": {
                "client_id": client_id
            }
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(multi_res.status(), 200);

    // Queue KV commands using client_id (automatic queuing)
    let set1_res = client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "kv.set",
            "request_id": "test-2",
            "payload": {
                "key": "tx:key1",
                "value": "value1",
                "client_id": client_id
            }
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(set1_res.status(), 200);
    let set1_body: serde_json::Value = set1_res.json().await.unwrap();
    // Command should be queued (Response { success: true, payload: { success: true, queued: true } })
    assert!(set1_body["success"].as_bool().unwrap_or(false));
    assert!(set1_body["payload"]["queued"].as_bool().unwrap_or(false));

    let set2_res = client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "kv.set",
            "request_id": "test-3",
            "payload": {
                "key": "tx:key2",
                "value": "value2",
                "client_id": client_id
            }
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(set2_res.status(), 200);

    let incr_res = client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "kv.incr",
            "request_id": "test-4",
            "payload": {
                "key": "tx:counter",
                "amount": 5,
                "client_id": client_id
            }
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(incr_res.status(), 200);

    // Execute transaction
    let exec_res = client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "transaction.exec",
            "request_id": "test-5",
            "payload": {
                "client_id": client_id
            }
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(exec_res.status(), 200);
    let exec_body: serde_json::Value = exec_res.json().await.unwrap();
    assert!(exec_body["success"].as_bool().unwrap());
    assert!(exec_body["payload"]["results"].is_array());
    assert_eq!(exec_body["payload"]["results"].as_array().unwrap().len(), 3);

    // Verify values were set atomically
    let get1_res = client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "kv.get",
            "request_id": "test-6",
            "payload": {
                "key": "tx:key1"
            }
        }))
        .send()
        .await
        .unwrap();
    let get1_body: serde_json::Value = get1_res.json().await.unwrap();
    // kv.get returns JSON string (with quotes), need to parse it
    let value1_raw = get1_body["payload"].as_str().unwrap();
    // Remove JSON string quotes if present
    let value1 = value1_raw.trim_matches('"');
    assert_eq!(value1, "value1");

    let get2_res = client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "kv.get",
            "request_id": "test-7",
            "payload": {
                "key": "tx:key2"
            }
        }))
        .send()
        .await
        .unwrap();
    let get2_body: serde_json::Value = get2_res.json().await.unwrap();
    // kv.get returns JSON string (with quotes), need to parse it
    let value2_raw = get2_body["payload"].as_str().unwrap();
    // Remove JSON string quotes if present
    let value2 = value2_raw.trim_matches('"');
    assert_eq!(value2, "value2");

    let counter_res = client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "kv.get",
            "request_id": "test-8",
            "payload": {
                "key": "tx:counter"
            }
        }))
        .send()
        .await
        .unwrap();
    let counter_body: serde_json::Value = counter_res.json().await.unwrap();
    // kv.get returns JSON string (with quotes), need to parse it
    let counter_value_raw = counter_body["payload"].as_str().unwrap();
    // Remove JSON string quotes if present
    let counter_value = counter_value_raw.trim_matches('"');
    assert_eq!(counter_value, "5");
}

#[tokio::test]
#[cfg(feature = "s2s-tests")]
async fn test_transaction_discard_queued_commands() {
    let base_url = spawn_test_server().await;
    let client = Client::new();
    let client_id = format!(
        "test_client_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );

    // Start transaction
    client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "transaction.multi",
            "request_id": "test-1",
            "payload": {
                "client_id": client_id
            }
        }))
        .send()
        .await
        .unwrap();

    // Queue a command
    client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "kv.set",
            "request_id": "test-2",
            "payload": {
                "key": "tx:discard:key",
                "value": "should_not_exist",
                "client_id": client_id
            }
        }))
        .send()
        .await
        .unwrap();

    // Discard transaction
    client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "transaction.discard",
            "request_id": "test-3",
            "payload": {
                "client_id": client_id
            }
        }))
        .send()
        .await
        .unwrap();

    // Verify value was NOT set
    let get_res = client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "kv.get",
            "request_id": "test-4",
            "payload": {
                "key": "tx:discard:key"
            }
        }))
        .send()
        .await
        .unwrap();
    let get_body: serde_json::Value = get_res.json().await.unwrap();
    assert!(get_body["payload"].is_null());
}

#[tokio::test]
#[cfg(feature = "s2s-tests")]
async fn test_transaction_watch_abort_on_conflict() {
    let base_url = spawn_test_server().await;
    let client = Client::new();
    let client_id = format!(
        "test_client_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let watched_key = format!(
        "tx:watch:{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );

    // Set initial value
    client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "kv.set",
            "request_id": "test-1",
            "payload": {
                "key": watched_key,
                "value": "initial"
            }
        }))
        .send()
        .await
        .unwrap();

    // Start transaction and watch the key
    client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "transaction.multi",
            "request_id": "test-2",
            "payload": {
                "client_id": client_id
            }
        }))
        .send()
        .await
        .unwrap();

    client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "transaction.watch",
            "request_id": "test-3",
            "payload": {
                "client_id": client_id,
                "keys": [watched_key]
            }
        }))
        .send()
        .await
        .unwrap();

    // Queue a command for a different key (to have commands in transaction)
    let other_key = format!(
        "tx:other:{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "kv.set",
            "request_id": "test-4",
            "payload": {
                "key": other_key,
                "value": "other_value",
                "client_id": client_id
            }
        }))
        .send()
        .await
        .unwrap();

    // Modify the watched key outside the transaction (simulate conflict)
    // This must happen AFTER WATCH but BEFORE EXEC
    client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "kv.set",
            "request_id": "test-5",
            "payload": {
                "key": watched_key,
                "value": "conflict"
            }
        }))
        .send()
        .await
        .unwrap();

    // Execute transaction (should abort)
    let exec_res = client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "transaction.exec",
            "request_id": "test-6",
            "payload": {
                "client_id": client_id
            }
        }))
        .send()
        .await
        .unwrap();
    let exec_body: serde_json::Value = exec_res.json().await.unwrap();
    // Transaction should be aborted
    // Response structure when aborted: { success: true, payload: { aborted: true, message: "..." } }
    assert!(exec_body["success"].as_bool().unwrap_or(false));
    // Check payload for aborted field
    if let Some(payload) = exec_body["payload"].as_object() {
        // Check if aborted field exists
        assert!(
            payload.contains_key("aborted"),
            "Payload should contain 'aborted' field: {:?}",
            payload
        );
        assert!(payload["aborted"].as_bool().unwrap_or(false));
    } else {
        // Fallback: check if payload itself indicates abort
        panic!(
            "Expected payload to be an object, got: {:?}",
            exec_body["payload"]
        );
    }

    // Verify the conflict value is still there (transaction didn't execute)
    let get_res = client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "kv.get",
            "request_id": "test-7",
            "payload": {
                "key": watched_key
            }
        }))
        .send()
        .await
        .unwrap();
    let get_body: serde_json::Value = get_res.json().await.unwrap();
    // kv.get returns JSON string (with quotes), need to parse it
    let conflict_value_raw = get_body["payload"].as_str().unwrap_or("");
    // Remove JSON string quotes if present
    let conflict_value = conflict_value_raw.trim_matches('"');
    assert_eq!(conflict_value, "conflict");
}

#[tokio::test]
#[cfg(feature = "s2s-tests")]
async fn test_transaction_mixed_kv_operations() {
    let base_url = spawn_test_server().await;
    let client = Client::new();
    let client_id = format!(
        "test_client_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );

    // Start transaction
    client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "transaction.multi",
            "request_id": "test-1",
            "payload": {
                "client_id": client_id
            }
        }))
        .send()
        .await
        .unwrap();

    // Queue multiple KV operations
    client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "kv.set",
            "request_id": "test-2",
            "payload": {
                "key": "tx:mixed:key1",
                "value": "value1",
                "client_id": client_id
            }
        }))
        .send()
        .await
        .unwrap();

    client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "kv.incr",
            "request_id": "test-3",
            "payload": {
                "key": "tx:mixed:counter",
                "amount": 10,
                "client_id": client_id
            }
        }))
        .send()
        .await
        .unwrap();

    client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "kv.set",
            "request_id": "test-4",
            "payload": {
                "key": "tx:mixed:key2",
                "value": "value2",
                "client_id": client_id
            }
        }))
        .send()
        .await
        .unwrap();

    client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "kv.del",
            "request_id": "test-5",
            "payload": {
                "key": "tx:mixed:key1",
                "client_id": client_id
            }
        }))
        .send()
        .await
        .unwrap();

    // Execute transaction
    let exec_res = client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "transaction.exec",
            "request_id": "test-6",
            "payload": {
                "client_id": client_id
            }
        }))
        .send()
        .await
        .unwrap();
    let exec_body: serde_json::Value = exec_res.json().await.unwrap();
    assert!(exec_body["success"].as_bool().unwrap());
    assert_eq!(exec_body["payload"]["results"].as_array().unwrap().len(), 4);

    // Verify results
    let get1_res = client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "kv.get",
            "request_id": "test-7",
            "payload": {
                "key": "tx:mixed:key1"
            }
        }))
        .send()
        .await
        .unwrap();
    let get1_body: serde_json::Value = get1_res.json().await.unwrap();
    assert!(get1_body["payload"].is_null()); // Should be deleted

    let counter_res = client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "kv.get",
            "request_id": "test-8",
            "payload": {
                "key": "tx:mixed:counter"
            }
        }))
        .send()
        .await
        .unwrap();
    let counter_body: serde_json::Value = counter_res.json().await.unwrap();
    // kv.get returns JSON string (with quotes), need to parse it
    let counter_value_raw = counter_body["payload"].as_str().unwrap();
    // Remove JSON string quotes if present
    let counter_value = counter_value_raw.trim_matches('"');
    assert_eq!(counter_value, "10");

    let get2_res = client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "kv.get",
            "request_id": "test-9",
            "payload": {
                "key": "tx:mixed:key2"
            }
        }))
        .send()
        .await
        .unwrap();
    let get2_body: serde_json::Value = get2_res.json().await.unwrap();
    // kv.get returns JSON string (with quotes), need to parse it
    let value2_raw = get2_body["payload"].as_str().unwrap();
    // Remove JSON string quotes if present
    let value2 = value2_raw.trim_matches('"');
    assert_eq!(value2, "value2");
}

#[tokio::test]
#[cfg(feature = "s2s-tests")]
async fn test_transaction_empty_execution() {
    let base_url = spawn_test_server().await;
    let client = Client::new();
    let client_id = format!(
        "test_client_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );

    // Start transaction
    client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "transaction.multi",
            "request_id": "test-1",
            "payload": {
                "client_id": client_id
            }
        }))
        .send()
        .await
        .unwrap();

    // Execute empty transaction
    let exec_res = client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "transaction.exec",
            "request_id": "test-2",
            "payload": {
                "client_id": client_id
            }
        }))
        .send()
        .await
        .unwrap();
    let exec_body: serde_json::Value = exec_res.json().await.unwrap();
    assert!(exec_body["success"].as_bool().unwrap());
    assert!(exec_body["payload"]["results"].is_array());
    assert_eq!(exec_body["payload"]["results"].as_array().unwrap().len(), 0);
}

#[tokio::test]
#[cfg(feature = "s2s-tests")]
async fn test_transaction_unwatch_prevents_abort() {
    let base_url = spawn_test_server().await;
    let client = Client::new();
    let client_id = format!(
        "test_client_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let watched_key = format!(
        "tx:unwatch:{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );

    // Set initial value
    client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "kv.set",
            "request_id": "test-1",
            "payload": {
                "key": watched_key,
                "value": "initial"
            }
        }))
        .send()
        .await
        .unwrap();

    // Start transaction and watch the key
    client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "transaction.multi",
            "request_id": "test-2",
            "payload": {
                "client_id": client_id
            }
        }))
        .send()
        .await
        .unwrap();

    client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "transaction.watch",
            "request_id": "test-3",
            "payload": {
                "client_id": client_id,
                "keys": [watched_key]
            }
        }))
        .send()
        .await
        .unwrap();

    // Unwatch before modifying
    client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "transaction.unwatch",
            "request_id": "test-4",
            "payload": {
                "client_id": client_id
            }
        }))
        .send()
        .await
        .unwrap();

    // Modify the key outside the transaction
    client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "kv.set",
            "request_id": "test-5",
            "payload": {
                "key": watched_key,
                "value": "modified"
            }
        }))
        .send()
        .await
        .unwrap();

    // Execute transaction (should NOT abort because we unwatched)
    let exec_res = client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "transaction.exec",
            "request_id": "test-6",
            "payload": {
                "client_id": client_id
            }
        }))
        .send()
        .await
        .unwrap();
    let exec_body: serde_json::Value = exec_res.json().await.unwrap();
    assert!(exec_body["success"].as_bool().unwrap());
    assert!(exec_body["payload"].get("aborted").is_none());
}

// ============================================================================
// Hash Operations in Transactions
// ============================================================================

#[tokio::test]
#[cfg(feature = "s2s-tests")]
async fn test_transaction_hash_operations() {
    let base_url = spawn_test_server().await;
    let client = Client::new();
    let client_id = format!(
        "test_client_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let hash_key = format!(
        "tx:hash:{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );

    // Start transaction
    client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "transaction.multi",
            "request_id": "multi-1",
            "payload": { "client_id": client_id }
        }))
        .send()
        .await
        .unwrap();

    // Queue hash operations
    client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "hash.set",
            "request_id": "hash-1",
            "payload": {
                "key": hash_key,
                "field": "field1",
                "value": "value1",
                "client_id": client_id
            }
        }))
        .send()
        .await
        .unwrap();

    client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "hash.set",
            "request_id": "hash-2",
            "payload": {
                "key": hash_key,
                "field": "field2",
                "value": "value2",
                "client_id": client_id
            }
        }))
        .send()
        .await
        .unwrap();

    client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "hash.incrby",
            "request_id": "hash-3",
            "payload": {
                "key": hash_key,
                "field": "counter",
                "increment": 5,
                "client_id": client_id
            }
        }))
        .send()
        .await
        .unwrap();

    // Execute transaction
    let exec_res = client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "transaction.exec",
            "request_id": "exec-1",
            "payload": { "client_id": client_id }
        }))
        .send()
        .await
        .unwrap();
    let exec_body: serde_json::Value = exec_res.json().await.unwrap();
    assert!(exec_body["success"].as_bool().unwrap());
    assert_eq!(exec_body["payload"]["results"].as_array().unwrap().len(), 3);

    // Verify hash values
    let get1_res = client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "hash.get",
            "request_id": "get-1",
            "payload": { "key": hash_key, "field": "field1" }
        }))
        .send()
        .await
        .unwrap();
    let get1_body: serde_json::Value = get1_res.json().await.unwrap();
    // hash.get returns { "found": true, "value": ... } or { "found": false }
    if get1_body["payload"]["found"].as_bool().unwrap_or(false) {
        let value1 = get1_body["payload"]["value"]
            .as_str()
            .unwrap()
            .trim_matches('"');
        assert_eq!(value1, "value1");
    } else {
        panic!(
            "Expected hash.get to return found=true, got: {:?}",
            get1_body["payload"]
        );
    }

    let get2_res = client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "hash.get",
            "request_id": "get-2",
            "payload": { "key": hash_key, "field": "field2" }
        }))
        .send()
        .await
        .unwrap();
    let get2_body: serde_json::Value = get2_res.json().await.unwrap();
    // hash.get returns { "found": true, "value": ... } or { "found": false }
    if get2_body["payload"]["found"].as_bool().unwrap_or(false) {
        let value2 = get2_body["payload"]["value"]
            .as_str()
            .unwrap()
            .trim_matches('"');
        assert_eq!(value2, "value2");
    } else {
        panic!(
            "Expected hash.get to return found=true, got: {:?}",
            get2_body["payload"]
        );
    }

    let counter_res = client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "hash.get",
            "request_id": "get-3",
            "payload": { "key": hash_key, "field": "counter" }
        }))
        .send()
        .await
        .unwrap();
    let counter_body: serde_json::Value = counter_res.json().await.unwrap();
    // hash.get returns { "found": true, "value": ... } or { "found": false }
    if counter_body["payload"]["found"].as_bool().unwrap_or(false) {
        // Value can be a string or number
        let counter_value = if let Some(s) = counter_body["payload"]["value"].as_str() {
            s.trim_matches('"').to_string()
        } else if let Some(n) = counter_body["payload"]["value"].as_u64() {
            n.to_string()
        } else {
            panic!(
                "Expected hash.get value to be string or number, got: {:?}",
                counter_body["payload"]["value"]
            );
        };
        assert_eq!(counter_value, "5");
    } else {
        panic!(
            "Expected hash.get to return found=true, got: {:?}",
            counter_body["payload"]
        );
    }
}

#[tokio::test]
#[cfg(feature = "s2s-tests")]
async fn test_transaction_hash_del() {
    let base_url = spawn_test_server().await;
    let client = Client::new();
    let client_id = format!(
        "test_client_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let hash_key = format!(
        "tx:hash:del:{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );

    // Set initial values
    client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "hash.set",
            "request_id": "init-1",
            "payload": { "key": hash_key, "field": "field1", "value": "value1" }
        }))
        .send()
        .await
        .unwrap();

    client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "hash.set",
            "request_id": "init-2",
            "payload": { "key": hash_key, "field": "field2", "value": "value2" }
        }))
        .send()
        .await
        .unwrap();

    // Start transaction and delete fields
    client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "transaction.multi",
            "request_id": "multi-1",
            "payload": { "client_id": client_id }
        }))
        .send()
        .await
        .unwrap();

    client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "hash.del",
            "request_id": "del-1",
            "payload": {
                "key": hash_key,
                "fields": ["field1", "field2"],
                "client_id": client_id
            }
        }))
        .send()
        .await
        .unwrap();

    // Execute transaction
    let exec_res = client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "transaction.exec",
            "request_id": "exec-1",
            "payload": { "client_id": client_id }
        }))
        .send()
        .await
        .unwrap();
    let exec_body: serde_json::Value = exec_res.json().await.unwrap();
    assert!(exec_body["success"].as_bool().unwrap());
    assert_eq!(exec_body["payload"]["results"][0]["deleted"], 2);

    // Verify fields were deleted
    let get_res = client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "hash.get",
            "request_id": "get-1",
            "payload": { "key": hash_key, "field": "field1" }
        }))
        .send()
        .await
        .unwrap();
    let get_body: serde_json::Value = get_res.json().await.unwrap();
    // hash.get returns { "found": false } when field doesn't exist
    assert_eq!(get_body["payload"]["found"].as_bool(), Some(false));
}
