//! Authentication and Authorization REST API Handlers
//!
//! Handlers for user management, API key management, and authentication

use crate::auth::{Action, ApiKeyManager, AuthContextExtractor, Permission, UserManager};
use crate::core::SynapError;
use axum::{
    Json,
    extract::{Path, State},
};
use serde::{Deserialize, Serialize};
use std::net::IpAddr;
use std::sync::Arc;
use tracing::debug;

/// Application state for auth handlers
#[derive(Clone)]
pub struct AuthState {
    pub user_manager: Arc<UserManager>,
    pub api_key_manager: Arc<ApiKeyManager>,
}

// ==================== Authentication Endpoints ====================

/// Login request
#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

/// Login response
#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub success: bool,
    pub user: Option<UserInfo>,
    pub message: String,
}

/// User info (without password)
#[derive(Debug, Serialize)]
pub struct UserInfo {
    pub username: String,
    pub roles: Vec<String>,
    pub is_admin: bool,
    pub enabled: bool,
}

/// POST /auth/login - Login with username/password
pub async fn auth_login(
    State(state): State<AuthState>,
    Json(req): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, SynapError> {
    debug!("Login attempt for user: {}", req.username);

    match state
        .user_manager
        .authenticate(&req.username, &req.password)
    {
        Ok(user) => {
            let user_info = UserInfo {
                username: user.username.clone(),
                roles: user.roles.clone(),
                is_admin: user.is_admin,
                enabled: user.enabled,
            };

            Ok(Json(LoginResponse {
                success: true,
                user: Some(user_info),
                message: "Login successful".to_string(),
            }))
        }
        Err(e) => {
            debug!("Login failed for user {}: {}", req.username, e);
            Ok(Json(LoginResponse {
                success: false,
                user: None,
                message: "Invalid credentials".to_string(),
            }))
        }
    }
}

/// GET /auth/me - Get current user info
pub async fn auth_me(
    State(_state): State<AuthState>,
    AuthContextExtractor(auth_context): AuthContextExtractor,
) -> Result<Json<UserInfo>, SynapError> {
    let username = auth_context
        .user_id
        .ok_or_else(|| SynapError::InvalidRequest("Not authenticated".to_string()))?;

    // Get user from manager (we need to pass it through state)
    // For now, return basic info from context
    Ok(Json(UserInfo {
        username,
        roles: vec![], // Would need to fetch from UserManager
        is_admin: auth_context.is_admin,
        enabled: true,
    }))
}

// ==================== API Key Management Endpoints ====================

/// Create API key request
#[derive(Debug, Deserialize)]
pub struct CreateKeyRequest {
    pub name: String,
    pub expires_in_seconds: Option<u64>,
    pub permissions: Option<Vec<PermissionRequest>>,
    pub allowed_ips: Option<Vec<String>>,
}

/// Permission request
#[derive(Debug, Deserialize)]
pub struct PermissionRequest {
    pub resource: String,
    pub action: String, // "read", "write", "delete", "configure", "admin", "all"
}

/// Create API key response
#[derive(Debug, Serialize)]
pub struct CreateKeyResponse {
    pub success: bool,
    pub key: Option<ApiKeyInfo>,
    pub message: String,
}

/// API key info (with secret shown only once)
#[derive(Debug, Serialize)]
pub struct ApiKeyInfo {
    pub id: String,
    pub key: String, // Only shown on creation
    pub name: String,
    pub expires_at: Option<String>,
    pub created_at: String,
}

/// POST /auth/keys - Create API key
pub async fn auth_create_key(
    State(state): State<AuthState>,
    AuthContextExtractor(auth_context): AuthContextExtractor,
    Json(req): Json<CreateKeyRequest>,
) -> Result<Json<CreateKeyResponse>, SynapError> {
    debug!("Creating API key: {}", req.name);

    // Convert permission requests to Permission objects
    let permissions: Vec<Permission> = req
        .permissions
        .unwrap_or_default()
        .into_iter()
        .map(|p| {
            let action = match p.action.as_str() {
                "read" => Action::Read,
                "write" => Action::Write,
                "delete" => Action::Delete,
                "configure" => Action::Configure,
                "admin" => Action::Admin,
                "all" => Action::All,
                _ => Action::Read,
            };
            Permission::new(p.resource, action)
        })
        .collect();

    // Parse allowed IPs
    let allowed_ips: Vec<IpAddr> = req
        .allowed_ips
        .unwrap_or_default()
        .into_iter()
        .filter_map(|ip_str| ip_str.parse().ok())
        .collect();

    // Get username from auth context
    let username = auth_context.user_id.clone();

    // Create key
    let api_key = if let Some(ttl) = req.expires_in_seconds {
        state
            .api_key_manager
            .create_temporary(req.name, username, permissions, allowed_ips, ttl)?
    } else {
        state
            .api_key_manager
            .create(req.name, username, permissions, allowed_ips, None)?
    };

    Ok(Json(CreateKeyResponse {
        success: true,
        key: Some(ApiKeyInfo {
            id: api_key.id.clone(),
            key: api_key.key.clone(), // Show secret only once
            name: api_key.name.clone(),
            expires_at: api_key.expires_at.map(|dt| dt.to_rfc3339()),
            created_at: api_key.created_at.to_rfc3339(),
        }),
        message: "API key created successfully".to_string(),
    }))
}

/// List API keys response
#[derive(Debug, Serialize)]
pub struct ListKeysResponse {
    pub success: bool,
    pub keys: Vec<ApiKeyMetadataResponse>,
}

/// API key metadata (without secret)
#[derive(Debug, Serialize)]
pub struct ApiKeyMetadataResponse {
    pub id: String,
    pub name: String,
    pub username: Option<String>,
    pub expires_at: Option<String>,
    pub enabled: bool,
    pub created_at: String,
    pub last_used_at: Option<String>,
    pub usage_count: u64,
}

/// GET /auth/keys - List API keys
pub async fn auth_list_keys(
    State(state): State<AuthState>,
    AuthContextExtractor(auth_context): AuthContextExtractor,
) -> Result<Json<ListKeysResponse>, SynapError> {
    debug!("Listing API keys");

    // Filter by user if not admin
    let keys = if auth_context.is_admin {
        state.api_key_manager.list()
    } else if let Some(user_id) = &auth_context.user_id {
        state.api_key_manager.list_by_user(user_id)
    } else {
        vec![]
    };

    let key_responses: Vec<ApiKeyMetadataResponse> = keys
        .into_iter()
        .map(|k| ApiKeyMetadataResponse {
            id: k.id,
            name: k.name,
            username: k.username,
            expires_at: k.expires_at.map(|dt| dt.to_rfc3339()),
            enabled: k.enabled,
            created_at: k.created_at.to_rfc3339(),
            last_used_at: k.last_used_at.map(|dt| dt.to_rfc3339()),
            usage_count: k.usage_count,
        })
        .collect();

    Ok(Json(ListKeysResponse {
        success: true,
        keys: key_responses,
    }))
}

/// DELETE /auth/keys/:id - Revoke API key
pub async fn auth_revoke_key(
    State(state): State<AuthState>,
    AuthContextExtractor(auth_context): AuthContextExtractor,
    Path(key_id): Path<String>,
) -> Result<Json<RevokeKeyResponse>, SynapError> {
    debug!("Revoking API key: {}", key_id);

    // Check if user owns the key or is admin
    if let Some(api_key) = state.api_key_manager.get(&key_id) {
        if !auth_context.is_admin {
            if let Some(user_id) = &auth_context.user_id {
                if api_key.username.as_ref() != Some(user_id) {
                    return Err(SynapError::InvalidRequest(
                        "Cannot revoke API key owned by another user".to_string(),
                    ));
                }
            } else {
                return Err(SynapError::InvalidRequest(
                    "Not authorized to revoke this API key".to_string(),
                ));
            }
        }
    }

    let revoked = state.api_key_manager.revoke(&key_id)?;

    Ok(Json(RevokeKeyResponse {
        success: revoked,
        message: if revoked {
            "API key revoked".to_string()
        } else {
            "API key not found".to_string()
        },
    }))
}

/// Revoke key response
#[derive(Debug, Serialize)]
pub struct RevokeKeyResponse {
    pub success: bool,
    pub message: String,
}

// ==================== User Management Endpoints (Admin Only) ====================

/// Create user request
#[derive(Debug, Deserialize)]
pub struct CreateUserRequest {
    pub username: String,
    pub password: String,
    pub roles: Option<Vec<String>>,
    pub is_admin: Option<bool>,
}

/// Create user response
#[derive(Debug, Serialize)]
pub struct CreateUserResponse {
    pub success: bool,
    pub message: String,
}

/// POST /auth/users - Create user (admin only)
pub async fn auth_create_user(
    State(state): State<AuthState>,
    AuthContextExtractor(auth_context): AuthContextExtractor,
    Json(req): Json<CreateUserRequest>,
) -> Result<Json<CreateUserResponse>, SynapError> {
    debug!("Creating user: {}", req.username);

    // Check admin permission
    if !auth_context.is_admin {
        return Err(SynapError::InvalidRequest(
            "Admin permission required".to_string(),
        ));
    }

    // Create user
    state
        .user_manager
        .create_user(&req.username, &req.password, req.is_admin.unwrap_or(false))?;

    // Assign roles if provided
    if let Some(roles) = req.roles {
        for role in roles {
            state.user_manager.add_user_role(&req.username, &role)?;
        }
    }

    Ok(Json(CreateUserResponse {
        success: true,
        message: format!("User {} created successfully", req.username),
    }))
}

/// List users response
#[derive(Debug, Serialize)]
pub struct ListUsersResponse {
    pub success: bool,
    pub users: Vec<UserInfo>,
}

/// GET /auth/users - List users (admin only)
pub async fn auth_list_users(
    State(state): State<AuthState>,
    AuthContextExtractor(auth_context): AuthContextExtractor,
) -> Result<Json<ListUsersResponse>, SynapError> {
    debug!("Listing users");

    // Check admin permission
    if !auth_context.is_admin {
        return Err(SynapError::InvalidRequest(
            "Admin permission required".to_string(),
        ));
    }

    let usernames = state.user_manager.list_users();
    let users: Vec<UserInfo> = usernames
        .into_iter()
        .filter_map(|username| {
            state.user_manager.get_user(&username).map(|user| UserInfo {
                username: user.username,
                roles: user.roles,
                is_admin: user.is_admin,
                enabled: user.enabled,
            })
        })
        .collect();

    Ok(Json(ListUsersResponse {
        success: true,
        users,
    }))
}

/// GET /auth/users/:username - Get user details
pub async fn auth_get_user(
    State(state): State<AuthState>,
    AuthContextExtractor(auth_context): AuthContextExtractor,
    Path(username): Path<String>,
) -> Result<Json<UserInfo>, SynapError> {
    debug!("Getting user: {}", username);

    // Check admin permission or self
    if !auth_context.is_admin && auth_context.user_id.as_ref() != Some(&username) {
        return Err(SynapError::InvalidRequest(
            "Not authorized to view this user".to_string(),
        ));
    }

    let user = state
        .user_manager
        .get_user(&username)
        .ok_or_else(|| SynapError::KeyNotFound(format!("User {} not found", username)))?;

    Ok(Json(UserInfo {
        username: user.username,
        roles: user.roles,
        is_admin: user.is_admin,
        enabled: user.enabled,
    }))
}

/// Delete user request
#[derive(Debug, Serialize)]
pub struct DeleteUserResponse {
    pub success: bool,
    pub message: String,
}

/// DELETE /auth/users/:username - Delete user (admin only)
pub async fn auth_delete_user(
    State(state): State<AuthState>,
    AuthContextExtractor(auth_context): AuthContextExtractor,
    Path(username): Path<String>,
) -> Result<Json<DeleteUserResponse>, SynapError> {
    debug!("Deleting user: {}", username);

    // Check admin permission
    if !auth_context.is_admin {
        return Err(SynapError::InvalidRequest(
            "Admin permission required".to_string(),
        ));
    }

    let deleted = state.user_manager.delete_user(&username)?;

    Ok(Json(DeleteUserResponse {
        success: deleted,
        message: if deleted {
            format!("User {} deleted", username)
        } else {
            format!("User {} not found", username)
        },
    }))
}

/// Change password request
#[derive(Debug, Deserialize)]
pub struct ChangePasswordRequest {
    pub new_password: String,
}

/// Change password response
#[derive(Debug, Serialize)]
pub struct ChangePasswordResponse {
    pub success: bool,
    pub message: String,
}

/// POST /auth/users/:username/password - Change password
pub async fn auth_change_password(
    State(state): State<AuthState>,
    AuthContextExtractor(auth_context): AuthContextExtractor,
    Path(username): Path<String>,
    Json(req): Json<ChangePasswordRequest>,
) -> Result<Json<ChangePasswordResponse>, SynapError> {
    debug!("Changing password for user: {}", username);

    // Check admin permission or self
    if !auth_context.is_admin && auth_context.user_id.as_ref() != Some(&username) {
        return Err(SynapError::InvalidRequest(
            "Not authorized to change this user's password".to_string(),
        ));
    }

    state
        .user_manager
        .change_password(&username, &req.new_password)?;

    Ok(Json(ChangePasswordResponse {
        success: true,
        message: "Password changed successfully".to_string(),
    }))
}

/// Enable/disable user request
#[derive(Debug, Deserialize)]
pub struct SetUserEnabledRequest {
    pub enabled: bool,
}

/// Enable/disable user response
#[derive(Debug, Serialize)]
pub struct SetUserEnabledResponse {
    pub success: bool,
    pub message: String,
}

/// POST /auth/users/:username/enable - Enable user
pub async fn auth_enable_user(
    State(state): State<AuthState>,
    AuthContextExtractor(auth_context): AuthContextExtractor,
    Path(username): Path<String>,
) -> Result<Json<SetUserEnabledResponse>, SynapError> {
    debug!("Enabling user: {}", username);

    if !auth_context.is_admin {
        return Err(SynapError::InvalidRequest(
            "Admin permission required".to_string(),
        ));
    }

    state.user_manager.set_user_enabled(&username, true)?;

    Ok(Json(SetUserEnabledResponse {
        success: true,
        message: format!("User {} enabled", username),
    }))
}

/// POST /auth/users/:username/disable - Disable user
pub async fn auth_disable_user(
    State(state): State<AuthState>,
    AuthContextExtractor(auth_context): AuthContextExtractor,
    Path(username): Path<String>,
) -> Result<Json<SetUserEnabledResponse>, SynapError> {
    debug!("Disabling user: {}", username);

    if !auth_context.is_admin {
        return Err(SynapError::InvalidRequest(
            "Admin permission required".to_string(),
        ));
    }

    state.user_manager.set_user_enabled(&username, false)?;

    Ok(Json(SetUserEnabledResponse {
        success: true,
        message: format!("User {} disabled", username),
    }))
}

// ==================== Role Management Endpoints ====================

/// Grant role request
#[derive(Debug, Deserialize)]
pub struct GrantRoleRequest {
    pub role: String,
}

/// Grant role response
#[derive(Debug, Serialize)]
pub struct GrantRoleResponse {
    pub success: bool,
    pub message: String,
}

/// POST /auth/users/:username/roles - Grant role (admin only)
pub async fn auth_grant_role(
    State(state): State<AuthState>,
    AuthContextExtractor(auth_context): AuthContextExtractor,
    Path(username): Path<String>,
    Json(req): Json<GrantRoleRequest>,
) -> Result<Json<GrantRoleResponse>, SynapError> {
    debug!("Granting role {} to user {}", req.role, username);

    if !auth_context.is_admin {
        return Err(SynapError::InvalidRequest(
            "Admin permission required".to_string(),
        ));
    }

    state.user_manager.add_user_role(&username, &req.role)?;

    Ok(Json(GrantRoleResponse {
        success: true,
        message: format!("Role {} granted to user {}", req.role, username),
    }))
}

/// DELETE /auth/users/:username/roles/:role - Revoke role (admin only)
pub async fn auth_revoke_role(
    State(state): State<AuthState>,
    AuthContextExtractor(auth_context): AuthContextExtractor,
    Path((username, role)): Path<(String, String)>,
) -> Result<Json<GrantRoleResponse>, SynapError> {
    debug!("Revoking role {} from user {}", role, username);

    if !auth_context.is_admin {
        return Err(SynapError::InvalidRequest(
            "Admin permission required".to_string(),
        ));
    }

    state.user_manager.remove_user_role(&username, &role)?;

    Ok(Json(GrantRoleResponse {
        success: true,
        message: format!("Role {} revoked from user {}", role, username),
    }))
}

/// List roles response
#[derive(Debug, Serialize)]
pub struct ListRolesResponse {
    pub success: bool,
    pub roles: Vec<RoleInfo>,
}

/// Role info
#[derive(Debug, Serialize)]
pub struct RoleInfo {
    pub name: String,
    pub permissions: Vec<PermissionInfo>,
}

/// Permission info
#[derive(Debug, Serialize)]
pub struct PermissionInfo {
    pub resource: String,
    pub action: String,
}

/// GET /auth/roles - List roles
pub async fn auth_list_roles(
    State(state): State<AuthState>,
    AuthContextExtractor(auth_context): AuthContextExtractor,
) -> Result<Json<ListRolesResponse>, SynapError> {
    debug!("Listing roles");

    if !auth_context.is_admin {
        return Err(SynapError::InvalidRequest(
            "Admin permission required".to_string(),
        ));
    }

    let role_names = state.user_manager.list_roles();
    let roles: Vec<RoleInfo> = role_names
        .into_iter()
        .filter_map(|name| {
            state.user_manager.get_role(&name).map(|role| RoleInfo {
                name: role.name.clone(),
                permissions: role
                    .permissions
                    .into_iter()
                    .map(|p| PermissionInfo {
                        resource: p.resource_pattern,
                        action: p.action.as_str().to_string(),
                    })
                    .collect(),
            })
        })
        .collect();

    Ok(Json(ListRolesResponse {
        success: true,
        roles,
    }))
}
