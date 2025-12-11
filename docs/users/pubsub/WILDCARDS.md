---
title: Wildcard Patterns
module: pubsub
id: pubsub-wildcards
order: 3
description: Pattern matching with wildcards in pub/sub topics
tags: [pubsub, wildcards, patterns, matching]
---

# Wildcard Patterns

How to use wildcards for flexible topic subscriptions in Synap pub/sub.

## Wildcard Types

### Single-Level Wildcard (`*`)

Matches exactly one level in the topic hierarchy.

**Examples:**
- `notifications.*` matches:
  - `notifications.email`
  - `notifications.sms`
  - `notifications.push`
  - Does NOT match: `notifications.email.urgent`

### Multi-Level Wildcard (`#`)

Matches zero or more levels in the topic hierarchy.

**Examples:**
- `events.user.#` matches:
  - `events.user`
  - `events.user.login`
  - `events.user.login.success`
  - `events.user.profile.update`
  - Does NOT match: `events.order`

## Pattern Examples

### Notification System

```javascript
// Subscribe to all notification types
const ws1 = new WebSocket('ws://localhost:15500/pubsub/ws?topics=notifications.*');

// Subscribe to all email notifications (any level)
const ws2 = new WebSocket('ws://localhost:15500/pubsub/ws?topics=notifications.email.#');

// Subscribe to all notifications (any type, any level)
const ws3 = new WebSocket('ws://localhost:15500/pubsub/ws?topics=notifications.#');
```

### Event System

```javascript
// Subscribe to all order events
const ws1 = new WebSocket('ws://localhost:15500/pubsub/ws?topics=events.order.*');

// Subscribe to all user events (any level)
const ws2 = new WebSocket('ws://localhost:15500/pubsub/ws?topics=events.user.#');

// Subscribe to all events
const ws3 = new WebSocket('ws://localhost:15500/pubsub/ws?topics=events.#');
```

### Hierarchical Topics

```javascript
// Subscribe to all payment events
const ws1 = new WebSocket('ws://localhost:15500/pubsub/ws?topics=payment.*');

// Subscribe to all payment success events (any level)
const ws2 = new WebSocket('ws://localhost:15500/pubsub/ws?topics=payment.success.#');

// Subscribe to all payment events (any level)
const ws3 = new WebSocket('ws://localhost:15500/pubsub/ws?topics=payment.#');
```

## Multiple Patterns

### Combine Patterns

```javascript
// Subscribe to multiple patterns
const topics = [
  'notifications.email',
  'notifications.sms',
  'events.order.*',
  'events.user.#'
];
const ws = new WebSocket(`ws://localhost:15500/pubsub/ws?topics=${topics.join(',')}`);
```

### Exclude Patterns

Wildcards don't support exclusion. Use multiple subscriptions and filter:

```javascript
const ws = new WebSocket('ws://localhost:15500/pubsub/ws?topics=events.*');

ws.onmessage = (event) => {
  const msg = JSON.parse(event.data);
  
  // Filter out unwanted topics
  if (!msg.topic.startsWith('events.debug.')) {
    handleMessage(msg);
  }
};
```

## Real-World Examples

### E-Commerce System

```javascript
// Order service - all order events
const orderWs = new WebSocket('ws://localhost:15500/pubsub/ws?topics=events.order.#');

// Payment service - payment events only
const paymentWs = new WebSocket('ws://localhost:15500/pubsub/ws?topics=events.payment.*');

// Analytics - all events
const analyticsWs = new WebSocket('ws://localhost:15500/pubsub/ws?topics=events.#');
```

### Notification System

```javascript
// Email service - email notifications
const emailWs = new WebSocket('ws://localhost:15500/pubsub/ws?topics=notifications.email.*');

// SMS service - SMS notifications
const smsWs = new WebSocket('ws://localhost:15500/pubsub/ws?topics=notifications.sms.*');

// Push service - push notifications
const pushWs = new WebSocket('ws://localhost:15500/pubsub/ws?topics=notifications.push.*');

// All notifications
const allWs = new WebSocket('ws://localhost:15500/pubsub/ws?topics=notifications.#');
```

### Microservices

```javascript
// User service - user-related events
const userWs = new WebSocket('ws://localhost:15500/pubsub/ws?topics=events.user.#');

// Product service - product-related events
const productWs = new WebSocket('ws://localhost:15500/pubsub/ws?topics=events.product.*');

// All services - all events
const allEventsWs = new WebSocket('ws://localhost:15500/pubsub/ws?topics=events.#');
```

## Best Practices

### Use Hierarchical Topics

Organize topics hierarchically for better wildcard matching:

```
events.order.created
events.order.paid
events.order.shipped
events.user.signup
events.user.login
```

### Prefer Specific Topics

Use specific topics when possible, wildcards when needed:

```javascript
// Good: Specific
const ws1 = new WebSocket('ws://localhost:15500/pubsub/ws?topics=notifications.email');

// Also good: Wildcard when needed
const ws2 = new WebSocket('ws://localhost:15500/pubsub/ws?topics=notifications.*');
```

### Combine with Filtering

Use wildcards for broad subscription, filter in code:

```javascript
const ws = new WebSocket('ws://localhost:15500/pubsub/ws?topics=events.*');

ws.onmessage = (event) => {
  const msg = JSON.parse(event.data);
  
  // Additional filtering
  if (msg.topic === 'events.order.created' || msg.topic === 'events.order.paid') {
    handleOrderEvent(msg);
  }
};
```

## Pattern Matching Rules

### Single-Level (`*`)

- Matches exactly one level
- Does NOT match empty level
- Does NOT match multiple levels

**Examples:**
- `a.*` matches `a.b` but NOT `a` or `a.b.c`
- `a.*.c` matches `a.b.c` but NOT `a.c` or `a.b.d.c`

### Multi-Level (`#`)

- Matches zero or more levels
- Must be at end of pattern
- Can match empty

**Examples:**
- `a.#` matches `a`, `a.b`, `a.b.c`
- `a.b.#` matches `a.b`, `a.b.c`, `a.b.c.d`
- `#` matches everything

## Related Topics

- [Publishing to Topics](./PUBLISHING.md) - Publishing messages
- [Subscribing to Topics](./SUBSCRIBING.md) - WebSocket subscriptions
- [Complete Pub/Sub Guide](./PUBSUB.md) - Comprehensive reference

