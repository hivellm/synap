// HTTP Status Code Tests
// Tests that REST API returns correct HTTP status codes for various scenarios

use reqwest::{Client, StatusCode};
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use synap_server::{AppState, KVConfig, KVStore, QueueConfig, QueueManager, create_router};
use tokio::net::TcpListener;

async fn spawn_test_server() -> String {
    let kv_store = Arc::new(KVStore::new(KVConfig::default()));
    let queue_manager = Arc::new(QueueManager::new(QueueConfig::default()));

    let state = AppState {
        kv_store,
        queue_manager: Some(queue_manager),
        stream_manager: None,
        pubsub_router: None,
        persistence: None,
        consumer_group_manager: None,
        partition_manager: None,
    };

    let app = create_router(
        state,
        synap_server::config::RateLimitConfig {
            enabled: false,
            requests_per_second: 100,
            burst_size: 10,
        },
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

// ==================== KV ENDPOINT STATUS CODES ====================

#[tokio::test]
async fn test_kv_set_returns_200_ok() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    let response = client
        .post(&format!("{}/kv/set", base_url))
        .json(&json!({
            "key": "test",
            "value": "hello"
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["success"], true);
    assert_eq!(body["key"], "test");
}

#[tokio::test]
async fn test_kv_get_returns_200_when_found() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Set a key first
    client
        .post(&format!("{}/kv/set", base_url))
        .json(&json!({"key": "existing", "value": "data"}))
        .send()
        .await
        .unwrap();

    // Get the key
    let response = client
        .get(&format!("{}/kv/get/existing", base_url))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["found"], true);
    assert_eq!(body["value"], "data");
}

#[tokio::test]
async fn test_kv_get_returns_200_when_not_found() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    let response = client
        .get(&format!("{}/kv/get/nonexistent", base_url))
        .send()
        .await
        .unwrap();

    // Returns 200 but with found: false
    assert_eq!(response.status(), StatusCode::OK);

    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["found"], false);
    assert_eq!(body["value"], serde_json::Value::Null);
}

#[tokio::test]
async fn test_kv_delete_returns_200() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Set a key
    client
        .post(&format!("{}/kv/set", base_url))
        .json(&json!({"key": "to_delete", "value": "temp"}))
        .send()
        .await
        .unwrap();

    // Delete it
    let response = client
        .delete(&format!("{}/kv/del/to_delete", base_url))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["deleted"], true);
}

#[tokio::test]
async fn test_kv_delete_nonexistent_returns_200_with_false() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    let response = client
        .delete(&format!("{}/kv/del/nonexistent", base_url))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["deleted"], false);
}

#[tokio::test]
async fn test_kv_stats_returns_200() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    let response = client
        .get(&format!("{}/kv/stats", base_url))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body: serde_json::Value = response.json().await.unwrap();
    assert!(body["total_keys"].is_number());
    assert!(body["hit_rate"].is_number());
}

#[tokio::test]
async fn test_invalid_json_returns_400() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    let response = client
        .post(&format!("{}/kv/set", base_url))
        .header("Content-Type", "application/json")
        .body("invalid json {")
        .send()
        .await
        .unwrap();

    // Axum returns 400 BAD_REQUEST for invalid JSON
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

// ==================== QUEUE ENDPOINT STATUS CODES ====================

#[tokio::test]
async fn test_queue_create_returns_200() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    let response = client
        .post(&format!("{}/queue/test_queue", base_url))
        .json(&json!({}))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["success"], true);
    assert_eq!(body["queue"], "test_queue");
}

