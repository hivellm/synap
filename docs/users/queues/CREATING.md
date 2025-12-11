---
title: Creating Queues
module: queues
id: queues-creating
order: 1
description: How to create and configure queues
tags: [queues, creating, configuration]
---

# Creating Queues

How to create and configure message queues in Synap.

## Basic Queue Creation

### Create Simple Queue

```bash
curl -X POST http://localhost:15500/queue/jobs \
  -H "Content-Type: application/json" \
  -d '{}'
```

Creates a queue with default settings.

### Create with Configuration

```bash
curl -X POST http://localhost:15500/queue/jobs \
  -H "Content-Type: application/json" \
  -d '{
    "max_depth": 1000,
    "ack_deadline_secs": 30,
    "default_max_retries": 3
  }'
```

## Configuration Options

### Max Depth

Maximum number of messages in queue:

```bash
curl -X POST http://localhost:15500/queue/jobs \
  -H "Content-Type: application/json" \
  -d '{
    "max_depth": 10000
  }'
```

When queue is full, new messages are rejected.

### ACK Deadline

Time in seconds before unacknowledged messages are returned to queue:

```bash
curl -X POST http://localhost:15500/queue/jobs \
  -H "Content-Type: application/json" \
  -d '{
    "ack_deadline_secs": 60
  }'
```

Default: 30 seconds.

### Max Retries

Default maximum retry attempts for messages:

```bash
curl -X POST http://localhost:15500/queue/jobs \
  -H "Content-Type: application/json" \
  -d '{
    "default_max_retries": 5
  }'
```

Messages exceeding max retries are moved to Dead Letter Queue (DLQ).

## Queue Management

### List Queues

```bash
curl http://localhost:15500/queue/list
```

**Response:**
```json
{
  "queues": ["jobs", "notifications", "tasks"]
}
```

### Get Queue Info

```bash
curl http://localhost:15500/queue/jobs/info
```

**Response:**
```json
{
  "name": "jobs",
  "max_depth": 1000,
  "ack_deadline_secs": 30,
  "default_max_retries": 3
}
```

### Get Queue Statistics

```bash
curl http://localhost:15500/queue/jobs/stats
```

**Response:**
```json
{
  "pending": 42,
  "in_flight": 5,
  "dlq_count": 0,
  "total_published": 1000,
  "total_consumed": 955
}
```

### Delete Queue

```bash
curl -X DELETE http://localhost:15500/queue/jobs
```

**Warning:** This permanently deletes the queue and all messages.

### Purge Queue

```bash
curl -X POST http://localhost:15500/queue/jobs/purge
```

Removes all messages but keeps the queue.

## Using SDKs

### Python

```python
from synap_sdk import SynapClient

client = SynapClient("http://localhost:15500")

# Create queue
client.queue.create("jobs", max_depth=1000, ack_deadline_secs=30)

# List queues
queues = client.queue.list()

# Get queue info
info = client.queue.info("jobs")

# Get statistics
stats = client.queue.stats("jobs")
```

### TypeScript

```typescript
import { Synap } from "@hivehub/synap";

const client = new SynapClient("http://localhost:15500");

// Create queue
await client.queue.create("jobs", {
  maxDepth: 1000,
  ackDeadlineSecs: 30
});

// List queues
const queues = await client.queue.list();

// Get queue info
const info = await client.queue.info("jobs");

// Get statistics
const stats = await client.queue.stats("jobs");
```

### Rust

```rust
use synap_sdk::SynapClient;

let client = SynapClient::new("http://localhost:15500")?;

// Create queue
client.queue.create("jobs", 1000, 30).await?;

// List queues
let queues = client.queue.list().await?;

// Get queue info
let info = client.queue.info("jobs").await?;

// Get statistics
let stats = client.queue.stats("jobs").await?;
```

## Best Practices

### Choose Appropriate Max Depth

- **Small queues** (100-1000): For low-volume, high-priority tasks
- **Medium queues** (1000-10000): For typical workloads
- **Large queues** (10000+): For high-volume, batch processing

### Set ACK Deadline Based on Processing Time

- **Short tasks** (< 10s): 30 seconds
- **Medium tasks** (10-60s): 60-120 seconds
- **Long tasks** (> 60s): 300+ seconds

### Configure Retries Appropriately

- **Idempotent operations**: 5-10 retries
- **Non-idempotent operations**: 1-3 retries
- **Critical operations**: Monitor DLQ closely

## Related Topics

- [Publishing Messages](./PUBLISHING.md) - Publishing to queues
- [Consuming Messages](./CONSUMING.md) - Consuming from queues
- [Complete Queues Guide](./QUEUES.md) - Comprehensive reference

