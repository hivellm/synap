//! Adaptive Caching Strategies
//!
//! Implements multiple cache eviction strategies:
//! - LRU (Least Recently Used)
//! - LFU (Least Frequently Used)
//! - ARC (Adaptive Replacement Cache)
//!
//! The system can adapt between strategies based on workload patterns

use std::collections::{HashMap, VecDeque};
use std::hash::Hash;
use std::time::Instant;

pub mod lfu;
pub mod arc;

pub use lfu::LfuCache;
pub use arc::ArcCache;

/// Cache eviction strategy
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CacheStrategy {
    /// Least Recently Used
    Lru,
    /// Least Frequently Used
    Lfu,
    /// Adaptive Replacement Cache (combines LRU + LFU)
    Arc,
}

/// Cache statistics for adaptive selection
#[derive(Debug, Clone, Default)]
pub struct CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
    pub inserts: u64,
    pub hit_rate: f64,
}

impl CacheStats {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record_hit(&mut self) {
        self.hits += 1;
        self.update_hit_rate();
    }

    pub fn record_miss(&mut self) {
        self.misses += 1;
        self.update_hit_rate();
    }

    pub fn record_eviction(&mut self) {
        self.evictions += 1;
    }

    pub fn record_insert(&mut self) {
        self.inserts += 1;
    }

