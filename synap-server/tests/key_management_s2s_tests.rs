//! S2S (Server-to-Server) integration tests for Key Management operations
//! These tests require a running Synap server

#[cfg(feature = "s2s-tests")]
#[cfg_attr(not(feature = "s2s-tests"), allow(unused_imports))]
use reqwest::Client;
#[cfg(feature = "s2s-tests")]
#[cfg_attr(not(feature = "s2s-tests"), allow(unused_imports))]
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

#[cfg(feature = "s2s-tests")]
#[cfg_attr(not(feature = "s2s-tests"), allow(dead_code))]
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
async fn test_key_exists_kv() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Test non-existent key
    let res = send_command(
        &client,
        &base_url,
        "key.exists",
        json!({"key": "nonexistent"}),
    )
    .await;

    assert_eq!(res["success"], true);
    assert_eq!(res["payload"]["exists"], false);

    // Create a key
    send_command(
        &client,
        &base_url,
        "kv.set",
        json!({"key": "test_key", "value": "test_value"}),
    )
    .await;

    // Test existing key
    let res = send_command(&client, &base_url, "key.exists", json!({"key": "test_key"})).await;

    assert_eq!(res["success"], true);
    assert_eq!(res["payload"]["exists"], true);
}

#[tokio::test]
#[cfg(feature = "s2s-tests")]
async fn test_key_exists_hash() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Create a hash
    send_command(
        &client,
        &base_url,
        "hash.set",
        json!({"key": "hash_key", "field": "field1", "value": "value1"}),
    )
    .await;

    // Test existing hash key
    let res = send_command(&client, &base_url, "key.exists", json!({"key": "hash_key"})).await;

    assert_eq!(res["success"], true);
    assert_eq!(res["payload"]["exists"], true);
}

#[tokio::test]
#[cfg(feature = "s2s-tests")]
async fn test_key_type() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Test non-existent key
    let res = send_command(
        &client,
        &base_url,
        "key.type",
        json!({"key": "nonexistent"}),
    )
    .await;

    assert_eq!(res["success"], true);
    assert_eq!(res["payload"]["type"], "none");

    // Test string type
    send_command(
        &client,
        &base_url,
        "kv.set",
        json!({"key": "string_key", "value": "value"}),
    )
    .await;

    let res = send_command(&client, &base_url, "key.type", json!({"key": "string_key"})).await;

    assert_eq!(res["success"], true);
    assert_eq!(res["payload"]["type"], "string");

    // Test hash type
    send_command(
        &client,
        &base_url,
        "hash.set",
        json!({"key": "hash_key", "field": "field1", "value": "value1"}),
    )
    .await;

    let res = send_command(&client, &base_url, "key.type", json!({"key": "hash_key"})).await;

    assert_eq!(res["success"], true);
    assert_eq!(res["payload"]["type"], "hash");

    // Test list type
    send_command(
        &client,
        &base_url,
        "list.lpush",
        json!({"key": "list_key", "values": ["value1"]}),
    )
    .await;

    let res = send_command(&client, &base_url, "key.type", json!({"key": "list_key"})).await;

    assert_eq!(res["success"], true);
    assert_eq!(res["payload"]["type"], "list");

    // Test set type
    send_command(
        &client,
        &base_url,
        "set.add",
        json!({"key": "set_key", "members": ["member1"]}),
    )
    .await;

    let res = send_command(&client, &base_url, "key.type", json!({"key": "set_key"})).await;

    assert_eq!(res["success"], true);
    assert_eq!(res["payload"]["type"], "set");

    // Test sorted set type
    send_command(
        &client,
        &base_url,
        "sortedset.zadd",
        json!({"key": "zset_key", "member": "member1", "score": 1.0}),
    )
    .await;

    let res = send_command(&client, &base_url, "key.type", json!({"key": "zset_key"})).await;

    assert_eq!(res["success"], true);
    assert_eq!(res["payload"]["type"], "zset");
}

#[tokio::test]
#[cfg(feature = "s2s-tests")]
async fn test_key_rename() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Create a key
    send_command(
        &client,
        &base_url,
        "kv.set",
        json!({"key": "source_key", "value": "test_value"}),
    )
    .await;

    // Rename the key
    let res = send_command(
        &client,
        &base_url,
        "key.rename",
        json!({"source": "source_key", "destination": "dest_key"}),
    )
    .await;

    assert_eq!(res["success"], true);
    assert_eq!(res["payload"]["success"], true);
    assert_eq!(res["payload"]["source"], "source_key");
    assert_eq!(res["payload"]["destination"], "dest_key");

    // Verify source key doesn't exist
    let exists_res = send_command(
        &client,
        &base_url,
        "key.exists",
        json!({"key": "source_key"}),
    )
    .await;

    assert_eq!(exists_res["payload"]["exists"], false);

    // Verify destination key exists with correct value
    let get_res = send_command(&client, &base_url, "kv.get", json!({"key": "dest_key"})).await;

    assert_eq!(get_res["success"], true);
    let value_str = get_res["payload"].as_str().unwrap();
    assert_eq!(value_str, "\"test_value\"");
}

