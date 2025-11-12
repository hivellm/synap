# Persistence Improvements Specification

## Overview

This specification covers two persistence-related improvements:
1. RENAME Operation WAL Logging
2. Queue Persistence Integration

## 1. RENAME Operation WAL Logging

### Current State

- RENAME operation works correctly at the application level
- Currently logged as a copy + delete sequence (workaround)
- No dedicated `KVRename` operation in WAL `Operation` enum
- TTL is preserved during RENAME (handled in `key_manager.rs`)

### Proposed Changes

#### 1.1 Add KVRename Operation Type

**File**: `synap-server/src/persistence/types.rs`

Add new variant to `Operation` enum:

```rust
pub enum Operation {
    // ... existing variants ...
    
    /// KV Store RENAME operation
    KVRename {
        source: String,
        destination: String,
    },
}
```

#### 1.2 Update WAL Replay Logic

**File**: `synap-server/src/persistence/recovery.rs`

Add handling for `KVRename` in replay logic:

```rust
match entry.operation {
    // ... existing cases ...
    Operation::KVRename { source, destination } => {
        let manager = create_key_manager(&state);
        manager.rename(&source, &destination).await?;
    }
}
```

#### 1.3 Update REST Handler

**File**: `synap-server/src/server/handlers.rs` (line ~678)

Replace TODO comment with actual logging:

```rust
pub async fn key_rename(
    State(state): State<AppState>,
    Path(source): Path<String>,
    Json(req): Json<RenameRequest>,
) -> Result<Json<RenameResponse>, SynapError> {
    // ... existing code ...
    
    let manager = create_key_manager(&state);
    manager.rename(&source, &req.destination).await?;

    // Log to WAL if persistence is enabled
    if let Some(ref persistence) = state.persistence {
        persistence
            .log_kv_rename(source.clone(), req.destination.clone())
            .await?;
    }

    Ok(Json(RenameResponse { /* ... */ }))
}
```

#### 1.4 Update StreamableHTTP Handler

**File**: `synap-server/src/server/handlers.rs` (line ~2572)

Replace TODO comment with actual logging:

```rust
async fn handle_key_rename_cmd(
    state: &AppState,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    // ... existing code ...
    
    let manager = create_key_manager(state);
    manager.rename(source, destination).await?;

    // Log to WAL if persistence is enabled
    if let Some(ref persistence) = state.persistence {
        persistence
            .log_kv_rename(source.to_string(), destination.to_string())
            .await?;
    }

    Ok(serde_json::json!({ /* ... */ }))
}
```

#### 1.5 Add Logging Method to Persistence Layer

**File**: `synap-server/src/persistence/layer.rs`

Add method to log RENAME operations:

```rust
impl PersistenceLayer {
    // ... existing methods ...
    
    pub async fn log_kv_rename(
        &self,
        source: String,
        destination: String,
    ) -> Result<u64> {
        self.wal
            .append(Operation::KVRename { source, destination })
            .await
    }
}
```

### Testing Requirements

- [ ] Unit test: WAL entry creation for RENAME
- [ ] Unit test: WAL replay with RENAME operation
- [ ] Integration test: RENAME with persistence enabled
- [ ] Test: RENAME preserves TTL correctly
- [ ] Test: RENAME across different key types (KV, Hash, List, Set, SortedSet)
- [ ] Test: RENAME with non-existent source key (error handling)
- [ ] Test: RENAME with existing destination (overwrite behavior)

### Performance Considerations

- RENAME logging adds minimal overhead (~1-2µs)
- Atomic operation ensures consistency
- No impact on existing RENAME functionality

---

## 2. Queue Persistence Integration

### Current State

- Queue persistence layer exists (`synap-server/src/persistence/queue_persistence.rs`)
- Queue operations are NOT currently persisted to WAL
- Queue persistence infrastructure is ready but not integrated
- Queue operations work correctly in memory

### Proposed Changes

#### 2.1 Integrate QueuePersistence into AppState

