// Hash Integration Tests
// End-to-end tests for hash operations via REST API, StreamableHTTP, and MCP

use reqwest::Client;
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use synap_server::persistence::PersistenceLayer;
use synap_server::persistence::types::{FsyncMode, PersistenceConfig, SnapshotConfig, WALConfig};
use synap_server::{
    AppState, KVConfig, KVStore, QueueConfig, QueueManager, ServerConfig, create_router,
};
use tokio::net::TcpListener;

/// Spawn a test server with hash support
async fn spawn_test_server() -> String {
    let config = ServerConfig::default();
    let kv_store = Arc::new(KVStore::new(config.to_kv_config()));
    let hash_store = Arc::new(synap_server::core::HashStore::new());
    let queue_manager = Arc::new(QueueManager::new(QueueConfig::default()));

    let app_state = AppState {
        kv_store,
        hash_store,
        list_store: Arc::new(synap_server::core::ListStore::new()),
        queue_manager: Some(queue_manager),
        stream_manager: None,
        pubsub_router: None,
        persistence: None,
        consumer_group_manager: None,
        partition_manager: None,
    };

    let app = create_router(
        app_state,
        synap_server::config::RateLimitConfig {
            enabled: false,
            requests_per_second: 100,
            burst_size: 10,
        },
    );

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    tokio::time::sleep(Duration::from_millis(100)).await;

    format!("http://{}:{}", addr.ip(), addr.port())
}

// ==================== REST API Integration Tests ====================

