use super::{AuthResult, Permission, Role, password_validation::validate_password};
use crate::core::SynapError;
use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha512};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, info};

/// User account
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    /// Unique username
    pub username: String,
    /// Hashed password (SHA512)
    #[serde(skip_serializing)]
    pub password_hash: String,
    /// User roles
    pub roles: Vec<String>,
    /// Is admin user
    pub is_admin: bool,
    /// Account enabled
    pub enabled: bool,
    /// When user was created
    pub created_at: DateTime<Utc>,
    /// Last login time
    pub last_login: Option<DateTime<Utc>>,
    /// Additional metadata
    pub metadata: HashMap<String, String>,
}

impl User {
    /// Create a new user with password
    pub fn new(username: impl Into<String>, password: &str, is_admin: bool) -> AuthResult<Self> {
        // Validate password requirements
        validate_password(password)?;

        let password_hash = Self::hash_password(password);

        Ok(Self {
            username: username.into(),
            password_hash,
            roles: Vec::new(),
            is_admin,
            enabled: true,
            created_at: Utc::now(),
            last_login: None,
            metadata: HashMap::new(),
        })
    }

    /// Hash password using SHA512
    fn hash_password(password: &str) -> String {
        let mut hasher = Sha512::new();
        hasher.update(password.as_bytes());
        hex::encode(hasher.finalize())
    }

    /// Verify password
    pub fn verify_password(&self, password: &str) -> bool {
        let hashed = Self::hash_password(password);
        hashed == self.password_hash
    }

    /// Change password
    pub fn change_password(&mut self, new_password: &str) -> AuthResult<()> {
        // Validate password requirements
        validate_password(new_password)?;

        self.password_hash = Self::hash_password(new_password);
        Ok(())
    }

    /// Add role
    pub fn add_role(&mut self, role: impl Into<String>) {
        let role = role.into();
        if !self.roles.contains(&role) {
            self.roles.push(role);
        }
    }

    /// Remove role
    pub fn remove_role(&mut self, role: &str) {
        self.roles.retain(|r| r != role);
    }

    /// Update last login time
    pub fn update_last_login(&mut self) {
        self.last_login = Some(Utc::now());
    }
}

/// User manager - handles user authentication and management
#[derive(Clone)]
pub struct UserManager {
    users: Arc<RwLock<HashMap<String, User>>>,
    roles: Arc<RwLock<HashMap<String, Role>>>,
    /// Root username (protected user, cannot be deleted)
    root_username: Arc<RwLock<Option<String>>>,
}

impl UserManager {
    /// Create a new user manager
    pub fn new() -> Self {
        info!("Initializing User Manager");

        let mut roles = HashMap::new();
        roles.insert("admin".to_string(), Role::admin());
        roles.insert("readonly".to_string(), Role::readonly());

        Self {
            users: Arc::new(RwLock::new(HashMap::new())),
            roles: Arc::new(RwLock::new(roles)),
            root_username: Arc::new(RwLock::new(None)),
        }
    }

    /// Initialize root user from configuration
    /// Root user has full permissions and cannot be deleted
    pub fn initialize_root_user(
        &self,
        username: &str,
        password: &str,
        enabled: bool,
    ) -> AuthResult<()> {
        debug!(
            "Initializing root user: {} (enabled: {})",
            username, enabled
        );

        let mut users = self.users.write();
        let mut root_username = self.root_username.write();

        // If root user already exists, update password and enabled status
        if let Some(existing_root) = root_username.as_ref() {
            if existing_root == username {
                if let Some(user) = users.get_mut(username) {
                    // Update password if provided
                    if !password.is_empty() {
                        user.change_password(password)?;
                    }
                    user.enabled = enabled;
                    info!("Updated root user: {} (enabled: {})", username, enabled);
                    return Ok(());
                }
            }
        }

        // Create root user if it doesn't exist
        if !users.contains_key(username) {
            info!("Creating root user: {}", username);
            let mut root = User::new(username, password, true)?;
            root.add_role("admin");
            root.enabled = enabled;
            users.insert(username.to_string(), root);
        } else {
            // Update existing user to be root
            if let Some(user) = users.get_mut(username) {
                user.is_admin = true;
                user.add_role("admin");
                if !password.is_empty() {
                    user.change_password(password)?;
                }
                user.enabled = enabled;
            }
        }

        *root_username = Some(username.to_string());
        Ok(())
    }

    /// Create default admin user if no users exist (legacy method, use initialize_root_user)
    pub fn ensure_admin_exists(&self, username: &str, password: &str) -> AuthResult<()> {
        let users = self.users.read();
        if users.is_empty() {
            drop(users);

            info!("Creating default admin user: {}", username);
            let mut admin = User::new(username, password, true)?;
            admin.add_role("admin");

            self.users.write().insert(username.to_string(), admin);
        }
        Ok(())
    }

