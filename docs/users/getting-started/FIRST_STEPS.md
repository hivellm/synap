---
title: First Steps
module: first-steps
id: first-steps-guide
order: 2
description: Complete guide after installation
tags: [first-steps, tutorial, getting-started]
---

# First Steps

Complete guide to get started with Synap after installation.

## Step 1: Verify Installation

```bash
# Check health
curl http://localhost:15500/health

# Expected output:
# {"status":"healthy","uptime_secs":5}

# Get server info
curl http://localhost:15500/info

# Get statistics
curl http://localhost:15500/kv/stats
```

## Step 2: Create Your First Key-Value Entry

```bash
# Set a key with value
curl -X POST http://localhost:15500/kv/set \
  -H "Content-Type: application/json" \
  -d '{
    "key": "greeting",
    "value": "Hello, Synap!"
  }'

# Get the value
curl http://localhost:15500/kv/get/greeting

# Output: "Hello, Synap!"

# Set with TTL (expires in 1 hour)
curl -X POST http://localhost:15500/kv/set \
  -H "Content-Type: application/json" \
  -d '{
    "key": "session:user123",
    "value": "{\"user_id\":123,\"role\":\"admin\"}",
    "ttl": 3600
  }'
```

## Step 3: Publish Your First Queue Message

```bash
# Create a queue
curl -X POST http://localhost:15500/queue/jobs \
  -H "Content-Type: application/json" \
  -d '{
    "max_depth": 1000,
    "ack_deadline_secs": 30
  }'

# Publish a message
curl -X POST http://localhost:15500/queue/jobs/publish \
  -H "Content-Type: application/json" \
  -d '{
    "payload": [72,101,108,108,111],
    "priority": 5
  }'

# Consume the message
curl http://localhost:15500/queue/jobs/consume/worker-1

# Acknowledge the message
curl -X POST http://localhost:15500/queue/jobs/ack \
  -H "Content-Type: application/json" \
  -d '{"message_id":"<message_id_from_consume>"}'
```

## Step 4: Consume Your First Stream Event

```bash
# Create a stream
curl -X POST http://localhost:15500/stream/notifications

# Publish an event
curl -X POST http://localhost:15500/stream/notifications/publish \
  -H "Content-Type: application/json" \
  -d '{
    "event": "user.signup",
    "data": "New user registered"
  }'

# Consume events
curl "http://localhost:15500/stream/notifications/consume/user-1?from_offset=0&limit=10"
```

## Step 5: Subscribe to Pub/Sub

```bash
# Publish to topic
curl -X POST http://localhost:15500/pubsub/events.order.created/publish \
  -H "Content-Type: application/json" \
  -d '{"message":"Order #123 created"}'
```

**Subscribe with WebSocket:**
```javascript
const ws = new WebSocket('ws://localhost:15500/pubsub/ws?topics=events.order.*');

ws.onmessage = (event) => {
  const msg = JSON.parse(event.data);
  console.log('Topic:', msg.topic);
  console.log('Message:', msg.message);
};
```

## Common Operations

### Check Statistics

```bash
# KV store stats
curl http://localhost:15500/kv/stats

# Queue stats
curl http://localhost:15500/queue/jobs/stats

# Stream stats
curl http://localhost:15500/stream/notifications/stats
```

### List Resources

```bash
# List all queues
curl http://localhost:15500/queue/list

# List all streams
curl http://localhost:15500/stream/list
```

## Next Steps

1. **[Basic KV Operations](../kv-store/BASIC.md)** - Learn key-value operations
2. **[Message Queues](../queues/CREATING.md)** - Learn about queues
3. **[Event Streams](../streams/CREATING.md)** - Learn about streams
4. **[Pub/Sub](../pubsub/PUBLISHING.md)** - Learn about pub/sub
5. **[Use Cases](../use-cases/)** - See real-world examples

## Related Topics

- [Quick Start Guide](./QUICK_START.md) - Quick start tutorial
- [Configuration Guide](../configuration/CONFIGURATION.md) - Configure Synap
- [API Reference](../api/API_REFERENCE.md) - Complete API documentation

