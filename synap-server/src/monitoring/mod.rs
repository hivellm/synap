//! Enhanced Monitoring Module
//!
//! Redis-style INFO command implementation with sections:
//! - SERVER: Version, uptime, process info
//! - MEMORY: Memory usage, allocation stats
//! - STATS: Command statistics, hit rates
//! - REPLICATION: Replication status (if enabled)
//!
//! Additional monitoring commands:
//! - SLOWLOG: Slow query logging
//! - MEMORY USAGE: Per-key memory tracking
//! - CLIENT LIST: Active connection tracking

use crate::core::{HashStore, KVStore, ListStore, SetStore, SortedSetStore};
use std::sync::Arc;
use std::time::Instant;

mod client_list;
mod info;
mod memory_usage;
mod slowlog;

pub use client_list::{ClientInfo, ClientList, ClientListManager};
pub use info::{InfoSection, KeyspaceInfo, MemoryInfo, ReplicationInfo, ServerInfo, StatsInfo};
pub use memory_usage::MemoryUsage;
pub use slowlog::{SlowLog, SlowLogEntry, SlowLogManager};

/// Type alias for store references tuple (to reduce complexity)
pub type StoreRefs = (
    Arc<KVStore>,
    Arc<HashStore>,
    Arc<ListStore>,
    Arc<SetStore>,
    Arc<SortedSetStore>,
);

/// Monitoring manager for collecting and serving monitoring data
#[derive(Clone)]
pub struct MonitoringManager {
    kv_store: Arc<KVStore>,
    hash_store: Arc<HashStore>,
    list_store: Arc<ListStore>,
    set_store: Arc<SetStore>,
    sorted_set_store: Arc<SortedSetStore>,
    slow_log: Arc<SlowLogManager>,
    start_time: Instant,
}

impl MonitoringManager {
    /// Create a new MonitoringManager
    pub fn new(
        kv_store: Arc<KVStore>,
        hash_store: Arc<HashStore>,
        list_store: Arc<ListStore>,
        set_store: Arc<SetStore>,
        sorted_set_store: Arc<SortedSetStore>,
    ) -> Self {
        Self {
            kv_store,
            hash_store,
            list_store,
            set_store,
            sorted_set_store,
            slow_log: Arc::new(SlowLogManager::new()),
            start_time: Instant::now(),
        }
    }

    /// Get server uptime in seconds
    pub fn uptime_secs(&self) -> u64 {
        self.start_time.elapsed().as_secs()
    }

    /// Get slow log manager
    pub fn slow_log(&self) -> Arc<SlowLogManager> {
        self.slow_log.clone()
    }

    /// Get KV store reference
    pub fn kv_store(&self) -> Arc<KVStore> {
        self.kv_store.clone()
    }

    /// Get all store references
    pub fn stores(&self) -> StoreRefs {
        (
            self.kv_store.clone(),
            self.hash_store.clone(),
            self.list_store.clone(),
            self.set_store.clone(),
            self.sorted_set_store.clone(),
        )
    }
}
