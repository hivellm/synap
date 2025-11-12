use serde::{Deserialize, Serialize};

/// Permission actions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Action {
    /// Read operations (GET, CONSUME, SUBSCRIBE, etc.)
    Read,
    /// Write operations (SET, PUBLISH, ADD, etc.)
    Write,
    /// Delete operations (DEL, REMOVE, POP, etc.)
    Delete,
    /// Configuration operations (CREATE, UPDATE, CONFIG)
    Configure,
    /// Administrative operations (USER_MANAGE, SYSTEM_CONFIG)
    Admin,
    /// All actions (wildcard)
    All,
}

impl Action {
    /// Check if this action includes another action
    pub fn includes(&self, other: Action) -> bool {
        match self {
            Action::All => true,
            Action::Admin => matches!(other, Action::Admin),
            Action::Configure => matches!(other, Action::Configure | Action::Read | Action::Write),
            _ => self == &other,
        }
    }

    /// Get action name as string
    pub fn as_str(&self) -> &'static str {
        match self {
            Action::Read => "read",
            Action::Write => "write",
            Action::Delete => "delete",
            Action::Configure => "configure",
            Action::Admin => "admin",
            Action::All => "all",
        }
    }
}

/// User role
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Role {
    pub name: String,
    pub permissions: Vec<Permission>,
}

impl Role {
    /// Create admin role with all permissions
    pub fn admin() -> Self {
        Self {
            name: "admin".to_string(),
            permissions: vec![Permission::new("*", Action::All)],
        }
    }

    /// Create read-only role
    pub fn readonly() -> Self {
        Self {
            name: "readonly".to_string(),
            permissions: vec![Permission::new("*", Action::Read)],
        }
    }

    /// Create custom role
    pub fn custom(name: impl Into<String>, permissions: Vec<Permission>) -> Self {
        Self {
            name: name.into(),
            permissions,
        }
    }

    /// Create queue manager role (read/write/configure on queues)
    pub fn queue_manager() -> Self {
        Self {
            name: "queue_manager".to_string(),
            permissions: vec![
                Permission::new("queue:*", Action::Read),
                Permission::new("queue:*", Action::Write),
                Permission::new("queue:*", Action::Configure),
            ],
        }
    }

    /// Create stream manager role (read/write/configure on streams)
    pub fn stream_manager() -> Self {
        Self {
            name: "stream_manager".to_string(),
            permissions: vec![
                Permission::new("stream:*", Action::Read),
                Permission::new("stream:*", Action::Write),
                Permission::new("stream:*", Action::Configure),
            ],
        }
    }

    /// Create data manager role (read/write on all data structures)
    pub fn data_manager() -> Self {
        Self {
            name: "data_manager".to_string(),
            permissions: vec![
                Permission::new("kv:*", Action::Read),
                Permission::new("kv:*", Action::Write),
                Permission::new("hash:*", Action::Read),
                Permission::new("hash:*", Action::Write),
                Permission::new("list:*", Action::Read),
                Permission::new("list:*", Action::Write),
                Permission::new("set:*", Action::Read),
                Permission::new("set:*", Action::Write),
                Permission::new("sortedset:*", Action::Read),
                Permission::new("sortedset:*", Action::Write),
            ],
        }
    }
}

/// Permission for a specific resource and action
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Permission {
    /// Resource pattern (supports wildcards)
    /// Examples:
    /// - "queue:*" - All queues
    /// - "queue:orders" - Specific queue
    /// - "kv:users:*" - All keys starting with "users:"
    pub resource_pattern: String,
    /// Action allowed on this resource
    pub action: Action,
}

impl Permission {
    /// Create a new permission
    pub fn new(resource_pattern: impl Into<String>, action: Action) -> Self {
        Self {
            resource_pattern: resource_pattern.into(),
            action,
        }
    }

    /// Check if this permission matches a resource and action
    pub fn matches(&self, resource: &str, action: Action) -> bool {
        if !self.action.includes(action) {
            return false;
        }

        self.matches_pattern(resource)
    }

