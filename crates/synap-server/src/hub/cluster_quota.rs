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

use crate::hub::HubClient;
use crate::hub::sdk::ResourceType;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use uuid::Uuid;

/// Wire snapshot of a user's quota sent between cluster nodes (issue #231).
/// `Uuid` is carried as `u128` to avoid the uuid serde feature.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuotaSnapshot {
    pub user_id: u128,
    pub storage_limit: u64,
    pub storage_used: u64,
    pub monthly_operations_limit: u64,
    pub monthly_operations: u64,
}

/// Wire form of a usage delta reported by a follower to the master.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeltaEntry {
    pub user_id: u128,
    pub storage_added: u64,
    pub storage_removed: u64,
    pub operations: u64,
}

/// Inter-node quota RPC messages (length-prefixed bincode over TCP).
#[derive(Debug, Clone, Serialize, Deserialize)]
enum QuotaRpc {
    /// Follower → master: fetch the authoritative quota for a user.
    GetQuota { user_id: u128 },
    /// Follower → master: report accumulated usage deltas.
    ApplyDeltas { deltas: Vec<DeltaEntry> },
    /// Master → follower: the requested quota snapshot.
    QuotaResponse(QuotaSnapshot),
    /// Master → follower: deltas accepted.
    Ack,
    /// Master → follower: request failed.
    Error(String),
}

/// Write a length-prefixed (u32 BE) bincode frame.
async fn write_frame(stream: &mut TcpStream, msg: &QuotaRpc) -> Result<(), String> {
    let data = bincode::serde::encode_to_vec(msg, bincode::config::legacy())
        .map_err(|e| format!("encode: {e}"))?;
    stream
        .write_all(&(data.len() as u32).to_be_bytes())
        .await
        .map_err(|e| format!("write len: {e}"))?;
    stream
        .write_all(&data)
        .await
        .map_err(|e| format!("write data: {e}"))?;
    stream.flush().await.map_err(|e| format!("flush: {e}"))?;
    Ok(())
}

/// Read a length-prefixed (u32 BE) bincode frame.
async fn read_frame(stream: &mut TcpStream) -> Result<QuotaRpc, String> {
    let mut len_buf = [0u8; 4];
    stream
        .read_exact(&mut len_buf)
        .await
        .map_err(|e| format!("read len: {e}"))?;
    let len = u32::from_be_bytes(len_buf) as usize;
    let mut data = vec![0u8; len];
    stream
        .read_exact(&mut data)
        .await
        .map_err(|e| format!("read data: {e}"))?;
    bincode::serde::decode_from_slice(&data, bincode::config::legacy())
        .map(|(m, _)| m)
        .map_err(|e| format!("decode: {e}"))
}

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
    hub_client: Option<Arc<HubClient>>,

    /// This node's ID
    node_id: String,

    /// Is this node the master (quota authority)?
    is_master: bool,

    /// Quota cache TTL
    cache_ttl: Duration,

    /// Sync interval (how often replicas send deltas to master)
    sync_interval: Duration,

    /// Master node's quota-RPC address (followers only). When set, quota queries
    /// and delta sync go to this address over TCP (issue #231).
    master_addr: Option<SocketAddr>,
}

impl ClusterQuotaManager {
    /// Create new cluster quota manager
    pub fn new(node_id: String, is_master: bool, hub_client: Option<Arc<HubClient>>) -> Self {
        Self {
            quota_cache: Arc::new(RwLock::new(HashMap::new())),
            pending_deltas: Arc::new(RwLock::new(HashMap::new())),
            hub_client,
            node_id,
            is_master,
            cache_ttl: Duration::from_secs(60),     // 60s cache
            sync_interval: Duration::from_secs(30), // 30s sync
            master_addr: None,
        }
    }

    /// Point a follower at its master's quota-RPC address (issue #231).
    pub fn with_master_addr(mut self, addr: SocketAddr) -> Self {
        self.master_addr = Some(addr);
        self
    }

