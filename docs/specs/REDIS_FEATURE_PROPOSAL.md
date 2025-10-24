# Redis Feature Implementation Proposal

> **Status**: Draft  
> **Created**: October 24, 2025  
> **Author**: AI Assistant  
> **Priority**: High  
> **Target Version**: v0.4.0 - v0.7.0

## Executive Summary

This proposal outlines a strategic roadmap to implement critical Redis-compatible features in Synap, transforming it into a comprehensive data platform that maintains its unique advantages (MCP/UMICP, Kafka-style streams, compression) while adding essential Redis data structures and operations.

**Goal**: Make Synap a viable Redis alternative for 80% of use cases while maintaining differentiation through modern protocols and AI integration.

---

## Motivation

### Current State

Synap currently supports:
- ✅ Key-Value Store (String operations)
- ✅ Message Queues (RabbitMQ-style)
- ✅ Event Streams (Kafka-style with partitions)
- ✅ Pub/Sub (topic-based)

### Gap Analysis

Based on comprehensive Redis comparison (`docs/REDIS_COMPARISON.md`), Synap lacks:
- ❌ **7 core data structures** (Hashes, Lists, Sets, Sorted Sets, Bitmaps, HyperLogLog, Geospatial)
- ❌ **100+ Redis commands** across structures
- ❌ **Transactions** (MULTI/EXEC/WATCH)
- ❌ **Lua scripting** for server-side logic
- ❌ **Cluster mode** for horizontal scaling

### Business Impact

**Why This Matters**:

1. **Market Adoption**: 90% of Redis users rely on Hashes, Lists, and Sets
2. **Migration Path**: Organizations can't migrate from Redis without these structures
3. **Feature Parity**: Developers expect Redis-like operations in modern data stores
4. **Competitive Position**: Synap needs these to compete with Redis, KeyDB, Dragonfly

**Target Users**:
- Teams migrating from Redis to modern infrastructure
- Developers building real-time applications
- AI/ML pipelines requiring both graph data and caching
- Microservices needing unified data layer

---

## Proposal Overview

### Four-Phase Implementation

```
Phase 1 (v0.4.0): Core Data Structures       → 3-6 months
Phase 2 (v0.5.0): Advanced Operations        → 6-9 months  
Phase 3 (v0.6.0): Transactions & Scripting   → 9-12 months
Phase 4 (v0.7.0): Cluster & Enterprise       → 12-18 months
```

### Success Metrics

- **Compatibility**: 80% Redis command coverage in target structures
- **Performance**: Within 2x of Redis latency benchmarks
- **Migration**: Zero-downtime migration tool from Redis
- **Adoption**: 1000+ downloads/month on crates.io
- **Community**: 100+ GitHub stars, 10+ contributors

---

## Phase 1: Core Data Structures (v0.4.0)

**Duration**: 3-6 months  
**Priority**: CRITICAL  
**Risk**: Medium

### 1.1 Hashes (HashMap within a key)

**Motivation**: Essential for structured objects (user profiles, product catalogs)

#### API Design

```rust
// Core operations
pub trait HashOps {
    async fn hset(&self, key: &str, field: &str, value: Value) -> Result<bool>;
    async fn hget(&self, key: &str, field: &str) -> Result<Option<Value>>;
    async fn hdel(&self, key: &str, fields: &[&str]) -> Result<usize>;
    async fn hgetall(&self, key: &str) -> Result<HashMap<String, Value>>;
    async fn hexists(&self, key: &str, field: &str) -> Result<bool>;
    async fn hkeys(&self, key: &str) -> Result<Vec<String>>;
    async fn hvals(&self, key: &str) -> Result<Vec<Value>>;
    async fn hlen(&self, key: &str) -> Result<usize>;
    
    // Atomic operations
    async fn hincrby(&self, key: &str, field: &str, increment: i64) -> Result<i64>;
    async fn hincrbyfloat(&self, key: &str, field: &str, increment: f64) -> Result<f64>;
    
    // Batch operations
    async fn hmset(&self, key: &str, fields: HashMap<String, Value>) -> Result<()>;
    async fn hmget(&self, key: &str, fields: &[&str]) -> Result<Vec<Option<Value>>>;
    
    // Conditional
    async fn hsetnx(&self, key: &str, field: &str, value: Value) -> Result<bool>;
    
    // Iteration
    async fn hscan(&self, key: &str, cursor: u64, pattern: Option<&str>, count: Option<usize>) -> Result<(u64, Vec<(String, Value)>)>;
}
```

#### Storage Implementation

```rust
// Internal representation
pub struct HashStorage {
    // Key -> Field -> Value
    data: Arc<RwLock<RadixMap<String, HashMap<String, StoredValue>>>>,
    // Same sharding strategy as KV store (64 shards)
    shards: [Arc<RwLock<HashMap<String, StoredValue>>>; 64],
}

// Metadata tracking
pub struct HashMeta {
    created_at: Instant,
    updated_at: Instant,
    field_count: usize,
    total_bytes: usize,
    ttl: Option<Instant>,  // TTL on entire hash, not per-field
}
```

#### REST API Endpoints

