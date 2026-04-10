//! MCP Authentication Integration Tests
//!
//! Tests that verify authentication and permission checks work correctly
//! for MCP (Model Context Protocol) requests.

use axum::http::StatusCode;
use base64::{Engine as _, engine::general_purpose};
use reqwest::Client;
use rmcp::model::CallToolRequestParam;
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use synap_server::auth::{Action, ApiKeyManager, AuthContext, Permission, UserManager};
use synap_server::config::{McpConfig, RateLimitConfig};
use synap_server::core::{
    GeospatialStore, HashStore, HyperLogLogStore, ListStore, SetStore, SortedSetStore,
    TransactionManager,
};
use synap_server::monitoring::{ClientListManager, MonitoringManager};
use synap_server::server::router::create_router;
use synap_server::{AppState, KVConfig, KVStore, ScriptManager};
use tokio::net::TcpListener;

/// Spawn a test server with MCP authentication enabled
async fn spawn_test_server_with_mcp_auth(
    auth_enabled: bool,
    require_auth: bool,
) -> (String, Arc<UserManager>, Arc<ApiKeyManager>) {
    let kv_store = Arc::new(KVStore::new(KVConfig::default()));
    let hash_store = Arc::new(HashStore::new());
    let list_store = Arc::new(ListStore::new());
    let set_store = Arc::new(SetStore::new());
    let sorted_set_store = Arc::new(SortedSetStore::new());

    let monitoring = Arc::new(MonitoringManager::new(
        kv_store.clone(),
        hash_store.clone(),
        list_store.clone(),
        set_store.clone(),
        sorted_set_store.clone(),
    ));

    let transaction_manager = Arc::new(TransactionManager::new(
        kv_store.clone(),
        hash_store.clone(),
        list_store.clone(),
        set_store.clone(),
        sorted_set_store.clone(),
    ));

    let script_manager = Arc::new(ScriptManager::default());
    let client_list_manager = Arc::new(ClientListManager::new());
    let hyperloglog_store = Arc::new(HyperLogLogStore::new());
    let bitmap_store = Arc::new(synap_server::core::BitmapStore::new());
    let geospatial_store = Arc::new(GeospatialStore::new(sorted_set_store.clone()));

    let state = AppState {
        kv_store,
        hash_store,
        list_store,
        set_store,
        sorted_set_store,
        hyperloglog_store,
        bitmap_store,
        geospatial_store,
        queue_manager: None,
        stream_manager: None,
        partition_manager: None,
        consumer_group_manager: None,
        pubsub_router: None,
        persistence: None,
        monitoring,
        transaction_manager,
        script_manager,
        client_list_manager,
        cluster_topology: None,
        cluster_migration: None,
        hub_client: None,
    };

    // Create user manager and API key manager
    let user_manager = Arc::new(UserManager::new());
    let api_key_manager = Arc::new(ApiKeyManager::new());

    // Initialize root user
    user_manager
        .initialize_root_user("root", "root12345", true)
        .unwrap();

    // Create test user with read-only permissions
    user_manager
        .create_user("readonly", "read12345", false)
        .unwrap();
    user_manager.add_user_role("readonly", "readonly").unwrap();

    // Create API key with read-only permissions
    let read_only_key = api_key_manager
        .create(
            "readonly-key",
            Some("readonly".to_string()),
            vec![Permission::new("kv:*", Action::Read)],
            vec![],
            None,
        )
        .unwrap();

    // Create API key with write permissions
    let write_key = api_key_manager
        .create(
            "write-key",
            Some("writer".to_string()),
            vec![Permission::new("kv:*", Action::Write)],
            vec![],
            None,
        )
        .unwrap();

    // Store keys for testing (in real scenario, keys are only shown once)
    // For testing, we'll use the keys we just created
    let _read_only_key_value = read_only_key.key.clone();
    let _write_key_value = write_key.key.clone();

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let base_url = format!("http://{}", addr);

    let router = create_router(
        state,
        RateLimitConfig {
            enabled: false,
            requests_per_second: 100,
            burst_size: 10,
        },
        McpConfig::default(),
        user_manager.clone(),
        api_key_manager.clone(),
        auth_enabled,
        require_auth,
    );

    tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });

    // Give server time to start
    tokio::time::sleep(Duration::from_millis(100)).await;

    (base_url, user_manager, api_key_manager)
}

/// Helper to make MCP request via HTTP
async fn make_mcp_request(
    client: &Client,
    base_url: &str,
    tool_name: &str,
    arguments: serde_json::Value,
    auth_header: Option<&str>,
) -> Result<reqwest::Response, reqwest::Error> {
    let mut request = client.post(format!("{}/mcp", base_url)).json(&json!({
        "method": "tools/call",
        "params": {
            "name": tool_name,
            "arguments": arguments
        }
    }));

    if let Some(auth) = auth_header {
        request = request.header("Authorization", auth);
    }

    request.send().await
}

