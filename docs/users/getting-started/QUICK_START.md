---
title: Quick Start Guide
module: quick-start
id: quick-start-guide
order: 1
description: Get up and running with Synap in minutes
tags: [quick-start, tutorial, getting-started]
---

# Quick Start Guide

Get up and running with Synap in minutes!

## Prerequisites

- Synap installed (see [Installation Guide](./INSTALLATION.md))
- Service running on `http://localhost:15500`
- `curl` or similar HTTP client (or use the SDKs)

## Step 1: Verify Installation

```bash
# Health check
curl http://localhost:15500/health

# Expected output:
# {"status":"healthy","uptime_secs":5}
```

## Step 2: Your First Key-Value Operations

```bash
# Set a key
curl -X POST http://localhost:15500/kv/set \
  -H "Content-Type: application/json" \
  -d '{"key":"user:1","value":"John Doe","ttl":3600}'

# Get the key
curl http://localhost:15500/kv/get/user:1

# Output: "John Doe"

# Delete the key
curl -X DELETE http://localhost:15500/kv/del/user:1
```

## Step 3: Your First Queue Message

```bash
# Create queue
curl -X POST http://localhost:15500/queue/tasks \
  -H "Content-Type: application/json" \
  -d '{"max_depth":1000,"ack_deadline_secs":30}'

# Publish message
curl -X POST http://localhost:15500/queue/tasks/publish \
  -H "Content-Type: application/json" \
  -d '{"payload":[72,101,108,108,111],"priority":5}'

# Consume message
curl http://localhost:15500/queue/tasks/consume/worker-1

# Acknowledge message
curl -X POST http://localhost:15500/queue/tasks/ack \
  -H "Content-Type: application/json" \
  -d '{"message_id":"<message_id_from_consume>"}'
```

## Step 4: Your First Stream Event

```bash
# Create stream room
curl -X POST http://localhost:15500/stream/chat-room-1

# Publish event
curl -X POST http://localhost:15500/stream/chat-room-1/publish \
  -H "Content-Type: application/json" \
  -d '{"event":"message","data":"Hello, World!"}'

# Consume events
curl "http://localhost:15500/stream/chat-room-1/consume/user-1?from_offset=0&limit=10"
```

## Step 5: Your First Pub/Sub Message

```bash
# Publish to topic
curl -X POST http://localhost:15500/pubsub/notifications.email/publish \
  -H "Content-Type: application/json" \
  -d '{"message":"New order received"}'
```

**Subscribe with WebSocket:**
```javascript
const ws = new WebSocket('ws://localhost:15500/pubsub/ws?topics=notifications.email');

ws.onmessage = (event) => {
  const msg = JSON.parse(event.data);
  console.log('Received:', msg);
};
```

## Using SDKs

### Python SDK

```python
from synap_sdk import SynapClient

client = SynapClient("http://localhost:15500")

# Key-Value
client.kv.set("user:1", "John Doe", ttl=3600)
value = client.kv.get("user:1")
client.kv.delete("user:1")

# Queue
client.queue.create("tasks", max_depth=1000)
client.queue.publish("tasks", "Hello", priority=5)
message = client.queue.consume("tasks", "worker-1")
client.queue.ack("tasks", message.message_id)
```

### TypeScript/JavaScript SDK

```typescript
import { Synap } from "@hivehub/synap";

const client = new SynapClient("http://localhost:15500");

// Key-Value
await client.kv.set("user:1", "John Doe", { ttl: 3600 });
const value = await client.kv.get("user:1");
await client.kv.delete("user:1");

// Queue
await client.queue.create("tasks", { maxDepth: 1000 });
await client.queue.publish("tasks", "Hello", { priority: 5 });
const message = await client.queue.consume("tasks", "worker-1");
await client.queue.ack("tasks", message.messageId);
```

### Rust SDK

```rust
use synap_sdk::SynapClient;

let client = SynapClient::new("http://localhost:15500")?;

// Key-Value
client.kv.set("user:1", "John Doe", Some(3600)).await?;
let value = client.kv.get("user:1").await?;
client.kv.delete("user:1").await?;

// Queue
client.queue.create("tasks", 1000).await?;
client.queue.publish("tasks", b"Hello", 5).await?;
let message = client.queue.consume("tasks", "worker-1").await?;
client.queue.ack("tasks", &message.message_id).await?;
```

## Next Steps

1. **[First Steps](./FIRST_STEPS.md)** - Complete guide after installation
2. **[Basic KV Operations](../kv-store/BASIC.md)** - Learn key-value operations
3. **[Message Queues](../queues/CREATING.md)** - Learn about queues
4. **[Use Cases](../use-cases/)** - See real-world examples

## Additional Resources

- [KV Store Guide](../kv-store/KV_STORE.md) - Complete key-value reference
- [Queues Guide](../queues/QUEUES.md) - Complete queues reference
- [Streams Guide](../streams/STREAMS.md) - Complete streams reference
- [API Reference](../api/API_REFERENCE.md) - REST API documentation