    /// Start the quota-RPC server (master only): accept follower connections and
    /// answer `GetQuota` / `ApplyDeltas` requests. Returns the bound address (so a
    /// caller passing port 0 learns the OS-assigned port).
    pub async fn start_rpc_server(
        self: Arc<Self>,
        listen_addr: SocketAddr,
    ) -> Result<SocketAddr, String> {
        let listener = TcpListener::bind(listen_addr)
            .await
            .map_err(|e| format!("bind quota rpc: {e}"))?;
        let bound = listener
            .local_addr()
            .map_err(|e| format!("local_addr: {e}"))?;
        tokio::spawn(async move {
            while let Ok((mut stream, _peer)) = listener.accept().await {
                let me = Arc::clone(&self);
                tokio::spawn(async move {
                    // One request/response per connection is sufficient here.
                    if let Ok(req) = read_frame(&mut stream).await {
                        let resp = me.handle_rpc(req).await;
                        let _ = write_frame(&mut stream, &resp).await;
                    }
                });
            }
        });
        Ok(bound)
    }

    /// Handle a single inter-node quota request (pure of I/O, unit-testable).
    async fn handle_rpc(&self, req: QuotaRpc) -> QuotaRpc {
        match req {
            QuotaRpc::GetQuota { user_id } => {
                match self.get_quota(Uuid::from_u128(user_id)).await {
                    Ok(q) => QuotaRpc::QuotaResponse(QuotaSnapshot {
                        user_id,
                        storage_limit: q.storage_limit,
                        storage_used: q.storage_used,
                        monthly_operations_limit: q.monthly_operations_limit,
                        monthly_operations: q.monthly_operations,
                    }),
                    Err(e) => QuotaRpc::Error(e),
                }
            }
            QuotaRpc::ApplyDeltas { deltas } => {
                // Aggregate follower deltas into this master's pending set; the
                // periodic apply_local_deltas() folds them into the cache + Hub.
                let mut pending = self.pending_deltas.write();
                for d in deltas {
                    let entry = pending.entry(Uuid::from_u128(d.user_id)).or_default();
                    entry.storage_added += d.storage_added;
                    entry.storage_removed += d.storage_removed;
                    entry.operations += d.operations;
                }
                QuotaRpc::Ack
            }
            other => QuotaRpc::Error(format!("unexpected request: {other:?}")),
        }
    }

