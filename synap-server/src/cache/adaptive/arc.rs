//! ARC (Adaptive Replacement Cache) Implementation
//!
//! Combines LRU and LFU by maintaining two lists:
//! - T1: Recent items (LRU)
//! - T2: Frequent items (LRU of frequently accessed)
//!
//! Dynamically balances between recency and frequency

use std::collections::{HashMap, VecDeque};
use std::hash::Hash;

/// ARC Cache - adaptive between LRU and LFU
pub struct ArcCache<K: Hash + Eq + Clone, V: Clone> {
    capacity: usize,
    target_t1: usize, // Dynamic target size for T1
    
    // T1: Recently accessed items (LRU)
    t1: HashMap<K, V>,
    t1_order: VecDeque<K>,
    
    // T2: Frequently accessed items (LRU of frequent)
    t2: HashMap<K, V>,
    t2_order: VecDeque<K>,
    
    // Ghost lists (metadata only, no values)
    b1: VecDeque<K>, // Recently evicted from T1
    b2: VecDeque<K>, // Recently evicted from T2
}

impl<K: Hash + Eq + Clone, V: Clone> ArcCache<K, V> {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            target_t1: capacity / 2, // Start with 50/50 split
            t1: HashMap::new(),
            t1_order: VecDeque::new(),
            t2: HashMap::new(),
            t2_order: VecDeque::new(),
            b1: VecDeque::new(),
            b2: VecDeque::new(),
        }
    }

    pub fn get(&mut self, key: &K) -> Option<V> {
        // Check T1 (recent)
        if let Some(value) = self.t1.remove(key) {
            // Move to T2 (now frequent)
            self.t1_order.retain(|k| k != key);
            self.t2.insert(key.clone(), value.clone());
            self.t2_order.push_back(key.clone());
            return Some(value);
        }

        // Check T2 (frequent)
        if let Some(value) = self.t2.get(key) {
            // Update LRU order in T2
            self.t2_order.retain(|k| k != key);
            self.t2_order.push_back(key.clone());
            return Some(value.clone());
        }

        None
    }

    pub fn insert(&mut self, key: K, value: V) {
        // If key is in T1 or T2, update it
        if self.t1.contains_key(&key) || self.t2.contains_key(&key) {
            self.get(&key); // Trigger promotion if in T1
            if self.t2.contains_key(&key) {
                self.t2.insert(key, value);
            } else {
                self.t1.insert(key, value);
            }
            return;
        }

        // Check ghost lists for adaptation
        if self.b1.contains(&key) {
            // Increase T1 target (recency is important)
            self.target_t1 = (self.target_t1 + 1).min(self.capacity);
            self.b1.retain(|k| k != &key);
        } else if self.b2.contains(&key) {
            // Decrease T1 target (frequency is important)
            self.target_t1 = self.target_t1.saturating_sub(1);
            self.b2.retain(|k| k != &key);
        }

        // Evict if necessary
        let total_size = self.t1.len() + self.t2.len();
        if total_size >= self.capacity {
            self.evict();
        }

        // Insert into T1 (recent)
        self.t1.insert(key.clone(), value);
        self.t1_order.push_back(key);
    }

    fn evict(&mut self) {
        if self.t1.len() > self.target_t1 {
            // Evict from T1
            if let Some(evict_key) = self.t1_order.pop_front() {
                self.t1.remove(&evict_key);
                
                // Add to B1 ghost list
                self.b1.push_back(evict_key);
                if self.b1.len() > self.capacity {
                    self.b1.pop_front();
                }
            }
        } else {
            // Evict from T2
            if let Some(evict_key) = self.t2_order.pop_front() {
                self.t2.remove(&evict_key);
                
                // Add to B2 ghost list
                self.b2.push_back(evict_key);
                if self.b2.len() > self.capacity {
                    self.b2.pop_front();
                }
            }
        }
    }

    pub fn len(&self) -> usize {
        self.t1.len() + self.t2.len()
    }

    pub fn is_empty(&self) -> bool {
        self.t1.is_empty() && self.t2.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_arc_basic() {
        let mut cache = ArcCache::new(3);
        
        cache.insert("a", 1);
        cache.insert("b", 2);
        cache.insert("c", 3);
        
        assert_eq!(cache.get(&"a"), Some(1));
        assert_eq!(cache.get(&"b"), Some(2));
        assert_eq!(cache.len(), 3);
        
        // Insert d, cache is at capacity, one will be evicted
        cache.insert("d", 4);
        
        assert_eq!(cache.len(), 3);
        assert_eq!(cache.get(&"d"), Some(4));
        
        // At least 2 of the original 3 should still be present
        let present_count = [
            cache.get(&"a").is_some(),
            cache.get(&"b").is_some(),
            cache.get(&"c").is_some(),
        ].iter().filter(|&&x| x).count();
        
        assert!(present_count >= 2, "At least 2 items should remain");
    }

    #[test]
    fn test_arc_promotion() {
        let mut cache = ArcCache::new(4);
        
        cache.insert("a", 1);
        cache.insert("b", 2);
        
        // Access "a" multiple times to promote to T2
        assert_eq!(cache.get(&"a"), Some(1));
        assert_eq!(cache.get(&"a"), Some(1));
        
        // "a" should now be in T2 (frequent)
        cache.insert("c", 3);
        cache.insert("d", 4);
        cache.insert("e", 5);
        
        // "a" should still be present (in T2)
        assert_eq!(cache.get(&"a"), Some(1));
    }
}

