use rmcp::model::CallToolRequestParam;
use serde_json::json;
use std::sync::Arc;
use synap_server::monitoring::MonitoringManager;
use synap_server::{
    AppState, KVStore, QueueConfig, QueueManager, ScriptManager, ServerConfig, handle_mcp_tool,
};

#[tokio::test]
async fn test_mcp_kv_get() {
    let config = ServerConfig::default();
    let kv_store = Arc::new(KVStore::new(config.to_kv_config()));
    let hash_store = Arc::new(synap_server::core::HashStore::new());
    let kv_store_clone = kv_store.clone();

    let list_store = Arc::new(synap_server::core::ListStore::new());
    let set_store = Arc::new(synap_server::core::SetStore::new());
    let sorted_set_store = Arc::new(synap_server::core::SortedSetStore::new());
    let monitoring = Arc::new(MonitoringManager::new(
        kv_store.clone(),
        hash_store.clone(),
        list_store.clone(),
        set_store.clone(),
        sorted_set_store.clone(),
    ));
    let transaction_manager = Arc::new(synap_server::core::TransactionManager::new(
        kv_store.clone(),
        hash_store.clone(),
        list_store.clone(),
        set_store.clone(),
        sorted_set_store.clone(),
    ));
    let geospatial_store = Arc::new(synap_server::core::GeospatialStore::new(
        sorted_set_store.clone(),
    ));
    let state = Arc::new(AppState {
        kv_store: kv_store.clone(),
        hash_store: hash_store.clone(),
        list_store,
        set_store,
        sorted_set_store,
        hyperloglog_store: Arc::new(synap_server::core::HyperLogLogStore::new()),
        bitmap_store: Arc::new(synap_server::core::BitmapStore::new()),
        geospatial_store,
        queue_manager: None,
        stream_manager: None,
        partition_manager: None,
        consumer_group_manager: None,
        pubsub_router: None,
        persistence: None,
        monitoring,
        transaction_manager,
        script_manager: Arc::new(ScriptManager::default()),
    });

    // Set a value first (use clone before moving to state)
    kv_store_clone
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
    let kv_store = Arc::new(KVStore::new(config.to_kv_config()));
    let list_store = Arc::new(synap_server::core::ListStore::new());
    let set_store = Arc::new(synap_server::core::SetStore::new());
    let sorted_set_store = Arc::new(synap_server::core::SortedSetStore::new());
    let monitoring = Arc::new(MonitoringManager::new(
        kv_store.clone(),
        hash_store.clone(),
        list_store.clone(),
        set_store.clone(),
        sorted_set_store.clone(),
    ));
    let transaction_manager = Arc::new(synap_server::core::TransactionManager::new(
        kv_store.clone(),
        hash_store.clone(),
        list_store.clone(),
        set_store.clone(),
        sorted_set_store.clone(),
    ));
    let geospatial_store = Arc::new(synap_server::core::GeospatialStore::new(
        sorted_set_store.clone(),
    ));
    let state = Arc::new(AppState {
        kv_store,
        hash_store,
        list_store,
        set_store,
        sorted_set_store,
        hyperloglog_store: Arc::new(synap_server::core::HyperLogLogStore::new()),
        bitmap_store: Arc::new(synap_server::core::BitmapStore::new()),
        geospatial_store,
        queue_manager: None,
        stream_manager: None,
        partition_manager: None,
        consumer_group_manager: None,
        pubsub_router: None,
        persistence: None,
        monitoring,
        transaction_manager,
        script_manager: Arc::new(ScriptManager::default()),
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

    // Verify it was set (use state.kv_store since state owns it)
    let value = state.kv_store.get("mcp_key").await.unwrap();
    assert!(value.is_some());
}

#[tokio::test]
async fn test_mcp_kv_delete() {
    let config = ServerConfig::default();
    let kv_store = Arc::new(KVStore::new(config.to_kv_config()));
    let hash_store = Arc::new(synap_server::core::HashStore::new());
    let kv_store_clone = kv_store.clone();

    let list_store = Arc::new(synap_server::core::ListStore::new());
    let set_store = Arc::new(synap_server::core::SetStore::new());
    let sorted_set_store = Arc::new(synap_server::core::SortedSetStore::new());
    let monitoring = Arc::new(MonitoringManager::new(
        kv_store.clone(),
        hash_store.clone(),
        list_store.clone(),
        set_store.clone(),
        sorted_set_store.clone(),
    ));
    let transaction_manager = Arc::new(synap_server::core::TransactionManager::new(
        kv_store.clone(),
        hash_store.clone(),
        list_store.clone(),
        set_store.clone(),
        sorted_set_store.clone(),
    ));
    let geospatial_store = Arc::new(synap_server::core::GeospatialStore::new(
        sorted_set_store.clone(),
    ));
    let state = Arc::new(AppState {
        kv_store: kv_store.clone(),
        hash_store: hash_store.clone(),
        list_store,
        set_store,
        sorted_set_store,
        hyperloglog_store: Arc::new(synap_server::core::HyperLogLogStore::new()),
        bitmap_store: Arc::new(synap_server::core::BitmapStore::new()),
        geospatial_store,
        queue_manager: None,
        stream_manager: None,
        partition_manager: None,
        consumer_group_manager: None,
        pubsub_router: None,
        persistence: None,
        monitoring,
        transaction_manager,
        script_manager: Arc::new(ScriptManager::default()),
    });

    // Set then delete (use clone before moving to state)
    kv_store_clone
        .set("to_del", b"value".to_vec(), None)
        .await
        .unwrap();

    let request = CallToolRequestParam {
        name: "synap_kv_delete".into(),
        arguments: json!({"key": "to_del"}).as_object().cloned(),
    };

    let result = handle_mcp_tool(request, state.clone()).await;
    assert!(result.is_ok());

    // Verify deleted (kv_store was moved into state, use clone)
    let value = kv_store_clone.get("to_del").await.unwrap();
    assert!(value.is_none());
}

// Test removed: synap_kv_scan tool was removed for Cursor MCP tool limit compatibility
// The functionality is still available via REST API
// #[tokio::test]
// async fn test_mcp_kv_scan() { ... }

#[tokio::test]
async fn test_mcp_queue_publish() {
    let config = ServerConfig::default();
    let queue_manager = Arc::new(QueueManager::new(QueueConfig::default()));
    let hash_store = Arc::new(synap_server::core::HashStore::new());

    let kv_store = Arc::new(KVStore::new(config.to_kv_config()));
    let list_store = Arc::new(synap_server::core::ListStore::new());
    let set_store = Arc::new(synap_server::core::SetStore::new());
    let sorted_set_store = Arc::new(synap_server::core::SortedSetStore::new());
    let monitoring = Arc::new(MonitoringManager::new(
        kv_store.clone(),
        hash_store.clone(),
        list_store.clone(),
        set_store.clone(),
        sorted_set_store.clone(),
    ));
    let transaction_manager = Arc::new(synap_server::core::TransactionManager::new(
        kv_store.clone(),
        hash_store.clone(),
        list_store.clone(),
        set_store.clone(),
        sorted_set_store.clone(),
    ));
    let geospatial_store = Arc::new(synap_server::core::GeospatialStore::new(
        sorted_set_store.clone(),
    ));
    let state = Arc::new(AppState {
        kv_store,
        hash_store,
        list_store,
        set_store,
        sorted_set_store,
        hyperloglog_store: Arc::new(synap_server::core::HyperLogLogStore::new()),
        bitmap_store: Arc::new(synap_server::core::BitmapStore::new()),
        geospatial_store,
        queue_manager: Some(queue_manager.clone()),
        stream_manager: None,
        partition_manager: None,
        consumer_group_manager: None,
        pubsub_router: None,
        persistence: None,
        monitoring,
        transaction_manager,
        script_manager: Arc::new(ScriptManager::default()),
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
