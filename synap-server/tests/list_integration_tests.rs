// List Integration Tests
// End-to-end tests for list operations via REST API and StreamableHTTP

use reqwest::Client;
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use synap_server::{AppState, KVStore, QueueConfig, QueueManager, ServerConfig, create_router};
use tokio::net::TcpListener;

/// Spawn a test server with list support
async fn spawn_test_server() -> String {
    let config = ServerConfig::default();
    let kv_store = Arc::new(KVStore::new(config.to_kv_config()));
    let hash_store = Arc::new(synap_server::core::HashStore::new());
    let list_store = Arc::new(synap_server::core::ListStore::new());
    let queue_manager = Arc::new(QueueManager::new(QueueConfig::default()));

    let app_state = AppState {
        kv_store,
        hash_store,
        list_store,
        set_store: Arc::new(synap_server::core::SetStore::new()),
        sorted_set_store: Arc::new(synap_server::core::SortedSetStore::new()),
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
async fn test_list_push_pop_rest() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // RPUSH elements
    let push_resp = client
        .post(format!("{}/list/mylist/rpush", base_url))
        .json(&json!({
            "values": ["one", "two", "three"]
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(push_resp.status(), 200);
    let push_body: serde_json::Value = push_resp.json().await.unwrap();
    assert_eq!(push_body["length"], 3);

    // LPOP one element
    let pop_resp = client
        .post(format!("{}/list/mylist/lpop", base_url))
        .json(&json!({
            "count": 1
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(pop_resp.status(), 200);
    let pop_body: serde_json::Value = pop_resp.json().await.unwrap();
    assert_eq!(pop_body["values"], json!(["one"]));

    // RPOP one element
    let rpop_resp = client
        .post(format!("{}/list/mylist/rpop", base_url))
        .json(&json!({
            "count": 1
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(rpop_resp.status(), 200);
    let rpop_body: serde_json::Value = rpop_resp.json().await.unwrap();
    assert_eq!(rpop_body["values"], json!(["three"]));
}

#[tokio::test]
async fn test_list_range_rest() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // RPUSH elements
    client
        .post(format!("{}/list/mylist/rpush", base_url))
        .json(&json!({
            "values": ["a", "b", "c", "d", "e"]
        }))
        .send()
        .await
        .unwrap();

    // LRANGE all elements
    let range_resp = client
        .get(format!("{}/list/mylist/range?start=0&stop=-1", base_url))
        .send()
        .await
        .unwrap();

    assert_eq!(range_resp.status(), 200);
    let range_body: serde_json::Value = range_resp.json().await.unwrap();
    assert_eq!(range_body["values"], json!(["a", "b", "c", "d", "e"]));

    // LRANGE partial
    let partial_resp = client
        .get(format!("{}/list/mylist/range?start=1&stop=3", base_url))
        .send()
        .await
        .unwrap();

    assert_eq!(partial_resp.status(), 200);
    let partial_body: serde_json::Value = partial_resp.json().await.unwrap();
    assert_eq!(partial_body["values"], json!(["b", "c", "d"]));
}

#[tokio::test]
async fn test_list_len_rest() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // RPUSH elements
    client
        .post(format!("{}/list/mylist/rpush", base_url))
        .json(&json!({
            "values": ["x", "y", "z"]
        }))
        .send()
        .await
        .unwrap();

    // LLEN
    let len_resp = client
        .get(format!("{}/list/mylist/len", base_url))
        .send()
        .await
        .unwrap();

    assert_eq!(len_resp.status(), 200);
    let len_body: serde_json::Value = len_resp.json().await.unwrap();
    assert_eq!(len_body["length"], 3);
}

#[tokio::test]
async fn test_list_index_rest() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    client
        .post(format!("{}/list/mylist/rpush", base_url))
        .json(&json!({
            "values": ["zero", "one", "two"]
        }))
        .send()
        .await
        .unwrap();

    // LINDEX positive
    let index_resp = client
        .get(format!("{}/list/mylist/index/1", base_url))
        .send()
        .await
        .unwrap();

    assert_eq!(index_resp.status(), 200);
    let index_body: serde_json::Value = index_resp.json().await.unwrap();
    assert_eq!(index_body["value"], "one");
}

#[tokio::test]
async fn test_list_set_rest() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    client
        .post(format!("{}/list/mylist/rpush", base_url))
        .json(&json!({
            "values": ["a", "b", "c"]
        }))
        .send()
        .await
        .unwrap();

    // LSET
    let set_resp = client
        .post(format!("{}/list/mylist/set", base_url))
        .json(&json!({
            "index": 1,
            "value": "B"
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(set_resp.status(), 200);

    // Verify changed
    let index_resp = client
        .get(format!("{}/list/mylist/index/1", base_url))
        .send()
        .await
        .unwrap();

    let index_body: serde_json::Value = index_resp.json().await.unwrap();
    assert_eq!(index_body["value"], "B");
}

#[tokio::test]
async fn test_list_trim_rest() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    client
        .post(format!("{}/list/mylist/rpush", base_url))
        .json(&json!({
            "values": ["0", "1", "2", "3", "4"]
        }))
        .send()
        .await
        .unwrap();

    // LTRIM
    let trim_resp = client
        .post(format!("{}/list/mylist/trim?start=1&stop=3", base_url))
        .send()
        .await
        .unwrap();

    assert_eq!(trim_resp.status(), 200);

    // Verify trimmed
    let range_resp = client
        .get(format!("{}/list/mylist/range?start=0&stop=-1", base_url))
        .send()
        .await
        .unwrap();

    let range_body: serde_json::Value = range_resp.json().await.unwrap();
    assert_eq!(range_body["values"], json!(["1", "2", "3"]));
}

#[tokio::test]
async fn test_list_rem_rest() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    client
        .post(format!("{}/list/mylist/rpush", base_url))
        .json(&json!({
            "values": ["a", "b", "a", "c", "a"]
        }))
        .send()
        .await
        .unwrap();

    // LREM - remove 2 occurrences of "a"
    let rem_resp = client
        .post(format!("{}/list/mylist/rem", base_url))
        .json(&json!({
            "count": 2,
            "value": "a"
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(rem_resp.status(), 200);
    let rem_body: serde_json::Value = rem_resp.json().await.unwrap();
    assert_eq!(rem_body["removed"], 2);
}

#[tokio::test]
async fn test_list_insert_rest() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    client
        .post(format!("{}/list/mylist/rpush", base_url))
        .json(&json!({
            "values": ["a", "c"]
        }))
        .send()
        .await
        .unwrap();

    // LINSERT before "c"
    let insert_resp = client
        .post(format!("{}/list/mylist/insert", base_url))
        .json(&json!({
            "before": true,
            "pivot": "c",
            "value": "b"
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(insert_resp.status(), 200);
    let insert_body: serde_json::Value = insert_resp.json().await.unwrap();
    assert_eq!(insert_body["length"], 3);

    // Verify order
    let range_resp = client
        .get(format!("{}/list/mylist/range?start=0&stop=-1", base_url))
        .send()
        .await
        .unwrap();

    let range_body: serde_json::Value = range_resp.json().await.unwrap();
    assert_eq!(range_body["values"], json!(["a", "b", "c"]));
}

#[tokio::test]
async fn test_list_rpoplpush_rest() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    client
        .post(format!("{}/list/source/rpush", base_url))
        .json(&json!({
            "values": ["a", "b", "c"]
        }))
        .send()
        .await
        .unwrap();

    // RPOPLPUSH
    let rpoplpush_resp = client
        .post(format!("{}/list/source/rpoplpush/dest", base_url))
        .send()
        .await
        .unwrap();

    assert_eq!(rpoplpush_resp.status(), 200);
    let rpoplpush_body: serde_json::Value = rpoplpush_resp.json().await.unwrap();
    assert_eq!(rpoplpush_body["value"], "c");

    // Verify source
    let source_resp = client
        .get(format!("{}/list/source/range?start=0&stop=-1", base_url))
        .send()
        .await
        .unwrap();

    let source_body: serde_json::Value = source_resp.json().await.unwrap();
    assert_eq!(source_body["values"], json!(["a", "b"]));

    // Verify destination
    let dest_resp = client
        .get(format!("{}/list/dest/range?start=0&stop=-1", base_url))
        .send()
        .await
        .unwrap();

    let dest_body: serde_json::Value = dest_resp.json().await.unwrap();
    assert_eq!(dest_body["values"], json!(["c"]));
}

#[tokio::test]
async fn test_list_stats_rest() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Create some lists
    client
        .post(format!("{}/list/list1/rpush", base_url))
        .json(&json!({
            "values": ["a", "b"]
        }))
        .send()
        .await
        .unwrap();

    client
        .post(format!("{}/list/list2/rpush", base_url))
        .json(&json!({
            "values": ["x", "y", "z"]
        }))
        .send()
        .await
        .unwrap();

    // GET stats
    let stats_resp = client
        .get(format!("{}/list/stats", base_url))
        .send()
        .await
        .unwrap();

    assert_eq!(stats_resp.status(), 200);
    let stats_body: serde_json::Value = stats_resp.json().await.unwrap();
    assert_eq!(stats_body["total_lists"], 2);
    assert_eq!(stats_body["total_elements"], 5);
    assert!(stats_body["operations"]["rpush_count"].as_u64().unwrap() >= 2);
}

#[tokio::test]
async fn test_list_lpushx_rest() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // LPUSHX on non-existent list should return 0
    let lpushx_resp = client
        .post(format!("{}/list/newlist/lpushx", base_url))
        .json(&json!({
            "values": ["x"]
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(lpushx_resp.status(), 200);
    let lpushx_body: serde_json::Value = lpushx_resp.json().await.unwrap();
    assert_eq!(lpushx_body["length"], 0);

    // Create the list first
    client
        .post(format!("{}/list/newlist/lpush", base_url))
        .json(&json!({
            "values": ["a"]
        }))
        .send()
        .await
        .unwrap();

    // Now LPUSHX should work
    let lpushx2_resp = client
        .post(format!("{}/list/newlist/lpushx", base_url))
        .json(&json!({
            "values": ["b"]
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(lpushx2_resp.status(), 200);
    let lpushx2_body: serde_json::Value = lpushx2_resp.json().await.unwrap();
    assert_eq!(lpushx2_body["length"], 2);
}

#[tokio::test]
async fn test_list_concurrent_access() {
    let base_url = spawn_test_server().await;
    let client = Arc::new(Client::new());

    // Spawn 10 concurrent push operations
    let mut handles = vec![];
    for i in 0..10 {
        let client = client.clone();
        let base_url = base_url.clone();
        let handle = tokio::spawn(async move {
            client
                .post(format!("{}/list/concurrent/rpush", base_url))
                .json(&json!({
                    "values": [format!("value-{}", i)]
                }))
                .send()
                .await
                .unwrap()
        });
        handles.push(handle);
    }

    // Wait for all to complete
    for handle in handles {
        handle.await.unwrap();
    }

    // Verify all 10 elements are present
    let len_resp = client
        .get(format!("{}/list/concurrent/len", base_url))
        .send()
        .await
        .unwrap();

    let len_body: serde_json::Value = len_resp.json().await.unwrap();
    assert_eq!(len_body["length"], 10);
}

#[tokio::test]
async fn test_list_large_elements() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Push 100 elements
    let values: Vec<String> = (0..100).map(|i| format!("value-{}", i)).collect();

    client
        .post(format!("{}/list/large/rpush", base_url))
        .json(&json!({
            "values": values
        }))
        .send()
        .await
        .unwrap();

    // LRANGE all
    let range_resp = client
        .get(format!("{}/list/large/range?start=0&stop=-1", base_url))
        .send()
        .await
        .unwrap();

    assert_eq!(range_resp.status(), 200);
    let range_body: serde_json::Value = range_resp.json().await.unwrap();
    assert_eq!(range_body["values"].as_array().unwrap().len(), 100);

    // LLEN
    let len_resp = client
        .get(format!("{}/list/large/len", base_url))
        .send()
        .await
        .unwrap();

    let len_body: serde_json::Value = len_resp.json().await.unwrap();
    assert_eq!(len_body["length"], 100);
}

#[tokio::test]
async fn test_list_edge_cases() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // LPOP on non-existent list
    let pop_resp = client
        .post(format!("{}/list/nonexistent/lpop", base_url))
        .json(&json!({
            "count": 1
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(pop_resp.status(), 404);

    // LLEN on non-existent list
    let len_resp = client
        .get(format!("{}/list/nonexistent/len", base_url))
        .send()
        .await
        .unwrap();

    assert_eq!(len_resp.status(), 404);

    // LINDEX out of range
    client
        .post(format!("{}/list/mylist/rpush", base_url))
        .json(&json!({
            "values": ["a"]
        }))
        .send()
        .await
        .unwrap();

    let index_resp = client
        .get(format!("{}/list/mylist/index/10", base_url))
        .send()
        .await
        .unwrap();

    assert_eq!(index_resp.status(), 400); // Bad Request for index out of range
}

#[tokio::test]
async fn test_list_empty_operations() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // RPUSH empty array
    let push_resp = client
        .post(format!("{}/list/mylist/rpush", base_url))
        .json(&json!({
            "values": []
        }))
        .send()
        .await
        .unwrap();

    // Should still succeed but with 0 length
    assert_eq!(push_resp.status(), 200);
}
