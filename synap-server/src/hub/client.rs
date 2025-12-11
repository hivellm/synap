//! HiveHub Client Wrapper
//!
//! Wrapper around HiveHubCloudClient SDK with caching and quota management integration.

use super::config::HubConfig;
use super::quota::{QuotaManager, UserQuota};
use super::restrictions::Plan;
use super::sdk_stubs::{
    CreateResourceRequest, HiveHubCloudClient, HiveHubCloudError, ResourceType,
    SynapUpdateUsageRequest,
};
use super::usage::UsageReporter;
use crate::core::error::SynapError;

use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{debug, error, info};
use uuid::Uuid;

/// Cached access key validation result
#[derive(Debug, Clone)]
struct AccessKeyCache {
    user_id: Uuid,
    plan: Plan,
    cached_at: Instant,
}

/// HiveHub client wrapper with caching and quota management
pub struct HubClient {
    /// Underlying SDK client
    sdk_client: Arc<HiveHubCloudClient>,
    /// Quota manager
    quota_manager: Arc<QuotaManager>,
    /// Usage reporter
    usage_reporter: Arc<UsageReporter>,
    /// Access key validation cache (access_key -> AccessKeyCache)
    access_key_cache: Arc<RwLock<HashMap<String, AccessKeyCache>>>,
    /// Configuration
    config: HubConfig,
}

impl HubClient {
    /// Create a new HubClient
    ///
    /// # Errors
    /// Returns error if SDK client initialization fails
    pub fn new(config: HubConfig) -> Result<Self, SynapError> {
        // Validate configuration
        config.validate().map_err(|e| {
            SynapError::InternalServerError(format!("Invalid Hub configuration: {}", e))
        })?;

        // Initialize SDK client
        let sdk_client =
            HiveHubCloudClient::new(config.service_api_key.clone(), config.api_url.clone())
                .map_err(|e| {
                    SynapError::InternalServerError(format!(
                        "Failed to initialize Hub SDK client: {}",
                        e
                    ))
                })?;

        info!("HubClient initialized with Hub URL: {}", config.api_url);

        Ok(Self {
            sdk_client: Arc::new(sdk_client),
            quota_manager: Arc::new(QuotaManager::new(config.cache_ttl_duration())),
            usage_reporter: Arc::new(UsageReporter::new(Duration::from_secs(
                config.usage_report_interval,
            ))),
            access_key_cache: Arc::new(RwLock::new(HashMap::new())),
            config,
        })
    }

    /// Validate access key and return user_id + plan
    ///
    /// Uses 60s cache to avoid hitting Hub API on every request
    pub async fn validate_access_key(&self, access_key: &str) -> Result<(Uuid, Plan), SynapError> {
        // Check cache first
        {
            let cache = self.access_key_cache.read();
            if let Some(cached) = cache.get(access_key) {
                if cached.cached_at.elapsed() < self.config.cache_ttl_duration() {
                    debug!(
                        "Access key validation cache hit for user {}",
                        cached.user_id
                    );
                    return Ok((cached.user_id, cached.plan));
                }
            }
        }

        // Cache miss - validate with Hub API
        debug!("Access key validation cache miss - calling Hub API");

        // TODO: Call Hub SDK to validate access key
        // For now, return error as the SDK doesn't have this method yet
        // Once implemented in HiveHub:
        // let validation = self.sdk_client.access_keys().validate(access_key).await?;

        Err(SynapError::InternalServerError(
            "Access key validation not yet implemented in Hub SDK".to_string(),
        ))
    }

    /// Get user quota (from cache or Hub API)
    pub async fn get_user_quota(&self, user_id: &Uuid) -> Result<UserQuota, SynapError> {
        // Check cache first
        if let Some(quota) = self.quota_manager.get_quota(user_id) {
            debug!("Quota cache hit for user {}", user_id);
            return Ok(quota);
        }

        // Cache miss - fetch from Hub API
        debug!("Quota cache miss for user {} - fetching from Hub", user_id);

        // TODO: Implement quota fetching from Hub
        // let response = self.sdk_client.synap().get_user_quota(user_id).await?;

        Err(SynapError::InternalServerError(
            "Quota fetching not yet implemented".to_string(),
        ))
    }

    // TODO: Implement quota checking once HiveHub API is available
    //     /// Check quota before resource creation
    //     pub async fn check_quota_for_resource(
    //         &self,
    //         user_id: &Uuid,
    //         resource_type: ResourceType,
    //         resource_name: &str,
    //         estimated_bytes: u64,
    //     ) -> Result<(), SynapError> {
    //         let request = QuotaCheckRequest {
    //             user_id: *user_id,
    //             resource_type,
    //             resource_name: resource_name.to_string(),
    //             estimated_bytes: Some(estimated_bytes),
    //         };
    //
    //         self.sdk_client
    //             .synap()
    //             .check_quota(&request)
    //             .await
    //             .map_err(Self::convert_sdk_error)?;
    //
    //         Ok(())
    //     }

