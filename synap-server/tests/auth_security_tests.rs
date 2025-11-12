// Comprehensive Authentication & Security Tests
// Tests authentication, authorization, API keys, and security scenarios

use std::net::IpAddr;
use synap_server::{
    Acl, AclRule, Action, ApiKeyManager, AuthContext, Permission, ResourceType, Role, UserManager,
};

// ==================== USER AUTHENTICATION TESTS ====================

#[test]
fn test_user_creation_and_authentication() {
    let manager = UserManager::new();

    // Create user
    manager
        .create_user("testuser", "P@ssw0rd123", false)
        .unwrap();

    // Valid credentials
    let user = manager.authenticate("testuser", "P@ssw0rd123").unwrap();
    assert_eq!(user.username, "testuser");
    assert!(!user.is_admin);
    assert!(user.enabled);
}

#[test]
fn test_authentication_fails_invalid_password() {
    let manager = UserManager::new();
    manager.create_user("user1", "correct_pass", false).unwrap();

    // Wrong password
    let result = manager.authenticate("user1", "wrong_pass");
    assert!(result.is_err());
}

#[test]
fn test_authentication_fails_nonexistent_user() {
    let manager = UserManager::new();

    let result = manager.authenticate("nonexistent", "anypass");
    assert!(result.is_err());
}

#[test]
fn test_disabled_user_cannot_authenticate() {
    let manager = UserManager::new();
    manager.create_user("user1", "password12345", false).unwrap();

    // Can authenticate when enabled
    assert!(manager.authenticate("user1", "password12345").is_ok());

    // Disable user
    manager.set_user_enabled("user1", false).unwrap();

    // Cannot authenticate when disabled
    let result = manager.authenticate("user1", "password12345");
    assert!(result.is_err());
}

#[test]
fn test_password_change() {
    let manager = UserManager::new();
    manager.create_user("user1", "oldpass123", false).unwrap();

    // Can authenticate with old password
    assert!(manager.authenticate("user1", "oldpass123").is_ok());

    // Change password
    manager.change_password("user1", "newpass123").unwrap();

    // Cannot authenticate with old password
    assert!(manager.authenticate("user1", "oldpass123").is_err());

    // Can authenticate with new password
    assert!(manager.authenticate("user1", "newpass123").is_ok());
}

#[test]
fn test_last_login_tracking() {
    let manager = UserManager::new();
    manager.create_user("user1", "pass12345", false).unwrap();

    // Initially no last login
    let user = manager.get_user("user1").unwrap();
    assert!(user.last_login.is_none());

    // After authentication, last login is set
    manager.authenticate("user1", "pass12345").unwrap();
    let user = manager.get_user("user1").unwrap();
    assert!(user.last_login.is_some());
}

#[test]
fn test_duplicate_user_creation_fails() {
    let manager = UserManager::new();
    manager.create_user("user1", "pass12345", false).unwrap();

    // Try to create duplicate
    let result = manager.create_user("user1", "pass23456", false);
    assert!(result.is_err());
}

#[test]
fn test_admin_user_flag() {
    let manager = UserManager::new();

    // Create admin
    manager.create_user("admin", "adminpass123", true).unwrap();

    // Create regular user
    manager.create_user("regular", "userpass123", false).unwrap();

    let admin = manager.get_user("admin").unwrap();
    assert!(admin.is_admin);

    let regular = manager.get_user("regular").unwrap();
    assert!(!regular.is_admin);
}

// ==================== ROLE & PERMISSION TESTS ====================

#[test]
fn test_role_assignment() {
    let manager = UserManager::new();
    manager.create_user("user1", "pass12345", false).unwrap();

    // Add role
    manager.add_user_role("user1", "readonly").unwrap();

    let user = manager.get_user("user1").unwrap();
    assert!(user.roles.contains(&"readonly".to_string()));

    // Remove role
    manager.remove_user_role("user1", "readonly").unwrap();

    let user = manager.get_user("user1").unwrap();
    assert!(!user.roles.contains(&"readonly".to_string()));
}

#[test]
fn test_add_nonexistent_role_fails() {
    let manager = UserManager::new();
    manager.create_user("user1", "pass12345", false).unwrap();

    let result = manager.add_user_role("user1", "nonexistent_role");
    assert!(result.is_err());
}

