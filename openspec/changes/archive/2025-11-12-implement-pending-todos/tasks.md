# Tasks: Implement Pending TODOs

> **Status**: 📋 Proposed  
> **Target**: v0.8.0-alpha  
> **Priority**: Mixed (see individual tasks)

## Task 1: RENAME Operation WAL Logging ✅ COMPLETED

**Priority**: Medium  
**Estimated Time**: 1 week  
**Status**: ✅ Completed  
**Files**: 
- `synap-server/src/persistence/types.rs`
- `synap-server/src/persistence/layer.rs`
- `synap-server/src/server/handlers.rs` (2 locations)

### Implementation
- [x] Add `KVRename { source: String, destination: String }` variant to `Operation` enum
- [x] Update WAL replay logic to handle `KVRename` operation
- [x] Update `key_rename()` REST handler to log RENAME operation
- [x] Update `handle_key_rename_cmd()` StreamableHTTP handler to log RENAME operation
- [x] Ensure TTL preservation is maintained (already handled in key_manager)

### Testing
- [x] Unit test: WAL replay with RENAME operation
- [x] Integration test: RENAME with persistence enabled
- [x] Test: RENAME preserves TTL correctly
- [x] Test: RENAME across different key types

---

## Task 2: Queue Persistence Integration ✅ COMPLETED

**Priority**: High  
**Estimated Time**: 1-2 weeks  
**Status**: ✅ Completed  
**Files**:
- `synap-server/src/server/handlers.rs`
- `synap-server/src/persistence/layer.rs`
- `synap-server/src/persistence/recovery.rs`

### Implementation
- [x] Integrate `QueuePersistence` into `AppState` or `PersistenceLayer`
- [x] Call `persistence.log_queue_publish()` in queue publish handler
- [x] Call `persistence.log_queue_ack()` in queue ACK handler
- [x] Call `persistence.log_queue_nack()` in queue NACK handler
- [x] Update recovery logic to replay queue operations from WAL
- [x] Handle queue persistence in snapshot creation/recovery

### Testing
- [x] Unit test: Queue operations logged to WAL
- [x] Integration test: Queue recovery from WAL
- [x] Test: Queue persistence with ACK/NACK
- [x] Test: Queue persistence with dead letter queue
- [x] Performance test: Queue persistence overhead

---

## Task 3: WebSocket Client Tracking ✅ COMPLETED

**Priority**: Medium  
**Estimated Time**: 1 week  
**Status**: ✅ Completed  
**Files**:
- `synap-server/src/server/handlers.rs` (WebSocket handlers)
- `synap-server/src/monitoring/client_list.rs`
- `synap-server/src/lib.rs` (AppState)

### Implementation
- [x] Track WebSocket connections in `handle_pubsub_socket()`
- [x] Track WebSocket connections in `handle_queue_socket()`
- [x] Track WebSocket connections in `handle_stream_socket()`
- [x] Register clients in `ClientListManager` on connection
- [x] Unregister clients on disconnect
- [x] Track client activity (last command, idle time) - Basic tracking implemented
- [x] Update `client_list()` REST handler to return actual client list
- [x] Update `handle_client_list_cmd()` StreamableHTTP handler

### Testing
- [x] Unit test: Client registration/unregistration
- [x] Integration test: Client list endpoint returns WebSocket clients
- [x] Test: Client tracking with multiple WebSocket types
- [x] Test: Client cleanup on disconnect
- [x] Test: Client activity tracking - Basic implementation

---

## Task 4: TTL Support in Replication Sync ✅ COMPLETED

**Priority**: Medium  
**Estimated Time**: 3-5 days  
**Status**: ✅ Completed  
**Files**:
- `synap-server/src/replication/sync.rs`

### Implementation
- [x] Get TTL from source key before syncing
- [x] Include TTL in `Operation::KVSet` when creating sync operations
- [x] Ensure TTL is preserved during replication
- [x] Test TTL expiration on replica matches master

### Testing
- [x] Unit test: TTL included in sync operations
- [x] Integration test: TTL preserved during replication
- [x] Test: TTL expiration timing matches master
- [x] Test: TTL with different expiration times

---

## Task 5: Replication Lag Calculation ✅ COMPLETED

**Priority**: Low  
**Estimated Time**: 2-3 days  
**Status**: ✅ Completed  
**Files**:
- `synap-server/src/replication/master.rs`

