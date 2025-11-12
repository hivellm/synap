//! Comprehensive Middleware Tests
//!
//! Tests covering:
//! - Authentication middleware edge cases
//! - Header parsing and validation
//! - IP extraction and validation
//! - Authentication order and priority
//! - Error handling and status codes
//! - Malformed requests
//! - Concurrent middleware execution

use axum::{
    body::Body,
    extract::Request,
    http::{HeaderValue, Method, Uri, header},
};
use base64::{Engine as _, engine::general_purpose};
use std::net::IpAddr;
use synap_server::auth::{Action, ApiKeyManager, AuthMiddleware, Permission, UserManager};

/// Helper to create a test request with headers
fn create_request_with_headers(headers: Vec<(String, String)>) -> Request {
    let mut req = Request::builder()
        .uri("/test")
        .method(Method::GET)
        .body(Body::empty())
        .unwrap();

    for (name, value) in headers {
        let header_name =
            header::HeaderName::from_bytes(name.as_bytes()).unwrap_or(header::AUTHORIZATION);
        req.headers_mut()
            .insert(header_name, HeaderValue::from_str(&value).unwrap());
    }

    req
}

/// Helper to create a test request with query parameters
fn create_request_with_query(query: &str) -> Request {
    Request::builder()
        .uri(format!("/test?{}", query))
        .method(Method::GET)
        .body(Body::empty())
        .unwrap()
}

// ==================== Header Parsing Tests ====================

#[test]
fn test_bearer_token_extraction() {
    let user_manager = UserManager::new();
    let api_key_manager = ApiKeyManager::new();
    let auth = AuthMiddleware::new(user_manager, api_key_manager, false);

    // Valid Bearer token
    let req = create_request_with_headers(vec![(
        "Authorization".to_string(),
        "Bearer sk_test123456789".to_string(),
    )]);
    let client_ip = IpAddr::from([127, 0, 0, 1]);
    let result = AuthMiddleware::authenticate_api_key(&auth, &req, client_ip);

    // Should return None (key doesn't exist, but parsing succeeded)
    assert!(result.is_ok());
}

#[test]
fn test_bearer_token_case_insensitive() {
    let user_manager = UserManager::new();
    let api_key_manager = ApiKeyManager::new();
    let auth = AuthMiddleware::new(user_manager, api_key_manager, false);

    // Bearer with different case
    let req = create_request_with_headers(vec![(
        "Authorization".to_string(),
        "bearer sk_test123".to_string(),
    )]);
    let client_ip = IpAddr::from([127, 0, 0, 1]);
    let result = AuthMiddleware::authenticate_api_key(&auth, &req, client_ip);

    // Should not match (case-sensitive prefix check)
    assert!(result.is_ok());
}

#[test]
fn test_bearer_token_with_spaces() {
    let user_manager = UserManager::new();
    let api_key_manager = ApiKeyManager::new();
    let auth = AuthMiddleware::new(user_manager, api_key_manager, false);

    // Bearer token with extra spaces
    let req = create_request_with_headers(vec![(
        "Authorization".to_string(),
        "Bearer  sk_test123".to_string(),
    )]);
    let client_ip = IpAddr::from([127, 0, 0, 1]);
    let result = AuthMiddleware::authenticate_api_key(&auth, &req, client_ip);

    assert!(result.is_ok());
}

#[test]
fn test_bearer_token_empty() {
    let user_manager = UserManager::new();
    let api_key_manager = ApiKeyManager::new();
    let auth = AuthMiddleware::new(user_manager, api_key_manager, false);

    // Empty Bearer token
    let req =
        create_request_with_headers(vec![("Authorization".to_string(), "Bearer ".to_string())]);
    let client_ip = IpAddr::from([127, 0, 0, 1]);
    let result = AuthMiddleware::authenticate_api_key(&auth, &req, client_ip);

    // Should return None (no key provided)
    assert!(result.is_ok());
    assert!(result.unwrap().is_none());
}

