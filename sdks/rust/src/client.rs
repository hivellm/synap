//! Synap client implementation

use crate::error::{Result, SynapError};
use crate::{KVStore, PubSubManager, QueueManager, StreamManager};
use reqwest::Client;
use serde_json::Value;
use std::sync::Arc;
use std::time::Duration;
use url::Url;

/// Synap client configuration
#[derive(Debug, Clone)]
pub struct SynapConfig {
    /// Base URL of the Synap server
    pub base_url: String,
    /// Request timeout
    pub timeout: Duration,
    /// Maximum retry attempts
    pub max_retries: u32,
    /// Optional authentication token
    pub auth_token: Option<String>,
}

impl SynapConfig {
    /// Create a new configuration with the given base URL
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            timeout: Duration::from_secs(30),
            max_retries: 3,
            auth_token: None,
        }
    }

    /// Set the timeout for requests
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Set the authentication token
    pub fn with_auth_token(mut self, token: impl Into<String>) -> Self {
        self.auth_token = Some(token.into());
        self
    }

    /// Set the maximum retry attempts
    pub fn with_max_retries(mut self, max_retries: u32) -> Self {
        self.max_retries = max_retries;
        self
    }
}

/// Main Synap client
#[derive(Clone)]
pub struct SynapClient {
    config: Arc<SynapConfig>,
    http_client: Client,
    base_url: Url,
}

impl SynapClient {
    /// Create a new Synap client
    pub fn new(config: SynapConfig) -> Result<Self> {
        let base_url = Url::parse(&config.base_url)?;

        let mut http_client_builder = Client::builder().timeout(config.timeout);

        if let Some(ref token) = config.auth_token {
            let mut headers = reqwest::header::HeaderMap::new();
            headers.insert(
                reqwest::header::AUTHORIZATION,
                format!("Bearer {}", token).parse().unwrap(),
            );
            http_client_builder = http_client_builder.default_headers(headers);
        }

        let http_client = http_client_builder.build()?;

        Ok(Self {
            config: Arc::new(config),
            http_client,
            base_url,
        })
    }

    /// Get the Key-Value store interface
    pub fn kv(&self) -> KVStore {
        KVStore::new(self.clone())
    }

    /// Get the Queue manager interface
    pub fn queue(&self) -> QueueManager {
        QueueManager::new(self.clone())
    }

    /// Get the Stream manager interface
    pub fn stream(&self) -> StreamManager {
        StreamManager::new(self.clone())
    }

    /// Get the Pub/Sub manager interface
    pub fn pubsub(&self) -> PubSubManager {
        PubSubManager::new(self.clone())
    }

    /// Send a POST request
    pub(crate) async fn post(&self, path: &str, body: Value) -> Result<Value> {
        let url = self.base_url.join(path)?;

        let response = self.http_client.post(url).json(&body).send().await?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(SynapError::ServerError(error_text));
        }

        let result = response.json().await?;
        Ok(result)
    }

    /// Send a GET request
    pub(crate) async fn get(&self, path: &str) -> Result<Value> {
        let url = self.base_url.join(path)?;

        let response = self.http_client.get(url).send().await?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(SynapError::ServerError(error_text));
        }

        let result = response.json().await?;
        Ok(result)
    }

    /// Send a DELETE request
    pub(crate) async fn delete(&self, path: &str) -> Result<Value> {
        let url = self.base_url.join(path)?;

        let response = self.http_client.delete(url).send().await?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(SynapError::ServerError(error_text));
        }

        let result = response.json().await?;
        Ok(result)
    }

    /// Get the base URL
    #[allow(dead_code)]
    pub fn base_url(&self) -> &Url {
        &self.base_url
    }

    /// Get the HTTP client
    #[allow(dead_code)]
    pub(crate) fn http_client(&self) -> &Client {
        &self.http_client
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_creation() {
        let config = SynapConfig::new("http://localhost:15500");
        assert_eq!(config.base_url, "http://localhost:15500");
        assert_eq!(config.timeout, Duration::from_secs(30));
        assert_eq!(config.max_retries, 3);
        assert!(config.auth_token.is_none());
    }

    #[test]
    fn test_config_builder() {
        let config = SynapConfig::new("http://localhost:15500")
            .with_timeout(Duration::from_secs(10))
            .with_auth_token("test-token")
            .with_max_retries(5);

        assert_eq!(config.timeout, Duration::from_secs(10));
        assert_eq!(config.auth_token, Some("test-token".to_string()));
        assert_eq!(config.max_retries, 5);
    }

    #[test]
    fn test_client_creation() {
        let config = SynapConfig::new("http://localhost:15500");
        let client = SynapClient::new(config);
        assert!(client.is_ok());
    }
}
