---
title: Python SDK
module: sdks
id: python-sdk
order: 1
description: Complete Python SDK guide
tags: [sdk, python, client, library]
---

# Python SDK

Complete guide to using the Synap Python SDK.

## Installation

### From PyPI

```bash
pip install synap-sdk
```

### From Source

```bash
git clone https://github.com/hivellm/synap-sdk-python.git
cd synap-sdk-python
pip install -e .
```

## Quick Start

```python
from synap_sdk import SynapClient

# Create client
client = SynapClient("http://localhost:15500")

# Key-Value operations
client.kv.set("user:1", "John Doe", ttl=3600)
value = client.kv.get("user:1")
client.kv.delete("user:1")

# Queue operations
client.queue.create("jobs", max_depth=1000)
client.queue.publish("jobs", b"Hello", priority=5)
message = client.queue.consume("jobs", "worker-1")
client.queue.ack("jobs", message.message_id)
```

## Authentication

### API Key

```python
client = SynapClient(
    "http://localhost:15500",
    api_key="sk_live_abc123..."
)
```

### Basic Auth

```python
client = SynapClient(
    "http://localhost:15500",
    username="admin",
    password="password"
)
```

## Key-Value Store

### Basic Operations

```python
# Set
client.kv.set("key", "value")
client.kv.set("key", "value", ttl=3600)

# Get
value = client.kv.get("key")

# Delete
client.kv.delete("key")

# Exists
exists = client.kv.exists("key")
```

### Batch Operations

```python
# Multiple set
client.kv.mset([
    ("key1", "value1"),
    ("key2", "value2"),
    ("key3", "value3")
])

# Multiple get
values = client.kv.mget(["key1", "key2", "key3"])
```

### Atomic Operations

```python
# Increment
value = client.kv.incr("counter")
value = client.kv.incrby("counter", 5)

# Decrement
value = client.kv.decr("counter")
value = client.kv.decrby("counter", 3)
```

## Message Queues

### Queue Management

```python
# Create queue
client.queue.create("jobs", max_depth=1000, ack_deadline_secs=30)

# List queues
queues = client.queue.list()

# Get stats
stats = client.queue.stats("jobs")
```

### Publishing

```python
# Publish message
client.queue.publish("jobs", b"Hello", priority=5)

# With retries
client.queue.publish("jobs", b"Hello", priority=5, max_retries=3)
```

### Consuming

```python
# Consume message
message = client.queue.consume("jobs", "worker-1")

# Process message
process(message.payload)

# Acknowledge
client.queue.ack("jobs", message.message_id)

# Or reject
client.queue.nack("jobs", message.message_id)
```

## Event Streams

### Stream Management

```python
# Create stream
client.stream.create("notifications", partitions=1, retention_hours=24)

# List streams
streams = client.stream.list()

# Get stats
stats = client.stream.stats("notifications")
```

### Publishing Events

```python
# Publish event
client.stream.publish("notifications", "user.signup", "New user registered")
```

### Consuming Events

```python
# Consume events
events = client.stream.consume("notifications", "user-1", from_offset=0, limit=10)

for event in events:
    print(f"Event: {event.event}, Data: {event.data}")
```

## Pub/Sub

### Publishing

```python
# Publish to topic
client.pubsub.publish("notifications.email", "New order received")
```

### Subscribing

```python
import asyncio
from synap_sdk import SynapClient

async def subscribe():
    client = SynapClient("http://localhost:15500")
    
    async for message in client.pubsub.subscribe(["notifications.email"]):
        print(f"Topic: {message.topic}, Message: {message.message}")

asyncio.run(subscribe())
```

## Error Handling

```python
from synap_sdk import SynapError

try:
    value = client.kv.get("key")
except SynapError as e:
    print(f"Error: {e.message}, Code: {e.status_code}")
```

## Async Support

```python
import asyncio
from synap_sdk import AsyncSynapClient

async def main():
    client = AsyncSynapClient("http://localhost:15500")
    
    await client.kv.set("key", "value")
    value = await client.kv.get("key")
    print(value)

asyncio.run(main())
```

## Related Topics

- [SDKs Overview](./SDKS.md) - SDK comparison
- [API Reference](../api/API_REFERENCE.md) - Complete API documentation

