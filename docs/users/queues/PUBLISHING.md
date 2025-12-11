---
title: Publishing Messages
module: queues
id: queues-publishing
order: 2
description: Publishing messages to queues
tags: [queues, publishing, messages]
---

# Publishing Messages

How to publish messages to Synap queues.

## Basic Publishing

### Simple Message

```bash
curl -X POST http://localhost:15500/queue/jobs/publish \
  -H "Content-Type: application/json" \
  -d '{
    "payload": [72, 101, 108, 108, 111]
  }'
```

Payload is an array of bytes (UTF-8 encoded).

### With Priority

```bash
curl -X POST http://localhost:15500/queue/jobs/publish \
  -H "Content-Type: application/json" \
  -d '{
    "payload": [72, 101, 108, 108, 111],
    "priority": 9
  }'
```

Priority range: 0-9 (9 = highest priority).

### With Retry Configuration

```bash
curl -X POST http://localhost:15500/queue/jobs/publish \
  -H "Content-Type: application/json" \
  -d '{
    "payload": [72, 101, 108, 108, 111],
    "priority": 5,
    "max_retries": 3
  }'
```

## Message Format

### String Payload

```python
import json
from synap_sdk import SynapClient

client = SynapClient("http://localhost:15500")

# String payload
message = json.dumps({"task": "process_video", "video_id": "123"})
payload = list(message.encode('utf-8'))

client.queue.publish("jobs", payload, priority=5)
```

### Binary Payload

```python
# Binary data
payload = [0x48, 0x65, 0x6c, 0x6c, 0x6f]  # "Hello" in bytes

client.queue.publish("jobs", payload, priority=5)
```

## Priority Levels

### Priority 0-2: Low Priority

```bash
curl -X POST http://localhost:15500/queue/jobs/publish \
  -H "Content-Type: application/json" \
  -d '{
    "payload": [72, 101, 108, 108, 111],
    "priority": 0
  }'
```

Use for background tasks, batch processing.

### Priority 3-6: Normal Priority

```bash
curl -X POST http://localhost:15500/queue/jobs/publish \
  -H "Content-Type: application/json" \
  -d '{
    "payload": [72, 101, 108, 108, 111],
    "priority": 5
  }'
```

Use for typical user-initiated tasks.

### Priority 7-9: High Priority

```bash
curl -X POST http://localhost:15500/queue/jobs/publish \
  -H "Content-Type: application/json" \
  -d '{
    "payload": [72, 101, 108, 108, 111],
    "priority": 9
  }'
```

Use for critical, time-sensitive operations.

## Batch Publishing

### Multiple Messages

```python
from synap_sdk import SynapClient

client = SynapClient("http://localhost:15500")

messages = [
    {"payload": list("task1".encode()), "priority": 5},
    {"payload": list("task2".encode()), "priority": 7},
    {"payload": list("task3".encode()), "priority": 3}
]

for msg in messages:
    client.queue.publish("jobs", msg["payload"], priority=msg["priority"])
```

## Response

### Success Response

```json
{
  "success": true,
  "message_id": "abc-123-def-456"
}
```

### Error Response

```json
{
  "success": false,
  "error": {
    "type": "QueueFull",
    "message": "Queue is full",
    "status_code": 503
  }
}
```

## Using SDKs

### Python

```python
from synap_sdk import SynapClient

client = SynapClient("http://localhost:15500")

# Basic publish
client.queue.publish("jobs", b"Hello", priority=5)

# With retry configuration
client.queue.publish("jobs", b"Hello", priority=5, max_retries=3)
```

### TypeScript

```typescript
import { Synap } from "@hivehub/synap";

const client = new SynapClient("http://localhost:15500");

// Basic publish
await client.queue.publish("jobs", Buffer.from("Hello"), { priority: 5 });

// With retry configuration
await client.queue.publish("jobs", Buffer.from("Hello"), {
  priority: 5,
  maxRetries: 3
});
```

### Rust

```rust
use synap_sdk::SynapClient;

let client = SynapClient::new("http://localhost:15500")?;

// Basic publish
client.queue.publish("jobs", b"Hello", 5).await?;

// With retry configuration
client.queue.publish_with_retries("jobs", b"Hello", 5, 3).await?;
```

## Best Practices

### Use Appropriate Priorities

- Reserve priority 9 for critical operations
- Use priority 5 for normal operations
- Use priority 0-2 for background tasks

### Set Max Retries Per Message Type

- **Idempotent operations**: 5-10 retries
- **Non-idempotent operations**: 1-3 retries
- **Critical operations**: Monitor closely

### Monitor Queue Depth

```bash
# Check queue statistics
curl http://localhost:15500/queue/jobs/stats
```

If `pending` is consistently high, consider:
- Adding more consumers
- Increasing processing capacity
- Reviewing message priority distribution

## Related Topics

- [Creating Queues](./CREATING.md) - Queue creation
- [Consuming Messages](./CONSUMING.md) - Message consumption
- [Complete Queues Guide](./QUEUES.md) - Comprehensive reference

