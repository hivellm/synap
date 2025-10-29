// Set Integration Tests
// End-to-end tests for set operations via REST API

use reqwest::Client;
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use synap_server::{AppState, KVStore, QueueConfig, QueueManager, ServerConfig, create_router};
use tokio::net::TcpListener;

/// Spawn a test server with set support
async fn spawn_test_server() -> String {
    let config = ServerConfig::default();
    let kv_store = Arc::new(KVStore::new(config.to_kv_config()));
    let hash_store = Arc::new(synap_server::core::HashStore::new());
    let list_store = Arc::new(synap_server::core::ListStore::new());
    let set_store = Arc::new(synap_server::core::SetStore::new());
    let queue_manager = Arc::new(QueueManager::new(QueueConfig::default()));

    let sorted_set_store = Arc::new(synap_server::core::SortedSetStore::new());

    let app_state = AppState {
        kv_store,
        hash_store,
        list_store,
        set_store,
        sorted_set_store,
        queue_manager: Some(queue_manager),
        stream_manager: None,
        pubsub_router: None,
        persistence: None,
        consumer_group_manager: None,
        partition_manager: None,
        monitoring: Arc::new(synap_server::monitoring::MonitoringManager::new(
            kv_store.clone(),
            hash_store.clone(),
            list_store.clone(),
            set_store.clone(),
            sorted_set_store.clone(),
        )),
    };

    let app = create_router(
        app_state,
        synap_server::config::RateLimitConfig {
            enabled: false,
            requests_per_second: 100,
            burst_size: 10,
        },
        synap_server::config::McpConfig::default(),
    );

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    tokio::time::sleep(Duration::from_millis(100)).await;

    format!("http://{}:{}", addr.ip(), addr.port())
}

#[tokio::test]
async fn test_set_add_and_members() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Add members to set
    let resp = client
        .post(format!("{}/set/test_set/add", base_url))
        .json(&json!({"members": ["apple", "banana", "cherry"]}))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["added"], 3);

    // Get members
    let resp = client
        .get(format!("{}/set/test_set/members", base_url))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["members"].as_array().unwrap().len(), 3);
}

#[tokio::test]
async fn test_set_rem_members() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Add members
    client
        .post(format!("{}/set/test_set2/add", base_url))
        .json(&json!({"members": ["a", "b", "c", "d", "e"]}))
        .send()
        .await
        .unwrap();

    // Remove members
    let resp = client
        .post(format!("{}/set/test_set2/rem", base_url))
        .json(&json!({"members": ["b", "d"]}))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);

    // Verify remaining members
    let resp = client
        .get(format!("{}/set/test_set2/members", base_url))
        .send()
        .await
        .unwrap();

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["members"].as_array().unwrap().len(), 3);
}

#[tokio::test]
async fn test_set_ismember() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Add members
    client
        .post(format!("{}/set/test_set3/add", base_url))
        .json(&json!({"members": ["member1", "member2"]}))
        .send()
        .await
        .unwrap();

    // Check member exists
    let resp = client
        .post(format!("{}/set/test_set3/ismember", base_url))
        .json(&json!({"member": "member1"}))
        .send()
        .await
        .unwrap();

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["is_member"], true);

    // Check member doesn't exist
    let resp = client
        .post(format!("{}/set/test_set3/ismember", base_url))
        .json(&json!({"member": "nonexistent"}))
        .send()
        .await
        .unwrap();

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["is_member"], false);
}

#[tokio::test]
async fn test_set_card() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Add members
    client
        .post(format!("{}/set/test_set4/add", base_url))
        .json(&json!({"members": [1, 2, 3, 4, 5]}))
        .send()
        .await
        .unwrap();

    // Get cardinality
    let resp = client
        .get(format!("{}/set/test_set4/card", base_url))
        .send()
        .await
        .unwrap();

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["count"], 5);
}

#[tokio::test]
async fn test_set_intersection() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Create set1
    client
        .post(format!("{}/set/set1/add", base_url))
        .json(&json!({"members": ["a", "b", "c"]}))
        .send()
        .await
        .unwrap();

    // Create set2
    client
        .post(format!("{}/set/set2/add", base_url))
        .json(&json!({"members": ["b", "c", "d"]}))
        .send()
        .await
        .unwrap();

    // Compute intersection
    let resp = client
        .post(format!("{}/set/inter", base_url))
        .json(&json!({"keys": ["set1", "set2"]}))
        .send()
        .await
        .unwrap();

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["members"].as_array().unwrap().len(), 2); // b, c
}

#[tokio::test]
async fn test_set_union() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Create set1
    client
        .post(format!("{}/set/set3/add", base_url))
        .json(&json!({"members": ["a", "b"]}))
        .send()
        .await
        .unwrap();

    // Create set2
    client
        .post(format!("{}/set/set4/add", base_url))
        .json(&json!({"members": ["b", "c"]}))
        .send()
        .await
        .unwrap();

    // Compute union
    let resp = client
        .post(format!("{}/set/union", base_url))
        .json(&json!({"keys": ["set3", "set4"]}))
        .send()
        .await
        .unwrap();

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["members"].as_array().unwrap().len(), 3); // a, b, c
}