#[test]
fn test_custom_role_creation() {
    let manager = UserManager::new();

    let role = Role::custom(
        "queue_admin",
        vec![
            Permission::new("queue:*", Action::All),
            Permission::new("kv:*", Action::Read),
        ],
    );

    manager.create_role(role).unwrap();

    let retrieved = manager.get_role("queue_admin").unwrap();
    assert_eq!(retrieved.name, "queue_admin");
    assert_eq!(retrieved.permissions.len(), 2);
}

#[test]
fn test_admin_user_has_all_permissions() {
    let manager = UserManager::new();
    manager.create_user("admin", "pass12345", true).unwrap();
    manager.add_user_role("admin", "admin").unwrap();

    let permissions = manager.get_user_permissions("admin");

    // Admin should have wildcard permission
    assert!(permissions.iter().any(|p| p.resource_pattern == "*"));
}

#[test]
fn test_permission_pattern_matching() {
    // Exact match
    let perm = Permission::new("queue:orders", Action::Read);
    assert!(perm.matches("queue:orders", Action::Read));
    assert!(!perm.matches("queue:payments", Action::Read));

    // Wildcard
    let perm = Permission::new("*", Action::All);
    assert!(perm.matches("queue:anything", Action::Write));
    assert!(perm.matches("kv:any:key", Action::Delete));

    // Prefix wildcard
    let perm = Permission::new("queue:*", Action::Write);
    assert!(perm.matches("queue:orders", Action::Write));
    assert!(perm.matches("queue:payments", Action::Write));
    assert!(!perm.matches("kv:data", Action::Write));
}

// ==================== API KEY TESTS ====================

#[test]
fn test_api_key_generation() {
    let manager = ApiKeyManager::new();

    let key = manager
        .create("test-key", Some("user1".to_string()), vec![], vec![], None)
        .unwrap();

    assert!(key.key.starts_with("sk_"));
    assert_eq!(key.key.len(), 35); // "sk_" + 32 chars
    assert_eq!(key.name, "test-key");
    assert!(key.enabled);
}

#[test]
fn test_api_key_verification() {
    let manager = ApiKeyManager::new();
    let client_ip = IpAddr::from([127, 0, 0, 1]);

    let key = manager.create("test", None, vec![], vec![], None).unwrap();

    // Valid key
    let verified = manager.verify(&key.key, client_ip);
    assert!(verified.is_ok());

    // Invalid key
    let result = manager.verify("sk_invalid", client_ip);
    assert!(result.is_err());
}

#[test]
fn test_api_key_expiration() {
    let manager = ApiKeyManager::new();

    // Create key that expires in 30 days
    let key = manager
        .create("expiring", None, vec![], vec![], Some(30))
        .unwrap();
    assert!(key.expires_at.is_some());
    assert!(key.is_valid());

    // Create key that never expires
    let key = manager
        .create("permanent", None, vec![], vec![], None)
        .unwrap();
    assert!(key.expires_at.is_none());
    assert!(key.is_valid());
}

#[test]
fn test_api_key_ip_filtering() {
    let manager = ApiKeyManager::new();

    let allowed_ip = IpAddr::from([192, 168, 1, 100]);
    let blocked_ip = IpAddr::from([10, 0, 0, 1]);

    // Create key with IP restriction
    let key = manager
        .create("restricted", None, vec![], vec![allowed_ip], None)
        .unwrap();

    // Allowed IP
    let result = manager.verify(&key.key, allowed_ip);
    assert!(result.is_ok());

    // Blocked IP
    let result = manager.verify(&key.key, blocked_ip);
    assert!(result.is_err());
}

#[test]
fn test_api_key_no_ip_restriction() {
    let manager = ApiKeyManager::new();

    // No IP restrictions
    let key = manager
        .create("unrestricted", None, vec![], vec![], None)
        .unwrap();

    // Any IP should work
    assert!(
        manager
            .verify(&key.key, IpAddr::from([192, 168, 1, 1]))
            .is_ok()
    );
    assert!(
        manager
            .verify(&key.key, IpAddr::from([10, 0, 0, 1]))
            .is_ok()
    );
    assert!(
        manager
            .verify(&key.key, IpAddr::from([172, 16, 0, 1]))
            .is_ok()
    );
}