    /// Check if resource matches the pattern
    fn matches_pattern(&self, resource: &str) -> bool {
        // Exact match
        if self.resource_pattern == resource {
            return true;
        }

        // Wildcard match
        if self.resource_pattern == "*" {
            return true;
        }

        // Pattern with wildcard suffix (e.g., "queue:*")
        if self.resource_pattern.ends_with('*') {
            let prefix = &self.resource_pattern[..self.resource_pattern.len() - 1];
            return resource.starts_with(prefix);
        }

        // Pattern with wildcard prefix (e.g., "*:orders")
        if self.resource_pattern.starts_with('*') {
            let suffix = &self.resource_pattern[1..];
            return resource.ends_with(suffix);
        }

        // Pattern with middle wildcard (e.g., "queue:*:orders")
        if self.resource_pattern.contains(":*:") {
            let parts: Vec<&str> = self.resource_pattern.split(":*:").collect();
            if parts.len() == 2 {
                let prefix = parts[0];
                let suffix = parts[1];
                // Resource must start with prefix and end with suffix
                if resource.starts_with(prefix) && resource.ends_with(suffix) {
                    // Extract the part after prefix and before suffix
                    let prefix_with_colon = format!("{}:", prefix);
                    if resource.starts_with(&prefix_with_colon) {
                        let after_prefix = &resource[prefix_with_colon.len()..];
                        // Must have at least one ':' before the suffix
                        if let Some(_suffix_start) = after_prefix.rfind(&format!(":{}", suffix)) {
                            // There's a ':' before the suffix, so it matches
                            return true;
                        }
                    }
                }
                return false;
            }
        }

        false
    }

    /// Get resource type from resource string (e.g., "queue:orders" -> "queue")
    pub fn get_resource_type(resource: &str) -> Option<&str> {
        resource.split(':').next()
    }

