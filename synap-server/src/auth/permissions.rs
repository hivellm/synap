use serde::{Deserialize, Serialize};

/// Permission actions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Action {
    /// Read operations (GET, CONSUME, etc.)
    Read,
    /// Write operations (SET, PUBLISH, etc.)
    Write,
    /// Delete operations
    Delete,
    /// Administrative operations
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
            _ => self == &other,
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

        false
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
}
