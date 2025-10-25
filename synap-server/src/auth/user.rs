use super::{AuthResult, Permission, Role};
use crate::core::SynapError;
use bcrypt::{DEFAULT_COST, hash, verify};
use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, info};

/// User account
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    /// Unique username
    pub username: String,
    /// Hashed password (bcrypt)
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
        let password_hash = hash(password, DEFAULT_COST)
            .map_err(|e| SynapError::InternalError(format!("Failed to hash password: {}", e)))?;

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

    /// Verify password
    pub fn verify_password(&self, password: &str) -> bool {
        verify(password, &self.password_hash).unwrap_or(false)
    }

    /// Change password
    pub fn change_password(&mut self, new_password: &str) -> AuthResult<()> {
        self.password_hash = hash(new_password, DEFAULT_COST)
            .map_err(|e| SynapError::InternalError(format!("Failed to hash password: {}", e)))?;
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
        }
    }

    /// Create default admin user if no users exist
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

    /// Delete user
    pub fn delete_user(&self, username: &str) -> AuthResult<bool> {
        debug!("Deleting user: {}", username);
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

    /// Enable/disable user
    pub fn set_user_enabled(&self, username: &str, enabled: bool) -> AuthResult<()> {
        debug!("Setting user {} enabled: {}", username, enabled);

        let mut users = self.users.write();
        let user = users
            .get_mut(username)
            .ok_or_else(|| SynapError::KeyNotFound(format!("User {} not found", username)))?;

        user.enabled = enabled;
        Ok(())
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
        let mut user = User::new("testuser", "oldpass", false).unwrap();
        assert!(user.verify_password("oldpass"));

        user.change_password("newpass").unwrap();
        assert!(!user.verify_password("oldpass"));
        assert!(user.verify_password("newpass"));
    }

    #[test]
    fn test_user_manager_create() {
        let manager = UserManager::new();
        manager.create_user("user1", "pass123", false).unwrap();

        let user = manager.get_user("user1").unwrap();
        assert_eq!(user.username, "user1");
    }

    #[test]
    fn test_user_manager_authenticate() {
        let manager = UserManager::new();
        manager.create_user("user1", "pass123", false).unwrap();

        // Valid credentials
        let result = manager.authenticate("user1", "pass123");
        assert!(result.is_ok());

        // Invalid password
        let result = manager.authenticate("user1", "wrongpass");
        assert!(result.is_err());

        // Non-existent user
        let result = manager.authenticate("nonexistent", "pass");
        assert!(result.is_err());
    }

    #[test]
    fn test_user_manager_disable_user() {
        let manager = UserManager::new();
        manager.create_user("user1", "pass123", false).unwrap();

        // Can authenticate when enabled
        assert!(manager.authenticate("user1", "pass123").is_ok());

        // Disable user
        manager.set_user_enabled("user1", false).unwrap();

        // Cannot authenticate when disabled
        assert!(manager.authenticate("user1", "pass123").is_err());
    }

    #[test]
    fn test_user_roles() {
        let manager = UserManager::new();
        manager.create_user("user1", "pass123", false).unwrap();

        manager.add_user_role("user1", "readonly").unwrap();
        let user = manager.get_user("user1").unwrap();
        assert!(user.roles.contains(&"readonly".to_string()));

        manager.remove_user_role("user1", "readonly").unwrap();
        let user = manager.get_user("user1").unwrap();
        assert!(!user.roles.contains(&"readonly".to_string()));
    }
}
