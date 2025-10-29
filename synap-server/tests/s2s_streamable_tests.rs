// Server-to-Server StreamableHTTP Protocol Tests
// Tests complete StreamableHTTP command interface

use reqwest::Client;
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use synap_server::{AppState, KVConfig, KVStore, create_router};
use tokio::net::TcpListener;

async fn spawn_test_server() -> String {
    let config = KVConfig {
        allow_flush_commands: true, // Enable FLUSHDB for tests
        ..Default::default()
    };
    let kv_store = Arc::new(KVStore::new(config));
    let hash_store = Arc::new(synap_server::core::HashStore::new());
    let state = AppState {
        kv_store,
        hash_store,
        list_store: Arc::new(synap_server::core::ListStore::new()),
        set_store: Arc::new(synap_server::core::SetStore::new()),
        sorted_set_store: Arc::new(synap_server::core::SortedSetStore::new()),
        queue_manager: None,
        stream_manager: None,
        pubsub_router: None,
        persistence: None,
        consumer_group_manager: None,
        partition_manager: None,
        monitoring: Arc::new(synap_server::monitoring::MonitoringManager::new(
            kv_store.clone(),
            hash_store.clone(),
            Arc::new(synap_server::core::ListStore::new()),
            Arc::new(synap_server::core::SetStore::new()),
            Arc::new(synap_server::core::SortedSetStore::new()),
        )),
    };
    let app = create_router(
        state,
        synap_server::config::RateLimitConfig {
            enabled: false,
            requests_per_second: 100,
            burst_size: 10,
        },
        synap_server::config::McpConfig::default(),
    ); // enable flush commands for tests

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
async fn test_streamable_kv_set() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    let res = send_command(
        &client,
        &base_url,
        "kv.set",
        json!({"key": "user:1", "value": "Alice"}),
    )
    .await;

    assert_eq!(res["success"], true);
    assert!(res["request_id"].as_str().is_some());
    assert_eq!(res["payload"]["success"], true);
}

#[tokio::test]
async fn test_streamable_kv_get() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Setup
    send_command(
        &client,
        &base_url,
        "kv.set",
        json!({"key": "test", "value": "hello"}),
    )
    .await;

    // Test GET
    let res = send_command(&client, &base_url, "kv.get", json!({"key": "test"})).await;

    assert_eq!(res["success"], true);
    // Payload now returns value directly as JSON string
    let value_str = res["payload"].as_str().unwrap();
    assert_eq!(value_str, "\"hello\"");
}

#[tokio::test]
async fn test_streamable_kv_del() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Setup
    send_command(
        &client,
        &base_url,
        "kv.set",
        json!({"key": "to_delete", "value": "temp"}),
    )
    .await;

    // Test DELETE
    let res = send_command(&client, &base_url, "kv.del", json!({"key": "to_delete"})).await;

    assert_eq!(res["success"], true);
    assert_eq!(res["payload"]["deleted"], true);
}

#[tokio::test]
async fn test_streamable_kv_exists() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Test non-existent key
    let res = send_command(
        &client,
        &base_url,
        "kv.exists",
        json!({"key": "nonexistent"}),
    )
    .await;
    assert_eq!(res["payload"]["exists"], false);

    // Setup: SET a key
    send_command(
        &client,
        &base_url,
        "kv.set",
        json!({"key": "existing", "value": "data"}),
    )
    .await;

    // Test existing key
    let res = send_command(&client, &base_url, "kv.exists", json!({"key": "existing"})).await;
    assert_eq!(res["payload"]["exists"], true);
}

#[tokio::test]
async fn test_streamable_kv_incr_decr() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Test INCR (creates key)
    let res = send_command(
        &client,
        &base_url,
        "kv.incr",
        json!({"key": "counter", "amount": 5}),
    )
    .await;

    assert_eq!(res["success"], true);
    assert_eq!(res["payload"]["value"], 5);

    // Test INCR (existing key)
    let res = send_command(
        &client,
        &base_url,
        "kv.incr",
        json!({"key": "counter", "amount": 3}),
    )
    .await;

    assert_eq!(res["payload"]["value"], 8);

    // Test DECR
    let res = send_command(
        &client,
        &base_url,
        "kv.decr",
        json!({"key": "counter", "amount": 2}),
    )
    .await;

    assert_eq!(res["payload"]["value"], 6);
}

