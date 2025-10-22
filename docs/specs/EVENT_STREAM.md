# Event Stream Specification

## Overview

The Synap Event Stream system provides Kafka-style event streaming with two operational modes:

1. **Simple Room-Based Streaming**: Room-based broadcasting where all subscribers receive all events in real-time with message history and replay capabilities.
2. **Partitioned Streaming** (Kafka-style): Full Kafka-compatible partitioned topics with consumer groups, partition assignment strategies, and advanced retention policies.

## Core Concepts

### Rooms
- Isolated event streams (similar to Kafka topics)
- Each room maintains its own event history
- Subscribers receive all events published to the room
- Examples: `chat-room-1`, `game-lobby-42`, `notifications-user-123`

### Events
- Immutable messages with sequential offsets
- Stored in ring buffer with configurable retention
- Can be replayed from any offset
- Include event type, data, and metadata

### Subscribers
- Clients connected to room via WebSocket
- Receive events in real-time as they're published
- Can request historical events from specific offset
- Multiple subscribers per room supported

## Data Structure

```rust
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use parking_lot::RwLock;

pub struct EventStreamManager {
    rooms: Arc<RwLock<HashMap<RoomId, Room>>>,
    config: StreamConfig,
}

pub struct Room {
    pub id: RoomId,
    pub events: RingBuffer<Event>,
    pub subscribers: HashSet<SubscriberId>,
    pub offset: AtomicU64,           // Current offset
    pub created_at: Instant,
    pub last_event_at: Option<Instant>,
    pub retention: Duration,
    pub max_events: usize,
}

pub struct Event {
    pub id: EventId,
    pub offset: u64,
    pub room: RoomId,
    pub event_type: String,
    pub data: Vec<u8>,
    pub timestamp: Instant,
    pub metadata: HashMap<String, String>,
}

pub struct RingBuffer<T> {
    buffer: VecDeque<T>,
    capacity: usize,
    oldest_offset: u64,
    newest_offset: u64,
}

pub struct Subscriber {
    pub id: SubscriberId,
    pub room: RoomId,
    pub connection: WebSocketConnection,
    pub current_offset: u64,
    pub subscribed_at: Instant,
}
```

## Event Lifecycle

```
PUBLISH Event
    │
    ▼
Generate Offset (atomic increment)
    │
    ▼
Add to Room RingBuffer
    │
    ▼
Broadcast to All Subscribers
    │
    ├─→ Subscriber 1 (WebSocket push)
    ├─→ Subscriber 2 (WebSocket push)
    └─→ Subscriber N (WebSocket push)
    │
    ▼
Replicate to Slave Nodes
    │
    ▼
Cleanup Old Events (retention policy)
```

## Operations Specification

### PUBLISH Event

**Command**: `stream.publish`

**Parameters**:
- `room` (string, required): Room identifier
- `event_type` (string, required): Event type name
- `data` (any, required): Event payload
- `metadata` (object, optional): Additional metadata

**Returns**:
- `event_id` (string): Unique event identifier
- `offset` (integer): Event offset in room
- `subscribers_notified` (integer): Number of active subscribers

**Example**:
```json
{
  "command": "stream.publish",
  "payload": {
    "room": "chat-room-1",
    "event_type": "message",
    "data": {
      "user": "alice",
      "text": "Hello everyone!",
      "timestamp": "2025-10-15T19:30:00Z"
    },
    "metadata": {
      "client_version": "1.0.0"
    }
  }
}
```

**Response**:
```json
{
  "event_id": "evt_abc123",
  "offset": 42,
  "subscribers_notified": 5
}
```

### SUBSCRIBE to Room

**Command**: `stream.subscribe`

**Protocol**: Requires WebSocket connection

**Parameters**:
- `room` (string, required): Room to subscribe to
- `from_offset` (integer, optional): Start from specific offset
- `replay` (boolean, optional): Replay historical events

**Returns**: Stream of events via WebSocket

