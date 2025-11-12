//! Axum extractor for AuthContext
//!
//! Provides an extractor to easily access AuthContext in handlers

use super::AuthContext;
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};

/// Extract AuthContext from request extensions
///
/// This extractor retrieves the AuthContext that was set by AuthMiddleware.
/// If no AuthContext is found, it returns a 500 Internal Server Error
/// (which should not happen if middleware is properly configured).
pub struct AuthContextExtractor(pub AuthContext);

impl<S> axum::extract::FromRequestParts<S> for AuthContextExtractor
where
    S: Send + Sync,
{
    type Rejection = Response;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        _state: &S,
    ) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<AuthContext>()
            .cloned()
            .map(Self)
            .ok_or_else(|| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "AuthContext not found in request extensions",
                )
                    .into_response()
            })
    }
}

/// Helper function to require a specific permission
///
/// Returns a SynapError::Forbidden with 403 Forbidden status
/// if the user doesn't have the required permission.
pub fn require_permission(
    ctx: &AuthContext,
    resource: &str,
    action: super::Action,
) -> Result<(), crate::core::SynapError> {
    if ctx.has_permission(resource, action) {
        Ok(())
    } else {
        Err(crate::core::SynapError::Forbidden(format!(
            "Insufficient permissions for resource: {}, action: {}",
            resource,
            action.as_str()
        )))
    }
}

/// Helper function to require authentication
///
/// Returns a SynapError::Unauthorized with 401 Unauthorized status
/// if the user is not authenticated.
pub fn require_auth(ctx: &AuthContext) -> Result<(), crate::core::SynapError> {
    if ctx.is_authenticated() {
        Ok(())
    } else {
        Err(crate::core::SynapError::Unauthorized(
            "Authentication required".to_string(),
        ))
    }
}

/// Helper function to require admin permissions
///
/// Returns a SynapError::Forbidden with 403 Forbidden status
/// if the user is not an admin.
pub fn require_admin(ctx: &AuthContext) -> Result<(), crate::core::SynapError> {
    if ctx.is_admin || ctx.has_permission("admin:*", super::Action::Admin) {
        Ok(())
    } else {
        Err(crate::core::SynapError::Forbidden(
            "Admin permission required".to_string(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::{Action, Permission};
    use std::net::IpAddr;

    fn create_test_context(permissions: Vec<Permission>, is_admin: bool) -> AuthContext {
        AuthContext {
            user_id: Some("test_user".to_string()),
            api_key_id: None,
            client_ip: IpAddr::from([127, 0, 0, 1]),
            permissions,
            is_admin,
        }
    }

    #[test]
    fn test_require_permission_success() {
        let ctx = create_test_context(vec![Permission::new("kv:*", Action::Read)], false);
        assert!(require_permission(&ctx, "kv:key1", Action::Read).is_ok());
    }

    #[test]
    fn test_require_permission_failure() {
        let ctx = create_test_context(vec![Permission::new("kv:*", Action::Read)], false);
        assert!(require_permission(&ctx, "kv:key1", Action::Write).is_err());
    }

    #[test]
    fn test_require_permission_admin() {
        let ctx = create_test_context(vec![], true);
        assert!(require_permission(&ctx, "kv:key1", Action::Write).is_ok());
    }

    #[test]
    fn test_require_auth_success() {
        let ctx = create_test_context(vec![], false);
        assert!(require_auth(&ctx).is_ok());
    }

    #[test]
    fn test_require_auth_failure() {
        let ctx = AuthContext::anonymous(IpAddr::from([127, 0, 0, 1]));
        assert!(require_auth(&ctx).is_err());
    }

    #[test]
    fn test_require_admin_success() {
        let ctx = create_test_context(vec![], true);
        assert!(require_admin(&ctx).is_ok());
    }

    #[test]
    fn test_require_admin_with_permission() {
        let ctx = create_test_context(vec![Permission::new("admin:*", Action::Admin)], false);
        assert!(require_admin(&ctx).is_ok());
    }

    #[test]
    fn test_require_admin_failure() {
        let ctx = create_test_context(vec![], false);
        assert!(require_admin(&ctx).is_err());
    }
}
