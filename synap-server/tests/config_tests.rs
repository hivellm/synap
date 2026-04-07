// Configuration Module Tests
// Tests for ServerConfig, loading, defaults, and conversions

use std::fs;
use synap_server::ServerConfig;

#[test]
fn test_config_default_values() {
    let config = ServerConfig::default();

    // Server defaults
    assert_eq!(config.server.host, "0.0.0.0");
    assert_eq!(config.server.port, 15500);
    assert!(!config.server.websocket_enabled);

    // KV Store defaults
    assert_eq!(config.kv_store.max_memory_mb, 4096);
    assert_eq!(config.kv_store.ttl_cleanup_interval_ms, 100);

    // Queue defaults
    assert!(config.queue.enabled);
    assert_eq!(config.queue.max_depth, 100_000);
    assert_eq!(config.queue.ack_deadline_secs, 30);
    assert_eq!(config.queue.default_max_retries, 3);
    assert_eq!(config.queue.default_priority, 5);

    // Logging defaults
    assert_eq!(config.logging.level, "info");
    assert_eq!(config.logging.format, "json");

    // Protocols defaults
    assert!(config.protocols.streamable_http.enabled);
    assert_eq!(config.protocols.streamable_http.path, "/api/v1/command");
    assert!(config.protocols.rest.enabled);
    assert_eq!(config.protocols.rest.prefix, "/kv");

    // Rate limit defaults
    assert!(!config.rate_limit.enabled);
    assert_eq!(config.rate_limit.requests_per_second, 1000);
    assert_eq!(config.rate_limit.burst_size, 100);
}

#[test]
fn test_config_server_addr() {
    let config = ServerConfig::default();
    assert_eq!(config.server_addr(), "0.0.0.0:15500");

    let mut custom_config = ServerConfig::default();
    custom_config.server.host = "127.0.0.1".to_string();
    custom_config.server.port = 8080;
    assert_eq!(custom_config.server_addr(), "127.0.0.1:8080");
}

#[test]
fn test_config_to_kv_config() {
    let config = ServerConfig::default();
    let kv_config = config.to_kv_config();

    assert_eq!(kv_config.max_memory_mb, 4096);
    assert_eq!(kv_config.ttl_cleanup_interval_ms, 100);
}

#[test]
fn test_config_to_queue_config() {
    let config = ServerConfig::default();
    let queue_config = config.to_queue_config();

    assert_eq!(queue_config.max_depth, 100_000);
    assert_eq!(queue_config.ack_deadline_secs, 30);
    assert_eq!(queue_config.default_max_retries, 3);
    assert_eq!(queue_config.default_priority, 5);
}

#[test]
fn test_config_from_file() {
    // Create temporary config file
    let temp_config = r#"
server:
  host: "127.0.0.1"
  port: 9999
  websocket_enabled: true

kv_store:
  max_memory_mb: 2048
  eviction_policy: "lru"
  ttl_cleanup_interval_ms: 200

queue:
  enabled: false
  max_depth: 50000
  ack_deadline_secs: 60
  default_max_retries: 5
  default_priority: 7

logging:
  level: "debug"
  format: "pretty"

protocols:
  streamable_http:
    enabled: true
    path: "/cmd"
  rest:
    enabled: false
    prefix: "/api"

rate_limit:
  enabled: true
  requests_per_second: 500
  burst_size: 50
"#;

    let temp_file = "/tmp/synap_test_config.yml";
    fs::write(temp_file, temp_config).unwrap();

    let config = ServerConfig::from_file(temp_file).unwrap();

    assert_eq!(config.server.host, "127.0.0.1");
    assert_eq!(config.server.port, 9999);
    assert!(config.server.websocket_enabled);

    assert_eq!(config.kv_store.max_memory_mb, 2048);
    assert_eq!(config.kv_store.ttl_cleanup_interval_ms, 200);

    assert!(!config.queue.enabled);
    assert_eq!(config.queue.max_depth, 50000);
    assert_eq!(config.queue.ack_deadline_secs, 60);

    assert_eq!(config.logging.level, "debug");
    assert_eq!(config.logging.format, "pretty");

    assert!(config.protocols.streamable_http.enabled);
    assert_eq!(config.protocols.streamable_http.path, "/cmd");
    assert!(!config.protocols.rest.enabled);

    assert!(config.rate_limit.enabled);
    assert_eq!(config.rate_limit.requests_per_second, 500);

    // Cleanup
    fs::remove_file(temp_file).ok();
}

#[test]
fn test_config_from_nonexistent_file_fails() {
    let result = ServerConfig::from_file("/nonexistent/path/config.yml");
    assert!(result.is_err());
}

#[test]
fn test_config_from_invalid_yaml() {
    let temp_file = "/tmp/synap_invalid_config.yml";
    fs::write(temp_file, "invalid: yaml: content: [[[").unwrap();

    let result = ServerConfig::from_file(temp_file);
    assert!(result.is_err());

    fs::remove_file(temp_file).ok();
}

#[test]
fn test_config_eviction_policy_variants() {
    use synap_server::EvictionPolicy;

    let yaml = r#"
server:
  host: "127.0.0.1"
  port: 15500
  websocket_enabled: false

kv_store:
  max_memory_mb: 1024
  eviction_policy: "lfu"
  ttl_cleanup_interval_ms: 100

queue:
  enabled: true
  max_depth: 100000
  ack_deadline_secs: 30
  default_max_retries: 3
  default_priority: 5

logging:
  level: "info"
  format: "json"

protocols:
  streamable_http:
    enabled: true
    path: "/api/v1/command"
  rest:
    enabled: true
    prefix: "/kv"

rate_limit:
  enabled: false
  requests_per_second: 1000
  burst_size: 100
"#;

    let temp_file = "/tmp/synap_eviction_test.yml";
    fs::write(temp_file, yaml).unwrap();

    let config = ServerConfig::from_file(temp_file).unwrap();
    assert_eq!(config.kv_store.eviction_policy, EvictionPolicy::Lfu);

    fs::remove_file(temp_file).ok();
}

#[test]
fn test_config_serialization_roundtrip() {
    let config = ServerConfig::default();

    // Serialize to YAML
    let yaml = serde_yaml::to_string(&config).unwrap();

    // Deserialize back
    let deserialized: ServerConfig = serde_yaml::from_str(&yaml).unwrap();

    assert_eq!(deserialized.server.host, config.server.host);
    assert_eq!(deserialized.server.port, config.server.port);
    assert_eq!(
        deserialized.kv_store.max_memory_mb,
        config.kv_store.max_memory_mb
    );
}
