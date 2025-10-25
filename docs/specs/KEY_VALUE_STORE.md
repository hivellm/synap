# Key-Value Store Specification

## Overview

The Synap Key-Value Store is an in-memory, high-performance storage system built on radix trie data structure for memory-efficient string key storage.

## Core Features

### Data Operations
- **SET**: Store key-value pair with optional TTL
- **GET**: Retrieve value by key
- **DEL**: Delete key-value pair
- **EXISTS**: Check if key exists
- **INCR/DECR**: Atomic increment/decrement for numeric values
- **EXPIRE**: Set or update TTL on existing key
- **TTL**: Get remaining time-to-live

### Batch Operations
- **MSET**: Set multiple key-value pairs atomically
- **MGET**: Get multiple values by keys
- **MDEL**: Delete multiple keys atomically

### Advanced Operations
- **SCAN**: Iterate keys with prefix matching
- **KEYS**: Get all keys matching pattern
- **RENAME**: Rename key atomically
- **COPY**: Copy value to new key

## Data Structure

### Radix Tree Storage

```rust
use radix_trie::Trie;
use std::sync::Arc;
use parking_lot::RwLock;

pub struct KVStore {
    data: Arc<RwLock<Trie<String, StoredValue>>>,
    stats: Arc<RwLock<KVStats>>,
    config: KVConfig,
}

pub struct StoredValue {
    pub data: Vec<u8>,
    pub ttl: Option<Instant>,
    pub metadata: HashMap<String, String>,
    pub created_at: Instant,
    pub accessed_at: Instant,
    pub access_count: AtomicU64,
}

pub struct KVStats {
    pub total_keys: usize,
    pub total_memory_bytes: usize,
    pub gets: AtomicU64,
    pub sets: AtomicU64,
    pub dels: AtomicU64,
    pub hits: AtomicU64,
    pub misses: AtomicU64,
}
```

### Key Characteristics

#### Memory Efficiency
Radix tree shares common prefixes among keys:
```
Keys:
  user:1:name
  user:1:email
  user:2:name
  user:2:email

Storage:
  user:
    ├─ 1:
    │  ├─ name
    │  └─ email
    └─ 2:
       ├─ name
       └─ email

Shared prefix "user:" stored once
```

#### Performance Characteristics
- **Lookup**: O(k) where k = key length
- **Insert**: O(k)
- **Delete**: O(k)
- **Scan**: O(n) where n = matching keys
- **Memory**: ~30% less than HashMap for typical key patterns

## Operations Specification

### SET Operation

**Command**: `kv.set`

**Parameters**:
- `key` (string, required): Key name
- `value` (any, required): Value to store (serialized)
- `ttl` (integer, optional): Time-to-live in seconds
- `nx` (boolean, optional): Only set if key doesn't exist
- `xx` (boolean, optional): Only set if key exists

**Returns**:
- `success` (boolean): Operation result
- `previous` (any, optional): Previous value if XX mode

**Example**:
```json
{
  "command": "kv.set",
  "payload": {
    "key": "user:1001",
    "value": {"name": "Alice", "age": 30},
    "ttl": 3600
  }
}
```

**Response**:
```json
{
  "success": true,
  "key": "user:1001"
}
```

### GET Operation

**Command**: `kv.get`

**Parameters**:
- `key` (string, required): Key to retrieve

**Returns**:
- `found` (boolean): Whether key exists
- `value` (any, optional): Stored value if found
- `ttl` (integer, optional): Remaining TTL in seconds

**Example**:
```json
{
  "command": "kv.get",
  "payload": {
    "key": "user:1001"
  }
}
```

**Response**:
```json
{
  "found": true,
  "value": {"name": "Alice", "age": 30},
  "ttl": 3542
}
```

### DEL Operation

**Command**: `kv.del`

**Parameters**:
- `keys` (array[string], required): Keys to delete

**Returns**:
- `deleted` (integer): Number of keys actually deleted

**Example**:
```json
{
  "command": "kv.del",
  "payload": {
    "keys": ["user:1001", "user:1002"]
  }
}
```

### INCR/DECR Operations

**Command**: `kv.incr` or `kv.decr`