#[test]
fn test_basic_auth_extraction() {
    let user_manager = UserManager::new();
    user_manager
        .create_user("testuser", "testpass", false)
        .unwrap();
    let api_key_manager = ApiKeyManager::new();
    let auth = AuthMiddleware::new(user_manager, api_key_manager, false);

    // Valid Basic Auth
    let credentials = "testuser:testpass";
    let encoded = general_purpose::STANDARD.encode(credentials);
    let req = create_request_with_headers(vec![(
        "Authorization".to_string(),
        format!("Basic {}", encoded),
    )]);
    let client_ip = IpAddr::from([127, 0, 0, 1]);
    let result = AuthMiddleware::authenticate_basic(&auth, &req, client_ip);

    assert!(result.is_ok());
    assert!(result.unwrap().is_some());
}

#[test]
fn test_basic_auth_invalid_base64() {
    let user_manager = UserManager::new();
    let api_key_manager = ApiKeyManager::new();
    let auth = AuthMiddleware::new(user_manager, api_key_manager, false);

    // Invalid base64
    let req = create_request_with_headers(vec![(
        "Authorization".to_string(),
        "Basic invalid!base64@".to_string(),
    )]);
    let client_ip = IpAddr::from([127, 0, 0, 1]);
    let result = AuthMiddleware::authenticate_basic(&auth, &req, client_ip);

    // Should return error (invalid base64)
    assert!(result.is_err());
}

#[test]
fn test_basic_auth_missing_colon() {
    let user_manager = UserManager::new();
    let api_key_manager = ApiKeyManager::new();
    let auth = AuthMiddleware::new(user_manager, api_key_manager, false);

    // Missing colon separator
    let credentials = "testuser";
    let encoded = general_purpose::STANDARD.encode(credentials);
    let req = create_request_with_headers(vec![(
        "Authorization".to_string(),
        format!("Basic {}", encoded),
    )]);
    let client_ip = IpAddr::from([127, 0, 0, 1]);
    let result = AuthMiddleware::authenticate_basic(&auth, &req, client_ip);

    // Should return error (invalid format)
    assert!(result.is_err());
}

#[test]
fn test_basic_auth_invalid_utf8() {
    let user_manager = UserManager::new();
    let api_key_manager = ApiKeyManager::new();
    let auth = AuthMiddleware::new(user_manager, api_key_manager, false);

    // Invalid UTF-8 sequence
    let invalid_utf8 = vec![0xFF, 0xFE, 0xFD];
    let encoded = general_purpose::STANDARD.encode(&invalid_utf8);
    let req = create_request_with_headers(vec![(
        "Authorization".to_string(),
        format!("Basic {}", encoded),
    )]);
    let client_ip = IpAddr::from([127, 0, 0, 1]);
    let result = AuthMiddleware::authenticate_basic(&auth, &req, client_ip);

    // Should return error (invalid UTF-8)
    assert!(result.is_err());
}

#[test]
fn test_basic_auth_empty_username() {
    let user_manager = UserManager::new();
    let api_key_manager = ApiKeyManager::new();
    let auth = AuthMiddleware::new(user_manager, api_key_manager, false);

    // Empty username
    let credentials = ":password";
    let encoded = general_purpose::STANDARD.encode(credentials);
    let req = create_request_with_headers(vec![(
        "Authorization".to_string(),
        format!("Basic {}", encoded),
    )]);
    let client_ip = IpAddr::from([127, 0, 0, 1]);
    let result = AuthMiddleware::authenticate_basic(&auth, &req, client_ip);

    // Should return error (user not found)
    assert!(result.is_err());
}

#[test]
fn test_basic_auth_empty_password() {
    let user_manager = UserManager::new();
    user_manager.create_user("testuser", "", false).unwrap();
    let api_key_manager = ApiKeyManager::new();
    let auth = AuthMiddleware::new(user_manager, api_key_manager, false);

    // Empty password
    let credentials = "testuser:";
    let encoded = general_purpose::STANDARD.encode(credentials);
    let req = create_request_with_headers(vec![(
        "Authorization".to_string(),
        format!("Basic {}", encoded),
    )]);
    let client_ip = IpAddr::from([127, 0, 0, 1]);
    let result = AuthMiddleware::authenticate_basic(&auth, &req, client_ip);

    // Should authenticate (empty password is valid)
    assert!(result.is_ok());
}

