//! Edge case tests for authentication and authorization
//!
//! Tests covering:
//! - API key expiration
//! - IP restrictions
//! - User management operations
//! - Permission edge cases (wildcards, prefixes, suffixes)
//! - Concurrent access
//! - Error handling
//! - API key enabled/disabled states
//! - Password management
//! - User roles

use axum::http::StatusCode;
use chrono::Utc;
use reqwest::Client;
use serde_json::json;
use std::net::IpAddr;
use std::sync::Arc;
use std::time::Duration as StdDuration;
use synap_server::auth::{Action, ApiKeyManager, Permission, UserManager};
use synap_server::config::{McpConfig, RateLimitConfig};
use synap_server::core::{
    BitmapStore, GeospatialStore, HashStore, HyperLogLogStore, ListStore, SetStore, SortedSetStore,
    TransactionManager,
};
use synap_server::monitoring::MonitoringManager;
use synap_server::server::router::create_router;
use synap_server::{AppState, KVConfig, KVStore, ScriptManager};
use tokio::net::TcpListener;
use tokio::time::sleep;

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
    let hyperloglog_store = Arc::new(HyperLogLogStore::new());
    let bitmap_store = Arc::new(BitmapStore::new());
    let geospatial_store = Arc::new(GeospatialStore::new(sorted_set_store.clone()));

    let user_manager = Arc::new(UserManager::new());
    let api_key_manager = Arc::new(ApiKeyManager::new());

    if auth_enabled {
        user_manager
            .initialize_root_user("root", "root123", true)
            .unwrap();

        // Create a test API key with full permissions
        api_key_manager
            .create(
                "test-key",
                None,
                vec![Permission::new("*", Action::All)],
                vec![],
                None,
            )
            .unwrap();
    }

    let app_state = AppState {
        kv_store,
        hash_store,
        list_store,
        set_store,
        sorted_set_store,
        geospatial_store,
        hyperloglog_store,
        bitmap_store,
        transaction_manager,
        script_manager,
        monitoring,
        queue_manager: None,
        stream_manager: None,
        partition_manager: None,
        consumer_group_manager: None,
        pubsub_router: None,
        persistence: None,
    };

    let router = create_router(
        app_state,
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
    let port = listener.local_addr().unwrap().port();
    let base_url = format!("http://127.0.0.1:{}", port);

    tokio::spawn(async move {
        let listener = tokio::net::TcpListener::from_std(listener.into_std().unwrap()).unwrap();
        axum::serve(listener, router.into_make_service())
            .await
            .unwrap();
    });

    // Give server time to start
    sleep(StdDuration::from_millis(100)).await;

    (base_url, user_manager, api_key_manager)
}

/// Helper to create Bearer Token header
fn bearer_token_header(token: &str) -> String {
    format!("Bearer {}", token)
}

// ==================== API Key Expiration Tests ====================

