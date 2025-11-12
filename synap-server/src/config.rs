use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

use crate::core::{EvictionPolicy, KVConfig, QueueConfig};
use crate::persistence::PersistenceConfig;
use crate::replication::ReplicationConfig;

/// Main server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub server: Server,
    pub kv_store: KVStoreConfig,
    pub queue: QueueSystemConfig,
    pub logging: LoggingConfig,
    pub protocols: ProtocolsConfig,
    pub rate_limit: RateLimitConfig,
    #[serde(default)]
    pub persistence: PersistenceConfig,
    #[serde(default)]
    pub replication: ReplicationConfig,
    #[serde(default)]
    pub mcp: McpConfig,
    #[serde(default)]
    pub auth: AuthConfig,
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
    #[serde(default)]
    pub allow_flush_commands: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueSystemConfig {
    pub enabled: bool,
    pub max_depth: usize,
    pub ack_deadline_secs: u64,
    pub default_max_retries: u32,
    pub default_priority: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitConfig {
    pub enabled: bool,
    pub requests_per_second: u64,
    pub burst_size: u64,
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

/// MCP (Model Context Protocol) Tools Configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpConfig {
    /// Enable KV (key-value) tools (synap_kv_get, synap_kv_set, synap_kv_delete)
    #[serde(default = "default_true")]
    pub enable_kv_tools: bool,

    /// Enable Hash tools (synap_hash_get, synap_hash_set, synap_hash_getall)
    #[serde(default)]
    pub enable_hash_tools: bool,

    /// Enable List tools (synap_list_push, synap_list_pop, synap_list_range)
    #[serde(default)]
    pub enable_list_tools: bool,

    /// Enable Set tools (synap_set_add, synap_set_members, synap_set_inter)
    #[serde(default)]
    pub enable_set_tools: bool,

    /// Enable Queue tools (synap_queue_publish)
    #[serde(default = "default_true")]
    pub enable_queue_tools: bool,

    /// Enable Sorted Set tools (synap_sortedset_zadd, synap_sortedset_zrange, synap_sortedset_zrank)
    #[serde(default)]
    pub enable_sortedset_tools: bool,

    /// Enable Transaction tools (synap_transaction_multi, synap_transaction_exec)
    #[serde(default)]
    pub enable_transaction_tools: bool,
}

fn default_true() -> bool {
    true
}

/// Authentication configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    /// Enable/disable authentication
    #[serde(default)]
    pub enabled: bool,
    /// Require authentication for all endpoints
    #[serde(default)]
    pub require_auth: bool,
    /// Root user configuration
    #[serde(default)]
    pub root: RootUserConfig,
    /// Default TTL for temporary API keys (in seconds)
    #[serde(default = "default_key_ttl")]
    pub default_key_ttl: u64,
}

/// Root user configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RootUserConfig {
    /// Root username (default: "root")
    #[serde(default = "default_root_username")]
    pub username: String,
    /// Root password (default: "root", must be changed in production)
    #[serde(default = "default_root_password")]
    pub password: String,
    /// Enable/disable root user (can disable after initial setup)
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_key_ttl() -> u64 {
    3600 // 1 hour
}

fn default_root_username() -> String {
    "root".to_string()
}

fn default_root_password() -> String {
    "root".to_string()
}

impl Default for RootUserConfig {
    fn default() -> Self {
        Self {
            username: "root".to_string(),
            password: "root".to_string(),
            enabled: true,
        }
    }
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            require_auth: false,
            root: RootUserConfig::default(),
            default_key_ttl: 3600,
        }
    }
}

impl Default for McpConfig {
    fn default() -> Self {
        Self {
            enable_kv_tools: true,           // Essential - always enabled by default
            enable_hash_tools: false,        // Optional
            enable_list_tools: false,        // Optional
            enable_set_tools: false,         // Optional
            enable_queue_tools: true,        // Essential - enabled by default
            enable_sortedset_tools: false,   // Optional
            enable_transaction_tools: false, // Optional
        }
    }
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
                allow_flush_commands: false,
            },
            queue: QueueSystemConfig {
                enabled: true,
                max_depth: 100_000,
                ack_deadline_secs: 30,
                default_max_retries: 3,
                default_priority: 5,
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
            rate_limit: RateLimitConfig {
                enabled: false,
                requests_per_second: 1000,
                burst_size: 100,
            },
            persistence: PersistenceConfig::default(),
            replication: ReplicationConfig::default(),
            mcp: McpConfig::default(),
            auth: AuthConfig::default(),
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
            allow_flush_commands: self.kv_store.allow_flush_commands,
        }
    }

    /// Convert to QueueConfig
    pub fn to_queue_config(&self) -> QueueConfig {
        QueueConfig {
            max_depth: self.queue.max_depth,
            ack_deadline_secs: self.queue.ack_deadline_secs,
            default_max_retries: self.queue.default_max_retries,
            default_priority: self.queue.default_priority,
        }
    }

    /// Get server address
    pub fn server_addr(&self) -> String {
        format!("{}:{}", self.server.host, self.server.port)
    }
}
