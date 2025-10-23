use reqwest::Client;
use serde_json::json;
use std::sync::Arc;
use synap_server::{AppState, KVStore, ServerConfig, StreamConfig, StreamManager, create_router};
use tokio::net::TcpListener;

/// Spawn a test server and return its base URL
async fn spawn_test_server() -> String {
    let config = ServerConfig::default();
    let kv_config = config.to_kv_config();

    let stream_mgr = Arc::new(StreamManager::new(StreamConfig::default()));
    stream_mgr.clone().start_compaction_task();

    let app_state = AppState {
        kv_store: Arc::new(KVStore::new(kv_config)),
        queue_manager: None,
        stream_manager: Some(stream_mgr),
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

    // Bind to random port
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    // Wait a moment for server to start
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    format!("http://{}", addr)
}

/// Helper function to send StreamableHTTP command
async fn send_command(
    client: &Client,
    base_url: &str,
    command: &str,
    payload: serde_json::Value,
) -> serde_json::Value {
    let request = json!({
        "command": command,
        "request_id": uuid::Uuid::new_v4().to_string(),
        "payload": payload
    });

    let res = client
        .post(format!("{}/api/v1/command", base_url))
        .json(&request)
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), 200);
    res.json().await.unwrap()
}

// ============================================================================
// Event Streams StreamableHTTP Tests
// ============================================================================

#[tokio::test]
async fn test_stream_create_command() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    let res = send_command(
        &client,
        &base_url,
        "stream.create",
        json!({
            "room": "test_room"
        }),
    )
    .await;

    assert_eq!(res["success"], true);
    assert_eq!(res["payload"]["room"], "test_room");
}

#[tokio::test]
async fn test_stream_publish_command() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Create room first
    send_command(
        &client,
        &base_url,
        "stream.create",
        json!({
            "room": "publish_room"
        }),
    )
    .await;

    // Publish event
    let res = send_command(
        &client,
        &base_url,
        "stream.publish",
        json!({
            "room": "publish_room",
            "event": "user.login",
            "data": {"user_id": 123, "timestamp": 1234567890}
        }),
    )
    .await;

    assert_eq!(res["success"], true);
    assert_eq!(res["payload"]["room"], "publish_room");
    assert!(res["payload"]["offset"].as_u64().is_some());
}

#[tokio::test]
async fn test_stream_consume_command() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Create room
    send_command(
        &client,
        &base_url,
        "stream.create",
        json!({
            "room": "consume_room"
        }),
    )
    .await;

    // Publish events
    for i in 1..=5 {
        send_command(
            &client,
            &base_url,
            "stream.publish",
            json!({
                "room": "consume_room",
                "event": "test.event",
                "data": {"value": i}
            }),
        )
        .await;
    }

    // Consume events
    let res = send_command(
        &client,
        &base_url,
        "stream.consume",
        json!({
            "room": "consume_room",
            "subscriber_id": "sub1",
            "from_offset": 0,
            "limit": 10
        }),
    )
    .await;

    assert_eq!(res["success"], true);
    let events = res["payload"]["events"].as_array().unwrap();
    assert_eq!(events.len(), 5);
    assert!(res["payload"]["next_offset"].as_u64().is_some());
}

#[tokio::test]
async fn test_stream_consume_with_offset() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Create room and publish
    send_command(
        &client,
        &base_url,
        "stream.create",
        json!({
            "room": "offset_room"
        }),
    )
    .await;

    // Publish 10 events
    for i in 1..=10 {
        send_command(
            &client,
            &base_url,
            "stream.publish",
            json!({
                "room": "offset_room",
                "event": "event",
                "data": {"num": i}
            }),
        )
        .await;
    }

    // Consume from offset 5
    let res = send_command(
        &client,
        &base_url,
        "stream.consume",
        json!({
            "room": "offset_room",
            "subscriber_id": "sub2",
            "from_offset": 5,
            "limit": 100
        }),
    )
    .await;

    assert_eq!(res["success"], true);
    let events = res["payload"]["events"].as_array().unwrap();
    assert!(events.len() >= 5, "Should get events from offset 5 onwards");
}

#[tokio::test]
async fn test_stream_consume_with_limit() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Create and publish
    send_command(
        &client,
        &base_url,
        "stream.create",
        json!({
            "room": "limit_room"
        }),
    )
    .await;

    for i in 1..=20 {
        send_command(
            &client,
            &base_url,
            "stream.publish",
            json!({
                "room": "limit_room",
                "event": "event",
                "data": {"i": i}
            }),
        )
        .await;
    }

    // Consume with limit
    let res = send_command(
        &client,
        &base_url,
        "stream.consume",
        json!({
            "room": "limit_room",
            "subscriber_id": "sub3",
            "from_offset": 0,
            "limit": 5
        }),
    )
    .await;

    let events = res["payload"]["events"].as_array().unwrap();
    assert_eq!(events.len(), 5, "Should respect limit parameter");
}