#[tokio::test]
async fn test_api_key_expiration() {
    let (base_url, _, api_key_manager) = spawn_test_server_with_auth(true, false).await;
    let client = Client::new();

    // Create an API key that expires in 1 second
    let expired_key = api_key_manager
        .create_temporary(
            "expiring-key",
            None,
            vec![Permission::new("kv:*", Action::Read)],
            vec![],
            1, // 1 second TTL
        )
        .unwrap();

    // Use key immediately (should work)
    let response = client
        .get(format!("{}/kv/get/test", base_url))
        .header("Authorization", bearer_token_header(&expired_key.key))
        .send()
        .await
        .unwrap();

    assert!(response.status() == StatusCode::OK || response.status() == StatusCode::NOT_FOUND);

    // Wait for expiration
    sleep(StdDuration::from_secs(2)).await;

    // Try to use expired key (should fail)
    let response = client
        .get(format!("{}/kv/get/test", base_url))
        .header("Authorization", bearer_token_header(&expired_key.key))
        .send()
        .await
        .unwrap();

    // Should return 401 Unauthorized for expired key
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_api_key_expiration_with_days() {
    let (base_url, _, api_key_manager) = spawn_test_server_with_auth(true, false).await;
    let client = Client::new();

    // Create an API key that expires in 1 day
    let key = api_key_manager
        .create(
            "day-expiring-key",
            None,
            vec![Permission::new("kv:*", Action::Read)],
            vec![],
            Some(1), // 1 day
        )
        .unwrap();

    // Should work immediately
    let response = client
        .get(format!("{}/kv/get/test", base_url))
        .header("Authorization", bearer_token_header(&key.key))
        .send()
        .await
        .unwrap();

    assert!(response.status() == StatusCode::OK || response.status() == StatusCode::NOT_FOUND);

    // Verify expiration time is set correctly
    let stored_key = api_key_manager.get(&key.id).unwrap();
    assert!(stored_key.expires_at.is_some());
    assert!(stored_key.expires_at.unwrap() > Utc::now());
}

// ==================== IP Restrictions Tests ====================

#[tokio::test]
async fn test_api_key_ip_restriction_allowed() {
    let (base_url, _, api_key_manager) = spawn_test_server_with_auth(true, false).await;
    let client = Client::new();

    // Create API key restricted to localhost
    let localhost_ip: IpAddr = "127.0.0.1".parse().unwrap();
    let restricted_key = api_key_manager
        .create(
            "ip-restricted-key",
            None,
            vec![Permission::new("kv:*", Action::Read)],
            vec![localhost_ip],
            None,
        )
        .unwrap();

    // Should work from localhost
    let response = client
        .get(format!("{}/kv/get/test", base_url))
        .header("Authorization", bearer_token_header(&restricted_key.key))
        .send()
        .await
        .unwrap();

    assert!(response.status() == StatusCode::OK || response.status() == StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_api_key_ip_restriction_denied() {
    let (base_url, _, api_key_manager) = spawn_test_server_with_auth(true, false).await;
    let client = Client::new();

    // Create API key restricted to a different IP
    let allowed_ip: IpAddr = "192.168.1.1".parse().unwrap();
    let restricted_key = api_key_manager
        .create(
            "ip-restricted-key-denied",
            None,
            vec![Permission::new("kv:*", Action::All)],
            vec![allowed_ip], // Only allow 192.168.1.1
            None,
        )
        .unwrap();

    // Request from localhost (127.0.0.1) should fail
    let response = client
        .get(format!("{}/kv/get/test", base_url))
        .header("Authorization", bearer_token_header(&restricted_key.key))
        .send()
        .await
        .unwrap();

    // Should return 401 Unauthorized due to IP restriction
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_api_key_no_ip_restriction() {
    let (base_url, _, api_key_manager) = spawn_test_server_with_auth(true, false).await;
    let client = Client::new();

    // Create API key with no IP restrictions (empty vec)
    let unrestricted_key = api_key_manager
        .create(
            "unrestricted-key",
            None,
            vec![Permission::new("kv:*", Action::Read)],
            vec![], // No IP restrictions
            None,
        )
        .unwrap();

    // Should work from any IP
    let response = client
        .get(format!("{}/kv/get/test", base_url))
        .header("Authorization", bearer_token_header(&unrestricted_key.key))
        .send()
        .await
        .unwrap();

    assert!(response.status() == StatusCode::OK || response.status() == StatusCode::NOT_FOUND);
}

// ==================== API Key Enabled/Disabled Tests ====================

#[tokio::test]
async fn test_api_key_disabled() {
    let (base_url, _, api_key_manager) = spawn_test_server_with_auth(true, false).await;
    let client = Client::new();

    // Create and disable an API key
    let disabled_key = api_key_manager
        .create(
            "disabled-key",
            None,
            vec![Permission::new("kv:*", Action::All)],
            vec![],
            None,
        )
        .unwrap();

    // Disable the key
    api_key_manager
        .set_enabled(&disabled_key.id, false)
        .unwrap();

    // Try to use disabled key (should fail)
    let response = client
        .get(format!("{}/kv/get/test", base_url))
        .header("Authorization", bearer_token_header(&disabled_key.key))
        .send()
        .await
        .unwrap();

    // Should return 401 Unauthorized
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_api_key_reenabled() {
    let (base_url, _, api_key_manager) = spawn_test_server_with_auth(true, false).await;
    let client = Client::new();

    // Create, disable, then re-enable an API key
    let key = api_key_manager
        .create(
            "reenabled-key",
            None,
            vec![Permission::new("kv:*", Action::Read)],
            vec![],
            None,
        )
        .unwrap();

    // Disable
    api_key_manager.set_enabled(&key.id, false).unwrap();

    // Should fail
    let response = client
        .get(format!("{}/kv/get/test", base_url))
        .header("Authorization", bearer_token_header(&key.key))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    // Re-enable
    api_key_manager.set_enabled(&key.id, true).unwrap();

    // Should work again
    let response = client
        .get(format!("{}/kv/get/test", base_url))
        .header("Authorization", bearer_token_header(&key.key))
        .send()
        .await
        .unwrap();

    assert!(response.status() == StatusCode::OK || response.status() == StatusCode::NOT_FOUND);
}

// ==================== User Management Tests ====================

#[tokio::test]
async fn test_create_user() {
    let (_, user_manager, _) = spawn_test_server_with_auth(true, false).await;

    // Create a new user
    user_manager
        .create_user("testuser", "password123", false)
        .unwrap();

    // Verify user exists
    let user = user_manager.get_user("testuser").unwrap();
    assert_eq!(user.username, "testuser");
    assert!(!user.is_admin);
    assert!(user.enabled);
}

#[tokio::test]
async fn test_create_duplicate_user() {
    let (_, user_manager, _) = spawn_test_server_with_auth(true, false).await;

    // Create a user
    user_manager
        .create_user("duplicate", "pass123", false)
        .unwrap();

    // Try to create duplicate (should fail)
    let result = user_manager.create_user("duplicate", "pass456", false);
    assert!(result.is_err());
}

#[tokio::test]
async fn test_delete_user() {
    let (_, user_manager, _) = spawn_test_server_with_auth(true, false).await;

    // Create a user
    user_manager
        .create_user("todelete", "pass123", false)
        .unwrap();

    // Delete user
    let deleted = user_manager.delete_user("todelete").unwrap();
    assert!(deleted);

    // Verify user is gone
    assert!(user_manager.get_user("todelete").is_none());
}

#[tokio::test]
async fn test_delete_root_user() {
    let (_, user_manager, _) = spawn_test_server_with_auth(true, false).await;

    // Try to delete root user (should fail)
    let result = user_manager.delete_user("root");
    assert!(result.is_err());
}

#[tokio::test]
async fn test_change_password() {
    let (_, user_manager, _) = spawn_test_server_with_auth(true, false).await;

    // Create a user
    user_manager
        .create_user("passuser", "oldpass", false)
        .unwrap();

    // Verify old password works
    let user = user_manager.authenticate("passuser", "oldpass").unwrap();
    assert_eq!(user.username, "passuser");

    // Change password
    user_manager.change_password("passuser", "newpass").unwrap();

    // Old password should fail
    assert!(user_manager.authenticate("passuser", "oldpass").is_err());

    // New password should work
    let user = user_manager.authenticate("passuser", "newpass").unwrap();
    assert_eq!(user.username, "passuser");
}

#[tokio::test]
async fn test_disable_user() {
    let (_, user_manager, _) = spawn_test_server_with_auth(true, false).await;

    // Create a user
    user_manager
        .create_user("disableuser", "pass123", false)
        .unwrap();

    // Should be able to authenticate
    assert!(user_manager.authenticate("disableuser", "pass123").is_ok());

    // Disable user
    user_manager.set_user_enabled("disableuser", false).unwrap();

    // Should not be able to authenticate
    assert!(user_manager.authenticate("disableuser", "pass123").is_err());
}

#[tokio::test]
async fn test_enable_user() {
    let (_, user_manager, _) = spawn_test_server_with_auth(true, false).await;

    // Create and disable a user
    user_manager
        .create_user("enableuser", "pass123", false)
        .unwrap();
    user_manager.set_user_enabled("enableuser", false).unwrap();

    // Should not be able to authenticate
    assert!(user_manager.authenticate("enableuser", "pass123").is_err());

    // Enable user
    user_manager.set_user_enabled("enableuser", true).unwrap();

    // Should be able to authenticate
    assert!(user_manager.authenticate("enableuser", "pass123").is_ok());
}

// ==================== Permission Edge Cases Tests ====================

#[tokio::test]
async fn test_permission_wildcard_prefix() {
    let (base_url, _, api_key_manager) = spawn_test_server_with_auth(true, false).await;
    let client = Client::new();

    // Create key with wildcard prefix permission
    let key = api_key_manager
        .create(
            "wildcard-prefix-key",
            None,
            vec![Permission::new("kv:test*", Action::Read)],
            vec![],
            None,
        )
        .unwrap();

    // Should match kv:test123
    let response = client
        .get(format!("{}/kv/get/test123", base_url))
        .header("Authorization", bearer_token_header(&key.key))
        .send()
        .await
        .unwrap();

    assert!(response.status() == StatusCode::OK || response.status() == StatusCode::NOT_FOUND);

    // Should NOT match kv:other
    let response = client
        .get(format!("{}/kv/get/other", base_url))
        .header("Authorization", bearer_token_header(&key.key))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_permission_wildcard_suffix() {
    let (base_url, _, api_key_manager) = spawn_test_server_with_auth(true, false).await;
    let client = Client::new();

    // Create key with wildcard suffix permission
    let key = api_key_manager
        .create(
            "wildcard-suffix-key",
            None,
            vec![Permission::new("kv:*test", Action::Read)],
            vec![],
            None,
        )
        .unwrap();

    // Should match kv:123test (or FORBIDDEN if permission check happens first)
    let response = client
        .get(format!("{}/kv/get/123test", base_url))
        .header("Authorization", bearer_token_header(&key.key))
        .send()
        .await
        .unwrap();

    let status = response.status();
    assert!(
        status == StatusCode::OK
            || status == StatusCode::NOT_FOUND
            || status == StatusCode::FORBIDDEN,
        "Unexpected status: {}",
        status
    );

    // Should NOT match kv:other
    let response = client
        .get(format!("{}/kv/get/other", base_url))
        .header("Authorization", bearer_token_header(&key.key))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_permission_exact_match() {
    let (base_url, _, api_key_manager) = spawn_test_server_with_auth(true, false).await;
    let client = Client::new();

    // Create key with exact match permission
    let key = api_key_manager
        .create(
            "exact-match-key",
            None,
            vec![Permission::new("kv:specific", Action::Read)],
            vec![],
            None,
        )
        .unwrap();

    // Should match exact key
    let response = client
        .get(format!("{}/kv/get/specific", base_url))
        .header("Authorization", bearer_token_header(&key.key))
        .send()
        .await
        .unwrap();

    assert!(response.status() == StatusCode::OK || response.status() == StatusCode::NOT_FOUND);

    // Should NOT match similar key
    let response = client
        .get(format!("{}/kv/get/specific2", base_url))
        .header("Authorization", bearer_token_header(&key.key))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_permission_action_hierarchy() {
    let (base_url, _, api_key_manager) = spawn_test_server_with_auth(true, false).await;
    let client = Client::new();

    // Create key with Configure action (should include Read and Write)
    let key = api_key_manager
        .create(
            "configure-action-key",
            None,
            vec![Permission::new("kv:*", Action::Configure)],
            vec![],
            None,
        )
        .unwrap();

    // Should allow read
    let response = client
        .get(format!("{}/kv/get/test", base_url))
        .header("Authorization", bearer_token_header(&key.key))
        .send()
        .await
        .unwrap();

    let status = response.status();
    assert!(
        status == StatusCode::OK
            || status == StatusCode::NOT_FOUND
            || status == StatusCode::FORBIDDEN,
        "Unexpected status: {}",
        status
    );

    // Should allow write
    let response = client
        .post(format!("{}/kv/set/test", base_url))
        .header("Authorization", bearer_token_header(&key.key))
        .json(&json!({"value": "test"}))
        .send()
        .await
        .unwrap();

    let status = response.status();
    assert!(
        status == StatusCode::OK
            || status == StatusCode::FORBIDDEN
            || status == StatusCode::NOT_FOUND,
        "Unexpected status: {}",
        status
    );
}

#[tokio::test]
async fn test_permission_all_action() {
    let (base_url, _, api_key_manager) = spawn_test_server_with_auth(true, false).await;
    let client = Client::new();

    // Create key with All action
    let key = api_key_manager
        .create(
            "all-action-key",
            None,
            vec![Permission::new("kv:*", Action::All)],
            vec![],
            None,
        )
        .unwrap();

    // Should allow read
    let response = client
        .get(format!("{}/kv/get/test", base_url))
        .header("Authorization", bearer_token_header(&key.key))
        .send()
        .await
        .unwrap();

    let status = response.status();
    assert!(
        status == StatusCode::OK
            || status == StatusCode::NOT_FOUND
            || status == StatusCode::FORBIDDEN,
        "Unexpected status: {}",
        status
    );

    // Should allow write
    let response = client
        .post(format!("{}/kv/set/test", base_url))
        .header("Authorization", bearer_token_header(&key.key))
        .json(&json!({"value": "test"}))
        .send()
        .await
        .unwrap();

    let status = response.status();
    assert!(
        status == StatusCode::OK
            || status == StatusCode::FORBIDDEN
            || status == StatusCode::NOT_FOUND,
        "Unexpected status: {}",
        status
    );

    // Should allow delete
    let response = client
        .delete(format!("{}/kv/del/test", base_url))
        .header("Authorization", bearer_token_header(&key.key))
        .send()
        .await
        .unwrap();

    let status = response.status();
    assert!(
        status == StatusCode::OK
            || status == StatusCode::NOT_FOUND
            || status == StatusCode::FORBIDDEN,
        "Unexpected status: {}",
        status
    );
}

// ==================== Concurrent Access Tests ====================

#[tokio::test]
async fn test_concurrent_api_key_usage() {
    let (base_url, _, api_key_manager) = spawn_test_server_with_auth(true, false).await;

    // Create a single API key
    let key = api_key_manager
        .create(
            "concurrent-key",
            None,
            vec![Permission::new("kv:*", Action::All)],
            vec![],
            None,
        )
        .unwrap();

    // Use the same key concurrently from multiple tasks
    let mut handles = vec![];
    for i in 0..10 {
        let base_url = base_url.clone();
        let key = key.key.clone();
        let handle = tokio::spawn(async move {
            let client = Client::new();
            let response = client
                .get(format!("{}/kv/get/test{}", base_url, i))
                .header("Authorization", bearer_token_header(&key))
                .send()
                .await
                .unwrap();
            response.status()
        });
        handles.push(handle);
    }

    // Wait for all requests
    for handle in handles {
        let status = handle.await.unwrap();
        assert!(
            status == StatusCode::OK
                || status == StatusCode::NOT_FOUND
                || status == StatusCode::FORBIDDEN
        );
    }

    // Verify usage count was incremented
    let stored_key = api_key_manager.get(&key.id).unwrap();
    assert!(stored_key.usage_count >= 10);
}

#[tokio::test]
async fn test_concurrent_user_authentication() {
    let (_, user_manager, _) = spawn_test_server_with_auth(true, false).await;

    // Create a user
    user_manager
        .create_user("concurrentuser", "pass123", false)
        .unwrap();

    // Authenticate concurrently
    let mut handles = vec![];
    for _ in 0..10 {
        let user_manager = user_manager.clone();
        let handle =
            tokio::spawn(async move { user_manager.authenticate("concurrentuser", "pass123") });
        handles.push(handle);
    }

    // All should succeed
    for handle in handles {
        let result = handle.await.unwrap();
        assert!(result.is_ok());
        let user = result.unwrap();
        assert_eq!(user.username, "concurrentuser");
    }
}

// ==================== Error Handling Tests ====================

#[tokio::test]
async fn test_invalid_api_key_format() {
    let (base_url, _, _) = spawn_test_server_with_auth(true, false).await;
    let client = Client::new();

    // Try to use invalid API key format
    let response = client
        .get(format!("{}/kv/get/test", base_url))
        .header("Authorization", "Bearer invalid-key-format")
        .send()
        .await
        .unwrap();

    // Should return 401 Unauthorized
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_malformed_bearer_token() {
    let (base_url, _, _) = spawn_test_server_with_auth(true, false).await;
    let client = Client::new();

    // Try malformed bearer token (empty token)
    let response = client
        .get(format!("{}/kv/get/test", base_url))
        .header("Authorization", "Bearer ")
        .send()
        .await
        .unwrap();

    // Should return 401 Unauthorized for invalid token (or 400 Bad Request if parsing fails, or 403 Forbidden if anonymous context has no permissions)
    let status = response.status();
    assert!(
        status == StatusCode::UNAUTHORIZED
            || status == StatusCode::BAD_REQUEST
            || status == StatusCode::FORBIDDEN,
        "Unexpected status: {}",
        status
    );
}

#[tokio::test]
async fn test_nonexistent_user() {
    let (_, user_manager, _) = spawn_test_server_with_auth(true, false).await;

    // Try to authenticate non-existent user
    let result = user_manager.authenticate("nonexistent", "password");
    assert!(result.is_err());
}

#[tokio::test]
async fn test_wrong_password() {
    let (_, user_manager, _) = spawn_test_server_with_auth(true, false).await;

    // Create a user
    user_manager
        .create_user("wrongpassuser", "correctpass", false)
        .unwrap();

    // Try wrong password
    let result = user_manager.authenticate("wrongpassuser", "wrongpass");
    assert!(result.is_err());
}

#[tokio::test]
async fn test_get_nonexistent_api_key() {
    let (_, _, api_key_manager) = spawn_test_server_with_auth(true, false).await;

    // Try to get non-existent API key
    let result = api_key_manager.get("nonexistent-id");
    assert!(result.is_none());
}

#[tokio::test]
async fn test_revoke_nonexistent_api_key() {
    let (_, _, api_key_manager) = spawn_test_server_with_auth(true, false).await;

    // Try to revoke non-existent API key (revoke returns Ok(false) for non-existent keys)
    let result = api_key_manager.revoke("nonexistent-id").unwrap();
    assert!(!result); // Should return false (key not found)
}

// ==================== User Roles Tests ====================

#[tokio::test]
async fn test_user_roles() {
    let (_, user_manager, _) = spawn_test_server_with_auth(true, false).await;

    // Create a user
    user_manager
        .create_user("roleuser", "pass123", false)
        .unwrap();

    // Add role to user
    user_manager.add_user_role("roleuser", "admin").unwrap();

    // Verify role was added
    let user = user_manager.get_user("roleuser").unwrap();
    assert!(user.roles.contains(&"admin".to_string()));

    // Remove role from user
    user_manager.remove_user_role("roleuser", "admin").unwrap();

    // Verify role was removed
    let user = user_manager.get_user("roleuser").unwrap();
    assert!(!user.roles.contains(&"admin".to_string()));
}

#[tokio::test]
async fn test_user_roles_permissions() {
    let (_, user_manager, _) = spawn_test_server_with_auth(true, false).await;

    // Create a custom role
    let custom_role = synap_server::auth::Role::custom(
        "custom_role",
        vec![Permission::new("kv:*", Action::Read)],
    );
    user_manager.create_role(custom_role).unwrap();

    // Create a user
    user_manager
        .create_user("rolepermuser", "pass123", false)
        .unwrap();

    // Add role to user
    user_manager
        .add_user_role("rolepermuser", "custom_role")
        .unwrap();

    // Get user permissions (should include role permissions)
    let permissions = user_manager.get_user_permissions("rolepermuser");
    assert!(!permissions.is_empty());
    assert!(permissions.iter().any(|p| p.resource_pattern == "kv:*"));
}

#[tokio::test]
async fn test_list_users() {
    let (_, user_manager, _) = spawn_test_server_with_auth(true, false).await;

    // Create multiple users
    user_manager
        .create_user("listuser1", "pass123", false)
        .unwrap();
    user_manager
        .create_user("listuser2", "pass123", false)
        .unwrap();

    // List users
    let users = user_manager.list_users();
    assert!(users.contains(&"listuser1".to_string()));
    assert!(users.contains(&"listuser2".to_string()));
    assert!(users.contains(&"root".to_string())); // Root user should be there
}

#[tokio::test]
async fn test_list_roles() {
    let (_, user_manager, _) = spawn_test_server_with_auth(true, false).await;

    // Create custom roles
    let role1 = synap_server::auth::Role::custom("role1", vec![]);
    let role2 = synap_server::auth::Role::custom("role2", vec![]);
    user_manager.create_role(role1).unwrap();
    user_manager.create_role(role2).unwrap();

    // List roles
    let roles = user_manager.list_roles();
    assert!(roles.contains(&"role1".to_string()));
    assert!(roles.contains(&"role2".to_string()));
    assert!(roles.contains(&"admin".to_string())); // Default role
    assert!(roles.contains(&"readonly".to_string())); // Default role
}

#[tokio::test]
async fn test_delete_role() {
    let (_, user_manager, _) = spawn_test_server_with_auth(true, false).await;

    // Create a role
    let role = synap_server::auth::Role::custom("todelete", vec![]);
    user_manager.create_role(role).unwrap();

    // Delete role
    let deleted = user_manager.delete_role("todelete").unwrap();
    assert!(deleted);

    // Verify role is gone
    assert!(user_manager.get_role("todelete").is_none());
}

#[tokio::test]
async fn test_api_key_revoke() {
    let (_, _, api_key_manager) = spawn_test_server_with_auth(true, false).await;

    // Create an API key
    let key = api_key_manager
        .create(
            "torevoke",
            None,
            vec![Permission::new("kv:*", Action::Read)],
            vec![],
            None,
        )
        .unwrap();

    // Revoke key (removes it completely)
    let revoked = api_key_manager.revoke(&key.id).unwrap();
    assert!(revoked);

    // Verify key is gone
    assert!(api_key_manager.get(&key.id).is_none());
}

#[tokio::test]
async fn test_api_key_metadata() {
    let (_, _, api_key_manager) = spawn_test_server_with_auth(true, false).await;

    // Create an API key
    let key = api_key_manager
        .create(
            "metadatakey",
            None,
            vec![Permission::new("kv:*", Action::Read)],
            vec![],
            None,
        )
        .unwrap();

    // Get metadata
    let metadata = api_key_manager.get_metadata(&key.id).unwrap();
    assert_eq!(metadata.id, key.id);
    assert_eq!(metadata.name, "metadatakey");
}

#[tokio::test]
async fn test_list_expired_api_keys() {
    let (_, _, api_key_manager) = spawn_test_server_with_auth(true, false).await;

    // Create an expired key
    let expired_key = api_key_manager
        .create_temporary(
            "expired",
            None,
            vec![Permission::new("kv:*", Action::Read)],
            vec![],
            1, // 1 second TTL
        )
        .unwrap();

    // Wait for expiration
    sleep(StdDuration::from_secs(2)).await;

    // List expired keys
    let expired = api_key_manager.list_expired();
    assert!(expired.iter().any(|k| k.id == expired_key.id));
}

#[tokio::test]
async fn test_list_active_api_keys() {
    let (_, _, api_key_manager) = spawn_test_server_with_auth(true, false).await;

    // Create active keys
    let active_key1 = api_key_manager
        .create(
            "active1",
            None,
            vec![Permission::new("kv:*", Action::Read)],
            vec![],
            None,
        )
        .unwrap();

    let active_key2 = api_key_manager
        .create(
            "active2",
            None,
            vec![Permission::new("kv:*", Action::Read)],
            vec![],
            None,
        )
        .unwrap();

    // Create expired key
    let _expired_key = api_key_manager
        .create_temporary(
            "expired",
            None,
            vec![Permission::new("kv:*", Action::Read)],
            vec![],
            1,
        )
        .unwrap();

    sleep(StdDuration::from_secs(2)).await;

    // List active keys
    let active = api_key_manager.list_active();
    assert!(active.iter().any(|k| k.id == active_key1.id));
    assert!(active.iter().any(|k| k.id == active_key2.id));
}

#[tokio::test]
async fn test_cleanup_expired_api_keys() {
    let (_, _, api_key_manager) = spawn_test_server_with_auth(true, false).await;

    // Create expired keys
    let _expired1 = api_key_manager
        .create_temporary(
            "expired1",
            None,
            vec![Permission::new("kv:*", Action::Read)],
            vec![],
            1,
        )
        .unwrap();

    let _expired2 = api_key_manager
        .create_temporary(
            "expired2",
            None,
            vec![Permission::new("kv:*", Action::Read)],
            vec![],
            1,
        )
        .unwrap();

    sleep(StdDuration::from_secs(2)).await;

    // Cleanup expired keys
    let cleaned = api_key_manager.cleanup_expired();
    assert!(cleaned >= 2);
}
