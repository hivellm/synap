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

// ============================================================================
// List Operations in Transactions
// ============================================================================

#[tokio::test]
#[cfg(feature = "s2s-tests")]
async fn test_transaction_list_operations() {
    let base_url = spawn_test_server().await;
    let client = Client::new();
    let client_id = format!(
        "test_client_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let list_key = format!(
        "tx:list:{}",
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

    // Queue list operations
    client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "list.lpush",
            "request_id": "list-1",
            "payload": {
                "key": list_key,
                "values": ["value1", "value2"],
                "client_id": client_id
            }
        }))
        .send()
        .await
        .unwrap();

    client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "list.rpush",
            "request_id": "list-2",
            "payload": {
                "key": list_key,
                "values": ["value3", "value4"],
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
    let results = exec_body["payload"]["results"].as_array().unwrap();
    assert_eq!(results.len(), 2);
    // First result is lpush (length 2), second is rpush (length 4)
    // Note: Results may be in different order, so check both possibilities
    let len1 = results[0]["length"].as_u64().unwrap_or(0);
    let len2 = results[1]["length"].as_u64().unwrap_or(0);
    assert!(
        (len1 == 2 && len2 == 4) || (len1 == 4 && len2 == 2),
        "Expected lengths 2 and 4, got {} and {}",
        len1,
        len2
    );

    // Verify list contents
    let len_res = client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "list.llen",
            "request_id": "len-1",
            "payload": { "key": list_key }
        }))
        .send()
        .await
        .unwrap();
    let len_body: serde_json::Value = len_res.json().await.unwrap();
    // list.llen returns { "length": length, "key": key }
    let length = len_body["payload"]["length"].as_u64().unwrap_or(0);
    assert_eq!(length, 4, "Expected list length 4, got {}", length);
}

#[tokio::test]
#[cfg(feature = "s2s-tests")]
async fn test_transaction_list_pop_operations() {
    let base_url = spawn_test_server().await;
    let client = Client::new();
    let client_id = format!(
        "test_client_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let list_key = format!(
        "tx:list:pop:{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );

    // Set initial values
    client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "list.rpush",
            "request_id": "init-1",
            "payload": { "key": list_key, "values": ["value1", "value2", "value3"] }
        }))
        .send()
        .await
        .unwrap();

    // Start transaction and pop values
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
            "command": "list.lpop",
            "request_id": "pop-1",
            "payload": {
                "key": list_key,
                "count": 1,
                "client_id": client_id
            }
        }))
        .send()
        .await
        .unwrap();

    client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "list.rpop",
            "request_id": "pop-2",
            "payload": {
                "key": list_key,
                "count": 1,
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
    assert_eq!(exec_body["payload"]["results"].as_array().unwrap().len(), 2);

    // Verify list length
    let len_res = client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "list.llen",
            "request_id": "len-1",
            "payload": { "key": list_key }
        }))
        .send()
        .await
        .unwrap();
    let len_body: serde_json::Value = len_res.json().await.unwrap();
    // After lpop and rpop, list should have 1 element remaining (started with 3)
    // list.llen returns { "length": length, "key": key }
    let length = len_body["payload"]["length"].as_u64().unwrap_or(0);
    assert_eq!(length, 1, "Expected list length 1, got {}", length);
}

// ============================================================================
// Set Operations in Transactions
// ============================================================================

#[tokio::test]
#[cfg(feature = "s2s-tests")]
async fn test_transaction_set_operations() {
    let base_url = spawn_test_server().await;
    let client = Client::new();
    let client_id = format!(
        "test_client_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let set_key = format!(
        "tx:set:{}",
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

    // Queue set operations
    client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "set.add",
            "request_id": "set-1",
            "payload": {
                "key": set_key,
                "members": ["member1", "member2", "member3"],
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
    assert_eq!(exec_body["payload"]["results"][0]["added"], 3);

    // Verify set size
    let size_res = client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "set.size",
            "request_id": "size-1",
            "payload": { "key": set_key }
        }))
        .send()
        .await
        .unwrap();
    let size_body: serde_json::Value = size_res.json().await.unwrap();
    // set.size returns { "size": count } in payload
    assert_eq!(size_body["payload"]["size"], 3);
}

#[tokio::test]
#[cfg(feature = "s2s-tests")]
async fn test_transaction_set_rem() {
    let base_url = spawn_test_server().await;
    let client = Client::new();
    let client_id = format!(
        "test_client_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let set_key = format!(
        "tx:set:rem:{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );

    // Set initial values
    client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "set.add",
            "request_id": "init-1",
            "payload": { "key": set_key, "members": ["member1", "member2", "member3"] }
        }))
        .send()
        .await
        .unwrap();

    // Start transaction and remove members
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
            "command": "set.rem",
            "request_id": "rem-1",
            "payload": {
                "key": set_key,
                "members": ["member1", "member2"],
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
    assert_eq!(exec_body["payload"]["results"][0]["removed"], 2);

    // Verify set size
    let size_res = client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "set.size",
            "request_id": "size-1",
            "payload": { "key": set_key }
        }))
        .send()
        .await
        .unwrap();
    let size_body: serde_json::Value = size_res.json().await.unwrap();
    // set.size returns { "size": count } in payload
    assert_eq!(size_body["payload"]["size"], 1);
}

