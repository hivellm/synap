//! SaaS Security Restrictions
//!
//! CRITICAL: Mandatory restrictions for shared SaaS environment to protect
//! the shared server from resource abuse. These restrictions are ONLY applied
//! in Hub/SaaS mode (hub.enabled = true). Standalone mode has NO restrictions.
//!
//! User requirement: "operacoes do synap em modo cluster obrigatoriamente sempre
//! precisa de TTL, e valores baixos e limite das operacoes, pois o servidor e
//! compartilhado entre varios usuarios"

use std::time::Duration;

use super::hub_auth::HubUserContext;
use crate::SynapError;

/// Plan-based limits for SaaS mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Plan {
    Free,
    Pro,
    Enterprise,
}

impl Plan {
    /// Get plan from string
    pub fn parse_plan(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "free" => Some(Plan::Free),
            "pro" => Some(Plan::Pro),
            "enterprise" => Some(Plan::Enterprise),
            _ => None,
        }
    }
}

/// SaaS security restrictions manager
pub struct HubSaaSRestrictions;

impl HubSaaSRestrictions {
    // ============================================================================
    // TTL RESTRICTIONS (MANDATORY IN SAAS MODE)
    // ============================================================================

    /// Maximum TTL allowed per plan (seconds)
    /// Free: 24 hours, Pro: 7 days, Enterprise: 30 days
    pub const fn max_ttl_seconds(plan: Plan) -> u64 {
        match plan {
            Plan::Free => 86_400,          // 24 hours
            Plan::Pro => 604_800,          // 7 days
            Plan::Enterprise => 2_592_000, // 30 days
        }
    }

    /// Default TTL when not specified (24 hours for all plans)
    pub const DEFAULT_TTL_SECONDS: u64 = 86_400;

    /// Minimum TTL (5 minutes for all plans)
    pub const MIN_TTL_SECONDS: u64 = 300;

    /// Get default TTL as Duration
    pub fn default_ttl() -> Duration {
        Duration::from_secs(Self::DEFAULT_TTL_SECONDS)
    }

    /// Validate and enforce TTL limits
    ///
    /// Returns:
    /// - Ok(Duration) with validated/clamped TTL
    /// - Err if TTL is invalid
    pub fn enforce_ttl(ttl: Option<Duration>, plan: Plan) -> Result<Duration, String> {
        let ttl_secs = match ttl {
            Some(d) => d.as_secs(),
            None => {
                // TTL is MANDATORY in SaaS mode - apply default
                return Ok(Self::default_ttl());
            }
        };

        let max_ttl = Self::max_ttl_seconds(plan);

        if ttl_secs < Self::MIN_TTL_SECONDS {
            return Err(format!(
                "TTL must be at least {} seconds (5 minutes)",
                Self::MIN_TTL_SECONDS
            ));
        }

        if ttl_secs > max_ttl {
            return Err(format!(
                "TTL exceeds plan limit of {} seconds ({} plan)",
                max_ttl,
                match plan {
                    Plan::Free => "Free",
                    Plan::Pro => "Pro",
                    Plan::Enterprise => "Enterprise",
                }
            ));
        }

        Ok(Duration::from_secs(ttl_secs))
    }

    // ============================================================================
    // PAYLOAD SIZE RESTRICTIONS
    // ============================================================================

    /// Maximum payload size per plan (bytes)
    /// Free: 256KB, Pro: 1MB, Enterprise: 10MB
    pub const fn max_payload_bytes(plan: Plan) -> usize {
        match plan {
            Plan::Free => 262_144,          // 256 KB
            Plan::Pro => 1_048_576,         // 1 MB
            Plan::Enterprise => 10_485_760, // 10 MB
        }
    }

    /// Validate payload size
    pub fn validate_payload_size(size: usize, plan: Plan) -> Result<(), String> {
        let max_size = Self::max_payload_bytes(plan);
        if size > max_size {
            return Err(format!(
                "Payload size {} bytes exceeds plan limit of {} bytes",
                size, max_size
            ));
        }
        Ok(())
    }

    // ============================================================================
    // RATE LIMITING
    // ============================================================================

