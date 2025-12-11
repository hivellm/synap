---
title: Creating Streams
module: streams
id: streams-creating
order: 1
description: How to create and configure streams
tags: [streams, creating, configuration]
---

# Creating Streams

How to create and configure event streams in Synap.

## Basic Stream Creation

### Create Simple Stream

```bash
curl -X POST http://localhost:15500/stream/notifications
```

Creates a stream with default settings.

### Create with Configuration

```bash
curl -X POST http://localhost:15500/stream/notifications \
  -H "Content-Type: application/json" \
  -d '{
    "partitions": 1,
    "retention_hours": 24
  }'
```

## Configuration Options

### Partitions

Number of partitions for horizontal scaling:

```bash
curl -X POST http://localhost:15500/stream/notifications \
  -H "Content-Type: application/json" \
  -d '{
    "partitions": 4
  }'
```

More partitions = higher throughput, but more complexity.

### Retention

How long to keep events (in hours):

```bash
curl -X POST http://localhost:15500/stream/notifications \
  -H "Content-Type: application/json" \
  -d '{
    "retention_hours": 168
  }'
```

Default: 24 hours. Set to 0 for unlimited retention.

## Stream Management

### List Streams

```bash
curl http://localhost:15500/stream/list
```

**Response:**
```json
{
  "streams": ["notifications", "events", "chat-room-1"]
}
```

### Get Stream Info

```bash
curl http://localhost:15500/stream/notifications/info
```

**Response:**
```json
{
  "name": "notifications",
  "partitions": 1,
  "retention_hours": 24,
  "message_count": 156,
  "min_offset": 0,
  "max_offset": 155
}
```

### Get Stream Statistics

```bash
curl http://localhost:15500/stream/notifications/stats
```

**Response:**
```json
{
  "message_count": 156,
  "subscribers": 3,
  "min_offset": 0,
  "max_offset": 155,
  "partitions": [
    {
      "partition": 0,
      "message_count": 156,
      "min_offset": 0,
      "max_offset": 155
    }
  ]
}
```

### Delete Stream

```bash
curl -X DELETE http://localhost:15500/stream/notifications
```

**Warning:** This permanently deletes the stream and all events.

## Consumer Groups

### Create Consumer Group

```bash
curl -X POST http://localhost:15500/stream/notifications/group/email-service
```

### List Consumer Groups

```bash
curl http://localhost:15500/stream/notifications/groups
```

**Response:**
```json
{
  "groups": ["email-service", "sms-service", "push-service"]
}
```

### Get Consumer Group Info

```bash
curl http://localhost:15500/stream/notifications/group/email-service/info
```

**Response:**
```json
{
  "group": "email-service",
  "consumers": ["consumer-1", "consumer-2"],
  "pending_messages": 5
}
```

## Using SDKs

### Python

```python
from synap_sdk import SynapClient

client = SynapClient("http://localhost:15500")

# Create stream
client.stream.create("notifications", partitions=1, retention_hours=24)

# List streams
streams = client.stream.list()

# Get stream info
info = client.stream.info("notifications")

# Get statistics
stats = client.stream.stats("notifications")
```

### TypeScript

```typescript
import { Synap } from "@hivehub/synap";

const client = new SynapClient("http://localhost:15500");

// Create stream
await client.stream.create("notifications", {
  partitions: 1,
  retentionHours: 24
});

// List streams
const streams = await client.stream.list();

// Get stream info
const info = await client.stream.info("notifications");

// Get statistics
const stats = await client.stream.stats("notifications");
```

### Rust

```rust
use synap_sdk::SynapClient;

let client = SynapClient::new("http://localhost:15500")?;

// Create stream
client.stream.create("notifications", 1, 24).await?;

// List streams
let streams = client.stream.list().await?;

// Get stream info
let info = client.stream.info("notifications").await?;

// Get statistics
let stats = client.stream.stats("notifications").await?;
```

## Best Practices

### Choose Appropriate Partitions

- **Single partition**: For low-volume, ordered events
- **Multiple partitions**: For high-volume, parallel processing
- **Rule of thumb**: 1 partition per 10K events/second

### Set Retention Based on Use Case

- **Real-time only**: 1-6 hours
- **Replay needed**: 24-168 hours
- **Audit/logging**: 0 (unlimited) with external storage

### Use Consumer Groups for Multiple Consumers

Consumer groups allow multiple consumers to share the load:

```bash
# Create consumer group
curl -X POST http://localhost:15500/stream/notifications/group/email-service

# Each consumer in the group gets different messages
```

## Related Topics

- [Publishing Events](./PUBLISHING.md) - Publishing to streams
- [Consuming Events](./CONSUMING.md) - Consuming from streams
- [Complete Streams Guide](./STREAMS.md) - Comprehensive reference

