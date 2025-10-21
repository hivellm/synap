use super::{AuthResult, Permission};
use crate::core::SynapError;
use chrono::{DateTime, Duration, Utc};
use parking_lot::RwLock;
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Arc;
use tracing::{debug, info};

/// API Key for authentication
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKey {
    /// Unique key ID
    pub id: String,
    /// The actual API key (secret)
    #[serde(skip_serializing)]
    pub key: String,
    /// Human-readable name/description
    pub name: String,
    /// Associated username (if any)
    pub username: Option<String>,
    /// Permissions granted to this key
    pub permissions: Vec<Permission>,
    /// Allowed IP addresses (empty = all IPs allowed)
    pub allowed_ips: Vec<IpAddr>,
    /// Expiration time (None = never expires)
    pub expires_at: Option<DateTime<Utc>>,
    /// Key enabled/disabled
    pub enabled: bool,
    /// When key was created
    pub created_at: DateTime<Utc>,
    /// Last time this key was used
    pub last_used_at: Option<DateTime<Utc>>,
    /// Usage count
    pub usage_count: u64,
    /// Additional metadata
    pub metadata: HashMap<String, String>,
}

impl ApiKey {
    /// Generate a new API key
    pub fn generate(
        name: impl Into<String>,
        username: Option<String>,
        permissions: Vec<Permission>,
        allowed_ips: Vec<IpAddr>,
        expires_in_days: Option<i64>,
    ) -> Self {
        let key = Self::generate_key();
        let id = uuid::Uuid::new_v4().to_string();

        let expires_at = expires_in_days.map(|days| Utc::now() + Duration::days(days));

        Self {
            id,
            key,
            name: name.into(),
            username,
            permissions,
            allowed_ips,
            expires_at,
            enabled: true,
            created_at: Utc::now(),
            last_used_at: None,
            usage_count: 0,
            metadata: HashMap::new(),
        }
    }

    /// Generate a random API key string
    fn generate_key() -> String {
        const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
        const KEY_LENGTH: usize = 32;

        let mut rng = rand::thread_rng();
        let key: String = (0..KEY_LENGTH)
            .map(|_| {
                let idx = rng.gen_range(0..CHARSET.len());
                CHARSET[idx] as char
            })
            .collect();

        format!("sk_{}", key)
    }

    /// Check if key is valid (enabled and not expired)
    pub fn is_valid(&self) -> bool {
        if !self.enabled {
            return false;
        }

        if let Some(expires_at) = self.expires_at {
            if Utc::now() > expires_at {
                return false;
            }
        }

        true
    }

    /// Check if IP is allowed
    pub fn is_ip_allowed(&self, ip: IpAddr) -> bool {
        if self.allowed_ips.is_empty() {
            return true; // No IP restrictions
        }

        self.allowed_ips.contains(&ip)
    }

    /// Update last used time and increment usage count
    pub fn mark_used(&mut self) {
        self.last_used_at = Some(Utc::now());
        self.usage_count += 1;
    }

    /// Check if key has permission for an action on a resource
    pub fn has_permission(&self, resource: &str, action: super::Action) -> bool {
        self.permissions.iter().any(|p| p.matches(resource, action))
    }
}

/// API Key manager
#[derive(Clone)]
pub struct ApiKeyManager {
    keys: Arc<RwLock<HashMap<String, ApiKey>>>,
    // Index by key for fast lookup
    key_index: Arc<RwLock<HashMap<String, String>>>, // key -> id
}

