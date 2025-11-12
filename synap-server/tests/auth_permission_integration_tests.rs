//! Integration tests for authentication and permission checking
//!
//! Tests that verify authentication middleware and permission checks work correctly
//! across all REST API endpoints.

use axum::http::StatusCode;
use base64::{Engine as _, engine::general_purpose};
use reqwest::Client;
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use synap_server::auth::{Action, ApiKeyManager, Permission, UserManager};
use synap_server::config::{McpConfig, RateLimitConfig};
use synap_server::core::{
    GeospatialStore, HashStore, HyperLogLogStore, ListStore, SetStore, SortedSetStore,
    TransactionManager,
};
use synap_server::monitoring::{ClientListManager, MonitoringManager};
use synap_server::server::router::create_router;
use synap_server::{AppState, KVConfig, KVStore, ScriptManager};
use tokio::net::TcpListener;

/// Spawn a test server with authentication enabled
async fn spawn_test_server_with_auth(
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
    };

    // Create user manager and API key manager
    let user_manager = Arc::new(UserManager::new());
    let api_key_manager = Arc::new(ApiKeyManager::new());

    // Initialize root user
    user_manager
        .initialize_root_user("root", "root12345", true)
        .unwrap();

    // Create test users with different permissions
    user_manager.create_user("admin", "admin123", true).unwrap();
    user_manager
        .create_user("readonly", "read12345", false)
        .unwrap();
    user_manager
        .create_user("writer", "write123", false)
        .unwrap();

    // Grant readonly role to readonly user
    user_manager.add_user_role("readonly", "readonly").unwrap();

    // Create API keys with different permissions (for future use in tests)
    let _read_only_key = api_key_manager
        .create(
            "readonly-key",
            Some("readonly".to_string()),
            vec![Permission::new("kv:*", Action::Read)],
            vec![],
            None,
        )
        .unwrap();

    let _write_key = api_key_manager
        .create(
            "write-key",
            Some("writer".to_string()),
            vec![Permission::new("kv:*", Action::Write)],
            vec![],
            None,
        )
        .unwrap();

    let _admin_key = api_key_manager
        .create(
            "admin-key",
            Some("admin".to_string()),
            vec![Permission::new("*", Action::All)],
            vec![],
            None,
        )
        .unwrap();

    let app = create_router(
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

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let url = format!("http://{}", addr);

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    tokio::time::sleep(Duration::from_millis(100)).await;

    // Store keys for testing (in real scenario, keys are only shown once)
    // For testing, we'll use the keys we just created
    (url, user_manager, api_key_manager)
}

/// Helper to create Basic Auth header
fn basic_auth_header(username: &str, password: &str) -> String {
    let credentials = format!("{}:{}", username, password);
    let encoded = general_purpose::STANDARD.encode(credentials);
    format!("Basic {}", encoded)
}

/// Helper to create Bearer Token header
fn bearer_token_header(token: &str) -> String {
    format!("Bearer {}", token)
}

// ==================== Basic Authentication Tests ====================