    /// Open a connection to the master and exchange one request/response.
    async fn rpc_call(&self, addr: SocketAddr, req: QuotaRpc) -> Result<QuotaRpc, String> {
        let mut stream = TcpStream::connect(addr)
            .await
            .map_err(|e| format!("connect master {addr}: {e}"))?;
        write_frame(&mut stream, &req).await?;
        read_frame(&mut stream).await
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

    /// Fetch quota from the master node over the inter-node RPC (issue #231).
    /// Falls back to a permissive quota only when no master address is configured.
    async fn fetch_from_master(&self, user_id: Uuid) -> Result<CachedQuota, String> {
        let Some(addr) = self.master_addr else {
            return Ok(CachedQuota {
                user_id,
                storage_limit: u64::MAX,
                storage_used: 0,
                monthly_operations_limit: u64::MAX,
                monthly_operations: 0,
                cached_at: SystemTime::now(),
                ttl: self.cache_ttl,
            });
        };

        match self
            .rpc_call(
                addr,
                QuotaRpc::GetQuota {
                    user_id: user_id.as_u128(),
                },
            )
            .await?
        {
            QuotaRpc::QuotaResponse(s) => {
                let cached = CachedQuota {
                    user_id,
                    storage_limit: s.storage_limit,
                    storage_used: s.storage_used,
                    monthly_operations_limit: s.monthly_operations_limit,
                    monthly_operations: s.monthly_operations,
                    cached_at: SystemTime::now(),
                    ttl: self.cache_ttl,
                };
                self.quota_cache.write().insert(user_id, cached.clone());
                Ok(cached)
            }
            QuotaRpc::Error(e) => Err(e),
            other => Err(format!("unexpected quota response: {other:?}")),
        }
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

            // Sync to Hub API if client is configured
            if let Some(hub_client) = &self.hub_client {
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

    /// Send accumulated usage deltas to the master over the inter-node RPC
    /// (issue #231). Deltas are drained only after the master acknowledges them,
    /// so a failed send does not silently lose usage.
    async fn send_deltas_to_master(&self) -> Result<(), String> {
        let Some(addr) = self.master_addr else {
            // No master configured — nothing to send to; keep deltas pending.
            return Ok(());
        };

        let snapshot: Vec<DeltaEntry> = {
            let pending = self.pending_deltas.read();
            pending
                .iter()
                .map(|(uid, d)| DeltaEntry {
                    user_id: uid.as_u128(),
                    storage_added: d.storage_added,
                    storage_removed: d.storage_removed,
                    operations: d.operations,
                })
                .collect()
        };
        if snapshot.is_empty() {
            return Ok(());
        }

        match self
            .rpc_call(addr, QuotaRpc::ApplyDeltas { deltas: snapshot })
            .await?
        {
            QuotaRpc::Ack => {
                // Master accepted — safe to clear the pending deltas now.
                self.pending_deltas.write().clear();
                Ok(())
            }
            QuotaRpc::Error(e) => Err(e),
            other => Err(format!("unexpected delta response: {other:?}")),
        }
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
        let manager = ClusterQuotaManager::new("node-1".to_string(), true, None);

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
        let master = ClusterQuotaManager::new("node-1".to_string(), true, None);

        assert!(master.is_master());
        assert_eq!(master.node_id(), "node-1");

        let replica = ClusterQuotaManager::new("node-2".to_string(), false, None);

        assert!(!replica.is_master());
        assert_eq!(replica.node_id(), "node-2");
    }

    fn seed_quota(mgr: &ClusterQuotaManager, user_id: Uuid, limit: u64, used: u64) {
        mgr.quota_cache.write().insert(
            user_id,
            CachedQuota {
                user_id,
                storage_limit: limit,
                storage_used: used,
                monthly_operations_limit: 1_000_000,
                monthly_operations: 0,
                cached_at: SystemTime::now(),
                ttl: Duration::from_secs(300),
            },
        );
    }

    /// A follower fetches quota from the master and reports deltas over the RPC
    /// (issue #231): the follower sees the master's authoritative quota and the
    /// master accumulates the reported deltas.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_quota_rpc_query_and_delta_roundtrip() {
        let user = Uuid::from_u128(42);

        let master = Arc::new(ClusterQuotaManager::new("master".to_string(), true, None));
        seed_quota(&master, user, 5000, 1200);
        let addr = Arc::clone(&master)
            .start_rpc_server("127.0.0.1:0".parse().unwrap())
            .await
            .unwrap();

        let follower =
            ClusterQuotaManager::new("follower".to_string(), false, None).with_master_addr(addr);

        // Query: follower gets the master's authoritative quota.
        let q = follower.get_quota(user).await.unwrap();
        assert_eq!(q.storage_limit, 5000);
        assert_eq!(q.storage_used, 1200);

        // Report deltas: follower accumulates then syncs to the master.
        follower.track_storage_add(user, 300);
        follower.track_operation(user);
        follower.track_operation(user);
        follower.sync_deltas_to_master().await.unwrap();

        // Master received and aggregated the deltas; follower cleared its pending.
        let master_pending = master.pending_deltas.read();
        let d = master_pending.get(&user).expect("master got deltas");
        assert_eq!(d.storage_added, 300);
        assert_eq!(d.operations, 2);
        assert!(follower.pending_deltas.read().is_empty());
    }

    /// With no master address configured, a follower falls back to a permissive
    /// quota (does not error) and keeps its deltas pending.
    #[tokio::test]
    async fn test_quota_rpc_no_master_is_permissive() {
        let follower = ClusterQuotaManager::new("follower".to_string(), false, None);
        let user = Uuid::from_u128(7);
        let q = follower.get_quota(user).await.unwrap();
        assert_eq!(q.storage_limit, u64::MAX);
        follower.track_operation(user);
        follower.sync_deltas_to_master().await.unwrap();
        // Deltas stay pending (not lost) when there is no master to send to.
        assert_eq!(
            follower
                .pending_deltas
                .read()
                .get(&user)
                .unwrap()
                .operations,
            1
        );
    }
}
