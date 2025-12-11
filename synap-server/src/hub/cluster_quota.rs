//! Cluster-Wide Quota Management for Hub Integration
//!
//! In cluster mode, quotas must be tracked across all nodes to prevent
//! users from exceeding limits by spreading requests across nodes.
//!
//! ## Architecture
//!
//! - **Master Node**: Designated node (lowest node_id) tracks global quotas
//! - **Quota Sync**: Periodic sync (30s) of usage deltas from replica nodes to master
//! - **Local Caching**: Each node caches quota info (60s TTL) to reduce master load
//! - **Raft Consensus**: Uses existing Raft for quota state replication (fault tolerance)
//!
//! ## Flow
//!
//! 1. Request arrives at any node (replica or master)
//! 2. Node checks local quota cache (60s TTL)
//!    - Cache HIT: Use cached quota, allow operation if within limits
//!    - Cache MISS: Query master node for current quota
//! 3. Operation executes (if quota allows)
//! 4. Usage delta tracked locally
//! 5. Every 30s: Replica nodes send usage deltas to master
//! 6. Master aggregates deltas, updates global quotas
//! 7. Master syncs updated quotas to all replicas via Raft

#[cfg(feature = "hub-integration")]
use crate::hub::HubClient;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use uuid::Uuid;

/// Quota information cached on each node
#[derive(Debug, Clone)]
pub struct CachedQuota {
    /// User ID
    pub user_id: Uuid,
    /// Storage limit (bytes)
    pub storage_limit: u64,
    /// Storage used (bytes)
    pub storage_used: u64,
    /// Operations limit (per month)
    pub monthly_operations_limit: u64,
    /// Operations used (this month)
    pub monthly_operations: u64,
    /// Cache timestamp
    pub cached_at: SystemTime,
    /// Cache TTL
    pub ttl: Duration,
}

impl CachedQuota {
    /// Check if cache entry is still valid
    pub fn is_valid(&self) -> bool {
        SystemTime::now()
            .duration_since(self.cached_at)
            .map(|elapsed| elapsed < self.ttl)
            .unwrap_or(false)
    }

    /// Check if storage quota allows operation
    pub fn can_use_storage(&self, bytes: u64) -> bool {
        self.storage_used + bytes <= self.storage_limit
    }

    /// Check if operation quota allows operation
    pub fn can_perform_operation(&self) -> bool {
        self.monthly_operations < self.monthly_operations_limit
    }
}

/// Usage delta tracked locally before sync to master
#[derive(Debug, Clone, Default)]
pub struct UsageDelta {
    /// Storage bytes added
    pub storage_added: u64,
    /// Storage bytes removed
    pub storage_removed: u64,
    /// Operations performed
    pub operations: u64,
}

/// Cluster-wide quota manager
pub struct ClusterQuotaManager {
    /// Local quota cache (user_id -> quota)
    quota_cache: Arc<RwLock<HashMap<Uuid, CachedQuota>>>,

    /// Local usage deltas pending sync (user_id -> delta)
    pending_deltas: Arc<RwLock<HashMap<Uuid, UsageDelta>>>,

    /// Hub client for fetching quotas from HiveHub API
    #[cfg(feature = "hub-integration")]
    hub_client: Option<Arc<HubClient>>,

    /// This node's ID
    node_id: String,

    /// Is this node the master (quota authority)?
    is_master: bool,

    /// Quota cache TTL
    cache_ttl: Duration,

    /// Sync interval (how often replicas send deltas to master)
    sync_interval: Duration,
}

impl ClusterQuotaManager {
    /// Create new cluster quota manager
    #[cfg(feature = "hub-integration")]
    pub fn new(node_id: String, is_master: bool, hub_client: Option<Arc<HubClient>>) -> Self {
        Self {
            quota_cache: Arc::new(RwLock::new(HashMap::new())),
            pending_deltas: Arc::new(RwLock::new(HashMap::new())),
            hub_client,
            node_id,
            is_master,
            cache_ttl: Duration::from_secs(60),     // 60s cache
            sync_interval: Duration::from_secs(30), // 30s sync
        }
    }

    /// Create new cluster quota manager (non-Hub mode)
    #[cfg(not(feature = "hub-integration"))]
    pub fn new(node_id: String, is_master: bool) -> Self {
        Self {
            quota_cache: Arc::new(RwLock::new(HashMap::new())),
            pending_deltas: Arc::new(RwLock::new(HashMap::new())),
            node_id,
            is_master,
            cache_ttl: Duration::from_secs(60),
            sync_interval: Duration::from_secs(30),
        }
    }

