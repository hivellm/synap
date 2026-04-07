---
title: Pub/Sub
module: pubsub
id: pubsub-index
order: 0
description: Topic-based messaging with wildcard support
tags: [pubsub, messaging, topics, wildcards]
---

# Pub/Sub

Complete guide to Synap's topic-based pub/sub messaging.

## Guides

### [Publishing](./PUBLISHING.md)

Publishing messages to topics:

- Basic publishing
- Topic naming
- Message format

### [Subscribing](./SUBSCRIBING.md)

Subscribing to topics:

- WebSocket subscriptions
- Multiple topics
- Connection management

### [Wildcards](./WILDCARDS.md)

Pattern matching:

- Single-level wildcard (`*`)
- Multi-level wildcard (`#`)
- Pattern examples

### [Complete Pub/Sub Guide](./PUBSUB.md)

Comprehensive reference:

- All operations
- Best practices
- Performance tips
- Examples

## Quick Start

### Publish to Topic

```bash
curl -X POST http://localhost:15500/pubsub/notifications.email/publish \
  -H "Content-Type: application/json" \
  -d '{"message":"New order received"}'
```

### Subscribe with WebSocket

```javascript
const ws = new WebSocket('ws://localhost:15500/pubsub/ws?topics=notifications.email');

ws.onmessage = (event) => {
  const msg = JSON.parse(event.data);
  console.log('Topic:', msg.topic);
  console.log('Message:', msg.message);
};
```

### Wildcard Subscriptions

```javascript
// Single-level wildcard
const ws1 = new WebSocket('ws://localhost:15500/pubsub/ws?topics=notifications.*');

// Multi-level wildcard
const ws2 = new WebSocket('ws://localhost:15500/pubsub/ws?topics=events.user.#');

// Multiple topics
const ws3 = new WebSocket('ws://localhost:15500/pubsub/ws?topics=events.user.#,notifications.*');
```

## Features

- **Topic-Based** - Organize messages by topic
- **Wildcards** - Pattern matching for flexible subscriptions
- **WebSocket** - Real-time delivery
- **Multiple Topics** - Subscribe to multiple topics
- **No Persistence** - Fire-and-forget messaging

## Related Topics

- [API Reference](../api/API_REFERENCE.md) - Complete API documentation
- [Use Cases](../use-cases/EVENT_BROADCASTING.md) - Event broadcasting example
- [Configuration Guide](../configuration/CONFIGURATION.md) - Pub/Sub configuration

