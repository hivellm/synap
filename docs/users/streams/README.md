---
title: Event Streams
module: streams
id: streams-index
order: 0
description: Kafka-style partitioned streams with consumer groups
tags: [streams, kafka, events, consumer-groups]
---

# Event Streams

Complete guide to Synap's Kafka-style event streams.

## Guides

### [Creating Streams](./CREATING.md)

How to create and configure streams:

- Stream creation
- Partition configuration
- Consumer groups

### [Publishing Events](./PUBLISHING.md)

Publishing events to streams:

- Basic publishing
- Event structure
- Partitioning
- Ordering guarantees

### [Consuming Events](./CONSUMING.md)

Consuming events from streams:

- Offset-based consumption
- Consumer groups
- WebSocket streaming
- Offset management

### [Complete Streams Guide](./STREAMS.md)

Comprehensive reference:

- All operations
- Best practices
- Performance tips
- Examples

## Quick Start

### Create Stream

```bash
curl -X POST http://localhost:15500/stream/notifications
```

### Publish Event

```bash
curl -X POST http://localhost:15500/stream/notifications/publish \
  -H "Content-Type: application/json" \
  -d '{
    "event": "user.signup",
    "data": "New user registered"
  }'
```

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

## Features

- **Offset-Based** - Track consumption position
- **Consumer Groups** - Multiple consumers per stream
- **WebSocket Support** - Real-time streaming
- **Partitioning** - Scale horizontally
- **Ordering** - Maintain event order
- **History** - Replay from any offset

## Related Topics

- [API Reference](../api/API_REFERENCE.md) - Complete API documentation
- [Use Cases](../use-cases/REAL_TIME_CHAT.md) - Real-time chat example
- [Configuration Guide](../configuration/CONFIGURATION.md) - Stream configuration