```http
POST /api/v1/hash/set
{
  "key": "user:1000",
  "field": "name",
  "value": "Alice"
}

POST /api/v1/hash/mset
{
  "key": "user:1000",
  "fields": {
    "name": "Alice",
    "age": 30,
    "email": "alice@example.com"
  }
}

GET /api/v1/hash/get/:key/:field
GET /api/v1/hash/getall/:key
POST /api/v1/hash/incrby
DELETE /api/v1/hash/del/:key/:field
```

#### StreamableHTTP Commands

```json
{
  "command": "hash.set",
  "payload": {
    "key": "user:1000",
    "field": "name",
    "value": "Alice"
  }
}

{
  "command": "hash.getall",
  "payload": {
    "key": "user:1000"
  }
}
```

#### Persistence Integration

- Hash changes append to OptimizedWAL
- Snapshots include hash data (field-by-field serialization)
- Recovery reconstructs hashes from WAL + snapshots
- TTL applies to entire hash, cleaned up by background task

#### Test Requirements

**Coverage**: 95%+

```rust
#[cfg(test)]
mod tests {
    // Basic operations
    #[tokio::test]
    async fn test_hset_hget()
    
    #[tokio::test]
    async fn test_hmset_hmget()
    
    // Atomic operations
    #[tokio::test]
    async fn test_hincrby_concurrent()
    
    // Edge cases
    #[tokio::test]
    async fn test_hash_with_ttl()
    
    #[tokio::test]
    async fn test_hash_field_overflow()
    
    // Performance
    #[tokio::test]
    async fn bench_hash_1k_fields()
    
    // Persistence
    #[tokio::test]
    async fn test_hash_recovery_from_wal()
}
```

#### Performance Targets

| Operation | Target Latency | Target Throughput |
|-----------|----------------|-------------------|
| HSET | < 100µs | 100K ops/sec |
| HGET | < 50µs | 200K ops/sec |
| HGETALL (100 fields) | < 500µs | 50K ops/sec |
| HINCRBY | < 100µs | 100K ops/sec |

#### Acceptance Criteria

- [ ] All 15 hash commands implemented
- [ ] REST API endpoints functional
- [ ] StreamableHTTP protocol support
- [ ] MCP tool: `synap_hash_set`, `synap_hash_get`, `synap_hash_getall`
- [ ] Persistence: WAL + Snapshot integration
- [ ] Replication: Hashes sync to replicas
- [ ] Tests: 95%+ coverage
- [ ] Benchmarks: Meet performance targets
- [ ] Documentation: API reference + examples

---

### 1.2 Lists (Linked list with push/pop)

**Motivation**: Essential for activity feeds, job queues, message buffers

#### API Design

```rust
pub trait ListOps {
    // Push operations
    async fn lpush(&self, key: &str, values: &[Value]) -> Result<usize>;
    async fn rpush(&self, key: &str, values: &[Value]) -> Result<usize>;
    async fn lpushx(&self, key: &str, value: Value) -> Result<usize>;
    async fn rpushx(&self, key: &str, value: Value) -> Result<usize>;
    
    // Pop operations
    async fn lpop(&self, key: &str, count: Option<usize>) -> Result<Vec<Value>>;
    async fn rpop(&self, key: &str, count: Option<usize>) -> Result<Vec<Value>>;
    
    // Blocking operations
    async fn blpop(&self, keys: &[&str], timeout: Duration) -> Result<Option<(String, Value)>>;
    async fn brpop(&self, keys: &[&str], timeout: Duration) -> Result<Option<(String, Value)>>;
    
    // Range operations
    async fn lrange(&self, key: &str, start: isize, stop: isize) -> Result<Vec<Value>>;
    async fn llen(&self, key: &str) -> Result<usize>;
    async fn lindex(&self, key: &str, index: isize) -> Result<Option<Value>>;
    async fn lset(&self, key: &str, index: isize, value: Value) -> Result<()>;
    
    // Trim and remove
    async fn ltrim(&self, key: &str, start: isize, stop: isize) -> Result<()>;
    async fn lrem(&self, key: &str, count: isize, value: Value) -> Result<usize>;
    
    // Insert
    async fn linsert(&self, key: &str, before: bool, pivot: Value, value: Value) -> Result<isize>;
    
    // Atomic move
    async fn rpoplpush(&self, source: &str, dest: &str) -> Result<Option<Value>>;
    async fn brpoplpush(&self, source: &str, dest: &str, timeout: Duration) -> Result<Option<Value>>;
}
```

#### Storage Implementation

```rust
pub struct ListStorage {
    // Key -> VecDeque (O(1) push/pop at both ends)
    lists: Arc<RwLock<RadixMap<String, VecDeque<StoredValue>>>>,
    
    // Blocking operation channels
    blocking_channels: Arc<DashMap<String, broadcast::Sender<()>>>,
}

// Metadata
pub struct ListMeta {
    created_at: Instant,
    updated_at: Instant,
    length: usize,
    total_bytes: usize,
    ttl: Option<Instant>,
}
```

#### Blocking Operations Design

```rust
// BLPOP implementation
async fn blpop(&self, keys: &[&str], timeout: Duration) -> Result<Option<(String, Value)>> {
    let deadline = Instant::now() + timeout;
    
    loop {
        // Try immediate pop
        for key in keys {
            if let Some(value) = self.lpop(key, Some(1)).await?.pop() {
                return Ok(Some((key.to_string(), value)));
            }
        }
        
        // Wait for notification or timeout
        tokio::select! {
            _ = tokio::time::sleep_until(deadline) => return Ok(None),
            _ = self.wait_for_list_update(keys) => continue,
        }
    }
}
```