#[tokio::test]
async fn test_hash_set_get_rest() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // SET a field
    let set_resp = client
        .post(format!("{}/hash/user:1000/set", base_url))
        .json(&json!({
            "field": "name",
            "value": "Alice"
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(set_resp.status(), 200);
    let set_body: serde_json::Value = set_resp.json().await.unwrap();
    assert_eq!(set_body["created"], true);

    // GET the field
    let get_resp = client
        .get(format!("{}/hash/user:1000/name", base_url))
        .send()
        .await
        .unwrap();

    assert_eq!(get_resp.status(), 200);
    let get_body: serde_json::Value = get_resp.json().await.unwrap();
    assert_eq!(get_body, "Alice");
}

#[tokio::test]
async fn test_hash_mset_getall_rest() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // MSET multiple fields
    let mset_resp = client
        .post(format!("{}/hash/user:2000/mset", base_url))
        .json(&json!({
            "fields": {
                "name": "Bob",
                "age": 25,
                "email": "bob@example.com"
            }
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(mset_resp.status(), 200);

    // GETALL fields
    let getall_resp = client
        .get(format!("{}/hash/user:2000/getall", base_url))
        .send()
        .await
        .unwrap();

    assert_eq!(getall_resp.status(), 200);
    let fields: HashMap<String, serde_json::Value> = getall_resp.json().await.unwrap();
    assert_eq!(fields.len(), 3);
    assert_eq!(fields["name"], "Bob");
    assert_eq!(fields["age"], 25);
}

#[tokio::test]
async fn test_hash_del_rest() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Setup: Create hash with fields
    client
        .post(format!("{}/hash/user:3000/mset", base_url))
        .json(&json!({
            "fields": {
                "name": "Charlie",
                "age": 35,
                "email": "charlie@example.com"
            }
        }))
        .send()
        .await
        .unwrap();

    // DELETE fields
    let del_resp = client
        .delete(format!("{}/hash/user:3000/del", base_url))
        .json(&json!({
            "fields": ["email", "age"]
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(del_resp.status(), 200);
    let del_body: serde_json::Value = del_resp.json().await.unwrap();
    assert_eq!(del_body["deleted"], 2);

    // Verify only name remains
    let len_resp = client
        .get(format!("{}/hash/user:3000/len", base_url))
        .send()
        .await
        .unwrap();

    let len_body: serde_json::Value = len_resp.json().await.unwrap();
    assert_eq!(len_body["length"], 1);
}

#[tokio::test]
async fn test_hash_incrby_rest() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // INCRBY starting from 0
    let incr1 = client
        .post(format!("{}/hash/stats:user:1000/incrby", base_url))
        .json(&json!({
            "field": "login_count",
            "increment": 1
        }))
        .send()
        .await
        .unwrap();

    let body1: serde_json::Value = incr1.json().await.unwrap();
    assert_eq!(body1["value"], 1);

    // INCRBY again
    let incr2 = client
        .post(format!("{}/hash/stats:user:1000/incrby", base_url))
        .json(&json!({
            "field": "login_count",
            "increment": 5
        }))
        .send()
        .await
        .unwrap();

    let body2: serde_json::Value = incr2.json().await.unwrap();
    assert_eq!(body2["value"], 6);
}

#[tokio::test]
async fn test_hash_exists_rest() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Setup
    client
        .post(format!("{}/hash/user:4000/set", base_url))
        .json(&json!({
            "field": "name",
            "value": "David"
        }))
        .send()
        .await
        .unwrap();

    // EXISTS - existing field
    let exists_resp = client
        .get(format!("{}/hash/user:4000/name/exists", base_url))
        .send()
        .await
        .unwrap();

    let exists_body: serde_json::Value = exists_resp.json().await.unwrap();
    assert_eq!(exists_body["exists"], true);

    // EXISTS - non-existent field
    let not_exists_resp = client
        .get(format!("{}/hash/user:4000/age/exists", base_url))
        .send()
        .await
        .unwrap();

    let not_exists_body: serde_json::Value = not_exists_resp.json().await.unwrap();
    assert_eq!(not_exists_body["exists"], false);
}

#[tokio::test]
async fn test_hash_keys_vals_rest() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Setup
    client
        .post(format!("{}/hash/user:5000/mset", base_url))
        .json(&json!({
            "fields": {
                "name": "Eve",
                "age": 28
            }
        }))
        .send()
        .await
        .unwrap();

    // KEYS
    let keys_resp = client
        .get(format!("{}/hash/user:5000/keys", base_url))
        .send()
        .await
        .unwrap();

    let keys: Vec<String> = keys_resp.json().await.unwrap();
    assert_eq!(keys.len(), 2);
    assert!(keys.contains(&String::from("name")));
    assert!(keys.contains(&String::from("age")));

    // VALS
    let vals_resp = client
        .get(format!("{}/hash/user:5000/vals", base_url))
        .send()
        .await
        .unwrap();

    let vals: Vec<serde_json::Value> = vals_resp.json().await.unwrap();
    assert_eq!(vals.len(), 2);
}

#[tokio::test]
async fn test_hash_stats_rest() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Perform some operations
    client
        .post(format!("{}/hash/user:6000/set", base_url))
        .json(&json!({"field": "name", "value": "Frank"}))
        .send()
        .await
        .unwrap();

    client
        .get(format!("{}/hash/user:6000/name", base_url))
        .send()
        .await
        .unwrap();

    // GET stats
    let stats_resp = client
        .get(format!("{}/hash/stats", base_url))
        .send()
        .await
        .unwrap();

    assert_eq!(stats_resp.status(), 200);
    let stats: serde_json::Value = stats_resp.json().await.unwrap();
    assert!(stats["operations"]["hset_count"].as_u64().unwrap() >= 1);
    assert!(stats["operations"]["hget_count"].as_u64().unwrap() >= 1);
}

// ==================== StreamableHTTP Integration Tests ====================

#[tokio::test]
async fn test_hash_streamable_http() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // hash.set command
    let set_resp = client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "hash.set",
            "request_id": "test-1",
            "payload": {
                "key": "user:7000",
                "field": "name",
                "value": "Grace"
            }
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(set_resp.status(), 200);
    let set_body: serde_json::Value = set_resp.json().await.unwrap();
    assert_eq!(set_body["success"], true);
    assert_eq!(set_body["payload"]["created"], true);

    // hash.get command
    let get_resp = client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "hash.get",
            "request_id": "test-2",
            "payload": {
                "key": "user:7000",
                "field": "name"
            }
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(get_resp.status(), 200);
    let get_body: serde_json::Value = get_resp.json().await.unwrap();
    assert_eq!(get_body["success"], true);
    assert_eq!(get_body["payload"]["found"], true);
    assert_eq!(get_body["payload"]["value"], "Grace");
}

#[tokio::test]
async fn test_hash_mset_getall_streamable() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // hash.mset command
    let mset_resp = client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "hash.mset",
            "request_id": "test-3",
            "payload": {
                "key": "user:8000",
                "fields": {
                    "name": "Henry",
                    "age": 40,
                    "city": "New York"
                }
            }
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(mset_resp.status(), 200);

    // hash.getall command
    let getall_resp = client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "hash.getall",
            "request_id": "test-4",
            "payload": {
                "key": "user:8000"
            }
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(getall_resp.status(), 200);
    let getall_body: serde_json::Value = getall_resp.json().await.unwrap();
    assert_eq!(getall_body["success"], true);
    let fields = &getall_body["payload"]["fields"];
    assert_eq!(fields["name"], "Henry");
    assert_eq!(fields["age"], 40);
}

