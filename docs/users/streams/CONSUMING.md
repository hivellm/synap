---
title: Consuming Events
module: streams
id: streams-consuming
order: 3
description: Consuming events from streams with offset management
tags: [streams, consuming, offset, consumer-groups]
---

# Consuming Events

How to consume events from Synap streams with offset management.

## Basic Consumption

### Consume Events

```bash
curl "http://localhost:15500/stream/notifications/consume/user-1?from_offset=0&limit=10"
```

**Response:**
```json
[
  {
    "offset": 0,
    "event": "user.signup",
    "data": "New user registered",
    "timestamp": 1234567890
  },
  {
    "offset": 1,
    "event": "user.login",
    "data": "User logged in",
    "timestamp": 1234567891
  }
]
```

### Query Parameters

- **from_offset**: Starting offset (default: 0)
- **limit**: Maximum events to return (default: 10)

## Offset Management

### Start from Beginning

```bash
curl "http://localhost:15500/stream/notifications/consume/user-1?from_offset=0&limit=100"
```

### Continue from Last Position

```bash
# Get last consumed offset (store in your app)
last_offset = 42

# Continue from last position
curl "http://localhost:15500/stream/notifications/consume/user-1?from_offset=43&limit=100"
```

### Get Latest Events

```bash
# Get current max offset
curl http://localhost:15500/stream/notifications/stats

# Consume from max offset
curl "http://localhost:15500/stream/notifications/consume/user-1?from_offset=155&limit=10"
```

## Consumer Groups

### Create Consumer Group

```bash
curl -X POST http://localhost:15500/stream/notifications/group/email-service
```

### Consume with Consumer Group

```bash
curl "http://localhost:15500/stream/notifications/group/email-service/consume?limit=10"
```

Consumer groups automatically manage offsets per group.

## WebSocket Streaming

### Real-Time Consumption

```javascript
const ws = new WebSocket('ws://localhost:15500/stream/notifications/ws/user-1?from_offset=0');

ws.onmessage = (event) => {
  const msg = JSON.parse(event.data);
  console.log('Offset:', msg.offset);
  console.log('Event:', msg.event);
  console.log('Data:', msg.data);
};

ws.onerror = (error) => {
  console.error('WebSocket error:', error);
};

ws.onclose = () => {
  console.log('WebSocket closed');
};
```

### Reconnect with Offset

```javascript
let lastOffset = 0;

function connect() {
  const ws = new WebSocket(`ws://localhost:15500/stream/notifications/ws/user-1?from_offset=${lastOffset}`);
  
  ws.onmessage = (event) => {
    const msg = JSON.parse(event.data);
    lastOffset = msg.offset + 1;  // Update offset
    processEvent(msg);
  };
  
  ws.onclose = () => {
    // Reconnect after delay
    setTimeout(connect, 1000);
  };
}

connect();
```

## Using SDKs

### Python

```python
from synap_sdk import SynapClient

client = SynapClient("http://localhost:15500")

# Consume events
events = client.stream.consume("notifications", "user-1", from_offset=0, limit=10)

for event in events:
    print(f"Offset: {event.offset}, Event: {event.event}, Data: {event.data}")
    # Update offset for next consumption
    last_offset = event.offset + 1
```

### TypeScript

```typescript
import { Synap } from "@hivehub/synap";

const client = new SynapClient("http://localhost:15500");

// Consume events
const events = await client.stream.consume("notifications", "user-1", {
  fromOffset: 0,
  limit: 10
});

for (const event of events) {
  console.log(`Offset: ${event.offset}, Event: ${event.event}, Data: ${event.data}`);
  // Update offset for next consumption
  lastOffset = event.offset + 1;
}
```

### Rust

```rust
use synap_sdk::SynapClient;

let client = SynapClient::new("http://localhost:15500")?;

// Consume events
let events = client.stream.consume("notifications", "user-1", 0, 10).await?;

for event in events {
    println!("Offset: {}, Event: {}, Data: {}", event.offset, event.event, event.data);
    // Update offset for next consumption
    last_offset = event.offset + 1;
}
```

## Polling Pattern

### Continuous Polling

```python
from synap_sdk import SynapClient
import time

client = SynapClient("http://localhost:15500")
consumer_id = "user-1"
last_offset = 0

while True:
    events = client.stream.consume("notifications", consumer_id, from_offset=last_offset, limit=10)
    
    if events:
        for event in events:
            process_event(event)
            last_offset = event.offset + 1
    else:
        # No new events, wait before next poll
        time.sleep(1)
```

## Consumer Groups

### Multiple Consumers

```python
# Consumer 1
events1 = client.stream.consume_group("notifications", "email-service", limit=10)

# Consumer 2 (same group, different messages)
events2 = client.stream.consume_group("notifications", "email-service", limit=10)
```

Each consumer in a group gets different messages (load balancing).

## Best Practices

### Store Offset Persistently

```python
# Store offset in database or file
def save_offset(consumer_id, offset):
    with open(f"offset_{consumer_id}.txt", "w") as f:
        f.write(str(offset))

def load_offset(consumer_id):
    try:
        with open(f"offset_{consumer_id}.txt", "r") as f:
            return int(f.read())
    except FileNotFoundError:
        return 0

# Load and use offset
last_offset = load_offset("user-1")
events = client.stream.consume("notifications", "user-1", from_offset=last_offset, limit=10)

for event in events:
    process_event(event)
    save_offset("user-1", event.offset + 1)
```

### Handle Gaps

If events are missing (due to retention), handle gracefully:

```python
events = client.stream.consume("notifications", "user-1", from_offset=last_offset, limit=10)

if not events:
    # Check if we're behind
    stats = client.stream.stats("notifications")
    if last_offset < stats.min_offset:
        # We're too far behind, start from min_offset
        last_offset = stats.min_offset
```

### Use Consumer Groups for Scaling

```python
# Multiple workers in same consumer group
# Each gets different messages automatically
events = client.stream.consume_group("notifications", "email-service", limit=10)
```

## Related Topics

- [Creating Streams](./CREATING.md) - Stream creation
- [Publishing Events](./PUBLISHING.md) - Event publishing
- [Complete Streams Guide](./STREAMS.md) - Comprehensive reference

