# Queue System Specification

## Overview

The Synap Queue System implements RabbitMQ-style message queues with acknowledgment, priority support, and delivery guarantees for reliable asynchronous message processing.

## Core Features

### Queue Operations
- **PUBLISH**: Add message to queue with priority
- **CONSUME**: Retrieve message for processing
- **ACK**: Acknowledge successful processing
- **NACK**: Negative acknowledge (requeue or dead letter)
- **PURGE**: Clear all messages from queue
- **DELETE**: Remove queue entirely

### Queue Management
- **CREATE**: Create new queue with configuration
- **LIST**: List all queues
- **STATS**: Get queue statistics (depth, consumers, etc.)
- **PEEK**: View next message without consuming

### Advanced Features
- Message priorities (0-9)
- Delivery acknowledgment with timeout
- Automatic retry with exponential backoff
- Dead letter queue for failed messages
- Prefetch control for load balancing
- Message TTL (time-to-live)

## Data Structure

### Queue Implementation

```rust
use std::collections::{VecDeque, HashMap};
use std::sync::Arc;
use parking_lot::RwLock;
use tokio::sync::mpsc;

pub struct QueueManager {
    queues: Arc<RwLock<HashMap<String, Queue>>>,
    config: QueueConfig,
}

pub struct Queue {
    pub name: String,
    pub messages: VecDeque<QueueMessage>,
    pub pending: HashMap<MessageId, PendingMessage>,
    pub consumers: Vec<Consumer>,
    pub stats: QueueStats,
    pub config: QueueConfig,
}

pub struct QueueMessage {
    pub id: MessageId,
    pub payload: Vec<u8>,
    pub priority: u8,              // 0-9 (9 = highest)
    pub retry_count: u32,
    pub max_retries: u32,
    pub created_at: Instant,
    pub headers: HashMap<String, String>,
}

pub struct PendingMessage {
    pub message: QueueMessage,
    pub consumer_id: ConsumerId,
    pub delivered_at: Instant,
    pub ack_deadline: Instant,
}

pub struct Consumer {
    pub id: ConsumerId,
    pub connection_id: String,
    pub prefetch: u32,
    pub current_messages: u32,
}

pub struct QueueStats {
    pub depth: usize,
    pub consumers: usize,
    pub published: AtomicU64,
    pub consumed: AtomicU64,
    pub acked: AtomicU64,
    pub nacked: AtomicU64,
    pub dead_lettered: AtomicU64,
}
```

## Queue Behavior

### Message Lifecycle

```
1. PUBLISH
     │
     ▼
2. In Queue (ordered by priority)
     │
     ▼
3. CONSUME → Pending (awaiting ACK)
     │
     ├─→ ACK → Removed
     │
     └─→ NACK or Timeout
         │
         ├─→ Retry (if retries left)
         └─→ Dead Letter Queue
```

### Priority Queuing

Messages are ordered by:
1. **Priority** (9 = highest, 0 = lowest)
2. **Timestamp** (FIFO within same priority)

```rust
impl Queue {
    fn insert_by_priority(&mut self, msg: QueueMessage) {
        let position = self.messages
            .iter()
            .position(|m| m.priority < msg.priority || 
                         (m.priority == msg.priority && 
                          m.created_at > msg.created_at))
            .unwrap_or(self.messages.len());
        
        self.messages.insert(position, msg);
    }
}
```

## Operations Specification

### PUBLISH Operation

**Command**: `queue.publish`

**Parameters**:
- `queue` (string, required): Queue name
- `message` (any, required): Message payload
- `priority` (integer, optional): Priority 0-9 (default: 5)
- `ttl` (integer, optional): Message TTL in seconds
- `headers` (object, optional): Custom headers

**Returns**:
- `message_id` (string): Unique message identifier
- `position` (integer): Position in queue

**Example**:
```json
{
  "command": "queue.publish",
  "payload": {
    "queue": "tasks",
    "message": {
      "type": "process_video",
      "video_id": "vid_123",
      "user_id": "user_456"
    },
    "priority": 8,
    "headers": {
      "source": "web-app"
    }
  }
}
```

**Response**:
```json
{
  "message_id": "msg_7890abcd",
  "position": 3
}
```

### CONSUME Operation

**Command**: `queue.consume`

**Parameters**:
- `queue` (string, required): Queue name
- `timeout` (integer, optional): Wait timeout in seconds (0 = no wait)
- `ack_deadline` (integer, optional): ACK deadline in seconds (default: 30)

**Returns**:
- `message_id` (string): Message identifier
- `message` (any): Message payload
- `priority` (integer): Message priority
- `retry_count` (integer): Number of previous retries
- `headers` (object): Message headers

**Example**:
```json
{
  "command": "queue.consume",
  "payload": {
    "queue": "tasks",
    "timeout": 10,
    "ack_deadline": 60
  }
}
```

**Response**:
```json
{
  "message_id": "msg_7890abcd",
  "message": {
    "type": "process_video",
    "video_id": "vid_123"
  },
  "priority": 8,
  "retry_count": 0,
  "headers": {"source": "web-app"}
}
```

### ACK Operation

**Command**: `queue.ack`

**Parameters**:
- `queue` (string, required): Queue name
- `message_id` (string, required): Message ID to acknowledge

**Returns**:
- `success` (boolean): Acknowledgment result