**Example Request**:
```json
{
  "command": "stream.subscribe",
  "payload": {
    "room": "chat-room-1",
    "from_offset": 35,
    "replay": true
  }
}
```

**Event Stream (WebSocket)**:
```json
{"event_id": "evt_001", "offset": 35, "type": "message", "data": {...}}
{"event_id": "evt_002", "offset": 36, "type": "message", "data": {...}}
{"event_id": "evt_003", "offset": 37, "type": "join", "data": {...}}
```

### UNSUBSCRIBE from Room

**Command**: `stream.unsubscribe`

**Parameters**:
- `room` (string, required): Room to unsubscribe from

**Returns**:
- `success` (boolean): Operation result

### GET Event History

**Command**: `stream.history`

**Parameters**:
- `room` (string, required): Room identifier
- `from_offset` (integer, optional): Start offset (default: oldest)
- `to_offset` (integer, optional): End offset (default: newest)
- `limit` (integer, optional): Max events (default: 100)

**Returns**:
- `events` (array): Array of events
- `oldest_offset` (integer): Oldest available offset
- `newest_offset` (integer): Newest offset in room

**Example**:
```json
{
  "command": "stream.history",
  "payload": {
    "room": "chat-room-1",
    "from_offset": 30,
    "limit": 10
  }
}
```

**Response**:
```json
{
  "events": [
    {"offset": 30, "type": "message", "data": {...}},
    {"offset": 31, "type": "join", "data": {...}},
    ...
  ],
  "oldest_offset": 1,
  "newest_offset": 42
}
```

## Ring Buffer Implementation

### Bounded Retention

```rust
impl<T> RingBuffer<T> {
    pub fn push(&mut self, item: T) -> u64 {
        if self.buffer.len() >= self.capacity {
            self.buffer.pop_front();
            self.oldest_offset += 1;
        }
        
        self.newest_offset += 1;
        self.buffer.push_back(item);
        self.newest_offset
    }
    
    pub fn get_range(&self, from: u64, to: u64) -> Vec<&T> {
        let start_idx = self.offset_to_index(from);
        let end_idx = self.offset_to_index(to);
        
        self.buffer.range(start_idx..=end_idx).collect()
    }
}
```

### Retention Policies

**Time-based**:
```yaml
stream:
  retention_mode: "time"
  retention_hours: 24  # Keep last 24 hours
```

**Count-based**:
```yaml
stream:
  retention_mode: "count"
  max_events_per_room: 10000  # Keep last 10K events
```

**Hybrid**:
```yaml
stream:
  retention_mode: "hybrid"
  retention_hours: 24
  max_events_per_room: 100000  # Whichever limit reached first
```

## Event Replay

### Replay from Offset

New subscribers can start from:
1. **Latest**: Only new events (`from_offset: null`)
2. **Beginning**: All available history (`from_offset: 0`)
3. **Specific**: From specific offset (`from_offset: 100`)
4. **Relative**: From N events ago (`from_offset: -100`)

**Example - New Chat User**:
```json
{
  "command": "stream.subscribe",
  "payload": {
    "room": "chat-room-1",
    "from_offset": -50,  // Last 50 messages
    "replay": true
  }
}
```

## Broadcasting Semantics

### At-Least-Once Delivery

For connected subscribers:
- Events delivered in order
- If subscriber disconnects, missed events available via history
- Offset tracking allows resuming from last received

### Event Ordering
- Events within a room are totally ordered
- Cross-room events have no ordering guarantee
- Offset is room-local monotonic counter

## Subscriber Management

### Connection Lifecycle

```
Client connects via WebSocket
    │
    ▼
SUBSCRIBE to room
    │
    ├─→ Add to room.subscribers
    │
    └─→ Send historical events (if requested)
    │
    ▼
Receive events in real-time
    │
    ▼
UNSUBSCRIBE or disconnect
    │
    └─→ Remove from room.subscribers
```

### Auto-cleanup