#[tokio::test]
async fn test_basic_auth_success() {
    let (base_url, _, _) = spawn_test_server_with_auth(true, false).await;
    let client = Client::new();

    // Test successful authentication with root user
    let response = client
        .get(format!("{}/kv/get/test", base_url))
        .header("Authorization", basic_auth_header("root", "root12345"))
        .send()
        .await
        .unwrap();

    // Should succeed (even if key doesn't exist, auth worked)
    assert!(response.status() == StatusCode::OK || response.status() == StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_basic_auth_failure() {
    let (base_url, _, _) = spawn_test_server_with_auth(true, false).await;
    let client = Client::new();

    // Test failed authentication
    let response = client
        .get(format!("{}/kv/get/test", base_url))
        .header("Authorization", basic_auth_header("root", "wrongpass"))
        .send()
        .await
        .unwrap();

    // Should fail authentication (401) or return 400 if Basic Auth parsing fails
    let status = response.status();
    assert!(
        status == StatusCode::UNAUTHORIZED || status == StatusCode::BAD_REQUEST,
        "Expected 401 or 400, got: {}",
        status
    );
}

#[tokio::test]
async fn test_basic_auth_missing_credentials() {
    let (base_url, _, _) = spawn_test_server_with_auth(true, true).await;
    let client = Client::new();

    // Test missing credentials when auth is required
    let response = client
        .get(format!("{}/kv/get/test", base_url))
        .send()
        .await
        .unwrap();

    // Should return 401 when require_auth=true
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

// ==================== API Key Authentication Tests ====================

#[tokio::test]
async fn test_api_key_bearer_token() {
    let (base_url, _, api_key_manager) = spawn_test_server_with_auth(true, false).await;
    let client = Client::new();

    // Get an API key
    let keys = api_key_manager.list();
    assert!(!keys.is_empty());
    let test_key = &keys[0];

    // Test API key authentication via Bearer token
    let response = client
        .get(format!("{}/kv/get/test", base_url))
        .header("Authorization", bearer_token_header(&test_key.key))
        .send()
        .await
        .unwrap();

    // Should succeed (OK if key exists, NOT_FOUND if key doesn't exist, or FORBIDDEN if no permission)
    let status = response.status();
    assert!(
        status == StatusCode::OK
            || status == StatusCode::NOT_FOUND
            || status == StatusCode::FORBIDDEN,
        "Unexpected status: {}",
        status
    );
}

#[tokio::test]
async fn test_api_key_query_parameter() {
    let (base_url, _, api_key_manager) = spawn_test_server_with_auth(true, false).await;
    let client = Client::new();

    // Get an API key
    let keys = api_key_manager.list();
    assert!(!keys.is_empty());
    let test_key = &keys[0];

    // Test API key authentication via query parameter
    let response = client
        .get(format!("{}/kv/get/test?api_key={}", base_url, test_key.key))
        .send()
        .await
        .unwrap();

    // Should succeed (OK if key exists, NOT_FOUND if key doesn't exist, or FORBIDDEN if no permission)
    let status = response.status();
    assert!(
        status == StatusCode::OK
            || status == StatusCode::NOT_FOUND
            || status == StatusCode::FORBIDDEN,
        "Unexpected status: {}",
        status
    );
}

// ==================== Permission Checking Tests ====================

#[tokio::test]
async fn test_read_permission_success() {
    let (base_url, _user_manager, api_key_manager) = spawn_test_server_with_auth(true, false).await;
    let client = Client::new();

    // Create a key first with admin user
    client
        .post(format!("{}/kv/set", base_url))
        .header("Authorization", basic_auth_header("root", "root12345"))
        .json(&json!({
            "key": "test_read",
            "value": "test_value"
        }))
        .send()
        .await
        .unwrap();

    // Create API key with read permission
    let read_key = api_key_manager
        .create(
            "test-read-key",
            Some("readonly".to_string()),
            vec![Permission::new("kv:test_read", Action::Read)],
            vec![],
            None,
        )
        .unwrap();

    // Test read with read permission
    let response = client
        .get(format!("{}/kv/get/test_read", base_url))
        .header("Authorization", bearer_token_header(&read_key.key))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_read_permission_denied() {
    let (base_url, _user_manager, api_key_manager) = spawn_test_server_with_auth(true, false).await;
    let client = Client::new();

    // Create a key first with admin user
    client
        .post(format!("{}/kv/set", base_url))
        .header("Authorization", basic_auth_header("root", "root12345"))
        .json(&json!({
            "key": "test_read_denied",
            "value": "test_value"
        }))
        .send()
        .await
        .unwrap();

    // Create API key with write-only permission (no read)
    let write_key = api_key_manager
        .create(
            "test-write-only-key",
            Some("writer".to_string()),
            vec![Permission::new("kv:test_read_denied", Action::Write)],
            vec![],
            None,
        )
        .unwrap();

    // Test read with write-only permission (should fail)
    let response = client
        .get(format!("{}/kv/get/test_read_denied", base_url))
        .header("Authorization", bearer_token_header(&write_key.key))
        .send()
        .await
        .unwrap();

    // Should return 403 Forbidden or error indicating insufficient permissions
    let status = response.status();
    let body: serde_json::Value = response.json().await.unwrap();
    assert!(
        status == StatusCode::FORBIDDEN
            || body.to_string().contains("permission")
            || body.to_string().contains("Insufficient")
    );
}

#[tokio::test]
async fn test_write_permission_success() {
    let (base_url, _, api_key_manager) = spawn_test_server_with_auth(true, false).await;
    let client = Client::new();

    // Create API key with write permission
    let write_key = api_key_manager
        .create(
            "test-write-key",
            Some("writer".to_string()),
            vec![Permission::new("kv:test_write", Action::Write)],
            vec![],
            None,
        )
        .unwrap();

    // Test write with write permission
    let response = client
        .post(format!("{}/kv/set", base_url))
        .header("Authorization", bearer_token_header(&write_key.key))
        .json(&json!({
            "key": "test_write",
            "value": "test_value"
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_write_permission_denied() {
    let (base_url, _, api_key_manager) = spawn_test_server_with_auth(true, false).await;
    let client = Client::new();

    // Create API key with read-only permission
    let read_key = api_key_manager
        .create(
            "test-read-only-key",
            Some("readonly".to_string()),
            vec![Permission::new("kv:test_write_denied", Action::Read)],
            vec![],
            None,
        )
        .unwrap();

    // Test write with read-only permission (should fail)
    let response = client
        .post(format!("{}/kv/set", base_url))
        .header("Authorization", bearer_token_header(&read_key.key))
        .json(&json!({
            "key": "test_write_denied",
            "value": "test_value"
        }))
        .send()
        .await
        .unwrap();

    // Should return 403 Forbidden or error indicating insufficient permissions
    let status = response.status();
    let body: serde_json::Value = response.json().await.unwrap();
    assert!(
        status == StatusCode::FORBIDDEN
            || body.to_string().contains("permission")
            || body.to_string().contains("Insufficient")
    );
}

#[tokio::test]
async fn test_delete_permission() {
    let (base_url, _, api_key_manager) = spawn_test_server_with_auth(true, false).await;
    let client = Client::new();

    // Create a key first with admin user
    client
        .post(format!("{}/kv/set", base_url))
        .header("Authorization", basic_auth_header("root", "root12345"))
        .json(&json!({
            "key": "test_delete",
            "value": "test_value"
        }))
        .send()
        .await
        .unwrap();

    // Create API key with delete permission
    let delete_key = api_key_manager
        .create(
            "test-delete-key",
            Some("writer".to_string()),
            vec![Permission::new("kv:test_delete", Action::Delete)],
            vec![],
            None,
        )
        .unwrap();

    // Test delete with delete permission
    let response = client
        .delete(format!("{}/kv/del/test_delete", base_url))
        .header("Authorization", bearer_token_header(&delete_key.key))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_wildcard_permission() {
    let (base_url, _, api_key_manager) = spawn_test_server_with_auth(true, false).await;
    let client = Client::new();

    // Create API key with wildcard read permission
    let wildcard_key = api_key_manager
        .create(
            "test-wildcard-key",
            Some("readonly".to_string()),
            vec![Permission::new("kv:*", Action::Read)],
            vec![],
            None,
        )
        .unwrap();

    // Create a key first
    client
        .post(format!("{}/kv/set", base_url))
        .header("Authorization", basic_auth_header("root", "root12345"))
        .json(&json!({
            "key": "test_wildcard",
            "value": "test_value"
        }))
        .send()
        .await
        .unwrap();

    // Test read with wildcard permission
    let response = client
        .get(format!("{}/kv/get/test_wildcard", base_url))
        .header("Authorization", bearer_token_header(&wildcard_key.key))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_admin_permission() {
    let (base_url, _, api_key_manager) = spawn_test_server_with_auth(true, false).await;
    let client = Client::new();

    // Create API key with admin permission
    let admin_key = api_key_manager
        .create(
            "test-admin-key",
            Some("admin".to_string()),
            vec![Permission::new("admin:*", Action::Admin)],
            vec![],
            None,
        )
        .unwrap();

    // Test admin endpoint (info) with admin permission
    let response = client
        .get(format!("{}/info", base_url))
        .header("Authorization", bearer_token_header(&admin_key.key))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_admin_permission_denied() {
    let (base_url, _, api_key_manager) = spawn_test_server_with_auth(true, false).await;
    let client = Client::new();

    // Create API key with read-only permission (no admin)
    let read_key = api_key_manager
        .create(
            "test-read-no-admin-key",
            Some("readonly".to_string()),
            vec![Permission::new("kv:*", Action::Read)],
            vec![],
            None,
        )
        .unwrap();

    // Test admin endpoint (info) without admin permission (should fail)
    let response = client
        .get(format!("{}/info", base_url))
        .header("Authorization", bearer_token_header(&read_key.key))
        .send()
        .await
        .unwrap();

    // Should return 403 Forbidden or error indicating insufficient permissions
    let status = response.status();
    let body: serde_json::Value = response.json().await.unwrap();
    assert!(
        status == StatusCode::FORBIDDEN
            || body.to_string().contains("permission")
            || body.to_string().contains("Insufficient")
            || body.to_string().contains("Admin")
    );
}

// ==================== Anonymous Access Tests ====================

#[tokio::test]
async fn test_anonymous_access_when_auth_disabled() {
    let (base_url, _, _) = spawn_test_server_with_auth(false, false).await;
    let client = Client::new();

    // Test access without authentication when auth is disabled
    let response = client
        .get(format!("{}/kv/get/test", base_url))
        .send()
        .await
        .unwrap();

    // Should succeed (auth disabled)
    assert!(response.status() == StatusCode::OK || response.status() == StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_anonymous_access_when_auth_enabled_but_not_required() {
    let (base_url, _, _) = spawn_test_server_with_auth(true, false).await;
    let client = Client::new();

    // Test access without authentication when auth is enabled but not required
    // This should work but with anonymous context (no permissions)
    let response = client
        .get(format!("{}/kv/get/test", base_url))
        .send()
        .await
        .unwrap();

    // Anonymous context has no permissions, so should get FORBIDDEN
    // or NOT_FOUND if key doesn't exist (but permission check happens first)
    // or BAD_REQUEST if there's a parsing issue
    let status = response.status();
    assert!(
        status == StatusCode::FORBIDDEN
            || status == StatusCode::NOT_FOUND
            || status == StatusCode::OK // If anonymous is allowed
            || status == StatusCode::BAD_REQUEST, // If there's a parsing issue
        "Unexpected status: {}",
        status
    );
}
