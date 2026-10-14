//! HiveHub Client Wrapper
//!
//! Wrapper around HiveHubCloudClient SDK with caching and quota management integration.

use super::config::HubConfig;
use super::quota::{QuotaManager, UserQuota};
use super::restrictions::Plan;
use super::sdk::{
    CreateResourceRequest, HiveHubCloudClient, HiveHubCloudError, ResourceType,
    SynapUpdateUsageRequest,
};
use super::usage::UsageReporter;
use crate::core::error::SynapError;

use futures_util::future::BoxFuture;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Resolves a Hub access key to the tenant (Hub user) that owns it.
///
/// Hub mode authenticates every data request through this, so the tenant it
/// returns decides which namespace the request may touch.
pub trait AccessKeyValidator: Send + Sync {
    /// The owner and plan of `access_key`. `Err(SynapError::Unauthorized)`
    /// means the key is unknown, revoked or expired; any other error means the
    /// key could not be checked at all.
    fn validate<'a>(
        &'a self,
        access_key: &'a str,
    ) -> BoxFuture<'a, Result<(Uuid, Plan), SynapError>>;
}

/// Why the production validator refuses every key.
pub const ACCESS_KEY_VALIDATION_UNAVAILABLE: &str = "Hub access-key validation is unavailable: \
     the HiveHub SDK has no endpoint that validates an access-key secret (hivellm/synap#230)";

/// The production validator: the HiveHub API.
///
/// hivehub-internal-sdk 1.0.0 has no endpoint that validates an access-key
/// secret (it can generate, list, fetch by id and revoke keys only), so no key
/// can be validated and Hub mode refuses every data request until the SDK
/// gains one (hivellm/synap#230). Failing closed is deliberate: accepting a
/// key it cannot check would hand any caller a tenant of its choosing.
struct HubApiAccessKeyValidator;

impl AccessKeyValidator for HubApiAccessKeyValidator {
    fn validate<'a>(
        &'a self,
        _access_key: &'a str,
    ) -> BoxFuture<'a, Result<(Uuid, Plan), SynapError>> {
        Box::pin(async {
            Err(SynapError::InternalServerError(
                ACCESS_KEY_VALIDATION_UNAVAILABLE.to_string(),
            ))
        })
    }
}

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
    /// Resolves the access keys the cache does not hold
    validator: Arc<dyn AccessKeyValidator>,
    /// Configuration
    config: HubConfig,
}

impl HubClient {
    /// Create a new HubClient that validates access keys against the HiveHub
    /// API, which cannot validate any yet: every key is refused
    /// (hivellm/synap#230).
    ///
    /// # Errors
    /// Returns error if SDK client initialization fails
    pub fn new(config: HubConfig) -> Result<Self, SynapError> {
        warn!(
            "Hub mode: {}. Every request that needs a Hub access key is refused until it is resolved",
            ACCESS_KEY_VALIDATION_UNAVAILABLE
        );
        Self::with_validator(config, Arc::new(HubApiAccessKeyValidator))
    }