```rust
impl Room {
    async fn cleanup_inactive_subscribers(&mut self) {
        let timeout = Duration::from_secs(30);
        let now = Instant::now();
        
        self.subscribers.retain(|sub_id| {
            if let Some(sub) = get_subscriber(sub_id) {
                now.duration_since(sub.last_ping) < timeout
            } else {
                false
            }
        });
    }
}
```

## Room Management

### AUTO-CREATE Rooms

Rooms are created automatically on first publish or subscribe:

```rust
impl EventStreamManager {
    pub async fn get_or_create_room(&self, room_id: &str) -> Arc<RwLock<Room>> {
        let mut rooms = self.rooms.write();
        
        rooms.entry(room_id.to_string())
            .or_insert_with(|| Room::new(room_id, &self.config))
            .clone()
    }
}
```

### Room Cleanup

Inactive rooms (no subscribers, no events recently) are removed:

```yaml
stream:
  room_inactive_timeout_hours: 24
  room_cleanup_interval_mins: 60
```

## Performance Optimization

### Subscriber Notification

**Broadcast Strategy**:
```rust
impl Room {
    async fn broadcast_event(&self, event: &Event) {
        // Parallel notification to all subscribers
        let notifications: Vec<_> = self.subscribers
            .iter()
            .map(|sub_id| {
                let event = event.clone();
                tokio::spawn(async move {
                    notify_subscriber(sub_id, event).await
                })
            })
            .collect();
        
        // Wait for all notifications
        futures::future::join_all(notifications).await;
    }
}
```

### Memory Management

**Ring Buffer Tuning**:
- Small rooms (< 100 events/min): 1K capacity
- Medium rooms (< 1K events/min): 10K capacity
- Large rooms (> 1K events/min): 100K capacity

## Error Conditions

```rust
pub enum StreamError {
    RoomNotFound(String),
    EventNotFound(u64),
    OffsetOutOfRange { requested: u64, oldest: u64, newest: u64 },
    SubscriptionFailed(String),
    BroadcastFailed(String),
}
```

## Configuration

```yaml
event_stream:
  enabled: true
  retention_mode: "hybrid"
  retention_hours: 24
  max_events_per_room: 100000
  max_rooms: 100000
  room_cleanup_interval_mins: 60
  room_inactive_timeout_hours: 24
  max_subscribers_per_room: 10000
  broadcast_timeout_ms: 1000
```

## Testing Requirements

### Unit Tests
- Ring buffer operations
- Offset calculation
- Event ordering
- Retention policies

### Integration Tests
- Multi-subscriber broadcasting
- Event replay
- Room lifecycle
- Offset-based consumption
- Concurrent publish/subscribe

### Performance Tests
- Broadcast latency (target: < 1ms)
- Throughput (target: 10K events/sec)
- Memory usage with 1K+ rooms
- Subscriber scaling (100+ per room)

## Example Use Cases

### Real-Time Chat
```typescript
// Join chat room
await client.stream.subscribe('chat-room-1', {
  fromOffset: -50,  // Get last 50 messages
  replay: true
});

// Publish message
await client.stream.publish('chat-room-1', 'message', {
  user: 'alice',
  text: 'Hello!',
  timestamp: new Date()
});
```

### Live Game State
```python
# Subscribe to game events
client.stream.subscribe('game-lobby-42', 
    from_offset=0,  # Full history
    replay=True
)

# Publish game action
client.stream.publish('game-lobby-42', 'player_move', {
    'player': 'player-1',
    'action': 'attack',
    'target': 'enemy-5'
})
```

### System Monitoring
```rust
// Subscribe to system events
client.stream_subscribe("system-events", None, false).await?;

// Events automatically pushed
loop {
    let event = subscriber.next_event().await?;
    println!("System event: {:?}", event);
}
```

---

# Partitioned Event Streaming (Kafka-Style)

## Overview

The Partitioned Event Streaming system provides full Kafka-compatible functionality including:

