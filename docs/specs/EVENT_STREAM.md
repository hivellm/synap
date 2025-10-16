# Event Stream Specification

## Overview

The Synap Event Stream system provides Kafka-style room-based broadcasting where all subscribers to a room receive all events in real-time with message history and replay capabilities.

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

## See Also

- [PUBSUB.md](PUBSUB.md) - Pub/Sub system comparison
- [QUEUE_SYSTEM.md](QUEUE_SYSTEM.md) - Queue vs Stream differences
- [CHAT_SAMPLE.md](../examples/CHAT_SAMPLE.md) - Full chat implementation
- [EVENT_BROADCAST.md](../examples/EVENT_BROADCAST.md) - Broadcasting example

