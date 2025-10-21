pub mod acl;
pub mod api_key;
pub mod middleware;
pub mod permissions;
pub mod user;

pub use acl::{Acl, AclRule, ResourceType};
pub use api_key::{ApiKey, ApiKeyManager};
pub use middleware::AuthMiddleware;
pub use permissions::{Action, Permission, Role};
pub use user::{User, UserManager};

use crate::core::SynapError;
use std::net::IpAddr;

/// Authentication result
pub type AuthResult<T> = Result<T, SynapError>;

/// Authenticated context
#[derive(Debug, Clone)]
pub struct AuthContext {
    /// User ID (if authenticated via user/password)
    pub user_id: Option<String>,
    /// API Key ID (if authenticated via API key)
    pub api_key_id: Option<String>,
    /// Client IP address
    pub client_ip: IpAddr,
    /// User permissions
    pub permissions: Vec<Permission>,
    /// Is admin user
    pub is_admin: bool,
}

impl AuthContext {
    /// Create a new unauthenticated context
    pub fn anonymous(client_ip: IpAddr) -> Self {
        Self {
            user_id: None,
            api_key_id: None,
            client_ip,
            permissions: vec![],
            is_admin: false,
        }
    }

    /// Check if user has permission for an action on a resource
    pub fn has_permission(&self, resource: &str, action: Action) -> bool {
        // Admin has all permissions
        if self.is_admin {
            return true;
        }

        // Check explicit permissions
        self.permissions.iter().any(|p| p.matches(resource, action))
    }

    /// Check if context is authenticated
    pub fn is_authenticated(&self) -> bool {
        self.user_id.is_some() || self.api_key_id.is_some()
    }
}