#[test]
fn test_api_key_usage_tracking() {
    let manager = ApiKeyManager::new();
    let client_ip = IpAddr::from([127, 0, 0, 1]);

    let key = manager
        .create("tracked", None, vec![], vec![], None)
        .unwrap();

    // Initial state
    assert_eq!(key.usage_count, 0);
    assert!(key.last_used_at.is_none());

    // Verify multiple times
    for _ in 0..10 {
        manager.verify(&key.key, client_ip).unwrap();
    }

    // Check usage tracking
    let updated_key = manager.get(&key.id).unwrap();
    assert_eq!(updated_key.usage_count, 10);
    assert!(updated_key.last_used_at.is_some());
}

#[test]
fn test_api_key_disable_enable() {
    let manager = ApiKeyManager::new();
    let client_ip = IpAddr::from([127, 0, 0, 1]);

    let key = manager
        .create("toggleable", None, vec![], vec![], None)
        .unwrap();

    // Initially enabled, can verify
    assert!(manager.verify(&key.key, client_ip).is_ok());

    // Disable
    manager.set_enabled(&key.id, false).unwrap();

    // Cannot verify when disabled
    let result = manager.verify(&key.key, client_ip);
    assert!(result.is_err());

    // Re-enable
    manager.set_enabled(&key.id, true).unwrap();

    // Can verify again
    assert!(manager.verify(&key.key, client_ip).is_ok());
}

#[test]
fn test_api_key_revocation() {
    let manager = ApiKeyManager::new();
    let client_ip = IpAddr::from([127, 0, 0, 1]);

    let key = manager
        .create("revokable", None, vec![], vec![], None)
        .unwrap();

    // Can verify before revocation
    assert!(manager.verify(&key.key, client_ip).is_ok());

    // Revoke
    let revoked = manager.revoke(&key.id).unwrap();
    assert!(revoked);

    // Cannot verify after revocation
    assert!(manager.verify(&key.key, client_ip).is_err());

    // Key no longer exists
    assert!(manager.get(&key.id).is_none());
}

#[test]
fn test_api_key_permissions() {
    let manager = ApiKeyManager::new();

    let permissions = vec![
        Permission::new("queue:orders", Action::Read),
        Permission::new("queue:payments", Action::Write),
        Permission::new("kv:*", Action::Read),
    ];

    let key = manager
        .create("permissioned", None, permissions, vec![], None)
        .unwrap();

    // Check has_permission
    assert!(key.has_permission("queue:orders", Action::Read));
    assert!(key.has_permission("queue:payments", Action::Write));
    assert!(key.has_permission("kv:users:123", Action::Read));

    // Should not have these permissions
    assert!(!key.has_permission("queue:orders", Action::Write));
    assert!(!key.has_permission("kv:data", Action::Write));
}

#[test]
fn test_api_key_list() {
    let manager = ApiKeyManager::new();

    // Create multiple keys
    manager.create("key1", None, vec![], vec![], None).unwrap();
    manager.create("key2", None, vec![], vec![], None).unwrap();
    manager.create("key3", None, vec![], vec![], None).unwrap();

    let keys = manager.list();
    assert_eq!(keys.len(), 3);

    // Verify names
    let names: Vec<String> = keys.iter().map(|k| k.name.clone()).collect();
    assert!(names.contains(&"key1".to_string()));
    assert!(names.contains(&"key2".to_string()));
    assert!(names.contains(&"key3".to_string()));
}

// ==================== ACL TESTS ====================

#[test]
fn test_acl_public_resource() {
    let acl = Acl::new();

    // Add public rule
    acl.add_rule(
        "queue:public",
        AclRule::public(ResourceType::Queue, "public"),
    );

    // Anonymous user can access
    let anon_ctx = AuthContext::anonymous(IpAddr::from([127, 0, 0, 1]));
    let result = acl.check_access(ResourceType::Queue, "public", Action::Read, &anon_ctx);
    assert!(result.is_ok());

    let result = acl.check_access(ResourceType::Queue, "public", Action::Write, &anon_ctx);
    assert!(result.is_ok());
}