**Parameters**:
- `key` (string, required): Key name
- `amount` (integer, optional): Increment amount (default: 1)

**Returns**:
- `value` (integer): New value after operation

**Behavior**:
- Creates key with initial value if doesn't exist
- Returns error if value is not numeric

**Example**:
```json
{
  "command": "kv.incr",
  "payload": {
    "key": "counter:views",
    "amount": 1
  }
}
```

**Response**:
```json
{
  "value": 42
}
```

### SCAN Operation

**Command**: `kv.scan`

**Parameters**:
- `prefix` (string, optional): Key prefix to match
- `cursor` (string, optional): Pagination cursor
- `count` (integer, optional): Max keys to return (default: 100)

**Returns**:
- `keys` (array[string]): Matching keys
- `cursor` (string, optional): Next cursor for pagination
- `has_more` (boolean): Whether more results available

**Example**:
```json
{
  "command": "kv.scan",
  "payload": {
    "prefix": "user:",
    "count": 50
  }
}
```

**Response**:
```json
{
  "keys": ["user:1", "user:2", ...],
  "cursor": "next-page-token",
  "has_more": true
}
```

## TTL Management

### TTL Behavior
- Set during initial SET operation or via EXPIRE command
- Automatic deletion when TTL expires
- Background task checks expiration every 100ms
- Lazy deletion on GET if expired

### TTL Implementation
```rust
impl KVStore {
    async fn cleanup_expired(&self) {
        let interval = Duration::from_millis(100);
        let mut timer = tokio::time::interval(interval);
        
        loop {
            timer.tick().await;
            
            let mut data = self.data.write();
            let now = Instant::now();
            
            // Collect expired keys
            let expired: Vec<_> = data.iter()
                .filter(|(_, v)| v.is_expired(now))
                .map(|(k, _)| k.clone())
                .collect();
            
            // Remove expired keys
            for key in expired {
                data.remove(&key);
            }
        }
    }
}
```

## Memory Management

### Eviction Policies

When memory limit reached:

1. **No Eviction**: Return error on SET
2. **LRU (Least Recently Used)**: Evict least accessed keys
3. **LFU (Least Frequently Used)**: Evict least frequently accessed
4. **TTL-based**: Evict keys with shortest TTL first

**Configuration**:
```yaml
kv_store:
  max_memory_mb: 4096
  eviction_policy: "lru"
  eviction_sample_size: 100
```

### Memory Estimation

```rust
struct MemoryTracker {
    total_bytes: AtomicUsize,
}

impl KVStore {
    fn estimate_entry_size(&self, key: &str, value: &[u8]) -> usize {
        key.len() + value.len() + size_of::<StoredValue>()
    }
    
    fn track_memory_add(&self, size: usize) {
        self.stats.write().total_memory_bytes += size;
    }
}
```

## Atomic Operations

### Compare-And-Set (CAS)

**Command**: `kv.cas`

**Behavior**: Set new value only if current value matches expected

```json
{
  "command": "kv.cas",
  "payload": {
    "key": "counter",
    "expect": 41,
    "value": 42
  }
}
```

**Response**:
```json
{
  "success": true,
  "previous": 41,
  "current": 42
}
```

### Increment with Bounds

**Command**: `kv.incr_bounded`

**Behavior**: Increment but stay within min/max bounds

```json
{
  "command": "kv.incr_bounded",
  "payload": {
    "key": "rate_limit:user:1",
    "amount": 1,
    "max": 100
  }
}
```

## Metadata Support

Each key can have associated metadata:

```json
{
  "key": "document:123",
  "value": "...",
  "metadata": {
    "content_type": "application/json",
    "owner": "user:1",
    "tags": "important,archived"
  }
}
```

**Use Cases**:
- Content-Type for binary values
- Owner/ACL information
- Custom application tags
- Indexing hints

## Performance Optimization

### Read Optimization
- RwLock allows concurrent reads
- Cached key lookup results
- Prefix compression in radix tree

### Write Optimization
- Batch operations reduce lock contention
- Async TTL cleanup doesn't block writes
- Memory-mapped replication log (planned)

