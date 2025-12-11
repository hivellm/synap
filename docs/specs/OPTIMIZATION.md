# Optimization Guide

## Performance Optimization Strategies

### 1. Memory Optimization

#### Radix Tree Tuning

```rust
// Configure radix tree for optimal memory usage
pub struct KVConfig {
    pub initial_capacity: usize,        // Pre-allocate for known size
    pub shrink_threshold: f32,          // Shrink when utilization < 25%
    pub compression_threshold: usize,   // Compress values > 1KB
}
```

#### Value Compression

```yaml
kv_store:
  compression:
    enabled: true
    threshold_bytes: 1024
    algorithm: "lz4"  # Fast compression
```

Benefits:
- 50-70% size reduction for text
- ~2μs compression overhead
- Worth it for values > 1KB

#### TTL Cleanup

```rust
impl KVStore {
    async fn optimized_ttl_cleanup(&self) {
        // Batch cleanup instead of per-key
        let mut interval = tokio::time::interval(Duration::from_secs(1));
        
        loop {
            interval.tick().await;
            
            let expired = self.collect_expired_batch(1000);
            self.delete_batch(&expired).await;
        }
    }
}
```

### 2. Network Optimization

#### HTTP/2 Multiplexing

Enable HTTP/2 for connection reuse:

```yaml
protocol:
  http2: true
  http2_max_concurrent_streams: 100
```

Benefits:
- Single TCP connection for multiple requests
- Header compression
- Server push (future)

#### Connection Keep-Alive

```yaml
protocol:
  keep_alive: true
  keep_alive_timeout_secs: 60
  keep_alive_max_requests: 1000
```

#### Compression

```yaml
protocol:
  compression: true
  compression_level: 6      # Balance speed/ratio
  compression_threshold: 512  # Only compress if > 512 bytes
```

### 3. CPU Optimization

#### Lock Contention Reduction

```rust
// Use DashMap for lock-free concurrent access
use dashmap::DashMap;

pub struct OptimizedRouter {
    // Instead of RwLock<HashMap>
    topics: DashMap<String, TopicSubscribers>,
}

// No lock required for reads
let subscribers = self.topics.get(&topic);
```

#### Read-Write Lock Strategy

```rust
// Read-heavy: use RwLock
let data: Arc<RwLock<Trie>> = Arc::new(RwLock::new(Trie::new()));

// Many concurrent readers
let value1 = data.read().get(&key1);
let value2 = data.read().get(&key2);

// Single writer
data.write().insert(key, value);
```

#### Async Task Optimization

```rust
// Spawn blocking work to dedicated thread pool
tokio::task::spawn_blocking(|| {
    // CPU-intensive work
    compress_large_value(&data)
}).await?;

// Keep async tasks non-blocking
tokio::spawn(async move {
    // Async I/O work
    stream_to_replica(&log_entry).await
});
```

### 4. I/O Optimization

#### Buffered I/O

```rust
use tokio::io::{AsyncWriteExt, BufWriter};

let file = File::create("replication.log").await?;
let mut writer = BufWriter::with_capacity(64 * 1024, file);

// Buffered writes
writer.write_all(&entry).await?;
writer.flush().await?;  // Explicit flush
```

#### Batch Replication

```rust
impl Master {
    async fn batch_replicate(&self) {
        let mut batch = Vec::with_capacity(100);
        let mut interval = interval(Duration::from_millis(10));
        
        loop {
            interval.tick().await;
            
            // Collect entries
            while let Some(entry) = self.log.pop() {
                batch.push(entry);
                
                if batch.len() >= 100 {
                    break;
                }
            }
            
            if !batch.is_empty() {
                self.send_batch(&batch).await?;
                batch.clear();
            }
        }
    }
}
```

### 5. Latency Optimization

#### Pre-allocate Buffers

```rust
// Pre-allocate response buffer
let mut buffer = Vec::with_capacity(4096);

// Reuse buffer
buffer.clear();
serialize_response(&response, &mut buffer)?;
```

#### Avoid Allocations in Hot Path

```rust
// Bad: allocates on every call
fn process_key(key: &str) -> String {
    format!("prefix:{}", key)  // Allocates
}

// Good: reuse string
fn process_key(key: &str, buffer: &mut String) {
    buffer.clear();
    buffer.push_str("prefix:");
    buffer.push_str(key);
}
```

#### Inline Small Functions

```rust
#[inline]
fn is_expired(&self, now: Instant) -> bool {
    self.ttl.map_or(false, |ttl| ttl < now)
}
```

### 6. Throughput Optimization

#### Parallel Processing

```rust
use rayon::prelude::*;

// Process batch in parallel
let results: Vec<_> = keys.par_iter()
    .map(|key| self.lookup(key))
    .collect();
```