#[tokio::test]
async fn test_hash_incrby_streamable() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // hash.incrby command
    let incr_resp = client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "hash.incrby",
            "request_id": "test-5",
            "payload": {
                "key": "stats:user:9000",
                "field": "views",
                "increment": 10
            }
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(incr_resp.status(), 200);
    let incr_body: serde_json::Value = incr_resp.json().await.unwrap();
    assert_eq!(incr_body["success"], true);
    assert_eq!(incr_body["payload"]["value"], 10);

    // Increment again
    let incr2_resp = client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "hash.incrby",
            "request_id": "test-6",
            "payload": {
                "key": "stats:user:9000",
                "field": "views",
                "increment": 5
            }
        }))
        .send()
        .await
        .unwrap();

    let incr2_body: serde_json::Value = incr2_resp.json().await.unwrap();
    assert_eq!(incr2_body["payload"]["value"], 15);
}

#[tokio::test]
async fn test_hash_del_streamable() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Setup
    client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "hash.mset",
            "request_id": "test-7",
            "payload": {
                "key": "user:10000",
                "fields": {"name": "Ivy", "age": 22, "email": "ivy@example.com"}
            }
        }))
        .send()
        .await
        .unwrap();

    // hash.del command
    let del_resp = client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "hash.del",
            "request_id": "test-8",
            "payload": {
                "key": "user:10000",
                "fields": ["email"]
            }
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(del_resp.status(), 200);
    let del_body: serde_json::Value = del_resp.json().await.unwrap();
    assert_eq!(del_body["success"], true);
    assert_eq!(del_body["payload"]["deleted"], 1);
}

// ==================== Persistence Integration Tests ====================

#[tokio::test]
async fn test_hash_with_persistence_recovery() {
    use tempfile::TempDir;

    let temp_dir = TempDir::new().unwrap();
    let wal_path = temp_dir.path().join("synap.wal");
    let snapshot_dir = temp_dir.path().join("snapshots");
    std::fs::create_dir_all(&snapshot_dir).unwrap();

    let persist_config = PersistenceConfig {
        enabled: true,
        wal: WALConfig {
            enabled: true,
            path: wal_path.to_path_buf(),
            buffer_size_kb: 64,
            fsync_mode: FsyncMode::Always,
            fsync_interval_ms: 1000,
            max_size_mb: 1024,
        },
        snapshot: SnapshotConfig {
            enabled: true,
            directory: snapshot_dir.to_path_buf(),
            interval_secs: 3600,
            operation_threshold: 10000,
            max_snapshots: 3,
            compression: false,
        },
    };

    // First run: Create hash data
    {
        let persistence = Arc::new(PersistenceLayer::new(persist_config.clone()).await.unwrap());
        let hash_store = Arc::new(synap_server::core::HashStore::new());

        // Set hash fields
        hash_store
            .hset("user:1000", "name", b"Alice".to_vec())
            .unwrap();
        hash_store.hset("user:1000", "age", b"30".to_vec()).unwrap();

        // Log to WAL
        persistence
            .log_hash_set(
                "user:1000".to_string(),
                "name".to_string(),
                b"Alice".to_vec(),
            )
            .await
            .unwrap();
        persistence
            .log_hash_set("user:1000".to_string(), "age".to_string(), b"30".to_vec())
            .await
            .unwrap();

        // Flush WAL
        drop(persistence);
    }

    // Second run: Recover from WAL
    {
        let kv_config = KVConfig::default();
        let queue_config = QueueConfig::default();

        let (_, hash_store, _, _, _) =
            synap_server::persistence::recover(&persist_config, kv_config, queue_config)
                .await
                .unwrap();

        let hash_store = hash_store.unwrap();

        // Verify recovered data
        let name = hash_store.hget("user:1000", "name").unwrap();
        assert_eq!(name, Some(b"Alice".to_vec()));

        let age = hash_store.hget("user:1000", "age").unwrap();
        assert_eq!(age, Some(b"30".to_vec()));

        let len = hash_store.hlen("user:1000").unwrap();
        assert_eq!(len, 2);
    }
}

