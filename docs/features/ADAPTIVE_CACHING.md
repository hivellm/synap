# Adaptive Caching Strategies

## Overview

Synap implements adaptive caching with automatic strategy selection based on workload patterns.

**Strategies**: LRU, LFU, ARC  
**Adaptation**: Automatic based on hit rate  
**Status**: ✅ Production Ready

## Caching Strategies

### 1. LRU (Least Recently Used)

**Best for**: Temporal locality (recent items accessed again)

```rust
use synap_server::cache::{AdaptiveCache, CacheStrategy};

let cache = AdaptiveCache::new(1000, CacheStrategy::Lru);
```

**Characteristics**:
- ✅ O(1) get/put operations
- ✅ Simple implementation
- ✅ Low memory overhead
- ❌ Ignores frequency

**Use Cases**:
- Session storage
- Recent user data
- Temporal workloads

### 2. LFU (Least Frequently Used)

**Best for**: Frequency-based access (popular items)

```rust
let cache = AdaptiveCache::new(1000, CacheStrategy::Lfu);
```

**Characteristics**:
- ✅ O(1) get/put operations
- ✅ Favors hot data
- ✅ Prevents cache pollution
- ❌ Slow to adapt to changing patterns

**Use Cases**:
- Popular content
- Leaderboards
- Frequently accessed config

### 3. ARC (Adaptive Replacement Cache)

**Best for**: Mixed workloads (combines LRU + LFU)

```rust
let cache = AdaptiveCache::new(1000, CacheStrategy::Arc);
```

**Characteristics**:
- ✅ Balances recency and frequency
- ✅ Self-tuning (adaptive target)
- ✅ Ghost lists for learning
- ⚠️ Higher complexity

**Use Cases**:
- Unknown workload patterns
- Mixed access patterns
- Production environments

## Adaptive Selection

The cache automatically switches strategies based on hit rate:

```rust
let mut cache = AdaptiveCache::new(10000, CacheStrategy::Lru);

// Cache monitors hit rate every 10K operations
// Switches to best-performing strategy if >5% improvement

// Example adaptation:
// LRU hit rate: 65% → LFU hit rate: 72% → Switches to LFU
```

### Evaluation Window

- **Window Size**: 10,000 operations
- **Threshold**: 5% hit rate improvement
- **Strategies Tested**: All 3 (LRU, LFU, ARC)

## L1 vs L2 Cache

### L1 Cache (Memory)

- **Location**: RAM
- **Speed**: ~56ns per GET
- **Capacity**: Configurable (default 4GB)
- **Eviction**: LRU, LFU, or ARC
- **Use**: Hot data

### L2 Cache (Disk)

- **Location**: Disk (memory-mapped files)
- **Speed**: ~10-50µs per GET
- **Capacity**: Configurable (default 1GB)
- **Eviction**: LFU only
- **Use**: Overflow from L1

### Automatic Promotion

```
Request → L1 Miss → L2 Hit → Promote to L1
Request → L1 Miss → L2 Miss → Fetch from source → Insert to L1
```

## Configuration

```yaml
# config.yml
cache:
  l1:
    strategy: "arc"  # lru, lfu, arc
    capacity_mb: 4096
    evaluation_window: 10000
  
  l2:
    enabled: true
    directory: "./data/cache/l2"
    max_size_mb: 1024
    max_entries: 100000
```

## Statistics

```rust
// Get cache stats
let stats = cache.get_stats();
println!("Hit rate: {:.2}%", stats.hit_rate * 100.0);
println!("Hits: {}, Misses: {}", stats.hits, stats.misses);
println!("Evictions: {}", stats.evictions);

// Get all strategy stats
let all_stats = cache.get_all_stats();
for (strategy, stats) in all_stats {
    println!("{:?}: {:.2}% hit rate", strategy, stats.hit_rate * 100.0);
}
```

## Performance

| Operation | L1 Cache | L2 Cache |
|-----------|----------|----------|
| **GET** | ~56ns | ~10-50µs |
| **SET** | ~100ns | ~1-5ms |
| **Eviction** | ~500ns | ~1-10ms |

## Monitoring

Prometheus metrics:
- `synap_cache_hits_total{strategy, level}`
- `synap_cache_misses_total{strategy, level}`
- `synap_cache_evictions_total{strategy, level}`
- `synap_cache_size_bytes{level}`

---

**Status**: ✅ Production Ready  
**Last Updated**: October 22, 2025  
**Tests**: 7 passing (LRU, LFU, ARC, L2)