impl ApiKeyManager {
    /// Create a new API key manager
    pub fn new() -> Self {
        info!("Initializing API Key Manager");
        Self {
            keys: Arc::new(RwLock::new(HashMap::new())),
            key_index: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Create a new API key
    pub fn create(
        &self,
        name: impl Into<String>,
        username: Option<String>,
        permissions: Vec<Permission>,
        allowed_ips: Vec<IpAddr>,
        expires_in_days: Option<i64>,
    ) -> AuthResult<ApiKey> {
        let api_key = ApiKey::generate(name, username, permissions, allowed_ips, expires_in_days);

        debug!("Creating API key: {} (ID: {})", api_key.name, api_key.id);

        let mut keys = self.keys.write();
        let mut index = self.key_index.write();

        // Store key
        index.insert(api_key.key.clone(), api_key.id.clone());
        keys.insert(api_key.id.clone(), api_key.clone());

        Ok(api_key)
    }

    /// Verify API key and return the key object
    pub fn verify(&self, key: &str, client_ip: IpAddr) -> AuthResult<ApiKey> {
        debug!("Verifying API key from IP: {}", client_ip);

        // Look up key ID
        let key_id = {
            let index = self.key_index.read();
            index
                .get(key)
                .ok_or_else(|| SynapError::InvalidRequest("Invalid API key".to_string()))?
                .clone()
        };

        // Get and verify key
        let mut keys = self.keys.write();
        let api_key = keys
            .get_mut(&key_id)
            .ok_or_else(|| SynapError::InvalidRequest("Invalid API key".to_string()))?;

        if !api_key.is_valid() {
            return Err(SynapError::InvalidRequest(
                "API key expired or disabled".to_string(),
            ));
        }

        if !api_key.is_ip_allowed(client_ip) {
            return Err(SynapError::InvalidRequest(format!(
                "IP {} not allowed for this API key",
                client_ip
            )));
        }

        // Mark as used
        api_key.mark_used();

        Ok(api_key.clone())
    }

    /// Get API key by ID
    pub fn get(&self, id: &str) -> Option<ApiKey> {
        self.keys.read().get(id).cloned()
    }

    /// List all API keys (excluding the secret key)
    pub fn list(&self) -> Vec<ApiKey> {
        self.keys.read().values().cloned().collect()
    }

    /// Revoke (delete) API key
    pub fn revoke(&self, id: &str) -> AuthResult<bool> {
        debug!("Revoking API key: {}", id);

        let mut keys = self.keys.write();
        if let Some(api_key) = keys.remove(id) {
            // Remove from index
            self.key_index.write().remove(&api_key.key);
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Enable/disable API key
    pub fn set_enabled(&self, id: &str, enabled: bool) -> AuthResult<()> {
        debug!("Setting API key {} enabled: {}", id, enabled);

        let mut keys = self.keys.write();
        let api_key = keys
            .get_mut(id)
            .ok_or_else(|| SynapError::KeyNotFound(format!("API key {} not found", id)))?;

        api_key.enabled = enabled;
        Ok(())
    }

    /// Clean up expired keys
    pub fn cleanup_expired(&self) -> usize {
        debug!("Cleaning up expired API keys");

        let mut keys = self.keys.write();
        let mut key_index = self.key_index.write();

        let now = Utc::now();
        let expired: Vec<String> = keys
            .iter()
            .filter(|(_, k)| {
                if let Some(expires_at) = k.expires_at {
                    now > expires_at
                } else {
                    false
                }
            })
            .map(|(id, _)| id.clone())
            .collect();

        let count = expired.len();
        for id in expired {
            if let Some(api_key) = keys.remove(&id) {
                key_index.remove(&api_key.key);
            }
        }

        if count > 0 {
            info!("Cleaned up {} expired API keys", count);
        }

        count
    }
}

impl Default for ApiKeyManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_generate_api_key() {
        let key = ApiKey::generate(
            "test-key",
            Some("user1".to_string()),
            vec![Permission::new("queue:*", super::super::Action::Read)],
            vec![],
            Some(30),
        );

        assert_eq!(key.name, "test-key");
        assert!(key.key.starts_with("sk_"));
        assert!(key.is_valid());
        assert!(key.expires_at.is_some());
    }

    #[test]
    fn test_api_key_expiration() {
        let mut key = ApiKey::generate("test", None, vec![], vec![], None);
        assert!(key.is_valid());

        // Set expiration to past
        key.expires_at = Some(Utc::now() - Duration::days(1));
        assert!(!key.is_valid());
    }

    #[test]
    fn test_api_key_ip_restriction() {
        let allowed_ip = IpAddr::from_str("192.168.1.100").unwrap();
        let other_ip = IpAddr::from_str("10.0.0.1").unwrap();

        let key = ApiKey::generate("test", None, vec![], vec![allowed_ip], None);

        assert!(key.is_ip_allowed(allowed_ip));
        assert!(!key.is_ip_allowed(other_ip));
    }

    #[test]
    fn test_api_key_manager_create() {
        let manager = ApiKeyManager::new();
        let key = manager
            .create(
                "test-key",
                Some("user1".to_string()),
                vec![],
                vec![],
                Some(30),
            )
            .unwrap();

        assert!(key.key.starts_with("sk_"));
        assert_eq!(key.name, "test-key");

        // Can retrieve by ID
        let retrieved = manager.get(&key.id).unwrap();
        assert_eq!(retrieved.id, key.id);
    }

    #[test]
    fn test_api_key_manager_verify() {
        let manager = ApiKeyManager::new();
        let key = manager.create("test", None, vec![], vec![], None).unwrap();

        let client_ip = IpAddr::from_str("127.0.0.1").unwrap();

        // Valid key
        let verified = manager.verify(&key.key, client_ip);
        assert!(verified.is_ok());

        // Invalid key
        let result = manager.verify("invalid_key", client_ip);
        assert!(result.is_err());
    }

    #[test]
    fn test_api_key_manager_revoke() {
        let manager = ApiKeyManager::new();
        let key = manager.create("test", None, vec![], vec![], None).unwrap();

        assert!(manager.get(&key.id).is_some());

        // Revoke
        let revoked = manager.revoke(&key.id).unwrap();
        assert!(revoked);

        // No longer exists
        assert!(manager.get(&key.id).is_none());
    }

    #[test]
    fn test_api_key_usage_tracking() {
        let manager = ApiKeyManager::new();
        let key = manager.create("test", None, vec![], vec![], None).unwrap();

        let client_ip = IpAddr::from_str("127.0.0.1").unwrap();

        // Verify multiple times
        for _ in 0..5 {
            manager.verify(&key.key, client_ip).unwrap();
        }

        // Check usage count
        let key_state = manager.get(&key.id).unwrap();
        assert_eq!(key_state.usage_count, 5);
        assert!(key_state.last_used_at.is_some());
    }
}
