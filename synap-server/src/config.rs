use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

use crate::core::{EvictionPolicy, KVConfig};

/// Main server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub server: Server,
    pub kv_store: KVStoreConfig,
    pub logging: LoggingConfig,
    pub protocols: ProtocolsConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Server {
    pub host: String,
    pub port: u16,
    pub websocket_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KVStoreConfig {
    pub max_memory_mb: usize,
    pub eviction_policy: EvictionPolicy,
    pub ttl_cleanup_interval_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    pub level: String,
    pub format: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolsConfig {
    pub streamable_http: StreamableHttpConfig,
    pub rest: RestConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamableHttpConfig {
    pub enabled: bool,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestConfig {
    pub enabled: bool,
    pub prefix: String,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            server: Server {
                host: "0.0.0.0".to_string(),
                port: 15500,
                websocket_enabled: false,
            },
            kv_store: KVStoreConfig {
                max_memory_mb: 4096,
                eviction_policy: EvictionPolicy::Lru,
                ttl_cleanup_interval_ms: 100,
            },
            logging: LoggingConfig {
                level: "info".to_string(),
                format: "json".to_string(),
            },
            protocols: ProtocolsConfig {
                streamable_http: StreamableHttpConfig {
                    enabled: true,
                    path: "/api/v1/command".to_string(),
                },
                rest: RestConfig {
                    enabled: true,
                    prefix: "/kv".to_string(),
                },
            },
        }
    }
}

impl ServerConfig {
    /// Load configuration from YAML file
    pub fn from_file<P: AsRef<Path>>(path: P) -> anyhow::Result<Self> {
        let content = fs::read_to_string(path)?;
        let config: ServerConfig = serde_yaml::from_str(&content)?;
        Ok(config)
    }

    /// Convert to KVConfig
    pub fn to_kv_config(&self) -> KVConfig {
        KVConfig {
            max_memory_mb: self.kv_store.max_memory_mb,
            eviction_policy: self.kv_store.eviction_policy,
            ttl_cleanup_interval_ms: self.kv_store.ttl_cleanup_interval_ms,
        }
    }

    /// Get server address
    pub fn server_addr(&self) -> String {
        format!("{}:{}", self.server.host, self.server.port)
    }
}
