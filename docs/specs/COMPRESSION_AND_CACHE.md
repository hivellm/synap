# Compression and Cache System

## Overview

Synap implements intelligent compression and caching strategies to optimize both network bandwidth and CPU usage, ensuring low-latency operations even under high load.

## Payload Compression

### Compression Algorithms

Synap supports multiple compression algorithms optimized for different scenarios:

#### 1. LZ4 (Default - Recommended)

**Best For**: Real-time applications requiring low latency

**Characteristics**:
- **Compression Speed**: ~500 MB/s
- **Decompression Speed**: ~2000 MB/s (extremely fast)
- **Compression Ratio**: ~2-3x
- **CPU Overhead**: Very low
- **Use Case**: Default for all real-time operations

**Why LZ4**:
- Sub-millisecond decompression for small payloads (<1MB)
- Minimal CPU overhead (< 5% CPU usage)
- Perfect balance between compression ratio and speed
- Used by: RocksDB, Kafka, Cassandra

#### 2. Zstandard (Zstd)

**Best For**: Storage and batch operations

**Characteristics**:
- **Compression Speed**: ~400 MB/s
- **Decompression Speed**: ~1000 MB/s
- **Compression Ratio**: ~3-5x
- **CPU Overhead**: Low to medium
- **Use Case**: Persistence layer, replication logs

**Why Zstd**:
- Better compression ratio than LZ4
- Adjustable compression levels (1-22)
- Fast decompression
- Dictionary support for similar data

#### 3. Snappy (Alternative)

**Best For**: Compatible with existing systems

**Characteristics**:
- **Compression Speed**: ~550 MB/s
- **Decompression Speed**: ~1800 MB/s
- **Compression Ratio**: ~2x
- **CPU Overhead**: Very low
- **Use Case**: Legacy system integration

### Compression Configuration

```yaml
compression:
  # Enable compression globally
  enabled: true
  
  # Default algorithm
  default_algorithm: "lz4"
  
  # Minimum payload size to compress (bytes)
  min_payload_size: 1024  # Don't compress < 1KB
  
  # Algorithm selection per operation type
  algorithms:
    kv_store: "lz4"        # Key-value: speed priority
    queue: "lz4"           # Queues: speed priority
    stream: "lz4"          # Streams: speed priority
    replication: "zstd"    # Replication: ratio priority
    persistence: "zstd"    # Persistence: ratio priority
  
  # Zstd-specific settings
  zstd:
    level: 3               # Compression level (1-22)
    dictionary_size: 0     # Dictionary size (0 = disabled)
  
  # Auto-detection based on content type
  auto_detect:
    enabled: true
    skip_already_compressed: true  # Skip .gz, .zip, .jpg, etc.
```

### Content-Type Aware Compression

Synap automatically detects content types and skips compression for already-compressed data:

```typescript
// Already compressed - skip compression
const types_skip = [
  'image/jpeg', 'image/png', 'image/webp',
  'video/mp4', 'video/webm',
  'application/gzip', 'application/zip',
  'application/pdf'
];

// High compression benefit - always compress
const types_compress = [
  'text/plain', 'text/html', 'text/csv',
  'application/json', 'application/xml',
  'application/javascript'
];
```

### Compression Performance

| Payload Size | Algorithm | Compression | Decompression | Ratio | Total Overhead |
|--------------|-----------|-------------|---------------|-------|----------------|
| 1 KB | LZ4 | 0.01ms | 0.005ms | 1.8x | 0.015ms |
| 10 KB | LZ4 | 0.05ms | 0.02ms | 2.2x | 0.07ms |
| 100 KB | LZ4 | 0.3ms | 0.15ms | 2.5x | 0.45ms |
| 1 MB | LZ4 | 2ms | 0.8ms | 2.8x | 2.8ms |
| 1 KB | Zstd | 0.02ms | 0.01ms | 2.5x | 0.03ms |
| 10 KB | Zstd | 0.1ms | 0.04ms | 3.5x | 0.14ms |
| 100 KB | Zstd | 0.8ms | 0.3ms | 4.2x | 1.1ms |

---

## Hot Data Cache System

### Overview

Synap implements a multi-tier caching system that keeps frequently accessed data decompressed in memory to eliminate CPU overhead on hot paths.

### Cache Architecture