#[tokio::test]
async fn test_streamable_kv_mset_mget() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Test MSET
    let res = send_command(
        &client,
        &base_url,
        "kv.mset",
        json!({
            "pairs": [
                {"key": "k1", "value": "v1"},
                {"key": "k2", "value": "v2"},
                {"key": "k3", "value": "v3"}
            ]
        }),
    )
    .await;

    assert_eq!(res["success"], true);

    // Test MGET
    let res = send_command(
        &client,
        &base_url,
        "kv.mget",
        json!({"keys": ["k1", "k2", "k3", "k4"]}),
    )
    .await;

    assert_eq!(res["success"], true);
    let values = res["payload"]["values"].as_array().unwrap();
    assert_eq!(values.len(), 4);
    assert_eq!(values[0], "v1");
    assert_eq!(values[1], "v2");
    assert_eq!(values[2], "v3");
    assert_eq!(values[3], serde_json::Value::Null);
}

#[tokio::test]
async fn test_streamable_kv_scan() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Setup: Create keys with prefix
    for i in 1..=10 {
        send_command(
            &client,
            &base_url,
            "kv.set",
            json!({"key": format!("user:{}", i), "value": format!("User {}", i)}),
        )
        .await;
    }

    // Test SCAN with prefix
    let res = send_command(
        &client,
        &base_url,
        "kv.scan",
        json!({"prefix": "user:", "limit": 100}),
    )
    .await;

    assert_eq!(res["success"], true);
    let keys = res["payload"]["keys"].as_array().unwrap();
    assert_eq!(keys.len(), 10);
}

#[tokio::test]
async fn test_streamable_kv_keys() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Setup
    for i in 1..=5 {
        send_command(
            &client,
            &base_url,
            "kv.set",
            json!({"key": format!("item:{}", i), "value": i}),
        )
        .await;
    }

    // Test KEYS
    let res = send_command(&client, &base_url, "kv.keys", json!({})).await;

    assert_eq!(res["success"], true);
    let keys = res["payload"]["keys"].as_array().unwrap();
    assert_eq!(keys.len(), 5);
}

#[tokio::test]
async fn test_streamable_kv_dbsize() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Test empty database
    let res = send_command(&client, &base_url, "kv.dbsize", json!({})).await;
    assert_eq!(res["payload"]["size"], 0);

    // Add keys
    for i in 1..=7 {
        send_command(
            &client,
            &base_url,
            "kv.set",
            json!({"key": format!("k{}", i), "value": i}),
        )
        .await;
    }

    // Test with keys
    let res = send_command(&client, &base_url, "kv.dbsize", json!({})).await;
    assert_eq!(res["payload"]["size"], 7);
}

#[tokio::test]
async fn test_streamable_kv_flushdb() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Setup: Add multiple keys
    for i in 1..=20 {
        send_command(
            &client,
            &base_url,
            "kv.set",
            json!({"key": format!("key{}", i), "value": i}),
        )
        .await;
    }

    // Verify keys exist
    let res = send_command(&client, &base_url, "kv.dbsize", json!({})).await;
    assert_eq!(res["payload"]["size"], 20);

    // Test FLUSHDB
    let res = send_command(&client, &base_url, "kv.flushdb", json!({})).await;
    assert!(
        res["success"].as_bool().unwrap_or(false),
        "FLUSHDB should succeed"
    );
    assert!(
        res["payload"]["flushed"].as_u64().is_some(),
        "Should return flushed count"
    );

    // Verify empty
    let res = send_command(&client, &base_url, "kv.dbsize", json!({})).await;
    assert_eq!(res["payload"]["size"], 0);
}