#[tokio::test]
async fn test_mcp_basic_auth_success() {
    let (base_url, _, _) = spawn_test_server_with_mcp_auth(true, false).await;
    let client = Client::new();

    // Set a value first using Basic Auth
    let credentials = general_purpose::STANDARD.encode("root:root12345");
    let auth_header = format!("Basic {}", credentials);

    // First set a value (would need REST API or direct MCP call)
    // For this test, we'll test that MCP accepts Basic Auth
    let response = make_mcp_request(
        &client,
        &base_url,
        "synap_kv_get",
        json!({"key": "test_key"}),
        Some(&auth_header),
    )
    .await
    .unwrap();

    // Should not return 401 (unauthorized)
    assert_ne!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_mcp_api_key_auth_success() {
    let (base_url, _, api_key_manager) = spawn_test_server_with_mcp_auth(true, false).await;
    let client = Client::new();

    // Create a new API key for testing
    // Note: In real scenario, we'd need to store the key when created
    let test_key = api_key_manager
        .create(
            "test-key",
            Some("test".to_string()),
            vec![Permission::new("kv:*", Action::Read)],
            vec![],
            None,
        )
        .unwrap();

    let auth_header = format!("Bearer {}", test_key.key);

    let response = make_mcp_request(
        &client,
        &base_url,
        "synap_kv_get",
        json!({"key": "test_key"}),
        Some(&auth_header),
    )
    .await
    .unwrap();

    // Should not return 401 (unauthorized)
    assert_ne!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_mcp_no_auth_when_disabled() {
    let (base_url, _, _) = spawn_test_server_with_mcp_auth(false, false).await;
    let client = Client::new();

    let response = make_mcp_request(
        &client,
        &base_url,
        "synap_kv_get",
        json!({"key": "test_key"}),
        None,
    )
    .await
    .unwrap();

    // Should work without auth when auth is disabled
    assert_ne!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_mcp_require_auth_rejects_anonymous() {
    let (base_url, _, _) = spawn_test_server_with_mcp_auth(true, true).await;
    let client = Client::new();

    let response = make_mcp_request(
        &client,
        &base_url,
        "synap_kv_get",
        json!({"key": "test_key"}),
        None,
    )
    .await
    .unwrap();

    // Should return 401 when auth is required but not provided
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_mcp_permission_check_read_only() {
    use synap_server::auth::{Action, set_auth_context};
    use synap_server::server::mcp_handlers::handle_mcp_tool;

    let config = synap_server::ServerConfig::default();
    let kv_store = Arc::new(KVStore::new(config.to_kv_config()));
    let hash_store = Arc::new(HashStore::new());
    let list_store = Arc::new(ListStore::new());
    let set_store = Arc::new(SetStore::new());
    let sorted_set_store = Arc::new(SortedSetStore::new());

    let monitoring = Arc::new(MonitoringManager::new(
        kv_store.clone(),
        hash_store.clone(),
        list_store.clone(),
        set_store.clone(),
        sorted_set_store.clone(),
    ));

    let transaction_manager = Arc::new(TransactionManager::new(
        kv_store.clone(),
        hash_store.clone(),
        list_store.clone(),
        set_store.clone(),
        sorted_set_store.clone(),
    ));

    let geospatial_store = Arc::new(GeospatialStore::new(sorted_set_store.clone()));

    let state = Arc::new(AppState {
        kv_store: kv_store.clone(),
        hash_store,
        list_store,
        set_store,
        sorted_set_store,
        hyperloglog_store: Arc::new(HyperLogLogStore::new()),
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
        client_list_manager: Arc::new(synap_server::monitoring::ClientListManager::new()),
        cluster_topology: None,
        cluster_migration: None,
        hub_client: None,
    });

    // Set a value first
    kv_store
        .set("test_key", b"test_value".to_vec(), None)
        .await
        .unwrap();

    // Create read-only auth context
    let read_only_ctx = AuthContext {
        user_id: Some("readonly".to_string()),
        api_key_id: None,
        client_ip: std::net::IpAddr::from([127, 0, 0, 1]),
        permissions: vec![Permission::new("kv:*", Action::Read)],
        is_admin: false,
    };

    // Test read operation (should succeed)
    set_auth_context(read_only_ctx.clone());
    let read_request = CallToolRequestParam {
        name: "synap_kv_get".into(),
        arguments: json!({"key": "test_key"}).as_object().cloned(),
    };
    let read_result = handle_mcp_tool(read_request, state.clone()).await;
    assert!(read_result.is_ok(), "Read operation should succeed");

    // Test write operation (should fail)
    let write_request = CallToolRequestParam {
        name: "synap_kv_set".into(),
        arguments: json!({
            "key": "test_key2",
            "value": "new_value"
        })
        .as_object()
        .cloned(),
    };
    let write_result = handle_mcp_tool(write_request, state.clone()).await;
    assert!(
        write_result.is_err(),
        "Write operation should fail for read-only user"
    );

    // Clean up
    synap_server::auth::clear_auth_context();
}

#[tokio::test]
async fn test_mcp_permission_check_write_allowed() {
    use synap_server::auth::{Action, set_auth_context};
    use synap_server::server::mcp_handlers::handle_mcp_tool;

    let config = synap_server::ServerConfig::default();
    let kv_store = Arc::new(KVStore::new(config.to_kv_config()));
    let hash_store = Arc::new(HashStore::new());
    let list_store = Arc::new(ListStore::new());
    let set_store = Arc::new(SetStore::new());
    let sorted_set_store = Arc::new(SortedSetStore::new());

    let monitoring = Arc::new(MonitoringManager::new(
        kv_store.clone(),
        hash_store.clone(),
        list_store.clone(),
        set_store.clone(),
        sorted_set_store.clone(),
    ));

    let transaction_manager = Arc::new(TransactionManager::new(
        kv_store.clone(),
        hash_store.clone(),
        list_store.clone(),
        set_store.clone(),
        sorted_set_store.clone(),
    ));

    let geospatial_store = Arc::new(GeospatialStore::new(sorted_set_store.clone()));

    let state = Arc::new(AppState {
        kv_store: kv_store.clone(),
        hash_store,
        list_store,
        set_store,
        sorted_set_store,
        hyperloglog_store: Arc::new(HyperLogLogStore::new()),
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
        client_list_manager: Arc::new(synap_server::monitoring::ClientListManager::new()),
        cluster_topology: None,
        cluster_migration: None,
        hub_client: None,
    });

    // Create write-enabled auth context
    let write_ctx = AuthContext {
        user_id: Some("writer".to_string()),
        api_key_id: None,
        client_ip: std::net::IpAddr::from([127, 0, 0, 1]),
        permissions: vec![Permission::new("kv:*", Action::Write)],
        is_admin: false,
    };

    // Test write operation (should succeed)
    set_auth_context(write_ctx.clone());
    let write_request = CallToolRequestParam {
        name: "synap_kv_set".into(),
        arguments: json!({
            "key": "test_write_key",
            "value": "test_value"
        })
        .as_object()
        .cloned(),
    };
    let write_result = handle_mcp_tool(write_request, state.clone()).await;
    assert!(write_result.is_ok(), "Write operation should succeed");

    // Clean up
    synap_server::auth::clear_auth_context();
}

#[tokio::test]
async fn test_mcp_admin_bypass_permissions() {
    use synap_server::auth::set_auth_context;
    use synap_server::server::mcp_handlers::handle_mcp_tool;

    let config = synap_server::ServerConfig::default();
    let kv_store = Arc::new(KVStore::new(config.to_kv_config()));
    let hash_store = Arc::new(HashStore::new());
    let list_store = Arc::new(ListStore::new());
    let set_store = Arc::new(SetStore::new());
    let sorted_set_store = Arc::new(SortedSetStore::new());

    let monitoring = Arc::new(MonitoringManager::new(
        kv_store.clone(),
        hash_store.clone(),
        list_store.clone(),
        set_store.clone(),
        sorted_set_store.clone(),
    ));

    let transaction_manager = Arc::new(TransactionManager::new(
        kv_store.clone(),
        hash_store.clone(),
        list_store.clone(),
        set_store.clone(),
        sorted_set_store.clone(),
    ));

    let geospatial_store = Arc::new(GeospatialStore::new(sorted_set_store.clone()));

    let state = Arc::new(AppState {
        kv_store: kv_store.clone(),
        hash_store,
        list_store,
        set_store,
        sorted_set_store,
        hyperloglog_store: Arc::new(HyperLogLogStore::new()),
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
        client_list_manager: Arc::new(synap_server::monitoring::ClientListManager::new()),
        cluster_topology: None,
        cluster_migration: None,
        hub_client: None,
    });

    // Create admin auth context (no specific permissions needed)
    let admin_ctx = AuthContext {
        user_id: Some("admin".to_string()),
        api_key_id: None,
        client_ip: std::net::IpAddr::from([127, 0, 0, 1]),
        permissions: vec![],
        is_admin: true, // Admin bypasses all permission checks
    };

    // Test all operations (should succeed for admin)
    set_auth_context(admin_ctx.clone());

    // Write operation
    let write_request = CallToolRequestParam {
        name: "synap_kv_set".into(),
        arguments: json!({
            "key": "admin_key",
            "value": "admin_value"
        })
        .as_object()
        .cloned(),
    };
    let write_result = handle_mcp_tool(write_request, state.clone()).await;
    assert!(write_result.is_ok(), "Admin should be able to write");

    // Read operation
    let read_request = CallToolRequestParam {
        name: "synap_kv_get".into(),
        arguments: json!({"key": "admin_key"}).as_object().cloned(),
    };
    let read_result = handle_mcp_tool(read_request, state.clone()).await;
    assert!(read_result.is_ok(), "Admin should be able to read");

    // Delete operation
    let delete_request = CallToolRequestParam {
        name: "synap_kv_delete".into(),
        arguments: json!({"key": "admin_key"}).as_object().cloned(),
    };
    let delete_result = handle_mcp_tool(delete_request, state.clone()).await;
    assert!(delete_result.is_ok(), "Admin should be able to delete");

    // Clean up
    synap_server::auth::clear_auth_context();
}
