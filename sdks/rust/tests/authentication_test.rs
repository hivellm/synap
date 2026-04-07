//! Authentication tests for Synap Rust SDK

mod common;

#[cfg(test)]
mod tests {
    use super::common::setup_test_client;
    use mockito::Matcher;
    use serde_json::json;
    use std::time::Duration;
    use synap_sdk::client::{SynapClient, SynapConfig};

    const TEST_URL: &str = "http://localhost:15500";
    const TEST_USERNAME: &str = "root";
    const TEST_PASSWORD: &str = "root";

    #[tokio::test]
    async fn test_basic_auth_config_creation() {
        let config = SynapConfig::new(TEST_URL).with_basic_auth(TEST_USERNAME, TEST_PASSWORD);
        assert_eq!(config.username, Some(TEST_USERNAME.to_string()));
        assert_eq!(config.password, Some(TEST_PASSWORD.to_string()));
        assert_eq!(config.auth_token, None);
    }

    #[tokio::test]
    async fn test_api_key_config_creation() {
        let config = SynapConfig::new(TEST_URL).with_auth_token("sk_test123");
        assert_eq!(config.auth_token, Some("sk_test123".to_string()));
        assert_eq!(config.username, None);
        assert_eq!(config.password, None);
    }

    #[tokio::test]
    async fn test_config_builder_pattern() {
        let config = SynapConfig::new(TEST_URL)
            .with_timeout(Duration::from_secs(60))
            .with_basic_auth("user", "pass");
        assert_eq!(config.timeout, Duration::from_secs(60));
        assert_eq!(config.username, Some("user".to_string()));
        assert_eq!(config.password, Some("pass".to_string()));
    }

    #[tokio::test]
    async fn test_config_auth_token_overrides_basic_auth() {
        let config = SynapConfig::new(TEST_URL)
            .with_basic_auth("user", "pass")
            .with_auth_token("sk_test123");
        assert_eq!(config.auth_token, Some("sk_test123".to_string()));
        assert_eq!(config.username, None);
        assert_eq!(config.password, None);
    }

    #[tokio::test]
    async fn test_config_basic_auth_overrides_auth_token() {
        let config = SynapConfig::new(TEST_URL)
            .with_auth_token("sk_test123")
            .with_basic_auth("user", "pass");
        assert_eq!(config.username, Some("user".to_string()));
        assert_eq!(config.password, Some("pass".to_string()));
        assert_eq!(config.auth_token, None);
    }

    #[tokio::test]
    async fn test_basic_auth_headers_sent() {
        let (_client, mut server) = setup_test_client().await;

        let _mock = server
            .mock("POST", "/api/v1/command")
            .match_header("authorization", Matcher::Regex(r"^Basic .+$".to_string()))
            .match_body(Matcher::PartialJson(json!({
                "command": "kv.set",
                "payload": {
                    "key": "test",
                    "value": "value"
                }
            })))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"success": true, "payload": {}}"#)
            .create_async()
            .await;

        // Create config with Basic Auth
        let config = SynapConfig::new(server.url()).with_basic_auth(TEST_USERNAME, TEST_PASSWORD);
        let auth_client = SynapClient::new(config).expect("Failed to create client");

        // Test that client sends Basic Auth header
        let result = auth_client.kv().set("test", "value", None).await;
        assert!(result.is_ok());
        _mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_api_key_headers_sent() {
        let (_client, mut server) = setup_test_client().await;

        let _mock = server
            .mock("POST", "/api/v1/command")
            .match_header("authorization", Matcher::Regex(r"^Bearer .+$".to_string()))
            .match_body(Matcher::PartialJson(json!({
                "command": "kv.set",
                "payload": {
                    "key": "test",
                    "value": "value"
                }
            })))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"success": true, "payload": {}}"#)
            .create_async()
            .await;

        // Create config with API Key
        let config = SynapConfig::new(server.url()).with_auth_token("sk_test123");
        let auth_client = SynapClient::new(config).expect("Failed to create client");

        // Test that client sends Bearer Token header
        let result = auth_client.kv().set("test", "value", None).await;
        assert!(result.is_ok());
        _mock.assert_async().await;
    }
}