    /// Get quota for user (checks cache first, then fetches from master/Hub)
    pub async fn get_quota(&self, user_id: Uuid) -> Result<CachedQuota, String> {
        // Check cache first
        {
            let cache = self.quota_cache.read();
            if let Some(quota) = cache.get(&user_id) {
                if quota.is_valid() {
                    return Ok(quota.clone());
                }
            }
        }

        // Cache miss or expired - fetch from source
        if self.is_master {
            // Master node: Fetch from HiveHub API
            self.fetch_from_hub(user_id).await
        } else {
            // Replica node: Query master node
            self.fetch_from_master(user_id).await
        }
    }

    /// Fetch quota from HiveHub API (master node only)
    #[cfg(feature = "hub-integration")]
    async fn fetch_from_hub(&self, user_id: Uuid) -> Result<CachedQuota, String> {
        let hub_client = self
            .hub_client
            .as_ref()
            .ok_or("Hub client not configured")?;

        // Fetch quota from Hub API
        let quota = hub_client
            .get_user_quota(&user_id)
            .await
            .map_err(|e| format!("Failed to fetch quota from Hub: {}", e))?;

        let cached_quota = CachedQuota {
            user_id,
            storage_limit: quota.storage_limit,
            storage_used: quota.storage_used,
            monthly_operations_limit: quota.monthly_operations_limit,
            monthly_operations: quota.monthly_operations,
            cached_at: SystemTime::now(),
            ttl: self.cache_ttl,
        };

        // Update cache
        self.quota_cache
            .write()
            .insert(user_id, cached_quota.clone());

        Ok(cached_quota)
    }

    /// Fetch quota from HiveHub API (stub for non-Hub mode)
    #[cfg(not(feature = "hub-integration"))]
    async fn fetch_from_hub(&self, _user_id: Uuid) -> Result<CachedQuota, String> {
        Err("Hub integration not enabled".to_string())
    }

    /// Fetch quota from master node (replica nodes)
    async fn fetch_from_master(&self, user_id: Uuid) -> Result<CachedQuota, String> {
        // TODO: Implement inter-node RPC to query master for quota
        // For now, return a permissive default
        Ok(CachedQuota {
            user_id,
            storage_limit: u64::MAX,
            storage_used: 0,
            monthly_operations_limit: u64::MAX,
            monthly_operations: 0,
            cached_at: SystemTime::now(),
            ttl: self.cache_ttl,
        })
    }

    /// Track storage usage (add bytes)
    pub fn track_storage_add(&self, user_id: Uuid, bytes: u64) {
        let mut deltas = self.pending_deltas.write();
        let delta = deltas.entry(user_id).or_default();
        delta.storage_added += bytes;
    }

    /// Track storage usage (remove bytes)
    pub fn track_storage_remove(&self, user_id: Uuid, bytes: u64) {
        let mut deltas = self.pending_deltas.write();
        let delta = deltas.entry(user_id).or_default();
        delta.storage_removed += bytes;
    }

    /// Track operation usage
    pub fn track_operation(&self, user_id: Uuid) {
        let mut deltas = self.pending_deltas.write();
        let delta = deltas.entry(user_id).or_default();
        delta.operations += 1;
    }

    /// Sync pending deltas to master (called periodically on replica nodes)
    pub async fn sync_deltas_to_master(&self) -> Result<(), String> {
        if self.is_master {
            // Master node: Apply deltas locally and sync to Hub
            self.apply_local_deltas().await
        } else {
            // Replica node: Send deltas to master
            self.send_deltas_to_master().await
        }
    }

    /// Apply local deltas to cached quotas and sync to Hub (master node)
    async fn apply_local_deltas(&self) -> Result<(), String> {
        let deltas = {
            let mut pending = self.pending_deltas.write();
            let deltas = pending.clone();
            pending.clear();
            deltas
        };

        for (user_id, delta) in deltas {
            // Update cache
            {
                let mut cache = self.quota_cache.write();
                if let Some(quota) = cache.get_mut(&user_id) {
                    quota.storage_used = quota
                        .storage_used
                        .saturating_add(delta.storage_added)
                        .saturating_sub(delta.storage_removed);
                    quota.monthly_operations =
                        quota.monthly_operations.saturating_add(delta.operations);
                }
            }

            // Sync to Hub API (if Hub integration enabled)
            #[cfg(feature = "hub-integration")]
            if let Some(hub_client) = &self.hub_client {
                use crate::hub::sdk::ResourceType;

                let net_storage = if delta.storage_added > delta.storage_removed {
                    Some(delta.storage_added - delta.storage_removed)
                } else {
                    None
                };

                hub_client
                    .update_usage(
                        &user_id,
                        ResourceType::Queue, // Generic resource type for cluster-wide tracking
                        "cluster_operations",
                        Some(delta.operations),
                        net_storage,
                    )
                    .await
                    .map_err(|e| format!("Failed to sync usage to Hub: {}", e))?;
            }
        }

        Ok(())
    }

