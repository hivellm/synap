//! HiveHub.Cloud Configuration
//!
//! Configuration structures for HiveHub integration.

use serde::{Deserialize, Serialize};
use std::time::Duration;

/// HiveHub.Cloud integration configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HubConfig {
    /// Enable Hub integration (true = SaaS mode, false = standalone mode)
    #[serde(default)]
    pub enabled: bool,

    /// HiveHub API base URL
    #[serde(default = "default_api_url")]
    pub api_url: String,

    /// Service API key for authenticating Synap service with Hub
    /// Should be loaded from environment variable: HIVEHUB_SERVICE_API_KEY
    #[serde(default)]
    pub service_api_key: String,

    /// Usage reporting interval in seconds (default: 300 = 5 minutes)
    #[serde(default = "default_usage_report_interval")]
    pub usage_report_interval: u64,

    /// Access key cache TTL in seconds (default: 60)
    #[serde(default = "default_cache_ttl")]
    pub cache_ttl: u64,

    /// Hub client timeout in seconds (default: 30)
    #[serde(default = "default_timeout")]
    pub timeout: u64,
}

fn default_api_url() -> String {
    "http://localhost:3000".to_string()
}

fn default_usage_report_interval() -> u64 {
    300 // 5 minutes
}

fn default_cache_ttl() -> u64 {
    60 // 1 minute
}

fn default_timeout() -> u64 {
    30 // 30 seconds
}

impl Default for HubConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            api_url: default_api_url(),
            service_api_key: String::new(),
            usage_report_interval: default_usage_report_interval(),
            cache_ttl: default_cache_ttl(),
            timeout: default_timeout(),
        }
    }
}

impl HubConfig {
    /// Get timeout as Duration
    pub fn timeout_duration(&self) -> Duration {
        Duration::from_secs(self.timeout)
    }

    /// Get cache TTL as Duration
    pub fn cache_ttl_duration(&self) -> Duration {
        Duration::from_secs(self.cache_ttl)
    }

    /// Get usage report interval as Duration
    pub fn usage_report_interval_duration(&self) -> Duration {
        Duration::from_secs(self.usage_report_interval)
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<(), String> {
        if self.enabled {
            if self.api_url.is_empty() {
                return Err("Hub API URL is required when hub.enabled = true".to_string());
            }

            if self.service_api_key.is_empty() {
                return Err("Service API key is required when hub.enabled = true. \
                    Set HIVEHUB_SERVICE_API_KEY environment variable"
                    .to_string());
            }

            if self.usage_report_interval < 60 {
                return Err("Usage report interval must be at least 60 seconds".to_string());
            }

            if self.cache_ttl < 10 || self.cache_ttl > 300 {
                return Err("Cache TTL must be between 10 and 300 seconds".to_string());
            }

            if self.timeout < 5 || self.timeout > 120 {
                return Err("Timeout must be between 5 and 120 seconds".to_string());
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = HubConfig::default();
        assert!(!config.enabled);
        assert_eq!(config.api_url, "http://localhost:3000");
        assert_eq!(config.usage_report_interval, 300);
        assert_eq!(config.cache_ttl, 60);
        assert_eq!(config.timeout, 30);
    }

    #[test]
    fn test_validation_disabled() {
        let config = HubConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_validation_enabled_missing_api_key() {
        let config = HubConfig {
            enabled: true,
            api_url: "https://hub.example.com".to_string(),
            ..Default::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_validation_enabled_valid() {
        let config = HubConfig {
            enabled: true,
            api_url: "https://hub.example.com".to_string(),
            service_api_key: "test-key".to_string(),
            ..Default::default()
        };
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_validation_invalid_interval() {
        let config = HubConfig {
            enabled: true,
            api_url: "https://hub.example.com".to_string(),
            service_api_key: "test-key".to_string(),
            usage_report_interval: 30, // Too low
            ..Default::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_duration_conversions() {
        let config = HubConfig::default();
        assert_eq!(config.timeout_duration(), Duration::from_secs(30));
        assert_eq!(config.cache_ttl_duration(), Duration::from_secs(60));
        assert_eq!(
            config.usage_report_interval_duration(),
            Duration::from_secs(300)
        );
    }
}
