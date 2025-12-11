# Implement Pending TODOs

> **Status**: 📋 Proposed  
> **Priority**: Mixed (High/Medium/Low)  
> **Target**: v0.8.0-alpha  
> **Duration**: 4-6 weeks (depending on priority)

## Why

Several TODO comments in the codebase indicate incomplete features or missing integrations:
- Queue persistence infrastructure exists but is not integrated
- RENAME operations are not properly logged to WAL
- WebSocket client tracking is not implemented
- Replication metrics are incomplete
- SDK features are missing

These gaps affect data durability, monitoring capabilities, and SDK completeness.

## What Changes

Implement 7 pending tasks across 4 categories:

### 1. Persistence & WAL (2 tasks)
- **RENAME Operation WAL Logging**: Add dedicated `KVRename` operation to WAL
- **Queue Persistence Integration**: Integrate existing queue persistence layer

### 2. Client Tracking & Monitoring (1 task)
- **WebSocket Client Tracking**: Track WebSocket connections for monitoring

### 3. Replication (3 tasks)
- **TTL Support in Replication Sync**: Include TTL when syncing keys
- **Replication Lag Calculation**: Calculate actual lag from timestamps
- **Replication Byte Tracking**: Track bytes replicated (currently hardcoded to 0)

### 4. SDK Features (1 task)
- **Reactive Subscription for PubSub**: Add reactive pattern to Rust SDK

## Impact

**NEW**: Enhanced persistence, monitoring, and SDK features  
**MODIFIED**: 
- `synap-server/src/persistence/types.rs` - Add KVRename operation
- `synap-server/src/server/handlers.rs` - Multiple handler updates
- `synap-server/src/replication/master.rs` - Lag and byte tracking
- `sdks/rust/src/pubsub.rs` - Reactive subscription

**Complexity**: 
- High: Queue persistence integration
- Medium: RENAME WAL, WebSocket tracking, TTL in replication
- Low: Lag/byte tracking, SDK reactive pattern

**Performance**: Minimal impact (mostly additive features)

## Dependencies

- Queue persistence infrastructure already exists
- ClientListManager already exists
- WebSocket handlers already exist
- Reactive patterns already exist for Queue/Stream

## Testing Requirements

- Unit tests for each new feature
- Integration tests for persistence and replication
- SDK tests for reactive PubSub
- 95%+ coverage maintained

## Breaking Changes

None - all changes are additive or internal improvements.