#[tokio::test]
async fn test_queue_publish_returns_200() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Create queue first
    client
        .post(&format!("{}/queue/pub_queue", base_url))
        .json(&json!({}))
        .send()
        .await
        .unwrap();

    // Publish message
    let response = client
        .post(&format!("{}/queue/pub_queue/publish", base_url))
        .json(&json!({
            "payload": [72, 101, 108, 108, 111], // "Hello" in bytes
            "priority": 5
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body: serde_json::Value = response.json().await.unwrap();
    assert!(body["message_id"].is_string());
}

#[tokio::test]
async fn test_queue_publish_to_nonexistent_returns_404() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    let response = client
        .post(&format!("{}/queue/nonexistent_queue/publish", base_url))
        .json(&json!({
            "payload": [1, 2, 3]
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    let body: serde_json::Value = response.json().await.unwrap();
    assert!(body["error"].as_str().unwrap().contains("Queue not found"));
}

#[tokio::test]
async fn test_queue_consume_returns_200_with_message() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Create queue and publish
    client
        .post(&format!("{}/queue/consume_queue", base_url))
        .json(&json!({}))
        .send()
        .await
        .unwrap();
    client
        .post(&format!("{}/queue/consume_queue/publish", base_url))
        .json(&json!({"payload": [1, 2, 3]}))
        .send()
        .await
        .unwrap();

    // Consume
    let response = client
        .get(&format!(
            "{}/queue/consume_queue/consume/worker-1",
            base_url
        ))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body: serde_json::Value = response.json().await.unwrap();
    assert!(body["message_id"].is_string());
    assert!(body["payload"].is_array());
}

#[tokio::test]
async fn test_queue_consume_empty_returns_200_null() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Create empty queue
    client
        .post(&format!("{}/queue/empty_queue", base_url))
        .json(&json!({}))
        .send()
        .await
        .unwrap();

    // Consume from empty queue
    let response = client
        .get(&format!("{}/queue/empty_queue/consume/worker-1", base_url))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["message_id"], serde_json::Value::Null);
}

#[tokio::test]
async fn test_queue_consume_nonexistent_returns_404() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    let response = client
        .get(&format!("{}/queue/nonexistent/consume/worker-1", base_url))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_queue_ack_valid_returns_200() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Create queue, publish, consume
    client
        .post(&format!("{}/queue/ack_queue", base_url))
        .json(&json!({}))
        .send()
        .await
        .unwrap();
    client
        .post(&format!("{}/queue/ack_queue/publish", base_url))
        .json(&json!({"payload": [1, 2, 3]}))
        .send()
        .await
        .unwrap();

    let consume_resp = client
        .get(&format!("{}/queue/ack_queue/consume/worker-1", base_url))
        .send()
        .await
        .unwrap();
    let consume_body: serde_json::Value = consume_resp.json().await.unwrap();
    let message_id = consume_body["message_id"].as_str().unwrap();

    // ACK
    let response = client
        .post(&format!("{}/queue/ack_queue/ack", base_url))
        .json(&json!({"message_id": message_id}))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["success"], true);
}

#[tokio::test]
async fn test_queue_ack_invalid_message_returns_404() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    client
        .post(&format!("{}/queue/ack_test", base_url))
        .json(&json!({}))
        .send()
        .await
        .unwrap();

    let response = client
        .post(&format!("{}/queue/ack_test/ack", base_url))
        .json(&json!({"message_id": "invalid-message-id"}))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    let body: serde_json::Value = response.json().await.unwrap();
    assert!(
        body["error"]
            .as_str()
            .unwrap()
            .contains("Message not found")
    );
}

#[tokio::test]
async fn test_queue_nack_returns_200() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Create queue, publish, consume
    client
        .post(&format!("{}/queue/nack_queue", base_url))
        .json(&json!({}))
        .send()
        .await
        .unwrap();
    client
        .post(&format!("{}/queue/nack_queue/publish", base_url))
        .json(&json!({"payload": [1, 2, 3], "max_retries": 3}))
        .send()
        .await
        .unwrap();

    let consume_resp = client
        .get(&format!("{}/queue/nack_queue/consume/worker-1", base_url))
        .send()
        .await
        .unwrap();
    let consume_body: serde_json::Value = consume_resp.json().await.unwrap();
    let message_id = consume_body["message_id"].as_str().unwrap();

    // NACK with requeue
    let response = client
        .post(&format!("{}/queue/nack_queue/nack", base_url))
        .json(&json!({"message_id": message_id, "requeue": true}))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["success"], true);
}