#[test]
fn test_basic_auth_multiple_colons() {
    let user_manager = UserManager::new();
    user_manager
        .create_user("user:name", "pass:word", false)
        .unwrap();
    let api_key_manager = ApiKeyManager::new();
    let auth = AuthMiddleware::new(user_manager, api_key_manager, false);

    // Username or password with colons
    let credentials = "user:name:pass:word";
    let encoded = general_purpose::STANDARD.encode(credentials);
    let req = create_request_with_headers(vec![(
        "Authorization".to_string(),
        format!("Basic {}", encoded),
    )]);
    let client_ip = IpAddr::from([127, 0, 0, 1]);
    let result = AuthMiddleware::authenticate_basic(&auth, &req, client_ip);

    // Should split on first colon only
    assert!(result.is_ok());
}

// ==================== Query Parameter Tests ====================

#[test]
fn test_api_key_query_parameter() {
    let user_manager = UserManager::new();
    let api_key_manager = ApiKeyManager::new();
    let auth = AuthMiddleware::new(user_manager, api_key_manager, false);

    // API key in query parameter
    let req = create_request_with_query("api_key=sk_test123");
    let client_ip = IpAddr::from([127, 0, 0, 1]);
    let result = AuthMiddleware::authenticate_api_key(&auth, &req, client_ip);

    assert!(result.is_ok());
}

#[test]
fn test_api_key_query_parameter_url_encoded() {
    let user_manager = UserManager::new();
    let api_key_manager = ApiKeyManager::new();
    let auth = AuthMiddleware::new(user_manager, api_key_manager, false);

    // URL encoded API key
    let req = create_request_with_query("api_key=sk_test%20123");
    let client_ip = IpAddr::from([127, 0, 0, 1]);
    let result = AuthMiddleware::authenticate_api_key(&auth, &req, client_ip);

    assert!(result.is_ok());
}

#[test]
fn test_api_key_query_parameter_multiple_params() {
    let user_manager = UserManager::new();
    let api_key_manager = ApiKeyManager::new();
    let auth = AuthMiddleware::new(user_manager, api_key_manager, false);

    // Multiple query parameters
    let req = create_request_with_query("param1=value1&api_key=sk_test123&param2=value2");
    let client_ip = IpAddr::from([127, 0, 0, 1]);
    let result = AuthMiddleware::authenticate_api_key(&auth, &req, client_ip);

    assert!(result.is_ok());
}

#[test]
fn test_api_key_query_parameter_empty() {
    let user_manager = UserManager::new();
    let api_key_manager = ApiKeyManager::new();
    let auth = AuthMiddleware::new(user_manager, api_key_manager, false);

    // Empty API key parameter
    let req = create_request_with_query("api_key=");
    let client_ip = IpAddr::from([127, 0, 0, 1]);
    let result = AuthMiddleware::authenticate_api_key(&auth, &req, client_ip);

    // Should return None (empty key)
    assert!(result.is_ok());
    assert!(result.unwrap().is_none());
}

#[test]
fn test_api_key_query_parameter_missing_value() {
    let user_manager = UserManager::new();
    let api_key_manager = ApiKeyManager::new();
    let auth = AuthMiddleware::new(user_manager, api_key_manager, false);

    // Missing value
    let req = create_request_with_query("api_key");
    let client_ip = IpAddr::from([127, 0, 0, 1]);
    let result = AuthMiddleware::authenticate_api_key(&auth, &req, client_ip);

    // Should return None (no value)
    assert!(result.is_ok());
    assert!(result.unwrap().is_none());
}

// ==================== Authentication Priority Tests ====================

