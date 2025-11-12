# TODO: Implementation Tasks

This document lists all pending implementation tasks found in the codebase.

## Persistence & WAL (Write-Ahead Log)

### 1. RENAME Operation WAL Logging
**Priority:** Medium  
**Files:**
- `synap-server/src/server/handlers.rs:678`
- `synap-server/src/server/handlers.rs:2572`

**Current Status:**
- RENAME operation exists and works correctly
- Currently logged as copy + delete sequence (workaround)
- No dedicated `KVRename` operation in WAL `Operation` enum

**Implementation Required:**
1. Add `KVRename { source: String, destination: String }` variant to `Operation` enum in `synap-server/src/persistence/types.rs`
2. Update WAL replay logic to handle `KVRename` operation
3. Update both REST and StreamableHTTP handlers to log RENAME operation:
   - `key_rename()` function (line 678)
   - `handle_key_rename_cmd()` function (line 2572)
4. Ensure TTL is preserved during RENAME (currently handled in `key_manager.rs`)

**Related Code:**
- `synap-server/src/core/key_manager.rs:147` - RENAME implementation
- `synap-server/src/persistence/types.rs:65` - Operation enum
- `synap-server/src/persistence/layer.rs` - WAL logging layer

---

### 2. Queue Persistence Integration
**Priority:** High  
**File:** `synap-server/src/server/handlers.rs:1540`

**Current Status:**
- Queue persistence layer exists (`synap-server/src/persistence/queue_persistence.rs`)
- Queue operations are NOT currently persisted to WAL
- Queue persistence infrastructure is ready but not integrated

**Implementation Required:**
1. Integrate `QueuePersistence` into `AppState` or `PersistenceLayer`
2. Call `persistence.log_queue_publish()` in queue publish handler
3. Call `persistence.log_queue_ack()` in queue ACK handler
4. Call `persistence.log_queue_nack()` in queue NACK handler
5. Update recovery logic to replay queue operations from WAL

**Related Code:**
- `synap-server/src/persistence/queue_persistence.rs` - Queue persistence implementation
- `synap-server/src/persistence/types.rs:76-90` - Queue operations in WAL
- `synap-server/src/server/handlers.rs:1537` - Queue publish handler

---

## Client Tracking & Monitoring

### 3. WebSocket Client Tracking
**Priority:** Medium  
**Files:**
- `synap-server/src/server/handlers.rs:858`
- `synap-server/src/server/handlers.rs:6305`

**Current Status:**
- `ClientListManager` exists (`synap-server/src/monitoring/client_list.rs`)
- WebSocket handlers exist for PubSub, Queue, and Stream
- Client tracking is NOT implemented for WebSocket connections
- REST endpoint `client_list()` returns empty array

**Implementation Required:**
1. Track WebSocket connections when they connect:
   - `handle_pubsub_socket()` - PubSub WebSocket
   - `handle_queue_socket()` - Queue WebSocket
   - `handle_stream_socket()` - Stream WebSocket
2. Register clients in `ClientListManager` on connection
3. Unregister clients on disconnect
4. Track client activity (last command, idle time)
5. Update `client_list()` handler to return actual client list
6. Update `handle_client_list_cmd()` StreamableHTTP handler

**Related Code:**
- `synap-server/src/monitoring/client_list.rs` - Client tracking infrastructure
- `synap-server/src/server/handlers.rs:5394-5800` - WebSocket handlers
- `synap-server/src/core/pubsub.rs:124` - PubSub connection registration

---

## Replication

### 4. TTL Support in Replication Sync
**Priority:** Medium  
**File:** `synap-server/src/replication/sync.rs:48`

**Current Status:**
- Replication sync works for KV operations
- TTL is NOT included when syncing keys
- Keys with TTL may lose expiration time during replication

**Implementation Required:**
1. Get TTL from source key before syncing
2. Include TTL in `Operation::KVSet` when creating sync operations
3. Ensure TTL is preserved during replication

**Related Code:**
- `synap-server/src/replication/sync.rs:45` - Sync operation creation
- `synap-server/src/core/kv_store.rs` - TTL management

---

### 5. Replication Lag Calculation
**Priority:** Low  
**Files:**
- `synap-server/src/replication/master.rs:436`
- `synap-server/src/replication/master.rs:457`

**Current Status:**
- Replication stats exist but lag calculation is hardcoded to 0
- Heartbeat timestamps are tracked but not used for lag calculation

**Implementation Required:**
1. Calculate lag from heartbeat timestamps:
   - Compare replica's last heartbeat with master's current time
   - Store lag in milliseconds
2. Calculate lag from operation timestamps:
   - Track when operations are sent to replica
   - Compare with replica's confirmed offset timestamp
3. Update `ReplicationStats` to include accurate lag metrics

**Related Code:**
- `synap-server/src/replication/master.rs:433` - Replica list with lag
- `synap-server/src/replication/master.rs:441` - Replication stats

---

### 6. Replication Byte Tracking
**Priority:** Low  
**File:** `synap-server/src/replication/master.rs:459`

**Current Status:**
- Total replicated operations are tracked
- Total bytes replicated is hardcoded to 0

**Implementation Required:**
1. Track size of each replicated operation
2. Accumulate total bytes sent to replicas
3. Update `ReplicationStats.total_bytes` with actual value

**Related Code:**
- `synap-server/src/replication/master.rs:441` - Replication stats structure

---

## SDK Features

### 7. Reactive Subscription for PubSub (Rust SDK)
**Priority:** Low  
**File:** `sdks/rust/README.md:264`

**Current Status:**
- Reactive patterns exist for Queue (`observe_messages`) and Stream (`observe_events`)
- PubSub reactive subscription is NOT implemented
- Comment indicates "coming soon"

**Implementation Required:**
1. Implement `observe()` method for PubSub similar to Queue/Stream
2. Return `Observable<Message>` or similar reactive type
3. Handle subscription lifecycle (subscribe/unsubscribe)
4. Update documentation with examples

**Related Code:**
- `sdks/rust/src/pubsub.rs` - PubSub implementation
- `sdks/rust/src/queue_reactive.rs` - Queue reactive pattern (reference)
- `sdks/rust/src/stream_reactive.rs` - Stream reactive pattern (reference)

---

## Summary

### By Priority

**High Priority:**
- Queue Persistence Integration (affects data durability)

**Medium Priority:**
- RENAME Operation WAL Logging (affects persistence correctness)
- WebSocket Client Tracking (affects monitoring)
- TTL Support in Replication (affects replication correctness)

**Low Priority:**
- Replication Lag Calculation (monitoring/metrics)
- Replication Byte Tracking (monitoring/metrics)
- Reactive Subscription for PubSub (SDK feature)

### By Category

**Persistence:** 2 tasks
- RENAME WAL logging
- Queue persistence integration

**Monitoring:** 3 tasks
- WebSocket client tracking
- Replication lag calculation
- Replication byte tracking

**Replication:** 2 tasks
- TTL support in sync
- Lag and byte tracking

**SDK:** 1 task
- Reactive PubSub subscription

---

## Notes

- All TODOs are marked with `// TODO:` comments in the code
- Some tasks may have dependencies (e.g., WebSocket tracking needed before client tracking)
- Queue persistence infrastructure exists but needs integration
- Most tasks are incremental improvements, not critical blockers

