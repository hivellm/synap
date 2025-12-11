//! Quota Management
//!
//! Manages user quotas and usage tracking for SaaS mode.
//! Quotas are fetched from HiveHub and enforced before operations.

use super::client::HubClient;
use super::restrictions::Plan;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::time::interval;
use tracing::{debug, error, info};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct UserQuota {
    pub user_id: Uuid,
    pub plan: Plan,
    pub storage_used: u64,
    pub storage_limit: u64,
    pub monthly_operations: u64,
    pub monthly_operations_limit: u64,
    pub updated_at: Instant,
}

impl UserQuota {
    /// Check if storage quota is available
    pub fn has_storage_available(&self, required_bytes: u64) -> bool {
        self.storage_used + required_bytes <= self.storage_limit
    }

    /// Check if operations quota is available
    pub fn has_operations_available(&self) -> bool {
        self.monthly_operations < self.monthly_operations_limit
    }

    /// Get remaining storage bytes
    pub fn remaining_storage(&self) -> u64 {
        self.storage_limit.saturating_sub(self.storage_used)
    }

    /// Get remaining operations
    pub fn remaining_operations(&self) -> u64 {
        self.monthly_operations_limit
            .saturating_sub(self.monthly_operations)
    }

    /// Check if quota data is stale (older than cache TTL)
    pub fn is_stale(&self, cache_ttl: Duration) -> bool {
        self.updated_at.elapsed() > cache_ttl
    }
}

/// Quota manager with local cache
pub struct QuotaManager {
    /// Cached quotas per user (user_id -> UserQuota)
    cache: Arc<RwLock<HashMap<Uuid, UserQuota>>>,
    /// Cache TTL
    cache_ttl: Duration,
}