#[tokio::test]
#[cfg(feature = "s2s-tests")]
async fn test_key_rename_hash() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Create a hash
    send_command(
        &client,
        &base_url,
        "hash.set",
        json!({"key": "source_hash", "field": "field1", "value": "value1"}),
    )
    .await;

    // Rename the hash
    let res = send_command(
        &client,
        &base_url,
        "key.rename",
        json!({"source": "source_hash", "destination": "dest_hash"}),
    )
    .await;

    assert_eq!(res["success"], true);

    // Verify destination hash exists
    let get_res = send_command(
        &client,
        &base_url,
        "hash.get",
        json!({"key": "dest_hash", "field": "field1"}),
    )
    .await;

    assert_eq!(get_res["payload"]["found"], true);
    assert_eq!(get_res["payload"]["value"], "value1");
}

#[tokio::test]
#[cfg(feature = "s2s-tests")]
async fn test_key_renamenx() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Create source key
    send_command(
        &client,
        &base_url,
        "kv.set",
        json!({"key": "source_key", "value": "value1"}),
    )
    .await;

    // Create destination key
    send_command(
        &client,
        &base_url,
        "kv.set",
        json!({"key": "dest_key", "value": "value2"}),
    )
    .await;

    // Try to rename when destination exists (should fail)
    let res = send_command(
        &client,
        &base_url,
        "key.renamenx",
        json!({"source": "source_key", "destination": "dest_key"}),
    )
    .await;

    assert_eq!(res["success"], false);
    assert!(res["error"].as_str().is_some());

    // Verify source_key still exists
    let source_exists = send_command(
        &client,
        &base_url,
        "key.exists",
        json!({"key": "source_key"}),
    )
    .await;
    assert_eq!(source_exists["payload"]["exists"], true);

    // Delete destination and try again
    let del_res = send_command(&client, &base_url, "kv.del", json!({"key": "dest_key"})).await;
    assert_eq!(del_res["success"], true);

    // Verify destination is deleted
    let dest_exists =
        send_command(&client, &base_url, "key.exists", json!({"key": "dest_key"})).await;
    assert_eq!(dest_exists["payload"]["exists"], false);

    // Verify source_key still exists before rename
    let source_exists_before = send_command(
        &client,
        &base_url,
        "key.exists",
        json!({"key": "source_key"}),
    )
    .await;
    assert_eq!(source_exists_before["payload"]["exists"], true);

    // Now rename should succeed
    let res = send_command(
        &client,
        &base_url,
        "key.renamenx",
        json!({"source": "source_key", "destination": "dest_key"}),
    )
    .await;

    assert_eq!(res["success"], true);
    assert_eq!(res["payload"]["success"], true);

    // Verify source_key no longer exists and dest_key exists
    let source_exists_after = send_command(
        &client,
        &base_url,
        "key.exists",
        json!({"key": "source_key"}),
    )
    .await;
    assert_eq!(source_exists_after["payload"]["exists"], false);

    let dest_exists_after =
        send_command(&client, &base_url, "key.exists", json!({"key": "dest_key"})).await;
    assert_eq!(dest_exists_after["payload"]["exists"], true);
}

#[tokio::test]
#[cfg(feature = "s2s-tests")]
async fn test_key_copy() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Create source key
    send_command(
        &client,
        &base_url,
        "kv.set",
        json!({"key": "source_key", "value": "test_value"}),
    )
    .await;

    // Copy the key
    let res = send_command(
        &client,
        &base_url,
        "key.copy",
        json!({"source": "source_key", "destination": "dest_key", "replace": false}),
    )
    .await;

    assert_eq!(res["success"], true);
    assert_eq!(res["payload"]["success"], true);

    // Verify both keys exist
    let source_res = send_command(&client, &base_url, "kv.get", json!({"key": "source_key"})).await;

    let dest_res = send_command(&client, &base_url, "kv.get", json!({"key": "dest_key"})).await;

    assert_eq!(source_res["payload"].as_str().unwrap(), "\"test_value\"");
    assert_eq!(dest_res["payload"].as_str().unwrap(), "\"test_value\"");
}