#[tokio::test]
async fn test_set_difference() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Create set1
    client
        .post(format!("{}/set/set5/add", base_url))
        .json(&json!({"members": ["a", "b", "c", "d"]}))
        .send()
        .await
        .unwrap();

    // Create set2
    client
        .post(format!("{}/set/set6/add", base_url))
        .json(&json!({"members": ["b", "d"]}))
        .send()
        .await
        .unwrap();

    // Compute difference (set1 - set2)
    let resp = client
        .post(format!("{}/set/diff", base_url))
        .json(&json!({"keys": ["set5", "set6"]}))
        .send()
        .await
        .unwrap();

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["members"].as_array().unwrap().len(), 2); // a, c
}

#[tokio::test]
async fn test_set_pop() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Add members
    client
        .post(format!("{}/set/test_set5/add", base_url))
        .json(&json!({"members": [1, 2, 3, 4, 5]}))
        .send()
        .await
        .unwrap();

    // Pop members with count as query parameter
    let resp = client
        .post(format!("{}/set/test_set5/pop?count=2", base_url))
        .send()
        .await
        .unwrap();

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["members"].as_array().unwrap().len(), 2);

    // Check remaining cardinality
    let resp = client
        .get(format!("{}/set/test_set5/card", base_url))
        .send()
        .await
        .unwrap();

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["count"], 3);
}

#[tokio::test]
async fn test_set_move() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Create source set
    client
        .post(format!("{}/set/source_set/add", base_url))
        .json(&json!({"members": ["item1", "item2"]}))
        .send()
        .await
        .unwrap();

    // Create destination set
    client
        .post(format!("{}/set/dest_set/add", base_url))
        .json(&json!({"members": ["item3"]}))
        .send()
        .await
        .unwrap();

    // Move member
    let resp = client
        .post(format!("{}/set/source_set/move/dest_set", base_url))
        .json(&json!({"member": "item1"}))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);

    // Verify source
    let resp = client
        .get(format!("{}/set/source_set/card", base_url))
        .send()
        .await
        .unwrap();
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["count"], 1);

    // Verify destination
    let resp = client
        .get(format!("{}/set/dest_set/card", base_url))
        .send()
        .await
        .unwrap();
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["count"], 2);
}

#[tokio::test]
async fn test_set_empty_operations() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Get members of non-existent set
    let resp = client
        .get(format!("{}/set/nonexistent/members", base_url))
        .send()
        .await
        .unwrap();

    let body: serde_json::Value = resp.json().await.unwrap();
    let members = body.get("members").and_then(|m| m.as_array());
    assert_eq!(members.map(|a| a.len()).unwrap_or(0), 0);

    // Get cardinality of non-existent set - should return 0 for count field
    let resp = client
        .get(format!("{}/set/nonexistent/card", base_url))
        .send()
        .await
        .unwrap();

    let body: serde_json::Value = resp.json().await.unwrap();
    let count = body.get("count").and_then(|v| v.as_u64()).unwrap_or(0);
    assert_eq!(count, 0);
}

#[tokio::test]
async fn test_set_randmember() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Add members
    client
        .post(format!("{}/set/rand_set/add", base_url))
        .json(&json!({"members": [1, 2, 3, 4, 5]}))
        .send()
        .await
        .unwrap();

    // Get random members
    let resp = client
        .get(format!("{}/set/rand_set/randmember?count=3", base_url))
        .send()
        .await
        .unwrap();

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["members"].as_array().unwrap().len(), 3);

    // Verify original set is unchanged
    let resp = client
        .get(format!("{}/set/rand_set/card", base_url))
        .send()
        .await
        .unwrap();

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["count"], 5);
}

#[tokio::test]
async fn test_set_stats() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Perform various operations
    client
        .post(format!("{}/set/stats_test/add", base_url))
        .json(&json!({"members": [1, 2, 3]}))
        .send()
        .await
        .unwrap();

    client
        .post(format!("{}/set/stats_test/rem", base_url))
        .json(&json!({"members": [1]}))
        .send()
        .await
        .unwrap();

    // Get stats
    let resp = client
        .get(format!("{}/set/stats", base_url))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body["total_sets"].as_u64().unwrap() >= 1);
    assert!(body["operations"]["sadd_count"].as_u64().unwrap() >= 1);
    assert!(body["operations"]["srem_count"].as_u64().unwrap() >= 1);
}

#[tokio::test]
async fn test_set_duplicate_members() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Add same members multiple times
    client
        .post(format!("{}/set/dup_test/add", base_url))
        .json(&json!({"members": ["a", "b", "c"]}))
        .send()
        .await
        .unwrap();

    let resp = client
        .post(format!("{}/set/dup_test/add", base_url))
        .json(&json!({"members": ["a", "b", "d"]}))
        .send()
        .await
        .unwrap();

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["added"], 1); // Only 'd' was added

    // Verify cardinality
    let resp = client
        .get(format!("{}/set/dup_test/card", base_url))
        .send()
        .await
        .unwrap();

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["count"], 4); // a, b, c, d
}

#[tokio::test]
async fn test_set_large_set() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Add 100 members
    let members: Vec<i32> = (0..100).collect();
    client
        .post(format!("{}/set/large_set/add", base_url))
        .json(&json!({"members": members}))
        .send()
        .await
        .unwrap();

    // Verify cardinality
    let resp = client
        .get(format!("{}/set/large_set/card", base_url))
        .send()
        .await
        .unwrap();

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["count"], 100);

    // Pop 10 members with query parameter
    let resp = client
        .post(format!("{}/set/large_set/pop?count=10", base_url))
        .send()
        .await
        .unwrap();

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["members"].as_array().unwrap().len(), 10);

    // Verify remaining
    let resp = client
        .get(format!("{}/set/large_set/card", base_url))
        .send()
        .await
        .unwrap();

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["count"], 90);
}
