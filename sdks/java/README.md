# Synap Java SDK

Official Java client library for [Synap](https://github.com/hivellm/synap) - High-Performance In-Memory Key-Value Store & Message Broker.

## Features

- Key-Value Store with TTL support
- Message Queues with ACK/NACK
- Event Streams with offset tracking
- Pub/Sub with topic-based messaging
- Hash, List, Set data structures
- Bearer token and Basic authentication
- Java 17+ with `java.net.http.HttpClient`

## Requirements

- Java 17 or higher
- Maven 3.8+
- Synap Server running

## Installation

### Maven

```xml
<dependency>
    <groupId>com.hivellm</groupId>
    <artifactId>synap-sdk</artifactId>
    <version>0.11.0</version>
</dependency>
```

## Quick Start

```java
import com.hivellm.synap.*;

public class Example {
    public static void main(String[] args) throws Exception {
        SynapConfig config = SynapConfig.builder("http://localhost:15500")
            .timeout(java.time.Duration.ofSeconds(5))
            .build();

        try (SynapClient client = new SynapClient(config)) {
            // Key-Value
            client.kv().set("greeting", "Hello, Synap!");
            String value = client.kv().get("greeting");
            System.out.println(value); // "Hello, Synap!"

            // Queue
            client.queue().create("tasks", 1000, 30);
            String msgId = client.queue().publish("tasks", "process-job".getBytes(), 5, 3);
            var msg = client.queue().consume("tasks", "worker-1");
            if (msg != null) {
                System.out.println("Got: " + new String(msg.getPayload()));
                client.queue().ack("tasks", msg.getId());
            }

            // Hash
            client.hash().set("user:1", "name", "Alice");
            String name = client.hash().get("user:1", "name");

            // List
            client.list().lpush("queue", "a", "b", "c");
            var items = client.list().range("queue", 0, -1);

            // Set
            client.set().add("tags", "rust", "java", "go");
            var members = client.set().members("tags");

            // Pub/Sub
            client.pubsub().publish("events", "{\"type\":\"created\"}", null);

            // Stream
            client.stream().create("logs", 10000);
            long offset = client.stream().publish("logs", "info", "{\"msg\":\"started\"}");
            var events = client.stream().consume("logs", 0L, 100);

            // Cleanup
            client.kv().delete("greeting");
        }
    }
}
```

## Authentication

```java
// Bearer token
SynapConfig config = SynapConfig.builder("http://localhost:15500")
    .authToken("your-api-key")
    .build();

// Basic auth
SynapConfig config = SynapConfig.builder("http://localhost:15500")
    .basicAuth("username", "password")
    .build();
```

## API Reference

### KV Store

| Method | Description |
|--------|-------------|
| `set(key, value)` | Set a key-value pair |
| `set(key, value, ttlSeconds)` | Set with TTL expiration |
| `get(key)` | Get value (null if not found) |
| `delete(key)` | Delete a key |
| `exists(key)` | Check if key exists |
| `incr(key)` | Increment integer value |
| `decr(key)` | Decrement integer value |

### Queue

| Method | Description |
|--------|-------------|
| `create(name, maxDepth, ackDeadline)` | Create a queue |
| `publish(name, payload, priority, maxRetries)` | Publish message |
| `consume(name, consumerId)` | Consume next message |
| `ack(name, messageId)` | Acknowledge message |
| `nack(name, messageId)` | Negative acknowledge |
| `stats(name)` | Get queue statistics |
| `list()` | List all queues |
| `delete(name)` | Delete a queue |

### Stream

| Method | Description |
|--------|-------------|
| `create(room, maxEvents)` | Create a stream room |
| `publish(room, eventType, data)` | Publish event |
| `consume(room, offset, limit)` | Consume events |
| `stats(room)` | Get stream statistics |
| `list()` | List all streams |
| `delete(room)` | Delete a stream |

### Pub/Sub

| Method | Description |
|--------|-------------|
| `publish(topic, data, priority)` | Publish to topic |
| `subscribe(subscriberId, topics)` | Subscribe to topics |
| `unsubscribe(subscriberId, topics)` | Unsubscribe |
| `listTopics()` | List all topics |

### Hash

| Method | Description |
|--------|-------------|
| `set(key, field, value)` | Set hash field |
| `get(key, field)` | Get hash field |
| `getAll(key)` | Get all fields |
| `del(key, field)` | Delete field |
| `exists(key, field)` | Check field exists |

### List

| Method | Description |
|--------|-------------|
| `lpush(key, values...)` | Push to head |
| `rpush(key, values...)` | Push to tail |
| `lpop(key, count)` | Pop from head |
| `rpop(key, count)` | Pop from tail |
| `range(key, start, stop)` | Get range |
| `len(key)` | Get length |

### Set

| Method | Description |
|--------|-------------|
| `add(key, members...)` | Add members |
| `members(key)` | Get all members |
| `isMember(key, member)` | Check membership |
| `remove(key, members...)` | Remove members |
| `card(key)` | Get cardinality |

## Building

```bash
mvn clean compile
mvn test
mvn package
```

## License

Apache License 2.0 - See [LICENSE](../../LICENSE) for details.
