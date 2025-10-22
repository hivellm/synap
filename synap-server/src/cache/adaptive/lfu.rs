//! LFU (Least Frequently Used) Cache Implementation

use std::collections::HashMap;
use std::hash::Hash;

/// Entry in LFU cache with frequency counter
struct LfuEntry<V> {
    value: V,
    frequency: u64,
}

/// LFU Cache - evicts least frequently used items
pub struct LfuCache<K: Hash + Eq + Clone, V: Clone> {
    capacity: usize,
    cache: HashMap<K, LfuEntry<V>>,
}

impl<K: Hash + Eq + Clone, V: Clone> LfuCache<K, V> {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            cache: HashMap::new(),
        }
    }

    pub fn get(&mut self, key: &K) -> Option<V> {
        if let Some(entry) = self.cache.get_mut(key) {
            entry.frequency += 1;
            Some(entry.value.clone())
        } else {
            None
        }
    }

    pub fn insert(&mut self, key: K, value: V) {
        // If at capacity and key is new, evict LFU item
        if self.cache.len() >= self.capacity && !self.cache.contains_key(&key) {
            self.evict_lfu();
        }

        // Insert or update
        self.cache
            .entry(key)
            .and_modify(|e| {
                e.value = value.clone();
                e.frequency += 1;
            })
            .or_insert(LfuEntry {
                value,
                frequency: 1,
            });
    }

    fn evict_lfu(&mut self) {
        // Find key with minimum frequency
        let mut min_freq = u64::MAX;
        let mut evict_key = None;

        for (key, entry) in &self.cache {
            if entry.frequency < min_freq {
                min_freq = entry.frequency;
                evict_key = Some(key.clone());
            }
        }

        if let Some(key) = evict_key {
            self.cache.remove(&key);
        }
    }

    pub fn len(&self) -> usize {
        self.cache.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cache.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lfu_basic() {
        let mut cache = LfuCache::new(3);
        
        cache.insert("a", 1);
        cache.insert("b", 2);
        cache.insert("c", 3);
        
        assert_eq!(cache.get(&"a"), Some(1)); // freq=2
        assert_eq!(cache.get(&"a"), Some(1)); // freq=3
        assert_eq!(cache.get(&"b"), Some(2)); // freq=2
        
        // Insert d, should evict c (freq=1, least frequent)
        cache.insert("d", 4);
        
        assert_eq!(cache.get(&"c"), None);
        assert_eq!(cache.get(&"a"), Some(1)); // freq=4, most frequent
        assert_eq!(cache.get(&"d"), Some(4));
    }
}