#[test]
fn test_acl_authenticated_resource() {
    let acl = Acl::new();

    acl.add_rule(
        "queue:private",
        AclRule::authenticated(ResourceType::Queue, "private", vec![Action::Read]),
    );

    let anon_ctx = AuthContext::anonymous(IpAddr::from([127, 0, 0, 1]));

    // Anonymous cannot access
    let result = acl.check_access(ResourceType::Queue, "private", Action::Read, &anon_ctx);
    assert!(result.is_err());

    // Authenticated user with permission can access
    let mut auth_ctx = anon_ctx;
    auth_ctx.user_id = Some("user1".to_string());
    auth_ctx.permissions = vec![Permission::new("queue:*", Action::All)];

    let result = acl.check_access(ResourceType::Queue, "private", Action::Read, &auth_ctx);
    assert!(result.is_ok());
}

#[test]
fn test_acl_admin_always_has_access() {
    let acl = Acl::new();

    acl.add_rule(
        "queue:restricted",
        AclRule::authenticated(ResourceType::Queue, "restricted", vec![Action::Read]),
    );

    let mut admin_ctx = AuthContext::anonymous(IpAddr::from([127, 0, 0, 1]));
    admin_ctx.user_id = Some("admin".to_string());
    admin_ctx.is_admin = true;
    admin_ctx.permissions = vec![Permission::new("*", Action::All)]; // Admin has all permissions

    // Admin can access even restricted resources
    let result = acl.check_access(ResourceType::Queue, "restricted", Action::Read, &admin_ctx);
    assert!(result.is_ok());

    let result = acl.check_access(ResourceType::Queue, "restricted", Action::Write, &admin_ctx);
    assert!(result.is_ok());
}

#[test]
fn test_acl_wildcard_rule() {
    let acl = Acl::new();

    // Wildcard for all queues
    acl.add_rule(
        "queue:*",
        AclRule::authenticated(ResourceType::Queue, "*", vec![Action::Read]),
    );

    let mut auth_ctx = AuthContext::anonymous(IpAddr::from([127, 0, 0, 1]));
    auth_ctx.user_id = Some("user1".to_string());
    auth_ctx.permissions = vec![Permission::new("queue:*", Action::Read)];

    // Can access any queue
    assert!(
        acl.check_access(ResourceType::Queue, "orders", Action::Read, &auth_ctx)
            .is_ok()
    );
    assert!(
        acl.check_access(ResourceType::Queue, "payments", Action::Read, &auth_ctx)
            .is_ok()
    );
    assert!(
        acl.check_access(ResourceType::Queue, "anything", Action::Read, &auth_ctx)
            .is_ok()
    );
}

#[test]
fn test_acl_default_deny() {
    let acl = Acl::new();

    // No rules defined
    let auth_ctx = AuthContext {
        user_id: Some("user1".to_string()),
        api_key_id: None,
        client_ip: IpAddr::from([127, 0, 0, 1]),
        permissions: vec![],
        is_admin: false,
    };

    // Should deny access by default
    let result = acl.check_access(ResourceType::Queue, "undefined", Action::Read, &auth_ctx);
    assert!(result.is_err());
}

#[test]
fn test_acl_specific_rule_overrides_wildcard() {
    let acl = Acl::new();

    // Wildcard: read-only
    acl.add_rule(
        "queue:*",
        AclRule::authenticated(ResourceType::Queue, "*", vec![Action::Read]),
    );

    // Specific: read + write
    acl.add_rule(
        "queue:special",
        AclRule::authenticated(
            ResourceType::Queue,
            "special",
            vec![Action::Read, Action::Write],
        ),
    );

    let mut auth_ctx = AuthContext::anonymous(IpAddr::from([127, 0, 0, 1]));
    auth_ctx.user_id = Some("user1".to_string());
    auth_ctx.permissions = vec![Permission::new("queue:*", Action::All)];

    // Specific rule allows write
    assert!(
        acl.check_access(ResourceType::Queue, "special", Action::Write, &auth_ctx)
            .is_ok()
    );

    // Wildcard only allows read
    assert!(
        acl.check_access(ResourceType::Queue, "other", Action::Read, &auth_ctx)
            .is_ok()
    );
}

// ==================== SECURITY EDGE CASES ====================

#[test]
fn test_empty_password_rejected() {
    let manager = UserManager::new();

    // Empty password should be rejected (minimum 8 characters)
    let result = manager.create_user("user1", "", false);
    assert!(result.is_err());

    // But authentication should fail with wrong password
    let result = manager.authenticate("user1", "anypass");
    assert!(result.is_err());
}

