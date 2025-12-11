---
title: Publishing to Topics
module: pubsub
id: pubsub-publishing
order: 1
description: Publishing messages to pub/sub topics
tags: [pubsub, publishing, topics]
---

# Publishing to Topics

How to publish messages to Synap pub/sub topics.

## Basic Publishing

### Publish to Topic

```bash
curl -X POST http://localhost:15500/pubsub/notifications.email/publish \
  -H "Content-Type: application/json" \
  -d '{
    "message": "New order received"
  }'
```

### Publish JSON Message

```bash
curl -X POST http://localhost:15500/pubsub/events.order.created/publish \
  -H "Content-Type: application/json" \
  -d '{
    "message": "{\"order_id\":123,\"total\":99.99}"
  }'
```

## Topic Naming

### Hierarchical Topics

Use dots (`.`) to create hierarchical topics:

```bash
# Order events
curl -X POST http://localhost:15500/pubsub/events.order.created/publish \
  -H "Content-Type: application/json" \
  -d '{"message": "Order created"}'

curl -X POST http://localhost:15500/pubsub/events.order.paid/publish \
  -H "Content-Type: application/json" \
  -d '{"message": "Order paid"}'

# Notification events
curl -X POST http://localhost:15500/pubsub/notifications.email/publish \
  -H "Content-Type: application/json" \
  -d '{"message": "Email sent"}'

curl -X POST http://localhost:15500/pubsub/notifications.sms/publish \
  -H "Content-Type: application/json" \
  -d '{"message": "SMS sent"}'
```

### Best Practices

- Use hierarchical naming: `category.subcategory.event`
- Keep topic names short but descriptive
- Use consistent naming conventions

## Message Format

### String Message

```bash
curl -X POST http://localhost:15500/pubsub/notifications.email/publish \
  -H "Content-Type: application/json" \
  -d '{
    "message": "New order received"
  }'
```

### JSON Message

```bash
curl -X POST http://localhost:15500/pubsub/events.order.created/publish \
  -H "Content-Type: application/json" \
  -d '{
    "message": "{\"order_id\":123,\"user_id\":456,\"total\":99.99}"
  }'
```

## Response

### Success Response

```json
{
  "success": true,
  "subscribers": 3
}
```

Returns number of active subscribers.

### No Subscribers

```json
{
  "success": true,
  "subscribers": 0
}
```

Message is published but no one is listening.

## Using SDKs

### Python

```python
from synap_sdk import SynapClient

client = SynapClient("http://localhost:15500")

# Publish string message
client.pubsub.publish("notifications.email", "New order received")

# Publish JSON message
import json
message = json.dumps({"order_id": 123, "total": 99.99})
client.pubsub.publish("events.order.created", message)
```

### TypeScript

```typescript
import { Synap } from "@hivehub/synap";

const client = new SynapClient("http://localhost:15500");

// Publish string message
await client.pubsub.publish("notifications.email", "New order received");

// Publish JSON message
const message = JSON.stringify({ orderId: 123, total: 99.99 });
await client.pubsub.publish("events.order.created", message);
```

### Rust

```rust
use synap_sdk::SynapClient;

let client = SynapClient::new("http://localhost:15500")?;

// Publish string message
client.pubsub.publish("notifications.email", "New order received").await?;

// Publish JSON message
let message = serde_json::json!({"order_id": 123, "total": 99.99}).to_string();
client.pubsub.publish("events.order.created", &message).await?;
```

## Publishing Patterns

### Event-Driven Architecture

```python
# Order service publishes events
client.pubsub.publish("events.order.created", order_data)
client.pubsub.publish("events.order.paid", payment_data)
client.pubsub.publish("events.order.shipped", shipping_data)
```

### Notification System

```python
# Notification service publishes to different channels
client.pubsub.publish("notifications.email", email_data)
client.pubsub.publish("notifications.sms", sms_data)
client.pubsub.publish("notifications.push", push_data)
```

## Best Practices

### Use Hierarchical Topics

Organize topics by category and subcategory:
- `events.order.*` - All order events
- `notifications.email` - Email notifications
- `system.alerts` - System alerts

### Keep Messages Small

Pub/Sub is fire-and-forget. Keep messages under 1MB for best performance.

### Monitor Subscribers

Check subscriber count in response to ensure messages are being received.

## Related Topics

- [Subscribing](./SUBSCRIBING.md) - Subscribing to topics
- [Wildcards](./WILDCARDS.md) - Pattern matching
- [Complete Pub/Sub Guide](./PUBSUB.md) - Comprehensive reference