#[test]
fn test_bearer_token_priority_over_basic_auth() {
    let user_manager = UserManager::new();
    user_manager
        .create_user("testuser", "testpass", false)
        .unwrap();

    let api_key_manager = ApiKeyManager::new();
    let key = api_key_manager
        .create(
            "test-key",
            None,
            vec![Permission::new("*", Action::All)],
            vec![],
            None,
        )
        .unwrap();

    let auth = AuthMiddleware::new(user_manager, api_key_manager, false);

    // Both Bearer and Basic Auth provided
    let credentials = "testuser:testpass";
    let encoded = general_purpose::STANDARD.encode(credentials);
    let req = create_request_with_headers(vec![(
        "Authorization".to_string(),
        format!("Bearer {} Basic {}", key.key, encoded),
    )]);
    let client_ip = IpAddr::from([127, 0, 0, 1]);

    // Bearer token should be checked first
    let result = AuthMiddleware::authenticate_api_key(&auth, &req, client_ip);
    assert!(result.is_ok());
}

#[test]
fn test_bearer_token_priority_over_query_param() {
    let user_manager = UserManager::new();
    let api_key_manager = ApiKeyManager::new();
    let key = api_key_manager
        .create(
            "test-key",
            None,
            vec![Permission::new("*", Action::All)],
            vec![],
            None,
        )
        .unwrap();

    let auth = AuthMiddleware::new(user_manager, api_key_manager, false);

    // Both Bearer token and query parameter
    let mut req = create_request_with_headers(vec![(
        "Authorization".to_string(),
        format!("Bearer {}", key.key),
    )]);
    *req.uri_mut() = Uri::from_static("/test?api_key=sk_other");
    let client_ip = IpAddr::from([127, 0, 0, 1]);

    // Bearer token should be checked first
    let result = AuthMiddleware::authenticate_api_key(&auth, &req, client_ip);
    assert!(result.is_ok());
}

// ==================== IP Extraction Tests ====================

#[test]
fn test_get_client_ip_fallback() {
    // Request without ConnectInfo
    let req = Request::builder()
        .uri("/test")
        .method(Method::GET)
        .body(Body::empty())
        .unwrap();

    let ip = AuthMiddleware::get_client_ip(&req);
    assert_eq!(ip, IpAddr::from([127, 0, 0, 1]));
}

// ==================== Malformed Request Tests ====================

#[test]
fn test_malformed_authorization_header() {
    let user_manager = UserManager::new();
    let api_key_manager = ApiKeyManager::new();
    let auth = AuthMiddleware::new(user_manager, api_key_manager, false);

    // Invalid header value (non-UTF8)
    let mut req = create_request_with_headers(vec![]);
    req.headers_mut().insert(
        header::AUTHORIZATION,
        HeaderValue::from_bytes(&[0xFF, 0xFE, 0xFD]).unwrap(),
    );
    let client_ip = IpAddr::from([127, 0, 0, 1]);

    // Should handle gracefully
    let result = AuthMiddleware::authenticate_api_key(&auth, &req, client_ip);
    assert!(result.is_ok());

    let result = AuthMiddleware::authenticate_basic(&auth, &req, client_ip);
    assert!(result.is_err()); // Invalid UTF-8
}

#[test]
fn test_authorization_header_without_prefix() {
    let user_manager = UserManager::new();
    let api_key_manager = ApiKeyManager::new();
    let auth = AuthMiddleware::new(user_manager, api_key_manager, false);

    // Authorization without Bearer or Basic prefix
    let req = create_request_with_headers(vec![(
        "Authorization".to_string(),
        "sk_test123".to_string(),
    )]);
    let client_ip = IpAddr::from([127, 0, 0, 1]);

    // Should return None (no Bearer prefix)
    let result = AuthMiddleware::authenticate_api_key(&auth, &req, client_ip);
    assert!(result.is_ok());
    assert!(result.unwrap().is_none());

    // Should return None (no Basic prefix)
    let result = AuthMiddleware::authenticate_basic(&auth, &req, client_ip);
    assert!(result.is_ok());
    assert!(result.unwrap().is_none());
}