**Example**:
```json
{
  "command": "queue.ack",
  "payload": {
    "queue": "tasks",
    "message_id": "msg_7890abcd"
  }
}
```

### NACK Operation

**Command**: `queue.nack`

**Parameters**:
- `queue` (string, required): Queue name
- `message_id` (string, required): Message ID
- `requeue` (boolean, optional): Requeue for retry (default: true)

**Returns**:
- `success` (boolean): Operation result
- `action` (string): "requeued" or "dead_lettered"

**Example**:
```json
{
  "command": "queue.nack",
  "payload": {
    "queue": "tasks",
    "message_id": "msg_7890abcd",
    "requeue": true
  }
}
```

## Acknowledgment Timeout

### Automatic Requeue

If message not ACKed within deadline:

```rust
impl Queue {
    async fn monitor_ack_deadlines(&mut self) {
        let mut interval = tokio::time::interval(Duration::from_secs(1));
        
        loop {
            interval.tick().await;
            let now = Instant::now();
            
            let timed_out: Vec<_> = self.pending.iter()
                .filter(|(_, p)| p.ack_deadline < now)
                .map(|(id, _)| *id)
                .collect();
            
            for msg_id in timed_out {
                self.handle_timeout(msg_id).await;
            }
        }
    }
    
    async fn handle_timeout(&mut self, msg_id: MessageId) {
        if let Some(pending) = self.pending.remove(&msg_id) {
            let mut msg = pending.message;
            msg.retry_count += 1;
            
            if msg.retry_count > msg.max_retries {
                self.dead_letter(msg).await;
            } else {
                self.requeue(msg).await;
            }
        }
    }
}
```

## Dead Letter Queue

### Behavior
Messages are moved to DLQ when:
- Max retries exceeded
- Explicitly NACKed with `requeue: false`
- Message TTL expired

### DLQ Structure
```rust
pub struct DeadLetterQueue {
    pub messages: Vec<DeadLetteredMessage>,
    pub max_size: usize,
}

pub struct DeadLetteredMessage {
    pub original_queue: String,
    pub message: QueueMessage,
    pub reason: DeadLetterReason,
    pub dead_lettered_at: Instant,
}

pub enum DeadLetterReason {
    MaxRetriesExceeded,
    ExplicitNack,
    TTLExpired,
}
```

### DLQ Operations
- **List**: View dead lettered messages
- **Retry**: Move message back to original queue
- **Purge**: Clear dead letter queue

## Prefetch Control

### Consumer Prefetch

Limits how many unacknowledged messages a consumer can have:

```json
{
  "command": "queue.consume",
  "payload": {
    "queue": "tasks",
    "prefetch": 10
  }
}
```

**Behavior**:
- Consumer receives up to `prefetch` messages
- Must ACK/NACK before receiving more
- Prevents overwhelming slow consumers
- Enables load balancing across consumers

## Queue Configuration

```yaml
queue:
  default_ack_deadline_secs: 30
  default_max_retries: 3
  default_priority: 5
  max_queue_depth: 100000
  dead_letter:
    enabled: true
    max_size: 10000
  prefetch:
    default: 10
    max: 1000
```

## Performance Characteristics

### Latency Targets
- **PUBLISH**: < 1ms (insert to VecDeque)
- **CONSUME**: < 0.5ms (pop from VecDeque)
- **ACK**: < 0.5ms (remove from HashMap)

### Throughput Targets
- **Publish Rate**: 50K msgs/sec
- **Consume Rate**: 100K msgs/sec
- **Concurrent Queues**: 10K+ queues

### Memory Usage
- **Per Message**: ~200 bytes + payload size
- **Per Queue**: ~1KB base overhead
- **Pending Tracking**: ~150 bytes per unacked message

## Error Handling

```rust
pub enum QueueError {
    QueueNotFound(String),
    QueueFull(String),
    MessageNotFound(MessageId),
    InvalidPriority(u8),
    ConsumerNotFound(ConsumerId),
    AckDeadlineExceeded,
}
```

## Monitoring Metrics

```json
{
  "queue": "tasks",
  "depth": 1523,
  "consumers": 5,
  "published_total": 100000,
  "consumed_total": 98477,
  "acked_total": 97850,
  "nacked_total": 627,
  "dead_lettered_total": 50,
  "avg_wait_time_ms": 125,
  "oldest_message_age_secs": 45
}
```

## Replication Behavior

### Write Replication
```
PUBLISH → Master
    │
    ├─→ Add to queue
    │
    └─→ Replicate
        {
          "op": "queue.publish",
          "queue": "tasks",
          "message": {...}
        }
```

### Consume from Replica
- Consumers can consume from read replicas
- CONSUME doesn't modify queue (read-only)
- ACK/NACK must be sent to master
- Master propagates ACK to replicas

## Testing

### Test Scenarios
1. Basic FIFO ordering
2. Priority ordering
3. Multiple consumers (load balancing)
4. ACK timeout and retry
5. Dead letter queue
6. Prefetch limits
7. Concurrent publish/consume
8. Queue full behavior

## See Also

- [EVENT_STREAM.md](EVENT_STREAM.md) - Event streaming system
- [PUBSUB.md](PUBSUB.md) - Pub/sub messaging
- [TASK_QUEUE.md](../examples/TASK_QUEUE.md) - Example implementation