### Memory Optimization
- Radix tree shares key prefixes
- Compression for large values (optional)
- Lazy deletion of expired keys

## Error Conditions

### Error Types
```rust
pub enum KVError {
    KeyNotFound(String),
    KeyExists(String),
    InvalidValue(String),
    MemoryLimitExceeded,
    TTLInvalid(String),
    CASFailed { expected: String, actual: String },
}
```

### Error Scenarios
| Scenario | Error | HTTP Code |
|----------|-------|-----------|
| Key not found on GET | KeyNotFound | 404 |
| SET NX key exists | KeyExists | 409 |
| Memory limit | MemoryLimitExceeded | 507 |
| Invalid TTL value | TTLInvalid | 400 |
| CAS mismatch | CASFailed | 409 |

## Statistics & Monitoring

### Exposed Metrics
```json
{
  "total_keys": 1000000,
  "total_memory_bytes": 536870912,
  "operations": {
    "gets": 5000000,
    "sets": 1000000,
    "dels": 50000,
    "hits": 4500000,
    "misses": 500000
  },
  "hit_rate": 0.90,
  "avg_ttl_seconds": 3600,
  "expired_keys_cleaned": 25000
}
```

### Performance Metrics
- Operations per second (by type)
- Average latency (p50, p95, p99)
- Memory growth rate
- Eviction rate

## Replication Integration

### Write Propagation
```
SET user:1 → Master
    │
    ├─→ Update local radix tree
    │
    └─→ Append to replication log
        {
          "op": "set",
          "key": "user:1",
          "value": "...",
          "ttl": 3600,
          "timestamp": 1234567890
        }
        │
        └─→ Stream to all replicas
```

### Read from Replica
- Replicas maintain identical radix tree
- TTL countdown synchronized
- Eventual consistency (< 10ms lag)

## Configuration

```yaml
kv_store:
  enabled: true
  max_keys: 10000000
  max_memory_mb: 4096
  eviction_policy: "lru"  # none, lru, lfu, ttl
  eviction_sample_size: 100
  ttl_cleanup_interval_ms: 100
  compression:
    enabled: false
    threshold_bytes: 1024
    algorithm: "lz4"
```

## Testing Requirements

### Unit Tests
- Radix tree operations
- TTL expiration
- Eviction policies
- Atomic operations (CAS, INCR)
- Memory tracking

### Integration Tests
- Concurrent reads/writes
- Replication synchronization
- Large key sets (1M+ keys)
- Memory limit behavior

### Benchmarks
- GET latency (target: < 0.5ms p95)
- SET latency (target: < 1ms p95)
- Throughput (target: 100K ops/sec)
- Memory efficiency vs HashMap

## Example Usage

### Simple Key-Value
```rust
// Rust SDK
let client = SynapClient::connect("http://localhost:15500")?;

client.kv_set("user:1", json!({"name": "Alice"}), Some(3600)).await?;
let value = client.kv_get("user:1").await?;
client.kv_del(&["user:1"]).await?;
```

### With TTL
```rust
// Set with 1 hour TTL
client.kv_set("session:abc", token, Some(3600)).await?;

// Check remaining TTL
let ttl = client.kv_ttl("session:abc").await?;
println!("Expires in {} seconds", ttl);
```

### Atomic Counter
```rust
// Increment view counter
let views = client.kv_incr("article:123:views", 1).await?;

// Rate limiting with bounded increment
let requests = client.kv_incr_bounded(
    "rate_limit:user:1",
    1,
    100  // max
).await?;

if requests > 100 {
    return Err("Rate limit exceeded");
}
```

### Batch Operations
```rust
// Set multiple keys
client.kv_mset(vec![
    ("user:1", value1),
    ("user:2", value2),
    ("user:3", value3),
]).await?;

// Get multiple keys
let values = client.kv_mget(&["user:1", "user:2", "user:3"]).await?;
```

## See Also

- [ARCHITECTURE.md](../ARCHITECTURE.md) - Overall system architecture
- [REPLICATION.md](REPLICATION.md) - Replication implementation
- [REST_API.md](../api/REST_API.md) - HTTP API reference
- [RUST.md](../sdks/RUST.md) - Rust SDK documentation

