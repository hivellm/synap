// Authentication Components Integration Tests
// Tests that all auth components work together

use std::sync::Arc;
use synap_server::{Action, ApiKeyManager, KVConfig, KVStore, Permission, UserManager};

fn create_test_managers() -> (Arc<KVStore>, Arc<UserManager>, Arc<ApiKeyManager>) {
    let kv_store = Arc::new(KVStore::new(KVConfig::default()));
    let user_manager = Arc::new(UserManager::new());
    let api_key_manager = Arc::new(ApiKeyManager::new());

    // Create test users
    user_manager
        .create_user("admin", "admin12345", true)
        .unwrap();
    user_manager
        .create_user("user1", "pass12345", false)
        .unwrap();
    user_manager.add_user_role("user1", "readonly").unwrap();

    // Create test API key
    api_key_manager
        .create(
            "test-key",
            Some("user1".to_string()),
            vec![Permission::new("*", Action::All)],
            vec![],
            None,
        )
        .unwrap();

    (kv_store, user_manager, api_key_manager)
}

// Note: Full middleware integration testing will be added when auth is integrated into router
// For now, testing auth system components directly

#[test]
fn test_user_manager_integration() {
    let (_, user_manager, _) = create_test_managers();

    // Test user was created
    assert!(user_manager.get_user("admin").is_some());
    assert!(user_manager.get_user("user1").is_some());

    // Test authentication
    assert!(user_manager.authenticate("admin", "admin12345").is_ok());
    assert!(user_manager.authenticate("user1", "pass12345").is_ok());
}

#[test]
fn test_api_key_manager_integration() {
    let (_, _, api_key_manager) = create_test_managers();

    let keys = api_key_manager.list();
    assert_eq!(keys.len(), 1);
    assert_eq!(keys[0].name, "test-key");
}

#[test]
fn test_auth_components_ready_for_integration() {
    // This test verifies all auth components are instantiable
    let user_manager = UserManager::new();
    let api_key_manager = ApiKeyManager::new();

    user_manager
        .create_user("test", "pass12345", false)
        .unwrap();
    let key = api_key_manager
        .create("key", None, vec![], vec![], None)
        .unwrap();

    assert!(user_manager.authenticate("test", "pass12345").is_ok());
    assert!(key.key.starts_with("sk_"));
}