    fn update_hit_rate(&mut self) {
        let total = self.hits + self.misses;
        if total > 0 {
            self.hit_rate = self.hits as f64 / total as f64;
        }
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

/// Adaptive cache that switches between strategies
pub struct AdaptiveCache<K: Hash + Eq + Clone, V: Clone> {
    current_strategy: CacheStrategy,
    capacity: usize,
    
    // Strategy implementations
    lru_cache: HashMap<K, (V, Instant)>,
    lru_order: VecDeque<K>,
    lfu_cache: Option<LfuCache<K, V>>,
    arc_cache: Option<ArcCache<K, V>>,
    
    // Statistics for each strategy
    stats: HashMap<CacheStrategy, CacheStats>,
    
    // Adaptation parameters
    evaluation_window: u64,
    last_evaluation: Instant,
}

impl<K: Hash + Eq + Clone, V: Clone> AdaptiveCache<K, V> {
    /// Create new adaptive cache
    pub fn new(capacity: usize, initial_strategy: CacheStrategy) -> Self {
        let mut cache = Self {
            current_strategy: initial_strategy,
            capacity,
            lru_cache: HashMap::new(),
            lru_order: VecDeque::new(),
            lfu_cache: None,
            arc_cache: None,
            stats: HashMap::new(),
            evaluation_window: 10000, // Evaluate every 10K ops
            last_evaluation: Instant::now(),
        };
        
        // Initialize stats for all strategies
        cache.stats.insert(CacheStrategy::Lru, CacheStats::new());
        cache.stats.insert(CacheStrategy::Lfu, CacheStats::new());
        cache.stats.insert(CacheStrategy::Arc, CacheStats::new());
        
        // Initialize the selected strategy
        match initial_strategy {
            CacheStrategy::Lfu => {
                cache.lfu_cache = Some(LfuCache::new(capacity));
            }
            CacheStrategy::Arc => {
                cache.arc_cache = Some(ArcCache::new(capacity));
            }
            _ => {} // LRU is default
        }
        
        cache
    }

    /// Get value from cache
    pub fn get(&mut self, key: &K) -> Option<V> {
        let result = match self.current_strategy {
            CacheStrategy::Lru => self.get_lru(key),
            CacheStrategy::Lfu => self.lfu_cache.as_mut()?.get(key),
            CacheStrategy::Arc => self.arc_cache.as_mut()?.get(key),
        };

        // Update statistics
        if let Some(stats) = self.stats.get_mut(&self.current_strategy) {
            if result.is_some() {
                stats.record_hit();
            } else {
                stats.record_miss();
            }
        }

        // Check if adaptation is needed
        self.maybe_adapt();

        result
    }

    /// Insert value into cache
    pub fn insert(&mut self, key: K, value: V) {
        match self.current_strategy {
            CacheStrategy::Lru => self.insert_lru(key, value),
            CacheStrategy::Lfu => {
                if let Some(cache) = self.lfu_cache.as_mut() {
                    cache.insert(key, value);
                }
            }
            CacheStrategy::Arc => {
                if let Some(cache) = self.arc_cache.as_mut() {
                    cache.insert(key, value);
                }
            }
        }

        if let Some(stats) = self.stats.get_mut(&self.current_strategy) {
            stats.record_insert();
        }
    }

    /// Simple LRU implementation
    fn get_lru(&mut self, key: &K) -> Option<V> {
        if let Some((value, _)) = self.lru_cache.get(key) {
            let value = value.clone();
            
            // Move to end (most recently used)
            self.lru_order.retain(|k| k != key);
            self.lru_order.push_back(key.clone());
            
            Some(value)
        } else {
            None
        }
    }

    fn insert_lru(&mut self, key: K, value: V) {
        // Evict if at capacity
        if self.lru_cache.len() >= self.capacity && !self.lru_cache.contains_key(&key) {
            if let Some(evict_key) = self.lru_order.pop_front() {
                self.lru_cache.remove(&evict_key);
                if let Some(stats) = self.stats.get_mut(&CacheStrategy::Lru) {
                    stats.record_eviction();
                }
            }
        }

        // Insert or update
        self.lru_cache.insert(key.clone(), (value, Instant::now()));
        
        // Update LRU order
        self.lru_order.retain(|k| k != &key);
        self.lru_order.push_back(key);
    }

    /// Evaluate and potentially switch strategies
    fn maybe_adapt(&mut self) {
        // Check if evaluation window has passed
        let total_ops: u64 = self
            .stats
            .values()
            .map(|s| s.hits + s.misses)
            .sum();

        if total_ops < self.evaluation_window {
            return;
        }

        // Find best performing strategy
        let mut best_strategy = self.current_strategy;
        let mut best_hit_rate = 0.0;

        for (strategy, stats) in &self.stats {
            if stats.hit_rate > best_hit_rate {
                best_hit_rate = stats.hit_rate;
                best_strategy = *strategy;
            }
        }

        // Switch if significantly better (>5% improvement)
        if best_strategy != self.current_strategy {
            let current_rate = self.stats[&self.current_strategy].hit_rate;
            if best_hit_rate > current_rate + 0.05 {
                tracing::info!(
                    "Adaptive cache switching from {:?} to {:?} (hit rate: {:.2}% -> {:.2}%)",
                    self.current_strategy,
                    best_strategy,
                    current_rate * 100.0,
                    best_hit_rate * 100.0
                );
                self.switch_strategy(best_strategy);
            }
        }

        // Reset statistics for next evaluation window
        for stats in self.stats.values_mut() {
            stats.reset();
        }
    }

    /// Switch to a different caching strategy
    fn switch_strategy(&mut self, new_strategy: CacheStrategy) {
        // Migrate data to new strategy
        // For simplicity, we'll just clear and start fresh
        // In production, you'd want to migrate data
        self.current_strategy = new_strategy;
        
        match new_strategy {
            CacheStrategy::Lru => {
                self.lfu_cache = None;
                self.arc_cache = None;
            }
            CacheStrategy::Lfu => {
                let mut lfu = LfuCache::new(self.capacity);
                // Migrate from LRU
                for (key, (value, _)) in &self.lru_cache {
                    lfu.insert(key.clone(), value.clone());
                }
                self.lfu_cache = Some(lfu);
                self.lru_cache.clear();
                self.lru_order.clear();
            }
            CacheStrategy::Arc => {
                let mut arc = ArcCache::new(self.capacity);
                // Migrate from current cache
                for (key, (value, _)) in &self.lru_cache {
                    arc.insert(key.clone(), value.clone());
                }
                self.arc_cache = Some(arc);
                self.lru_cache.clear();
                self.lru_order.clear();
            }
        }
    }

    /// Get current statistics
    pub fn get_stats(&self) -> &CacheStats {
        &self.stats[&self.current_strategy]
    }

    /// Get all statistics
    pub fn get_all_stats(&self) -> &HashMap<CacheStrategy, CacheStats> {
        &self.stats
    }

    /// Get current strategy
    pub fn current_strategy(&self) -> CacheStrategy {
        self.current_strategy
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adaptive_cache_lru() {
        let mut cache = AdaptiveCache::new(3, CacheStrategy::Lru);
        
        cache.insert("a", 1);
        cache.insert("b", 2);
        cache.insert("c", 3);
        
        assert_eq!(cache.get(&"a"), Some(1));
        assert_eq!(cache.get(&"b"), Some(2));
        assert_eq!(cache.get(&"c"), Some(3));
        
        // Insert d, should evict a (least recently used)
        cache.insert("d", 4);
        assert_eq!(cache.get(&"a"), None);
        assert_eq!(cache.get(&"d"), Some(4));
    }

    #[test]
    fn test_cache_stats() {
        let mut cache = AdaptiveCache::new(10, CacheStrategy::Lru);
        
        cache.insert("key1", "value1");
        assert_eq!(cache.get(&"key1"), Some("value1")); // Hit
        assert_eq!(cache.get(&"key2"), None); // Miss
        
        let stats = cache.get_stats();
        assert_eq!(stats.hits, 1);
        assert_eq!(stats.misses, 1);
        assert_eq!(stats.hit_rate, 0.5);
    }
}