```
┌─────────────────────────────────────────────────────────┐
│                    Client Request                        │
└─────────────────────────────────────────────────────────┘
                         │
                         ▼
┌─────────────────────────────────────────────────────────┐
│              L1 Cache (Hot Data)                         │
│  - Decompressed payloads                                 │
│  - Millisecond-level TTL                                 │
│  - LRU eviction                                          │
│  - Max size: 10% of total memory                         │
└─────────────────────────────────────────────────────────┘
                         │ (miss)
                         ▼
┌─────────────────────────────────────────────────────────┐
│              L2 Cache (Warm Data)                        │
│  - Compressed payloads                                   │
│  - Second-level TTL                                      │
│  - FIFO eviction                                         │
│  - Max size: 20% of total memory                         │
└─────────────────────────────────────────────────────────┘
                         │ (miss)
                         ▼
┌─────────────────────────────────────────────────────────┐
│              Primary Storage                             │
│  - Radix Tree (compressed)                               │
│  - Queue buffers (compressed)                            │
│  - Event streams (compressed)                            │
└─────────────────────────────────────────────────────────┘
```

### L1 Cache (Hot Data)

**Purpose**: Eliminate decompression overhead for frequently accessed data

**Characteristics**:
- **Storage**: Decompressed payloads ready to send
- **TTL**: 2-10 seconds (configurable)
- **Eviction**: LRU (Least Recently Used)
- **Size**: 10% of total memory (configurable)
- **Metrics**: Track access frequency

**Configuration**:
```yaml
cache:
  l1_hot_data:
    enabled: true
    max_size_mb: 512      # 512MB for hot data
    ttl_seconds: 5        # Keep for 5 seconds
    min_access_count: 3   # Promote after 3 accesses
    eviction_policy: "lru"
    
    # Adaptive TTL based on access pattern
    adaptive_ttl:
      enabled: true
      min_ttl: 2          # Minimum 2 seconds
      max_ttl: 10         # Maximum 10 seconds
      increment_factor: 1.5
```

### L2 Cache (Warm Data)

**Purpose**: Reduce primary storage lookups

**Characteristics**:
- **Storage**: Compressed payloads
- **TTL**: 30-60 seconds
- **Eviction**: FIFO or LRU
- **Size**: 20% of total memory
- **Fast**: Still in memory, faster than disk

**Configuration**:
```yaml
cache:
  l2_warm_data:
    enabled: true
    max_size_mb: 1024     # 1GB for warm data
    ttl_seconds: 30       # Keep for 30 seconds
    eviction_policy: "fifo"
```

### Cache Promotion Strategy

Data moves between cache levels based on access patterns:

```
Access Pattern → Cache Level
─────────────────────────────
< 2 accesses   → No cache (primary storage)
2-3 accesses   → L2 (compressed)
> 3 accesses   → L1 (decompressed)
No access 10s  → Evict from L1 → L2
No access 60s  → Evict from L2
```

### Adaptive Caching

Synap intelligently adjusts cache behavior based on workload:

#### Frequency Detection

```rust
struct AccessMetrics {
    access_count: u64,
    last_access: Instant,
    access_rate: f64,  // accesses per second
}

fn should_cache_decompressed(metrics: &AccessMetrics) -> bool {
    // Cache if access rate > 10/sec
    metrics.access_rate > 10.0 ||
    // Or if accessed 3+ times in last 5 seconds
    (metrics.access_count >= 3 && 
     metrics.last_access.elapsed() < Duration::from_secs(5))
}
```

#### Workload-Based Tuning

```yaml
cache:
  adaptive:
    enabled: true
    
    # Read-heavy workload (default)
    read_heavy:
      l1_size_percent: 15    # More hot cache
      l1_ttl_seconds: 10
      promotion_threshold: 2
    
    # Write-heavy workload
    write_heavy:
      l1_size_percent: 5     # Less hot cache
      l1_ttl_seconds: 2
      promotion_threshold: 5
    
    # Balanced workload
    balanced:
      l1_size_percent: 10
      l1_ttl_seconds: 5
      promotion_threshold: 3
```

---

## Performance Optimization

### Cache Hit Rate Targets

| Cache Level | Target Hit Rate | Typical Latency |
|-------------|----------------|-----------------|
| L1 (Hot) | > 80% | < 0.1ms |
| L2 (Warm) | > 90% | < 0.5ms |
| Storage (Cold) | 100% | < 1ms |

### CPU Overhead Reduction

**Without Caching**:
```
Request → Decompress (0.5ms) → Process → Compress (0.3ms) → Response
Total: ~0.8ms CPU overhead per request
```

**With L1 Cache**:
```
Request → L1 Hit (0.01ms) → Response
Total: ~0.01ms CPU overhead (80x faster)
```

### Memory vs CPU Trade-off

