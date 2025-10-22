//! Cache Module
//!
//! Provides multiple caching layers:
//! - L1: In-memory cache (LRU in KVStore)
//! - L2: Disk-backed cache for overflow
//! - Adaptive: Dynamic strategy selection (LRU/LFU/ARC)

pub mod adaptive;
pub mod l2_disk;

pub use adaptive::{AdaptiveCache, CacheStrategy, CacheStats};
pub use l2_disk::{L2CacheConfig, L2CacheStats, L2DiskCache};