    /// Check if username is the root user
    pub fn is_root_user(&self, username: &str) -> bool {
        self.root_username
            .read()
            .as_ref()
            .is_some_and(|root| root == username)
    }

    /// Get root username
    pub fn get_root_username(&self) -> Option<String> {
        self.root_username.read().clone()
    }

    /// Create a new user
    pub fn create_user(
        &self,
        username: impl Into<String>,
        password: &str,
        is_admin: bool,
    ) -> AuthResult<()> {
        let username = username.into();
        debug!("Creating user: {}", username);

        let mut users = self.users.write();

        if users.contains_key(&username) {
            return Err(SynapError::InvalidRequest(format!(
                "User {} already exists",
                username
            )));
        }

        let user = User::new(&username, password, is_admin)?;
        users.insert(username.clone(), user);

        Ok(())
    }

    /// Authenticate user with username/password
    pub fn authenticate(&self, username: &str, password: &str) -> AuthResult<User> {
        debug!("Authenticating user: {}", username);

        // Check if root user is disabled
        if self.is_root_user(username) {
            let root_username = self.root_username.read();
            if let Some(root) = root_username.as_ref() {
                if root == username {
                    let users = self.users.read();
                    if let Some(user) = users.get(username) {
                        if !user.enabled {
                            return Err(SynapError::InvalidRequest(
                                "Root user is disabled".to_string(),
                            ));
                        }
                    }
                }
            }
        }

        let mut users = self.users.write();

        let user = users
            .get_mut(username)
            .ok_or_else(|| SynapError::InvalidRequest("Invalid credentials".to_string()))?;

        if !user.enabled {
            return Err(SynapError::InvalidRequest("Account disabled".to_string()));
        }

        if !user.verify_password(password) {
            return Err(SynapError::InvalidRequest(
                "Invalid credentials".to_string(),
            ));
        }

        user.update_last_login();
        Ok(user.clone())
    }

    /// Get user by username
    pub fn get_user(&self, username: &str) -> Option<User> {
        self.users.read().get(username).cloned()
    }

    /// Delete user (cannot delete root user)
    pub fn delete_user(&self, username: &str) -> AuthResult<bool> {
        debug!("Deleting user: {}", username);

        // Protect root user from deletion
        if self.is_root_user(username) {
            return Err(SynapError::InvalidRequest(
                "Cannot delete root user".to_string(),
            ));
        }

        Ok(self.users.write().remove(username).is_some())
    }

    /// Change user password
    pub fn change_password(&self, username: &str, new_password: &str) -> AuthResult<()> {
        debug!("Changing password for user: {}", username);

        let mut users = self.users.write();
        let user = users
            .get_mut(username)
            .ok_or_else(|| SynapError::KeyNotFound(format!("User {} not found", username)))?;

        user.change_password(new_password)?;
        Ok(())
    }

    /// Enable/disable user (can disable root user via config)
    pub fn set_user_enabled(&self, username: &str, enabled: bool) -> AuthResult<()> {
        debug!("Setting user {} enabled: {}", username, enabled);

        let mut users = self.users.write();
        let user = users
            .get_mut(username)
            .ok_or_else(|| SynapError::KeyNotFound(format!("User {} not found", username)))?;

        user.enabled = enabled;
        Ok(())
    }

    /// Disable root user (for security after initial setup)
    pub fn disable_root_user(&self) -> AuthResult<()> {
        if let Some(root_username) = self.get_root_username() {
            debug!("Disabling root user: {}", root_username);
            self.set_user_enabled(&root_username, false)
        } else {
            Err(SynapError::InvalidRequest(
                "No root user configured".to_string(),
            ))
        }
    }

    /// Get user permissions (from roles)
    pub fn get_user_permissions(&self, username: &str) -> Vec<Permission> {
        let users = self.users.read();
        let roles_map = self.roles.read();

        let user = match users.get(username) {
            Some(u) => u,
            None => return vec![],
        };

        if user.is_admin {
            return vec![Permission::new("*", super::Action::All)];
        }

        let mut permissions = Vec::new();
        for role_name in &user.roles {
            if let Some(role) = roles_map.get(role_name) {
                permissions.extend(role.permissions.clone());
            }
        }

        permissions
    }

    /// List all users
    pub fn list_users(&self) -> Vec<String> {
        self.users.read().keys().cloned().collect()
    }