#[tokio::test]
#[cfg(feature = "s2s-tests")]
async fn test_key_copy_replace() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Create source key
    send_command(
        &client,
        &base_url,
        "kv.set",
        json!({"key": "source_key", "value": "source_value"}),
    )
    .await;

    // Create destination key
    send_command(
        &client,
        &base_url,
        "kv.set",
        json!({"key": "dest_key", "value": "dest_value"}),
    )
    .await;

    // Copy with replace=false (should fail)
    let res = send_command(
        &client,
        &base_url,
        "key.copy",
        json!({"source": "source_key", "destination": "dest_key", "replace": false}),
    )
    .await;

    assert_eq!(res["success"], false);

    // Copy with replace=true (should succeed)
    let res = send_command(
        &client,
        &base_url,
        "key.copy",
        json!({"source": "source_key", "destination": "dest_key", "replace": true}),
    )
    .await;

    assert_eq!(res["success"], true);

    // Verify destination has source value
    let dest_res = send_command(&client, &base_url, "kv.get", json!({"key": "dest_key"})).await;

    assert_eq!(dest_res["payload"].as_str().unwrap(), "\"source_value\"");
}

#[tokio::test]
#[cfg(feature = "s2s-tests")]
async fn test_key_copy_hash() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Create source hash
    send_command(
        &client,
        &base_url,
        "hash.set",
        json!({"key": "source_hash", "field": "field1", "value": "value1"}),
    )
    .await;

    send_command(
        &client,
        &base_url,
        "hash.set",
        json!({"key": "source_hash", "field": "field2", "value": "value2"}),
    )
    .await;

    // Copy the hash
    let res = send_command(
        &client,
        &base_url,
        "key.copy",
        json!({"source": "source_hash", "destination": "dest_hash", "replace": false}),
    )
    .await;

    assert_eq!(res["success"], true);

    // Verify destination hash has all fields
    let field1_res = send_command(
        &client,
        &base_url,
        "hash.get",
        json!({"key": "dest_hash", "field": "field1"}),
    )
    .await;

    let field2_res = send_command(
        &client,
        &base_url,
        "hash.get",
        json!({"key": "dest_hash", "field": "field2"}),
    )
    .await;

    assert_eq!(field1_res["payload"]["found"], true);
    assert_eq!(field1_res["payload"]["value"], "value1");
    assert_eq!(field2_res["payload"]["found"], true);
    assert_eq!(field2_res["payload"]["value"], "value2");
}

#[tokio::test]
#[cfg(feature = "s2s-tests")]
async fn test_key_randomkey() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Test with no keys (should return null)
    let res = send_command(&client, &base_url, "key.randomkey", json!({})).await;

    assert_eq!(res["success"], true);
    assert!(res["payload"]["key"].is_null());

    // Create some keys
    send_command(
        &client,
        &base_url,
        "kv.set",
        json!({"key": "key1", "value": "value1"}),
    )
    .await;

    send_command(
        &client,
        &base_url,
        "kv.set",
        json!({"key": "key2", "value": "value2"}),
    )
    .await;

    send_command(
        &client,
        &base_url,
        "hash.set",
        json!({"key": "hash1", "field": "field1", "value": "value1"}),
    )
    .await;

    // Get random key (should return one of the keys)
    let res = send_command(&client, &base_url, "key.randomkey", json!({})).await;

    assert_eq!(res["success"], true);
    let random_key = res["payload"]["key"].as_str().unwrap();
    assert!(random_key == "key1" || random_key == "key2" || random_key == "hash1");
}

#[tokio::test]
#[cfg(feature = "s2s-tests")]
async fn test_key_operations_across_types() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Create keys of different types
    send_command(
        &client,
        &base_url,
        "kv.set",
        json!({"key": "string_key", "value": "value"}),
    )
    .await;

    send_command(
        &client,
        &base_url,
        "hash.set",
        json!({"key": "hash_key", "field": "field1", "value": "value1"}),
    )
    .await;

    send_command(
        &client,
        &base_url,
        "list.lpush",
        json!({"key": "list_key", "values": ["value1"]}),
    )
    .await;

    // Test EXISTS for all types
    let string_exists = send_command(
        &client,
        &base_url,
        "key.exists",
        json!({"key": "string_key"}),
    )
    .await;
    assert_eq!(string_exists["payload"]["exists"], true);

    let hash_exists =
        send_command(&client, &base_url, "key.exists", json!({"key": "hash_key"})).await;
    assert_eq!(hash_exists["payload"]["exists"], true);

    let list_exists =
        send_command(&client, &base_url, "key.exists", json!({"key": "list_key"})).await;
    assert_eq!(list_exists["payload"]["exists"], true);

    // Test TYPE for all types
    let string_type =
        send_command(&client, &base_url, "key.type", json!({"key": "string_key"})).await;
    assert_eq!(string_type["payload"]["type"], "string");

    let hash_type = send_command(&client, &base_url, "key.type", json!({"key": "hash_key"})).await;
    assert_eq!(hash_type["payload"]["type"], "hash");

    let list_type = send_command(&client, &base_url, "key.type", json!({"key": "list_key"})).await;
    assert_eq!(list_type["payload"]["type"], "list");
}
