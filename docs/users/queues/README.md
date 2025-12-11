---
title: Message Queues
module: queues
id: queues-index
order: 0
description: RabbitMQ-style message queues with ACK/NACK
tags: [queues, messaging, rabbitmq, job-queue]
---

# Message Queues

Complete guide to Synap's RabbitMQ-style message queues.

## Guides

### [Creating Queues](./CREATING.md)

How to create and configure queues:

- Queue creation
- Configuration options
- Max depth and retries
- ACK deadline

### [Publishing Messages](./PUBLISHING.md)

Publishing messages to queues:

- Basic publishing
- Priority levels (0-9)
- Message payloads
- Retry configuration

### [Consuming Messages](./CONSUMING.md)

Consuming and acknowledging messages:

- Consumer patterns
- ACK/NACK operations
- Dead letter queue (DLQ)
- Error handling

### [Complete Queues Guide](./QUEUES.md)

Comprehensive reference:

- All operations
- Best practices
- Performance tips
- Examples

## Quick Start

### Create Queue

```bash
curl -X POST http://localhost:15500/queue/jobs \
  -H "Content-Type: application/json" \
  -d '{
    "max_depth": 1000,
    "ack_deadline_secs": 30
  }'
```

### Publish Message

```bash
curl -X POST http://localhost:15500/queue/jobs/publish \
  -H "Content-Type: application/json" \
  -d '{
    "payload": [72,101,108,108,111],
    "priority": 5
  }'
```

### Consume Message

```bash
curl http://localhost:15500/queue/jobs/consume/worker-1
```

### Acknowledge Message

```bash
curl -X POST http://localhost:15500/queue/jobs/ack \
  -H "Content-Type: application/json" \
  -d '{"message_id":"<message_id>"}'
```

## Features

- **Priority Support** - Messages with priority 0-9 (9 = highest)
- **ACK/NACK** - Acknowledge or reject messages
- **Retry Logic** - Automatic retries with configurable max attempts
- **Dead Letter Queue** - Failed messages moved to DLQ
- **Consumer Groups** - Multiple consumers per queue
- **Statistics** - Track pending, in-flight, and DLQ messages

## Related Topics

- [API Reference](../api/API_REFERENCE.md) - Complete API documentation
- [Use Cases](../use-cases/BACKGROUND_JOBS.md) - Background jobs example
- [Configuration Guide](../configuration/CONFIGURATION.md) - Queue configuration

