use super::{Action, AuthContext, AuthResult};
use crate::core::SynapError;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::debug;

/// Resource type
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ResourceType {
    Queue,
    KV,
    Stream,
    PubSub,
    Admin,
}

/// ACL rule for resource access
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AclRule {
    /// Resource type
    pub resource_type: ResourceType,
    /// Resource name/pattern
    pub resource_name: String,
    /// Allowed actions
    pub allowed_actions: Vec<Action>,
    /// Users allowed (empty = all authenticated users)
    pub allowed_users: Vec<String>,
    /// Roles allowed (empty = all roles)
    pub allowed_roles: Vec<String>,
    /// Required permission level
    pub require_auth: bool,
}

impl AclRule {
    /// Create a public rule (no auth required)
    pub fn public(resource_type: ResourceType, resource_name: impl Into<String>) -> Self {
        Self {
            resource_type,
            resource_name: resource_name.into(),
            allowed_actions: vec![Action::Read, Action::Write],
            allowed_users: vec![],
            allowed_roles: vec![],
            require_auth: false,
        }
    }

    /// Create an authenticated-only rule
    pub fn authenticated(
        resource_type: ResourceType,
        resource_name: impl Into<String>,
        actions: Vec<Action>,
    ) -> Self {
        Self {
            resource_type,
            resource_name: resource_name.into(),
            allowed_actions: actions,
            allowed_users: vec![],
            allowed_roles: vec![],
            require_auth: true,
        }
    }

    /// Check if context has access
    pub fn check_access(&self, ctx: &AuthContext, action: Action) -> bool {
        // If no auth required, allow
        if !self.require_auth {
            return true;
        }

        // Admin always has access (check BEFORE action validation)
        if ctx.is_admin {
            return true;
        }

        // Check if action is allowed
        if !self.allowed_actions.contains(&action) && !self.allowed_actions.contains(&Action::All) {
            return false;
        }

        // Check if authenticated
        if !ctx.is_authenticated() {
            return false;
        }

        // If specific users are listed, check membership
        if !self.allowed_users.is_empty() {
            if let Some(user_id) = &ctx.user_id {
                if !self.allowed_users.contains(user_id) {
                    return false;
                }
            } else {
                return false;
            }
        }

        // Check user permissions
        let resource_path = format!(
            "{}:{}",
            match self.resource_type {
                ResourceType::Queue => "queue",
                ResourceType::KV => "kv",
                ResourceType::Stream => "stream",
                ResourceType::PubSub => "pubsub",
                ResourceType::Admin => "admin",
            },
            self.resource_name
        );

        ctx.has_permission(&resource_path, action)
    }
}

/// Access Control List manager
#[derive(Clone)]
pub struct Acl {
    rules: Arc<RwLock<HashMap<String, AclRule>>>,
}

impl Acl {
    /// Create a new ACL
    pub fn new() -> Self {
        Self {
            rules: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Add ACL rule
    pub fn add_rule(&self, key: impl Into<String>, rule: AclRule) {
        let key_string = key.into();
        debug!("Adding ACL rule: {}", key_string);
        self.rules.write().insert(key_string, rule);
    }

    /// Remove ACL rule
    pub fn remove_rule(&self, key: &str) -> bool {
        debug!("Removing ACL rule: {}", key);
        self.rules.write().remove(key).is_some()
    }

    /// Check access to a resource
    pub fn check_access(
        &self,
        resource_type: ResourceType,
        resource_name: &str,
        action: Action,
        ctx: &AuthContext,
    ) -> AuthResult<()> {
        let key = format!(
            "{}:{}",
            match resource_type {
                ResourceType::Queue => "queue",
                ResourceType::KV => "kv",
                ResourceType::Stream => "stream",
                ResourceType::PubSub => "pubsub",
                ResourceType::Admin => "admin",
            },
            resource_name
        );

        let rules = self.rules.read();

        // Check specific rule
        if let Some(rule) = rules.get(&key) {
            if rule.check_access(ctx, action) {
                return Ok(());
            }
        }

        // Check wildcard rule
        let wildcard_key = format!(
            "{}:*",
            match resource_type {
                ResourceType::Queue => "queue",
                ResourceType::KV => "kv",
                ResourceType::Stream => "stream",
                ResourceType::PubSub => "pubsub",
                ResourceType::Admin => "admin",
            }
        );

        if let Some(rule) = rules.get(&wildcard_key) {
            if rule.check_access(ctx, action) {
                return Ok(());
            }
        }

        // Default deny if no rules match
        Err(SynapError::InvalidRequest("Access denied".to_string()))
    }

    /// List all rules
    pub fn list_rules(&self) -> Vec<(String, AclRule)> {
        self.rules.read().clone().into_iter().collect()
    }
}

impl Default for Acl {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::IpAddr;
    use std::str::FromStr;

    #[test]
    fn test_public_rule() {
        let rule = AclRule::public(ResourceType::Queue, "public_queue");
        let ctx = AuthContext::anonymous(IpAddr::from_str("127.0.0.1").unwrap());

        assert!(rule.check_access(&ctx, Action::Read));
        assert!(rule.check_access(&ctx, Action::Write));
    }

    #[test]
    fn test_authenticated_rule() {
        let rule = AclRule::authenticated(
            ResourceType::Queue,
            "private_queue",
            vec![Action::Read, Action::Write],
        );

        let anon_ctx = AuthContext::anonymous(IpAddr::from_str("127.0.0.1").unwrap());
        assert!(!rule.check_access(&anon_ctx, Action::Read));

        let mut auth_ctx = anon_ctx.clone();
        auth_ctx.user_id = Some("user1".to_string());
        auth_ctx.permissions = vec![super::super::Permission::new("queue:*", Action::All)];

        assert!(rule.check_access(&auth_ctx, Action::Read));
    }

    #[test]
    fn test_acl_check_access() {
        let acl = Acl::new();

        // Add authenticated rule
        acl.add_rule(
            "queue:orders",
            AclRule::authenticated(
                ResourceType::Queue,
                "orders",
                vec![Action::Read, Action::Write],
            ),
        );

        let anon_ctx = AuthContext::anonymous(IpAddr::from_str("127.0.0.1").unwrap());
        let result = acl.check_access(ResourceType::Queue, "orders", Action::Read, &anon_ctx);
        assert!(result.is_err());

        let mut auth_ctx = anon_ctx;
        auth_ctx.user_id = Some("user1".to_string());
        auth_ctx.permissions = vec![super::super::Permission::new("queue:*", Action::All)];

        let result = acl.check_access(ResourceType::Queue, "orders", Action::Read, &auth_ctx);
        assert!(result.is_ok());
    }
}