#### Performance Targets

| Operation | Target Latency | Target Throughput |
|-----------|----------------|-------------------|
| LPUSH/RPUSH | < 100µs | 100K ops/sec |
| LPOP/RPOP | < 100µs | 100K ops/sec |
| LRANGE (100 items) | < 500µs | 50K ops/sec |
| BLPOP (no wait) | < 100µs | 100K ops/sec |
| BLPOP (with wait) | timeout duration | N/A |

#### Acceptance Criteria

- [ ] All 16 list commands implemented
- [ ] Blocking operations with timeout support
- [ ] REST API + StreamableHTTP protocol
- [ ] MCP tools for list operations
- [ ] Persistence integration
- [ ] Replication support
- [ ] 95%+ test coverage
- [ ] Performance benchmarks met

---

### 1.3 Sets (Unordered unique collections)

**Motivation**: Tags, relationships, unique tracking

#### API Design

```rust
pub trait SetOps {
    // Basic operations
    async fn sadd(&self, key: &str, members: &[Value]) -> Result<usize>;
    async fn srem(&self, key: &str, members: &[Value]) -> Result<usize>;
    async fn sismember(&self, key: &str, member: &Value) -> Result<bool>;
    async fn smembers(&self, key: &str) -> Result<HashSet<Value>>;
    async fn scard(&self, key: &str) -> Result<usize>;
    
    // Random operations
    async fn spop(&self, key: &str, count: Option<usize>) -> Result<Vec<Value>>;
    async fn srandmember(&self, key: &str, count: Option<isize>) -> Result<Vec<Value>>;
    
    // Set algebra (multi-key operations)
    async fn sinter(&self, keys: &[&str]) -> Result<HashSet<Value>>;
    async fn sunion(&self, keys: &[&str]) -> Result<HashSet<Value>>;
    async fn sdiff(&self, keys: &[&str]) -> Result<HashSet<Value>>;
    
    // Store results
    async fn sinterstore(&self, dest: &str, keys: &[&str]) -> Result<usize>;
    async fn sunionstore(&self, dest: &str, keys: &[&str]) -> Result<usize>;
    async fn sdiffstore(&self, dest: &str, keys: &[&str]) -> Result<usize>;
    
    // Move
    async fn smove(&self, source: &str, dest: &str, member: Value) -> Result<bool>;
    
    // Iteration
    async fn sscan(&self, key: &str, cursor: u64, pattern: Option<&str>, count: Option<usize>) -> Result<(u64, Vec<Value>)>;
}
```

#### Storage Implementation

```rust
pub struct SetStorage {
    // Key -> HashSet
    sets: Arc<RwLock<RadixMap<String, HashSet<Value>>>>,
}

// Use DashSet for concurrent operations
use dashmap::DashSet;
```

#### Set Algebra Optimization

```rust
// Optimized intersection using smallest set
async fn sinter(&self, keys: &[&str]) -> Result<HashSet<Value>> {
    let sets: Vec<_> = keys.iter()
        .map(|k| self.smembers(k))
        .collect::<Result<Vec<_>>>()?;
    
    // Sort by size, iterate smallest
    let smallest = sets.iter().min_by_key(|s| s.len()).unwrap();
    
    smallest.iter()
        .filter(|v| sets.iter().all(|s| s.contains(v)))
        .cloned()
        .collect()
}
```

#### Performance Targets

| Operation | Target Latency | Target Throughput |
|-----------|----------------|-------------------|
| SADD | < 100µs | 100K ops/sec |
| SISMEMBER | < 50µs | 200K ops/sec |
| SMEMBERS (1K items) | < 1ms | 50K ops/sec |
| SINTER (2 sets, 10K items each) | < 5ms | 10K ops/sec |

#### Acceptance Criteria

- [ ] All 15 set commands implemented
- [ ] Multi-key operations (SINTER, SUNION, SDIFF)
- [ ] Atomic SINTERSTORE, etc.
- [ ] REST API + StreamableHTTP
- [ ] MCP tools
- [ ] Persistence + Replication
- [ ] 95%+ coverage
- [ ] Benchmarks met

---

### 1.4 Implementation Strategy for Phase 1

#### Development Order

1. **Week 1-4**: Hashes
   - Core HSET/HGET/HDEL implementation
   - Persistence integration
   - REST API endpoints
   
2. **Week 5-8**: Hashes Advanced
   - HINCRBY, HMSET, HSCAN
   - Replication support
   - MCP tools
   - Testing + benchmarks

3. **Week 9-12**: Lists
   - Core LPUSH/RPUSH/LPOP/RPOP
   - LRANGE, LINDEX, LSET
   - Persistence integration

4. **Week 13-16**: Lists Advanced
   - Blocking operations (BLPOP, BRPOP)
   - RPOPLPUSH
   - REST API + MCP
   - Testing + benchmarks

5. **Week 17-20**: Sets
   - Core SADD/SREM/SISMEMBER
   - SMEMBERS, SCARD
   - Persistence integration

6. **Week 21-24**: Sets Advanced
   - Set algebra (SINTER, SUNION, SDIFF)
   - Store operations
   - REST API + MCP
   - Testing + benchmarks