    /// Update usage metrics for a user
    pub async fn update_usage(
        &self,
        user_id: &Uuid,
        resource_type: ResourceType,
        resource_name: &str,
        message_count: Option<u64>,
        storage_bytes: Option<u64>,
    ) -> Result<(), SynapError> {
        let request = SynapUpdateUsageRequest {
            resource_type,
            resource_name: resource_name.to_string(),
            message_count,
            storage_bytes,
        };

        self.sdk_client
            .synap()
            .update_usage(user_id, &request)
            .await
            .map_err(Self::convert_sdk_error)?;

        debug!(
            "Updated usage for user {} - resource: {} (messages: {:?}, bytes: {:?})",
            user_id, resource_name, message_count, storage_bytes
        );

        Ok(())
    }

    /// Request resource creation (validates quota via Hub)
    pub async fn create_resource(
        &self,
        user_id: &Uuid,
        request: &CreateResourceRequest,
    ) -> Result<String, SynapError> {
        let response = self
            .sdk_client
            .synap()
            .create_resource(user_id, request)
            .await
            .map_err(Self::convert_sdk_error)?;

        info!(
            "Resource created for user {}: {}",
            user_id, response.full_resource_name
        );

        Ok(response.full_resource_name)
    }

    /// Get quota manager reference
    pub fn quota_manager(&self) -> &Arc<QuotaManager> {
        &self.quota_manager
    }

    /// Get usage reporter reference
    pub fn usage_reporter(&self) -> &Arc<UsageReporter> {
        &self.usage_reporter
    }

    /// Invalidate access key cache for a specific key
    pub fn invalidate_access_key_cache(&self, access_key: &str) {
        let mut cache = self.access_key_cache.write();
        cache.remove(access_key);
    }

    /// Clear all caches
    pub fn clear_all_caches(&self) {
        {
            let mut cache = self.access_key_cache.write();
            cache.clear();
        }
        self.quota_manager.clear_cache();
        info!("All Hub client caches cleared");
    }

    /// Convert Hub SDK error to Synap error
    fn convert_sdk_error(err: HiveHubCloudError) -> SynapError {
        match err {
            HiveHubCloudError::Authentication(_) => {
                SynapError::Unauthorized("Invalid or expired access key".to_string())
            }
            HiveHubCloudError::QuotaExceeded(msg) => SynapError::QuotaExceeded(msg),
            HiveHubCloudError::NotFound(_) => {
                SynapError::ResourceNotFound("Resource not found in Hub".to_string())
            }
            HiveHubCloudError::BadRequest(msg) => SynapError::BadRequest(msg),
            HiveHubCloudError::Http(e) => {
                error!("Hub API HTTP error: {}", e);
                SynapError::InternalServerError(format!("Hub API HTTP error: {}", e))
            }
            HiveHubCloudError::Unknown(msg) => {
                error!("Hub API server error: {}", msg);
                SynapError::InternalServerError(format!("Hub API error: {}", msg))
            }
        }
    }

    /// Get cache statistics
    pub fn get_cache_stats(&self) -> HubClientStats {
        let access_key_cache_size = self.access_key_cache.read().len();
        let quota_stats = self.quota_manager.get_stats();

        HubClientStats {
            access_key_cache_size,
            quota_cache_size: quota_stats.cached_users,
            cache_ttl_seconds: self.config.cache_ttl,
        }
    }
}

#[derive(Debug, Clone)]
pub struct HubClientStats {
    pub access_key_cache_size: usize,
    pub quota_cache_size: usize,
    pub cache_ttl_seconds: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_config() -> HubConfig {
        HubConfig {
            enabled: true,
            api_url: "http://localhost:12000".to_string(),
            service_api_key: "test_api_key".to_string(),
            usage_report_interval: 300,
            cache_ttl: 60,
            timeout: 30,
        }
    }

    #[test]
    fn test_hub_client_creation() {
        let config = create_test_config();
        let result = HubClient::new(config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_hub_client_invalid_config() {
        let mut config = create_test_config();
        config.service_api_key = String::new(); // Invalid - empty key
        let result = HubClient::new(config);
        assert!(result.is_err());
    }

    #[test]
    fn test_get_cache_stats() {
        let config = create_test_config();
        let client = HubClient::new(config).unwrap();
        let stats = client.get_cache_stats();
        assert_eq!(stats.access_key_cache_size, 0);
        assert_eq!(stats.quota_cache_size, 0);
        assert_eq!(stats.cache_ttl_seconds, 60);
    }

    #[test]
    fn test_clear_all_caches() {
        let config = create_test_config();
        let client = HubClient::new(config).unwrap();
        client.clear_all_caches();
        let stats = client.get_cache_stats();
        assert_eq!(stats.access_key_cache_size, 0);
        assert_eq!(stats.quota_cache_size, 0);
    }

    #[test]
    fn test_convert_sdk_error_auth() {
        let sdk_err = HiveHubCloudError::Authentication("Invalid key".to_string());
        let synap_err = HubClient::convert_sdk_error(sdk_err);
        assert!(matches!(synap_err, SynapError::Unauthorized(_)));
    }

    #[test]
    fn test_convert_sdk_error_quota() {
        let sdk_err = HiveHubCloudError::QuotaExceeded("Quota exceeded".to_string());
        let synap_err = HubClient::convert_sdk_error(sdk_err);
        assert!(matches!(synap_err, SynapError::QuotaExceeded(_)));
    }
}