// ============================================================================
// Mixed Operations in Transactions
// ============================================================================

#[tokio::test]
#[cfg(feature = "s2s-tests")]
async fn test_transaction_mixed_data_structures() {
    let base_url = spawn_test_server().await;
    let client = Client::new();
    let client_id = format!(
        "test_client_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();

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

    // Queue mixed operations: KV, Hash, List, Set
    client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "kv.set",
            "request_id": "kv-1",
            "payload": {
                "key": format!("tx:mixed:kv:{}", timestamp),
                "value": "kv_value",
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
            "request_id": "hash-1",
            "payload": {
                "key": format!("tx:mixed:hash:{}", timestamp),
                "field": "field1",
                "value": "hash_value",
                "client_id": client_id
            }
        }))
        .send()
        .await
        .unwrap();

    client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "list.lpush",
            "request_id": "list-1",
            "payload": {
                "key": format!("tx:mixed:list:{}", timestamp),
                "values": ["list_value"],
                "client_id": client_id
            }
        }))
        .send()
        .await
        .unwrap();

    client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "set.add",
            "request_id": "set-1",
            "payload": {
                "key": format!("tx:mixed:set:{}", timestamp),
                "members": ["set_value"],
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
    let results = exec_body["payload"]["results"].as_array().unwrap();
    // Should have 4 results: kv.set, hash.set, list.lpush, set.add
    assert_eq!(
        results.len(),
        4,
        "Expected 4 results, got {}",
        results.len()
    );

    // Verify all operations were executed atomically
    let kv_res = client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "kv.get",
            "request_id": "verify-kv",
            "payload": { "key": format!("tx:mixed:kv:{}", timestamp) }
        }))
        .send()
        .await
        .unwrap();
    let kv_body: serde_json::Value = kv_res.json().await.unwrap();
    assert_eq!(
        kv_body["payload"].as_str().unwrap().trim_matches('"'),
        "kv_value"
    );

    let hash_res = client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "hash.get",
            "request_id": "verify-hash",
            "payload": { "key": format!("tx:mixed:hash:{}", timestamp), "field": "field1" }
        }))
        .send()
        .await
        .unwrap();
    let hash_body: serde_json::Value = hash_res.json().await.unwrap();
    // hash.get returns { "found": true, "value": ... } or { "found": false }
    if hash_body["payload"]["found"].as_bool().unwrap_or(false) {
        let hash_value = hash_body["payload"]["value"]
            .as_str()
            .unwrap()
            .trim_matches('"');
        assert_eq!(hash_value, "hash_value");
    } else {
        panic!(
            "Expected hash.get to return found=true, got: {:?}",
            hash_body["payload"]
        );
    }

    let list_res = client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "list.llen",
            "request_id": "verify-list",
            "payload": { "key": format!("tx:mixed:list:{}", timestamp) }
        }))
        .send()
        .await
        .unwrap();
    let list_body: serde_json::Value = list_res.json().await.unwrap();
    // list.llen returns { "length": length, "key": key }
    let length = list_body["payload"]["length"].as_u64().unwrap_or(0);
    assert_eq!(length, 1, "Expected list length 1, got {}", length);

    let set_res = client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "set.size",
            "request_id": "verify-set",
            "payload": { "key": format!("tx:mixed:set:{}", timestamp) }
        }))
        .send()
        .await
        .unwrap();
    let set_body: serde_json::Value = set_res.json().await.unwrap();
    // set.size returns { "size": count } in payload
    assert_eq!(set_body["payload"]["size"], 1);
}

// ============================================================================
// Multiple Concurrent Transactions
// ============================================================================