#### Shared Infrastructure

**Common Components** (develop once, reuse):

1. **Type System Extension**:
```rust
pub enum StorageType {
    String,      // Existing
    Hash,        // NEW
    List,        // NEW
    Set,         // NEW
    SortedSet,   // Phase 2
    // ...
}

pub enum Value {
    String(String),
    Integer(i64),
    Float(f64),
    Bytes(Vec<u8>),
    Hash(HashMap<String, Value>),    // NEW
    List(VecDeque<Value>),            // NEW
    Set(HashSet<Value>),              // NEW
}
```

2. **Unified Persistence**:
```rust
// WAL entries for all types
pub enum WalEntry {
    KVSet { key: String, value: Value },
    HashSet { key: String, field: String, value: Value },
    ListPush { key: String, position: Position, value: Value },
    SetAdd { key: String, member: Value },
}
```

3. **Replication Protocol Extension**:
```rust
pub enum ReplicationOp {
    KV(KVOp),
    Hash(HashOp),
    List(ListOp),
    Set(SetOp),
}
```

#### Testing Strategy

**Test Matrix**:
- Unit tests per operation (95% coverage minimum)
- Integration tests across data types
- Concurrent access tests (100+ threads)
- Persistence recovery tests
- Replication consistency tests
- Performance benchmarks vs Redis

**Test Infrastructure**:
```rust
// Shared test utilities
mod test_utils {
    pub fn setup_test_server() -> SynapServer { ... }
    pub fn assert_eventually<F>(condition: F, timeout: Duration) { ... }
    pub fn benchmark_op<F>(name: &str, op: F) { ... }
}
```

---

## Phase 2: Advanced Operations (v0.5.0)

**Duration**: 6-9 months (cumulative)  
**Priority**: HIGH  
**Risk**: Medium-High

### 2.1 Sorted Sets (Score-based ordering)

**Motivation**: Leaderboards, time-series indexes, priority queues

#### API Design (25+ commands)

```rust
pub trait SortedSetOps {
    // Add/update
    async fn zadd(&self, key: &str, members: &[(f64, Value)]) -> Result<usize>;
    async fn zincrby(&self, key: &str, increment: f64, member: Value) -> Result<f64>;
    
    // Remove
    async fn zrem(&self, key: &str, members: &[Value]) -> Result<usize>;
    async fn zremrangebyrank(&self, key: &str, start: isize, stop: isize) -> Result<usize>;
    async fn zremrangebyscore(&self, key: &str, min: f64, max: f64) -> Result<usize>;
    
    // Query
    async fn zscore(&self, key: &str, member: Value) -> Result<Option<f64>>;
    async fn zrank(&self, key: &str, member: Value) -> Result<Option<usize>>;
    async fn zrevrank(&self, key: &str, member: Value) -> Result<Option<usize>>;
    
    // Range
    async fn zrange(&self, key: &str, start: isize, stop: isize) -> Result<Vec<Value>>;
    async fn zrevrange(&self, key: &str, start: isize, stop: isize) -> Result<Vec<Value>>;
    async fn zrangebyscore(&self, key: &str, min: f64, max: f64) -> Result<Vec<Value>>;
    
    // Count
    async fn zcard(&self, key: &str) -> Result<usize>;
    async fn zcount(&self, key: &str, min: f64, max: f64) -> Result<usize>;
    
    // Pop
    async fn zpopmin(&self, key: &str, count: Option<usize>) -> Result<Vec<(Value, f64)>>;
    async fn zpopmax(&self, key: &str, count: Option<usize>) -> Result<Vec<(Value, f64)>>;
    
    // Set operations with scores
    async fn zinterstore(&self, dest: &str, keys: &[&str], weights: &[f64], aggregate: Aggregate) -> Result<usize>;
    async fn zunionstore(&self, dest: &str, keys: &[&str], weights: &[f64], aggregate: Aggregate) -> Result<usize>;
}

pub enum Aggregate {
    Sum,
    Min,
    Max,
}
```

#### Storage Implementation

```rust
pub struct SortedSetStorage {
    // Dual data structure for O(log n) operations
    // Member -> Score lookup
    member_to_score: HashMap<Value, f64>,
    // Score -> Members (sorted)
    score_to_members: BTreeMap<OrderedFloat<f64>, HashSet<Value>>,
}

// Wrapper for f64 that implements Ord
use ordered_float::OrderedFloat;
```

#### Performance Targets

| Operation | Target Latency | Complexity |
|-----------|----------------|------------|
| ZADD | < 200µs | O(log n) |
| ZSCORE | < 50µs | O(1) |
| ZRANGE | < 1ms (100 items) | O(log n + m) |
| ZRANGEBYSCORE | < 2ms (100 items) | O(log n + m) |
| ZINTERSTORE | < 10ms (2 sets, 10K items) | O(n log n) |

#### Acceptance Criteria

- [ ] All 25+ sorted set commands
- [ ] Dual data structure implementation
- [ ] Set operations with weights and aggregation
- [ ] REST API + StreamableHTTP
- [ ] MCP tools
- [ ] Persistence + Replication
- [ ] 95%+ coverage
- [ ] Benchmarks competitive with Redis

---

### 2.2 String Command Extensions

Extend existing KV store with missing string operations:

