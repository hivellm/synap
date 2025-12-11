# Publish/Subscribe Specification

## Overview

The Synap Pub/Sub system implements topic-based publish/subscribe messaging with hierarchical topics and wildcard subscriptions for flexible event routing.

## Core Concepts

### Topics
- Hierarchical namespace using dot notation
- Examples: `notifications.email`, `metrics.cpu.usage`, `events.user.login`
- Case-sensitive
- No length limit (practical limit ~256 chars)

### Publishers
- Publish messages to specific topics
- No knowledge of subscribers
- Fire-and-forget pattern
- Can publish to any topic without pre-creation

### Subscribers
- Subscribe to topics using exact match or wildcards
- Receive all messages published to matching topics
- Multiple subscribers can listen to same topic
- WebSocket connection for push delivery

## Wildcard Subscriptions

### Single-level Wildcard (*)

Matches exactly one level in topic hierarchy:

```
notifications.*        → Matches: notifications.email, notifications.sms
                        → Not: notifications.email.user
metrics.*.usage        → Matches: metrics.cpu.usage, metrics.mem.usage
                        → Not: metrics.cpu, metrics.cpu.usage.percent
```

### Multi-level Wildcard (#)

Matches zero or more levels:

```
notifications.#        → Matches: notifications, notifications.email,
                                  notifications.email.user
events.user.#          → Matches: events.user, events.user.login,
                                  events.user.login.success
#                      → Matches: ALL topics
```

## Data Structure

```rust
use radix_trie::Trie;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use parking_lot::RwLock;

pub struct PubSubRouter {
    // Exact topic subscriptions
    topics: Arc<RwLock<Trie<String, TopicSubscribers>>>,
    
    // Wildcard subscriptions (separate for efficiency)
    wildcard_subs: Arc<RwLock<Vec<WildcardSubscription>>>,
    
    stats: Arc<RwLock<PubSubStats>>,
}

pub struct TopicSubscribers {
    pub topic: String,
    pub subscribers: HashSet<SubscriberId>,
    pub message_count: AtomicU64,
    pub created_at: Instant,
}

pub struct WildcardSubscription {
    pub pattern: String,
    pub subscriber_id: SubscriberId,
    pub compiled_pattern: WildcardMatcher,
}

pub struct WildcardMatcher {
    pub segments: Vec<SegmentMatcher>,
}

pub enum SegmentMatcher {
    Exact(String),       // Literal string
    SingleLevel,         // * wildcard
    MultiLevel,          // # wildcard
}

pub struct PubSubStats {
    pub total_topics: usize,
    pub total_subscribers: usize,
    pub messages_published: AtomicU64,
    pub messages_delivered: AtomicU64,
}
```

## Operations Specification

### PUBLISH Message

**Command**: `pubsub.publish`

**Parameters**:
- `topic` (string, required): Topic name
- `message` (any, required): Message payload
- `metadata` (object, optional): Custom metadata

**Returns**:
- `message_id` (string): Unique message identifier
- `topic` (string): Published topic
- `subscribers_matched` (integer): Number of subscribers notified

**Example**:
```json
{
  "command": "pubsub.publish",
  "payload": {
    "topic": "notifications.email.user",
    "message": {
      "to": "alice@example.com",
      "subject": "Welcome!",
      "body": "Thanks for signing up"
    }
  }
}
```

**Response**:
```json
{
  "message_id": "msg_xyz789",
  "topic": "notifications.email.user",
  "subscribers_matched": 3
}
```

### SUBSCRIBE to Topic

**Command**: `pubsub.subscribe`

**Protocol**: Requires WebSocket connection

**Parameters**:
- `topics` (array[string], required): Topics or patterns to subscribe

**Returns**: Stream of messages via WebSocket

**Example Request**:
```json
{
  "command": "pubsub.subscribe",
  "payload": {
    "topics": [
      "notifications.email.*",
      "events.user.#",
      "metrics.cpu.usage"
    ]
  }
}
```

**Message Stream (WebSocket)**:
```json
{"message_id": "m1", "topic": "notifications.email.user", "message": {...}}
{"message_id": "m2", "topic": "events.user.login", "message": {...}}
{"message_id": "m3", "topic": "metrics.cpu.usage", "message": {...}}
```

### UNSUBSCRIBE from Topics

**Command**: `pubsub.unsubscribe`

**Parameters**:
- `topics` (array[string], required): Topics or patterns to unsubscribe

**Returns**:
- `unsubscribed` (integer): Number of subscriptions removed

## Topic Matching Algorithm

### Exact Match (Fast Path)

```rust
impl PubSubRouter {
    fn find_exact_subscribers(&self, topic: &str) -> HashSet<SubscriberId> {
        self.topics.read()
            .get(topic)
            .map(|t| t.subscribers.clone())
            .unwrap_or_default()
    }
}
```

### Wildcard Match

```rust
impl PubSubRouter {
    fn find_wildcard_subscribers(&self, topic: &str) -> HashSet<SubscriberId> {
        let topic_segments: Vec<&str> = topic.split('.').collect();
        let wildcards = self.wildcard_subs.read();
        
        wildcards.iter()
            .filter(|sub| sub.compiled_pattern.matches(&topic_segments))
            .map(|sub| sub.subscriber_id)
            .collect()
    }
}

impl WildcardMatcher {
    fn matches(&self, topic_segments: &[&str]) -> bool {
        let mut seg_idx = 0;
        
        for matcher in &self.segments {
            match matcher {
                SegmentMatcher::Exact(s) => {
                    if seg_idx >= topic_segments.len() || 
                       topic_segments[seg_idx] != s {
                        return false;
                    }
                    seg_idx += 1;
                }
                SegmentMatcher::SingleLevel => {
                    if seg_idx >= topic_segments.len() {
                        return false;
                    }
                    seg_idx += 1;
                }
                SegmentMatcher::MultiLevel => {
                    return true;  // # matches rest
                }
            }
        }
        
        seg_idx == topic_segments.len()
    }
}
```