#[tokio::test]
#[cfg(feature = "s2s-tests")]
async fn test_transaction_concurrent_clients() {
    let base_url = spawn_test_server().await;
    let client = Client::new();
    let client1_id = format!(
        "client1_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let client2_id = format!(
        "client2_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();

    // Start transaction for client1
    client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "transaction.multi",
            "request_id": "multi-1",
            "payload": { "client_id": client1_id }
        }))
        .send()
        .await
        .unwrap();

    // Start transaction for client2
    client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "transaction.multi",
            "request_id": "multi-2",
            "payload": { "client_id": client2_id }
        }))
        .send()
        .await
        .unwrap();

    // Queue commands for client1
    client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "kv.set",
            "request_id": "set-1",
            "payload": {
                "key": format!("tx:client1:{}", timestamp),
                "value": "client1_value",
                "client_id": client1_id
            }
        }))
        .send()
        .await
        .unwrap();

    // Queue commands for client2
    client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "kv.set",
            "request_id": "set-2",
            "payload": {
                "key": format!("tx:client2:{}", timestamp),
                "value": "client2_value",
                "client_id": client2_id
            }
        }))
        .send()
        .await
        .unwrap();

    // Execute both transactions
    let exec1_res = client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "transaction.exec",
            "request_id": "exec-1",
            "payload": { "client_id": client1_id }
        }))
        .send()
        .await
        .unwrap();
    let exec1_body: serde_json::Value = exec1_res.json().await.unwrap();
    assert!(exec1_body["success"].as_bool().unwrap());

    let exec2_res = client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "transaction.exec",
            "request_id": "exec-2",
            "payload": { "client_id": client2_id }
        }))
        .send()
        .await
        .unwrap();
    let exec2_body: serde_json::Value = exec2_res.json().await.unwrap();
    assert!(exec2_body["success"].as_bool().unwrap());

    // Verify both transactions executed independently
    let get1_res = client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "kv.get",
            "request_id": "get-1",
            "payload": { "key": format!("tx:client1:{}", timestamp) }
        }))
        .send()
        .await
        .unwrap();
    let get1_body: serde_json::Value = get1_res.json().await.unwrap();
    assert_eq!(
        get1_body["payload"].as_str().unwrap().trim_matches('"'),
        "client1_value"
    );

    let get2_res = client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "kv.get",
            "request_id": "get-2",
            "payload": { "key": format!("tx:client2:{}", timestamp) }
        }))
        .send()
        .await
        .unwrap();
    let get2_body: serde_json::Value = get2_res.json().await.unwrap();
    assert_eq!(
        get2_body["payload"].as_str().unwrap().trim_matches('"'),
        "client2_value"
    );
}

// ============================================================================
// WATCH Multiple Keys
// ============================================================================

#[tokio::test]
#[cfg(feature = "s2s-tests")]
async fn test_transaction_watch_multiple_keys() {
    let base_url = spawn_test_server().await;
    let client = Client::new();
    let client_id = format!(
        "test_client_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let key1 = format!("tx:watch1:{}", timestamp);
    let key2 = format!("tx:watch2:{}", timestamp);

    // Set initial values
    client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "kv.set",
            "request_id": "init-1",
            "payload": { "key": key1, "value": "initial1" }
        }))
        .send()
        .await
        .unwrap();

    client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "kv.set",
            "request_id": "init-2",
            "payload": { "key": key2, "value": "initial2" }
        }))
        .send()
        .await
        .unwrap();

    // Start transaction and watch both keys
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
            "command": "transaction.watch",
            "request_id": "watch-1",
            "payload": {
                "client_id": client_id,
                "keys": [key1.clone(), key2.clone()]
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
            "request_id": "set-1",
            "payload": {
                "key": format!("tx:other:{}", timestamp),
                "value": "other_value",
                "client_id": client_id
            }
        }))
        .send()
        .await
        .unwrap();

    // Modify one of the watched keys (should abort transaction)
    client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "kv.set",
            "request_id": "modify-1",
            "payload": { "key": key1, "value": "modified" }
        }))
        .send()
        .await
        .unwrap();

    // Execute transaction (should abort)
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
    assert!(exec_body["payload"]["aborted"].as_bool().unwrap_or(false));

    // Verify the queued command was NOT executed
    let get_res = client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "kv.get",
            "request_id": "verify-1",
            "payload": { "key": format!("tx:other:{}", timestamp) }
        }))
        .send()
        .await
        .unwrap();
    let get_body: serde_json::Value = get_res.json().await.unwrap();
    assert!(get_body["payload"].is_null());
}

// ============================================================================
// Large Transaction (Many Commands)
// ============================================================================

