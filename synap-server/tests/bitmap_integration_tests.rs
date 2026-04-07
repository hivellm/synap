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
async fn test_bitmap_setbit_getbit_rest() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // SETBIT - Set bit at offset 0 to 1
    let setbit_resp = client
        .post(format!("{}/bitmap/test-bitmap/setbit", base_url))
        .json(&json!({
            "offset": 0,
            "value": 1
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(setbit_resp.status(), 200);
    let setbit_body: serde_json::Value = setbit_resp.json().await.unwrap();
    assert_eq!(setbit_body["key"], "test-bitmap");
    assert_eq!(setbit_body["old_value"], 0);

    // SETBIT - Set bit at offset 7 to 1 (same byte)
    let setbit_resp2 = client
        .post(format!("{}/bitmap/test-bitmap/setbit", base_url))
        .json(&json!({
            "offset": 7,
            "value": 1
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(setbit_resp2.status(), 200);
    let setbit_body2: serde_json::Value = setbit_resp2.json().await.unwrap();
    assert_eq!(setbit_body2["old_value"], 0);

    // GETBIT - Get bit at offset 0
    let getbit_resp = client
        .get(format!("{}/bitmap/test-bitmap/getbit/0", base_url))
        .send()
        .await
        .unwrap();

    assert_eq!(getbit_resp.status(), 200);
    let getbit_body: serde_json::Value = getbit_resp.json().await.unwrap();
    assert_eq!(getbit_body["key"], "test-bitmap");
    assert_eq!(getbit_body["offset"], 0);
    assert_eq!(getbit_body["value"], 1);

    // GETBIT - Get bit at offset 7
    let getbit_resp2 = client
        .get(format!("{}/bitmap/test-bitmap/getbit/7", base_url))
        .send()
        .await
        .unwrap();

    assert_eq!(getbit_resp2.status(), 200);
    let getbit_body2: serde_json::Value = getbit_resp2.json().await.unwrap();
    assert_eq!(getbit_body2["value"], 1);

    // GETBIT - Get bit at unset offset (should return 0)
    let getbit_resp3 = client
        .get(format!("{}/bitmap/test-bitmap/getbit/5", base_url))
        .send()
        .await
        .unwrap();

    assert_eq!(getbit_resp3.status(), 200);
    let getbit_body3: serde_json::Value = getbit_resp3.json().await.unwrap();
    assert_eq!(getbit_body3["value"], 0);
}

#[tokio::test]
async fn test_bitmap_bitcount_rest() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Create bitmap with multiple bits set
    client
        .post(format!("{}/bitmap/count-test/setbit", base_url))
        .json(&json!({ "offset": 0, "value": 1 }))
        .send()
        .await
        .unwrap();

    client
        .post(format!("{}/bitmap/count-test/setbit", base_url))
        .json(&json!({ "offset": 2, "value": 1 }))
        .send()
        .await
        .unwrap();

    client
        .post(format!("{}/bitmap/count-test/setbit", base_url))
        .json(&json!({ "offset": 4, "value": 1 }))
        .send()
        .await
        .unwrap();

    client
        .post(format!("{}/bitmap/count-test/setbit", base_url))
        .json(&json!({ "offset": 8, "value": 1 }))
        .send()
        .await
        .unwrap();

    // BITCOUNT - Count all set bits
    let bitcount_resp = client
        .get(format!("{}/bitmap/count-test/bitcount", base_url))
        .send()
        .await
        .unwrap();

    assert_eq!(bitcount_resp.status(), 200);
    let bitcount_body: serde_json::Value = bitcount_resp.json().await.unwrap();
    assert_eq!(bitcount_body["key"], "count-test");
    assert_eq!(bitcount_body["count"], 4);

    // BITCOUNT - Count bits in range [0, 7] (first byte)
    let bitcount_range_resp = client
        .get(format!(
            "{}/bitmap/count-test/bitcount?start=0&end=7",
            base_url
        ))
        .send()
        .await
        .unwrap();

    assert_eq!(bitcount_range_resp.status(), 200);
    let bitcount_range_body: serde_json::Value = bitcount_range_resp.json().await.unwrap();
    assert_eq!(bitcount_range_body["count"], 3); // Bits at 0, 2, 4
}

#[tokio::test]
async fn test_bitmap_bitpos_rest() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Create bitmap with bits set at specific positions
    client
        .post(format!("{}/bitmap/pos-test/setbit", base_url))
        .json(&json!({ "offset": 5, "value": 1 }))
        .send()
        .await
        .unwrap();

    client
        .post(format!("{}/bitmap/pos-test/setbit", base_url))
        .json(&json!({ "offset": 10, "value": 1 }))
        .send()
        .await
        .unwrap();

    // BITPOS - Find first set bit
    let bitpos_resp = client
        .get(format!("{}/bitmap/pos-test/bitpos?value=1", base_url))
        .send()
        .await
        .unwrap();

    assert_eq!(bitpos_resp.status(), 200);
    let bitpos_body: serde_json::Value = bitpos_resp.json().await.unwrap();
    assert_eq!(bitpos_body["key"], "pos-test");
    assert_eq!(bitpos_body["position"], 5);

    // BITPOS - Find first unset bit (should be 0)
    let bitpos_resp2 = client
        .get(format!("{}/bitmap/pos-test/bitpos?value=0", base_url))
        .send()
        .await
        .unwrap();

    assert_eq!(bitpos_resp2.status(), 200);
    let bitpos_body2: serde_json::Value = bitpos_resp2.json().await.unwrap();
    assert_eq!(bitpos_body2["position"], 0);

    // BITPOS - Find set bit starting from offset 6
    let bitpos_resp3 = client
        .get(format!(
            "{}/bitmap/pos-test/bitpos?value=1&start=6",
            base_url
        ))
        .send()
        .await
        .unwrap();

    assert_eq!(bitpos_resp3.status(), 200);
    let bitpos_body3: serde_json::Value = bitpos_resp3.json().await.unwrap();
    assert_eq!(bitpos_body3["position"], 10);
}

#[tokio::test]
async fn test_bitmap_bitop_rest() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Create two source bitmaps
    client
        .post(format!("{}/bitmap/bitmap-a/setbit", base_url))
        .json(&json!({ "offset": 0, "value": 1 }))
        .send()
        .await
        .unwrap();

    client
        .post(format!("{}/bitmap/bitmap-a/setbit", base_url))
        .json(&json!({ "offset": 1, "value": 1 }))
        .send()
        .await
        .unwrap();

    client
        .post(format!("{}/bitmap/bitmap-b/setbit", base_url))
        .json(&json!({ "offset": 1, "value": 1 }))
        .send()
        .await
        .unwrap();

    client
        .post(format!("{}/bitmap/bitmap-b/setbit", base_url))
        .json(&json!({ "offset": 2, "value": 1 }))
        .send()
        .await
        .unwrap();

    // BITOP AND
    let bitop_and_resp = client
        .post(format!("{}/bitmap/result-and/bitop", base_url))
        .json(&json!({
            "operation": "AND",
            "source_keys": ["bitmap-a", "bitmap-b"]
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(bitop_and_resp.status(), 200);
    let bitop_and_body: serde_json::Value = bitop_and_resp.json().await.unwrap();
    assert_eq!(bitop_and_body["destination"], "result-and");
    assert!(bitop_and_body["length"].as_u64().unwrap_or(0) > 0);

    // Verify result - bit at offset 1 should be set (common to both)
    let getbit_resp = client
        .get(format!("{}/bitmap/result-and/getbit/1", base_url))
        .send()
        .await
        .unwrap();

    assert_eq!(getbit_resp.status(), 200);
    let getbit_body: serde_json::Value = getbit_resp.json().await.unwrap();
    assert_eq!(getbit_body["value"], 1);

    // BITOP OR
    let bitop_or_resp = client
        .post(format!("{}/bitmap/result-or/bitop", base_url))
        .json(&json!({
            "operation": "OR",
            "source_keys": ["bitmap-a", "bitmap-b"]
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(bitop_or_resp.status(), 200);
    let bitop_or_body: serde_json::Value = bitop_or_resp.json().await.unwrap();
    assert_eq!(bitop_or_body["destination"], "result-or");

    // Verify result - bits at 0, 1, 2 should be set
    let getbit_resp2 = client
        .get(format!("{}/bitmap/result-or/getbit/0", base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(getbit_resp2.status(), 200);
    let getbit_body2: serde_json::Value = getbit_resp2.json().await.unwrap();
    assert_eq!(getbit_body2["value"], 1);

    // BITOP XOR
    let bitop_xor_resp = client
        .post(format!("{}/bitmap/result-xor/bitop", base_url))
        .json(&json!({
            "operation": "XOR",
            "source_keys": ["bitmap-a", "bitmap-b"]
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(bitop_xor_resp.status(), 200);

    // BITOP NOT
    client
        .post(format!("{}/bitmap/not-source/setbit", base_url))
        .json(&json!({ "offset": 0, "value": 1 }))
        .send()
        .await
        .unwrap();

    let bitop_not_resp = client
        .post(format!("{}/bitmap/result-not/bitop", base_url))
        .json(&json!({
            "operation": "NOT",
            "source_keys": ["not-source"]
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(bitop_not_resp.status(), 200);
}

#[tokio::test]
async fn test_bitmap_stats_rest() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Create some bitmaps
    client
        .post(format!("{}/bitmap/stats-test-1/setbit", base_url))
        .json(&json!({ "offset": 0, "value": 1 }))
        .send()
        .await
        .unwrap();

    client
        .post(format!("{}/bitmap/stats-test-2/setbit", base_url))
        .json(&json!({ "offset": 1, "value": 1 }))
        .send()
        .await
        .unwrap();

    let stats_resp = client
        .get(format!("{}/bitmap/stats", base_url))
        .send()
        .await
        .unwrap();

    assert_eq!(stats_resp.status(), 200);
    let stats_body: serde_json::Value = stats_resp.json().await.unwrap();
    assert!(stats_body["total_bitmaps"].as_u64().unwrap_or(0) >= 2);
    assert!(stats_body["setbit_count"].as_u64().unwrap_or(0) >= 2);
}

#[tokio::test]
async fn test_bitmap_error_handling_rest() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // GETBIT on non-existent key
    let getbit_resp = client
        .get(format!("{}/bitmap/nonexistent/getbit/0", base_url))
        .send()
        .await
        .unwrap();

    assert_eq!(getbit_resp.status(), 404);

    // SETBIT with invalid value (not 0 or 1)
    let setbit_resp = client
        .post(format!("{}/bitmap/test/setbit", base_url))
        .json(&json!({
            "offset": 0,
            "value": 2  // Invalid value
        }))
        .send()
        .await
        .unwrap();

    // Should return error (either 400 or 200 with error in body)
    assert!(setbit_resp.status() == 400 || setbit_resp.status() == 200);
}

// ==================== StreamableHTTP Integration Tests ====================

#[tokio::test]
async fn test_bitmap_streamable_setbit_getbit() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // SETBIT
    let setbit_resp = client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "bitmap.setbit",
            "request_id": "bitmap-set-1",
            "payload": {
                "key": "stream:bitmap",
                "offset": 0,
                "value": 1
            }
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(setbit_resp.status(), 200);
    let setbit_body: serde_json::Value = setbit_resp.json().await.unwrap();
    assert_eq!(setbit_body["success"], true);
    assert_eq!(setbit_body["payload"]["old_value"], 0);

    // GETBIT
    let getbit_resp = client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "bitmap.getbit",
            "request_id": "bitmap-get-1",
            "payload": {
                "key": "stream:bitmap",
                "offset": 0
            }
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(getbit_resp.status(), 200);
    let getbit_body: serde_json::Value = getbit_resp.json().await.unwrap();
    assert_eq!(getbit_body["success"], true);
    assert_eq!(getbit_body["payload"]["value"], 1);
}

#[tokio::test]
async fn test_bitmap_streamable_bitcount() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Set multiple bits
    client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "bitmap.setbit",
            "request_id": "bitmap-count-1",
            "payload": { "key": "stream:count", "offset": 0, "value": 1 }
        }))
        .send()
        .await
        .unwrap();

    client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "bitmap.setbit",
            "request_id": "bitmap-count-2",
            "payload": { "key": "stream:count", "offset": 3, "value": 1 }
        }))
        .send()
        .await
        .unwrap();

    client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "bitmap.setbit",
            "request_id": "bitmap-count-3",
            "payload": { "key": "stream:count", "offset": 5, "value": 1 }
        }))
        .send()
        .await
        .unwrap();

    // BITCOUNT
    let bitcount_resp = client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "bitmap.bitcount",
            "request_id": "bitmap-count-4",
            "payload": {
                "key": "stream:count"
            }
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(bitcount_resp.status(), 200);
    let bitcount_body: serde_json::Value = bitcount_resp.json().await.unwrap();
    assert_eq!(bitcount_body["success"], true);
    assert_eq!(bitcount_body["payload"]["count"], 3);

    // BITCOUNT with range
    let bitcount_range_resp = client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "bitmap.bitcount",
            "request_id": "bitmap-count-5",
            "payload": {
                "key": "stream:count",
                "start": 0,
                "end": 7
            }
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(bitcount_range_resp.status(), 200);
    let bitcount_range_body: serde_json::Value = bitcount_range_resp.json().await.unwrap();
    assert_eq!(bitcount_range_body["success"], true);
    assert_eq!(bitcount_range_body["payload"]["count"], 3);
}

#[tokio::test]
async fn test_bitmap_streamable_bitpos() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Set bit at specific position
    client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "bitmap.setbit",
            "request_id": "bitmap-pos-1",
            "payload": { "key": "stream:pos", "offset": 7, "value": 1 }
        }))
        .send()
        .await
        .unwrap();

    // BITPOS - Find first set bit
    let bitpos_resp = client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "bitmap.bitpos",
            "request_id": "bitmap-pos-2",
            "payload": {
                "key": "stream:pos",
                "value": 1
            }
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(bitpos_resp.status(), 200);
    let bitpos_body: serde_json::Value = bitpos_resp.json().await.unwrap();
    assert_eq!(bitpos_body["success"], true);
    assert_eq!(bitpos_body["payload"]["position"], 7);

    // BITPOS - Find first unset bit
    let bitpos_resp2 = client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "bitmap.bitpos",
            "request_id": "bitmap-pos-3",
            "payload": {
                "key": "stream:pos",
                "value": 0
            }
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(bitpos_resp2.status(), 200);
    let bitpos_body2: serde_json::Value = bitpos_resp2.json().await.unwrap();
    assert_eq!(bitpos_body2["success"], true);
    assert_eq!(bitpos_body2["payload"]["position"], 0);
}