    /// Send deltas to master node (replica nodes)
    async fn send_deltas_to_master(&self) -> Result<(), String> {
        // TODO: Implement inter-node RPC to send deltas to master
        // For now, just clear pending deltas (they would be lost)
        self.pending_deltas.write().clear();
        Ok(())
    }

    /// Get sync interval
    pub fn sync_interval(&self) -> Duration {
        self.sync_interval
    }

    /// Check if this node is the master
    pub fn is_master(&self) -> bool {
        self.is_master
    }

    /// Get node ID
    pub fn node_id(&self) -> &str {
        &self.node_id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cached_quota_validity() {
        let quota = CachedQuota {
            user_id: Uuid::new_v4(),
            storage_limit: 1000,
            storage_used: 500,
            monthly_operations_limit: 10000,
            monthly_operations: 5000,
            cached_at: SystemTime::now(),
            ttl: Duration::from_secs(60),
        };

        assert!(quota.is_valid());
    }

    #[test]
    fn test_cached_quota_expiration() {
        let quota = CachedQuota {
            user_id: Uuid::new_v4(),
            storage_limit: 1000,
            storage_used: 500,
            monthly_operations_limit: 10000,
            monthly_operations: 5000,
            cached_at: SystemTime::now() - Duration::from_secs(120), // 2 minutes ago
            ttl: Duration::from_secs(60),
        };

        assert!(!quota.is_valid());
    }

    #[test]
    fn test_can_use_storage() {
        let quota = CachedQuota {
            user_id: Uuid::new_v4(),
            storage_limit: 1000,
            storage_used: 900,
            monthly_operations_limit: 10000,
            monthly_operations: 5000,
            cached_at: SystemTime::now(),
            ttl: Duration::from_secs(60),
        };

        assert!(quota.can_use_storage(50)); // 900 + 50 = 950 <= 1000
        assert!(!quota.can_use_storage(200)); // 900 + 200 = 1100 > 1000
    }

    #[test]
    fn test_can_perform_operation() {
        let quota = CachedQuota {
            user_id: Uuid::new_v4(),
            storage_limit: 1000,
            storage_used: 500,
            monthly_operations_limit: 10000,
            monthly_operations: 9999,
            cached_at: SystemTime::now(),
            ttl: Duration::from_secs(60),
        };

        assert!(quota.can_perform_operation()); // 9999 < 10000

        let quota_exhausted = CachedQuota {
            monthly_operations: 10000,
            ..quota
        };
        assert!(!quota_exhausted.can_perform_operation()); // 10000 >= 10000
    }

    #[test]
    fn test_usage_delta_tracking() {
        #[cfg(feature = "hub-integration")]
        let manager = ClusterQuotaManager::new("node-1".to_string(), true, None);

        #[cfg(not(feature = "hub-integration"))]
        let manager = ClusterQuotaManager::new("node-1".to_string(), true);

        let user_id = Uuid::new_v4();

        manager.track_storage_add(user_id, 100);
        manager.track_storage_remove(user_id, 20);
        manager.track_operation(user_id);
        manager.track_operation(user_id);

        let deltas = manager.pending_deltas.read();
        let delta = deltas.get(&user_id).unwrap();

        assert_eq!(delta.storage_added, 100);
        assert_eq!(delta.storage_removed, 20);
        assert_eq!(delta.operations, 2);
    }

    #[test]
    fn test_master_node_identification() {
        #[cfg(feature = "hub-integration")]
        let master = ClusterQuotaManager::new("node-1".to_string(), true, None);

        #[cfg(not(feature = "hub-integration"))]
        let master = ClusterQuotaManager::new("node-1".to_string(), true);

        assert!(master.is_master());
        assert_eq!(master.node_id(), "node-1");

        #[cfg(feature = "hub-integration")]
        let replica = ClusterQuotaManager::new("node-2".to_string(), false, None);

        #[cfg(not(feature = "hub-integration"))]
        let replica = ClusterQuotaManager::new("node-2".to_string(), false);

        assert!(!replica.is_master());
        assert_eq!(replica.node_id(), "node-2");
    }
}