#[tokio::test]
async fn test_hash_hincrby_persistence() {
    use tempfile::TempDir;

    let temp_dir = TempDir::new().unwrap();
    let wal_path = temp_dir.path().join("synap.wal");
    let snapshot_dir = temp_dir.path().join("snapshots");
    std::fs::create_dir_all(&snapshot_dir).unwrap();

    let persist_config = PersistenceConfig {
        enabled: true,
        wal: WALConfig {
            enabled: true,
            path: wal_path.to_path_buf(),
            buffer_size_kb: 64,
            fsync_mode: FsyncMode::Always,
            fsync_interval_ms: 1000,
            max_size_mb: 1024,
        },
        snapshot: SnapshotConfig {
            enabled: true,
            directory: snapshot_dir.to_path_buf(),
            interval_secs: 3600,
            operation_threshold: 10000,
            max_snapshots: 3,
            compression: false,
        },
    };

    // First run: Increment counter
    {
        let persistence = Arc::new(PersistenceLayer::new(persist_config.clone()).await.unwrap());
        let hash_store = Arc::new(synap_server::core::HashStore::new());

        hash_store.hincrby("stats:app", "requests", 100).unwrap();
        persistence
            .log_hash_incrby("stats:app".to_string(), "requests".to_string(), 100)
            .await
            .unwrap();

        hash_store.hincrby("stats:app", "requests", 50).unwrap();
        persistence
            .log_hash_incrby("stats:app".to_string(), "requests".to_string(), 50)
            .await
            .unwrap();

        drop(persistence);
    }

    // Second run: Recover and verify
    {
        let kv_config = KVConfig::default();
        let queue_config = QueueConfig::default();

        let (_, hash_store, _, _, _) =
            synap_server::persistence::recover(&persist_config, kv_config, queue_config)
                .await
                .unwrap();

        let hash_store = hash_store.unwrap();

        // Verify counter value
        let value = hash_store.hget("stats:app", "requests").unwrap().unwrap();
        let count_str = String::from_utf8(value).unwrap();
        let count: i64 = count_str.parse().unwrap();
        assert_eq!(count, 150);
    }
}

// ==================== Concurrent Access Tests ====================

#[tokio::test]
async fn test_hash_concurrent_rest_access() {
    let base_url = spawn_test_server().await;
    let client = Arc::new(Client::new());

    let mut handles = vec![];

    // Spawn 10 concurrent tasks
    for i in 0..10 {
        let base_url = base_url.clone();
        let client = Arc::clone(&client);

        let handle = tokio::spawn(async move {
            // Each task sets its own field
            for j in 0..5 {
                let field = format!("field_{}_{}", i, j);
                let value = format!("value_{}_{}", i, j);

                client
                    .post(format!("{}/hash/concurrent:test/set", base_url))
                    .json(&json!({"field": field, "value": value}))
                    .send()
                    .await
                    .unwrap();
            }
        });

        handles.push(handle);
    }

    // Wait for all tasks
    for handle in handles {
        handle.await.unwrap();
    }

    // Verify all fields were set
    let len_resp = client
        .get(format!("{}/hash/concurrent:test/len", base_url))
        .send()
        .await
        .unwrap();

    let len_body: serde_json::Value = len_resp.json().await.unwrap();
    assert_eq!(len_body["length"], 50); // 10 tasks * 5 fields
}

// ==================== Error Handling Tests ====================

#[tokio::test]
async fn test_hash_invalid_increment() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Set non-numeric field
    client
        .post(format!("{}/hash/user:11000/set", base_url))
        .json(&json!({
            "field": "name",
            "value": "NotANumber"
        }))
        .send()
        .await
        .unwrap();

    // Try to increment (should fail)
    let incr_resp = client
        .post(format!("{}/hash/user:11000/incrby", base_url))
        .json(&json!({
            "field": "name",
            "increment": 1
        }))
        .send()
        .await
        .unwrap();

    // Should return error
    assert!(!incr_resp.status().is_success());
}

