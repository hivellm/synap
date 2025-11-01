//! Synap client implementation

use crate::error::{Result, SynapError};
use crate::{
    BitmapManager, GeospatialManager, HashManager, HyperLogLogManager, KVStore, ListManager,
    PubSubManager, QueueManager, ScriptManager, SetManager, SortedSetManager, StreamManager,
    TransactionManager,
};
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
    #[allow(dead_code)]
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

    /// Get the Hash manager interface
    pub fn hash(&self) -> HashManager {
        HashManager::new(self.clone())
    }

    /// Get the List manager interface
    pub fn list(&self) -> ListManager {
        ListManager::new(self.clone())
    }

    /// Get the Set manager interface
    pub fn set(&self) -> SetManager {
        SetManager::new(self.clone())
    }

    /// Get the Sorted Set manager interface
    pub fn sorted_set(&self) -> SortedSetManager {
        SortedSetManager::new(self.clone())
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

    /// Get the Transaction manager interface
    pub fn transaction(&self) -> TransactionManager {
        TransactionManager::new(self.clone())
    }

    /// Get the scripting manager interface
    pub fn script(&self) -> ScriptManager {
        ScriptManager::new(self.clone())
    }

    /// Get the HyperLogLog manager interface
    pub fn hyperloglog(&self) -> HyperLogLogManager {
        HyperLogLogManager::new(self.clone())
    }

    /// Get the Bitmap manager interface
    pub fn bitmap(&self) -> BitmapManager {
        BitmapManager::new(self.clone())
    }

    /// Get the Geospatial manager interface
    pub fn geospatial(&self) -> GeospatialManager {
        GeospatialManager::new(self.clone())
    }

    /// Send a StreamableHTTP command
    ///
    /// This is the primary method for communicating with Synap server.
    /// All commands use the StreamableHTTP protocol format:
    /// ```json
    /// {
    ///   "command": "kv.get",
    ///   "request_id": "uuid",
    ///   "payload": { ... }
    /// }
    /// ```
    pub(crate) async fn send_command(&self, command: &str, payload: Value) -> Result<Value> {
        let request_id = uuid::Uuid::new_v4().to_string();

        let body = serde_json::json!({
            "command": command,
            "request_id": request_id,
            "payload": payload,
        });

        let url = self.base_url.join("api/v1/command")?;

        let response = self.http_client.post(url).json(&body).send().await?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(SynapError::ServerError(error_text));
        }

        let result: Value = response.json().await?;

        // Check if command succeeded
        if !result["success"].as_bool().unwrap_or(false) {
            let error_msg = result["error"]
                .as_str()
                .unwrap_or("Unknown error")
                .to_string();
            return Err(SynapError::ServerError(error_msg));
        }

        // Return the payload
        Ok(result["payload"].clone())
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

    #[test]
    fn test_client_with_auth() {
        let config = SynapConfig::new("http://localhost:15500").with_auth_token("secret-token-123");
        let client = SynapClient::new(config);
        assert!(client.is_ok());
    }

    #[test]
    fn test_client_invalid_url() {
        let config = SynapConfig::new("not-a-valid-url");
        let client = SynapClient::new(config);
        assert!(client.is_err());
    }

    #[test]
    fn test_client_relative_url() {
        let config = SynapConfig::new("/relative/path");
        let client = SynapClient::new(config);
        assert!(client.is_err());
    }

    #[test]
    fn test_client_kv_interface() {
        let config = SynapConfig::new("http://localhost:15500");
        let client = SynapClient::new(config).unwrap();
        let _kv = client.kv();
        // Just verify it doesn't panic
    }

    #[test]
    fn test_client_queue_interface() {
        let config = SynapConfig::new("http://localhost:15500");
        let client = SynapClient::new(config).unwrap();
        let _queue = client.queue();
        // Just verify it doesn't panic
    }

    #[test]
    fn test_client_transaction_interface() {
        let config = SynapConfig::new("http://localhost:15500");
        let client = SynapClient::new(config).unwrap();
        let _tx = client.transaction();
    }

    #[test]
    fn test_client_script_interface() {
        let config = SynapConfig::new("http://localhost:15500");
        let client = SynapClient::new(config).unwrap();
        let _script = client.script();
    }

    #[test]
    fn test_client_hyperloglog_interface() {
        let config = SynapConfig::new("http://localhost:15500");
        let client = SynapClient::new(config).unwrap();
        let _hll = client.hyperloglog();
    }

    #[test]
    fn test_client_bitmap_interface() {
        let config = SynapConfig::new("http://localhost:15500");
        let client = SynapClient::new(config).unwrap();
        let _bitmap = client.bitmap();
    }

    #[test]
    fn test_client_geospatial_interface() {
        let config = SynapConfig::new("http://localhost:15500");
        let client = SynapClient::new(config).unwrap();
        let _geospatial = client.geospatial();
    }

    #[test]
    fn test_client_stream_interface() {
        let config = SynapConfig::new("http://localhost:15500");
        let client = SynapClient::new(config).unwrap();
        let _stream = client.stream();
        // Just verify it doesn't panic
    }

    #[test]
    fn test_client_pubsub_interface() {
        let config = SynapConfig::new("http://localhost:15500");
        let client = SynapClient::new(config).unwrap();
        let _pubsub = client.pubsub();
        // Just verify it doesn't panic
    }

    #[test]
    fn test_client_clone() {
        let config = SynapConfig::new("http://localhost:15500");
        let client = SynapClient::new(config).unwrap();
        let client2 = client.clone();
        assert!(std::ptr::eq(
            &*client.config as *const _,
            &*client2.config as *const _
        ));
    }

    #[test]
    fn test_base_url_getter() {
        let config = SynapConfig::new("http://localhost:15500");
        let client = SynapClient::new(config).unwrap();
        assert_eq!(client.base_url().as_str(), "http://localhost:15500/");
    }

    #[test]
    fn test_http_client_getter() {
        let config = SynapConfig::new("http://localhost:15500");
        let client = SynapClient::new(config).unwrap();
        let _http_client = client.http_client();
        // Just verify it doesn't panic
    }

    #[test]
    fn test_config_with_custom_timeout() {
        let config =
            SynapConfig::new("http://localhost:15500").with_timeout(Duration::from_secs(60));
        assert_eq!(config.timeout, Duration::from_secs(60));
    }

    #[test]
    fn test_config_with_zero_retries() {
        let config = SynapConfig::new("http://localhost:15500").with_max_retries(0);
        assert_eq!(config.max_retries, 0);
    }

    #[test]
    fn test_config_clone() {
        let config = SynapConfig::new("http://localhost:15500").with_auth_token("token");
        let config2 = config.clone();
        assert_eq!(config.base_url, config2.base_url);
        assert_eq!(config.auth_token, config2.auth_token);
    }

    #[test]
    fn test_config_debug_format() {
        let config = SynapConfig::new("http://localhost:15500");
        let debug_str = format!("{:?}", config);
        assert!(debug_str.contains("SynapConfig"));
        assert!(debug_str.contains("http://localhost:15500"));
    }
}
