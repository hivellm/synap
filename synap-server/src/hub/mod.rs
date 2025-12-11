//! HiveHub.Cloud Integration Module
//!
//! This module provides integration with HiveHub.Cloud for multi-tenant SaaS deployments.
//! It includes:
//! - Hub client wrapper for SDK integration
//! - User-scoped resource naming
//! - Quota management and enforcement
//! - Usage tracking and reporting
//! - SaaS restrictions for shared server protection

#[cfg(feature = "hub-integration")]
pub mod client;

#[cfg(feature = "hub-integration")]
pub mod config;

#[cfg(feature = "hub-integration")]
pub mod extractor;

#[cfg(feature = "hub-integration")]
pub mod hub_auth;

#[cfg(feature = "hub-integration")]
pub mod naming;

#[cfg(feature = "hub-integration")]
pub mod multi_tenant;

#[cfg(feature = "hub-integration")]
pub mod quota;

#[cfg(feature = "hub-integration")]
pub mod restrictions;

#[cfg(feature = "hub-integration")]
pub mod usage;

#[cfg(feature = "hub-integration")]
pub mod cluster_quota;

#[cfg(feature = "hub-integration")]
pub use client::HubClient;

#[cfg(feature = "hub-integration")]
pub use config::HubConfig;

#[cfg(feature = "hub-integration")]
pub use extractor::HubContextExtractor;

#[cfg(feature = "hub-integration")]
pub use hub_auth::{
    HubUserContext, extract_access_key, hub_auth_middleware, require_hub_auth_middleware,
};

#[cfg(feature = "hub-integration")]
pub use multi_tenant::MultiTenant;

#[cfg(feature = "hub-integration")]
pub use naming::ResourceNaming;

#[cfg(feature = "hub-integration")]
pub use quota::QuotaManager;

#[cfg(feature = "hub-integration")]
pub use restrictions::{HubSaaSRestrictions, Plan, is_standalone_mode, require_standalone_mode};

#[cfg(feature = "hub-integration")]
pub use usage::UsageReporter;

#[cfg(feature = "hub-integration")]
pub use cluster_quota::ClusterQuotaManager;
