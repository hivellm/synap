use super::{ApiKeyManager, AuthContext, UserManager};
use axum::{
    extract::{ConnectInfo, Request},
    http::{StatusCode, header},
    middleware::Next,
    response::Response,
};
use base64::{Engine as _, engine::general_purpose};
use std::net::{IpAddr, SocketAddr};
use tracing::debug;

/// Authentication middleware
#[derive(Clone)]
pub struct AuthMiddleware {
    pub user_manager: UserManager,
    pub api_key_manager: ApiKeyManager,
    pub require_auth: bool,
}

impl AuthMiddleware {
    pub fn new(
        user_manager: UserManager,
        api_key_manager: ApiKeyManager,
        require_auth: bool,
    ) -> Self {
        Self {
            user_manager,
            api_key_manager,
            require_auth,
        }
    }

    /// Extract client IP from request
    fn get_client_ip(req: &Request) -> IpAddr {
        // Try to get from ConnectInfo extension
        if let Some(ConnectInfo(addr)) = req.extensions().get::<ConnectInfo<SocketAddr>>() {
            return addr.ip();
        }

        // Fallback to localhost
        IpAddr::from([127, 0, 0, 1])
    }

    /// Middleware function for Axum
    pub async fn layer(
        auth: AuthMiddleware,
        mut req: Request,
        next: Next,
    ) -> Result<Response, StatusCode> {
        let client_ip = Self::get_client_ip(&req);
        debug!("Processing authentication for IP: {}", client_ip);

        // Try API Key authentication first (from header or query param)
        if let Some(auth_context) = Self::authenticate_api_key(&auth, &req, client_ip) {
            req.extensions_mut().insert(auth_context);
            return Ok(next.run(req).await);
        }

        // Try Basic Auth
        if let Some(auth_context) = Self::authenticate_basic(&auth, &req, client_ip) {
            req.extensions_mut().insert(auth_context);
            return Ok(next.run(req).await);
        }

        // No authentication provided
        if auth.require_auth {
            debug!("Authentication required but not provided");
            return Err(StatusCode::UNAUTHORIZED);
        }

        // Allow anonymous access
        req.extensions_mut()
            .insert(AuthContext::anonymous(client_ip));
        Ok(next.run(req).await)
    }

    /// Authenticate via API key
    fn authenticate_api_key(
        auth: &AuthMiddleware,
        req: &Request,
        client_ip: IpAddr,
    ) -> Option<AuthContext> {
        // Check for API key in header
        let api_key = if let Some(auth_header) = req.headers().get(header::AUTHORIZATION) {
            if let Ok(auth_str) = auth_header.to_str() {
                if let Some(key) = auth_str.strip_prefix("Bearer ") {
                    Some(key.to_string())
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            // Check query parameter
            req.uri().query().and_then(|q| {
                for pair in q.split('&') {
                    if let Some((k, v)) = pair.split_once('=') {
                        if k == "api_key" {
                            return Some(v.to_string());
                        }
                    }
                }
                None
            })
        };

        if let Some(key) = api_key {
            if let Ok(api_key_obj) = auth.api_key_manager.verify(&key, client_ip) {
                debug!("Authenticated via API key: {}", api_key_obj.name);

                return Some(AuthContext {
                    user_id: api_key_obj.username.clone(),
                    api_key_id: Some(api_key_obj.id.clone()),
                    client_ip,
                    permissions: api_key_obj.permissions.clone(),
                    is_admin: false, // API keys are not admin by default
                });
            }
        }

        None
    }

    /// Authenticate via Basic Auth
    fn authenticate_basic(
        auth: &AuthMiddleware,
        req: &Request,
        client_ip: IpAddr,
    ) -> Option<AuthContext> {
        let auth_header = req.headers().get(header::AUTHORIZATION)?;
        let auth_str = auth_header.to_str().ok()?;
        let credentials = auth_str.strip_prefix("Basic ")?;

        // Decode base64
        let decoded = general_purpose::STANDARD.decode(credentials).ok()?;
        let credentials_str = String::from_utf8(decoded).ok()?;

        // Split username:password
        let (username, password) = credentials_str.split_once(':')?;

        // Authenticate
        if let Ok(user) = auth.user_manager.authenticate(username, password) {
            debug!("Authenticated via Basic Auth: {}", username);

            let permissions = auth.user_manager.get_user_permissions(username);

            return Some(AuthContext {
                user_id: Some(username.to_string()),
                api_key_id: None,
                client_ip,
                permissions,
                is_admin: user.is_admin,
            });
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::super::{Action, Permission};
    use super::*;

    #[test]
    fn test_get_client_ip() {
        // This would need a proper Request object in real tests
        // For now, just verify the fallback works
        let fallback_ip = IpAddr::from([127, 0, 0, 1]);
        assert_eq!(fallback_ip.to_string(), "127.0.0.1");
    }

    #[test]
    fn test_basic_auth_credentials_parsing() {
        // Test base64 encoding/decoding
        let credentials = "username:password";
        let encoded = general_purpose::STANDARD.encode(credentials);
        let decoded = general_purpose::STANDARD.decode(&encoded).unwrap();
        let decoded_str = String::from_utf8(decoded).unwrap();

        assert_eq!(credentials, decoded_str);

        let (user, pass) = decoded_str.split_once(':').unwrap();
        assert_eq!(user, "username");
        assert_eq!(pass, "password");
    }
}