    /// Create a HubClient that resolves access keys with `validator`.
    ///
    /// # Errors
    /// Returns error if the configuration is invalid or SDK client
    /// initialization fails
    pub fn with_validator(
        config: HubConfig,
        validator: Arc<dyn AccessKeyValidator>,
    ) -> Result<Self, SynapError> {
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
            validator,
            config,
        })
    }

    /// Validate access key and return user_id + plan
    ///
    /// Successful validations are cached for `cache_ttl` seconds so the Hub
    /// API is not hit on every request; failures are never cached.
    pub async fn validate_access_key(&self, access_key: &str) -> Result<(Uuid, Plan), SynapError> {
        // Check cache first
        {
            let cache = self.access_key_cache.read();
            if let Some(cached) = cache.get(access_key)
                && cached.cached_at.elapsed() < self.config.cache_ttl_duration()
            {
                debug!(
                    "Access key validation cache hit for user {}",
                    cached.user_id
                );
                return Ok((cached.user_id, cached.plan));
            }
        }

        // Cache miss - ask the validator; only a successful answer is cached.
        debug!("Access key validation cache miss - calling the validator");
        let (user_id, plan) = self.validator.validate(access_key).await?;
        self.access_key_cache.write().insert(
            access_key.to_string(),
            AccessKeyCache {
                user_id,
                plan,
                cached_at: Instant::now(),
            },
        );
        Ok((user_id, plan))
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

        // Quota fetching from the Hub is not yet available — tracked in
        // hivellm/synap#230.
        // let response = self.sdk_client.synap().get_user_quota(user_id).await?;

        Err(SynapError::InternalServerError(
            "Quota fetching not yet implemented".to_string(),
        ))
    }

    // Quota checking awaits the HiveHub API — tracked in hivellm/synap#230.
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
            HiveHubCloudError::Authentication(msg) => {
                SynapError::Unauthorized(format!("Invalid or expired access key: {}", msg))
            }
            HiveHubCloudError::QuotaExceeded(msg) => SynapError::QuotaExceeded(msg),
            HiveHubCloudError::NotFound(msg) => SynapError::ResourceNotFound(msg),
            HiveHubCloudError::BadRequest(msg) => SynapError::BadRequest(msg),
            HiveHubCloudError::Validation(msg) => SynapError::BadRequest(msg),
            HiveHubCloudError::ServiceUnavailable(msg) => {
                error!("Hub service unavailable: {}", msg);
                SynapError::InternalServerError(format!("Hub service unavailable: {}", msg))
            }
            HiveHubCloudError::Http(e) => {
                error!("Hub API HTTP error: {}", e);
                SynapError::InternalServerError(format!("Hub API HTTP error: {}", e))
            }
            HiveHubCloudError::Serialization(e) => {
                error!("Hub API serialization error: {}", e);
                SynapError::InternalServerError(format!("Hub API serialization error: {}", e))
            }
            HiveHubCloudError::Configuration(msg) => {
                error!("Hub configuration error: {}", msg);
                SynapError::InternalServerError(format!("Hub configuration error: {}", msg))
            }
            HiveHubCloudError::Unknown(msg) => {
                error!("Hub API error: {}", msg);
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

    /// Resolves exactly one key and counts how often it is asked.
    struct OneKey {
        key: &'static str,
        user_id: Uuid,
        calls: std::sync::atomic::AtomicUsize,
    }

    impl AccessKeyValidator for OneKey {
        fn validate<'a>(
            &'a self,
            access_key: &'a str,
        ) -> BoxFuture<'a, Result<(Uuid, Plan), SynapError>> {
            self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            let known = access_key == self.key;
            let user_id = self.user_id;
            Box::pin(async move {
                if known {
                    Ok((user_id, Plan::Pro))
                } else {
                    Err(SynapError::Unauthorized("unknown key".to_string()))
                }
            })
        }
    }

    #[tokio::test]
    async fn production_validator_refuses_every_key_and_names_the_issue() {
        let client = HubClient::new(create_test_config()).unwrap();
        let err = client.validate_access_key("sk_any").await.unwrap_err();
        assert!(matches!(err, SynapError::InternalServerError(_)), "{err}");
        assert!(err.to_string().contains("hivellm/synap#230"), "{err}");
        assert_eq!(client.get_cache_stats().access_key_cache_size, 0);
    }

    #[tokio::test]
    async fn validated_keys_are_cached_and_failures_are_not() {
        let validator = Arc::new(OneKey {
            key: "good",
            user_id: Uuid::new_v4(),
            calls: std::sync::atomic::AtomicUsize::new(0),
        });
        let client = HubClient::with_validator(create_test_config(), validator.clone()).unwrap();

        let (user, plan) = client.validate_access_key("good").await.unwrap();
        assert_eq!(user, validator.user_id);
        assert_eq!(plan, Plan::Pro);
        client.validate_access_key("good").await.unwrap();
        assert_eq!(
            validator.calls.load(std::sync::atomic::Ordering::SeqCst),
            1,
            "the second lookup is a cache hit"
        );

        for _ in 0..2 {
            let err = client.validate_access_key("bad").await.unwrap_err();
            assert!(matches!(err, SynapError::Unauthorized(_)), "{err}");
        }
        assert_eq!(validator.calls.load(std::sync::atomic::Ordering::SeqCst), 3);
        assert_eq!(client.get_cache_stats().access_key_cache_size, 1);
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