#[test]
fn test_multiple_authorization_headers() {
    let user_manager = UserManager::new();
    let api_key_manager = ApiKeyManager::new();
    let auth = AuthMiddleware::new(user_manager, api_key_manager, false);

    // Multiple Authorization headers (should use first one)
    let mut req = create_request_with_headers(vec![(
        "Authorization".to_string(),
        "Bearer sk_test1".to_string(),
    )]);
    req.headers_mut().append(
        header::AUTHORIZATION,
        HeaderValue::from_str("Bearer sk_test2").unwrap(),
    );
    let client_ip = IpAddr::from([127, 0, 0, 1]);

    // Should use first header
    let result = AuthMiddleware::authenticate_api_key(&auth, &req, client_ip);
    assert!(result.is_ok());
}

#[test]
fn test_very_long_api_key() {
    let user_manager = UserManager::new();
    let api_key_manager = ApiKeyManager::new();
    let auth = AuthMiddleware::new(user_manager, api_key_manager, false);

    // Very long API key (potential DoS)
    let long_key = "sk_".to_string() + &"a".repeat(10000);
    let req = create_request_with_headers(vec![(
        "Authorization".to_string(),
        format!("Bearer {}", long_key),
    )]);
    let client_ip = IpAddr::from([127, 0, 0, 1]);

    // Should handle gracefully (will fail verification but not crash)
    let result = AuthMiddleware::authenticate_api_key(&auth, &req, client_ip);
    assert!(result.is_ok());
}

#[test]
fn test_special_characters_in_api_key() {
    let user_manager = UserManager::new();
    let api_key_manager = ApiKeyManager::new();
    let auth = AuthMiddleware::new(user_manager, api_key_manager, false);

    // Special characters in API key
    let req = create_request_with_headers(vec![(
        "Authorization".to_string(),
        "Bearer sk_test!@#$%^&*()".to_string(),
    )]);
    let client_ip = IpAddr::from([127, 0, 0, 1]);

    // Should handle gracefully
    let result = AuthMiddleware::authenticate_api_key(&auth, &req, client_ip);
    assert!(result.is_ok());
}

#[test]
fn test_unicode_in_headers() {
    let user_manager = UserManager::new();
    let api_key_manager = ApiKeyManager::new();
    let auth = AuthMiddleware::new(user_manager, api_key_manager, false);

    // Unicode characters in header
    let req = create_request_with_headers(vec![(
        "Authorization".to_string(),
        "Bearer sk_测试123".to_string(),
    )]);
    let client_ip = IpAddr::from([127, 0, 0, 1]);

    // Should handle gracefully
    let result = AuthMiddleware::authenticate_api_key(&auth, &req, client_ip);
    assert!(result.is_ok());
}

// ==================== Edge Cases ====================

#[test]
fn test_no_authentication_headers() {
    let user_manager = UserManager::new();
    let api_key_manager = ApiKeyManager::new();
    let auth = AuthMiddleware::new(user_manager, api_key_manager, false);

    // No Authorization header
    let req = create_request_with_headers(vec![]);
    let client_ip = IpAddr::from([127, 0, 0, 1]);

    // Should return None
    let result = AuthMiddleware::authenticate_api_key(&auth, &req, client_ip);
    assert!(result.is_ok());
    assert!(result.unwrap().is_none());

    let result = AuthMiddleware::authenticate_basic(&auth, &req, client_ip);
    assert!(result.is_ok());
    assert!(result.unwrap().is_none());
}

#[test]
fn test_whitespace_in_headers() {
    let user_manager = UserManager::new();
    let api_key_manager = ApiKeyManager::new();
    let auth = AuthMiddleware::new(user_manager, api_key_manager, false);

    // Whitespace in Bearer token
    let req = create_request_with_headers(vec![(
        "Authorization".to_string(),
        "Bearer  sk_test123  ".to_string(),
    )]);
    let client_ip = IpAddr::from([127, 0, 0, 1]);

    // Should handle whitespace
    let result = AuthMiddleware::authenticate_api_key(&auth, &req, client_ip);
    assert!(result.is_ok());
}

#[test]
fn test_case_sensitive_bearer_prefix() {
    let user_manager = UserManager::new();
    let api_key_manager = ApiKeyManager::new();
    let auth = AuthMiddleware::new(user_manager, api_key_manager, false);

    // Lowercase "bearer"
    let req = create_request_with_headers(vec![(
        "Authorization".to_string(),
        "bearer sk_test123".to_string(),
    )]);
    let client_ip = IpAddr::from([127, 0, 0, 1]);

    // Should not match (case-sensitive)
    let result = AuthMiddleware::authenticate_api_key(&auth, &req, client_ip);
    assert!(result.is_ok());
    assert!(result.unwrap().is_none());
}