## Message Routing

### Fan-out to Subscribers

```rust
impl PubSubRouter {
    pub async fn publish(&self, topic: &str, message: Message) -> Result<usize> {
        // Find all matching subscribers
        let mut subscribers = self.find_exact_subscribers(topic);
        subscribers.extend(self.find_wildcard_subscribers(topic));
        
        let count = subscribers.len();
        
        // Broadcast to all subscribers in parallel
        let tasks: Vec<_> = subscribers.into_iter()
            .map(|sub_id| {
                let msg = message.clone();
                let topic = topic.to_string();
                
                tokio::spawn(async move {
                    deliver_to_subscriber(sub_id, topic, msg).await
                })
            })
            .collect();
        
        // Wait for all deliveries
        futures::future::join_all(tasks).await;
        
        Ok(count)
    }
}
```

## Delivery Guarantees

### At-Most-Once
- Default delivery mode
- Messages sent to active subscribers
- No retry on delivery failure
- If subscriber disconnects, message is lost

### Best-Effort
- Attempt delivery with timeout
- Log failures but continue
- Suitable for non-critical notifications

### No Persistence
- Messages not stored after delivery
- Subscribers must be connected to receive
- Compare to Event Streams which store history

## Performance Characteristics

### Latency
- **Topic Matching**: O(k) for exact, O(n×m) for wildcards
  - k = topic length
  - n = wildcard subscriptions count
  - m = pattern segments
- **Delivery**: O(s) where s = subscriber count
- **Target**: < 0.5ms for topic routing + delivery

### Throughput
- **Messages/sec**: 100K+ (exact topics)
- **Messages/sec**: 10K-50K (with wildcards)
- **Subscribers**: 100K+ total across all topics
- **Topics**: Unlimited (radix tree scales well)

### Memory
- **Per Topic**: ~100 bytes + topic string
- **Per Subscription**: ~50 bytes
- **Per Wildcard**: ~200 bytes (pattern compilation)

## Configuration

```yaml
pubsub:
  enabled: true
  max_topics: 1000000
  max_subscribers_per_topic: 10000
  max_wildcard_subscriptions: 10000
  delivery_timeout_ms: 1000
  topic_cleanup_interval_mins: 60
  inactive_topic_timeout_hours: 24
```

## Comparison with Event Streams

| Feature | Pub/Sub | Event Stream |
|---------|---------|--------------|
| **Message Storage** | No | Yes (ring buffer) |
| **History** | No | Yes |
| **Replay** | No | Yes |
| **Ordering** | Best-effort | Guaranteed |
| **Wildcards** | Yes | No |
| **Topics** | Hierarchical | Flat rooms |
| **Use Case** | Notifications | Chat, logs, events |
| **Latency** | Lower | Slightly higher |

## Error Handling

```rust
pub enum PubSubError {
    InvalidTopic(String),
    InvalidPattern(String),
    TopicNotFound(String),
    SubscriberNotFound(SubscriberId),
    DeliveryTimeout,
    MaxSubscribersExceeded,
}
```

## Monitoring Metrics

```json
{
  "total_topics": 5000,
  "total_subscribers": 15000,
  "messages_published": 1000000,
  "messages_delivered": 3500000,
  "delivery_failures": 150,
  "avg_delivery_latency_ms": 0.3,
  "top_topics": [
    {"topic": "events.user.#", "subscribers": 500, "messages": 50000},
    {"topic": "notifications.*", "subscribers": 200, "messages": 30000}
  ]
}
```

## Testing Requirements

### Unit Tests
- Topic matching (exact)
- Wildcard matching (*, #)
- Subscriber management
- Fan-out logic

### Integration Tests
- Multiple subscribers to same topic
- Wildcard subscriptions
- Cross-topic patterns
- Concurrent publish

### Benchmarks
- Topic routing latency
- Wildcard match performance
- Fan-out to 100+ subscribers

## Example Usage

### Simple Pub/Sub

```typescript
// TypeScript SDK
const client = new SynapClient('http://localhost:15500');

// Subscribe to notifications
await client.pubsub.subscribe(['notifications.email.*']);

client.on('message', (topic, message) => {
  console.log(`[${topic}] ${message}`);
});

// Publish notification
await client.pubsub.publish('notifications.email.user', {
  to: 'alice@example.com',
  subject: 'Welcome!'
});
```

### Wildcard Subscriptions

```python
# Python SDK
client = SynapClient('http://localhost:15500')

# Subscribe to all user events
client.pubsub.subscribe(['events.user.#'])

# Subscribe to all metrics
client.pubsub.subscribe(['metrics.#'])

# Publish events
client.pubsub.publish('events.user.login', {'user_id': 123})
client.pubsub.publish('metrics.cpu.usage', {'percent': 75})
```

### Topic Hierarchy

```rust
// Rust SDK
let client = SynapClient::connect("http://localhost:15500").await?;

// Specific topic
client.pubsub_subscribe(&["alerts.critical.database"]).await?;

// All database alerts
client.pubsub_subscribe(&["alerts.critical.*"]).await?;

// All alerts
client.pubsub_subscribe(&["alerts.#"]).await?;
```

## See Also

- [EVENT_STREAM.md](EVENT_STREAM.md) - Room-based streaming
- [QUEUE_SYSTEM.md](QUEUE_SYSTEM.md) - Message queues
- [PUBSUB_PATTERN.md](../examples/PUBSUB_PATTERN.md) - Example usage