#[tokio::test]
async fn test_queue_stats_returns_200() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    client
        .post(&format!("{}/queue/stats_queue", base_url))
        .json(&json!({}))
        .send()
        .await
        .unwrap();

    let response = client
        .get(&format!("{}/queue/stats_queue/stats", base_url))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body: serde_json::Value = response.json().await.unwrap();
    assert!(body["depth"].is_number());
    assert!(body["published"].is_number());
}

#[tokio::test]
async fn test_queue_stats_nonexistent_returns_404() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    let response = client
        .get(&format!("{}/queue/nonexistent_queue/stats", base_url))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_queue_list_returns_200() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Create some queues
    client
        .post(&format!("{}/queue/q1", base_url))
        .json(&json!({}))
        .send()
        .await
        .unwrap();
    client
        .post(&format!("{}/queue/q2", base_url))
        .json(&json!({}))
        .send()
        .await
        .unwrap();

    let response = client
        .get(&format!("{}/queue/list", base_url))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body: serde_json::Value = response.json().await.unwrap();
    assert!(body["queues"].is_array());
    let queues = body["queues"].as_array().unwrap();
    assert!(queues.len() >= 2);
}

#[tokio::test]
async fn test_queue_purge_returns_200() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Create queue and add messages
    client
        .post(&format!("{}/queue/purge_queue", base_url))
        .json(&json!({}))
        .send()
        .await
        .unwrap();
    for i in 0..5 {
        client
            .post(&format!("{}/queue/purge_queue/publish", base_url))
            .json(&json!({"payload": [i]}))
            .send()
            .await
            .unwrap();
    }

    // Purge
    let response = client
        .post(&format!("{}/queue/purge_queue/purge", base_url))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["success"], true);
    assert_eq!(body["purged"], 5);
}

#[tokio::test]
async fn test_queue_delete_returns_200() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Create queue
    client
        .post(&format!("{}/queue/del_queue", base_url))
        .json(&json!({}))
        .send()
        .await
        .unwrap();

    // Delete
    let response = client
        .delete(&format!("{}/queue/del_queue", base_url))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["success"], true);
    assert_eq!(body["deleted"], true);
}

#[tokio::test]
async fn test_queue_delete_nonexistent_returns_200_false() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    let response = client
        .delete(&format!("{}/queue/nonexistent_delete", base_url))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["deleted"], false);
}

// ==================== HEALTH CHECK ====================

#[tokio::test]
async fn test_health_check_returns_200() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    let response = client
        .get(&format!("{}/health", base_url))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["status"], "healthy");
    assert_eq!(body["service"], "synap");
}

// ==================== ERROR RESPONSE FORMAT TESTS ====================

#[tokio::test]
async fn test_error_response_has_correct_format() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Try to ACK a message that doesn't exist
    client
        .post(&format!("{}/queue/error_test", base_url))
        .json(&json!({}))
        .send()
        .await
        .unwrap();

    let response = client
        .post(&format!("{}/queue/error_test/ack", base_url))
        .json(&json!({"message_id": "invalid-id"}))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    let body: serde_json::Value = response.json().await.unwrap();
    assert!(body["error"].is_string());
    assert!(body["code"].is_number());
    assert_eq!(body["code"], 404);
}

#[tokio::test]
async fn test_queue_not_found_error_format() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    let response = client
        .post(&format!("{}/queue/missing_queue/publish", base_url))
        .json(&json!({"payload": [1, 2, 3]}))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    let body: serde_json::Value = response.json().await.unwrap();
    assert!(body["error"].as_str().unwrap().contains("Queue not found"));
    assert_eq!(body["code"], 404);
}

// ==================== STREAMABLE HTTP STATUS CODES ====================

#[tokio::test]
async fn test_streamable_unknown_command_returns_200_with_error() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    let response = client
        .post(&format!("{}/api/v1/command", base_url))
        .json(&json!({
            "request_id": "test-req-1",
            "command": "unknown.command",
            "payload": {}
        }))
        .send()
        .await
        .unwrap();

    // StreamableHTTP returns 200 but with success: false in envelope
    assert_eq!(response.status(), StatusCode::OK);

    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["success"], false);
    assert!(body["error"].is_string());
    assert!(body["error"].as_str().unwrap().contains("Unknown command"));
}