| Command | Purpose | Complexity |
|---------|---------|------------|
| `APPEND` | Append to string | Low |
| `GETRANGE` | Get substring | Low |
| `SETRANGE` | Set substring | Low |
| `STRLEN` | String length | Low |
| `GETSET` | Atomic get and set | Low |
| `MSETNX` | Multi-set if all not exist | Medium |

#### Implementation

```rust
pub trait StringOpsExtended {
    async fn append(&self, key: &str, value: &str) -> Result<usize>;
    async fn getrange(&self, key: &str, start: isize, end: isize) -> Result<String>;
    async fn setrange(&self, key: &str, offset: usize, value: &str) -> Result<usize>;
    async fn strlen(&self, key: &str) -> Result<usize>;
    async fn getset(&self, key: &str, value: Value) -> Result<Option<Value>>;
    async fn msetnx(&self, pairs: &[(&str, Value)]) -> Result<bool>;
}
```

---

### 2.3 Key Management Commands

Add missing key operations:

| Command | Purpose | Complexity |
|---------|---------|------------|
| `EXISTS` | Check key existence | Low |
| `TYPE` | Get key type | Low |
| `RENAME` | Rename key | Medium |
| `RENAMENX` | Rename if new key doesn't exist | Medium |
| `COPY` | Copy key | Medium |
| `RANDOMKEY` | Get random key | Medium |

#### Implementation

```rust
pub trait KeyOps {
    async fn exists(&self, keys: &[&str]) -> Result<usize>;
    async fn key_type(&self, key: &str) -> Result<Option<StorageType>>;
    async fn rename(&self, key: &str, newkey: &str) -> Result<()>;
    async fn renamenx(&self, key: &str, newkey: &str) -> Result<bool>;
    async fn copy(&self, source: &str, dest: &str, replace: bool) -> Result<bool>;
    async fn randomkey(&self) -> Result<Option<String>>;
}
```

---

### 2.4 Enhanced Monitoring

Add Redis INFO-style introspection:

```rust
pub struct ServerInfo {
    // Server
    pub version: String,
    pub uptime_seconds: u64,
    pub process_id: u32,
    
    // Memory
    pub used_memory: usize,
    pub used_memory_peak: usize,
    pub memory_fragmentation_ratio: f64,
    
    // Stats
    pub total_connections_received: u64,
    pub total_commands_processed: u64,
    pub ops_per_sec: f64,
    
    // Replication
    pub role: Role,  // Master/Replica
    pub connected_replicas: usize,
    pub replication_offset: u64,
    pub replication_lag_seconds: f64,
    
    // Data structures
    pub keyspace: HashMap<StorageType, usize>,
}
```

#### REST Endpoints

```http
GET /api/v1/info
GET /api/v1/info/server
GET /api/v1/info/memory
GET /api/v1/info/stats
GET /api/v1/info/replication
GET /api/v1/slowlog
```

---

## Phase 3: Transactions & Scripting (v0.6.0)

**Duration**: 9-12 months (cumulative)  
**Priority**: HIGH  
**Risk**: High

### 3.1 Transactions (MULTI/EXEC/WATCH)

**Motivation**: Atomic multi-key operations, optimistic locking

#### API Design

```rust
pub struct Transaction {
    commands: Vec<Command>,
    watched_keys: Vec<String>,
    watched_versions: HashMap<String, u64>,
}

pub trait TransactionOps {
    async fn multi(&self) -> Transaction;
    async fn exec(&self, tx: Transaction) -> Result<Vec<Value>>;
    async fn discard(&self, tx: Transaction);
    async fn watch(&self, tx: &mut Transaction, keys: &[&str]) -> Result<()>;
    async fn unwatch(&self, tx: &mut Transaction);
}
```

#### Implementation Strategy

**Key Versioning**:
```rust
pub struct VersionedValue {
    value: Value,
    version: u64,  // Incremented on every write
}
```

**Transaction Execution**:
```rust
async fn exec(&self, tx: Transaction) -> Result<Vec<Value>> {
    // 1. Check watched keys haven't changed
    for (key, expected_version) in tx.watched_versions {
        let current = self.get_version(&key).await?;
        if current != expected_version {
            return Err(SynapError::TransactionAborted);
        }
    }
    
    // 2. Acquire locks on all accessed keys (sorted to avoid deadlock)
    let mut locks = self.acquire_locks(&tx.get_keys()).await?;
    
    // 3. Execute commands atomically
    let mut results = Vec::new();
    for cmd in tx.commands {
        results.push(self.execute_command(cmd).await?);
    }
    
    // 4. Release locks
    drop(locks);
    
    Ok(results)
}
```

#### Test Requirements

```rust
#[tokio::test]
async fn test_multi_exec_atomic() {
    // Transfer funds atomically
    let tx = server.multi().await;
    tx.decrby("account:A", 100).await;
    tx.incrby("account:B", 100).await;
    let results = tx.exec().await.unwrap();
    
    assert_eq!(results.len(), 2);
}

#[tokio::test]
async fn test_watch_abort_on_conflict() {
    let mut tx = server.multi().await;
    server.watch(&mut tx, &["key"]).await;
    
    // Concurrent write
    server.set("key", "modified").await;
    
    tx.set("key", "new_value").await;
    assert_eq!(tx.exec().await, Err(SynapError::TransactionAborted));
}
```

#### Performance Targets

