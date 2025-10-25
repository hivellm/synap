# Synap Python SDK

Official Python client library for [Synap](https://github.com/hivellm/hivellm) - High-Performance In-Memory Key-Value Store & Message Broker.

## Features

- 💾 **Key-Value Store**: Fast in-memory KV operations with TTL support
- 📨 **Message Queues**: RabbitMQ-style queues with ACK/NACK
- 📡 **Event Streams**: Kafka-style event streams with offset tracking
- 🔔 **Pub/Sub**: Topic-based messaging with wildcards
- ⚡ **StreamableHTTP Protocol**: Unified endpoint for all operations
- 🛡️ **Type-Safe**: Full type hints with mypy strict mode
- 📦 **Async/Await**: Built on modern async patterns with httpx

## Requirements

- Python 3.11 or higher
- Synap Server running

## Installation

```bash
pip install synap-sdk
```

## Quick Start

```python
import asyncio
from synap_sdk import SynapClient, SynapConfig

async def main():
    # Create client
    config = SynapConfig.create("http://localhost:15500")
    async with SynapClient(config) as client:
        # Key-Value operations
        await client.kv.set("user:1", "John Doe")
        value = await client.kv.get("user:1")
        print(f"Value: {value}")

        # Queue operations
        await client.queue.create_queue("tasks")
        msg_id = await client.queue.publish("tasks", {"task": "process-video"}, priority=9)
        message = await client.queue.consume("tasks", "worker-1")
        
        if message:
            # Process message
            print(f"Received: {message.payload}")
            await client.queue.ack("tasks", message.id)

        # Event Stream
        await client.stream.create_room("chat-room-1")
        offset = await client.stream.publish("chat-room-1", "message", {
            "user": "alice",
            "text": "Hello!"
        })

        # Pub/Sub
        await client.pubsub.subscribe_topics("user-123", ["notifications.*"])
        delivered = await client.pubsub.publish("notifications.email", {
            "to": "user@example.com",
            "subject": "Welcome"
        })

asyncio.run(main())
```

## API Reference

### Configuration

```python
from synap_sdk import SynapConfig

config = SynapConfig.create("http://localhost:15500") \
    .with_timeout(60) \
    .with_auth_token("your-token") \
    .with_max_retries(5)

async with SynapClient(config) as client:
    # Use client
    pass
```

### Key-Value Store

```python
# Set value with TTL
await client.kv.set("session:abc", session_data, ttl=3600)

# Get value
data = await client.kv.get("session:abc")

# Atomic operations
new_value = await client.kv.incr("counter", delta=1)
exists = await client.kv.exists("session:abc")

# Scan keys
keys = await client.kv.scan("user:*", limit=100)

# Get stats
stats = await client.kv.stats()

# Delete key
await client.kv.delete("session:abc")
```

### Message Queues

```python
# Create queue
await client.queue.create_queue("tasks", max_size=10000, message_ttl=3600)

# Publish message with priority
message_id = await client.queue.publish(
    "tasks",
    {"action": "encode", "file": "video.mp4"},
    priority=9,
    max_retries=3
)

# Consume and process
message = await client.queue.consume("tasks", "worker-1")
if message:
    try:
        # Process message
        await process_message(message.payload)
        await client.queue.ack("tasks", message.id)
    except Exception:
        await client.queue.nack("tasks", message.id)  # Requeue

# Get queue stats
stats = await client.queue.stats("tasks")
queues = await client.queue.list()

# Delete queue
await client.queue.delete_queue("tasks")
```

### Event Streams

```python
# Create stream room
await client.stream.create_room("events")

# Publish event
offset = await client.stream.publish("events", "user.created", {
    "userId": "123",
    "name": "Alice"
})

# Read events
events = await client.stream.read("events", offset=0, limit=100)
for evt in events:
    print(f"Event: {evt.event} at offset {evt.offset}")

# Get stream stats
stats = await client.stream.stats("events")
rooms = await client.stream.list_rooms()

# Delete room
await client.stream.delete_room("events")
```

### Pub/Sub

```python
# Subscribe to topics (supports wildcards)
await client.pubsub.subscribe_topics("subscriber-1", [
    "user.created",
    "notifications.*",
    "events.#"
])

# Publish message
delivered = await client.pubsub.publish("notifications.email", {
    "to": "user@example.com",
    "subject": "Welcome!"
})
print(f"Delivered to {delivered} subscribers")

# Unsubscribe
await client.pubsub.unsubscribe_topics("subscriber-1", ["notifications.*"])

# Get stats
stats = await client.pubsub.stats()
```

## Async Context Manager

The client supports async context manager for automatic cleanup:

```python
async with SynapClient(config) as client:
    await client.kv.set("key", "value")
    # Client automatically closed when exiting context
```

Or manual close:

```python
client = SynapClient(config)
try:
    await client.kv.set("key", "value")
finally:
    await client.close()
```

## Error Handling

```python
from synap_sdk.exceptions import SynapException

try:
    await client.kv.set("key", "value")
except SynapException as e:
    print(f"Synap error: {e}")
```

## Custom HTTP Client

You can provide your own `httpx.AsyncClient` for advanced scenarios:

```python
import httpx
from synap_sdk import SynapClient, SynapConfig

http_client = httpx.AsyncClient(timeout=120)
config = SynapConfig.create("http://localhost:15500")

async with SynapClient(config, http_client) as client:
    # Use client
    pass
```

## Type Hints

The SDK is fully typed and passes mypy strict mode:

```bash
mypy synap_sdk
```

## Development

### Install Development Dependencies

```bash
pip install -e ".[dev]"
```

### Run Tests

```bash
pytest
```

### Run Tests with Coverage

```bash
pytest --cov=synap_sdk --cov-report=html
```

### Type Checking

```bash
mypy synap_sdk
```

### Linting

```bash
ruff check synap_sdk
```

### Formatting

```bash
ruff format synap_sdk
```

## License

MIT License - see [LICENSE](LICENSE) file for details.

## Contributing

Contributions are welcome! Please see [CONTRIBUTING.md](../../CONTRIBUTING.md) for details.

## Links

- [Synap Server](https://github.com/hivellm/hivellm/tree/main/synap)
- [Documentation](https://github.com/hivellm/hivellm/blob/main/synap/README.md)
- [Other SDKs](https://github.com/hivellm/hivellm/tree/main/synap/sdks)

