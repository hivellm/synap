//! Stub types for HiveHub SDK
//!
//! These are temporary stubs until the actual SDK is available.
//! They allow the hub-integration feature to compile without the external dependency.

use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

/// Stub HiveHub Cloud client
pub struct HiveHubCloudClient {
    _api_key: String,
    _api_url: String,
}

impl HiveHubCloudClient {
    pub fn new(api_key: String, api_url: String) -> Result<Self, HiveHubCloudError> {
        Ok(Self {
            _api_key: api_key,
            _api_url: api_url,
        })
    }

    pub fn synap(&self) -> SynapApi {
        SynapApi
    }
}

/// Stub Synap API
pub struct SynapApi;

impl SynapApi {
    pub async fn update_usage(
        &self,
        _user_id: &Uuid,
        _request: &SynapUpdateUsageRequest,
    ) -> Result<(), HiveHubCloudError> {
        Err(HiveHubCloudError::Unknown(
            "SDK not implemented".to_string(),
        ))
    }

    pub async fn create_resource(
        &self,
        _user_id: &Uuid,
        _request: &CreateResourceRequest,
    ) -> Result<CreateResourceResponse, HiveHubCloudError> {
        Err(HiveHubCloudError::Unknown(
            "SDK not implemented".to_string(),
        ))
    }
}

/// Resource type enum
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceType {
    KeyValue,
    Queue,
    Stream,
    PubSub,
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
}

/// HiveHub Cloud SDK errors
#[derive(Debug)]
#[allow(dead_code)]
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
