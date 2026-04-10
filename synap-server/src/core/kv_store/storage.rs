use super::super::types::{KeyBuf, StoredValue};
use ahash::RandomState;
use parking_lot::{Mutex, RwLock};
use radix_trie::{Trie, TrieCommon};
use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap};
use tracing::debug;

pub(crate) const SHARD_COUNT: usize = 64;
const HASHMAP_THRESHOLD: usize = 10_000; // Switch to RadixTrie after 10K keys

/// Storage backend for a shard (adaptive: HashMap for small, RadixTrie for large)
/// Note: CompactString could reduce memory by 30% for short keys, but RadixTrie
/// doesn't implement TrieKey for CompactString. Using String for compatibility.
pub(crate) enum ShardStorage {
    /// HashMap for datasets < 10K keys (2-3x faster), using ahash for fast
    /// lookup and `KeyBuf` (CompactString) so short keys live inline in the
    /// bucket without an extra heap allocation.
    Small(HashMap<KeyBuf, StoredValue, RandomState>),
    /// RadixTrie for datasets >= 10K keys (memory efficient for large sets).
    /// The `radix_trie` crate does not implement `TrieKey` for
    /// `CompactString`, so keys here remain `String`.
    Large(Trie<String, StoredValue>),
}

impl ShardStorage {
    pub(crate) fn new() -> Self {
        // Start with HashMap for better small-dataset performance.
        Self::Small(HashMap::with_hasher(RandomState::new()))
    }

    #[allow(dead_code)]
    pub(crate) fn len(&self) -> usize {
        match self {
            Self::Small(map) => map.len(),
            Self::Large(trie) => trie.len(),
        }
    }

    pub(crate) fn get(&self, key: &str) -> Option<&StoredValue> {
        match self {
            Self::Small(map) => map.get(key),
            Self::Large(trie) => trie.get(key),
        }
    }

    pub(crate) fn get_mut(&mut self, key: &str) -> Option<&mut StoredValue> {
        match self {
            Self::Small(map) => map.get_mut(key),
            Self::Large(trie) => trie.get_mut(key),
        }
    }

    pub(crate) fn insert(&mut self, key: String, value: StoredValue) -> Option<StoredValue> {
        match self {
            Self::Small(map) => {
                // CompactString::from(String) inlines short keys (≤24 bytes)
                // and reuses the heap buffer for long keys.
                let result = map.insert(KeyBuf::from(key), value);
                // Check if we need to upgrade to RadixTrie
                if map.len() >= HASHMAP_THRESHOLD {
                    self.upgrade_to_trie();
                }
                result
            }
            Self::Large(trie) => trie.insert(key, value),
        }
    }

    pub(crate) fn remove(&mut self, key: &str) -> Option<StoredValue> {
        match self {
            Self::Small(map) => map.remove(key),
            Self::Large(trie) => trie.remove(key),
        }
    }

    pub(crate) fn iter(&self) -> Vec<(String, StoredValue)> {
        match self {
            Self::Small(map) => map
                .iter()
                .map(|(k, v)| (k.as_str().to_owned(), v.clone()))
                .collect(),
            Self::Large(trie) => trie.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
        }
    }

    pub(crate) fn keys(&self) -> Vec<String> {
        match self {
            Self::Small(map) => map.keys().map(|k| k.as_str().to_owned()).collect(),
            Self::Large(trie) => trie.keys().cloned().collect(),
        }
    }

    pub(crate) fn clear(&mut self) {
        match self {
            Self::Small(map) => map.clear(),
            Self::Large(trie) => *trie = Trie::new(),
        }
    }

    /// Get keys with a specific prefix (for SCAN command)
    pub(crate) fn get_prefix_keys(&self, prefix: &str) -> Vec<String> {
        match self {
            Self::Small(map) => {
                // HashMap doesn't have prefix search, so filter manually.
                map.keys()
                    .filter(|k| k.as_str().starts_with(prefix))
                    .map(|k| k.as_str().to_owned())
                    .collect()
            }
            Self::Large(trie) => {
                // Use RadixTrie's efficient prefix search
                trie.get_raw_descendant(prefix)
                    .map(|subtrie| subtrie.keys().cloned().collect())
                    .unwrap_or_default()
            }
        }
    }

    /// Upgrade from HashMap to RadixTrie when threshold is reached
    fn upgrade_to_trie(&mut self) {
        if let Self::Small(map) = self {
            debug!(
                "Upgrading shard from HashMap to RadixTrie (threshold {} reached)",
                HASHMAP_THRESHOLD
            );
            let mut trie = Trie::new();
            for (k, v) in map.drain() {
                // Materialise the inline KeyBuf into a heap-allocated
                // String once, to satisfy radix_trie's TrieKey bound.
                trie.insert(k.into_string(), v);
            }
            *self = Self::Large(trie);
        }
    }
}

/// Single shard of the KV store with adaptive storage.
///
/// `ttl_heap` is a per-shard min-heap of `(expires_at_ms, key)` pairs used
/// by [`KVStore::cleanup_expired`] to evict expiring keys in expiry order
/// instead of by random sampling. The heap is **append-only** at write
/// time and **filtered at pop time** — when the cleanup task pops a stale
/// entry (key removed or rewritten with a new TTL) it simply discards it
/// and continues. This avoids any per-write heap fix-up.
///
/// While the shard is in `ShardStorage::Small` mode, the heap is the
/// authoritative expiration index. After [`ShardStorage::upgrade_to_trie`]
/// flips the shard to `Large` mode, the heap entries become stale; the
/// next cleanup pass drains them once and the sampling path takes over.
pub(crate) struct KVShard {
    pub(crate) data: RwLock<ShardStorage>,
    pub(crate) ttl_heap: Mutex<BinaryHeap<Reverse<(u64, KeyBuf)>>>,
}

impl KVShard {
    pub(crate) fn new() -> Self {
        Self {
            data: RwLock::new(ShardStorage::new()),
            ttl_heap: Mutex::new(BinaryHeap::new()),
        }
    }

    /// Push `(expires_at, key)` onto the TTL heap if `value` is expiring.
    /// Cheap no-op for persistent values.
    #[inline]
    pub(crate) fn track_ttl(&self, value: &StoredValue, key: &str) {
        if let Some(expires_at) = value.expires_at_ms() {
            self.ttl_heap
                .lock()
                .push(Reverse((expires_at, KeyBuf::from(key))));
        }
    }
}
