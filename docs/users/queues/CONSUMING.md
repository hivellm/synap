---
title: Consuming Messages
module: queues
id: queues-consuming
order: 3
description: Consuming and acknowledging messages from queues
tags: [queues, consuming, ack, nack]
---

# Consuming Messages

How to consume and acknowledge messages from Synap queues.

## Basic Consumption

### Consume Message

```bash
curl http://localhost:15500/queue/jobs/consume/worker-1
```

**Response:**
```json
{
  "message_id": "abc-123-def-456",
  "payload": [72, 101, 108, 108, 111],
  "priority": 5,
  "retry_count": 0,
  "timestamp": 1234567890
}
```

### No Messages Available

If no messages are available, returns `204 No Content`.

## Acknowledging Messages

### Acknowledge (ACK)

```bash
curl -X POST http://localhost:15500/queue/jobs/ack \
  -H "Content-Type: application/json" \
  -d '{"message_id":"abc-123-def-456"}'
```

**Response:**
```json
{
  "success": true
}
```

Message is removed from queue after ACK.

### Negative Acknowledge (NACK)

```bash
curl -X POST http://localhost:15500/queue/jobs/nack \
  -H "Content-Type: application/json" \
  -d '{"message_id":"abc-123-def-456"}'
```

**Response:**
```json
{
  "success": true,
  "retry_count": 1
}
```

Message goes back to queue with incremented retry count.

## Consumer Patterns

### Polling Pattern

```python
from synap_sdk import SynapClient
import time

client = SynapClient("http://localhost:15500")
worker_id = "worker-1"

while True:
    message = client.queue.consume("jobs", worker_id)
    
    if message:
        try:
            # Process message
            process_message(message.payload)
            
            # ACK on success
            client.queue.ack("jobs", message.message_id)
        except Exception as e:
            # NACK on error
            client.queue.nack("jobs", message.message_id)
            print(f"Error processing message: {e}")
    else:
        # No messages, wait before next poll
        time.sleep(1)
```

### Long Polling

```python
# Consume with timeout (if supported)
message = client.queue.consume("jobs", worker_id, timeout=30)
```

## Dead Letter Queue (DLQ)

### Messages Exceeding Max Retries

When a message exceeds `max_retries`, it's moved to DLQ:

```bash
# Check DLQ count
curl http://localhost:15500/queue/jobs/stats
```

**Response:**
```json
{
  "pending": 10,
  "in_flight": 2,
  "dlq_count": 5,
  "total_published": 1000
}
```

### Consume from DLQ

```bash
curl http://localhost:15500/queue/jobs/dlq/consume/worker-1
```

### Purge DLQ

```bash
curl -X POST http://localhost:15500/queue/jobs/dlq/purge
```

## Message Processing

### Process and ACK

```python
def process_message(payload):
    # Decode payload
    data = bytes(payload).decode('utf-8')
    task = json.loads(data)
    
    # Process task
    result = execute_task(task)
    
    return result

# Consume and process
message = client.queue.consume("jobs", "worker-1")
if message:
    try:
        result = process_message(message.payload)
        client.queue.ack("jobs", message.message_id)
    except Exception as e:
        client.queue.nack("jobs", message.message_id)
```

### Idempotent Processing

```python
def is_already_processed(message_id):
    # Check if message was already processed
    return client.kv.exists(f"processed:{message_id}")

def mark_as_processed(message_id):
    # Mark message as processed
    client.kv.set(f"processed:{message_id}", "1", ttl=3600)

message = client.queue.consume("jobs", "worker-1")
if message:
    if not is_already_processed(message.message_id):
        process_message(message.payload)
        mark_as_processed(message.message_id)
    client.queue.ack("jobs", message.message_id)
```

## ACK Deadline

### Understanding ACK Deadline

Messages have an ACK deadline. If not ACKed within deadline, message returns to queue:

```yaml
# Queue configuration
ack_deadline_secs: 30  # 30 seconds
```

### Extending Deadline

```bash
# Extend ACK deadline (if supported)
curl -X POST http://localhost:15500/queue/jobs/extend \
  -H "Content-Type: application/json" \
  -d '{
    "message_id": "abc-123",
    "deadline_secs": 60
  }'
```

## Using SDKs

### Python

```python
from synap_sdk import SynapClient

client = SynapClient("http://localhost:15500")

# Consume
message = client.queue.consume("jobs", "worker-1")

if message:
    # Process
    process(message.payload)
    
    # ACK
    client.queue.ack("jobs", message.message_id)
    
    # Or NACK
    # client.queue.nack("jobs", message.message_id)
```

### TypeScript

```typescript
import { Synap } from "@hivehub/synap";

const client = new SynapClient("http://localhost:15500");

// Consume
const message = await client.queue.consume("jobs", "worker-1");

if (message) {
    // Process
    processMessage(message.payload);
    
    // ACK
    await client.queue.ack("jobs", message.messageId);
    
    // Or NACK
    // await client.queue.nack("jobs", message.messageId);
}
```

### Rust

```rust
use synap_sdk::SynapClient;

let client = SynapClient::new("http://localhost:15500")?;

// Consume
if let Some(message) = client.queue.consume("jobs", "worker-1").await? {
    // Process
    process_message(&message.payload)?;
    
    // ACK
    client.queue.ack("jobs", &message.message_id).await?;
    
    // Or NACK
    // client.queue.nack("jobs", &message.message_id).await?;
}
```

## Best Practices

### Always ACK or NACK

Never leave messages unacknowledged. Always ACK on success or NACK on error.

### Handle Errors Gracefully

```python
try:
    result = process_message(message.payload)
    client.queue.ack("jobs", message.message_id)
except RetryableError as e:
    # Retry later
    client.queue.nack("jobs", message.message_id)
except PermanentError as e:
    # Don't retry, move to DLQ
    client.queue.ack("jobs", message.message_id)
    log_error(e)
```

### Monitor Queue Depth

```bash
# Check queue statistics regularly
curl http://localhost:15500/queue/jobs/stats
```

### Use Appropriate ACK Deadline

Set ACK deadline based on processing time:
- Short tasks: 30 seconds
- Medium tasks: 60-120 seconds
- Long tasks: 300+ seconds

## Related Topics

- [Creating Queues](./CREATING.md) - Queue creation
- [Publishing Messages](./PUBLISHING.md) - Publishing messages
- [Complete Queues Guide](./QUEUES.md) - Comprehensive reference

