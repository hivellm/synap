//! Hub Authentication Middleware
//!
//! Handles access key extraction and validation for Hub integration mode.
//! Supports hybrid authentication (Hub access keys OR local auth).

use super::client::HubClient;
use super::restrictions::Plan;
use crate::core::error::SynapError;

use axum::{
    extract::{Request, State},
    http::HeaderMap,
    middleware::Next,
    response::Response,
};
use std::sync::Arc;
use tracing::{debug, warn};
use uuid::Uuid;

/// User context extracted from Hub access key
#[derive(Debug, Clone)]
pub struct HubUserContext {
    /// User ID from Hub
    pub user_id: Uuid,
    /// User's subscription plan
    pub plan: Plan,
    /// Original access key (for logging/audit)
    pub access_key: String,
}

impl HubUserContext {
    /// Create a new Hub user context
    pub fn new(user_id: Uuid, plan: Plan, access_key: String) -> Self {
        Self {
            user_id,
            plan,
            access_key,
        }
    }

    /// Get user ID
    pub fn user_id(&self) -> &Uuid {
        &self.user_id
    }

    /// Get user plan
    pub fn plan(&self) -> Plan {
        self.plan
    }
}

/// Extract Hub access key from request headers
///
/// Checks for access key in the following order:
/// 1. `X-Hub-Access-Key` header
/// 2. `Authorization: Bearer <token>` header
/// 3. Query parameter `access_key`
pub fn extract_access_key(headers: &HeaderMap) -> Option<String> {
    // Check X-Hub-Access-Key header
    if let Some(key) = headers.get("x-hub-access-key") {
        if let Ok(key_str) = key.to_str() {
            debug!("Access key found in X-Hub-Access-Key header");
            return Some(key_str.to_string());
        }
    }

    // Check Authorization Bearer token
    if let Some(auth) = headers.get("authorization") {
        if let Ok(auth_str) = auth.to_str() {
            if let Some(token) = auth_str.strip_prefix("Bearer ") {
                debug!("Access key found in Authorization Bearer header");
                return Some(token.to_string());
            }
        }
    }

    None
}

/// Hub authentication middleware
///
/// Validates Hub access keys and adds HubUserContext to request extensions.
/// In hybrid mode, falls back to local auth if no Hub access key is provided.
pub async fn hub_auth_middleware(
    State(hub_client): State<Arc<HubClient>>,
    mut request: Request,
    next: Next,
) -> Result<Response, SynapError> {
    let headers = request.headers();

    // Extract access key from headers
    let access_key = match extract_access_key(headers) {
        Some(key) => key,
        None => {
            // In hybrid mode, allow requests without Hub access keys
            // They will fall back to local authentication
            debug!("No Hub access key found - falling back to local auth");
            return Ok(next.run(request).await);
        }
    };

    // Validate access key with Hub
    match hub_client.validate_access_key(&access_key).await {
        Ok((user_id, plan)) => {
            debug!("Hub access key validated successfully for user {}", user_id);

            // Create user context and add to request extensions
            let user_context = HubUserContext::new(user_id, plan, access_key);
            request.extensions_mut().insert(user_context);

            Ok(next.run(request).await)
        }
        Err(err) => {
            warn!("Hub access key validation failed: {}", err);
            Err(SynapError::Unauthorized(
                "Invalid or expired Hub access key".to_string(),
            ))
        }
    }
}

/// Require Hub authentication - fail if no valid Hub access key
///
/// Use this for endpoints that MUST have Hub authentication
/// (not suitable for hybrid mode)
pub async fn require_hub_auth_middleware(
    State(hub_client): State<Arc<HubClient>>,
    mut request: Request,
    next: Next,
) -> Result<Response, SynapError> {
    let headers = request.headers();

    // Extract access key from headers
    let access_key = extract_access_key(headers).ok_or_else(|| {
        SynapError::Unauthorized("Hub access key required but not provided".to_string())
    })?;

    // Validate access key with Hub
    let (user_id, plan) = hub_client.validate_access_key(&access_key).await?;

    debug!("Hub access key validated successfully for user {}", user_id);

    // Create user context and add to request extensions
    let user_context = HubUserContext::new(user_id, plan, access_key);
    request.extensions_mut().insert(user_context);

    Ok(next.run(request).await)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    #[test]
    fn test_extract_access_key_from_custom_header() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-hub-access-key",
            HeaderValue::from_static("test_access_key_123"),
        );

        let key = extract_access_key(&headers);
        assert_eq!(key, Some("test_access_key_123".to_string()));
    }

    #[test]
    fn test_extract_access_key_from_bearer_token() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "authorization",
            HeaderValue::from_static("Bearer test_token_456"),
        );

        let key = extract_access_key(&headers);
        assert_eq!(key, Some("test_token_456".to_string()));
    }

    #[test]
    fn test_extract_access_key_priority() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-hub-access-key",
            HeaderValue::from_static("custom_header_key"),
        );
        headers.insert(
            "authorization",
            HeaderValue::from_static("Bearer bearer_token"),
        );

        // Custom header should take priority
        let key = extract_access_key(&headers);
        assert_eq!(key, Some("custom_header_key".to_string()));
    }

    #[test]
    fn test_extract_access_key_no_key() {
        let headers = HeaderMap::new();
        let key = extract_access_key(&headers);
        assert_eq!(key, None);
    }

    #[test]
    fn test_extract_access_key_invalid_bearer() {
        let mut headers = HeaderMap::new();
        headers.insert("authorization", HeaderValue::from_static("Basic xyz"));

        let key = extract_access_key(&headers);
        assert_eq!(key, None);
    }

    #[test]
    fn test_hub_user_context_creation() {
        let user_id = Uuid::new_v4();
        let plan = Plan::Pro;
        let access_key = "test_key".to_string();

        let ctx = HubUserContext::new(user_id, plan, access_key.clone());

        assert_eq!(ctx.user_id(), &user_id);
        assert_eq!(ctx.plan(), Plan::Pro);
        assert_eq!(ctx.access_key, access_key);
    }

    #[test]
    fn test_hub_user_context_clone() {
        let user_id = Uuid::new_v4();
        let ctx = HubUserContext::new(user_id, Plan::Free, "key".to_string());
        let ctx_clone = ctx.clone();

        assert_eq!(ctx.user_id(), ctx_clone.user_id());
        assert_eq!(ctx.plan(), ctx_clone.plan());
    }
}