- Transaction overhead: < 500µs
- Watch overhead: < 100µs per key
- Support 1000+ concurrent transactions

---

### 3.2 Lua Scripting

**Motivation**: Complex server-side logic, reduced network round-trips

#### API Design

```rust
pub trait ScriptOps {
    async fn eval(&self, script: &str, keys: &[&str], args: &[Value]) -> Result<Value>;
    async fn evalsha(&self, sha: &str, keys: &[&str], args: &[Value]) -> Result<Value>;
    async fn script_load(&self, script: &str) -> Result<String>;
    async fn script_exists(&self, shas: &[&str]) -> Result<Vec<bool>>;
    async fn script_flush(&self) -> Result<()>;
    async fn script_kill(&self) -> Result<()>;
}
```

#### Implementation Using mlua

```rust
use mlua::{Lua, Table, Function};

pub struct ScriptEngine {
    lua: Lua,
    script_cache: Arc<RwLock<HashMap<String, String>>>,
}

impl ScriptEngine {
    pub fn new() -> Self {
        let lua = Lua::new();
        
        // Inject redis.call function
        let globals = lua.globals();
        let redis_table = lua.create_table().unwrap();
        
        redis_table.set("call", lua.create_function(|lua, (cmd, args): (String, mlua::MultiValue)| {
            // Execute Synap command from Lua
            // ...
        }).unwrap()).unwrap();
        
        globals.set("redis", redis_table).unwrap();
        
        Self {
            lua,
            script_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    pub async fn eval(&self, script: &str, keys: &[&str], args: &[Value]) -> Result<Value> {
        // Compile and execute
        let chunk = self.lua.load(script);
        let result: mlua::Value = chunk.eval_async().await?;
        
        // Convert mlua::Value to Synap Value
        self.convert_lua_value(result)
    }
}
```

#### Sandboxing

```rust
// Disable dangerous functions
lua.sandbox(true)?;

// Timeout for long-running scripts
tokio::time::timeout(Duration::from_secs(5), script.eval()).await?
```

#### Example Scripts

```lua
-- Rate limiting
local current = redis.call('INCR', KEYS[1])
if current == 1 then
    redis.call('EXPIRE', KEYS[1], ARGV[1])
end
if current > tonumber(ARGV[2]) then
    return 0  -- Rate limit exceeded
end
return 1  -- OK
```

#### Performance Targets

- Script compilation: < 10ms
- Cached script execution: < 1ms overhead
- Support 100+ cached scripts
- Timeout enforcement: < 5 seconds default

---

## Phase 4: Cluster & Enterprise (v0.7.0)

**Duration**: 12-18 months (cumulative)  
**Priority**: MEDIUM  
**Risk**: Very High

### 4.1 Cluster Mode (Horizontal Sharding)

**Motivation**: Scale writes beyond single master, handle TB+ datasets

#### Architecture

```
┌─────────────────────────────────────────────┐
│            Client (Cluster-Aware)           │
└─────────────────────────────────────────────┘
                    │
        ┌───────────┼───────────┐
        ▼           ▼           ▼
  ┌─────────┐ ┌─────────┐ ┌─────────┐
  │ Master 1│ │ Master 2│ │ Master 3│
  │ Slots:  │ │ Slots:  │ │ Slots:  │
  │ 0-5460  │ │5461-10922│ │10923-16383│
  └─────────┘ └─────────┘ └─────────┘
      │           │           │
  ┌───┴───┐   ┌───┴───┐   ┌───┴───┐
  │Replica│   │Replica│   │Replica│
  └───────┘   └───────┘   └───────┘
```

#### Hash Slot Algorithm

```rust
fn hash_slot(key: &str) -> u16 {
    // Extract hash tag if present: {user:1000}:profile -> user:1000
    let hash_key = if let Some(start) = key.find('{') {
        if let Some(end) = key[start+1..].find('}') {
            &key[start+1..start+1+end]
        } else {
            key
        }
    } else {
        key
    };
    
    // CRC16 mod 16384
    crc16(hash_key.as_bytes()) % 16384
}
```

#### Cluster Protocol

```rust
pub struct ClusterNode {
    id: String,
    address: SocketAddr,
    role: NodeRole,
    slots: Vec<SlotRange>,
    replicas: Vec<String>,
}

pub enum NodeRole {
    Master,
    Replica { master_id: String },
}

pub struct SlotRange {
    start: u16,
    end: u16,
}
```

#### Migration

```rust
pub struct SlotMigration {
    slot: u16,
    from_node: String,
    to_node: String,
    state: MigrationState,
}

pub enum MigrationState {
    Preparing,
    Migrating { keys_moved: usize, keys_total: usize },
    Finalizing,
    Complete,
}
```

#### Complexity Assessment

**Very High** due to:
- Distributed consensus (Raft/Paxos)
- Slot migration with zero downtime
- Automatic failover
- Cluster topology management
- Client redirection protocol
- Split-brain prevention

**Estimated Effort**: 6-9 months with 2-3 engineers

---

### 4.2 Bitmaps, HyperLogLog, Geospatial

Lower priority probabilistic and specialized structures.

**Defer to Phase 4 or later** (v0.8.0+)

---

## Cross-Cutting Concerns

### Persistence Integration

All new data structures must integrate with:

1. **OptimizedWAL**:
   - Append operations to WAL before applying
   - Batch writes for performance
   - fsync modes: Always, Periodic, Never

