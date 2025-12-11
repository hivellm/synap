//! HiveHub.Cloud Integration Module
//!
//! This module provides integration with HiveHub.Cloud for multi-tenant SaaS deployments.
//! It includes:
//! - Hub client wrapper for SDK integration
//! - User-scoped resource naming
//! - Quota management and enforcement
//! - Usage tracking and reporting
//! - SaaS restrictions for shared server protection

pub mod sdk;

pub mod client;

pub mod config;

pub mod extractor;

pub mod hub_auth;

pub mod naming;

pub mod multi_tenant;

pub mod quota;

pub mod restrictions;

pub mod usage;

pub mod cluster_quota;

pub use client::HubClient;

pub use config::HubConfig;

pub use extractor::HubContextExtractor;

pub use hub_auth::{
    HubUserContext, extract_access_key, hub_auth_middleware, require_hub_auth_middleware,
};

pub use multi_tenant::MultiTenant;

pub use naming::ResourceNaming;

pub use quota::QuotaManager;

pub use restrictions::{HubSaaSRestrictions, Plan, is_standalone_mode, require_standalone_mode};

pub use usage::UsageReporter;

pub use cluster_quota::ClusterQuotaManager;
