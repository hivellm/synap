# Replication Improvements Specification

## Overview

This specification covers three replication-related improvements:
1. TTL Support in Replication Sync
2. Replication Lag Calculation
3. Replication Byte Tracking

## 1. TTL Support in Replication Sync

### Current State

- Replication sync works for KV operations
- TTL is NOT included when syncing keys
- Keys with TTL may lose expiration time during replication
- TTL is preserved during normal replication operations

### Proposed Changes

#### 1.1 Get TTL During Sync

**File**: `synap-server/src/replication/sync.rs` (line ~48)

Update sync operation creation to include TTL:

```rust
pub async fn create_sync_operations(
    kv_store: &KVStore,
    keys: &[String],
) -> Result<Vec<Operation>> {
    let mut operations = Vec::new();
    
    for key in keys {
        let value = kv_store.get(key).await?;
        if let Some(value_bytes) = value {
            // Get TTL for the key
            let ttl = kv_store.ttl(key).await?;
            
            operations.push(Operation::KVSet {
                key: key.clone(),
                value: value_bytes,
                ttl, // Include TTL
            });
        }
    }
    
    Ok(operations)
}
```

#### 1.2 Ensure TTL Preservation

**File**: `synap-server/src/replication/replica.rs`

Verify TTL is set correctly on replica:

```rust
pub async fn apply_operation(&mut self, operation: Operation) -> Result<()> {
    match operation {
        Operation::KVSet { key, value, ttl } => {
            self.kv_store.set(&key, value, ttl).await?;
            // TTL is automatically handled by set() method
        }
        // ... other operations ...
    }
    Ok(())
}
```

### Testing Requirements

- [ ] Unit test: TTL included in sync operations
- [ ] Integration test: TTL preserved during replication
- [ ] Test: TTL expiration timing matches master
- [ ] Test: TTL with different expiration times
- [ ] Test: TTL with very short expiration (< 1 second)
- [ ] Test: TTL with very long expiration (> 1 year)

### Performance Considerations

- Getting TTL adds ~50ns overhead per key
- No impact on replication throughput
- TTL is already stored, just needs to be retrieved

---

## 2. Replication Lag Calculation

### Current State

- Replication stats exist but lag calculation is hardcoded to 0
- Heartbeat timestamps are tracked but not used for lag calculation
- Operation timestamps are available but not compared

### Proposed Changes

#### 2.1 Calculate Lag from Heartbeats

**File**: `synap-server/src/replication/master.rs` (line ~436)

Update replica list to calculate lag:

```rust
pub fn list_replicas(&self) -> Vec<ReplicaInfo> {
    let replicas = self.replicas.read();
    let now = SystemTime::now();
    
    replicas
        .iter()
        .map(|r| {
            let lag_ms = if let Ok(duration) = now.duration_since(r.last_heartbeat) {
                duration.as_millis() as u64
            } else {
                0
            };
            
            ReplicaInfo {
                id: r.id.clone(),
                address: r.address.clone(),
                offset: r.offset,
                connected_at: r.connected_at,
                last_sync: r.last_heartbeat,
                lag_ms, // Calculate from heartbeat
            }
        })
        .collect()
}
```

#### 2.2 Calculate Lag from Operation Timestamps

**File**: `synap-server/src/replication/master.rs` (line ~457)

Update replication stats to calculate lag:

```rust
pub fn stats(&self) -> ReplicationStats {
    let replicas = self.replicas.read();
    let current_offset = self.replication_log.current_offset();
    
    let min_replica_offset = replicas
        .iter()
        .map(|r| r.offset)
        .min()
        .unwrap_or(current_offset);
    
    // Calculate lag from operation timestamps
    let lag_ms = if let Some(last_op_time) = self.replication_log.last_operation_time() {
        let now = SystemTime::now();
        if let Ok(duration) = now.duration_since(last_op_time) {
            duration.as_millis() as u64
        } else {
            0
        }
    } else {
        0
    };
    
    ReplicationStats {
        master_offset: current_offset,
        replica_offset: min_replica_offset,
        lag_operations: current_offset.saturating_sub(min_replica_offset),
        lag_ms, // Calculate from timestamps
        total_replicated: current_offset,
        total_bytes: 0, // TODO: Task 6
        last_heartbeat: Self::current_timestamp(),
    }
}
```

