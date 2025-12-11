# Synap C# SDK

Official C# client library for [Synap](https://github.com/hivellm/synap) - High-Performance In-Memory Key-Value Store & Message Broker.

## Features

- 💾 **Key-Value Store**: Fast in-memory KV operations with TTL support
- 📨 **Message Queues**: RabbitMQ-style queues with ACK/NACK
- 📡 **Event Streams**: Kafka-style event streams with offset tracking
- 🔔 **Pub/Sub**: Topic-based messaging with wildcards
- ⚡ **StreamableHTTP Protocol**: Unified endpoint for all operations
- 🛡️ **Type-Safe**: Leverages C# 12+ type system for correctness
- 📦 **Async/Await**: Built on modern async patterns with ConfigureAwait

## Requirements

- .NET 8.0 or higher
- Synap Server running

## Installation

```bash
dotnet add package HiveHub.Synap.SDK
```

## Quick Start

```csharp
using Synap.SDK;

// Create client
var config = SynapConfig.Create("http://localhost:15500");
var client = new SynapClient(config);

// Key-Value operations
await client.KV.SetAsync("user:1", "John Doe");
var value = await client.KV.GetAsync<string>("user:1");
Console.WriteLine($"Value: {value}");

// Queue operations
await client.Queue.CreateQueueAsync("tasks");
var msgId = await client.Queue.PublishAsync("tasks", new { task = "process-video" }, priority: 9);
var message = await client.Queue.ConsumeAsync("tasks", "worker-1");

if (message is not null)
{
    // Process message
    Console.WriteLine($"Received: {message.Payload}");
    await client.Queue.AckAsync("tasks", message.Id);
}

// Event Stream
await client.Stream.CreateRoomAsync("chat-room-1");
var offset = await client.Stream.PublishAsync("chat-room-1", "message", new
{
    user = "alice",
    text = "Hello!"
});

// Pub/Sub
await client.PubSub.SubscribeTopicsAsync("user-123", new List<string> { "notifications.*" });
var delivered = await client.PubSub.PublishAsync("notifications.email", new
{
    to = "user@example.com",
    subject = "Welcome"
});

// Dispose client when done
client.Dispose();
```

## API Reference

### Configuration

```csharp
using Synap.SDK;

var config = SynapConfig.Create("http://localhost:15500")
    .WithTimeout(60)
    .WithAuthToken("your-token")
    .WithMaxRetries(5);

var client = new SynapClient(config);
```

### Key-Value Store

```csharp
// Set value with TTL
await client.KV.SetAsync("session:abc", sessionData, ttl: 3600);

// Get value (generic)
var data = await client.KV.GetAsync<SessionData>("session:abc");

// Atomic operations
var newValue = await client.KV.IncrAsync("counter", delta: 1);
var exists = await client.KV.ExistsAsync("session:abc");

// Scan keys
var keys = await client.KV.ScanAsync("user:*", limit: 100);

// Get stats
var stats = await client.KV.StatsAsync();
```

### Message Queues

```csharp
// Create queue
await client.Queue.CreateQueueAsync("tasks", maxSize: 10000, messageTtl: 3600);

// Publish message with priority
var messageId = await client.Queue.PublishAsync(
    "tasks",
    new { action = "encode", file = "video.mp4" },
    priority: 9,
    maxRetries: 3
);

// Consume and process
var message = await client.Queue.ConsumeAsync("tasks", "worker-1");
if (message is not null)
{
    try
    {
        // Process message
        await ProcessAsync(message.Payload);
        await client.Queue.AckAsync("tasks", message.Id);
    }
    catch
    {
        await client.Queue.NackAsync("tasks", message.Id); // Requeue
    }
}

// Get queue stats
var stats = await client.Queue.StatsAsync("tasks");
var queues = await client.Queue.ListAsync();
```

### Event Streams

```csharp
// Create stream room
await client.Stream.CreateRoomAsync("events");

// Publish event
var offset = await client.Stream.PublishAsync("events", "user.created", new
{
    userId = "123",
    name = "Alice"
});

// Read events
var events = await client.Stream.ReadAsync("events", offset: 0, limit: 100);
foreach (var evt in events)
{
    Console.WriteLine($"Event: {evt.Event} at offset {evt.Offset}");
}

// Get stream stats
var stats = await client.Stream.StatsAsync("events");
var rooms = await client.Stream.ListRoomsAsync();
```

### Pub/Sub

```csharp
// Subscribe to topics (supports wildcards)
await client.PubSub.SubscribeTopicsAsync("subscriber-1", new List<string>
{
    "user.created",
    "notifications.*",
    "events.#"
});

// Publish message
var delivered = await client.PubSub.PublishAsync("notifications.email", new
{
    to = "user@example.com",
    subject = "Welcome!"
});

Console.WriteLine($"Delivered to {delivered} subscribers");

// Unsubscribe
await client.PubSub.UnsubscribeTopicsAsync("subscriber-1", new List<string> { "notifications.*" });

// Get stats
var stats = await client.PubSub.StatsAsync();
```

## Async Patterns

All operations support `CancellationToken`:

```csharp
var cts = new CancellationTokenSource();
cts.CancelAfter(TimeSpan.FromSeconds(5));

try
{
    var value = await client.KV.GetAsync<string>("key", cts.Token);
}
catch (OperationCanceledException)
{
    Console.WriteLine("Operation was cancelled");
}
```

## Error Handling

```csharp
using Synap.SDK.Exceptions;

try
{
    await client.KV.SetAsync("key", "value");
}
catch (SynapException ex)
{
    Console.WriteLine($"Synap error: {ex.Message}");
}
```

## Custom HttpClient

You can provide your own `HttpClient` for advanced scenarios:

```csharp
var httpClient = new HttpClient
{
    Timeout = TimeSpan.FromSeconds(120)
};

var config = SynapConfig.Create("http://localhost:15500");
var client = new SynapClient(config, httpClient);
```

## License

MIT License - see [LICENSE](LICENSE) file for details.

## Contributing

Contributions are welcome! Please see [CONTRIBUTING.md](../../CONTRIBUTING.md) for details.

## Links

- [Synap Server](https://github.com/hivellm/synap/tree/main)
- [Documentation](https://github.com/hivellm/synap/tree/main/docs)
- [Other SDKs](https://github.com/hivellm/synap/tree/main/sdks)

