use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::sync::atomic::{AtomicI64, AtomicU32, AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// Key buffer used by the in-memory KV shard storage.
///
/// Backed by `compact_str::CompactString`, which inlines strings up to
/// 24 bytes (the typical Redis-style key) directly inside the
/// `HashMap` bucket — eliminating one heap allocation and one
/// indirection per stored entry. Long keys spill to the heap exactly
/// like `String`. `CompactString` implements `Borrow<str>`, so all
/// existing `&str` lookups remain zero-cost.
pub type KeyBuf = compact_str::CompactString;

/// Expiry specification for SET operations.
///
/// Converted to an absolute millisecond timestamp before storage, enabling
/// sub-second precision (Redis PX/PXAT compatibility).
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "type", content = "value")]
pub enum Expiry {
    /// Expire after N seconds (EX)
    Seconds(u64),
    /// Expire after N milliseconds (PX)
    Milliseconds(u64),
    /// Expire at absolute Unix timestamp in seconds (EXAT)
    UnixSeconds(u64),
    /// Expire at absolute Unix timestamp in milliseconds (PXAT)
    UnixMilliseconds(u64),
}

impl Expiry {
    /// Convert to an absolute Unix timestamp in milliseconds.
    pub fn to_unix_ms(self) -> u64 {
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        match self {
            Expiry::Seconds(s) => now_ms.saturating_add(s.saturating_mul(1_000)),
            Expiry::Milliseconds(ms) => now_ms.saturating_add(ms),
            Expiry::UnixSeconds(s) => s.saturating_mul(1_000),
            Expiry::UnixMilliseconds(ms) => ms,
        }
    }
}

/// Options for SET operations (NX / XX / GET / KEEPTTL).
#[derive(Debug, Clone, Default)]
pub struct SetOptions {
    /// Only set if key does NOT exist (NX)
    pub if_absent: bool,
    /// Only set if key DOES exist (XX)
    pub if_present: bool,
    /// Preserve the existing TTL of the key (KEEPTTL)
    pub keep_ttl: bool,
    /// Return the old value before overwriting (GET)
    pub return_old: bool,
}

/// Result returned by `KVStore::set_with_opts()`.
#[derive(Debug)]
pub struct SetResult {
    /// Whether the SET actually wrote a value (false = NX/XX condition not met)
    pub written: bool,
    /// Previous value, populated only when `SetOptions::return_old = true`
    pub old_value: Option<Vec<u8>>,
}

/// Stored value in the KV store with compact metadata.
///
/// `last_access` is `AtomicU32` so GET operations can update it under a
/// **read** lock instead of a write lock — enabling fully concurrent reads.
#[derive(Debug)]
pub enum StoredValue {
    /// Persistent value without TTL.
    ///
    /// The payload is an `Arc<[u8]>` so a read returns a shared handle (a refcount
    /// bump) instead of copying the whole value, and cloning a `StoredValue`
    /// (getset, cache fill, snapshot iteration) is cheap (audit M-018 read half).
    Persistent(Arc<[u8]>),

    /// Integer-encoded persistent value — the analogue of Redis's `object.c`
    /// int encoding (phase13 int-encoding-counters). `INCR`/`DECR` mutate
    /// `value` and re-render the inline decimal cache: **zero heap allocation
    /// per op**. `buf[..len]` always holds the decimal ASCII rendering, so
    /// `data()` can hand out a borrow like any other variant. 20 bytes fit
    /// `i64::MIN` (`-` + 19 digits).
    Int { value: i64, buf: [u8; 20], len: u8 },

    /// Expiring value with TTL and LRU tracking.
    ///
    /// `expires_at` is a Unix timestamp in **milliseconds** (u64) to support
    /// millisecond-precision expiry (Redis PX / PXAT compatibility).
    /// `last_access` is `AtomicU32` (seconds) so GET can write it without a
    /// write lock. `data` is an `Arc<[u8]>` shared buffer (see `Persistent`).
    Expiring {
        data: Arc<[u8]>,
        expires_at: u64,        // Unix timestamp in milliseconds
        last_access: AtomicU32, // Unix timestamp in seconds for LRU
        freq: AtomicU32,        // access-frequency counter for LFU eviction
    },
}