impl QuotaManager {
    /// Create a new quota manager
    pub fn new(cache_ttl: Duration) -> Self {
        Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
            cache_ttl,
        }
    }

    /// Get quota for a user (from cache if available)
    pub fn get_quota(&self, user_id: &Uuid) -> Option<UserQuota> {
        let cache = self.cache.read();
        cache
            .get(user_id)
            .filter(|q| !q.is_stale(self.cache_ttl))
            .cloned()
    }

    /// Update quota in cache
    pub fn update_quota(&self, quota: UserQuota) {
        let mut cache = self.cache.write();
        cache.insert(quota.user_id, quota);
    }

    /// Remove quota from cache (force refresh on next access)
    pub fn invalidate_quota(&self, user_id: &Uuid) {
        let mut cache = self.cache.write();
        cache.remove(user_id);
    }

    /// Clear all cached quotas
    pub fn clear_cache(&self) {
        let mut cache = self.cache.write();
        cache.clear();
    }

    /// Check if user can perform a storage operation
    pub fn check_storage_quota(&self, user_id: &Uuid, required_bytes: u64) -> Result<(), String> {
        let quota = self
            .get_quota(user_id)
            .ok_or_else(|| "Quota not found. Please authenticate.".to_string())?;

        if !quota.has_storage_available(required_bytes) {
            return Err(format!(
                "Storage quota exceeded. Used: {} bytes, Limit: {} bytes, Required: {} bytes",
                quota.storage_used, quota.storage_limit, required_bytes
            ));
        }

        Ok(())
    }

    /// Check if user can perform an operation
    pub fn check_operations_quota(&self, user_id: &Uuid) -> Result<(), String> {
        let quota = self
            .get_quota(user_id)
            .ok_or_else(|| "Quota not found. Please authenticate.".to_string())?;

        if !quota.has_operations_available() {
            return Err(format!(
                "Monthly operations quota exceeded. Used: {}, Limit: {}",
                quota.monthly_operations, quota.monthly_operations_limit
            ));
        }

        Ok(())
    }

    /// Increment storage usage locally (will be synced to Hub periodically)
    pub fn increment_storage(&self, user_id: &Uuid, bytes: u64) {
        let mut cache = self.cache.write();
        if let Some(quota) = cache.get_mut(user_id) {
            quota.storage_used = quota.storage_used.saturating_add(bytes);
            quota.updated_at = Instant::now();
        }
    }

    /// Decrement storage usage locally
    pub fn decrement_storage(&self, user_id: &Uuid, bytes: u64) {
        let mut cache = self.cache.write();
        if let Some(quota) = cache.get_mut(user_id) {
            quota.storage_used = quota.storage_used.saturating_sub(bytes);
            quota.updated_at = Instant::now();
        }
    }

    /// Increment operations count locally
    pub fn increment_operations(&self, user_id: &Uuid) {
        let mut cache = self.cache.write();
        if let Some(quota) = cache.get_mut(user_id) {
            quota.monthly_operations = quota.monthly_operations.saturating_add(1);
            quota.updated_at = Instant::now();
        }
    }

    /// Get cache statistics
    pub fn get_stats(&self) -> QuotaManagerStats {
        let cache = self.cache.read();
        QuotaManagerStats {
            cached_users: cache.len(),
            cache_ttl_seconds: self.cache_ttl.as_secs(),
        }
    }

    /// Start periodic quota sync background task
    ///
    /// Periodically refreshes stale quotas from HiveHub API to ensure
    /// local cache stays in sync with the source of truth.
    ///
    /// # Phase 4.6 - Periodic quota sync
    ///
    /// # Arguments
    /// * `hub_client` - HubClient for fetching quotas from Hub API
    /// * `sync_interval` - How often to check and refresh stale quotas
    pub async fn start_quota_sync_task(&self, hub_client: Arc<HubClient>, sync_interval: Duration) {
        let mut ticker = interval(sync_interval);
        let cache_ref = self.cache.clone();
        let cache_ttl = self.cache_ttl;

        info!(
            "Quota sync task started - sync interval: {} seconds, cache TTL: {} seconds",
            sync_interval.as_secs(),
            cache_ttl.as_secs()
        );

        loop {
            ticker.tick().await;

            // Get list of users with stale quotas
            let stale_users: Vec<Uuid> = {
                let cache = cache_ref.read();
                cache
                    .iter()
                    .filter(|(_, quota)| quota.is_stale(cache_ttl))
                    .map(|(user_id, _)| *user_id)
                    .collect()
            };

            if stale_users.is_empty() {
                debug!("No stale quotas to refresh");
                continue;
            }

            info!(
                "Refreshing quotas for {} users with stale cache",
                stale_users.len()
            );

            // Refresh each stale quota from Hub API
            for user_id in stale_users {
                debug!("Refreshing quota for user {}", user_id);

                match hub_client.get_user_quota(&user_id).await {
                    Ok(quota) => {
                        debug!(
                            "Successfully refreshed quota for user {}: storage={}/{}, ops={}/{}",
                            user_id,
                            quota.storage_used,
                            quota.storage_limit,
                            quota.monthly_operations,
                            quota.monthly_operations_limit
                        );
                        self.update_quota(quota);
                    }
                    Err(e) => {
                        // Don't fail the sync task - just log and continue
                        // If Hub API is temporarily unavailable, we keep using cached data
                        error!(
                            "Failed to refresh quota for user {}: {}. Will retry in {} seconds",
                            user_id,
                            e,
                            sync_interval.as_secs()
                        );
                    }
                }
            }

            debug!("Quota sync cycle completed");
        }
    }
}

#[derive(Debug, Clone)]
pub struct QuotaManagerStats {
    pub cached_users: usize,
    pub cache_ttl_seconds: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hub::restrictions::HubSaaSRestrictions;

    fn create_test_quota(user_id: Uuid) -> UserQuota {
        UserQuota {
            user_id,
            plan: Plan::Free,
            storage_used: 1_000_000,
            storage_limit: HubSaaSRestrictions::max_storage_bytes(Plan::Free),
            monthly_operations: 100,
            monthly_operations_limit: 10_000,
            updated_at: Instant::now(),
        }
    }

    #[test]
    fn test_user_quota_has_storage_available() {
        let quota = create_test_quota(Uuid::new_v4());
        assert!(quota.has_storage_available(1000));
        assert!(!quota.has_storage_available(quota.storage_limit));
    }

    #[test]
    fn test_user_quota_has_operations_available() {
        let quota = create_test_quota(Uuid::new_v4());
        assert!(quota.has_operations_available());

        let mut quota_exceeded = quota.clone();
        quota_exceeded.monthly_operations = quota_exceeded.monthly_operations_limit;
        assert!(!quota_exceeded.has_operations_available());
    }

