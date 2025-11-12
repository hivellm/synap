# Tasks: Implement Pending TODOs

> **Status**: 📋 Proposed  
> **Target**: v0.8.0-alpha  
> **Priority**: Mixed (see individual tasks)

## Task 1: RENAME Operation WAL Logging

**Priority**: Medium  
**Estimated Time**: 1 week  
**Files**: 
- `synap-server/src/persistence/types.rs`
- `synap-server/src/persistence/layer.rs`
- `synap-server/src/server/handlers.rs` (2 locations)

### Implementation
- [ ] Add `KVRename { source: String, destination: String }` variant to `Operation` enum
- [ ] Update WAL replay logic to handle `KVRename` operation
- [ ] Update `key_rename()` REST handler to log RENAME operation
- [ ] Update `handle_key_rename_cmd()` StreamableHTTP handler to log RENAME operation
- [ ] Ensure TTL preservation is maintained (already handled in key_manager)

### Testing
- [ ] Unit test: WAL replay with RENAME operation
- [ ] Integration test: RENAME with persistence enabled
- [ ] Test: RENAME preserves TTL correctly
- [ ] Test: RENAME across different key types

---

## Task 2: Queue Persistence Integration

**Priority**: High  
**Estimated Time**: 1-2 weeks  
**Files**:
- `synap-server/src/server/handlers.rs`
- `synap-server/src/persistence/layer.rs`
- `synap-server/src/persistence/recovery.rs`

### Implementation
- [ ] Integrate `QueuePersistence` into `AppState` or `PersistenceLayer`
- [ ] Call `persistence.log_queue_publish()` in queue publish handler
- [ ] Call `persistence.log_queue_ack()` in queue ACK handler
- [ ] Call `persistence.log_queue_nack()` in queue NACK handler
- [ ] Update recovery logic to replay queue operations from WAL
- [ ] Handle queue persistence in snapshot creation/recovery

### Testing
- [ ] Unit test: Queue operations logged to WAL
- [ ] Integration test: Queue recovery from WAL
- [ ] Test: Queue persistence with ACK/NACK
- [ ] Test: Queue persistence with dead letter queue
- [ ] Performance test: Queue persistence overhead

---

## Task 3: WebSocket Client Tracking

**Priority**: Medium  
**Estimated Time**: 1 week  
**Files**:
- `synap-server/src/server/handlers.rs` (WebSocket handlers)
- `synap-server/src/monitoring/client_list.rs`
- `synap-server/src/lib.rs` (AppState)

### Implementation
- [ ] Track WebSocket connections in `handle_pubsub_socket()`
- [ ] Track WebSocket connections in `handle_queue_socket()`
- [ ] Track WebSocket connections in `handle_stream_socket()`
- [ ] Register clients in `ClientListManager` on connection
- [ ] Unregister clients on disconnect
- [ ] Track client activity (last command, idle time)
- [ ] Update `client_list()` REST handler to return actual client list
- [ ] Update `handle_client_list_cmd()` StreamableHTTP handler

### Testing
- [ ] Unit test: Client registration/unregistration
- [ ] Integration test: Client list endpoint returns WebSocket clients
- [ ] Test: Client tracking with multiple WebSocket types
- [ ] Test: Client cleanup on disconnect
- [ ] Test: Client activity tracking

---

## Task 4: TTL Support in Replication Sync

**Priority**: Medium  
**Estimated Time**: 3-5 days  
**Files**:
- `synap-server/src/replication/sync.rs`

### Implementation
- [ ] Get TTL from source key before syncing
- [ ] Include TTL in `Operation::KVSet` when creating sync operations
- [ ] Ensure TTL is preserved during replication
- [ ] Test TTL expiration on replica matches master

### Testing
- [ ] Unit test: TTL included in sync operations
- [ ] Integration test: TTL preserved during replication
- [ ] Test: TTL expiration timing matches master
- [ ] Test: TTL with different expiration times

---

## Task 5: Replication Lag Calculation

**Priority**: Low  
**Estimated Time**: 2-3 days  
**Files**:
- `synap-server/src/replication/master.rs`

### Implementation
- [ ] Calculate lag from heartbeat timestamps
  - Compare replica's last heartbeat with master's current time
  - Store lag in milliseconds
- [ ] Calculate lag from operation timestamps
  - Track when operations are sent to replica
  - Compare with replica's confirmed offset timestamp
- [ ] Update `ReplicationStats` to include accurate lag metrics
- [ ] Update replica list endpoint to show actual lag

### Testing
- [ ] Unit test: Lag calculation from heartbeats
- [ ] Unit test: Lag calculation from operation timestamps
- [ ] Integration test: Lag metrics in replication stats
- [ ] Test: Lag accuracy under load

---

## Task 6: Replication Byte Tracking

**Priority**: Low  
**Estimated Time**: 1-2 days  
**Files**:
- `synap-server/src/replication/master.rs`

### Implementation
- [ ] Track size of each replicated operation
- [ ] Accumulate total bytes sent to replicas
- [ ] Update `ReplicationStats.total_bytes` with actual value
- [ ] Add per-replica byte tracking (optional)

### Testing
- [ ] Unit test: Byte tracking for operations
- [ ] Integration test: Total bytes in replication stats
- [ ] Test: Byte tracking accuracy

---

## Task 7: Reactive Subscription for PubSub (Rust SDK)

**Priority**: Low  
**Estimated Time**: 3-5 days  
**Files**:
- `sdks/rust/src/pubsub.rs`
- `sdks/rust/src/pubsub_reactive.rs` (new file)

### Implementation
- [ ] Create `pubsub_reactive.rs` module (similar to `queue_reactive.rs`)
- [ ] Implement `observe()` method for PubSub
- [ ] Return `Observable<Message>` or similar reactive type
- [ ] Handle subscription lifecycle (subscribe/unsubscribe)
- [ ] Support wildcard subscriptions
- [ ] Update documentation with examples

### Testing
- [ ] Unit test: Reactive PubSub subscription
- [ ] Unit test: Subscription lifecycle
- [ ] Integration test: Reactive PubSub with server
- [ ] Test: Wildcard subscriptions
- [ ] Test: Multiple subscriptions
- [ ] Documentation examples

---

## Summary

### By Priority

**High Priority (1 task):**
- Queue Persistence Integration

**Medium Priority (3 tasks):**
- RENAME Operation WAL Logging
- WebSocket Client Tracking
- TTL Support in Replication

**Low Priority (3 tasks):**
- Replication Lag Calculation
- Replication Byte Tracking
- Reactive Subscription for PubSub

### Estimated Timeline

- **Week 1**: Task 2 (Queue Persistence) - High priority
- **Week 2**: Task 1 (RENAME WAL) + Task 3 (WebSocket Tracking) - Medium priority
- **Week 3**: Task 4 (TTL in Replication) + Task 5 (Lag Calculation) - Medium/Low
- **Week 4**: Task 6 (Byte Tracking) + Task 7 (SDK Reactive) - Low priority

**Total**: 4 weeks (can be parallelized to 2-3 weeks with multiple developers)

### Dependencies

- Task 2 (Queue Persistence) can be done independently
- Task 1 (RENAME WAL) can be done independently
- Task 3 (WebSocket Tracking) requires WebSocket handlers (already exist)
- Task 4 (TTL Replication) can be done independently
- Tasks 5-6 (Replication metrics) can be done together
- Task 7 (SDK) can be done independently