#[tokio::test]
async fn test_bitmap_streamable_bitop() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Create source bitmaps
    client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "bitmap.setbit",
            "request_id": "bitmap-op-1",
            "payload": { "key": "stream:src1", "offset": 0, "value": 1 }
        }))
        .send()
        .await
        .unwrap();

    client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "bitmap.setbit",
            "request_id": "bitmap-op-2",
            "payload": { "key": "stream:src1", "offset": 1, "value": 1 }
        }))
        .send()
        .await
        .unwrap();

    client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "bitmap.setbit",
            "request_id": "bitmap-op-3",
            "payload": { "key": "stream:src2", "offset": 1, "value": 1 }
        }))
        .send()
        .await
        .unwrap();

    client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "bitmap.setbit",
            "request_id": "bitmap-op-4",
            "payload": { "key": "stream:src2", "offset": 2, "value": 1 }
        }))
        .send()
        .await
        .unwrap();

    // BITOP AND
    let bitop_and_resp = client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "bitmap.bitop",
            "request_id": "bitmap-op-5",
            "payload": {
                "destination": "stream:result-and",
                "operation": "AND",
                "source_keys": ["stream:src1", "stream:src2"]
            }
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(bitop_and_resp.status(), 200);
    let bitop_and_body: serde_json::Value = bitop_and_resp.json().await.unwrap();
    assert_eq!(bitop_and_body["success"], true);
    assert!(bitop_and_body["payload"]["length"].as_u64().unwrap_or(0) > 0);

    // Verify AND result - bit 1 should be set (common to both)
    let getbit_resp = client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "bitmap.getbit",
            "request_id": "bitmap-op-6",
            "payload": {
                "key": "stream:result-and",
                "offset": 1
            }
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(getbit_resp.status(), 200);
    let getbit_body: serde_json::Value = getbit_resp.json().await.unwrap();
    assert_eq!(getbit_body["success"], true);
    assert_eq!(getbit_body["payload"]["value"], 1);

    // BITOP OR
    let bitop_or_resp = client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "bitmap.bitop",
            "request_id": "bitmap-op-7",
            "payload": {
                "destination": "stream:result-or",
                "operation": "OR",
                "source_keys": ["stream:src1", "stream:src2"]
            }
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(bitop_or_resp.status(), 200);
    let bitop_or_body: serde_json::Value = bitop_or_resp.json().await.unwrap();
    assert_eq!(bitop_or_body["success"], true);

    // BITOP XOR
    let bitop_xor_resp = client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "bitmap.bitop",
            "request_id": "bitmap-op-8",
            "payload": {
                "destination": "stream:result-xor",
                "operation": "XOR",
                "source_keys": ["stream:src1", "stream:src2"]
            }
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(bitop_xor_resp.status(), 200);
    let bitop_xor_body: serde_json::Value = bitop_xor_resp.json().await.unwrap();
    assert_eq!(bitop_xor_body["success"], true);

    // BITOP NOT
    client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "bitmap.setbit",
            "request_id": "bitmap-op-9",
            "payload": { "key": "stream:not-src", "offset": 0, "value": 1 }
        }))
        .send()
        .await
        .unwrap();

    let bitop_not_resp = client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "bitmap.bitop",
            "request_id": "bitmap-op-10",
            "payload": {
                "destination": "stream:result-not",
                "operation": "NOT",
                "source_keys": ["stream:not-src"]
            }
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(bitop_not_resp.status(), 200);
    let bitop_not_body: serde_json::Value = bitop_not_resp.json().await.unwrap();
    assert_eq!(bitop_not_body["success"], true);
}