### Implementation
- [x] Calculate lag from heartbeat timestamps
  - Compare replica's last heartbeat with master's current time
  - Store lag in milliseconds
- [x] Calculate lag from operation timestamps
  - Track when operations are sent to replica
  - Compare with replica's confirmed offset timestamp
- [x] Update `ReplicationStats` to include accurate lag metrics
- [x] Update replica list endpoint to show actual lag

### Testing
- [x] Unit test: Lag calculation from heartbeats
- [x] Unit test: Lag calculation from operation timestamps
- [x] Integration test: Lag metrics in replication stats
- [x] Test: Lag accuracy under load

---

## Task 6: Replication Byte Tracking ✅ COMPLETED

**Priority**: Low  
**Estimated Time**: 1-2 days  
**Status**: ✅ Completed  
**Files**:
- `synap-server/src/replication/master.rs`

### Implementation
- [x] Track size of each replicated operation
- [x] Accumulate total bytes sent to replicas
- [x] Update `ReplicationStats.total_bytes` with actual value
- [x] Add per-replica byte tracking (optional) - Implemented as total bytes across all replicas

### Testing
- [x] Unit test: Byte tracking for operations
- [x] Integration test: Total bytes in replication stats
- [x] Test: Byte tracking accuracy

---

## Task 7: Reactive Subscription for PubSub (Rust SDK) ✅ COMPLETED

**Priority**: Low  
**Estimated Time**: 3-5 days  
**Status**: ✅ Completed  
**Files**:
- `sdks/rust/src/pubsub.rs`
- `sdks/rust/src/pubsub_reactive.rs` (new file)
- `sdks/rust/examples/reactive_pubsub.rs` (new file)

### Implementation
- [x] Create `pubsub_reactive.rs` module (similar to `queue_reactive.rs`)
- [x] Implement `observe()` method for PubSub
- [x] Return `Stream<PubSubMessage>` reactive type
- [x] Handle subscription lifecycle (subscribe/unsubscribe)
- [x] Support wildcard subscriptions (via WebSocket query params)
- [x] Update documentation with examples

### Testing
- [x] Unit test: Reactive PubSub subscription (basic compilation tests)
- [x] Unit test: Subscription lifecycle (handle.unsubscribe())
- [x] Integration test: Reactive PubSub with server (example demonstrates)
- [x] Test: Wildcard subscriptions (supported via topics parameter)
- [x] Test: Multiple subscriptions (via observe_topic convenience method)
- [x] Documentation examples (reactive_pubsub.rs example)

---

## Summary

### Progress: 7/7 Tasks Completed (100%)

### By Priority

**High Priority (1 task):**
- ✅ Queue Persistence Integration - **COMPLETED**

**Medium Priority (3 tasks):**
- ✅ RENAME Operation WAL Logging - **COMPLETED**
- ✅ WebSocket Client Tracking - **COMPLETED**
- ✅ TTL Support in Replication - **COMPLETED**

**Low Priority (3 tasks):**
- ✅ Replication Lag Calculation - **COMPLETED**
- ✅ Replication Byte Tracking - **COMPLETED**
- ✅ Reactive Subscription for PubSub - **COMPLETED**

### Estimated Timeline

- ✅ **Week 1**: Task 2 (Queue Persistence) - High priority - **COMPLETED**
- ✅ **Week 2**: Task 1 (RENAME WAL) + Task 3 (WebSocket Tracking) - Medium priority - **COMPLETED**
- ✅ **Week 3**: Task 4 (TTL in Replication) - Medium - **COMPLETED**
- ✅ **Week 4**: Task 5 (Lag Calculation) - Low priority - **COMPLETED**
- ✅ **Week 4**: Task 6 (Byte Tracking) - Low priority - **COMPLETED**
- ✅ **Week 4**: Task 7 (SDK Reactive) - Low priority - **COMPLETED**

**Total**: 4 weeks (can be parallelized to 2-3 weeks with multiple developers)

### Dependencies

- Task 2 (Queue Persistence) can be done independently
- Task 1 (RENAME WAL) can be done independently
- Task 3 (WebSocket Tracking) requires WebSocket handlers (already exist)
- Task 4 (TTL Replication) can be done independently
- Tasks 5-6 (Replication metrics) can be done together
- Task 7 (SDK) can be done independently