**File**: `synap-server/src/lib.rs`

Add `QueuePersistence` to `AppState`:

```rust
pub struct AppState {
    // ... existing fields ...
    persistence: Option<PersistenceLayer>,
    queue_persistence: Option<QueuePersistence>, // NEW
}
```

#### 2.2 Initialize Queue Persistence

**File**: `synap-server/src/main.rs`

Initialize queue persistence when persistence is enabled:

```rust
let queue_persistence = if config.persistence.enabled {
    Some(QueuePersistence::new(config.persistence.wal.clone()).await?)
} else {
    None
};
```

#### 2.3 Update Queue Publish Handler

**File**: `synap-server/src/server/handlers.rs` (line ~1540)

Add persistence logging:

```rust
async fn handle_queue_publish_cmd(
    state: &AppState,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    // ... existing code ...
    
    let message = queue_manager.publish(/* ... */).await?;

    // Log to WAL if queue persistence is enabled
    if let Some(ref queue_persistence) = state.queue_persistence {
        queue_persistence
            .log_publish(queue_name.clone(), message.clone())
            .await?;
    }

    Ok(serde_json::json!({ /* ... */ }))
}
```

#### 2.4 Update Queue ACK Handler

**File**: `synap-server/src/server/handlers.rs`

Add persistence logging:

```rust
async fn handle_queue_ack_cmd(
    state: &AppState,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    // ... existing code ...
    
    queue_manager.ack(/* ... */).await?;

    // Log to WAL if queue persistence is enabled
    if let Some(ref queue_persistence) = state.queue_persistence {
        queue_persistence
            .log_ack(queue_name.clone(), message_id.clone())
            .await?;
    }

    Ok(serde_json::json!({ /* ... */ }))
}
```

#### 2.5 Update Queue NACK Handler

**File**: `synap-server/src/server/handlers.rs`

Add persistence logging:

```rust
async fn handle_queue_nack_cmd(
    state: &AppState,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    // ... existing code ...
    
    queue_manager.nack(/* ... */).await?;

    // Log to WAL if queue persistence is enabled
    if let Some(ref queue_persistence) = state.queue_persistence {
        queue_persistence
            .log_nack(queue_name.clone(), message_id.clone(), requeue)
            .await?;
    }

    Ok(serde_json::json!({ /* ... */ }))
}
```

#### 2.6 Update Recovery Logic

**File**: `synap-server/src/persistence/recovery.rs`

Add queue recovery from WAL:

```rust
pub async fn recover_queues(
    &self,
    queue_manager: &QueueManager,
) -> Result<u64> {
    if let Some(ref queue_persistence) = self.queue_persistence {
        queue_persistence
            .recover(queue_manager, &self.wal_path)
            .await
    } else {
        Ok(0)
    }
}
```

#### 2.7 Update Snapshot Creation

**File**: `synap-server/src/persistence/snapshot.rs`

Include queue data in snapshots:

```rust
pub struct Snapshot {
    // ... existing fields ...
    pub queue_data: HashMap<String, Vec<QueueMessage>>,
}
```

### Testing Requirements

- [ ] Unit test: Queue publish logged to WAL
- [ ] Unit test: Queue ACK logged to WAL
- [ ] Unit test: Queue NACK logged to WAL
- [ ] Integration test: Queue recovery from WAL
- [ ] Test: Queue persistence with ACK/NACK
- [ ] Test: Queue persistence with dead letter queue
- [ ] Test: Queue persistence with priorities
- [ ] Test: Queue persistence with TTL
- [ ] Performance test: Queue persistence overhead
- [ ] Test: Queue snapshot creation and recovery

### Performance Considerations

- Queue persistence adds ~5-10µs overhead per operation
- Batch logging can reduce overhead
- Recovery time depends on WAL size
- Consider periodic snapshot creation for large queues

### Migration Notes

- Existing queues without persistence will continue to work
- Enabling persistence requires server restart
- WAL replay will restore queue state on startup

