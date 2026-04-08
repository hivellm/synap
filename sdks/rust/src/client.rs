//! Synap client implementation

use std::sync::Arc;
use std::time::Duration;

use reqwest::Client;
use serde_json::Value;
use url::Url;

use crate::error::{Result, SynapError};
use crate::transport::{
    Resp3Transport, SynapRpcTransport, TransportMode, map_command, map_response,
};
use crate::{
    BitmapManager, GeospatialManager, HashManager, HyperLogLogManager, KVStore, ListManager,
    PubSubManager, QueueManager, ScriptManager, SetManager, SortedSetManager, StreamManager,
    TransactionManager,
};

// ── SynapConfig ───────────────────────────────────────────────────────────────

/// Synap client configuration.
#[derive(Debug, Clone)]
pub struct SynapConfig {
    /// Base URL of the Synap HTTP server (used for HTTP transport and fallback).
    pub base_url: String,
    /// Host for the SynapRPC TCP listener (default: `127.0.0.1`).
    pub rpc_host: String,
    /// Port for the SynapRPC TCP listener (default: `15501`).
    pub rpc_port: u16,
    /// Host for the RESP3 TCP listener (default: `127.0.0.1`).
    pub resp3_host: String,
    /// Port for the RESP3 TCP listener (default: `6379`).
    pub resp3_port: u16,
    /// Which binary protocol to use as primary transport (default: `SynapRpc`).
    pub transport: TransportMode,
    /// Request / connection timeout.
    pub timeout: Duration,
    /// Maximum retry attempts for HTTP requests.
    pub max_retries: u32,
    /// Optional API key token (Bearer token for HTTP).
    pub auth_token: Option<String>,
    /// Optional username for HTTP Basic Auth.
    pub username: Option<String>,
    /// Optional password for HTTP Basic Auth.
    pub password: Option<String>,
}

impl SynapConfig {
    /// Create a configuration that defaults to **SynapRPC** transport.
    ///
    /// Provide the HTTP `base_url` for commands that fall back to REST
    /// (pub/sub, queues, streams, and any command not yet mapped natively).
    ///
    /// The SynapRPC and RESP3 addresses default to `127.0.0.1:15501` and
    /// `127.0.0.1:6379` respectively. Use the builder methods to customise.
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            rpc_host: "127.0.0.1".into(),
            rpc_port: 15501,
            resp3_host: "127.0.0.1".into(),
            resp3_port: 6379,
            transport: TransportMode::SynapRpc,
            timeout: Duration::from_secs(30),
            max_retries: 3,
            auth_token: None,
            username: None,
            password: None,
        }
    }

    /// Use the HTTP REST transport only (original SDK behaviour).
    pub fn with_http_transport(mut self) -> Self {
        self.transport = TransportMode::Http;
        self
    }

    /// Use the SynapRPC binary transport (MessagePack over TCP). This is the
    /// default and has the lowest latency of the three options.
    pub fn with_synap_rpc_transport(mut self) -> Self {
        self.transport = TransportMode::SynapRpc;
        self
    }

    /// Use the RESP3 text transport (Redis-compatible wire protocol over TCP).
    pub fn with_resp3_transport(mut self) -> Self {
        self.transport = TransportMode::Resp3;
        self
    }

    /// Override the SynapRPC listener address (host + port).
    pub fn with_rpc_addr(mut self, host: impl Into<String>, port: u16) -> Self {
        self.rpc_host = host.into();
        self.rpc_port = port;
        self
    }

    /// Override the RESP3 listener address (host + port).
    pub fn with_resp3_addr(mut self, host: impl Into<String>, port: u16) -> Self {
        self.resp3_host = host.into();
        self.resp3_port = port;
        self
    }

    /// Set the timeout for connections and requests.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Set the authentication token (API key / Bearer token for HTTP).
    pub fn with_auth_token(mut self, token: impl Into<String>) -> Self {
        self.auth_token = Some(token.into());
        self.username = None;
        self.password = None;
        self
    }

    /// Set HTTP Basic Auth credentials.
    pub fn with_basic_auth(
        mut self,
        username: impl Into<String>,
        password: impl Into<String>,
    ) -> Self {
        self.username = Some(username.into());
        self.password = Some(password.into());
        self.auth_token = None;
        self
    }

    /// Set the maximum HTTP retry attempts.
    pub fn with_max_retries(mut self, max_retries: u32) -> Self {
        self.max_retries = max_retries;
        self
    }
}