    /// Create or update role
    pub fn create_role(&self, role: Role) -> AuthResult<()> {
        debug!("Creating role: {}", role.name);
        self.roles.write().insert(role.name.clone(), role);
        Ok(())
    }

    /// Get role
    pub fn get_role(&self, name: &str) -> Option<Role> {
        self.roles.read().get(name).cloned()
    }

    /// Delete role
    pub fn delete_role(&self, name: &str) -> AuthResult<bool> {
        debug!("Deleting role: {}", name);
        Ok(self.roles.write().remove(name).is_some())
    }

    /// List all roles
    pub fn list_roles(&self) -> Vec<String> {
        self.roles.read().keys().cloned().collect()
    }

    /// Add role to user
    pub fn add_user_role(&self, username: &str, role_name: &str) -> AuthResult<()> {
        debug!("Adding role {} to user {}", role_name, username);

        // Check role exists
        if !self.roles.read().contains_key(role_name) {
            return Err(SynapError::InvalidRequest(format!(
                "Role {} does not exist",
                role_name
            )));
        }

        let mut users = self.users.write();
        let user = users
            .get_mut(username)
            .ok_or_else(|| SynapError::KeyNotFound(format!("User {} not found", username)))?;

        user.add_role(role_name);
        Ok(())
    }

    /// Remove role from user
    pub fn remove_user_role(&self, username: &str, role_name: &str) -> AuthResult<()> {
        debug!("Removing role {} from user {}", role_name, username);

        let mut users = self.users.write();
        let user = users
            .get_mut(username)
            .ok_or_else(|| SynapError::KeyNotFound(format!("User {} not found", username)))?;

        user.remove_role(role_name);
        Ok(())
    }
}

