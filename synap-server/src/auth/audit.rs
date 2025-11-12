//! Audit logging for authentication events

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Type of authentication event
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AuthEventType {
    /// User login successful
    LoginSuccess,
    /// User login failed
    LoginFailure,
    /// User logout
    Logout,
    /// API key used successfully
    ApiKeySuccess,
    /// API key validation failed
    ApiKeyFailure,
    /// User created
    UserCreated,
    /// User deleted
    UserDeleted,
    /// User enabled
    UserEnabled,
    /// User disabled
    UserDisabled,
    /// Password changed
    PasswordChanged,
    /// API key created
    ApiKeyCreated,
    /// API key revoked
    ApiKeyRevoked,
    /// Permission denied
    PermissionDenied,
}

/// Audit log entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLogEntry {
    /// Timestamp of the event
    pub timestamp: DateTime<Utc>,
    /// Type of event
    pub event_type: AuthEventType,
    /// Username (if applicable)
    pub username: Option<String>,
    /// API key ID (if applicable)
    pub api_key_id: Option<String>,
    /// Client IP address
    pub client_ip: String,
    /// Resource accessed (if applicable)
    pub resource: Option<String>,
    /// Action attempted (if applicable)
    pub action: Option<String>,
    /// Success or failure
    pub success: bool,
    /// Error message (if failure)
    pub error_message: Option<String>,
    /// Additional metadata
    pub metadata: std::collections::HashMap<String, String>,
}

impl AuditLogEntry {
    /// Create a new audit log entry
    pub fn new(
        event_type: AuthEventType,
        username: Option<String>,
        client_ip: String,
        success: bool,
    ) -> Self {
        Self {
            timestamp: Utc::now(),
            event_type,
            username,
            api_key_id: None,
            client_ip,
            resource: None,
            action: None,
            success,
            error_message: None,
            metadata: std::collections::HashMap::new(),
        }
    }

    /// Create a login success entry
    pub fn login_success(username: String, client_ip: String) -> Self {
        Self::new(AuthEventType::LoginSuccess, Some(username), client_ip, true)
    }

    /// Create a login failure entry
    pub fn login_failure(
        username: Option<String>,
        client_ip: String,
        error_message: String,
    ) -> Self {
        let mut entry = Self::new(AuthEventType::LoginFailure, username, client_ip, false);
        entry.error_message = Some(error_message);
        entry
    }

    /// Create an API key success entry
    pub fn api_key_success(
        api_key_id: String,
        username: Option<String>,
        client_ip: String,
    ) -> Self {
        let mut entry = Self::new(AuthEventType::ApiKeySuccess, username, client_ip, true);
        entry.api_key_id = Some(api_key_id);
        entry
    }

    /// Create an API key failure entry
    pub fn api_key_failure(client_ip: String, error_message: String) -> Self {
        let mut entry = Self::new(AuthEventType::ApiKeyFailure, None, client_ip, false);
        entry.error_message = Some(error_message);
        entry
    }

    /// Create a permission denied entry
    pub fn permission_denied(
        username: Option<String>,
        api_key_id: Option<String>,
        client_ip: String,
        resource: String,
        action: String,
    ) -> Self {
        let error_msg = format!("Permission denied for {} on {}", action, resource);
        let mut entry = Self::new(AuthEventType::PermissionDenied, username, client_ip, false);
        entry.api_key_id = api_key_id;
        entry.resource = Some(resource);
        entry.action = Some(action);
        entry.error_message = Some(error_msg);
        entry
    }
}

/// Audit log manager
#[derive(Clone)]
pub struct AuditLogManager {
    entries: Arc<RwLock<Vec<AuditLogEntry>>>,
    max_entries: usize,
    enabled: bool,
}

impl AuditLogManager {
    /// Create a new audit log manager
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Arc::new(RwLock::new(Vec::new())),
            max_entries,
            enabled: true,
        }
    }


    /// Enable or disable audit logging
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Check if audit logging is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Log an authentication event
    pub async fn log(&self, entry: AuditLogEntry) {
        if !self.enabled {
            return;
        }

        // Log to tracing
        match entry.event_type {
            AuthEventType::LoginSuccess => {
                info!(
                    "AUDIT: Login success - user: {}, ip: {}",
                    entry.username.as_deref().unwrap_or("unknown"),
                    entry.client_ip
                );
            }
            AuthEventType::LoginFailure => {
                warn!(
                    "AUDIT: Login failure - user: {}, ip: {}, error: {}",
                    entry.username.as_deref().unwrap_or("unknown"),
                    entry.client_ip,
                    entry.error_message.as_deref().unwrap_or("unknown")
                );
            }
            AuthEventType::ApiKeySuccess => {
                debug!(
                    "AUDIT: API key success - key_id: {}, ip: {}",
                    entry.api_key_id.as_deref().unwrap_or("unknown"),
                    entry.client_ip
                );
            }
            AuthEventType::ApiKeyFailure => {
                warn!(
                    "AUDIT: API key failure - ip: {}, error: {}",
                    entry.client_ip,
                    entry.error_message.as_deref().unwrap_or("unknown")
                );
            }
            AuthEventType::PermissionDenied => {
                warn!(
                    "AUDIT: Permission denied - user: {}, resource: {}, action: {}, ip: {}",
                    entry.username.as_deref().unwrap_or("unknown"),
                    entry.resource.as_deref().unwrap_or("unknown"),
                    entry.action.as_deref().unwrap_or("unknown"),
                    entry.client_ip
                );
            }
            _ => {
                debug!(
                    "AUDIT: {:?} - user: {:?}, ip: {}",
                    entry.event_type, entry.username, entry.client_ip
                );
            }
        }

        // Store entry
        let mut entries = self.entries.write().await;
        entries.push(entry);

        // Keep only the last N entries
        if entries.len() > self.max_entries {
            entries.remove(0);
        }
    }

    /// Get audit log entries
    pub async fn get_entries(
        &self,
        limit: Option<usize>,
        event_type: Option<AuthEventType>,
        username: Option<&str>,
    ) -> Vec<AuditLogEntry> {
        let entries = self.entries.read().await;
        let mut filtered: Vec<_> = entries.iter().cloned().collect();

        // Filter by event type
        if let Some(event_type) = event_type {
            filtered.retain(|e| e.event_type == event_type);
        }

        // Filter by username
        if let Some(username) = username {
            filtered.retain(|e| e.username.as_deref() == Some(username));
        }

        // Reverse to get most recent first
        filtered.reverse();

        // Apply limit
        if let Some(limit) = limit {
            filtered.truncate(limit);
        }

        filtered
    }

    /// Get recent failed login attempts
    pub async fn get_failed_logins(&self, limit: Option<usize>) -> Vec<AuditLogEntry> {
        self.get_entries(limit, Some(AuthEventType::LoginFailure), None)
            .await
    }

    /// Get recent failed API key attempts
    pub async fn get_failed_api_keys(&self, limit: Option<usize>) -> Vec<AuditLogEntry> {
        self.get_entries(limit, Some(AuthEventType::ApiKeyFailure), None)
            .await
    }

    /// Clear audit log
    pub async fn clear(&self) {
        let mut entries = self.entries.write().await;
        entries.clear();
    }

    /// Get number of entries
    pub async fn len(&self) -> usize {
        self.entries.read().await.len()
    }

    /// Check if audit log is empty
    pub async fn is_empty(&self) -> bool {
        self.entries.read().await.is_empty()
    }
}

impl Default for AuditLogManager {
    fn default() -> Self {
        Self::new(1000)
    }
}