#[tokio::test]
async fn test_streamable_kv_expire_persist() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Setup
    send_command(
        &client,
        &base_url,
        "kv.set",
        json!({"key": "temp_key", "value": "data"}),
    )
    .await;

    // Test EXPIRE
    let res = send_command(
        &client,
        &base_url,
        "kv.expire",
        json!({"key": "temp_key", "ttl": 60}),
    )
    .await;

    assert_eq!(res["success"], true);
    assert_eq!(res["payload"]["result"], true);

    // Test PERSIST
    let res = send_command(&client, &base_url, "kv.persist", json!({"key": "temp_key"})).await;

    assert_eq!(res["success"], true);
    assert_eq!(res["payload"]["result"], true);
}

#[tokio::test]
async fn test_streamable_error_unknown_command() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    let res = send_command(&client, &base_url, "kv.unknown", json!({})).await;

    assert_eq!(res["success"], false);
    assert!(res["error"].as_str().is_some());
    assert!(res["error"].as_str().unwrap().contains("Unknown command"));
}

#[tokio::test]
async fn test_streamable_error_missing_params() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // kv.set without required fields
    let res = send_command(&client, &base_url, "kv.set", json!({"key": "test"})).await;

    assert_eq!(res["success"], false);
    assert!(res["error"].as_str().unwrap().contains("Missing"));
}

#[tokio::test]
async fn test_streamable_request_id_tracking() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    let request_id = "custom-request-123";

    let res = client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "kv.set",
            "request_id": request_id,
            "payload": {"key": "test", "value": "data"}
        }))
        .send()
        .await
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap();

    assert_eq!(res["request_id"], request_id);
}

#[tokio::test]
async fn test_streamable_complete_workflow() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // 1. SET multiple keys
    for i in 1..=5 {
        let res = send_command(
            &client,
            &base_url,
            "kv.set",
            json!({"key": format!("item:{}", i), "value": i * 10}),
        )
        .await;
        assert_eq!(res["success"], true);
    }

    // 2. GET values
    for i in 1..=5 {
        let res = send_command(
            &client,
            &base_url,
            "kv.get",
            json!({"key": format!("item:{}", i)}),
        )
        .await;
        // Payload returns value directly as JSON string
        let value_str = res["payload"].as_str().unwrap();
        let value: i64 = serde_json::from_str(value_str).unwrap();
        assert_eq!(value, i * 10);
    }

    // 3. SCAN with prefix
    let res = send_command(&client, &base_url, "kv.scan", json!({"prefix": "item:"})).await;
    assert_eq!(res["payload"]["keys"].as_array().unwrap().len(), 5);

    // 4. DBSIZE
    let res = send_command(&client, &base_url, "kv.dbsize", json!({})).await;
    assert_eq!(res["payload"]["size"], 5);

    // 5. DELETE via MDEL
    let res = send_command(
        &client,
        &base_url,
        "kv.mdel",
        json!({"keys": ["item:1", "item:2", "item:3"]}),
    )
    .await;
    assert_eq!(res["payload"]["deleted"], 3);

    // 6. Final DBSIZE
    let res = send_command(&client, &base_url, "kv.dbsize", json!({})).await;
    assert_eq!(res["payload"]["size"], 2);

    // 7. FLUSHDB
    let res = send_command(&client, &base_url, "kv.flushdb", json!({})).await;
    assert!(
        res["success"].as_bool().unwrap_or(false),
        "FLUSHDB should succeed"
    );
    assert!(
        res["payload"]["flushed"].as_u64().is_some(),
        "Should return flushed count"
    );

    // 8. Verify empty
    let res = send_command(&client, &base_url, "kv.dbsize", json!({})).await;
    assert_eq!(res["payload"]["size"], 0);
}