#### 2.3 Track Operation Timestamps

**File**: `synap-server/src/replication/log.rs`

Add timestamp tracking to replication log:

```rust
pub struct ReplicationLog {
    // ... existing fields ...
    last_operation_time: Arc<RwLock<Option<SystemTime>>>, // NEW
}

impl ReplicationLog {
    pub async fn append(&mut self, operation: Operation) -> Result<u64> {
        // ... existing append logic ...
        
        // Update last operation time
        *self.last_operation_time.write().await = Some(SystemTime::now());
        
        Ok(offset)
    }
    
    pub fn last_operation_time(&self) -> Option<SystemTime> {
        *self.last_operation_time.read()
    }
}
```

### Testing Requirements

- [ ] Unit test: Lag calculation from heartbeats
- [ ] Unit test: Lag calculation from operation timestamps
- [ ] Integration test: Lag metrics in replication stats
- [ ] Test: Lag accuracy under load
- [ ] Test: Lag with disconnected replicas
- [ ] Test: Lag with slow replicas

### Performance Considerations

- Lag calculation adds ~10ns overhead
- Timestamp tracking adds minimal memory overhead
- No impact on replication throughput

---

## 3. Replication Byte Tracking

### Current State

- Total replicated operations are tracked
- Total bytes replicated is hardcoded to 0
- Operation sizes are not tracked

### Proposed Changes

#### 3.1 Track Operation Sizes

**File**: `synap-server/src/replication/master.rs` (line ~459)

Add byte tracking:

```rust
pub struct ReplicationMaster {
    // ... existing fields ...
    total_bytes_replicated: Arc<AtomicU64>, // NEW
}

impl ReplicationMaster {
    pub async fn replicate_to_replicas(&self, operation: Operation) -> Result<()> {
        // Calculate operation size
        let operation_size = self.calculate_operation_size(&operation);
        
        // ... existing replication logic ...
        
        // Update total bytes
        self.total_bytes_replicated
            .fetch_add(operation_size, Ordering::Relaxed);
        
        Ok(())
    }
    
    fn calculate_operation_size(&self, operation: &Operation) -> u64 {
        match operation {
            Operation::KVSet { key, value, .. } => {
                (key.len() + value.len()) as u64
            }
            Operation::KVDel { keys } => {
                keys.iter().map(|k| k.len() as u64).sum()
            }
            // ... other operations ...
            _ => 0,
        }
    }
}
```

#### 3.2 Update Replication Stats

**File**: `synap-server/src/replication/master.rs`

Update stats to include byte tracking:

```rust
pub fn stats(&self) -> ReplicationStats {
    // ... existing code ...
    
    ReplicationStats {
        // ... existing fields ...
        total_bytes: self.total_bytes_replicated.load(Ordering::Relaxed),
    }
}
```

#### 3.3 Add Per-Replica Byte Tracking (Optional)

**File**: `synap-server/src/replication/master.rs`

Track bytes per replica:

```rust
pub struct ReplicaInfo {
    // ... existing fields ...
    pub bytes_replicated: u64, // NEW
}
```

### Testing Requirements

- [ ] Unit test: Byte tracking for KV operations
- [ ] Unit test: Byte tracking for different operation types
- [ ] Integration test: Total bytes in replication stats
- [ ] Test: Byte tracking accuracy
- [ ] Test: Byte tracking with large values
- [ ] Test: Byte tracking reset on snapshot

### Performance Considerations

- Byte calculation adds ~20ns overhead per operation
- Atomic operations are lock-free
- Memory overhead: 8 bytes per replica (if per-replica tracking)

### Future Enhancements

- Per-replica byte tracking
- Byte tracking by operation type
- Network bandwidth metrics
- Compression ratio tracking