```yaml
# High performance mode (more memory, less CPU)
performance_mode: "high"
cache:
  l1_hot_data:
    max_size_mb: 2048    # 2GB hot cache
    ttl_seconds: 15
  compression:
    enabled: true
    min_payload_size: 4096  # Compress less

# Balanced mode (default)
performance_mode: "balanced"
cache:
  l1_hot_data:
    max_size_mb: 512     # 512MB hot cache
    ttl_seconds: 5
  compression:
    enabled: true
    min_payload_size: 1024

# Memory-efficient mode (less memory, more CPU)
performance_mode: "memory_efficient"
cache:
  l1_hot_data:
    max_size_mb: 128     # 128MB hot cache
    ttl_seconds: 2
  compression:
    enabled: true
    min_payload_size: 512   # Compress more
```

---

## Implementation Examples

### Client Compression Negotiation

```typescript
// Client specifies supported compression
const client = new SynapClient({
  url: 'http://localhost:15500',
  compression: {
    accept: ['lz4', 'zstd', 'none'],
    min_size: 1024
  }
});

// Server responds with compressed data
const response = await client.get('large-key');
// Automatically decompressed by SDK
```

### Cache Monitoring

```typescript
// Get cache statistics
const stats = await client.admin.getCacheStats();

console.log('L1 Cache:', {
  hitRate: stats.l1.hit_rate,
  size: stats.l1.size_mb,
  entries: stats.l1.entry_count,
  evictions: stats.l1.eviction_count
});

console.log('L2 Cache:', {
  hitRate: stats.l2.hit_rate,
  size: stats.l2.size_mb,
  entries: stats.l2.entry_count
});

console.log('Overall:', {
  totalHitRate: stats.overall.hit_rate,
  cpuSaved: stats.overall.cpu_time_saved_ms,
  compressionRatio: stats.compression.avg_ratio
});
```

---

## Monitoring and Metrics

### Prometheus Metrics

```
# Cache metrics
synap_cache_l1_hit_total
synap_cache_l1_miss_total
synap_cache_l1_size_bytes
synap_cache_l1_eviction_total
synap_cache_l2_hit_total
synap_cache_l2_miss_total

# Compression metrics
synap_compression_ratio_avg
synap_compression_time_seconds
synap_decompression_time_seconds
synap_compression_cpu_percent
synap_compression_bytes_saved_total

# Performance metrics
synap_request_decompression_skip_total  # L1 cache hits
synap_cpu_overhead_reduction_percent
```

### Cache Dashboard

Monitor cache efficiency:

```
┌─────────────────────────────────────────────────────────┐
│               Cache Performance Dashboard                │
├─────────────────────────────────────────────────────────┤
│ L1 Hot Cache                                             │
│   Hit Rate: ████████████████░░░░ 82%                     │
│   Size: 487 MB / 512 MB                                  │
│   Entries: 45,231                                        │
│   Avg Response: 0.08ms                                   │
├─────────────────────────────────────────────────────────┤
│ L2 Warm Cache                                            │
│   Hit Rate: ██████████████████░░ 91%                     │
│   Size: 923 MB / 1024 MB                                 │
│   Entries: 128,456                                       │
│   Avg Response: 0.42ms                                   │
├─────────────────────────────────────────────────────────┤
│ Compression                                              │
│   Algorithm: LZ4                                         │
│   Avg Ratio: 2.3x                                        │
│   CPU Overhead: 3.2%                                     │
│   Bytes Saved: 1.2 TB (lifetime)                         │
└─────────────────────────────────────────────────────────┘
```

---

## Best Practices

### 1. Choose Right Algorithm

```yaml
# Real-time applications
compression:
  default_algorithm: "lz4"

# Storage-heavy applications  
compression:
  default_algorithm: "zstd"
  zstd:
    level: 5  # Medium compression
```

### 2. Tune Cache Sizes

```bash
# Calculate based on workload
Total Memory: 16 GB
L1 Cache: 1.6 GB (10%)
L2 Cache: 3.2 GB (20%)
Primary Storage: 11.2 GB (70%)
```

### 3. Monitor Access Patterns

```typescript
// Track hot keys
const hotKeys = await client.admin.getHotKeys({
  limit: 100,
  min_access_rate: 10  // > 10 accesses/sec
});

// Optimize cache for hot keys
await client.admin.pinToCache(hotKeys);
```

### 4. Disable for Small Payloads

```yaml
compression:
  min_payload_size: 1024  # Don't compress < 1KB
  # Compression overhead > space savings
```

---

## See Also

- [Performance Guide](PERFORMANCE.md)
- [Architecture](ARCHITECTURE.md)
- [Configuration Reference](CONFIGURATION.md)
- [Optimization Strategies](OPTIMIZATION.md)

