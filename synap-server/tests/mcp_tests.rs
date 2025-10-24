use rmcp::model::CallToolRequestParam;
use serde_json::json;
use std::sync::Arc;
use synap_server::{AppState, KVStore, QueueConfig, QueueManager, ServerConfig, handle_mcp_tool};

#[tokio::test]
async fn test_mcp_kv_get() {
    let config = ServerConfig::default();
    let kv_store = Arc::new(KVStore::new(config.to_kv_config()));
    let hash_store = Arc::new(synap_server::core::HashStore::new());

    let state = Arc::new(AppState {
        kv_store: kv_store.clone(),
        hash_store,
        queue_manager: None,
        stream_manager: None,
        partition_manager: None,
        consumer_group_manager: None,
        pubsub_router: None,
        persistence: None,
    });

    // Set a value first
    kv_store
        .set("test_key", b"test_value".to_vec(), None)
        .await
        .unwrap();

    // Test MCP tool call
    let request = CallToolRequestParam {
        name: "synap_kv_get".into(),
        arguments: json!({"key": "test_key"}).as_object().cloned(),
    };

    let result = handle_mcp_tool(request, state).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_mcp_kv_set() {
    let config = ServerConfig::default();
    let hash_store = Arc::new(synap_server::core::HashStore::new());
    let state = Arc::new(AppState {
        kv_store: Arc::new(KVStore::new(config.to_kv_config())),
        hash_store,
        queue_manager: None,
        stream_manager: None,
        partition_manager: None,
        consumer_group_manager: None,
        pubsub_router: None,
        persistence: None,
    });

    let request = CallToolRequestParam {
        name: "synap_kv_set".into(),
        arguments: json!({
            "key": "mcp_key",
            "value": "mcp_value",
            "ttl": 60
        })
        .as_object()
        .cloned(),
    };

    let result = handle_mcp_tool(request, state.clone()).await;
    assert!(result.is_ok());

    // Verify it was set
    let value = state.kv_store.get("mcp_key").await.unwrap();
    assert!(value.is_some());
}

#[tokio::test]
async fn test_mcp_kv_delete() {
    let config = ServerConfig::default();
    let kv_store = Arc::new(KVStore::new(config.to_kv_config()));
    let hash_store = Arc::new(synap_server::core::HashStore::new());

    let state = Arc::new(AppState {
        kv_store: kv_store.clone(),
        hash_store,
        queue_manager: None,
        stream_manager: None,
        partition_manager: None,
        consumer_group_manager: None,
        pubsub_router: None,
        persistence: None,
    });

    // Set then delete
    kv_store
        .set("to_del", b"value".to_vec(), None)
        .await
        .unwrap();

    let request = CallToolRequestParam {
        name: "synap_kv_delete".into(),
        arguments: json!({"key": "to_del"}).as_object().cloned(),
    };

    let result = handle_mcp_tool(request, state.clone()).await;
    assert!(result.is_ok());

    // Verify deleted
    let value = kv_store.get("to_del").await.unwrap();
    assert!(value.is_none());
}

#[tokio::test]
async fn test_mcp_kv_scan() {
    let config = ServerConfig::default();
    let kv_store = Arc::new(KVStore::new(config.to_kv_config()));
    let hash_store = Arc::new(synap_server::core::HashStore::new());

    let state = Arc::new(AppState {
        kv_store: kv_store.clone(),
        hash_store,
        queue_manager: None,
        stream_manager: None,
        partition_manager: None,
        consumer_group_manager: None,
        pubsub_router: None,
        persistence: None,
    });

    // Set multiple keys
    for i in 1..=5 {
        kv_store
            .set(&format!("user:{}", i), b"data".to_vec(), None)
            .await
            .unwrap();
    }

    let request = CallToolRequestParam {
        name: "synap_kv_scan".into(),
        arguments: json!({
            "prefix": "user:",
            "limit": 10
        })
        .as_object()
        .cloned(),
    };

    let result = handle_mcp_tool(request, state).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_mcp_queue_publish() {
    let config = ServerConfig::default();
    let queue_manager = Arc::new(QueueManager::new(QueueConfig::default()));
    let hash_store = Arc::new(synap_server::core::HashStore::new());

    let state = Arc::new(AppState {
        kv_store: Arc::new(KVStore::new(config.to_kv_config())),
        hash_store,
        queue_manager: Some(queue_manager.clone()),
        stream_manager: None,
        partition_manager: None,
        consumer_group_manager: None,
        pubsub_router: None,
        persistence: None,
    });

    // Create queue
    queue_manager.create_queue("test_q", None).await.unwrap();

    let request = CallToolRequestParam {
        name: "synap_queue_publish".into(),
        arguments: json!({
            "queue": "test_q",
            "message": "test message",
            "priority": 8
        })
        .as_object()
        .cloned(),
    };

    let result = handle_mcp_tool(request, state).await;
    assert!(result.is_ok());
}