    /// Requests per second per user
    /// Free: 10 req/s, Pro: 100 req/s, Enterprise: 1000 req/s
    pub const fn max_requests_per_second(plan: Plan) -> u32 {
        match plan {
            Plan::Free => 10,
            Plan::Pro => 100,
            Plan::Enterprise => 1000,
        }
    }

    // ============================================================================
    // BATCH OPERATION LIMITS
    // ============================================================================

    /// Maximum batch size per plan
    /// Free: 10, Pro: 100, Enterprise: 1000
    pub const fn max_batch_size(plan: Plan) -> usize {
        match plan {
            Plan::Free => 10,
            Plan::Pro => 100,
            Plan::Enterprise => 1000,
        }
    }

    /// Validate batch size
    pub fn validate_batch_size(size: usize, plan: Plan) -> Result<(), String> {
        let max_size = Self::max_batch_size(plan);
        if size > max_size {
            return Err(format!(
                "Batch size {} exceeds plan limit of {}",
                size, max_size
            ));
        }
        Ok(())
    }

    // ============================================================================
    // OPERATION TIMEOUTS
    // ============================================================================

    /// Maximum operation timeout per plan
    /// Free: 10s, Pro: 30s, Enterprise: 60s
    pub const fn max_operation_timeout_seconds(plan: Plan) -> u64 {
        match plan {
            Plan::Free => 10,
            Plan::Pro => 30,
            Plan::Enterprise => 60,
        }
    }

    // ============================================================================
    // STORAGE QUOTAS (enforced via QuotaManager)
    // ============================================================================

    /// Maximum storage per user per plan (bytes)
    /// Free: 100MB, Pro: 10GB, Enterprise: 100GB
    pub const fn max_storage_bytes(plan: Plan) -> u64 {
        match plan {
            Plan::Free => 104_857_600,           // 100 MB
            Plan::Pro => 10_737_418_240,         // 10 GB
            Plan::Enterprise => 107_374_182_400, // 100 GB
        }
    }

    // ============================================================================
    // RESOURCE COUNT LIMITS
    // ============================================================================

    /// Maximum number of queues per user per plan
    pub const fn max_queues(plan: Plan) -> usize {
        match plan {
            Plan::Free => 10,
            Plan::Pro => 100,
            Plan::Enterprise => 1000,
        }
    }

    /// Maximum number of streams per user per plan
    pub const fn max_streams(plan: Plan) -> usize {
        match plan {
            Plan::Free => 10,
            Plan::Pro => 100,
            Plan::Enterprise => 1000,
        }
    }

    /// Maximum number of keys in KV store per user per plan
    pub const fn max_kv_keys(plan: Plan) -> usize {
        match plan {
            Plan::Free => 1_000,
            Plan::Pro => 100_000,
            Plan::Enterprise => 1_000_000,
        }
    }
}

// ============================================================================
// DANGEROUS COMMAND BLOCKING
// ============================================================================

/// Check if Hub mode is active and block operation if so
///
/// Use this for commands that should ONLY work in standalone mode:
/// - System-wide data destruction (FLUSH, script flush/kill)
/// - Global enumeration (client list, scan all keys)
/// - Commands that could affect other users
///
/// # Arguments
/// * `hub_ctx` - Optional Hub context (Some = Hub mode, None = standalone)
///
/// # Returns
/// * Ok(()) if standalone mode (command allowed)
/// * Err(Forbidden) if Hub mode (command blocked)
pub fn require_standalone_mode(hub_ctx: &Option<HubUserContext>) -> Result<(), SynapError> {
    if hub_ctx.is_some() {
        return Err(SynapError::Forbidden(
            "This command is not available in Hub mode for security reasons".to_string(),
        ));
    }
    Ok(())
}