- **Partitioned Topics**: Events distributed across multiple partitions
- **Consumer Groups**: Coordinated consumption with partition assignment
- **Advanced Retention**: Time, size, count, and combined retention policies
- **Key-Based Routing**: Consistent partition routing using message keys
- **Offset Management**: Commit/checkpoint consumer positions

## Partitioned Topics

### Topic Creation

**REST API**:
```http
POST /topics/:topic_name
Content-Type: application/json

{
  "num_partitions": 3,
  "replication_factor": 1,
  "retention_policy": {
    "type": "Combined",
    "retention_secs": 3600,
    "max_bytes": 104857600,
    "max_messages": 100000
  },
  "segment_bytes": 104857600
}
```

**Response**:
```json
{
  "success": true,
  "topic": "orders",
  "num_partitions": 3,
  "replication_factor": 1
}
```

### Retention Policies

#### Time-Based Retention
```json
{
  "type": "Time",
  "retention_secs": 3600  // 1 hour
}
```

#### Size-Based Retention
```json
{
  "type": "Size",
  "max_bytes": 104857600  // 100 MB
}
```

#### Count-Based Retention
```json
{
  "type": "Messages",
  "max_messages": 10000
}
```

#### Combined Retention
```json
{
  "type": "Combined",
  "retention_secs": 3600,
  "max_bytes": 104857600,
  "max_messages": 100000
}
```
*Note: The smallest limit wins (time, size, or count)*

#### Infinite Retention
```json
{
  "type": "Infinite"
}
```

## Publishing to Topics

### With Partition Key (Hash-Based Routing)
```http
POST /topics/orders/publish
Content-Type: application/json

{
  "event_type": "order.created",
  "key": "customer-123",  // Same key always goes to same partition
  "data": {
    "order_id": "ORD-456",
    "customer": "customer-123",
    "amount": 99.99
  }
}
```

**Response**:
```json
{
  "partition_id": 2,
  "offset": 1234,
  "topic": "orders"
}
```

### Without Key (Round-Robin)
```http
POST /topics/events/publish
Content-Type: application/json

{
  "event_type": "page.view",
  "data": {
    "page": "/home",
    "timestamp": "2025-10-22T12:00:00Z"
  }
}
```

## Consuming from Topics

### Direct Partition Consumption
```http
POST /topics/orders/partitions/2/consume
Content-Type: application/json

{
  "from_offset": 1000,
  "limit": 100
}
```

**Response**:
```json
{
  "topic": "orders",
  "partition_id": 2,
  "events": [
    {
      "id": "evt-001",
      "partition_id": 2,
      "offset": 1000,
      "topic": "orders",
      "event_type": "order.created",
      "key": "customer-123",
      "data": {...},
      "timestamp": 1729598400,
      "size_bytes": 256
    }
  ],
  "next_offset": 1001,
  "count": 1
}
```

## Consumer Groups

### Creating Consumer Groups

```http
POST /consumer-groups/order-processors
Content-Type: application/json

{
  "topic": "orders",
  "partition_count": 3,
  "strategy": "round_robin",  // or "range", "sticky"
  "session_timeout_secs": 30
}
```

**Response**:
```json
{
  "success": true,
  "group_id": "order-processors",
  "topic": "orders"
}
```

### Assignment Strategies

#### Round-Robin
Distributes partitions evenly in round-robin fashion:
```
Partitions: [0, 1, 2, 3, 4, 5]
Consumer 1: [0, 3]
Consumer 2: [1, 4]
Consumer 3: [2, 5]
```

#### Range
Assigns contiguous ranges of partitions:
```
Partitions: [0, 1, 2, 3, 4, 5]
Consumer 1: [0, 1]
Consumer 2: [2, 3]
Consumer 3: [4, 5]
```

#### Sticky
Minimizes partition movement on rebalance (retains existing assignments when possible).

### Joining Consumer Group