/// Render `n` as decimal ASCII into a 20-byte buffer, returning `(buf, len)`.
#[inline]
fn render_int(n: i64) -> ([u8; 20], u8) {
    let mut buf = [0u8; 20];
    let mut pos = buf.len();
    let neg = n < 0;
    let mut v = n.unsigned_abs();
    loop {
        pos -= 1;
        buf[pos] = b'0' + (v % 10) as u8;
        v /= 10;
        if v == 0 {
            break;
        }
    }
    if neg {
        pos -= 1;
        buf[pos] = b'-';
    }
    let len = (buf.len() - pos) as u8;
    // Shift the digits to the front so `data()` is simply `&buf[..len]`.
    buf.copy_within(pos.., 0);
    (buf, len)
}

impl Clone for StoredValue {
    fn clone(&self) -> Self {
        match self {
            Self::Persistent(data) => Self::Persistent(data.clone()),
            Self::Int { value, buf, len } => Self::Int {
                value: *value,
                buf: *buf,
                len: *len,
            },
            Self::Expiring {
                data,
                expires_at,
                last_access,
                freq,
            } => Self::Expiring {
                data: data.clone(),
                expires_at: *expires_at,
                last_access: AtomicU32::new(last_access.load(Ordering::Relaxed)),
                freq: AtomicU32::new(freq.load(Ordering::Relaxed)),
            },
        }
    }
}

impl StoredValue {
    /// Create a new stored value using a seconds-based TTL (backward-compatible).
    ///
    /// Accepts anything convertible into the shared buffer: a `Vec<u8>` (one
    /// alloc + copy) or an `Arc<[u8]>` (identity — zero copy, the phase13
    /// parse-bulk-into-arc write path).
    pub fn new(data: impl Into<Arc<[u8]>>, ttl_secs: Option<u64>) -> Self {
        match ttl_secs {
            None => Self::Persistent(data.into()),
            Some(secs) => Self::with_expiry(data, Expiry::Seconds(secs)),
        }
    }

    /// Create a new stored value using the rich `Expiry` enum.
    pub fn with_expiry(data: impl Into<Arc<[u8]>>, expiry: Expiry) -> Self {
        Self::Expiring {
            data: data.into(),
            expires_at: expiry.to_unix_ms(),
            last_access: AtomicU32::new(Self::current_timestamp_secs()),
            freq: AtomicU32::new(1),
        }
    }

    /// Create a new stored value that expires at a specific absolute millisecond timestamp.
    pub fn with_expires_at_ms(data: Vec<u8>, expires_at_ms: u64) -> Self {
        Self::Expiring {
            data: data.into(),
            expires_at: expires_at_ms,
            last_access: AtomicU32::new(Self::current_timestamp_secs()),
            freq: AtomicU32::new(1),
        }
    }

