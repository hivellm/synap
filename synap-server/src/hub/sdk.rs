//! HiveHub Cloud SDK Implementation
//!
//! HTTP client for communicating with HiveHub.Cloud API.

use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::time::Duration;
use uuid::Uuid;

/// HiveHub Cloud client
pub struct HiveHubCloudClient {
    http_client: Client,
    api_key: String,
    base_url: String,
}

impl HiveHubCloudClient {
    /// Create a new HiveHub client
    pub fn new(api_key: String, base_url: String) -> Result<Self, HiveHubCloudError> {
        if api_key.is_empty() {
            return Err(HiveHubCloudError::Authentication(
                "API key cannot be empty".to_string(),
            ));
        }

        let http_client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| HiveHubCloudError::Http(e.to_string()))?;

        Ok(Self {
            http_client,
            api_key,
            base_url: base_url.trim_end_matches('/').to_string(),
        })
    }

    /// Get Synap API interface
    pub fn synap(&self) -> SynapApi<'_> {
        SynapApi { client: self }
    }

    /// Validate an access key
    pub async fn validate_access_key(
        &self,
        access_key: &str,
    ) -> Result<AccessKeyValidation, HiveHubCloudError> {
        let url = format!("{}/api/v1/access-keys/validate", self.base_url);

        let response = self
            .http_client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&serde_json::json!({ "access_key": access_key }))
            .send()
            .await
            .map_err(|e| HiveHubCloudError::Http(e.to_string()))?;

        Self::handle_response(response).await
    }

    /// Get user quota
    pub async fn get_user_quota(&self, user_id: &Uuid) -> Result<UserQuota, HiveHubCloudError> {
        let url = format!("{}/api/v1/synap/users/{}/quota", self.base_url, user_id);

        let response = self
            .http_client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .send()
            .await
            .map_err(|e| HiveHubCloudError::Http(e.to_string()))?;

        Self::handle_response(response).await
    }

    async fn handle_response<T: for<'de> Deserialize<'de>>(
        response: reqwest::Response,
    ) -> Result<T, HiveHubCloudError> {
        let status = response.status();

        if status.is_success() {
            response
                .json()
                .await
                .map_err(|e| HiveHubCloudError::Unknown(format!("Failed to parse response: {}", e)))
        } else {
            let error_body = response.text().await.unwrap_or_default();

            match status.as_u16() {
                401 => Err(HiveHubCloudError::Authentication(error_body)),
                403 => Err(HiveHubCloudError::QuotaExceeded(error_body)),
                404 => Err(HiveHubCloudError::NotFound(error_body)),
                400 => Err(HiveHubCloudError::BadRequest(error_body)),
                _ => Err(HiveHubCloudError::Unknown(format!(
                    "HTTP {}: {}",
                    status, error_body
                ))),
            }
        }
    }
}

/// Synap API interface
pub struct SynapApi<'a> {
    client: &'a HiveHubCloudClient,
}

impl<'a> SynapApi<'a> {
    /// Update usage metrics for a user
    pub async fn update_usage(
        &self,
        user_id: &Uuid,
        request: &SynapUpdateUsageRequest,
    ) -> Result<(), HiveHubCloudError> {
        let url = format!(
            "{}/api/v1/synap/users/{}/usage",
            self.client.base_url, user_id
        );

        let response = self
            .client
            .http_client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.client.api_key))
            .header("Content-Type", "application/json")
            .json(request)
            .send()
            .await
            .map_err(|e| HiveHubCloudError::Http(e.to_string()))?;

        if response.status().is_success() {
            Ok(())
        } else {
            let status = response.status();
            let error_body = response.text().await.unwrap_or_default();

            match status.as_u16() {
                401 => Err(HiveHubCloudError::Authentication(error_body)),
                403 => Err(HiveHubCloudError::QuotaExceeded(error_body)),
                404 => Err(HiveHubCloudError::NotFound(error_body)),
                400 => Err(HiveHubCloudError::BadRequest(error_body)),
                _ => Err(HiveHubCloudError::Unknown(format!(
                    "HTTP {}: {}",
                    status, error_body
                ))),
            }
        }
    }

    /// Create a resource (validates quota first)
    pub async fn create_resource(
        &self,
        user_id: &Uuid,
        request: &CreateResourceRequest,
    ) -> Result<CreateResourceResponse, HiveHubCloudError> {
        let url = format!(
            "{}/api/v1/synap/users/{}/resources",
            self.client.base_url, user_id
        );

        let response = self
            .client
            .http_client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.client.api_key))
            .header("Content-Type", "application/json")
            .json(request)
            .send()
            .await
            .map_err(|e| HiveHubCloudError::Http(e.to_string()))?;

        HiveHubCloudClient::handle_response(response).await
    }

    /// Check quota before operation
    pub async fn check_quota(
        &self,
        user_id: &Uuid,
        request: &QuotaCheckRequest,
    ) -> Result<QuotaCheckResponse, HiveHubCloudError> {
        let url = format!(
            "{}/api/v1/synap/users/{}/quota/check",
            self.client.base_url, user_id
        );

        let response = self
            .client
            .http_client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.client.api_key))
            .header("Content-Type", "application/json")
            .json(request)
            .send()
            .await
            .map_err(|e| HiveHubCloudError::Http(e.to_string()))?;

        HiveHubCloudClient::handle_response(response).await
    }
}

