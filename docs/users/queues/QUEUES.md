---
title: Complete Queues Guide
module: queues
id: queues-complete
order: 4
description: Comprehensive message queues reference
tags: [queues, reference, complete, guide]
---

# Complete Queues Guide

Comprehensive reference for Synap's message queue operations.

## Overview

Synap provides RabbitMQ-style message queues with:
- **Priority Support**: 0-9 (9 = highest)
- **ACK/NACK**: Acknowledge or reject messages
- **Retry Logic**: Automatic retries with configurable max attempts
- **Dead Letter Queue**: Failed messages moved to DLQ
- **Consumer Groups**: Multiple consumers per queue
- **High Throughput**: 44K+ ops/sec

## Queue Lifecycle

### Create Queue

```bash
curl -X POST http://localhost:15500/queue/jobs \
  -H "Content-Type: application/json" \
  -d '{
    "max_depth": 1000,
    "ack_deadline_secs": 30,
    "default_max_retries": 3
  }'
```

### List Queues

```bash
curl http://localhost:15500/queue/list
```

### Get Queue Info

```bash
curl http://localhost:15500/queue/jobs/info
```

### Delete Queue

```bash
curl -X DELETE http://localhost:15500/queue/jobs
```

## Publishing Messages

### Basic Publish

```bash
curl -X POST http://localhost:15500/queue/jobs/publish \
  -H "Content-Type: application/json" \
  -d '{
    "payload": [72, 101, 108, 108, 111],
    "priority": 5
  }'
```

### With Retries

```bash
curl -X POST http://localhost:15500/queue/jobs/publish \
  -H "Content-Type: application/json" \
  -d '{
    "payload": [72, 101, 108, 108, 111],
    "priority": 5,
    "max_retries": 3
  }'
```

## Consuming Messages

### Consume

```bash
curl http://localhost:15500/queue/jobs/consume/worker-1
```

### Acknowledge

```bash
curl -X POST http://localhost:15500/queue/jobs/ack \
  -H "Content-Type: application/json" \
  -d '{"message_id":"abc-123"}'
```

### Negative Acknowledge

```bash
curl -X POST http://localhost:15500/queue/jobs/nack \
  -H "Content-Type: application/json" \
  -d '{"message_id":"abc-123"}'
```

## Priority Levels

- **0-2**: Low priority (background tasks)
- **3-6**: Normal priority (typical operations)
- **7-9**: High priority (critical operations)

## Dead Letter Queue

### Check DLQ Count

```bash
curl http://localhost:15500/queue/jobs/stats
```

### Consume from DLQ

```bash
curl http://localhost:15500/queue/jobs/dlq/consume/worker-1
```

### Purge DLQ

```bash
curl -X POST http://localhost:15500/queue/jobs/dlq/purge
```

## Statistics

### Get Queue Stats

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

## Consumer Patterns

### Polling Pattern

```python
while True:
    message = client.queue.consume("jobs", "worker-1")
    if message:
        process(message)
        client.queue.ack("jobs", message.message_id)
    else:
        time.sleep(1)
```

### Idempotent Processing

```python
def is_processed(message_id):
    return client.kv.exists(f"processed:{message_id}")

message = client.queue.consume("jobs", "worker-1")
if message and not is_processed(message.message_id):
    process(message)
    client.kv.set(f"processed:{message.message_id}", "1", ttl=3600)
    client.queue.ack("jobs", message.message_id)
```

## Best Practices

### ACK Deadline

Set based on processing time:
- Short tasks: 30 seconds
- Medium tasks: 60-120 seconds
- Long tasks: 300+ seconds

### Retry Configuration

- Idempotent operations: 5-10 retries
- Non-idempotent: 1-3 retries
- Monitor DLQ regularly

### Monitor Queue Depth

```bash
# Check queue statistics
curl http://localhost:15500/queue/jobs/stats
```

If `pending` is consistently high:
- Add more consumers
- Increase processing capacity
- Review message priority distribution

## Related Topics

- [Creating Queues](./CREATING.md) - Queue creation
- [Publishing Messages](./PUBLISHING.md) - Publishing messages
- [Consuming Messages](./CONSUMING.md) - Consuming messages