    /// Get current Unix timestamp in seconds (u32 — sufficient for LRU).
    #[inline]
    fn current_timestamp_secs() -> u32 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs() as u32)
            .unwrap_or(0)
    }

    /// Get current Unix timestamp in milliseconds.
    #[inline]
    fn current_timestamp_ms() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    }

    /// Check if the value has expired.
    #[inline]
    pub fn is_expired(&self) -> bool {
        match self {
            Self::Persistent(_) | Self::Int { .. } => false,
            Self::Expiring { expires_at, .. } => Self::current_timestamp_ms() >= *expires_at,
        }
    }

    /// Update access time for LRU and bump the LFU frequency counter (takes
    /// `&self` — the atomics allow interior mutability on the read/GET path).
    pub fn update_access(&self) {
        if let Self::Expiring {
            last_access, freq, ..
        } = self
        {
            last_access.store(Self::current_timestamp_secs(), Ordering::Relaxed);
            // Saturating increment; the LFU eviction arm evicts the lowest freq.
            let cur = freq.load(Ordering::Relaxed);
            freq.store(cur.saturating_add(1), Ordering::Relaxed);
        }
    }

    /// Access-frequency counter for LFU eviction (0 for persistent values, which
    /// carry no counter — like `last_access`, LFU is meaningful for volatile keys).
    pub fn freq(&self) -> u32 {
        match self {
            Self::Persistent(_) | Self::Int { .. } => 0,
            Self::Expiring { freq, .. } => freq.load(Ordering::Relaxed),
        }
    }

    /// Get remaining TTL in seconds, rounded down (for cache layer).
    pub fn ttl_remaining(&self) -> Option<u64> {
        match self {
            Self::Persistent(_) | Self::Int { .. } => None,
            Self::Expiring { expires_at, .. } => {
                let now_ms = Self::current_timestamp_ms();
                if *expires_at > now_ms {
                    Some((*expires_at - now_ms) / 1_000)
                } else {
                    Some(0)
                }
            }
        }
    }

    /// Get remaining TTL in seconds.
    pub fn remaining_ttl_secs(&self) -> Option<u64> {
        match self {
            Self::Persistent(_) | Self::Int { .. } => None,
            Self::Expiring { expires_at, .. } => {
                let now_ms = Self::current_timestamp_ms();
                if now_ms >= *expires_at {
                    Some(0)
                } else {
                    Some((*expires_at - now_ms) / 1_000)
                }
            }
        }
    }

    /// Get remaining TTL in milliseconds (new — for PX/PTTL support).
    pub fn remaining_ttl_ms(&self) -> Option<u64> {
        match self {
            Self::Persistent(_) | Self::Int { .. } => None,
            Self::Expiring { expires_at, .. } => {
                let now_ms = Self::current_timestamp_ms();
                if now_ms >= *expires_at {
                    Some(0)
                } else {
                    Some(*expires_at - now_ms)
                }
            }
        }
    }

    /// Get absolute expiry in milliseconds (for KEEPTTL on overwrite).
    pub fn expires_at_ms(&self) -> Option<u64> {
        match self {
            Self::Persistent(_) | Self::Int { .. } => None,
            Self::Expiring { expires_at, .. } => Some(*expires_at),
        }
    }

    /// Get reference to data regardless of variant.
    #[inline]
    pub fn data(&self) -> &[u8] {
        match self {
            Self::Persistent(data) => data,
            Self::Int { buf, len, .. } => &buf[..*len as usize],
            Self::Expiring { data, .. } => data,
        }
    }

    /// Cheap clone of the shared payload buffer — a refcount bump, not a byte
    /// copy. This is the zero-copy read handle threaded to the response boundary.
    #[inline]
    pub fn data_arc(&self) -> Arc<[u8]> {
        match self {
            Self::Persistent(data) => Arc::clone(data),
            // Int has no shared buffer; materialize one (~20B) for the read handle.
            Self::Int { buf, len, .. } => Arc::from(&buf[..*len as usize]),
            Self::Expiring { data, .. } => Arc::clone(data),
        }
    }

    /// Replace the payload with a new buffer (copy-on-write). The `Arc<[u8]>`
    /// payload is immutable, so the rare in-place mutators (APPEND / SETRANGE)
    /// build a fresh buffer and swap it in, preserving TTL/access for `Expiring`.
    #[inline]
    pub fn set_data(&mut self, new: Vec<u8>) {
        match self {
            Self::Persistent(data) => *data = new.into(),
            // Writing raw bytes over a counter degrades it to a plain value.
            Self::Int { .. } => *self = Self::Persistent(new.into()),
            Self::Expiring { data, .. } => *data = new.into(),
        }
    }

    /// Get last access timestamp in seconds (for LRU eviction).
    pub fn last_access(&self) -> u32 {
        match self {
            Self::Persistent(_) | Self::Int { .. } => 0,
            Self::Expiring { last_access, .. } => last_access.load(Ordering::Relaxed),
        }
    }

    // ── Integer encoding (phase13 int-encoding-counters) ─────────────────────

    /// Create an integer-encoded persistent value.
    pub fn new_int(n: i64) -> Self {
        let (buf, len) = render_int(n);
        Self::Int { value: n, buf, len }
    }

    /// Read the value as an `i64`: direct for the `Int` variant (no parse), a
    /// byte parse for the others. `None` when the payload is not an integer.
    #[inline]
    pub fn as_int(&self) -> Option<i64> {
        match self {
            Self::Int { value, .. } => Some(*value),
            other => std::str::from_utf8(other.data())
                .ok()?
                .trim()
                .parse::<i64>()
                .ok(),
        }
    }

    /// Store an integer result (INCR/DECR):
    /// - `Int` — mutate in place and re-render the inline cache (**zero alloc**);
    /// - `Persistent` — upgrade to the `Int` encoding (subsequent INCRs are free);
    /// - `Expiring` — keep the variant (preserves TTL/LRU) and swap the bytes.
    #[inline]
    pub fn set_int(&mut self, n: i64) {
        match self {
            Self::Int { value, buf, len } => {
                *value = n;
                let (b, l) = render_int(n);
                *buf = b;
                *len = l;
            }
            Self::Persistent(_) => *self = Self::new_int(n),
            Self::Expiring { data, .. } => {
                let (b, l) = render_int(n);
                *data = Arc::from(&b[..l as usize]);
            }
        }
    }
}