#### SIMD for Vector Operations

```rust
#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;

// Use SIMD for hash calculation
unsafe fn fast_hash(data: &[u8]) -> u64 {
    // SIMD hash implementation
}
```

## Client-Side Optimization

### Connection Pooling

```typescript
const client = new SynapClient({
  poolSize: 20,      // 20 persistent connections
  keepAlive: true,
  poolMaxIdleTime: 60000
});
```

### Request Batching

```typescript
// Batch multiple gets
const keys = ['user:1', 'user:2', 'user:3'];

const results = await client.batch(
  keys.map(key => ({
    command: 'kv.get',
    payload: { key }
  }))
);
```

### Caching Layer

```typescript
class CachedClient {
  private cache = new Map();
  private client: SynapClient;
  
  async get(key: string): Promise<any> {
    // Check local cache first
    if (this.cache.has(key)) {
      return this.cache.get(key);
    }
    
    // Fetch from Synap
    const result = await this.client.kv.get(key);
    
    if (result.found) {
      this.cache.set(key, result.value);
      
      // Invalidate after TTL
      if (result.ttl) {
        setTimeout(() => this.cache.delete(key), result.ttl * 1000);
      }
    }
    
    return result.value;
  }
}
```

## Server-Side Optimization

### Tokio Configuration

```toml
[profile.release]
opt-level = 3              # Maximum optimization
lto = "fat"                # Link-time optimization
codegen-units = 1          # Better optimization, slower compile
strip = true               # Strip symbols
panic = "abort"            # Smaller binary
```

### Memory Allocator

```rust
// Use jemalloc for better performance
#[global_allocator]
static GLOBAL: jemallocator::Jemalloc = jemallocator::Jemalloc;
```

Benefits:
- 10-20% faster allocations
- Better memory fragmentation handling
- Reduced memory overhead

### CPU Pinning

```bash
# Pin server to specific CPUs
taskset -c 0-7 ./synap-server
```

### Kernel Tuning

```bash
# Increase file descriptors
ulimit -n 1000000

# TCP tuning
sysctl -w net.core.somaxconn=65535
sysctl -w net.ipv4.tcp_max_syn_backlog=65535
sysctl -w net.ipv4.ip_local_port_range="1024 65535"
```

## Profiling Results

### CPU Hotspots (Expected)

```
Function                          CPU %
├─ radix_trie::get               15%
├─ serde_json::serialize         12%
├─ axum::routing                  8%
├─ tokio::runtime::schedule       7%
├─ replication::stream_log        6%
└─ other                         52%
```

### Memory Allocation (Expected)

```
Component                    Memory
├─ Radix Tree (10M keys)    2.0 GB
├─ Queue System (100K msgs) 100 MB
├─ Event Streams (1K rooms) 200 MB
├─ Connection State         500 MB
├─ Replication Log          200 MB
└─ Other                    100 MB
Total:                      3.1 GB
```

## Performance Checklist

### Development
- [ ] Use `--release` flag for builds
- [ ] Enable LTO in Cargo.toml
- [ ] Use jemalloc allocator
- [ ] Avoid allocations in hot paths
- [ ] Use buffered I/O
- [ ] Batch operations where possible

### Deployment
- [ ] Enable HTTP/2
- [ ] Configure connection pooling
- [ ] Set appropriate worker threads
- [ ] Enable compression for large payloads
- [ ] Use MessagePack for binary efficiency
- [ ] Deploy read replicas for scaling

### Monitoring
- [ ] Track p95/p99 latencies
- [ ] Monitor queue depths
- [ ] Watch replication lag
- [ ] Alert on performance degradation
- [ ] Regular load testing
- [ ] Profile production workloads

## Trade-offs

### Speed vs Memory

| Choice | Speed | Memory | When to Use |
|--------|-------|--------|-------------|
| No compression | Fast | High | Small values |
| LZ4 compression | Medium | Medium | Large values |
| Zstd compression | Slow | Low | Archive data |

### Consistency vs Latency

| Mode | Latency | Consistency | When to Use |
|------|---------|-------------|-------------|
| Read from master | +0ms | Strong | Critical reads |
| Read from replica | +0ms | Eventual | High-throughput |
| Wait for 1 replica ACK | +10ms | Strong | Important writes |

### Durability vs Throughput

| Mode | Throughput | Durability | When to Use |
|------|------------|------------|-------------|
| In-memory only | Highest | Lowest | Cache |
| + Replication | High | Medium | Production |
| + Disk persistence | Medium | High | Critical data |

## See Also

- [PERFORMANCE.md](PERFORMANCE.md) - Performance targets and benchmarks
- [CONFIGURATION.md](CONFIGURATION.md) - Configuration reference
- [DEPLOYMENT.md](DEPLOYMENT.md) - Deployment strategies

