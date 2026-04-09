# Synap PHP SDK

Official PHP client library for [Synap](https://github.com/hivellm/synap) - High-Performance In-Memory Key-Value Store & Message Broker.

## Features

- 💾 **Key-Value Store**: Fast in-memory KV operations with TTL support
- 📨 **Message Queues**: RabbitMQ-style queues with ACK/NACK
- 📡 **Event Streams**: Kafka-style event streams with offset tracking
- 🔔 **Pub/Sub**: Topic-based messaging with wildcards
- ⚡ **StreamableHTTP Protocol**: Unified endpoint for all operations
- 🛡️ **Type-Safe**: Leverages PHP 8.2+ type system for correctness
- 📦 **PSR-4**: Standard autoloading and best practices

## Requirements

- PHP 8.2 or higher
- Composer
- Synap Server running

## Installation

```bash
composer require hivehub/synap-sdk
```

## Quick Start

```php
<?php

use Synap\SDK\SynapClient;
use Synap\SDK\SynapConfig;

// Create client
$config = SynapConfig::create('http://localhost:15500');
$client = new SynapClient($config);

// Key-Value operations
$client->kv()->set('user:1', 'John Doe');
$value = $client->kv()->get('user:1');
echo "Value: {$value}\n";

// Queue operations
$client->queue()->createQueue('tasks');
$msgId = $client->queue()->publish('tasks', ['task' => 'process-video'], 9);
$message = $client->queue()->consume('tasks', 'worker-1');

if ($message) {
    // Process message
    $client->queue()->ack('tasks', $message->id);
}

// Event Stream
$client->stream()->createRoom('chat-room-1');
$offset = $client->stream()->publish('chat-room-1', 'message', [
    'user' => 'alice',
    'text' => 'Hello!',
]);

// Pub/Sub
$client->pubsub()->subscribeTopics('user-123', ['notifications.*']);
$delivered = $client->pubsub()->publish('notifications.email', [
    'to' => 'user@example.com',
    'subject' => 'Welcome',
]);
```

## Transports

Since v0.11.0 the SDK selects the transport via **URL scheme** — no separate builder methods required:

| URL scheme    | Default port | When to use                                               |
|---------------|--------------|-----------------------------------------------------------|
| `synap://`    | `15501`      | **✅ Recommended default** — MessagePack over persistent TCP, lowest latency. |
| `resp3://`    | `6379`       | Redis-compatible text protocol — interop with existing Redis tooling. |
| `http://` / `https://` | `15500` | Original REST transport — full command coverage. |

All commands (KV, Hash, List, Set, Sorted Set, Queue, Stream, Pub/Sub, Transactions, Scripts, Geo, HyperLogLog) are fully supported on every transport. Native transports throw `UnsupportedCommandException` instead of silently falling back to HTTP.

```php
use Synap\SDK\SynapConfig;
use Synap\SDK\SynapClient;

// SynapRPC — recommended default
$config = new SynapConfig('synap://127.0.0.1:15501');
$client = new SynapClient($config);

// RESP3 — Redis-compatible
$config = new SynapConfig('resp3://127.0.0.1:6379');
$client = new SynapClient($config);

// HTTP — full REST access
$config = new SynapConfig('http://127.0.0.1:15500');
$client = new SynapClient($config);
```

**Queue, stream and pub/sub over `synap://`:**

```php
use Synap\SDK\SynapConfig;
use Synap\SDK\SynapClient;

$client = new SynapClient(new SynapConfig('synap://127.0.0.1:15501'));

// Queue round-trip
$client->queue()->createQueue('tasks', 1000, 60);
$id = $client->queue()->publish('tasks', ['job' => 'resize'], priority: 5);
$msg = $client->queue()->consume('tasks', 'worker-1');
$client->queue()->ack('tasks', $msg->id);

// Stream publish + read
$client->stream()->createRoom('events');
$client->stream()->publish('events', 'user.created', ['id' => 'u1']);
$events = $client->stream()->read('events', offset: 0);

// Reactive pub/sub (server-push, blocking loop on synap://)
$client->pubsub()->observe(['news.*'], function (array $msg) {
    echo 'got: ' . json_encode($msg) . PHP_EOL;
    return false; // return false to stop listening
});
```

## API Reference

### Configuration

```php
use Synap\SDK\SynapConfig;

$config = SynapConfig::create('http://localhost:15500')
    ->withTimeout(30)
    ->withAuthToken('your-api-key')
    ->withMaxRetries(5);

$client = new SynapClient($config);
```

### Key-Value Store

```php
// Set a value
$client->kv()->set('key', 'value');
$client->kv()->set('session', 'token', 3600); // with TTL

// Get a value
$value = $client->kv()->get('key');

// Delete a key
$client->kv()->delete('key');

// Check existence
$exists = $client->kv()->exists('key');

// Atomic operations
$newValue = $client->kv()->incr('counter');
$newValue = $client->kv()->decr('counter');

// Get statistics
$stats = $client->kv()->stats();

// Scan keys
$keys = $client->kv()->scan('user:', 100);
```

### Message Queues

```php
// Create a queue
$client->queue()->createQueue('tasks', 10000, 30);

// Publish a message
$msgId = $client->queue()->publish(
    'tasks',
    ['task' => 'process-video'],
    9,  // priority (0-9)
    3   // max retries
);

// Consume a message
$message = $client->queue()->consume('tasks', 'worker-1');

if ($message) {
    // Process message
    echo "Processing: {$message->id}\n";
    
    // Acknowledge (success)
    $client->queue()->ack('tasks', $message->id);
    
    // Or NACK (requeue)
    // $client->queue()->nack('tasks', $message->id);
}

// Get queue stats
$stats = $client->queue()->stats('tasks');

// List all queues
$queues = $client->queue()->list();

// Delete a queue
$client->queue()->deleteQueue('tasks');
```

### Event Streams

```php
// Create a stream room
$client->stream()->createRoom('chat-room-1', 10000);

// Publish an event
$offset = $client->stream()->publish(
    'chat-room-1',
    'message',
    ['user' => 'alice', 'text' => 'Hello!']
);

// Consume events
$events = $client->stream()->consume('chat-room-1', 0, 100);

foreach ($events as $event) {
    echo "Event {$event->offset}: {$event->event}\n";
}

// Get room stats
$stats = $client->stream()->stats('chat-room-1');

// List all rooms
$rooms = $client->stream()->list();

// Delete a room
$client->stream()->deleteRoom('chat-room-1');
```

### Pub/Sub

```php
// Publish to a topic
$delivered = $client->pubsub()->publish(
    'notifications.email',
    ['to' => 'user@example.com', 'subject' => 'Welcome'],
    5,    // priority
    []    // headers
);

// Subscribe to topics (with wildcards)
$subId = $client->pubsub()->subscribeTopics(
    'user-123',
    [
        'events.user.*',      // single-level wildcard
        'notifications.#',    // multi-level wildcard
    ]
);

// Unsubscribe
$client->pubsub()->unsubscribe('user-123', [
    'events.user.*',
    'notifications.#',
]);

// List active topics
$topics = $client->pubsub()->listTopics();

// Get subscriber info
$info = $client->pubsub()->getSubscriber('user-123');
```

## Error Handling

```php
use Synap\SDK\Exception\SynapException;

try {
    $value = $client->kv()->get('key');
} catch (SynapException $e) {
    echo "Error: {$e->getMessage()}\n";
}
```

## Examples

See the [`examples/`](examples/) directory:

- [`basic.php`](examples/basic.php) - All features demo

Run examples:

```bash
php examples/basic.php
```

## Testing

```bash
# Run tests
composer test

# With coverage
composer test:coverage

# Static analysis
composer phpstan

# Code style
composer cs-check
composer cs-fix
```

## StreamableHTTP Protocol

This SDK uses the StreamableHTTP protocol with a unified endpoint (`/api/stream`):

```php
// All operations use this format internally:
POST /api/stream
{
    "operation": "kv.set",
    "target": "user:1",
    "data": {
        "value": "John Doe",
        "ttl": 3600
    }
}
```

## License

MIT License - See [LICENSE](LICENSE) for details.

## Links

- [Synap Server](https://github.com/hivellm/synap)
- [Documentation](https://github.com/hivellm/synap/tree/main/docs)
- [Rust SDK](../rust)
- [TypeScript SDK](../typescript)
- [Python SDK](../python)