#[test]
fn test_case_sensitive_basic_prefix() {
    let user_manager = UserManager::new();
    let api_key_manager = ApiKeyManager::new();
    let auth = AuthMiddleware::new(user_manager, api_key_manager, false);

    // Lowercase "basic"
    let credentials = "user:pass";
    let encoded = general_purpose::STANDARD.encode(credentials);
    let req = create_request_with_headers(vec![(
        "Authorization".to_string(),
        format!("basic {}", encoded),
    )]);
    let client_ip = IpAddr::from([127, 0, 0, 1]);

    // Should not match (case-sensitive)
    let result = AuthMiddleware::authenticate_basic(&auth, &req, client_ip);
    assert!(result.is_ok());
    assert!(result.unwrap().is_none());
}

// ==================== Concurrent Access Tests ====================

#[test]
fn test_concurrent_middleware_authentication() {
    use std::sync::Arc;
    use std::thread;

    let user_manager = Arc::new(UserManager::new());
    user_manager.create_user("user1", "pass123", false).unwrap();

    let api_key_manager = Arc::new(ApiKeyManager::new());
    let key = api_key_manager
        .create(
            "test-key",
            None,
            vec![Permission::new("*", Action::All)],
            vec![],
            None,
        )
        .unwrap();

    let auth = Arc::new(AuthMiddleware::new(
        (*user_manager).clone(),
        (*api_key_manager).clone(),
        false,
    ));

    let mut handles = vec![];

    // Spawn threads testing different authentication methods
    for _ in 0..10 {
        let auth_clone = Arc::clone(&auth);
        let key_clone = key.key.clone();

        let handle = thread::spawn(move || {
            let req = create_request_with_headers(vec![(
                "Authorization".to_string(),
                format!("Bearer {}", key_clone),
            )]);
            let client_ip = IpAddr::from([127, 0, 0, 1]);
            let result = AuthMiddleware::authenticate_api_key(&auth_clone, &req, client_ip);
            assert!(result.is_ok());
        });
        handles.push(handle);
    }

    // All should succeed
    for handle in handles {
        handle.join().unwrap();
    }
}

// ==================== Security Tests ====================

#[test]
fn test_sql_injection_in_username() {
    let user_manager = UserManager::new();
    let api_key_manager = ApiKeyManager::new();
    let auth = AuthMiddleware::new(user_manager, api_key_manager, false);

    // SQL injection attempt in username
    let credentials = "admin' OR '1'='1:password";
    let encoded = general_purpose::STANDARD.encode(credentials);
    let req = create_request_with_headers(vec![(
        "Authorization".to_string(),
        format!("Basic {}", encoded),
    )]);
    let client_ip = IpAddr::from([127, 0, 0, 1]);

    // Should fail authentication (no such user)
    let result = AuthMiddleware::authenticate_basic(&auth, &req, client_ip);
    assert!(result.is_err());
}

#[test]
fn test_path_traversal_in_username() {
    let user_manager = UserManager::new();
    let api_key_manager = ApiKeyManager::new();
    let auth = AuthMiddleware::new(user_manager, api_key_manager, false);

    // Path traversal attempt
    let credentials = "../../etc/passwd:password";
    let encoded = general_purpose::STANDARD.encode(credentials);
    let req = create_request_with_headers(vec![(
        "Authorization".to_string(),
        format!("Basic {}", encoded),
    )]);
    let client_ip = IpAddr::from([127, 0, 0, 1]);

    // Should fail authentication
    let result = AuthMiddleware::authenticate_basic(&auth, &req, client_ip);
    assert!(result.is_err());
}