2. **Snapshots**:
   - Streaming snapshot v2 (O(1) memory)
   - Incremental snapshots for large datasets
   - Background saving (BGSAVE equivalent)

3. **Recovery**:
   - WAL replay on startup
   - Snapshot + incremental WAL
   - Verify data integrity (CRC32)

### Replication Protocol

Extend existing master-slave replication:

```rust
pub enum ReplicationEntry {
    // Existing
    KVSet { key: String, value: Value, ttl: Option<Duration> },
    KVDel { key: String },
    
    // Phase 1
    HashSet { key: String, field: String, value: Value },
    HashDel { key: String, fields: Vec<String> },
    ListPush { key: String, position: Position, values: Vec<Value> },
    ListPop { key: String, position: Position, count: usize },
    SetAdd { key: String, members: Vec<Value> },
    SetRem { key: String, members: Vec<Value> },
    
    // Phase 2
    SortedSetAdd { key: String, members: Vec<(f64, Value)> },
    SortedSetRem { key: String, members: Vec<Value> },
    
    // Phase 3
    Transaction { commands: Vec<Command> },
    ScriptExec { sha: String, keys: Vec<String>, args: Vec<Value> },
}
```

### MCP Integration

Extend MCP tools for each data structure:

**Phase 1 MCP Tools**:
```json
{
  "tools": [
    "synap_hash_set",
    "synap_hash_get",
    "synap_hash_getall",
    "synap_list_push",
    "synap_list_pop",
    "synap_list_range",
    "synap_set_add",
    "synap_set_members",
    "synap_set_inter"
  ]
}
```

### REST API Consistency

Maintain consistent REST API design:

```http
# Pattern: /api/v1/{structure}/{operation}
POST /api/v1/hash/set
POST /api/v1/hash/mset
GET  /api/v1/hash/get/:key/:field
GET  /api/v1/hash/getall/:key

POST /api/v1/list/push
POST /api/v1/list/pop
GET  /api/v1/list/range/:key

POST /api/v1/set/add
POST /api/v1/set/rem
GET  /api/v1/set/members/:key
POST /api/v1/set/inter
```

### Performance Benchmarking

Continuous benchmarking against Redis:

```bash
# Automated benchmark suite
cargo bench --bench redis_comparison

# Test matrix
- Data structure: Hash, List, Set, SortedSet
- Operation: Add, Get, Delete, Range
- Dataset size: 1K, 10K, 100K, 1M keys
- Concurrency: 1, 10, 100, 1000 threads
- Value size: 10B, 100B, 1KB, 10KB
```

**Target**: Within 2x of Redis performance for equivalent operations

---

## Migration Path

### From Redis to Synap

Provide migration tooling:

```rust
pub struct RedisMigrator {
    redis_client: redis::Client,
    synap_client: SynapClient,
}

impl RedisMigrator {
    pub async fn migrate(&self, options: MigrationOptions) -> Result<MigrationReport> {
        // 1. Scan Redis keyspace
        let keys = self.scan_redis_keys().await?;
        
        // 2. For each key, determine type
        for key in keys {
            let key_type = self.redis_client.key_type(&key).await?;
            
            match key_type {
                "string" => self.migrate_string(&key).await?,
                "hash" => self.migrate_hash(&key).await?,
                "list" => self.migrate_list(&key).await?,
                "set" => self.migrate_set(&key).await?,
                "zset" => self.migrate_sorted_set(&key).await?,
                _ => warn!("Unsupported type: {}", key_type),
            }
        }
        
        // 3. Verify data integrity
        self.verify_migration().await?;
        
        Ok(report)
    }
}
```

### Zero-Downtime Migration

```
┌──────────┐
│  Redis   │◄─── Writes (during migration)
└──────────┘
     │
     ├──────► Synap (copy existing data)
     │
     ▼
┌──────────┐
│  Synap   │◄─── Writes (after switchover)
└──────────┘
```

Strategy:
1. Start Synap instance
2. Copy existing Redis data (SCAN + RESTORE)
3. Enable dual-write (app writes to both)
4. Verify data consistency
5. Switch reads to Synap
6. Disable Redis writes
7. Decommission Redis

---

## Risk Assessment

### Technical Risks

| Risk | Impact | Probability | Mitigation |
|------|--------|-------------|------------|
| **Complexity Underestimation** | High | Medium | Phased approach, quarterly reviews |
| **Performance Degradation** | High | Medium | Continuous benchmarking, profiling |
| **Data Corruption** | Critical | Low | Extensive testing, CRC verification |
| **Replication Bugs** | High | Medium | Integration tests, chaos testing |
| **Memory Leaks** | High | Low | Valgrind, memory profiling |

### Schedule Risks

| Risk | Impact | Probability | Mitigation |
|------|--------|-------------|------------|
| **Scope Creep** | Medium | High | Strict phase boundaries, no mid-phase additions |
| **Resource Constraints** | High | Medium | Prioritize critical features, outsource docs |
| **Dependency Issues** | Low | Low | Pin crate versions, vendor critical deps |

### Market Risks

| Risk | Impact | Probability | Mitigation |
|------|--------|-------------|------------|
| **Low Adoption** | High | Medium | Early beta program, community engagement |
| **Redis Compatibility Issues** | Medium | Medium | Comprehensive test suite vs Redis |
| **Competing Solutions** | Medium | Low | Differentiate with MCP/UMICP, focus on AI use cases |