#[tokio::test]
#[cfg(feature = "s2s-tests")]
async fn test_transaction_large_number_of_commands() {
    let base_url = spawn_test_server().await;
    let client = Client::new();
    let client_id = format!(
        "test_client_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();

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

    // Queue 50 commands
    for i in 0..50 {
        client
            .post(format!("{}/api/v1/command", base_url))
            .json(&json!({
                "command": "kv.set",
                "request_id": format!("set-{}", i),
                "payload": {
                    "key": format!("tx:large:{}:{}", timestamp, i),
                    "value": format!("value_{}", i),
                    "client_id": client_id
                }
            }))
            .send()
            .await
            .unwrap();
    }

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
    assert_eq!(
        exec_body["payload"]["results"].as_array().unwrap().len(),
        50
    );

    // Verify a few random keys
    for i in [0, 25, 49] {
        let get_res = client
            .post(format!("{}/api/v1/command", base_url))
            .json(&json!({
                "command": "kv.get",
                "request_id": format!("get-{}", i),
                "payload": { "key": format!("tx:large:{}:{}", timestamp, i) }
            }))
            .send()
            .await
            .unwrap();
        let get_body: serde_json::Value = get_res.json().await.unwrap();
        assert_eq!(
            get_body["payload"].as_str().unwrap().trim_matches('"'),
            format!("value_{}", i)
        );
    }
}

// ============================================================================
// Edge Cases
// ============================================================================

#[tokio::test]
#[cfg(feature = "s2s-tests")]
async fn test_transaction_multiple_watch_unwatch() {
    let base_url = spawn_test_server().await;
    let client = Client::new();
    let client_id = format!(
        "test_client_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let key1 = format!("tx:watch1:{}", timestamp);
    let key2 = format!("tx:watch2:{}", timestamp);

    // Set initial values
    client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "kv.set",
            "request_id": "init-1",
            "payload": { "key": key1.clone(), "value": "initial1" }
        }))
        .send()
        .await
        .unwrap();

    client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "kv.set",
            "request_id": "init-2",
            "payload": { "key": key2.clone(), "value": "initial2" }
        }))
        .send()
        .await
        .unwrap();

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

    // Watch key1
    client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "transaction.watch",
            "request_id": "watch-1",
            "payload": {
                "client_id": client_id,
                "keys": [key1.clone()]
            }
        }))
        .send()
        .await
        .unwrap();

    // Watch key2
    client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "transaction.watch",
            "request_id": "watch-2",
            "payload": {
                "client_id": client_id,
                "keys": [key2.clone()]
            }
        }))
        .send()
        .await
        .unwrap();

    // Unwatch all
    client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "transaction.unwatch",
            "request_id": "unwatch-1",
            "payload": { "client_id": client_id }
        }))
        .send()
        .await
        .unwrap();

    // Queue a command
    client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "kv.set",
            "request_id": "set-1",
            "payload": {
                "key": format!("tx:other:{}", timestamp),
                "value": "other_value",
                "client_id": client_id
            }
        }))
        .send()
        .await
        .unwrap();

    // Modify watched keys (should NOT abort because we unwatched)
    client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "kv.set",
            "request_id": "modify-1",
            "payload": { "key": key1, "value": "modified1" }
        }))
        .send()
        .await
        .unwrap();

    client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "kv.set",
            "request_id": "modify-2",
            "payload": { "key": key2, "value": "modified2" }
        }))
        .send()
        .await
        .unwrap();

    // Execute transaction (should NOT abort)
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
    assert!(exec_body["payload"].get("aborted").is_none());

    // Verify the queued command WAS executed
    let get_res = client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "kv.get",
            "request_id": "verify-1",
            "payload": { "key": format!("tx:other:{}", timestamp) }
        }))
        .send()
        .await
        .unwrap();
    let get_body: serde_json::Value = get_res.json().await.unwrap();
    assert_eq!(
        get_body["payload"].as_str().unwrap().trim_matches('"'),
        "other_value"
    );
}

#[tokio::test]
#[cfg(feature = "s2s-tests")]
async fn test_transaction_discard_after_watch() {
    let base_url = spawn_test_server().await;
    let client = Client::new();
    let client_id = format!(
        "test_client_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let watched_key = format!("tx:watch:{}", timestamp);

    // Set initial value
    client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "kv.set",
            "request_id": "init-1",
            "payload": { "key": watched_key.clone(), "value": "initial" }
        }))
        .send()
        .await
        .unwrap();

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

    // Watch key
    client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "transaction.watch",
            "request_id": "watch-1",
            "payload": {
                "client_id": client_id,
                "keys": [watched_key.clone()]
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
            "request_id": "set-1",
            "payload": {
                "key": format!("tx:other:{}", timestamp),
                "value": "other_value",
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
            "request_id": "discard-1",
            "payload": { "client_id": client_id }
        }))
        .send()
        .await
        .unwrap();

    // Verify the queued command was NOT executed
    let get_res = client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "kv.get",
            "request_id": "verify-1",
            "payload": { "key": format!("tx:other:{}", timestamp) }
        }))
        .send()
        .await
        .unwrap();
    let get_body: serde_json::Value = get_res.json().await.unwrap();
    assert!(get_body["payload"].is_null());

    // Verify we can start a new transaction with the same client_id
    client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "transaction.multi",
            "request_id": "multi-2",
            "payload": { "client_id": client_id }
        }))
        .send()
        .await
        .unwrap();

    // Execute empty transaction
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
    assert_eq!(exec_body["payload"]["results"].as_array().unwrap().len(), 0);
}