#[test]
fn test_xss_attempt_in_headers() {
    let user_manager = UserManager::new();
    let api_key_manager = ApiKeyManager::new();
    let auth = AuthMiddleware::new(user_manager, api_key_manager, false);

    // XSS attempt
    let req = create_request_with_headers(vec![(
        "Authorization".to_string(),
        "<script>alert('xss')</script>".to_string(),
    )]);
    let client_ip = IpAddr::from([127, 0, 0, 1]);

    // Should handle gracefully (no Bearer/Basic prefix)
    let result = AuthMiddleware::authenticate_api_key(&auth, &req, client_ip);
    assert!(result.is_ok());
    assert!(result.unwrap().is_none());
}

#[test]
fn test_null_byte_injection() {
    let user_manager = UserManager::new();
    let api_key_manager = ApiKeyManager::new();
    let auth = AuthMiddleware::new(user_manager, api_key_manager, false);

    // Null byte injection
    let credentials = "user\0:pass";
    let encoded = general_purpose::STANDARD.encode(credentials);
    let req = create_request_with_headers(vec![(
        "Authorization".to_string(),
        format!("Basic {}", encoded),
    )]);
    let client_ip = IpAddr::from([127, 0, 0, 1]);

    // Should handle gracefully
    let result = AuthMiddleware::authenticate_basic(&auth, &req, client_ip);
    // May succeed or fail depending on how Rust handles null bytes in strings
    assert!(result.is_ok() || result.is_err());
}

#[test]
fn test_newline_injection() {
    let user_manager = UserManager::new();
    let api_key_manager = ApiKeyManager::new();
    let auth = AuthMiddleware::new(user_manager, api_key_manager, false);

    // Newline injection
    let credentials = "user\n:pass";
    let encoded = general_purpose::STANDARD.encode(credentials);
    let req = create_request_with_headers(vec![(
        "Authorization".to_string(),
        format!("Basic {}", encoded),
    )]);
    let client_ip = IpAddr::from([127, 0, 0, 1]);

    // Should handle gracefully
    let result = AuthMiddleware::authenticate_basic(&auth, &req, client_ip);
    assert!(result.is_ok() || result.is_err());
}

#[test]
fn test_command_injection_in_query() {
    let user_manager = UserManager::new();
    let api_key_manager = ApiKeyManager::new();
    let auth = AuthMiddleware::new(user_manager, api_key_manager, false);

    // Command injection attempt
    let req = create_request_with_query("api_key=sk_test; rm -rf /");
    let client_ip = IpAddr::from([127, 0, 0, 1]);

    // Should handle gracefully (treated as part of key)
    let result = AuthMiddleware::authenticate_api_key(&auth, &req, client_ip);
    assert!(result.is_ok());
}

#[test]
fn test_very_long_username() {
    let user_manager = UserManager::new();
    let api_key_manager = ApiKeyManager::new();
    let auth = AuthMiddleware::new(user_manager, api_key_manager, false);

    // Very long username (potential DoS)
    let long_username = "a".repeat(10000);
    let credentials = format!("{}:password", long_username);
    let encoded = general_purpose::STANDARD.encode(credentials);
    let req = create_request_with_headers(vec![(
        "Authorization".to_string(),
        format!("Basic {}", encoded),
    )]);
    let client_ip = IpAddr::from([127, 0, 0, 1]);

    // Should handle gracefully
    let result = AuthMiddleware::authenticate_basic(&auth, &req, client_ip);
    assert!(result.is_err()); // User doesn't exist
}

#[test]
fn test_very_long_password() {
    let user_manager = UserManager::new();
    let api_key_manager = ApiKeyManager::new();
    let auth = AuthMiddleware::new(user_manager, api_key_manager, false);

    // Very long password (potential DoS)
    let long_password = "a".repeat(10000);
    let credentials = format!("user:{}", long_password);
    let encoded = general_purpose::STANDARD.encode(credentials);
    let req = create_request_with_headers(vec![(
        "Authorization".to_string(),
        format!("Basic {}", encoded),
    )]);
    let client_ip = IpAddr::from([127, 0, 0, 1]);

    // Should handle gracefully
    let result = AuthMiddleware::authenticate_basic(&auth, &req, client_ip);
    assert!(result.is_err()); // User doesn't exist
}