// ── Internal transport enum ───────────────────────────────────────────────────

enum Transport {
    Http,
    SynapRpc(Arc<SynapRpcTransport>),
    Resp3(Arc<Resp3Transport>),
}

// ── SynapClient ───────────────────────────────────────────────────────────────

/// Main Synap client.
///
/// Internally uses one of three transports — SynapRPC (default), RESP3, or
/// HTTP — selected via [`SynapConfig::transport`].  Commands that have no
/// native-protocol mapping automatically fall back to HTTP regardless of the
/// chosen transport.
#[derive(Clone)]
pub struct SynapClient {
    #[allow(dead_code)]
    config: Arc<SynapConfig>,
    http_client: Client,
    base_url: Url,
    transport: Arc<Transport>,
}

impl SynapClient {
    /// Create a new Synap client using the provided configuration.
    pub fn new(config: SynapConfig) -> Result<Self> {
        let base_url = Url::parse(&config.base_url)?;

        // Build reqwest HTTP client (needed for fallback and Http transport).
        let mut builder = Client::builder().timeout(config.timeout);

        if let Some(ref token) = config.auth_token {
            let mut headers = reqwest::header::HeaderMap::new();
            headers.insert(
                reqwest::header::AUTHORIZATION,
                format!("Bearer {}", token).parse().unwrap(),
            );
            builder = builder.default_headers(headers);
        } else if let (Some(username), Some(password)) = (&config.username, &config.password) {
            use base64::Engine;
            let encoded = base64::engine::general_purpose::STANDARD
                .encode(format!("{}:{}", username, password));
            let mut headers = reqwest::header::HeaderMap::new();
            headers.insert(
                reqwest::header::AUTHORIZATION,
                format!("Basic {}", encoded).parse().unwrap(),
            );
            builder = builder.default_headers(headers);
        }

        let http_client = builder.build()?;

        let transport = match config.transport {
            TransportMode::Http => Arc::new(Transport::Http),
            TransportMode::SynapRpc => Arc::new(Transport::SynapRpc(Arc::new(
                SynapRpcTransport::new(&config.rpc_host, config.rpc_port, config.timeout),
            ))),
            TransportMode::Resp3 => Arc::new(Transport::Resp3(Arc::new(Resp3Transport::new(
                &config.resp3_host,
                config.resp3_port,
                config.timeout,
            )))),
        };

        Ok(Self {
            config: Arc::new(config),
            http_client,
            base_url,
            transport,
        })
    }

    // ── Manager accessors ─────────────────────────────────────────────────────

    /// Get the Key-Value store interface.
    pub fn kv(&self) -> KVStore {
        KVStore::new(self.clone())
    }

    /// Get the Hash manager interface.
    pub fn hash(&self) -> HashManager {
        HashManager::new(self.clone())
    }

    /// Get the List manager interface.
    pub fn list(&self) -> ListManager {
        ListManager::new(self.clone())
    }

    /// Get the Set manager interface.
    pub fn set(&self) -> SetManager {
        SetManager::new(self.clone())
    }

    /// Get the Sorted Set manager interface.
    pub fn sorted_set(&self) -> SortedSetManager {
        SortedSetManager::new(self.clone())
    }

    /// Get the Queue manager interface.
    pub fn queue(&self) -> QueueManager {
        QueueManager::new(self.clone())
    }

    /// Get the Stream manager interface.
    pub fn stream(&self) -> StreamManager {
        StreamManager::new(self.clone())
    }

    /// Get the Pub/Sub manager interface.
    pub fn pubsub(&self) -> PubSubManager {
        PubSubManager::new(self.clone())
    }

    /// Get the Transaction manager interface.
    pub fn transaction(&self) -> TransactionManager {
        TransactionManager::new(self.clone())
    }

    /// Get the Scripting manager interface.
    pub fn script(&self) -> ScriptManager {
        ScriptManager::new(self.clone())
    }