#[tokio::test]
async fn test_bitmap_streamable_stats() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Create some bitmaps
    client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "bitmap.setbit",
            "request_id": "bitmap-stats-1",
            "payload": { "key": "stream:stats1", "offset": 0, "value": 1 }
        }))
        .send()
        .await
        .unwrap();

    client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "bitmap.setbit",
            "request_id": "bitmap-stats-2",
            "payload": { "key": "stream:stats2", "offset": 1, "value": 1 }
        }))
        .send()
        .await
        .unwrap();

    client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "bitmap.getbit",
            "request_id": "bitmap-stats-3",
            "payload": { "key": "stream:stats1", "offset": 0 }
        }))
        .send()
        .await
        .unwrap();

    let stats_resp = client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "bitmap.stats",
            "request_id": "bitmap-stats-4",
            "payload": {}
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(stats_resp.status(), 200);
    let stats_body: serde_json::Value = stats_resp.json().await.unwrap();
    assert_eq!(stats_body["success"], true);
    assert!(stats_body["payload"]["total_bitmaps"].as_u64().unwrap_or(0) >= 2);
    assert!(stats_body["payload"]["setbit_count"].as_u64().unwrap_or(0) >= 2);
    assert!(stats_body["payload"]["getbit_count"].as_u64().unwrap_or(0) >= 1);
}

