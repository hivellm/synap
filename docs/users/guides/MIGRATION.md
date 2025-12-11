---
title: Migration Guide
module: guides
id: migration-guide
order: 7
description: Migrating from Redis, RabbitMQ, and Kafka to Synap
tags: [guides, migration, redis, rabbitmq, kafka]
---

# Migration Guide

Complete guide to migrating from Redis, RabbitMQ, and Kafka to Synap.

## Overview

Synap provides compatibility with:
- **Redis**: Key-value operations and data structures
- **RabbitMQ**: Message queues with ACK/NACK
- **Kafka**: Event streams with partitions

## Migrating from Redis

### Key-Value Operations

**Redis:**
```python
import redis
r = redis.Redis(host='localhost', port=6379)
r.set('key', 'value')
value = r.get('key')
```

**Synap:**
```python
from synap_sdk import SynapClient
client = SynapClient("http://localhost:15500")
client.kv.set('key', 'value')
value = client.kv.get('key')
```

### Hash Operations

**Redis:**
```python
r.hset('user:1', 'name', 'John')
name = r.hget('user:1', 'name')
```

**Synap:**
```python
client.hash.hset('user:1', 'name', 'John')
name = client.hash.hget('user:1', 'name')
```

### List Operations

**Redis:**
```python
r.lpush('tasks', 'task1')
task = r.rpop('tasks')
```

**Synap:**
```python
client.list.lpush('tasks', 'task1')
task = client.list.rpop('tasks')
```

### Set Operations

**Redis:**
```python
r.sadd('tags', 'tag1')
members = r.smembers('tags')
```

**Synap:**
```python
client.set.sadd('tags', 'tag1')
members = client.set.smembers('tags')
```

### Sorted Set Operations

**Redis:**
```python
r.zadd('leaderboard', {'player1': 100})
top = r.zrange('leaderboard', 0, 9)
```

**Synap:**
```python
client.sortedset.zadd('leaderboard', 'player1', 100)
top = client.sortedset.zrange('leaderboard', 0, 9)
```

## Migrating from RabbitMQ

### Queue Operations

**RabbitMQ:**
```python
import pika

connection = pika.BlockingConnection(pika.ConnectionParameters('localhost'))
channel = connection.channel()
channel.queue_declare(queue='jobs')
channel.basic_publish(exchange='', routing_key='jobs', body='Hello')
```

**Synap:**
```python
from synap_sdk import SynapClient

client = SynapClient("http://localhost:15500")
client.queue.create('jobs', max_depth=1000, ack_deadline_secs=30)
client.queue.publish('jobs', b'Hello', priority=5)
```

### Consuming Messages

**RabbitMQ:**
```python
def callback(ch, method, properties, body):
    process(body)
    ch.basic_ack(delivery_tag=method.delivery_tag)

channel.basic_consume(queue='jobs', on_message_callback=callback)
channel.start_consuming()
```

**Synap:**
```python
while True:
    message = client.queue.consume('jobs', 'worker-1')
    if message:
        process(message.payload)
        client.queue.ack('jobs', message.message_id)
```

## Migrating from Kafka

### Producer

**Kafka:**
```python
from kafka import KafkaProducer

producer = KafkaProducer(bootstrap_servers=['localhost:9092'])
producer.send('events', value=b'Hello')
```

**Synap:**
```python
from synap_sdk import SynapClient

client = SynapClient("http://localhost:15500")
client.stream.publish('events', 'event.type', 'Hello')
```

### Consumer

**Kafka:**
```python
from kafka import KafkaConsumer

consumer = KafkaConsumer('events', bootstrap_servers=['localhost:9092'])
for message in consumer:
    process(message.value)
```

**Synap:**
```python
last_offset = 0
while True:
    events = client.stream.consume('events', 'consumer-1', from_offset=last_offset, limit=10)
    for event in events:
        process(event.data)
        last_offset = event.offset + 1
```

## Migration Strategy

### Phase 1: Parallel Running

Run both systems in parallel:

```python
# Write to both
redis_client.set('key', 'value')
synap_client.kv.set('key', 'value')

# Read from Redis (gradually migrate reads)
value = redis_client.get('key')
```

### Phase 2: Read Migration

Migrate reads to Synap:

```python
# Try Synap first, fallback to Redis
try:
    value = synap_client.kv.get('key')
except:
    value = redis_client.get('key')
```

### Phase 3: Full Migration

Switch all operations to Synap:

```python
# All operations use Synap
value = synap_client.kv.get('key')
```

## Data Migration

### Export from Redis

```bash
# Use redis-cli to export
redis-cli --rdb dump.rdb
```

### Import to Synap

```python
# Read Redis dump and import to Synap
import redis
from synap_sdk import SynapClient

redis_client = redis.Redis()
synap_client = SynapClient("http://localhost:15500")

# Migrate keys
for key in redis_client.scan_iter():
    value = redis_client.get(key)
    ttl = redis_client.ttl(key)
    synap_client.kv.set(key, value, ttl=ttl if ttl > 0 else None)
```

## Compatibility Notes

### Differences

1. **Protocol**: Synap uses HTTP REST, Redis uses RESP
2. **Connection**: Synap is stateless HTTP, Redis is persistent TCP
3. **Commands**: Some Redis commands may have different names

### Similarities

1. **Data Structures**: Same structures (Hash, List, Set, Sorted Set)
2. **Operations**: Same operations (GET, SET, LPUSH, etc.)
3. **TTL**: Same TTL semantics

## Best Practices

### Test Migration

1. Test in development environment
2. Verify data integrity
3. Performance testing
4. Rollback plan

### Gradual Migration

1. Start with non-critical data
2. Migrate reads first
3. Then migrate writes
4. Monitor for issues

## Related Topics

- [Basic KV Operations](../kv-store/BASIC.md) - Basic operations
- [Data Structures](../kv-store/DATA_STRUCTURES.md) - Data structures
- [Queues Guide](../queues/QUEUES.md) - Queue operations
- [Streams Guide](../streams/STREAMS.md) - Stream operations

