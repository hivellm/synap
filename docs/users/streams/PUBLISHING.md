---
title: Publishing Events
module: streams
id: streams-publishing
order: 2
description: Publishing events to streams
tags: [streams, publishing, events]
---

# Publishing Events

How to publish events to Synap streams.

## Basic Publishing

### Publish Event

```bash
curl -X POST http://localhost:15500/stream/notifications/publish \
  -H "Content-Type: application/json" \
  -d '{
    "event": "user.signup",
    "data": "New user registered"
  }'
```

**Response:**
```json
{
  "success": true,
  "offset": 0
}
```

### Publish JSON Event

```bash
curl -X POST http://localhost:15500/stream/notifications/publish \
  -H "Content-Type: application/json" \
  -d '{
    "event": "order.created",
    "data": "{\"order_id\":123,\"user_id\":456,\"total\":99.99}"
  }'
```

## Event Structure

### Event Fields

- **event**: Event type/name (string)
- **data**: Event payload (string, typically JSON)

### Example Events

```bash
# User events
curl -X POST http://localhost:15500/stream/events/publish \
  -H "Content-Type: application/json" \
  -d '{
    "event": "user.signup",
    "data": "{\"user_id\":123,\"email\":\"user@example.com\"}"
  }'

# Order events
curl -X POST http://localhost:15500/stream/events/publish \
  -H "Content-Type: application/json" \
  -d '{
    "event": "order.created",
    "data": "{\"order_id\":456,\"total\":99.99}"
  }'

# System events
curl -X POST http://localhost:15500/stream/events/publish \
  -H "Content-Type: application/json" \
  -d '{
    "event": "system.alert",
    "data": "{\"level\":\"warning\",\"message\":\"High memory usage\"}"
  }'
```

## Partitioning

### Automatic Partitioning

Events are automatically distributed across partitions based on key (if provided):

```bash
curl -X POST http://localhost:15500/stream/notifications/publish \
  -H "Content-Type: application/json" \
  -d '{
    "event": "user.signup",
    "data": "New user",
    "partition_key": "user-123"
  }'
```

Events with same partition key go to same partition (maintains order).

### Round-Robin Partitioning

If no partition key, events are distributed round-robin across partitions.

## Using SDKs

### Python

```python
from synap_sdk import SynapClient
import json

client = SynapClient("http://localhost:15500")

# Publish simple event
client.stream.publish("notifications", "user.signup", "New user registered")

# Publish JSON event
event_data = json.dumps({"user_id": 123, "email": "user@example.com"})
client.stream.publish("notifications", "user.signup", event_data)

# With partition key
client.stream.publish("notifications", "user.signup", event_data, partition_key="user-123")
```

### TypeScript

```typescript
import { Synap } from "@hivehub/synap";

const client = new SynapClient("http://localhost:15500");

// Publish simple event
await client.stream.publish("notifications", "user.signup", "New user registered");

// Publish JSON event
const eventData = JSON.stringify({ userId: 123, email: "user@example.com" });
await client.stream.publish("notifications", "user.signup", eventData);

// With partition key
await client.stream.publish("notifications", "user.signup", eventData, {
  partitionKey: "user-123"
});
```

### Rust

```rust
use synap_sdk::SynapClient;
use serde_json;

let client = SynapClient::new("http://localhost:15500")?;

// Publish simple event
client.stream.publish("notifications", "user.signup", "New user registered").await?;

// Publish JSON event
let event_data = serde_json::json!({
    "user_id": 123,
    "email": "user@example.com"
}).to_string();
client.stream.publish("notifications", "user.signup", &event_data).await?;

// With partition key
client.stream.publish_with_key("notifications", "user.signup", &event_data, "user-123").await?;
```

## Batch Publishing

### Multiple Events

```python
events = [
    {"event": "user.signup", "data": "User 1"},
    {"event": "user.signup", "data": "User 2"},
    {"event": "user.login", "data": "User 1 logged in"}
]

for evt in events:
    client.stream.publish("notifications", evt["event"], evt["data"])
```

## Event Ordering

### Ordered Events

Events with same partition key maintain order:

```python
# All events for user-123 go to same partition
client.stream.publish("notifications", "user.signup", data1, partition_key="user-123")
client.stream.publish("notifications", "user.login", data2, partition_key="user-123")
client.stream.publish("notifications", "user.logout", data3, partition_key="user-123")
```

### Unordered Events

Events without partition key may be processed out of order:

```python
# No partition key - may be out of order
client.stream.publish("notifications", "event1", data1)
client.stream.publish("notifications", "event2", data2)
```

## Best Practices

### Use Consistent Event Names

- Use hierarchical naming: `category.action`
- Examples: `user.signup`, `order.created`, `payment.processed`

### Keep Events Small

- Keep event data under 1MB
- Use references for large data
- Store large data separately and reference in event

### Use Partition Keys for Ordering

If order matters, use partition keys:

```python
# Ordered by user_id
client.stream.publish("events", "order.created", data, partition_key=f"user-{user_id}")
```

### Monitor Stream Statistics

```bash
# Check stream stats
curl http://localhost:15500/stream/notifications/stats
```

## Related Topics

- [Creating Streams](./CREATING.md) - Stream creation
- [Consuming Events](./CONSUMING.md) - Event consumption
- [Complete Streams Guide](./STREAMS.md) - Comprehensive reference

