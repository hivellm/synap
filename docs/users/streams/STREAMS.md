---
title: Complete Streams Guide
module: streams
id: streams-complete
order: 4
description: Comprehensive event streams reference
tags: [streams, reference, complete, guide]
---

# Complete Streams Guide

Comprehensive reference for Synap's event stream operations.

## Overview

Synap provides Kafka-style event streams with:
- **Offset-Based**: Track consumption position
- **Consumer Groups**: Multiple consumers per stream
- **WebSocket Support**: Real-time streaming
- **Partitioning**: Scale horizontally
- **Ordering**: Maintain event order
- **History**: Replay from any offset

## Stream Lifecycle

### Create Stream

```bash
curl -X POST http://localhost:15500/stream/notifications \
  -H "Content-Type: application/json" \
  -d '{
    "partitions": 1,
    "retention_hours": 24
  }'
```

### List Streams

```bash
curl http://localhost:15500/stream/list
```

### Get Stream Info

```bash
curl http://localhost:15500/stream/notifications/info
```

### Delete Stream

```bash
curl -X DELETE http://localhost:15500/stream/notifications
```

## Publishing Events

### Basic Publish

```bash
curl -X POST http://localhost:15500/stream/notifications/publish \
  -H "Content-Type: application/json" \
  -d '{
    "event": "user.signup",
    "data": "New user registered"
  }'
```

### With Partition Key

```bash
curl -X POST http://localhost:15500/stream/notifications/publish \
  -H "Content-Type: application/json" \
  -d '{
    "event": "user.signup",
    "data": "New user",
    "partition_key": "user-123"
  }'
```

## Consuming Events

### Consume Events

```bash
curl "http://localhost:15500/stream/notifications/consume/user-1?from_offset=0&limit=10"
```

### WebSocket Streaming

```javascript
const ws = new WebSocket('ws://localhost:15500/stream/notifications/ws/user-1?from_offset=0');

ws.onmessage = (event) => {
  const msg = JSON.parse(event.data);
  console.log('Event:', msg.event, 'Data:', msg.data);
};
```

## Consumer Groups

### Create Consumer Group

```bash
curl -X POST http://localhost:15500/stream/notifications/group/email-service
```

### Consume with Group

```bash
curl "http://localhost:15500/stream/notifications/group/email-service/consume?limit=10"
```

### List Consumer Groups

```bash
curl http://localhost:15500/stream/notifications/groups
```

## Offset Management

### Start from Beginning

```bash
curl "http://localhost:15500/stream/notifications/consume/user-1?from_offset=0&limit=100"
```

### Continue from Last Position

```python
last_offset = get_last_offset("user-1")
events = client.stream.consume("notifications", "user-1", from_offset=last_offset + 1, limit=10)
```

### Get Current Offset

```bash
curl http://localhost:15500/stream/notifications/stats
```

## Statistics

### Get Stream Stats

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

## Partitioning

### Automatic Partitioning

Events are distributed based on partition key:

```python
# Events with same partition key go to same partition
client.stream.publish("notifications", "event", data, partition_key="user-123")
```

### Round-Robin

If no partition key, events distributed round-robin.

## Best Practices

### Use Partition Keys for Ordering

```python
# Maintain order per user
client.stream.publish("events", "order.created", data, partition_key=f"user-{user_id}")
```

### Store Offset Persistently

```python
# Store offset in database or file
save_offset("user-1", event.offset + 1)
```

### Handle Reconnection

```javascript
let lastOffset = 0;

function connect() {
  const ws = new WebSocket(`ws://localhost:15500/stream/notifications/ws/user-1?from_offset=${lastOffset}`);
  
  ws.onmessage = (event) => {
    const msg = JSON.parse(event.data);
    lastOffset = msg.offset + 1;
    handleMessage(msg);
  };
  
  ws.onclose = () => {
    setTimeout(connect, 1000);
  };
}
```

## Related Topics

- [Creating Streams](./CREATING.md) - Stream creation
- [Publishing Events](./PUBLISHING.md) - Publishing events
- [Consuming Events](./CONSUMING.md) - Consuming events