    /// Get the HyperLogLog manager interface.
    pub fn hyperloglog(&self) -> HyperLogLogManager {
        HyperLogLogManager::new(self.clone())
    }

    /// Get the Bitmap manager interface.
    pub fn bitmap(&self) -> BitmapManager {
        BitmapManager::new(self.clone())
    }

    /// Get the Geospatial manager interface.
    pub fn geospatial(&self) -> GeospatialManager {
        GeospatialManager::new(self.clone())
    }

    // ── Command dispatch ──────────────────────────────────────────────────────

    /// Dispatch a command to the active transport.
    ///
    /// For SynapRPC and RESP3 transports, commands that have a native mapping
    /// are sent directly over TCP; unmapped commands fall back to HTTP REST.
    pub(crate) async fn send_command(&self, command: &str, payload: Value) -> Result<Value> {
        match self.transport.as_ref() {
            Transport::Http => self.send_http(command, payload).await,

            Transport::SynapRpc(rpc) => {
                if let Some((raw_cmd, args)) = map_command(command, &payload) {
                    let wire = rpc.execute(raw_cmd, args).await?;
                    Ok(map_response(command, wire))
                } else {
                    // Unmapped command — fall back to HTTP.
                    self.send_http(command, payload).await
                }
            }

            Transport::Resp3(resp3) => {
                if let Some((raw_cmd, args)) = map_command(command, &payload) {
                    let wire = resp3.execute(raw_cmd, args).await?;
                    Ok(map_response(command, wire))
                } else {
                    self.send_http(command, payload).await
                }
            }
        }
    }

    /// Send a command via HTTP REST (original `api/v1/command` endpoint).
    async fn send_http(&self, command: &str, payload: Value) -> Result<Value> {
        let request_id = uuid::Uuid::new_v4().to_string();

        let body = serde_json::json!({
            "command": command,
            "request_id": request_id,
            "payload": payload,
        });

        let url = self
            .base_url
            .join("api/v1/command")
            .map_err(SynapError::InvalidUrl)?;

        let response = self.http_client.post(url).json(&body).send().await?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(SynapError::ServerError(error_text));
        }

        let result: Value = response.json().await?;

        if !result["success"].as_bool().unwrap_or(false) {
            let error_msg = result["error"]
                .as_str()
                .unwrap_or("Unknown error")
                .to_string();
            return Err(SynapError::ServerError(error_msg));
        }

