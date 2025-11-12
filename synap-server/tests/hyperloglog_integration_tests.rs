use reqwest::Client;
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use synap_server::auth::{ApiKeyManager, UserManager};
use synap_server::core::{HashStore, ListStore, SetStore, SortedSetStore};
use synap_server::{AppState, KVStore, ServerConfig, create_router};
use tokio::net::TcpListener;

mod app_state_helper;

use app_state_helper::create_test_app_state_with_stores;

async fn spawn_test_server() -> String {
    let server_config = ServerConfig::default();
    let kv_store = Arc::new(KVStore::new(server_config.to_kv_config()));
    let hash_store = Arc::new(HashStore::new());
    let list_store = Arc::new(ListStore::new());
    let set_store = Arc::new(SetStore::new());
    let sorted_set_store = Arc::new(SortedSetStore::new());

    let app_state: AppState = create_test_app_state_with_stores(
        kv_store,
        hash_store,
        list_store,
        set_store,
        sorted_set_store,
    );

    let user_manager = Arc::new(UserManager::new());
    let api_key_manager = Arc::new(ApiKeyManager::new());
    let app = create_router(
        app_state,
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

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    tokio::time::sleep(Duration::from_millis(100)).await;

    format!("http://{}:{}", addr.ip(), addr.port())
}

// ==================== REST API Integration Tests ====================

#[tokio::test]
async fn test_hyperloglog_pfadd_pfcount_rest() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    let add_resp = client
        .post(format!("{}/hyperloglog/unique-users/pfadd", base_url))
        .json(&json!({
            "elements": ["user:1", "user:2", "user:3"],
            "ttl_secs": 60
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(add_resp.status(), 200);
    let add_body: serde_json::Value = add_resp.json().await.unwrap();
    assert!(add_body["added"].as_u64().unwrap_or(0) >= 1);

    let count_resp = client
        .get(format!("{}/hyperloglog/unique-users/pfcount", base_url))
        .send()
        .await
        .unwrap();

    assert_eq!(count_resp.status(), 200);
    let count_body: serde_json::Value = count_resp.json().await.unwrap();
    assert!(count_body["count"].as_u64().unwrap_or(0) >= 3);

    let stats_resp = client
        .get(format!("{}/hyperloglog/stats", base_url))
        .send()
        .await
        .unwrap();

    assert_eq!(stats_resp.status(), 200);
    let stats_body: serde_json::Value = stats_resp.json().await.unwrap();
    assert!(stats_body["total_hlls"].as_u64().unwrap_or(0) >= 1);
    assert!(stats_body["pfadd_count"].as_u64().unwrap_or(0) >= 1);
}

#[tokio::test]
async fn test_hyperloglog_pfmerge_rest() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Populate source HyperLogLogs
    client
        .post(format!("{}/hyperloglog/source-a/pfadd", base_url))
        .json(&json!({ "elements": ["user:a", "user:b"] }))
        .send()
        .await
        .unwrap();

    client
        .post(format!("{}/hyperloglog/source-b/pfadd", base_url))
        .json(&json!({ "elements": ["user:c", "user:d"] }))
        .send()
        .await
        .unwrap();

    let merge_resp = client
        .post(format!("{}/hyperloglog/merged/pfmerge", base_url))
        .json(&json!({
            "sources": ["source-a", "source-b"]
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(merge_resp.status(), 200);
    let merge_body: serde_json::Value = merge_resp.json().await.unwrap();
    assert!(merge_body["count"].as_u64().unwrap_or(0) >= 3);

    let count_resp = client
        .get(format!("{}/hyperloglog/merged/pfcount", base_url))
        .send()
        .await
        .unwrap();

    assert_eq!(count_resp.status(), 200);
    let count_body: serde_json::Value = count_resp.json().await.unwrap();
    assert!(count_body["count"].as_u64().unwrap_or(0) >= 3);
}

// ==================== StreamableHTTP Integration Tests ====================

#[tokio::test]
async fn test_hyperloglog_streamable_pfadd_and_pfcount() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    let elements: Vec<Vec<u8>> = vec![
        b"visitor:1".to_vec(),
        b"visitor:2".to_vec(),
        b"visitor:3".to_vec(),
    ];

    let add_resp = client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "hyperloglog.pfadd",
            "request_id": "hll-stream-1",
            "payload": {
                "key": "stream:hll",
                "elements": elements
            }
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(add_resp.status(), 200);
    let add_body: serde_json::Value = add_resp.json().await.unwrap();
    assert_eq!(add_body["success"], true);
    assert!(add_body["payload"]["added"].as_u64().unwrap_or(0) >= 1);

    let count_resp = client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "hyperloglog.pfcount",
            "request_id": "hll-stream-2",
            "payload": {
                "key": "stream:hll"
            }
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(count_resp.status(), 200);
    let count_body: serde_json::Value = count_resp.json().await.unwrap();
    assert_eq!(count_body["success"], true);
    assert!(count_body["payload"]["count"].as_u64().unwrap_or(0) >= 3);
}

#[tokio::test]
async fn test_hyperloglog_streamable_pfmerge() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    let source_a: Vec<Vec<u8>> = vec![b"alpha".to_vec(), b"beta".to_vec()];
    let source_b: Vec<Vec<u8>> = vec![b"gamma".to_vec(), b"delta".to_vec()];

    // Populate source structures
    client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "hyperloglog.pfadd",
            "request_id": "hll-merge-1",
            "payload": {
                "key": "hll:source:a",
                "elements": source_a
            }
        }))
        .send()
        .await
        .unwrap();

    client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "hyperloglog.pfadd",
            "request_id": "hll-merge-2",
            "payload": {
                "key": "hll:source:b",
                "elements": source_b
            }
        }))
        .send()
        .await
        .unwrap();

    let merge_resp = client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "hyperloglog.pfmerge",
            "request_id": "hll-merge-3",
            "payload": {
                "destination": "hll:dest",
                "sources": ["hll:source:a", "hll:source:b"]
            }
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(merge_resp.status(), 200);
    let merge_body: serde_json::Value = merge_resp.json().await.unwrap();
    assert_eq!(merge_body["success"], true);
    assert!(merge_body["payload"]["count"].as_u64().unwrap_or(0) >= 3);

    let stats_resp = client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "hyperloglog.stats",
            "request_id": "hll-merge-4",
            "payload": {}
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(stats_resp.status(), 200);
    let stats_body: serde_json::Value = stats_resp.json().await.unwrap();
    assert_eq!(stats_body["success"], true);
    assert!(stats_body["payload"]["pfmerge_count"].as_u64().unwrap_or(0) >= 1);
}
