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
    /// Create a configuration, inferring the transport from the URL scheme.
    ///
    /// | Scheme | Transport | Default port |
    /// |--------|-----------|--------------|
    /// | `http://` or `https://` | HTTP REST | — (as given) |
    /// | `synap://` | SynapRPC (MessagePack/TCP) | 15501 |
    /// | `resp3://` | RESP3 (Redis wire/TCP) | 6379 |
    ///
    /// For `synap://` and `resp3://` URLs the host and port are parsed from
    /// the URL.  The HTTP base URL is set to `http://<host>:15500` as a
    /// convenience for any HTTP-only admin calls.
    ///
    /// # Examples
    /// ```
    /// use synap_sdk::SynapConfig;
    ///
    /// let c = SynapConfig::new("synap://localhost:15501");
    /// let c = SynapConfig::new("resp3://localhost:6379");
    /// let c = SynapConfig::new("http://localhost:15500");
    /// ```
    pub fn new(url: impl Into<String>) -> Self {
        let raw = url.into();

        // Parse scheme to infer transport without pulling in the full `url` crate
        // at this point (we just need scheme + host + port).
        if let Some(rest) = raw.strip_prefix("synap://") {
            let (host, port) = Self::parse_host_port(rest, 15501);
            return Self {
                base_url: format!("http://{}:15500", host),
                rpc_host: host,
                rpc_port: port,
                resp3_host: "127.0.0.1".into(),
                resp3_port: 6379,
                transport: TransportMode::SynapRpc,
                timeout: Duration::from_secs(30),
                max_retries: 3,
                auth_token: None,
                username: None,
                password: None,
            };
        }

        if let Some(rest) = raw.strip_prefix("resp3://") {
            let (host, port) = Self::parse_host_port(rest, 6379);
            return Self {
                base_url: format!("http://{}:15500", host),
                rpc_host: "127.0.0.1".into(),
                rpc_port: 15501,
                resp3_host: host,
                resp3_port: port,
                transport: TransportMode::Resp3,
                timeout: Duration::from_secs(30),
                max_retries: 3,
                auth_token: None,
                username: None,
                password: None,
            };
        }

        // http:// or https:// → HTTP transport
        Self {
            base_url: raw,
            rpc_host: "127.0.0.1".into(),
            rpc_port: 15501,
            resp3_host: "127.0.0.1".into(),
            resp3_port: 6379,
            transport: TransportMode::Http,
            timeout: Duration::from_secs(30),
            max_retries: 3,
            auth_token: None,
            username: None,
            password: None,
        }
    }

    /// Parse `"host:port"` from a URL authority string, falling back to
    /// `default_port` when no port is present.
    fn parse_host_port(authority: &str, default_port: u16) -> (String, u16) {
        // Strip any trailing path component.
        let authority = authority.split('/').next().unwrap_or(authority);
        if let Some(colon) = authority.rfind(':') {
            let host = authority[..colon].to_string();
            let port = authority[colon + 1..]
                .parse::<u16>()
                .unwrap_or(default_port);
            (host, port)
        } else {
            (authority.to_string(), default_port)
        }
    }

    /// Use the HTTP REST transport only (original SDK behaviour).
    ///
    /// # Deprecated
    /// Prefer passing an `http://` URL to [`SynapConfig::new`].
    #[deprecated(
        since = "0.11.0",
        note = "Pass an `http://` URL to SynapConfig::new instead"
    )]
    pub fn with_http_transport(mut self) -> Self {
        self.transport = TransportMode::Http;
        self
    }

    /// Use the SynapRPC binary transport (MessagePack over TCP). This is the
    /// default and has the lowest latency of the three options.
    ///
    /// # Deprecated
    /// Prefer passing a `synap://` URL to [`SynapConfig::new`].
    #[deprecated(
        since = "0.11.0",
        note = "Pass a `synap://` URL to SynapConfig::new instead"
    )]
    pub fn with_synap_rpc_transport(mut self) -> Self {
        self.transport = TransportMode::SynapRpc;
        self
    }

    /// Use the RESP3 text transport (Redis-compatible wire protocol over TCP).
    ///
    /// # Deprecated
    /// Prefer passing a `resp3://` URL to [`SynapConfig::new`].
    #[deprecated(
        since = "0.11.0",
        note = "Pass a `resp3://` URL to SynapConfig::new instead"
    )]
    pub fn with_resp3_transport(mut self) -> Self {
        self.transport = TransportMode::Resp3;
        self
    }

    /// Override the SynapRPC listener address (host + port).
    ///
    /// # Deprecated
    /// Encode the host and port in the `synap://host:port` URL instead.
    #[deprecated(
        since = "0.11.0",
        note = "Encode host and port in the `synap://` URL passed to SynapConfig::new"
    )]
    pub fn with_rpc_addr(mut self, host: impl Into<String>, port: u16) -> Self {
        self.rpc_host = host.into();
        self.rpc_port = port;
        self
    }

    /// Override the RESP3 listener address (host + port).
    ///
    /// # Deprecated
    /// Encode the host and port in the `resp3://host:port` URL instead.
    #[deprecated(
        since = "0.11.0",
        note = "Encode host and port in the `resp3://` URL passed to SynapConfig::new"
    )]
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
    /// For SynapRPC and RESP3 transports every command must have a native
    /// mapping in the transport mapper.  Unmapped commands return
    /// [`SynapError::UnsupportedCommand`] — there is no silent HTTP fallback.
    /// Use an `http://` URL if you need HTTP REST for a command that is not
    /// yet in the mapper.
    pub(crate) async fn send_command(&self, command: &str, payload: Value) -> Result<Value> {
        match self.transport.as_ref() {
            Transport::Http => self.send_http(command, payload).await,

            Transport::SynapRpc(rpc) => match map_command(command, &payload) {
                Some((raw_cmd, args)) => {
                    let wire = rpc.execute(raw_cmd, args).await?;
                    Ok(map_response(command, wire))
                }
                None => Err(SynapError::UnsupportedCommand {
                    command: command.to_owned(),
                    transport: "SynapRpc".to_owned(),
                }),
            },

            Transport::Resp3(resp3) => match map_command(command, &payload) {
                Some((raw_cmd, args)) => {
                    let wire = resp3.execute(raw_cmd, args).await?;
                    Ok(map_response(command, wire))
                }
                None => Err(SynapError::UnsupportedCommand {
                    command: command.to_owned(),
                    transport: "Resp3".to_owned(),
                }),
            },
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

    /// Return a reference to the `SynapRpcTransport` when the active transport
    /// is `SynapRpc`, or `None` for HTTP / RESP3.
    pub(crate) fn synap_rpc_transport(&self) -> Option<Arc<SynapRpcTransport>> {
        match self.transport.as_ref() {
            Transport::SynapRpc(rpc) => Some(Arc::clone(rpc)),
            _ => None,
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn http_config() -> SynapConfig {
        SynapConfig::new("http://localhost:15500")
    }

    #[test]
    fn test_config_creation_http_url_infers_http_transport() {
        let config = SynapConfig::new("http://localhost:15500");
        assert_eq!(config.base_url, "http://localhost:15500");
        assert_eq!(config.timeout, Duration::from_secs(30));
        assert_eq!(config.max_retries, 3);
        assert!(config.auth_token.is_none());
        // http:// URLs → Http transport (not SynapRpc)
        assert!(matches!(config.transport, TransportMode::Http));
    }

    #[test]
    fn test_config_synap_url_infers_rpc_transport() {
        let c = SynapConfig::new("synap://localhost:15501");
        assert!(matches!(c.transport, TransportMode::SynapRpc));
        assert_eq!(c.rpc_host, "localhost");
        assert_eq!(c.rpc_port, 15501);
    }

    #[test]
    fn test_config_resp3_url_infers_resp3_transport() {
        let c = SynapConfig::new("resp3://localhost:6379");
        assert!(matches!(c.transport, TransportMode::Resp3));
        assert_eq!(c.resp3_host, "localhost");
        assert_eq!(c.resp3_port, 6379);
    }

    #[test]
    fn test_config_synap_url_default_port() {
        let c = SynapConfig::new("synap://myhost");
        assert!(matches!(c.transport, TransportMode::SynapRpc));
        assert_eq!(c.rpc_host, "myhost");
        assert_eq!(c.rpc_port, 15501); // default
    }

    #[test]
    fn test_config_resp3_url_default_port() {
        let c = SynapConfig::new("resp3://myhost");
        assert!(matches!(c.transport, TransportMode::Resp3));
        assert_eq!(c.resp3_host, "myhost");
        assert_eq!(c.resp3_port, 6379); // default
    }

    #[test]
    #[allow(deprecated)]
    fn test_config_deprecated_transport_selection() {
        let c = SynapConfig::new("http://localhost:15500").with_http_transport();
        assert!(matches!(c.transport, TransportMode::Http));

        let c = SynapConfig::new("http://localhost:15500").with_resp3_transport();
        assert!(matches!(c.transport, TransportMode::Resp3));

        let c = SynapConfig::new("http://localhost:15500").with_synap_rpc_transport();
        assert!(matches!(c.transport, TransportMode::SynapRpc));
    }

    #[test]
    #[allow(deprecated)]
    fn test_config_deprecated_rpc_addr() {
        let c = SynapConfig::new("http://localhost:15500").with_rpc_addr("10.0.0.1", 15502);
        assert_eq!(c.rpc_host, "10.0.0.1");
        assert_eq!(c.rpc_port, 15502);
    }

    #[test]
    #[allow(deprecated)]
    fn test_config_deprecated_resp3_addr() {
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
        let config = SynapConfig::new("resp3://localhost:6379");
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
        // No recognised scheme → falls through to HTTP with the raw string as base_url.
        // The HTTP client builder rejects non-absolute URLs.
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