#[tokio::test]
async fn test_stream_stats_command() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Create room
    send_command(
        &client,
        &base_url,
        "stream.create",
        json!({
            "room": "stats_room"
        }),
    )
    .await;

    // Publish some events
    for i in 1..=3 {
        send_command(
            &client,
            &base_url,
            "stream.publish",
            json!({
                "room": "stats_room",
                "event": "event",
                "data": {"i": i}
            }),
        )
        .await;
    }

    // Get stats
    let res = send_command(
        &client,
        &base_url,
        "stream.stats",
        json!({
            "room": "stats_room"
        }),
    )
    .await;

    assert_eq!(res["success"], true);
    assert_eq!(res["payload"]["name"], "stats_room"); // RoomStats has "name" field
    assert!(res["payload"]["message_count"].as_u64().unwrap() >= 3);
}

#[tokio::test]
async fn test_stream_list_command() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Create multiple rooms
    for i in 1..=3 {
        send_command(
            &client,
            &base_url,
            "stream.create",
            json!({
                "room": format!("room{}", i)
            }),
        )
        .await;
    }

    // List rooms
    let res = send_command(&client, &base_url, "stream.list", json!({})).await;

    assert_eq!(res["success"], true);
    assert_eq!(res["payload"]["count"], 3);
    let rooms = res["payload"]["rooms"].as_array().unwrap();
    assert_eq!(rooms.len(), 3);
}

#[tokio::test]
async fn test_stream_delete_command() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Create room
    send_command(
        &client,
        &base_url,
        "stream.create",
        json!({
            "room": "delete_room"
        }),
    )
    .await;

    // Delete room
    let res = send_command(
        &client,
        &base_url,
        "stream.delete",
        json!({
            "room": "delete_room"
        }),
    )
    .await;

    assert_eq!(res["success"], true);
    assert_eq!(res["payload"]["deleted"], "delete_room");

    // Verify room is deleted by trying to get stats
    let res = send_command(
        &client,
        &base_url,
        "stream.stats",
        json!({
            "room": "delete_room"
        }),
    )
    .await;

    assert_eq!(res["success"], false);
}

#[tokio::test]
async fn test_stream_multiple_subscribers() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Create and publish
    send_command(
        &client,
        &base_url,
        "stream.create",
        json!({
            "room": "multi_sub_room"
        }),
    )
    .await;

    for i in 1..=10 {
        send_command(
            &client,
            &base_url,
            "stream.publish",
            json!({
                "room": "multi_sub_room",
                "event": "event",
                "data": {"i": i}
            }),
        )
        .await;
    }

    // Multiple subscribers consume
    for sub_id in 1..=3 {
        let res = send_command(
            &client,
            &base_url,
            "stream.consume",
            json!({
                "room": "multi_sub_room",
                "subscriber_id": format!("sub{}", sub_id),
                "from_offset": 0,
                "limit": 10
            }),
        )
        .await;

        let events = res["payload"]["events"].as_array().unwrap();
        assert_eq!(events.len(), 10, "All subscribers should get all events");
    }
}

#[tokio::test]
async fn test_stream_error_missing_room() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    let res = send_command(
        &client,
        &base_url,
        "stream.publish",
        json!({
            // Missing room field
            "event": "test",
            "data": {}
        }),
    )
    .await;

    assert_eq!(res["success"], false);
    assert!(res["error"].as_str().unwrap().contains("room"));
}

#[tokio::test]
async fn test_stream_error_room_not_found() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    let res = send_command(
        &client,
        &base_url,
        "stream.publish",
        json!({
            "room": "nonexistent_room",
            "event": "test",
            "data": {}
        }),
    )
    .await;

    assert_eq!(res["success"], false);
}

#[tokio::test]
async fn test_stream_offset_tracking() {
    let base_url = spawn_test_server().await;
    let client = Client::new();

    // Create room
    send_command(
        &client,
        &base_url,
        "stream.create",
        json!({
            "room": "offset_track"
        }),
    )
    .await;

    // Publish and track offsets
    let mut last_offset = 0;
    for i in 1..=5 {
        let res = send_command(
            &client,
            &base_url,
            "stream.publish",
            json!({
                "room": "offset_track",
                "event": "event",
                "data": {"i": i}
            }),
        )
        .await;

        let offset = res["payload"]["offset"].as_u64().unwrap();
        assert!(
            offset >= last_offset,
            "Offsets should be monotonically increasing"
        );
        last_offset = offset;
    }
}