```http
POST /consumer-groups/order-processors/join
Content-Type: application/json

{
  "session_timeout_secs": 30
}
```

**Response**:
```json
{
  "member_id": "550e8400-e29b-41d4-a716-446655440000",
  "group_id": "order-processors"
}
```

### Getting Partition Assignment

```http
GET /consumer-groups/order-processors/members/{member_id}/assignment
```

**Response**:
```json
{
  "member_id": "550e8400-e29b-41d4-a716-446655440000",
  "group_id": "order-processors",
  "partitions": [0, 3]
}
```

### Heartbeat

Keep consumer alive in group:
```http
POST /consumer-groups/order-processors/members/{member_id}/heartbeat
```

**Response**:
```json
{
  "success": true
}
```

### Committing Offsets

```http
POST /consumer-groups/order-processors/offsets/commit
Content-Type: application/json

{
  "partition_id": 0,
  "offset": 1500
}
```

**Response**:
```json
{
  "success": true,
  "partition_id": 0,
  "offset": 1500
}
```

### Getting Committed Offset

```http
GET /consumer-groups/order-processors/offsets/0
```

**Response**:
```json
{
  "group_id": "order-processors",
  "partition_id": 0,
  "offset": 1500
}
```

### Leaving Consumer Group

```http
DELETE /consumer-groups/order-processors/members/{member_id}/leave
```

**Response**:
```json
{
  "success": true,
  "member_id": "550e8400-e29b-41d4-a716-446655440000"
}
```

## Consumer Group Rebalancing

Automatic rebalancing occurs when:
- A new consumer joins the group
- An existing consumer leaves the group
- A consumer fails to send heartbeat (session timeout)

```
Initial State:
  Consumer 1: [0, 1, 2]
  Consumer 2: [3, 4, 5]

Consumer 3 Joins -> Rebalance Triggered:
  Consumer 1: [0, 3]
  Consumer 2: [1, 4]
  Consumer 3: [2, 5]

Consumer 2 Leaves -> Rebalance Triggered:
  Consumer 1: [0, 1, 2]
  Consumer 3: [3, 4, 5]
```

## Topic Statistics

```http
GET /topics/orders/stats
```

**Response**:
```json
{
  "topic": "orders",
  "partitions": [
    {
      "partition_id": 0,
      "topic": "orders",
      "message_count": 1500,
      "total_bytes": 384000,
      "min_offset": 0,
      "max_offset": 1499,
      "last_compaction": 60
    },
    {
      "partition_id": 1,
      "message_count": 1482,
      "total_bytes": 379392,
      "min_offset": 0,
      "max_offset": 1481,
      "last_compaction": 58
    },
    {
      "partition_id": 2,
      "message_count": 1523,
      "total_bytes": 389888,
      "min_offset": 0,
      "max_offset": 1522,
      "last_compaction": 62
    }
  ]
}
```

## Consumer Group Statistics

```http
GET /consumer-groups/order-processors/stats
```

**Response**:
```json
{
  "group_id": "order-processors",
  "topic": "orders",
  "state": "Stable",
  "member_count": 3,
  "generation": 5,
  "partition_count": 3,
  "committed_partitions": 3,
  "last_rebalance_secs": 120
}
```

## Use Cases

### Kafka-Compatible Event Processing

```rust
use synap_server::{PartitionManager, ConsumerGroupManager};

// Create topic with partitions
let pm = PartitionManager::new(PartitionConfig {
    num_partitions: 10,
    retention: RetentionPolicy::Time { retention_secs: 86400 },
    ..Default::default()
});
pm.create_topic("events", None).await?;

// Create consumer group
let cgm = ConsumerGroupManager::new(ConsumerGroupConfig::default());
cgm.create_group("processors", "events", 10, None).await?;

// Join group
let member = cgm.join_group("processors", 30).await?;
cgm.rebalance_group("processors").await?;

// Get assigned partitions
let partitions = cgm.get_assignment("processors", &member.id).await?;

// Consume from assigned partitions
for partition_id in partitions {
    let events = pm.consume_partition("events", partition_id, 0, 100).await?;
    
    // Process events...
    
    // Commit offset
    if let Some(last) = events.last() {
        cgm.commit_offset("processors", partition_id, last.offset + 1).await?;
    }
}
```