---

## Success Criteria

### Phase 1 Success

- [ ] Hashes, Lists, Sets fully implemented
- [ ] 95%+ test coverage
- [ ] Performance within 2x of Redis
- [ ] 10+ external beta testers
- [ ] Zero critical bugs in production

### Phase 2 Success

- [ ] Sorted Sets operational
- [ ] Extended string/key commands
- [ ] Enhanced monitoring dashboard
- [ ] 100+ GitHub stars
- [ ] 5+ community contributions

### Phase 3 Success

- [ ] Transactions working reliably
- [ ] Lua scripting with 50+ example scripts
- [ ] 1000+ downloads/month on crates.io
- [ ] Production use by 3+ companies

### Phase 4 Success

- [ ] Cluster mode (3+ node deployment tested)
- [ ] Migration tool used by 10+ Redis users
- [ ] Published performance comparison paper
- [ ] Conference talk accepted (RustConf, RedisConf)

---

## Resource Requirements

### Team Composition

**Minimum Viable Team**:
- 1 Senior Rust Engineer (full-time, 18 months)
- 1 Mid-level Rust Engineer (full-time, 18 months)
- 1 DevOps Engineer (50%, for CI/CD, monitoring)
- 1 Technical Writer (25%, for documentation)

**Ideal Team**:
- 2 Senior Rust Engineers
- 2 Mid-level Rust Engineers
- 1 QA Engineer (testing, benchmarking)
- 1 DevOps Engineer
- 1 Technical Writer

### Budget Estimate

| Item | Cost (USD) |
|------|------------|
| Engineering (2x FTE, 18 months) | $450,000 |
| DevOps/Infrastructure | $30,000 |
| Documentation | $25,000 |
| Community/Marketing | $15,000 |
| **Total** | **$520,000** |

---

## Alternatives Considered

### Alternative 1: Full Redis Fork

**Pros**:
- 100% compatibility
- Proven codebase

**Cons**:
- C codebase (not Rust)
- Loses Synap advantages (MCP, compression, etc.)
- Large maintenance burden

**Decision**: Rejected

---

### Alternative 2: Redis RESP Protocol Compatibility Layer

**Pros**:
- Drop-in replacement for Redis clients
- Easier migration

**Cons**:
- Significant protocol implementation effort
- Limits modern API design
- Binary protocol complexity

**Decision**: Consider for Phase 5 (v0.8.0+)

---

### Alternative 3: Minimal Redis Subset

**Pros**:
- Faster time-to-market
- Less complexity

**Cons**:
- Insufficient for most use cases
- Poor competitive position
- Still requires migration effort

**Decision**: Rejected, go with full Phase 1-3 plan

---

## Next Steps

### Immediate Actions (Next 30 Days)

1. **Stakeholder Approval**:
   - Review this proposal with core team
   - Get sign-off on priorities and timeline
   - Allocate budget and resources

2. **Technical Preparation**:
   - Create detailed Hash implementation spec
   - Set up benchmark infrastructure
   - Design unified type system

3. **Community Engagement**:
   - Post roadmap to GitHub Discussions
   - Solicit feedback from potential users
   - Recruit beta testers

4. **Project Setup**:
   - Create GitHub project board
   - Set up CI/CD for new modules
   - Write contribution guide for new features

### First Sprint (Weeks 1-2)

1. Implement `StorageType` enum extension
2. Create `HashStorage` skeleton
3. Implement HSET/HGET/HDEL (core 3)
4. Write 20+ unit tests
5. Integration with existing persistence layer

---

## Appendices

### Appendix A: Command Compatibility Matrix

See `docs/REDIS_COMPARISON.md` for full 600+ line analysis.

### Appendix B: Performance Benchmarks

```bash
# Redis baseline (run locally)
redis-benchmark -t set,get,hset,hget,lpush,lpop,sadd,smembers -n 100000 -q

# Synap target (after implementation)
synap-benchmark -t set,get,hset,hget,lpush,lpop,sadd,smembers -n 100000 -q
```

### Appendix C: Migration Examples

**Example 1: Migrate User Profiles (Hash)**
```bash
# Redis
HSET user:1000 name "Alice" age "30" email "alice@example.com"

# Synap (same API)
curl -X POST http://localhost:15500/api/v1/hash/mset \
  -d '{"key": "user:1000", "fields": {"name": "Alice", "age": 30, "email": "alice@example.com"}}'
```

**Example 2: Migrate Activity Feed (List)**
```bash
# Redis
LPUSH feed:user:1000 "event1" "event2" "event3"

# Synap
curl -X POST http://localhost:15500/api/v1/list/push \
  -d '{"key": "feed:user:1000", "position": "left", "values": ["event1", "event2", "event3"]}'
```

### Appendix D: References

1. Redis Documentation: https://redis.io/docs/
2. Redis Internals: https://redis.io/topics/internals
3. Synap Architecture: `docs/ARCHITECTURE.md`
4. Synap Redis Comparison: `docs/REDIS_COMPARISON.md`

---

**End of Proposal**

**Status**: Draft - Awaiting Review  
**Next Review**: TBD  
**Approval Required**: Core Team Lead, Product Manager, CTO