    #[test]
    fn test_user_quota_remaining() {
        let quota = create_test_quota(Uuid::new_v4());
        assert_eq!(
            quota.remaining_storage(),
            quota.storage_limit - quota.storage_used
        );
        assert_eq!(
            quota.remaining_operations(),
            quota.monthly_operations_limit - quota.monthly_operations
        );
    }

    #[test]
    fn test_quota_manager_update_and_get() {
        let manager = QuotaManager::new(Duration::from_secs(60));
        let user_id = Uuid::new_v4();
        let quota = create_test_quota(user_id);

        manager.update_quota(quota.clone());
        let retrieved = manager.get_quota(&user_id);
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().user_id, user_id);
    }

    #[test]
    fn test_quota_manager_invalidate() {
        let manager = QuotaManager::new(Duration::from_secs(60));
        let user_id = Uuid::new_v4();
        let quota = create_test_quota(user_id);

        manager.update_quota(quota);
        assert!(manager.get_quota(&user_id).is_some());

        manager.invalidate_quota(&user_id);
        assert!(manager.get_quota(&user_id).is_none());
    }

    #[test]
    fn test_quota_manager_check_storage() {
        let manager = QuotaManager::new(Duration::from_secs(60));
        let user_id = Uuid::new_v4();
        let quota = create_test_quota(user_id);

        manager.update_quota(quota.clone());

        // Should succeed - within quota
        assert!(manager.check_storage_quota(&user_id, 1000).is_ok());

        // Should fail - exceeds quota
        assert!(
            manager
                .check_storage_quota(&user_id, quota.storage_limit)
                .is_err()
        );
    }

    #[test]
    fn test_quota_manager_check_operations() {
        let manager = QuotaManager::new(Duration::from_secs(60));
        let user_id = Uuid::new_v4();
        let mut quota = create_test_quota(user_id);

        manager.update_quota(quota.clone());
        assert!(manager.check_operations_quota(&user_id).is_ok());

        // Exceed operations quota
        quota.monthly_operations = quota.monthly_operations_limit;
        manager.update_quota(quota);
        assert!(manager.check_operations_quota(&user_id).is_err());
    }

    #[test]
    fn test_quota_manager_increment_decrement() {
        let manager = QuotaManager::new(Duration::from_secs(60));
        let user_id = Uuid::new_v4();
        let quota = create_test_quota(user_id);
        let initial_storage = quota.storage_used;

        manager.update_quota(quota);

        manager.increment_storage(&user_id, 500);
        let updated = manager.get_quota(&user_id).unwrap();
        assert_eq!(updated.storage_used, initial_storage + 500);

        manager.decrement_storage(&user_id, 200);
        let updated = manager.get_quota(&user_id).unwrap();
        assert_eq!(updated.storage_used, initial_storage + 300);
    }

    #[test]
    fn test_quota_manager_stats() {
        let manager = QuotaManager::new(Duration::from_secs(60));
        let user1 = Uuid::new_v4();
        let user2 = Uuid::new_v4();

        manager.update_quota(create_test_quota(user1));
        manager.update_quota(create_test_quota(user2));

        let stats = manager.get_stats();
        assert_eq!(stats.cached_users, 2);
        assert_eq!(stats.cache_ttl_seconds, 60);
    }

    #[test]
    fn test_quota_manager_clear_cache() {
        let manager = QuotaManager::new(Duration::from_secs(60));
        let user_id = Uuid::new_v4();
        manager.update_quota(create_test_quota(user_id));

        assert!(manager.get_quota(&user_id).is_some());
        manager.clear_cache();
        assert!(manager.get_quota(&user_id).is_none());
    }

    #[test]
    fn test_quota_is_stale() {
        let mut quota = create_test_quota(Uuid::new_v4());

        // Fresh quota
        assert!(!quota.is_stale(Duration::from_secs(60)));

        // Simulate old quota
        quota.updated_at = Instant::now() - Duration::from_secs(120);
        assert!(quota.is_stale(Duration::from_secs(60)));
    }
}