impl Default for UserManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_user() {
        let user = User::new("testuser", "password123", false).unwrap();
        assert_eq!(user.username, "testuser");
        assert!(user.verify_password("password123"));
        assert!(!user.verify_password("wrongpassword"));
        assert!(!user.is_admin);
        assert!(user.enabled);
    }

    #[test]
    fn test_change_password() {
        let mut user = User::new("testuser", "oldpass123", false).unwrap();
        assert!(user.verify_password("oldpass123"));

        user.change_password("newpass123").unwrap();
        assert!(!user.verify_password("oldpass123"));
        assert!(user.verify_password("newpass123"));
    }

    #[test]
    fn test_user_manager_create() {
        let manager = UserManager::new();
        manager.create_user("user1", "pass12345", false).unwrap();

        let user = manager.get_user("user1").unwrap();
        assert_eq!(user.username, "user1");
    }

    #[test]
    fn test_user_manager_authenticate() {
        let manager = UserManager::new();
        manager.create_user("user1", "pass12345", false).unwrap();

        // Valid credentials
        let result = manager.authenticate("user1", "pass12345");
        assert!(result.is_ok());

        // Invalid password
        let result = manager.authenticate("user1", "wrongpass");
        assert!(result.is_err());

        // Non-existent user
        let result = manager.authenticate("nonexistent", "pass12345");
        assert!(result.is_err());
    }

    #[test]
    fn test_user_manager_disable_user() {
        let manager = UserManager::new();
        manager.create_user("user1", "pass12345", false).unwrap();

        // Can authenticate when enabled
        assert!(manager.authenticate("user1", "pass12345").is_ok());

        // Disable user
        manager.set_user_enabled("user1", false).unwrap();

        // Cannot authenticate when disabled
        assert!(manager.authenticate("user1", "pass12345").is_err());
    }

    #[test]
    fn test_user_roles() {
        let manager = UserManager::new();
        manager.create_user("user1", "pass12345", false).unwrap();

        manager.add_user_role("user1", "readonly").unwrap();
        let user = manager.get_user("user1").unwrap();
        assert!(user.roles.contains(&"readonly".to_string()));

        manager.remove_user_role("user1", "readonly").unwrap();
        let user = manager.get_user("user1").unwrap();
        assert!(!user.roles.contains(&"readonly".to_string()));
    }

    // Root user tests
    #[test]
    fn test_initialize_root_user() {
        let manager = UserManager::new();
        manager
            .initialize_root_user("root", "rootpass", true)
            .unwrap();

        assert!(manager.is_root_user("root"));
        assert_eq!(manager.get_root_username(), Some("root".to_string()));

        let root = manager.get_user("root").unwrap();
        assert!(root.is_admin);
        assert!(root.enabled);
        assert!(root.roles.contains(&"admin".to_string()));
    }

    #[test]
    fn test_root_user_cannot_be_deleted() {
        let manager = UserManager::new();
        manager
            .initialize_root_user("root", "rootpass", true)
            .unwrap();

        let result = manager.delete_user("root");
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Cannot delete root user")
        );

        // Root user still exists
        assert!(manager.get_user("root").is_some());
    }

    #[test]
    fn test_root_user_disabled_cannot_authenticate() {
        let manager = UserManager::new();
        manager
            .initialize_root_user("root", "rootpass", false)
            .unwrap();

        // Root user exists but is disabled
        let root = manager.get_user("root").unwrap();
        assert!(!root.enabled);

        // Cannot authenticate when disabled
        let result = manager.authenticate("root", "rootpass");
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Root user is disabled")
        );
    }

    #[test]
    fn test_root_user_enabled_can_authenticate() {
        let manager = UserManager::new();
        manager
            .initialize_root_user("root", "rootpass", true)
            .unwrap();

        let result = manager.authenticate("root", "rootpass");
        assert!(result.is_ok());

        let authenticated = result.unwrap();
        assert_eq!(authenticated.username, "root");
        assert!(authenticated.is_admin);
    }

    #[test]
    fn test_update_root_user_password() {
        let manager = UserManager::new();
        manager
            .initialize_root_user("root", "oldpass123", true)
            .unwrap();

        // Update password
        manager
            .initialize_root_user("root", "newpass123", true)
            .unwrap();

        // Old password doesn't work
        assert!(manager.authenticate("root", "oldpass123").is_err());

        // New password works
        assert!(manager.authenticate("root", "newpass123").is_ok());
    }

    #[test]
    fn test_disable_root_user() {
        let manager = UserManager::new();
        manager
            .initialize_root_user("root", "rootpass", true)
            .unwrap();

        // Can authenticate initially
        assert!(manager.authenticate("root", "rootpass").is_ok());

        // Disable root user
        manager.disable_root_user().unwrap();

        // Cannot authenticate after disable
        let result = manager.authenticate("root", "rootpass");
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Root user is disabled")
        );
    }

    #[test]
    fn test_disable_root_user_when_not_configured() {
        let manager = UserManager::new();
        // No root user initialized

        let result = manager.disable_root_user();
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("No root user configured")
        );
    }

    #[test]
    fn test_root_user_has_admin_permissions() {
        let manager = UserManager::new();
        manager
            .initialize_root_user("root", "rootpass", true)
            .unwrap();

        let permissions = manager.get_user_permissions("root");
        assert!(!permissions.is_empty());
        assert!(permissions[0].resource_pattern == "*");
    }

    #[test]
    fn test_multiple_root_users_not_allowed() {
        let manager = UserManager::new();
        manager
            .initialize_root_user("root1", "pass12345", true)
            .unwrap();
        assert_eq!(manager.get_root_username(), Some("root1".to_string()));

        // Initialize different root user - should update root_username
        manager
            .initialize_root_user("root2", "pass23456", true)
            .unwrap();
        assert_eq!(manager.get_root_username(), Some("root2".to_string()));

        // root1 is no longer root
        assert!(!manager.is_root_user("root1"));
        assert!(manager.is_root_user("root2"));
    }

    #[test]
    fn test_regular_user_can_be_deleted() {
        let manager = UserManager::new();
        manager
            .initialize_root_user("root", "rootpass123", true)
            .unwrap();
        manager.create_user("regular", "pass12345", false).unwrap();

        // Regular user can be deleted
        let deleted = manager.delete_user("regular").unwrap();
        assert!(deleted);
        assert!(manager.get_user("regular").is_none());

        // Root user still exists
        assert!(manager.get_user("root").is_some());
    }

    #[test]
    fn test_root_user_initialization_with_existing_user() {
        let manager = UserManager::new();
        // Create regular user first
        manager.create_user("admin", "oldpass123", false).unwrap();

        // Initialize as root user
        manager
            .initialize_root_user("admin", "newpass123", true)
            .unwrap();

        assert!(manager.is_root_user("admin"));
        let admin = manager.get_user("admin").unwrap();
        assert!(admin.is_admin);
        assert!(admin.verify_password("newpass123"));
    }

    #[test]
    fn test_root_user_get_root_username() {
        let manager = UserManager::new();
        assert_eq!(manager.get_root_username(), None);

        manager.initialize_root_user("root", "pass12345", true).unwrap();
        assert_eq!(manager.get_root_username(), Some("root".to_string()));
    }

    #[test]
    fn test_is_root_user() {
        let manager = UserManager::new();
        assert!(!manager.is_root_user("root"));
        assert!(!manager.is_root_user("admin"));

        manager.initialize_root_user("root", "pass12345", true).unwrap();
        assert!(manager.is_root_user("root"));
        assert!(!manager.is_root_user("admin"));
    }
}