#[tokio::test]
async fn test_streamable_missing_params_returns_200_with_error() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    let response = client
        .post(&format!("{}/api/v1/command", base_url))
        .json(&json!({
            "request_id": "test-req-2",
            "command": "kv.set",
            "payload": {} // Missing required params
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["success"], false);
    assert!(body["error"].is_string());
}

#[tokio::test]
async fn test_streamable_successful_returns_200() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    let response = client
        .post(&format!("{}/api/v1/command", base_url))
        .json(&json!({
            "request_id": "test-req-3",
            "command": "kv.set",
            "payload": {
                "key": "test_key",
                "value": "test_value"
            }
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["success"], true);
    assert_eq!(body["request_id"], "test-req-3");
    // Payload can be null or object depending on command
    assert!(body.get("payload").is_some());
}

// ==================== CONCURRENT REQUEST STATUS CODES ====================

#[tokio::test]
async fn test_concurrent_requests_all_return_200() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    let mut handles = vec![];

    // Spawn 20 concurrent requests
    for i in 0..20 {
        let url = base_url.clone();
        let client = client.clone();

        let handle = tokio::spawn(async move {
            let response = client
                .post(&format!("{}/kv/set", url))
                .json(&json!({
                    "key": format!("concurrent-{}", i),
                    "value": format!("value-{}", i)
                }))
                .send()
                .await
                .unwrap();

            response.status()
        });

        handles.push(handle);
    }

    // All should return 200
    for handle in handles {
        let status = handle.await.unwrap();
        assert_eq!(status, StatusCode::OK);
    }
}

// ==================== CONTENT TYPE TESTS ====================

#[tokio::test]
async fn test_json_content_type_required() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Send plain text instead of JSON
    let response = client
        .post(&format!("{}/kv/set", base_url))
        .header("Content-Type", "text/plain")
        .body("not json")
        .send()
        .await
        .unwrap();

    // Should fail to parse
    assert!(response.status().is_client_error());
}

#[tokio::test]
async fn test_response_is_json() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    let response = client
        .get(&format!("{}/health", base_url))
        .send()
        .await
        .unwrap();

    let content_type = response.headers().get("content-type").unwrap();
    assert!(content_type.to_str().unwrap().contains("application/json"));
}

// ==================== ROUTE NOT FOUND ====================

#[tokio::test]
async fn test_nonexistent_route_returns_404() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    let response = client
        .get(&format!("{}/nonexistent/route", base_url))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_wrong_http_method_returns_405() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // health check expects GET, send POST
    let response = client
        .post(&format!("{}/health", base_url))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
}

// ==================== QUEUE FULL STATUS CODE ====================

#[tokio::test]
async fn test_queue_full_returns_507() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Create queue with max_depth = 5
    client
        .post(&format!("{}/queue/small_queue", base_url))
        .json(&json!({
            "max_depth": 5
        }))
        .send()
        .await
        .unwrap();

    // Fill the queue
    for i in 0..5 {
        client
            .post(&format!("{}/queue/small_queue/publish", base_url))
            .json(&json!({"payload": [i]}))
            .send()
            .await
            .unwrap();
    }

    // Try to publish one more (should fail)
    let response = client
        .post(&format!("{}/queue/small_queue/publish", base_url))
        .json(&json!({"payload": [99]}))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::INSUFFICIENT_STORAGE); // 507

    let body: serde_json::Value = response.json().await.unwrap();
    assert!(body["error"].as_str().unwrap().contains("Queue is full"));
}

// ==================== SUMMARY TEST ====================

#[tokio::test]
async fn test_all_status_codes_comprehensive() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // 200 OK
    let resp = client
        .get(&format!("{}/health", base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // 404 Not Found
    let resp = client
        .get(&format!("{}/nonexistent", base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);

    // 405 Method Not Allowed
    let resp = client
        .post(&format!("{}/health", base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::METHOD_NOT_ALLOWED);

    // 400 Bad Request (invalid JSON)
    let resp = client
        .post(&format!("{}/kv/set", base_url))
        .header("Content-Type", "application/json")
        .body("{invalid json}")
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    println!("✅ All HTTP status codes verified!");
}