#[tokio::test]
async fn test_hash_mget_partial_fields() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Setup with only 2 fields
    client
        .post(format!("{}/hash/user:12000/mset", base_url))
        .json(&json!({
            "fields": {
                "name": "John",
                "age": 45
            }
        }))
        .send()
        .await
        .unwrap();

    // MGET with some non-existent fields
    let mget_resp = client
        .post(format!("{}/hash/user:12000/mget", base_url))
        .json(&json!({
            "fields": ["name", "nonexistent", "age", "also_nonexistent"]
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(mget_resp.status(), 200);
    let values: Vec<Option<serde_json::Value>> = mget_resp.json().await.unwrap();
    assert_eq!(values.len(), 4);
    assert_eq!(values[0], Some(json!("John")));
    assert_eq!(values[1], None);
    assert_eq!(values[2], Some(json!(45)));
    assert_eq!(values[3], None);
}

// ==================== Edge Cases ====================

#[tokio::test]
async fn test_hash_empty_operations() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // GETALL on non-existent hash
    let getall_resp = client
        .get(format!("{}/hash/nonexistent:key/getall", base_url))
        .send()
        .await
        .unwrap();

    let fields: HashMap<String, serde_json::Value> = getall_resp.json().await.unwrap();
    assert!(fields.is_empty());

    // HLEN on non-existent hash
    let len_resp = client
        .get(format!("{}/hash/nonexistent:key/len", base_url))
        .send()
        .await
        .unwrap();

    let len_body: serde_json::Value = len_resp.json().await.unwrap();
    assert_eq!(len_body["length"], 0);

    // HDEL on non-existent hash
    let del_resp = client
        .delete(format!("{}/hash/nonexistent:key/del", base_url))
        .json(&json!({
            "fields": ["any", "fields"]
        }))
        .send()
        .await
        .unwrap();

    let del_body: serde_json::Value = del_resp.json().await.unwrap();
    assert_eq!(del_body["deleted"], 0);
}

#[tokio::test]
async fn test_hash_hsetnx_conditional() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // First SETNX should succeed
    let setnx1 = client
        .post(format!("{}/hash/user:13000/setnx", base_url))
        .json(&json!({
            "field": "name",
            "value": "Alice"
        }))
        .send()
        .await
        .unwrap();

    let body1: serde_json::Value = setnx1.json().await.unwrap();
    assert_eq!(body1["created"], true);

    // Second SETNX should fail (field exists)
    let setnx2 = client
        .post(format!("{}/hash/user:13000/setnx", base_url))
        .json(&json!({
            "field": "name",
            "value": "Bob"
        }))
        .send()
        .await
        .unwrap();

    let body2: serde_json::Value = setnx2.json().await.unwrap();
    assert_eq!(body2["created"], false);

    // Verify original value wasn't changed
    let get_resp = client
        .get(format!("{}/hash/user:13000/name", base_url))
        .send()
        .await
        .unwrap();

    let value: serde_json::Value = get_resp.json().await.unwrap();
    assert_eq!(value, "Alice");
}

// ==================== Performance Tests ====================

#[tokio::test]
async fn test_hash_large_field_count() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Create hash with 1000 fields
    let mut fields = HashMap::new();
    for i in 0..1000 {
        fields.insert(format!("field_{}", i), json!(format!("value_{}", i)));
    }

    let mset_resp = client
        .post(format!("{}/hash/large:test/mset", base_url))
        .json(&json!({ "fields": fields }))
        .timeout(Duration::from_secs(5))
        .send()
        .await
        .unwrap();

    assert_eq!(mset_resp.status(), 200);

    // Verify count
    let len_resp = client
        .get(format!("{}/hash/large:test/len", base_url))
        .send()
        .await
        .unwrap();

    let len_body: serde_json::Value = len_resp.json().await.unwrap();
    assert_eq!(len_body["length"], 1000);

    // GETALL should complete within reasonable time
    let start = std::time::Instant::now();
    let getall_resp = client
        .get(format!("{}/hash/large:test/getall", base_url))
        .timeout(Duration::from_secs(5))
        .send()
        .await
        .unwrap();

    let elapsed = start.elapsed();
    assert!(
        elapsed < Duration::from_secs(1),
        "GETALL took too long: {:?}",
        elapsed
    );

    assert_eq!(getall_resp.status(), 200);
    let all_fields: HashMap<String, serde_json::Value> = getall_resp.json().await.unwrap();
    assert_eq!(all_fields.len(), 1000);
}

#[tokio::test]
async fn test_hash_large_value_size() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Create large value (1MB)
    let large_value = "x".repeat(1024 * 1024);

    let set_resp = client
        .post(format!("{}/hash/large:value/set", base_url))
        .json(&json!({
            "field": "data",
            "value": large_value
        }))
        .timeout(Duration::from_secs(5))
        .send()
        .await
        .unwrap();

    assert_eq!(set_resp.status(), 200);

    // Retrieve large value
    let get_resp = client
        .get(format!("{}/hash/large:value/data", base_url))
        .timeout(Duration::from_secs(5))
        .send()
        .await
        .unwrap();

    assert_eq!(get_resp.status(), 200);
    let retrieved: String = get_resp.json().await.unwrap();
    assert_eq!(retrieved.len(), 1024 * 1024);
}