/// Eviction policy for memory management (Redis-compatible naming).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum EvictionPolicy {
    /// No eviction — return error when memory limit is reached (default, preserves v0.9.x behavior).
    #[default]
    #[serde(rename = "noeviction")]
    NoEviction,
    /// Evict any key using approximated LRU.
    #[serde(rename = "allkeys-lru")]
    AllKeysLru,
    /// Evict only keys with a TTL set, using approximated LRU.
    #[serde(rename = "volatile-lru")]
    VolatileLru,
    /// Evict any key at random.
    #[serde(rename = "allkeys-random")]
    AllKeysRandom,
    /// Evict only keys with a TTL set, chosen at random.
    #[serde(rename = "volatile-random")]
    VolatileRandom,
    /// Evict only keys with a TTL set, prioritising those expiring soonest.
    #[serde(rename = "volatile-ttl")]
    VolatileTtl,
    /// Evict any key using approximated LFU (least-frequently-used).
    #[serde(rename = "allkeys-lfu")]
    AllKeysLfu,
    /// Evict only keys with a TTL set, using approximated LFU.
    #[serde(rename = "volatile-lfu")]
    VolatileLfu,
}

/// Configuration for KV store
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KVConfig {
    /// Maximum memory in MB
    pub max_memory_mb: usize,
    /// Eviction policy when memory limit reached
    pub eviction_policy: EvictionPolicy,
    /// TTL cleanup interval in milliseconds
    pub ttl_cleanup_interval_ms: u64,
    /// Allow dangerous FLUSH commands (disabled by default like Redis)
    pub allow_flush_commands: bool,
    /// Maximum allowed value size in bytes. SET requests exceeding this limit
    /// are rejected before any allocation. None means no limit (default).
    pub max_value_size_bytes: Option<usize>,
    /// Number of random keys sampled per shard during eviction (Redis default: 5).
    pub eviction_sample_size: usize,
}

impl Default for KVConfig {
    fn default() -> Self {
        Self {
            max_memory_mb: 4096,
            eviction_policy: EvictionPolicy::NoEviction,
            ttl_cleanup_interval_ms: 100,
            allow_flush_commands: false,
            max_value_size_bytes: None,
            eviction_sample_size: 5,
        }
    }
}

/// Statistics for KV store (snapshot — returned by `KVStore::stats()`)
#[derive(Debug, Default, Clone, Serialize)]
pub struct KVStats {
    /// Total number of keys
    pub total_keys: i64,
    /// Estimated memory usage in bytes
    pub total_memory_bytes: i64,
    /// Number of GET operations
    pub gets: u64,
    /// Number of SET operations
    pub sets: u64,
    /// Number of DELETE operations
    pub dels: u64,
    /// Number of cache hits
    pub hits: u64,
    /// Number of cache misses
    pub misses: u64,
}