// ============================================================================
// Request/Response Types
// ============================================================================

/// Resource type enum
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceType {
    KeyValue,
    Queue,
    Stream,
    PubSub,
}

/// Access key validation response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessKeyValidation {
    pub valid: bool,
    pub user_id: Uuid,
    pub plan: String,
    pub permissions: Vec<String>,
}

/// User quota information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserQuota {
    pub user_id: Uuid,
    pub plan: String,
    pub storage_limit_bytes: u64,
    pub storage_used_bytes: u64,
    pub monthly_operations_limit: u64,
    pub monthly_operations_used: u64,
    pub max_keys: u64,
    pub max_queues: u64,
    pub max_streams: u64,
    pub max_pubsub_topics: u64,
}

/// Synap usage update request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SynapUpdateUsageRequest {
    pub resource_type: ResourceType,
    pub resource_name: String,
    pub message_count: Option<u64>,
    pub storage_bytes: Option<u64>,
}

/// Create resource request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateResourceRequest {
    pub resource_type: ResourceType,
    pub resource_name: String,
    pub estimated_bytes: Option<u64>,
}

/// Create resource response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateResourceResponse {
    pub full_resource_name: String,
    pub resource_id: String,
}

/// Quota check request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuotaCheckRequest {
    pub resource_type: ResourceType,
    pub operation: String,
    pub estimated_bytes: Option<u64>,
}

/// Quota check response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuotaCheckResponse {
    pub allowed: bool,
    pub reason: Option<String>,
    pub remaining_quota: Option<u64>,
}

// ============================================================================
// Error Types
// ============================================================================

/// HiveHub Cloud SDK errors
#[derive(Debug)]
pub enum HiveHubCloudError {
    Authentication(String),
    QuotaExceeded(String),
    NotFound(String),
    BadRequest(String),
    Unknown(String),
    Http(String),
}

impl fmt::Display for HiveHubCloudError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Authentication(msg) => write!(f, "Authentication error: {}", msg),
            Self::QuotaExceeded(msg) => write!(f, "Quota exceeded: {}", msg),
            Self::NotFound(msg) => write!(f, "Not found: {}", msg),
            Self::BadRequest(msg) => write!(f, "Bad request: {}", msg),
            Self::Unknown(msg) => write!(f, "Unknown error: {}", msg),
            Self::Http(msg) => write!(f, "HTTP error: {}", msg),
        }
    }
}

impl std::error::Error for HiveHubCloudError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_creation() {
        let client = HiveHubCloudClient::new(
            "test_api_key".to_string(),
            "http://localhost:12000".to_string(),
        );
        assert!(client.is_ok());
    }

    #[test]
    fn test_client_empty_api_key() {
        let client = HiveHubCloudClient::new(String::new(), "http://localhost:12000".to_string());
        assert!(client.is_err());
    }

    #[test]
    fn test_resource_type_serialization() {
        let kv = ResourceType::KeyValue;
        let json = serde_json::to_string(&kv).unwrap();
        assert_eq!(json, "\"key_value\"");
    }

    #[test]
    fn test_update_usage_request_serialization() {
        let request = SynapUpdateUsageRequest {
            resource_type: ResourceType::KeyValue,
            resource_name: "test_key".to_string(),
            message_count: Some(10),
            storage_bytes: Some(1024),
        };
        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("\"resource_type\":\"key_value\""));
        assert!(json.contains("\"resource_name\":\"test_key\""));
    }

    #[test]
    fn test_error_display() {
        let err = HiveHubCloudError::Authentication("Invalid token".to_string());
        assert_eq!(format!("{}", err), "Authentication error: Invalid token");
    }
}