/// Check if a command is allowed in Hub mode
///
/// This is the inverse of require_standalone_mode - returns true if Hub mode is NOT active
pub fn is_standalone_mode(hub_ctx: &Option<HubUserContext>) -> bool {
    hub_ctx.is_none()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plan_from_str() {
        assert_eq!(Plan::parse_plan("free"), Some(Plan::Free));
        assert_eq!(Plan::parse_plan("Free"), Some(Plan::Free));
        assert_eq!(Plan::parse_plan("FREE"), Some(Plan::Free));
        assert_eq!(Plan::parse_plan("pro"), Some(Plan::Pro));
        assert_eq!(Plan::parse_plan("enterprise"), Some(Plan::Enterprise));
        assert_eq!(Plan::parse_plan("invalid"), None);
    }

    #[test]
    fn test_enforce_ttl_default() {
        // No TTL provided - should return default
        let result = HubSaaSRestrictions::enforce_ttl(None, Plan::Free);
        assert!(result.is_ok());
        assert_eq!(
            result.unwrap().as_secs(),
            HubSaaSRestrictions::DEFAULT_TTL_SECONDS
        );
    }

    #[test]
    fn test_enforce_ttl_valid() {
        let ttl = Duration::from_secs(3600); // 1 hour
        let result = HubSaaSRestrictions::enforce_ttl(Some(ttl), Plan::Free);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().as_secs(), 3600);
    }

    #[test]
    fn test_enforce_ttl_too_low() {
        let ttl = Duration::from_secs(60); // 1 minute - below minimum
        let result = HubSaaSRestrictions::enforce_ttl(Some(ttl), Plan::Free);
        assert!(result.is_err());
    }

    #[test]
    fn test_enforce_ttl_exceeds_free() {
        let ttl = Duration::from_secs(172_800); // 2 days - exceeds Free limit
        let result = HubSaaSRestrictions::enforce_ttl(Some(ttl), Plan::Free);
        assert!(result.is_err());
    }

    #[test]
    fn test_enforce_ttl_valid_for_pro() {
        let ttl = Duration::from_secs(172_800); // 2 days - valid for Pro
        let result = HubSaaSRestrictions::enforce_ttl(Some(ttl), Plan::Pro);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_payload_size() {
        // Free plan - 256KB limit
        assert!(HubSaaSRestrictions::validate_payload_size(100_000, Plan::Free).is_ok());
        assert!(HubSaaSRestrictions::validate_payload_size(300_000, Plan::Free).is_err());

        // Pro plan - 1MB limit
        assert!(HubSaaSRestrictions::validate_payload_size(500_000, Plan::Pro).is_ok());
        assert!(HubSaaSRestrictions::validate_payload_size(2_000_000, Plan::Pro).is_err());
    }

    #[test]
    fn test_validate_batch_size() {
        // Free plan - 10 limit
        assert!(HubSaaSRestrictions::validate_batch_size(5, Plan::Free).is_ok());
        assert!(HubSaaSRestrictions::validate_batch_size(20, Plan::Free).is_err());

        // Pro plan - 100 limit
        assert!(HubSaaSRestrictions::validate_batch_size(50, Plan::Pro).is_ok());
        assert!(HubSaaSRestrictions::validate_batch_size(200, Plan::Pro).is_err());
    }

    #[test]
    fn test_max_ttl_seconds() {
        assert_eq!(HubSaaSRestrictions::max_ttl_seconds(Plan::Free), 86_400);
        assert_eq!(HubSaaSRestrictions::max_ttl_seconds(Plan::Pro), 604_800);
        assert_eq!(
            HubSaaSRestrictions::max_ttl_seconds(Plan::Enterprise),
            2_592_000
        );
    }

    #[test]
    fn test_max_requests_per_second() {
        assert_eq!(HubSaaSRestrictions::max_requests_per_second(Plan::Free), 10);
        assert_eq!(HubSaaSRestrictions::max_requests_per_second(Plan::Pro), 100);
        assert_eq!(
            HubSaaSRestrictions::max_requests_per_second(Plan::Enterprise),
            1000
        );
    }

    #[test]
    fn test_resource_limits() {
        // Queues
        assert_eq!(HubSaaSRestrictions::max_queues(Plan::Free), 10);
        assert_eq!(HubSaaSRestrictions::max_queues(Plan::Pro), 100);
        assert_eq!(HubSaaSRestrictions::max_queues(Plan::Enterprise), 1000);

        // Streams
        assert_eq!(HubSaaSRestrictions::max_streams(Plan::Free), 10);
        assert_eq!(HubSaaSRestrictions::max_streams(Plan::Pro), 100);
        assert_eq!(HubSaaSRestrictions::max_streams(Plan::Enterprise), 1000);

        // KV Keys
        assert_eq!(HubSaaSRestrictions::max_kv_keys(Plan::Free), 1_000);
        assert_eq!(HubSaaSRestrictions::max_kv_keys(Plan::Pro), 100_000);
        assert_eq!(
            HubSaaSRestrictions::max_kv_keys(Plan::Enterprise),
            1_000_000
        );
    }

    #[test]
    fn test_storage_limits() {
        assert_eq!(
            HubSaaSRestrictions::max_storage_bytes(Plan::Free),
            104_857_600
        );
        assert_eq!(
            HubSaaSRestrictions::max_storage_bytes(Plan::Pro),
            10_737_418_240
        );
        assert_eq!(
            HubSaaSRestrictions::max_storage_bytes(Plan::Enterprise),
            107_374_182_400
        );
    }

    #[test]
    fn test_require_standalone_mode_with_hub_context() {
        // Create Hub user context
        let user_id = uuid::Uuid::new_v4();
        let hub_ctx = Some(HubUserContext::new(
            user_id,
            Plan::Free,
            "test_key".to_string(),
        ));

        // Should return Forbidden error in Hub mode
        let result = require_standalone_mode(&hub_ctx);
        assert!(result.is_err());
        assert!(matches!(result, Err(SynapError::Forbidden(_))));
    }

    #[test]
    fn test_require_standalone_mode_without_hub_context() {
        // No Hub context = standalone mode
        let hub_ctx: Option<HubUserContext> = None;

        // Should allow operation in standalone mode
        let result = require_standalone_mode(&hub_ctx);
        assert!(result.is_ok());
    }

    #[test]
    fn test_is_standalone_mode_with_hub() {
        let user_id = uuid::Uuid::new_v4();
        let hub_ctx = Some(HubUserContext::new(
            user_id,
            Plan::Pro,
            "test_key".to_string(),
        ));

        // Should return false when Hub context is present
        assert!(!is_standalone_mode(&hub_ctx));
    }

    #[test]
    fn test_is_standalone_mode_without_hub() {
        let hub_ctx: Option<HubUserContext> = None;

        // Should return true when no Hub context
        assert!(is_standalone_mode(&hub_ctx));
    }

    #[test]
    fn test_plan_based_restrictions_free_vs_pro() {
        // Free plan has more restrictive limits than Pro
        assert!(
            HubSaaSRestrictions::max_ttl_seconds(Plan::Free)
                < HubSaaSRestrictions::max_ttl_seconds(Plan::Pro)
        );
        assert!(
            HubSaaSRestrictions::max_payload_bytes(Plan::Free)
                < HubSaaSRestrictions::max_payload_bytes(Plan::Pro)
        );
        assert!(
            HubSaaSRestrictions::max_batch_size(Plan::Free)
                < HubSaaSRestrictions::max_batch_size(Plan::Pro)
        );
        assert!(
            HubSaaSRestrictions::max_requests_per_second(Plan::Free)
                < HubSaaSRestrictions::max_requests_per_second(Plan::Pro)
        );
        assert!(
            HubSaaSRestrictions::max_queues(Plan::Free)
                < HubSaaSRestrictions::max_queues(Plan::Pro)
        );
        assert!(
            HubSaaSRestrictions::max_streams(Plan::Free)
                < HubSaaSRestrictions::max_streams(Plan::Pro)
        );
        assert!(
            HubSaaSRestrictions::max_storage_bytes(Plan::Free)
                < HubSaaSRestrictions::max_storage_bytes(Plan::Pro)
        );
    }

    #[test]
    fn test_plan_based_restrictions_pro_vs_enterprise() {
        // Pro plan has more restrictive limits than Enterprise
        assert!(
            HubSaaSRestrictions::max_ttl_seconds(Plan::Pro)
                < HubSaaSRestrictions::max_ttl_seconds(Plan::Enterprise)
        );
        assert!(
            HubSaaSRestrictions::max_payload_bytes(Plan::Pro)
                < HubSaaSRestrictions::max_payload_bytes(Plan::Enterprise)
        );
        assert!(
            HubSaaSRestrictions::max_batch_size(Plan::Pro)
                < HubSaaSRestrictions::max_batch_size(Plan::Enterprise)
        );
        assert!(
            HubSaaSRestrictions::max_requests_per_second(Plan::Pro)
                < HubSaaSRestrictions::max_requests_per_second(Plan::Enterprise)
        );
        assert!(
            HubSaaSRestrictions::max_queues(Plan::Pro)
                < HubSaaSRestrictions::max_queues(Plan::Enterprise)
        );
        assert!(
            HubSaaSRestrictions::max_streams(Plan::Pro)
                < HubSaaSRestrictions::max_streams(Plan::Enterprise)
        );
        assert!(
            HubSaaSRestrictions::max_storage_bytes(Plan::Pro)
                < HubSaaSRestrictions::max_storage_bytes(Plan::Enterprise)
        );
    }

    #[test]
    fn test_operation_timeouts() {
        // Test operation timeout limits for each plan
        assert_eq!(
            HubSaaSRestrictions::max_operation_timeout_seconds(Plan::Free),
            10
        );
        assert_eq!(
            HubSaaSRestrictions::max_operation_timeout_seconds(Plan::Pro),
            30
        );
        assert_eq!(
            HubSaaSRestrictions::max_operation_timeout_seconds(Plan::Enterprise),
            60
        );
    }

    #[test]
    fn test_enforce_ttl_edge_cases() {
        // Test exactly at minimum TTL
        let min_ttl = Duration::from_secs(HubSaaSRestrictions::MIN_TTL_SECONDS);
        assert!(HubSaaSRestrictions::enforce_ttl(Some(min_ttl), Plan::Free).is_ok());

        // Test exactly at maximum TTL for Free plan
        let max_free_ttl = Duration::from_secs(HubSaaSRestrictions::max_ttl_seconds(Plan::Free));
        assert!(HubSaaSRestrictions::enforce_ttl(Some(max_free_ttl), Plan::Free).is_ok());

        // Test 1 second over maximum for Free plan
        let over_max = Duration::from_secs(HubSaaSRestrictions::max_ttl_seconds(Plan::Free) + 1);
        assert!(HubSaaSRestrictions::enforce_ttl(Some(over_max), Plan::Free).is_err());
    }

    #[test]
    fn test_validate_payload_edge_cases() {
        // Test exactly at limit
        let free_limit = HubSaaSRestrictions::max_payload_bytes(Plan::Free);
        assert!(HubSaaSRestrictions::validate_payload_size(free_limit, Plan::Free).is_ok());

        // Test 1 byte over limit
        assert!(HubSaaSRestrictions::validate_payload_size(free_limit + 1, Plan::Free).is_err());

        // Test Pro plan with Free plan limit (should be OK)
        assert!(HubSaaSRestrictions::validate_payload_size(free_limit, Plan::Pro).is_ok());
    }

    #[test]
    fn test_validate_batch_edge_cases() {
        // Test exactly at limit
        let free_batch_limit = HubSaaSRestrictions::max_batch_size(Plan::Free);
        assert!(HubSaaSRestrictions::validate_batch_size(free_batch_limit, Plan::Free).is_ok());

        // Test 1 over limit
        assert!(
            HubSaaSRestrictions::validate_batch_size(free_batch_limit + 1, Plan::Free).is_err()
        );

        // Test Enterprise plan with large batch
        let enterprise_batch = HubSaaSRestrictions::max_batch_size(Plan::Enterprise);
        assert!(
            HubSaaSRestrictions::validate_batch_size(enterprise_batch, Plan::Enterprise).is_ok()
        );
        assert!(HubSaaSRestrictions::validate_batch_size(enterprise_batch, Plan::Free).is_err());
    }
}