### User Activity Tracking with Key-Based Routing

```typescript
// All events for same user go to same partition
await client.publishToTopic('user-activity', {
  event_type: 'page.view',
  key: `user-${userId}`,  // Ensures ordering per user
  data: { page: '/dashboard', timestamp: Date.now() }
});
```

### Multi-Tenant Data Isolation

```python
# Each tenant's data in consistent partition
client.publish_to_topic(
    'tenant-events',
    event_type='data.updated',
    key=f'tenant-{tenant_id}',
    data={'entity': 'user', 'action': 'created'}
)
```

## Configuration

```yaml
partitioned_streams:
  enabled: true
  default_partitions: 3
  default_replication_factor: 1
  default_retention:
    type: "Combined"
    retention_secs: 3600
    max_bytes: 104857600
    max_messages: 100000
  segment_bytes: 104857600
  compaction_interval_secs: 60

consumer_groups:
  enabled: true
  default_strategy: "round_robin"
  session_timeout_secs: 30
  rebalance_timeout_secs: 60
  auto_commit: true
  auto_commit_interval_secs: 5
  rebalance_check_interval_secs: 10
```

## Performance Characteristics

### Partitioning Benefits
- **Parallel Processing**: Multiple consumers process different partitions simultaneously
- **Scalability**: Add partitions to increase throughput
- **Ordering Guarantees**: Events with same key maintain order within partition
- **Load Distribution**: Automatic load balancing via partition assignment

### Retention Efficiency
- **Time-based**: Automatically removes old events
- **Size-based**: Prevents unbounded memory growth
- **Count-based**: Maintains fixed buffer size
- **Combined**: Multiple constraints for fine-grained control

## Comparison: Simple vs Partitioned Streams

| Feature | Simple Rooms | Partitioned Topics |
|---------|--------------|-------------------|
| Use Case | Real-time broadcasting | Event processing pipelines |
| Consumers | All receive all events | Partition-based distribution |
| Ordering | Per-room total order | Per-partition total order |
| Scalability | Vertical (single room) | Horizontal (multiple partitions) |
| Consumer Groups | No | Yes |
| Key Routing | No | Yes |
| Retention | Time or count | Time, size, count, combined |
| Offset Management | Manual | Automatic commit/checkpoint |

## Testing

### Unit Tests
- ✅ Partition creation and deletion
- ✅ Key-based routing consistency
- ✅ Retention policies (time, size, count, combined)
- ✅ Consumer group creation and management
- ✅ Partition assignment strategies (round-robin, range, sticky)
- ✅ Offset commit and retrieval
- ✅ Consumer heartbeat and session timeout

### Integration Tests
- ✅ Kafka-style publish-consume with consumer groups
- ✅ Consumer group rebalancing on member join/leave
- ✅ Multiple consumer groups on same topic
- ✅ Partition key routing consistency
- ✅ Time-based retention with compaction
- ✅ Size-based retention enforcement
- ✅ Combined retention policy

### Performance Benchmarks
- Throughput: 10K+ events/sec per partition
- Consumer group rebalance: < 100ms
- Offset commit latency: < 1ms
- Partition assignment: O(n) where n = number of partitions

## See Also

- [PUBSUB.md](PUBSUB.md) - Pub/Sub system comparison
- [QUEUE_SYSTEM.md](QUEUE_SYSTEM.md) - Queue vs Stream differences
- [CHAT_SAMPLE.md](../examples/CHAT_SAMPLE.md) - Full chat implementation
- [EVENT_BROADCAST.md](../examples/EVENT_BROADCAST.md) - Broadcasting example

