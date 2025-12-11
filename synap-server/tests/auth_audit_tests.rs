//! Tests for authentication audit logging

use synap_server::auth::audit::{AuditLogEntry, AuditLogManager, AuthEventType};

#[tokio::test]
async fn test_audit_log_manager_creation() {
    let manager = AuditLogManager::new(100);
    assert_eq!(manager.len().await, 0);
    assert!(manager.is_enabled());
}

#[tokio::test]
async fn test_audit_log_entry_creation() {
    let entry = AuditLogEntry::login_success("testuser".to_string(), "127.0.0.1".to_string());
    assert_eq!(entry.event_type, AuthEventType::LoginSuccess);
    assert_eq!(entry.username, Some("testuser".to_string()));
    assert!(entry.success);
}

#[tokio::test]
async fn test_audit_log_login_success() {
    let manager = AuditLogManager::new(100);
    let entry = AuditLogEntry::login_success("testuser".to_string(), "127.0.0.1".to_string());
    manager.log(entry).await;

    assert_eq!(manager.len().await, 1);
    let entries = manager
        .get_entries(None, Some(AuthEventType::LoginSuccess), None)
        .await;
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].username, Some("testuser".to_string()));
}

#[tokio::test]
async fn test_audit_log_login_failure() {
    let manager = AuditLogManager::new(100);
    let entry = AuditLogEntry::login_failure(
        Some("testuser".to_string()),
        "127.0.0.1".to_string(),
        "Invalid password".to_string(),
    );
    manager.log(entry).await;

    let failed_logins = manager.get_failed_logins(None).await;
    assert_eq!(failed_logins.len(), 1);
    assert!(!failed_logins[0].success);
}

#[tokio::test]
async fn test_audit_log_api_key_success() {
    let manager = AuditLogManager::new(100);
    let entry = AuditLogEntry::api_key_success(
        "key_123".to_string(),
        Some("testuser".to_string()),
        "127.0.0.1".to_string(),
    );
    manager.log(entry).await;

    assert_eq!(manager.len().await, 1);
    let entries = manager
        .get_entries(None, Some(AuthEventType::ApiKeySuccess), None)
        .await;
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].api_key_id, Some("key_123".to_string()));
}

#[tokio::test]
async fn test_audit_log_api_key_failure() {
    let manager = AuditLogManager::new(100);
    let entry =
        AuditLogEntry::api_key_failure("127.0.0.1".to_string(), "Invalid API key".to_string());
    manager.log(entry).await;

    let failed_keys = manager.get_failed_api_keys(None).await;
    assert_eq!(failed_keys.len(), 1);
    assert!(!failed_keys[0].success);
}

#[tokio::test]
async fn test_audit_log_permission_denied() {
    let manager = AuditLogManager::new(100);
    let entry = AuditLogEntry::permission_denied(
        Some("testuser".to_string()),
        None,
        "127.0.0.1".to_string(),
        "kv:test".to_string(),
        "write".to_string(),
    );
    manager.log(entry).await;

    let entries = manager
        .get_entries(None, Some(AuthEventType::PermissionDenied), None)
        .await;
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].resource, Some("kv:test".to_string()));
    assert_eq!(entries[0].action, Some("write".to_string()));
}

#[tokio::test]
async fn test_audit_log_filtering() {
    let manager = AuditLogManager::new(100);

    // Log multiple events
    manager
        .log(AuditLogEntry::login_success(
            "user1".to_string(),
            "127.0.0.1".to_string(),
        ))
        .await;
    manager
        .log(AuditLogEntry::login_success(
            "user2".to_string(),
            "127.0.0.2".to_string(),
        ))
        .await;
    manager
        .log(AuditLogEntry::login_failure(
            Some("user1".to_string()),
            "127.0.0.1".to_string(),
            "Invalid password".to_string(),
        ))
        .await;

    // Filter by username
    let user1_entries = manager.get_entries(None, None, Some("user1")).await;
    assert_eq!(user1_entries.len(), 2);

    // Filter by event type
    let login_success = manager
        .get_entries(None, Some(AuthEventType::LoginSuccess), None)
        .await;
    assert_eq!(login_success.len(), 2);

    // Filter by both
    let user1_success = manager
        .get_entries(None, Some(AuthEventType::LoginSuccess), Some("user1"))
        .await;
    assert_eq!(user1_success.len(), 1);
}

#[tokio::test]
async fn test_audit_log_limit() {
    let manager = AuditLogManager::new(100);

    // Log more than limit
    for i in 0..150 {
        manager
            .log(AuditLogEntry::login_success(
                format!("user{}", i),
                "127.0.0.1".to_string(),
            ))
            .await;
    }

    // Should only keep last 100 entries
    assert_eq!(manager.len().await, 100);
}

#[tokio::test]
async fn test_audit_log_disabled() {
    let mut manager = AuditLogManager::new(100);
    manager.set_enabled(false);

    manager
        .log(AuditLogEntry::login_success(
            "testuser".to_string(),
            "127.0.0.1".to_string(),
        ))
        .await;

    // Should not log when disabled
    assert_eq!(manager.len().await, 0);
}

#[tokio::test]
async fn test_audit_log_clear() {
    let manager = AuditLogManager::new(100);

    manager
        .log(AuditLogEntry::login_success(
            "testuser".to_string(),
            "127.0.0.1".to_string(),
        ))
        .await;
    assert_eq!(manager.len().await, 1);

    manager.clear().await;
    assert_eq!(manager.len().await, 0);
}