        Ok(result["payload"].clone())
    }

    // ── Accessors ─────────────────────────────────────────────────────────────

    /// Get the configured base URL.
    #[allow(dead_code)]
    pub fn base_url(&self) -> &Url {
        &self.base_url
    }

    /// Get the underlying reqwest HTTP client.
    #[allow(dead_code)]
    pub(crate) fn http_client(&self) -> &Client {
        &self.http_client
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn http_config() -> SynapConfig {
        SynapConfig::new("http://localhost:15500").with_http_transport()
    }

    #[test]
    fn test_config_creation() {
        let config = SynapConfig::new("http://localhost:15500");
        assert_eq!(config.base_url, "http://localhost:15500");
        assert_eq!(config.timeout, Duration::from_secs(30));
        assert_eq!(config.max_retries, 3);
        assert!(config.auth_token.is_none());
        assert!(matches!(config.transport, TransportMode::SynapRpc));
    }

    #[test]
    fn test_config_transport_selection() {
        let c = SynapConfig::new("http://localhost:15500").with_http_transport();
        assert!(matches!(c.transport, TransportMode::Http));

        let c = SynapConfig::new("http://localhost:15500").with_resp3_transport();
        assert!(matches!(c.transport, TransportMode::Resp3));

        let c = SynapConfig::new("http://localhost:15500").with_synap_rpc_transport();
        assert!(matches!(c.transport, TransportMode::SynapRpc));
    }

    #[test]
    fn test_config_rpc_addr() {
        let c = SynapConfig::new("http://localhost:15500").with_rpc_addr("10.0.0.1", 15502);
        assert_eq!(c.rpc_host, "10.0.0.1");
        assert_eq!(c.rpc_port, 15502);
    }

    #[test]
    fn test_config_resp3_addr() {
        let c = SynapConfig::new("http://localhost:15500").with_resp3_addr("10.0.0.1", 6380);
        assert_eq!(c.resp3_host, "10.0.0.1");
        assert_eq!(c.resp3_port, 6380);
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
    fn test_client_creation_http() {
        let config = http_config();
        let client = SynapClient::new(config);
        assert!(client.is_ok());
    }

    #[test]
    fn test_client_creation_synap_rpc() {
        // SynapRpc transport: creating the client succeeds even when no server
        // is running — the connection is established lazily.
        let config = SynapConfig::new("http://localhost:15500");
        let client = SynapClient::new(config);
        assert!(client.is_ok());
    }

    #[test]
    fn test_client_creation_resp3() {
        let config = SynapConfig::new("http://localhost:15500").with_resp3_transport();
        let client = SynapClient::new(config);
        assert!(client.is_ok());
    }

    #[test]
    fn test_client_with_auth() {
        let config = http_config().with_auth_token("secret-token-123");
        let client = SynapClient::new(config);
        assert!(client.is_ok());
    }

    #[test]
    fn test_client_invalid_url() {
        let config = SynapConfig::new("not-a-valid-url").with_http_transport();
        let client = SynapClient::new(config);
        assert!(client.is_err());
    }

    #[test]
    fn test_client_relative_url() {
        let config = SynapConfig::new("/relative/path").with_http_transport();
        let client = SynapClient::new(config);
        assert!(client.is_err());
    }

    #[test]
    fn test_client_kv_interface() {
        let client = SynapClient::new(http_config()).unwrap();
        let _kv = client.kv();
    }

    #[test]
    fn test_client_queue_interface() {
        let client = SynapClient::new(http_config()).unwrap();
        let _queue = client.queue();
    }

    #[test]
    fn test_client_transaction_interface() {
        let client = SynapClient::new(http_config()).unwrap();
        let _tx = client.transaction();
    }

    #[test]
    fn test_client_script_interface() {
        let client = SynapClient::new(http_config()).unwrap();
        let _script = client.script();
    }

    #[test]
    fn test_client_hyperloglog_interface() {
        let client = SynapClient::new(http_config()).unwrap();
        let _hll = client.hyperloglog();
    }

    #[test]
    fn test_client_bitmap_interface() {
        let client = SynapClient::new(http_config()).unwrap();
        let _bitmap = client.bitmap();
    }

    #[test]
    fn test_client_geospatial_interface() {
        let client = SynapClient::new(http_config()).unwrap();
        let _geo = client.geospatial();
    }

    #[test]
    fn test_client_stream_interface() {
        let client = SynapClient::new(http_config()).unwrap();
        let _stream = client.stream();
    }

    #[test]
    fn test_client_pubsub_interface() {
        let client = SynapClient::new(http_config()).unwrap();
        let _pubsub = client.pubsub();
    }

    #[test]
    fn test_client_clone() {
        let client = SynapClient::new(http_config()).unwrap();
        let client2 = client.clone();
        assert!(std::ptr::eq(
            &*client.config as *const _,
            &*client2.config as *const _
        ));
    }

    #[test]
    fn test_base_url_getter() {
        let client = SynapClient::new(http_config()).unwrap();
        assert_eq!(client.base_url().as_str(), "http://localhost:15500/");
    }

    #[test]
    fn test_http_client_getter() {
        let client = SynapClient::new(http_config()).unwrap();
        let _http = client.http_client();
    }

    #[test]
    fn test_config_with_custom_timeout() {
        let config = http_config().with_timeout(Duration::from_secs(60));
        assert_eq!(config.timeout, Duration::from_secs(60));
    }

    #[test]
    fn test_config_with_zero_retries() {
        let config = http_config().with_max_retries(0);
        assert_eq!(config.max_retries, 0);
    }

    #[test]
    fn test_config_clone() {
        let config = http_config().with_auth_token("token");
        let config2 = config.clone();
        assert_eq!(config.base_url, config2.base_url);
        assert_eq!(config.auth_token, config2.auth_token);
    }

    #[test]
    fn test_config_debug_format() {
        let config = http_config();
        let debug_str = format!("{:?}", config);
        assert!(debug_str.contains("SynapConfig"));
        assert!(debug_str.contains("http://localhost:15500"));
    }
}
