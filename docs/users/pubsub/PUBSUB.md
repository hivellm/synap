---
title: Complete Pub/Sub Guide
module: pubsub
id: pubsub-complete
order: 4
description: Comprehensive pub/sub messaging reference
tags: [pubsub, reference, complete, guide]
---

# Complete Pub/Sub Guide

Comprehensive reference for Synap's pub/sub messaging.

## Overview

Synap provides topic-based pub/sub messaging with:
- **Topic-Based**: Organize messages by topic
- **Wildcards**: Pattern matching for flexible subscriptions
- **WebSocket**: Real-time delivery
- **Multiple Topics**: Subscribe to multiple topics
- **Fire-and-Forget**: No persistence

## Publishing

### Publish to Topic

```bash
curl -X POST http://localhost:15500/pubsub/notifications.email/publish \
  -H "Content-Type: application/json" \
  -d '{"message":"New order received"}'
```

### Publish JSON

```bash
curl -X POST http://localhost:15500/pubsub/events.order.created/publish \
  -H "Content-Type: application/json" \
  -d '{"message":"{\"order_id\":123,\"total\":99.99}"}'
```

## Subscribing

### Single Topic

```javascript
const ws = new WebSocket('ws://localhost:15500/pubsub/ws?topics=notifications.email');

ws.onmessage = (event) => {
  const msg = JSON.parse(event.data);
  console.log('Topic:', msg.topic);
  console.log('Message:', msg.message);
};
```

### Multiple Topics

```javascript
const topics = ['notifications.email', 'notifications.sms', 'events.order.*'];
const ws = new WebSocket(`ws://localhost:15500/pubsub/ws?topics=${topics.join(',')}`);
```

## Wildcards

### Single-Level (`*`)

Matches exactly one level.

```javascript
// Matches: notifications.email, notifications.sms
// Does NOT match: notifications.email.urgent
const ws = new WebSocket('ws://localhost:15500/pubsub/ws?topics=notifications.*');
```

### Multi-Level (`#`)

Matches zero or more levels.

```javascript
// Matches: events.user, events.user.login, events.user.login.success
const ws = new WebSocket('ws://localhost:15500/pubsub/ws?topics=events.user.#');
```

## Message Format

### Received Message

```json
{
  "topic": "notifications.email",
  "message": "New order received",
  "timestamp": 1234567890
}
```

### Parse JSON Messages

```javascript
ws.onmessage = (event) => {
  const msg = JSON.parse(event.data);
  
  let data;
  try {
    data = JSON.parse(msg.message);
  } catch (e) {
    data = msg.message;
  }
  
  handleMessage(msg.topic, data);
};
```

## Connection Management

### Reconnect Logic

```javascript
function createSubscription(topics, onMessage) {
  let ws;
  let reconnectDelay = 1000;
  
  function connect() {
    ws = new WebSocket(`ws://localhost:15500/pubsub/ws?topics=${topics.join(',')}`);
    
    ws.onopen = () => {
      reconnectDelay = 1000;
    };
    
    ws.onmessage = (event) => {
      const msg = JSON.parse(event.data);
      onMessage(msg);
    };
    
    ws.onclose = () => {
      setTimeout(connect, reconnectDelay);
      reconnectDelay = Math.min(reconnectDelay * 2, 30000);
    };
  }
  
  connect();
  return () => ws.close();
}
```

## Best Practices

### Use Hierarchical Topics

Organize topics hierarchically:
- `events.order.created`
- `events.order.paid`
- `notifications.email`
- `notifications.sms`

### Keep Messages Small

Pub/Sub is fire-and-forget. Keep messages under 1MB.

### Use Wildcards for Flexibility

```javascript
// Subscribe to all notifications
const ws = new WebSocket('ws://localhost:15500/pubsub/ws?topics=notifications.*');

// Subscribe to all events
const ws2 = new WebSocket('ws://localhost:15500/pubsub/ws?topics=events.#');
```

## Related Topics

- [Publishing to Topics](./PUBLISHING.md) - Publishing messages
- [Subscribing to Topics](./SUBSCRIBING.md) - WebSocket subscriptions
- [Wildcards](./WILDCARDS.md) - Pattern matching