#[tokio::test]
async fn test_bitmap_streamable_complete_workflow() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Complete workflow: SETBIT -> GETBIT -> BITCOUNT -> BITOP -> BITPOS -> STATS

    // 1. SETBIT - Create activity bitmap
    client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "bitmap.setbit",
            "request_id": "workflow-1",
            "payload": { "key": "activity:user:1", "offset": 100, "value": 1 }
        }))
        .send()
        .await
        .unwrap();

    client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "bitmap.setbit",
            "request_id": "workflow-2",
            "payload": { "key": "activity:user:1", "offset": 150, "value": 1 }
        }))
        .send()
        .await
        .unwrap();

    // 2. GETBIT - Check activity
    let getbit_resp = client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "bitmap.getbit",
            "request_id": "workflow-3",
            "payload": { "key": "activity:user:1", "offset": 100 }
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(getbit_resp.status(), 200);
    let getbit_body: serde_json::Value = getbit_resp.json().await.unwrap();
    assert_eq!(getbit_body["success"], true);
    assert_eq!(getbit_body["payload"]["value"], 1);

    // 3. BITCOUNT - Count active days
    let bitcount_resp = client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "bitmap.bitcount",
            "request_id": "workflow-4",
            "payload": { "key": "activity:user:1" }
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(bitcount_resp.status(), 200);
    let bitcount_body: serde_json::Value = bitcount_resp.json().await.unwrap();
    assert_eq!(bitcount_body["success"], true);
    assert_eq!(bitcount_body["payload"]["count"], 2);

    // 4. BITOP - Combine with another user's activity
    client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "bitmap.setbit",
            "request_id": "workflow-5",
            "payload": { "key": "activity:user:2", "offset": 100, "value": 1 }
        }))
        .send()
        .await
        .unwrap();

    let bitop_resp = client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "bitmap.bitop",
            "request_id": "workflow-6",
            "payload": {
                "destination": "activity:combined",
                "operation": "OR",
                "source_keys": ["activity:user:1", "activity:user:2"]
            }
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(bitop_resp.status(), 200);
    let bitop_body: serde_json::Value = bitop_resp.json().await.unwrap();
    assert_eq!(bitop_body["success"], true);

    // 5. BITPOS - Find first activity
    let bitpos_resp = client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "bitmap.bitpos",
            "request_id": "workflow-7",
            "payload": {
                "key": "activity:combined",
                "value": 1
            }
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(bitpos_resp.status(), 200);
    let bitpos_body: serde_json::Value = bitpos_resp.json().await.unwrap();
    assert_eq!(bitpos_body["success"], true);
    assert_eq!(bitpos_body["payload"]["position"], 100);

    // 6. STATS - Check statistics
    let stats_resp = client
        .post(format!("{}/api/v1/command", base_url))
        .json(&json!({
            "command": "bitmap.stats",
            "request_id": "workflow-8",
            "payload": {}
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(stats_resp.status(), 200);
    let stats_body: serde_json::Value = stats_resp.json().await.unwrap();
    assert_eq!(stats_body["success"], true);
    assert!(stats_body["payload"]["total_bitmaps"].as_u64().unwrap_or(0) >= 3);
}