    /// Get resource name from resource string (e.g., "queue:orders" -> "orders")
    pub fn get_resource_name(resource: &str) -> Option<&str> {
        resource.split(':').nth(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_action_includes() {
        assert!(Action::All.includes(Action::Read));
        assert!(Action::All.includes(Action::Write));
        assert!(Action::All.includes(Action::Delete));
        assert!(Action::All.includes(Action::Admin));

        assert!(!Action::Read.includes(Action::Write));
        assert!(Action::Read.includes(Action::Read));
    }

    #[test]
    fn test_permission_exact_match() {
        let perm = Permission::new("queue:orders", Action::Read);
        assert!(perm.matches("queue:orders", Action::Read));
        assert!(!perm.matches("queue:orders", Action::Write));
        assert!(!perm.matches("queue:payments", Action::Read));
    }

    #[test]
    fn test_permission_wildcard() {
        let perm = Permission::new("*", Action::Read);
        assert!(perm.matches("queue:orders", Action::Read));
        assert!(perm.matches("kv:users:123", Action::Read));
        assert!(!perm.matches("queue:orders", Action::Write));
    }

    #[test]
    fn test_permission_prefix_wildcard() {
        let perm = Permission::new("queue:*", Action::Write);
        assert!(perm.matches("queue:orders", Action::Write));
        assert!(perm.matches("queue:payments", Action::Write));
        assert!(!perm.matches("kv:users", Action::Write));
    }

    #[test]
    fn test_role_admin() {
        let role = Role::admin();
        assert_eq!(role.name, "admin");
        assert!(role.permissions[0].matches("anything", Action::Admin));
    }

    #[test]
    fn test_action_configure_includes() {
        assert!(Action::Configure.includes(Action::Configure));
        assert!(Action::Configure.includes(Action::Read));
        assert!(Action::Configure.includes(Action::Write));
        assert!(!Action::Configure.includes(Action::Delete));
        assert!(!Action::Configure.includes(Action::Admin));
    }

    #[test]
    fn test_permission_middle_wildcard() {
        let perm = Permission::new("queue:*:orders", Action::Read);
        assert!(perm.matches("queue:region1:orders", Action::Read));
        assert!(perm.matches("queue:region2:orders", Action::Read));
        assert!(!perm.matches("queue:orders", Action::Read));
        assert!(!perm.matches("stream:region1:orders", Action::Read));
    }

    #[test]
    fn test_get_resource_type() {
        assert_eq!(Permission::get_resource_type("queue:orders"), Some("queue"));
        assert_eq!(Permission::get_resource_type("kv:users:123"), Some("kv"));
        assert_eq!(Permission::get_resource_type("invalid"), Some("invalid"));
    }

    #[test]
    fn test_get_resource_name() {
        assert_eq!(
            Permission::get_resource_name("queue:orders"),
            Some("orders")
        );
        assert_eq!(Permission::get_resource_name("kv:users:123"), Some("users"));
        assert_eq!(Permission::get_resource_name("invalid"), None);
    }

    #[test]
    fn test_queue_manager_role() {
        let role = Role::queue_manager();
        assert_eq!(role.name, "queue_manager");

        // Should have read/write/configure permissions
        assert!(
            role.permissions
                .iter()
                .any(|p| p.matches("queue:orders", Action::Read))
        );
        assert!(
            role.permissions
                .iter()
                .any(|p| p.matches("queue:orders", Action::Write))
        );
        assert!(
            role.permissions
                .iter()
                .any(|p| p.matches("queue:orders", Action::Configure))
        );

        // Should not have delete or admin
        assert!(
            !role
                .permissions
                .iter()
                .any(|p| p.matches("queue:orders", Action::Delete))
        );
    }

    #[test]
    fn test_stream_manager_role() {
        let role = Role::stream_manager();
        assert_eq!(role.name, "stream_manager");

        assert!(
            role.permissions
                .iter()
                .any(|p| p.matches("stream:chat", Action::Read))
        );
        assert!(
            role.permissions
                .iter()
                .any(|p| p.matches("stream:chat", Action::Write))
        );
        assert!(
            role.permissions
                .iter()
                .any(|p| p.matches("stream:chat", Action::Configure))
        );
    }

    #[test]
    fn test_data_manager_role() {
        let role = Role::data_manager();
        assert_eq!(role.name, "data_manager");

        // Should have read/write on all data structures
        assert!(
            role.permissions
                .iter()
                .any(|p| p.matches("kv:key1", Action::Read))
        );
        assert!(
            role.permissions
                .iter()
                .any(|p| p.matches("kv:key1", Action::Write))
        );
        assert!(
            role.permissions
                .iter()
                .any(|p| p.matches("hash:key1", Action::Read))
        );
        assert!(
            role.permissions
                .iter()
                .any(|p| p.matches("list:key1", Action::Read))
        );
    }

    #[test]
    fn test_permission_multiple_patterns() {
        // Test that multiple permissions can match the same resource
        let perms = vec![
            Permission::new("queue:*", Action::Read),
            Permission::new("queue:orders", Action::Write),
        ];

        assert!(
            perms
                .iter()
                .any(|p| p.matches("queue:orders", Action::Read))
        );
        assert!(
            perms
                .iter()
                .any(|p| p.matches("queue:orders", Action::Write))
        );
    }

    #[test]
    fn test_permission_specific_vs_wildcard() {
        // Specific permission should take precedence (more restrictive)
        let specific = Permission::new("queue:orders", Action::Read);
        let wildcard = Permission::new("queue:*", Action::Write);

        assert!(specific.matches("queue:orders", Action::Read));
        assert!(!specific.matches("queue:orders", Action::Write));
        assert!(!specific.matches("queue:payments", Action::Read));

        assert!(wildcard.matches("queue:orders", Action::Write));
        assert!(wildcard.matches("queue:payments", Action::Write));
    }

    #[test]
    fn test_action_as_str() {
        assert_eq!(Action::Read.as_str(), "read");
        assert_eq!(Action::Write.as_str(), "write");
        assert_eq!(Action::Delete.as_str(), "delete");
        assert_eq!(Action::Configure.as_str(), "configure");
        assert_eq!(Action::Admin.as_str(), "admin");
        assert_eq!(Action::All.as_str(), "all");
    }

    #[test]
    fn test_permission_kv_patterns() {
        let perm1 = Permission::new("kv:*", Action::Read);
        assert!(perm1.matches("kv:users:123", Action::Read));
        assert!(perm1.matches("kv:orders:456", Action::Read));

        let perm2 = Permission::new("kv:users:*", Action::Write);
        assert!(perm2.matches("kv:users:123", Action::Write));
        assert!(perm2.matches("kv:users:456", Action::Write));
        assert!(!perm2.matches("kv:orders:123", Action::Write));
    }

    #[test]
    fn test_permission_stream_patterns() {
        let perm = Permission::new("stream:chat-*", Action::Read);
        assert!(perm.matches("stream:chat-room1", Action::Read));
        assert!(perm.matches("stream:chat-room2", Action::Read));
        assert!(!perm.matches("stream:notifications", Action::Read));
    }

    #[test]
    fn test_permission_pubsub_patterns() {
        let perm = Permission::new("pubsub:*", Action::Write);
        assert!(perm.matches("pubsub:user.created", Action::Write));
        assert!(perm.matches("pubsub:order.placed", Action::Write));
    }
}