#[test]
fn test_special_characters_in_username() {
    let manager = UserManager::new();

    // Special characters should be allowed
    manager
        .create_user("user@example.com", "pass12345", false)
        .unwrap();
    manager.create_user("user-123_test", "pass12345", false).unwrap();

    assert!(manager.get_user("user@example.com").is_some());
    assert!(manager.get_user("user-123_test").is_some());
}

#[test]
fn test_case_sensitive_usernames() {
    let manager = UserManager::new();

    manager.create_user("User", "pass12345", false).unwrap();
    manager.create_user("user", "pass23456", false).unwrap();

    // Should be different users
    assert!(manager.authenticate("User", "pass12345").is_ok());
    assert!(manager.authenticate("user", "pass23456").is_ok());
    assert!(manager.authenticate("User", "pass23456").is_err());
    assert!(manager.authenticate("user", "pass12345").is_err());
}

#[test]
fn test_concurrent_authentication() {
    use std::sync::Arc;
    use std::thread;

    let manager = Arc::new(UserManager::new());
    manager.create_user("user1", "password12345", false).unwrap();

    let mut handles = vec![];

    // Spawn 10 threads authenticating concurrently
    for _ in 0..10 {
        let manager_clone = Arc::clone(&manager);
        let handle = thread::spawn(move || {
            manager_clone.authenticate("user1", "password12345").unwrap();
        });
        handles.push(handle);
    }

    // All should succeed
    for handle in handles {
        handle.join().unwrap();
    }
}

#[test]
fn test_api_key_uniqueness() {
    let manager = ApiKeyManager::new();

    let mut keys = vec![];
    for i in 0..100 {
        let key = manager
            .create(format!("key-{}", i), None, vec![], vec![], None)
            .unwrap();
        keys.push(key.key.clone());
    }

    // All keys should be unique
    let unique_count: std::collections::HashSet<_> = keys.iter().collect();
    assert_eq!(unique_count.len(), 100);
}

#[test]
fn test_auth_context_permission_check() {
    let ctx = AuthContext {
        user_id: Some("user1".to_string()),
        api_key_id: None,
        client_ip: IpAddr::from([127, 0, 0, 1]),
        permissions: vec![
            Permission::new("queue:orders", Action::Read),
            Permission::new("queue:*", Action::Write),
            Permission::new("kv:*", Action::All),
        ],
        is_admin: false,
    };

    // Has explicit read permission
    assert!(ctx.has_permission("queue:orders", Action::Read));

    // Has wildcard write permission
    assert!(ctx.has_permission("queue:payments", Action::Write));

    // Has all permissions on KV
    assert!(ctx.has_permission("kv:any:key", Action::Delete));

    // Doesn't have this permission
    assert!(!ctx.has_permission("queue:orders", Action::Delete));
}

#[test]
fn test_admin_bypasses_all_checks() {
    let ctx = AuthContext {
        user_id: Some("admin".to_string()),
        api_key_id: None,
        client_ip: IpAddr::from([127, 0, 0, 1]),
        permissions: vec![], // No explicit permissions
        is_admin: true,
    };

    // Admin has access to everything
    assert!(ctx.has_permission("queue:anything", Action::All));
    assert!(ctx.has_permission("kv:anything", Action::Delete));
    assert!(ctx.has_permission("admin:config", Action::Admin));
}

#[test]
fn test_user_deletion() {
    let manager = UserManager::new();
    manager.create_user("temp_user", "pass12345", false).unwrap();

    assert!(manager.get_user("temp_user").is_some());

    // Delete user
    let deleted = manager.delete_user("temp_user").unwrap();
    assert!(deleted);

    // User no longer exists
    assert!(manager.get_user("temp_user").is_none());

    // Deleting again returns false
    let deleted = manager.delete_user("temp_user").unwrap();
    assert!(!deleted);
}

#[test]
fn test_role_deletion() {
    let manager = UserManager::new();

    let role = Role::custom(
        "custom_role",
        vec![Permission::new("queue:*", Action::Read)],
    );
    manager.create_role(role).unwrap();

    assert!(manager.get_role("custom_role").is_some());

    // Delete role
    let deleted = manager.delete_role("custom_role").unwrap();
    assert!(deleted);

    // Role no longer exists
    assert!(manager.get_role("custom_role").is_none());
}