impl KVStats {
    /// Calculate hit rate
    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            0.0
        } else {
            self.hits as f64 / total as f64
        }
    }
}

/// Lock-free atomic stats for KV store.
///
/// Replaces `Arc<RwLock<KVStats>>` — every SET/GET/DEL now updates counters
/// with `fetch_add(Relaxed)` without acquiring any global lock.
///
/// `total_memory_bytes` and `total_keys` are `AtomicI64` (signed) so that
/// subtraction on overwrite/delete never wraps into a huge positive value;
/// they should never go negative in correct code, but signed saturating reads
/// make bugs visible instead of hiding them.
#[derive(Debug, Default)]
pub struct AtomicKVStats {
    pub total_keys: AtomicI64,
    /// `Arc` so it can be registered with the shared cross-datatype
    /// [`GlobalMemory`](crate::core::GlobalMemory) budget (audit M-018); derefs
    /// to `AtomicI64`, so all `fetch_add`/`load`/`store` call sites are unchanged.
    pub total_memory_bytes: Arc<AtomicI64>,
    pub gets: AtomicU64,
    pub sets: AtomicU64,
    pub dels: AtomicU64,
    pub hits: AtomicU64,
    pub misses: AtomicU64,
}

impl AtomicKVStats {
    pub fn snapshot(&self) -> KVStats {
        KVStats {
            total_keys: self.total_keys.load(Ordering::Relaxed),
            total_memory_bytes: self.total_memory_bytes.load(Ordering::Relaxed),
            gets: self.gets.load(Ordering::Relaxed),
            sets: self.sets.load(Ordering::Relaxed),
            dels: self.dels.load(Ordering::Relaxed),
            hits: self.hits.load(Ordering::Relaxed),
            misses: self.misses.load(Ordering::Relaxed),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stored_value_persistent() {
        let data = vec![1, 2, 3, 4, 5];
        let value = StoredValue::new(data.clone(), None);

        match &value {
            StoredValue::Persistent(d) => assert_eq!(&**d, data.as_slice()),
            _ => panic!("Expected Persistent variant"),
        }

        assert!(!value.is_expired());
        assert_eq!(value.ttl_remaining(), None);
        assert_eq!(value.data(), &data);
    }

    #[test]
    fn test_stored_value_expiring() {
        let data = vec![1, 2, 3];
        let value = StoredValue::new(data.clone(), Some(60)); // 60 seconds TTL

        match value {
            StoredValue::Expiring { .. } => {}
            _ => panic!("Expected Expiring variant"),
        }

        assert!(!value.is_expired());
        assert!(value.ttl_remaining().is_some());
        assert_eq!(value.data(), &data);
    }

    #[test]
    fn test_stored_value_expiration() {
        let data = vec![1, 2, 3];
        let value = StoredValue::Expiring {
            data: data.clone().into(),
            expires_at: 0, // Already expired
            last_access: AtomicU32::new(0),
            freq: AtomicU32::new(1),
        };

        assert!(value.is_expired());
        assert_eq!(value.ttl_remaining(), Some(0));
    }

    #[test]
    fn test_lfu_freq_counter() {
        // Volatile values start at freq 1 and bump on each access.
        let v = StoredValue::new(vec![1, 2, 3], Some(60));
        assert_eq!(v.freq(), 1);
        v.update_access();
        v.update_access();
        assert_eq!(v.freq(), 3);

        // Persistent values carry no counter.
        let p = StoredValue::new(vec![1], None);
        assert_eq!(p.freq(), 0);
        p.update_access(); // no-op
        assert_eq!(p.freq(), 0);
    }

    #[test]
    fn test_eviction_policy_lfu_serde() {
        let lfu: EvictionPolicy = serde_json::from_str("\"allkeys-lfu\"").unwrap();
        assert_eq!(lfu, EvictionPolicy::AllKeysLfu);
        let vlfu: EvictionPolicy = serde_json::from_str("\"volatile-lfu\"").unwrap();
        assert_eq!(vlfu, EvictionPolicy::VolatileLfu);
        assert_eq!(
            serde_json::to_string(&EvictionPolicy::AllKeysLfu).unwrap(),
            "\"allkeys-lfu\""
        );
    }

    #[test]
    fn test_stored_value_update_access() {
        let value = StoredValue::new(vec![1, 2, 3], Some(60));

        let before = match &value {
            StoredValue::Expiring { last_access, .. } => last_access.load(Ordering::Relaxed),
            _ => panic!("Expected Expiring variant"),
        };

        std::thread::sleep(std::time::Duration::from_millis(10));
        value.update_access();

        let after = match &value {
            StoredValue::Expiring { last_access, .. } => last_access.load(Ordering::Relaxed),
            _ => panic!("Expected Expiring variant"),
        };

        assert!(after >= before);
    }

    #[test]
    fn test_stored_value_set_data_and_arc() {
        let mut value = StoredValue::new(vec![1, 2, 3], None);

        // Copy-on-write replacement (Arc payload is immutable).
        let mut new = value.data().to_vec();
        new.push(4);
        value.set_data(new);
        assert_eq!(value.data(), &[1, 2, 3, 4]);

        // data_arc shares the buffer: cloning bumps the refcount, no copy.
        let a = value.data_arc();
        let b = value.data_arc();
        assert_eq!(&*a, &[1, 2, 3, 4]);
        assert_eq!(a.as_ptr(), b.as_ptr()); // same backing allocation
    }

    #[test]
    fn test_eviction_policy_default() {
        let policy = EvictionPolicy::default();
        assert_eq!(policy, EvictionPolicy::NoEviction);
    }

    #[test]
    fn test_eviction_policy_serialization() {
        // Each variant round-trips with its Redis-compatible name.
        let cases = [
            (EvictionPolicy::NoEviction, "\"noeviction\""),
            (EvictionPolicy::AllKeysLru, "\"allkeys-lru\""),
            (EvictionPolicy::VolatileLru, "\"volatile-lru\""),
            (EvictionPolicy::AllKeysRandom, "\"allkeys-random\""),
            (EvictionPolicy::VolatileRandom, "\"volatile-random\""),
            (EvictionPolicy::VolatileTtl, "\"volatile-ttl\""),
        ];
        for (policy, expected) in cases {
            let serialized = serde_json::to_string(&policy).unwrap();
            assert_eq!(serialized, expected, "serialize {policy:?}");
            let deserialized: EvictionPolicy = serde_json::from_str(&serialized).unwrap();
            assert_eq!(deserialized, policy, "round-trip {policy:?}");
        }
    }

    #[test]
    fn test_kv_config_default() {
        let config = KVConfig::default();
        assert_eq!(config.max_memory_mb, 4096);
        assert_eq!(config.eviction_policy, EvictionPolicy::NoEviction);
        assert_eq!(config.eviction_sample_size, 5);
        assert!(!config.allow_flush_commands);
    }

    #[test]
    fn test_kv_stats_hit_rate() {
        let mut stats = KVStats::default();
        assert_eq!(stats.hit_rate(), 0.0);

        stats.hits = 80;
        stats.misses = 20;
        assert_eq!(stats.hit_rate(), 0.8);

        stats.hits = 100;
        stats.misses = 0;
        assert_eq!(stats.hit_rate(), 1.0);
    }

    #[test]
    fn test_stored_value_last_access() {
        let persistent = StoredValue::new(vec![1], None);
        assert_eq!(persistent.last_access(), 0);

        let expiring = StoredValue::new(vec![1], Some(60));
        assert!(expiring.last_access() > 0);
    }

    #[test]
    fn test_stored_value_remaining_ttl() {
        let value = StoredValue::new(vec![1, 2, 3], Some(60));

        let remaining = value.remaining_ttl_secs();
        assert!(remaining.is_some());
        let ttl = remaining.unwrap();
        assert!(ttl > 0 && ttl <= 60);
    }
}