#[tokio::test]
async fn test_streamable_batch_operations() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // MSET with 100 keys
    let mut pairs = vec![];
    for i in 1..=100 {
        pairs.push(json!({"key": format!("batch:{}", i), "value": format!("value{}", i)}));
    }

    let res = send_command(&client, &base_url, "kv.mset", json!({"pairs": pairs})).await;
    assert_eq!(res["success"], true);

    // MGET all keys
    let keys: Vec<String> = (1..=100).map(|i| format!("batch:{}", i)).collect();
    let res = send_command(&client, &base_url, "kv.mget", json!({"keys": keys})).await;

    let values = res["payload"]["values"].as_array().unwrap();
    assert_eq!(values.len(), 100);
    assert_eq!(values[0], "value1");
    assert_eq!(values[99], "value100");
}

#[tokio::test]
async fn test_streamable_concurrent_commands() {
    let base_url = spawn_test_server().await;
    let client = Arc::new(Client::new());

    // Send 20 concurrent commands
    let mut handles = vec![];
    for i in 0..20 {
        let client = Arc::clone(&client);
        let url = base_url.clone();

        let handle = tokio::spawn(async move {
            send_command(
                &client,
                &url,
                "kv.set",
                json!({"key": format!("concurrent:{}", i), "value": i}),
            )
            .await
        });

        handles.push(handle);
    }

    // Wait for all
    for handle in handles {
        let res = handle.await.unwrap();
        assert_eq!(res["success"], true);
    }

    // Verify count
    let res = send_command(&client, &base_url, "kv.dbsize", json!({})).await;
    assert_eq!(res["payload"]["size"], 20);
}

#[tokio::test]
async fn test_streamable_stats_command() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Perform various operations
    send_command(
        &client,
        &base_url,
        "kv.set",
        json!({"key": "k1", "value": "v1"}),
    )
    .await;
    send_command(&client, &base_url, "kv.get", json!({"key": "k1"})).await;
    send_command(&client, &base_url, "kv.get", json!({"key": "nonexistent"})).await;
    send_command(&client, &base_url, "kv.del", json!({"key": "k1"})).await;

    // Get stats
    let res = send_command(&client, &base_url, "kv.stats", json!({})).await;

    assert_eq!(res["success"], true);
    assert!(res["payload"]["operations"]["sets"].as_u64().unwrap() >= 1);
    assert!(res["payload"]["operations"]["gets"].as_u64().unwrap() >= 2);
    assert!(res["payload"]["operations"]["dels"].as_u64().unwrap() >= 1);
    assert!(res["payload"]["hit_rate"].as_f64().is_some());
}

#[tokio::test]
async fn test_streamable_ttl_workflow() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // SET with TTL via StreamableHTTP
    let res = send_command(
        &client,
        &base_url,
        "kv.set",
        json!({"key": "ttl_test", "value": "temporary", "ttl": 1}),
    )
    .await;
    assert_eq!(res["success"], true);

    // GET immediately
    let res = send_command(&client, &base_url, "kv.get", json!({"key": "ttl_test"})).await;
    // Payload returns value directly
    let value_str = res["payload"].as_str().unwrap();
    assert!(!value_str.is_empty(), "Expected value to be found");

    // Wait for expiration
    tokio::time::sleep(Duration::from_secs(2)).await;

    // GET after expiration
    let res = send_command(&client, &base_url, "kv.get", json!({"key": "ttl_test"})).await;
    // Payload returns null for not found
    assert_eq!(res["payload"], serde_json::Value::Null);
}

#[tokio::test]
async fn test_streamable_mdel_command() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Setup: Create 10 keys
    for i in 1..=10 {
        send_command(
            &client,
            &base_url,
            "kv.set",
            json!({"key": format!("del:{}", i), "value": i}),
        )
        .await;
    }

    // Delete 5 keys (3 exist, 2 don't)
    let res = send_command(
        &client,
        &base_url,
        "kv.mdel",
        json!({"keys": ["del:1", "del:2", "del:3", "nonexistent1", "nonexistent2"]}),
    )
    .await;

    assert_eq!(res["success"], true);
    assert_eq!(res["payload"]["deleted"], 3);

    // Verify remaining
    let res = send_command(&client, &base_url, "kv.dbsize", json!({})).await;
    assert_eq!(res["payload"]["size"], 7);
}
