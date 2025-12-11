# Synap Rust SDK

Official Rust client library for [Synap](https://github.com/hivellm/synap) - High-Performance In-Memory Key-Value Store & Message Broker.

## Features

- 💾 **Key-Value Store**: Fast async KV operations with TTL support
- 📨 **Message Queues**: RabbitMQ-style queues with ACK/NACK + reactive consumption
- 📡 **Event Streams**: Kafka-style event streams **reactive by default** 🔥
- 🔔 **Pub/Sub**: Topic-based messaging **reactive by default** 🔥
- 🔄 **Reactive Patterns**: `futures::Stream` for event-driven consumption
- 🧾 **Transactions**: MULTI/EXEC/WATCH/DISCARD helpers
- 📜 **Lua Scripting**: EVAL/EVALSHA with SHA1 caching
- 🔢 **HyperLogLog**: Cardinality estimation (PFADD/PFCOUNT/PFMERGE)
- ⚡ **StreamableHTTP Protocol**: Single unified endpoint for all operations
- 🛡️ **Type-Safe**: Leverages Rust's type system for correctness
- 📦 **Async/Await**: Built on Tokio for high-performance async I/O

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
synap-sdk = "0.1"
tokio = { version = "1", features = ["full"] }
```

## Quick Start

```rust
use synap_sdk::{SynapClient, SynapConfig};
use serde_json::json;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create client
    let config = SynapConfig::new("http://localhost:15500");
    let client = SynapClient::new(config)?;

    // Key-Value operations
    client.kv().set("user:1", "John Doe", None).await?;
    let value: Option<String> = client.kv().get("user:1").await?;
    tracing::info!("Value: {:?}", value);

    // Queue operations
    client.queue().create_queue("tasks", None, None).await?;
    let msg_id = client.queue().publish("tasks", b"process-video", Some(9), None).await?;
    let message = client.queue().consume("tasks", "worker-1").await?;
    
    if let Some(msg) = message {
        tracing::info!("Received: {:?}", msg);
        client.queue().ack("tasks", &msg.id).await?;
    }

    // Event Stream (reactive by default)
    client.stream().create_room("chat-room-1", None).await?;
    client.stream().publish(
        "chat-room-1",
        "message",
        json!({"user": "alice", "text": "Hello!"})
    ).await?;

    // Lua scripting
    let eval = client
        .script()
        .eval::<serde_json::Value>(
            "return redis.call('GET', KEYS[1])",
            synap_sdk::ScriptEvalOptions {
                keys: vec!["user:1".to_string()],
                args: vec![],
                timeout_ms: None,
            },
        )
        .await?;
    tracing::info!("Script result: {:?} (sha1={})", eval.result, eval.sha1);

    // HyperLogLog
    client
        .hyperloglog()
        .pfadd("visitors", ["user:a", "user:b", "user:c"])
        .await?;
    let visitors = client.hyperloglog().pfcount("visitors").await?;
    tracing::info!("Approx unique visitors: {}", visitors);

    // Transactions
    let tx = client.transaction();
    tx.multi(Default::default()).await?;
    client.kv().set("txn:key", "value", None).await?;
    match tx.exec(Default::default()).await? {
        synap_sdk::TransactionExecResult::Success { results } => {
            tracing::info!("TX results: {:?}", results);
        }
        synap_sdk::TransactionExecResult::Aborted { message, .. } => {
            tracing::info!("Transaction aborted: {:?}", message);
        }
    }
    
    // Reactive event consumption
    use futures::StreamExt;
    let (mut events, handle) = client.stream()
        .observe_events("chat-room-1", Some(0), Duration::from_millis(500));
    
    while let Some(event) = events.next().await {
        tracing::info!("Event {}: {:?}", event.offset, event.data);
        if event.offset > 10 { break; }
    }
    handle.unsubscribe();

    // Pub/Sub (reactive by default)
    let count = client.pubsub().publish(
        "notifications.email",
        json!({"to": "user@example.com", "subject": "Welcome"}),
        None,
        None
    ).await?;
    tracing::info!("Delivered to {} subscribers", count);

    Ok(())
}
```

## API Reference

### Key-Value Store

```rust
// Set a value
client.kv().set("key", "value", None).await?;
client.kv().set("session", "token", Some(3600)).await?; // with TTL

// Get a value
let value: Option<String> = client.kv().get("key").await?;
let number: Option<i64> = client.kv().get("counter").await?;

// Delete a key
client.kv().delete("key").await?;

// Check existence
let exists = client.kv().exists("key").await?;

// Atomic operations
let new_value = client.kv().incr("counter").await?;
let new_value = client.kv().decr("counter").await?;

// Get statistics
let stats = client.kv().stats().await?;
tracing::info!("Total keys: {}", stats.total_keys);
```

### Message Queues

```rust
// Create a queue
client.queue().create_queue("tasks", Some(10000), Some(30)).await?;

// Publish a message
let msg_id = client.queue().publish(
    "tasks",
    b"process-video",
    Some(9),      // priority (0-9)
    Some(3)       // max retries
).await?;

// Consume a message
let message = client.queue().consume("tasks", "worker-1").await?;

if let Some(msg) = message {
    // Process message
    tracing::info!("Processing: {:?}", msg);
    
    // Acknowledge (success)
    client.queue().ack("tasks", &msg.id).await?;
    
    // Or NACK (requeue)
    // client.queue().nack("tasks", &msg.id).await?;
}

// Get queue stats
let stats = client.queue().stats("tasks").await?;
tracing::info!("Queue depth: {}", stats.depth);

// List all queues
let queues = client.queue().list().await?;

// Delete a queue
client.queue().delete_queue("tasks").await?;
```

### Event Streams (Reactive by Default)

Event streams are **reactive by default** - use `observe_events()` or `observe_event()` for continuous event consumption.

```rust
use futures::StreamExt;
use std::time::Duration;

// Create a stream room
client.stream().create_room("chat-room-1", Some(10000)).await?;

// Publish an event
let offset = client.stream().publish(
    "chat-room-1",
    "message",
    json!({"user": "alice", "text": "Hello!"})
).await?;

// ✨ Reactive: Observe ALL events
let (mut events, handle) = client.stream()
    .observe_events("chat-room-1", Some(0), Duration::from_millis(500));

tokio::spawn(async move {
    while let Some(event) = events.next().await {
        tracing::info!("Event {}: {:?}", event.offset, event.data);
    }
});

// ✨ Reactive: Observe SPECIFIC event type
let (mut messages, handle2) = client.stream()
    .observe_event("chat-room-1", "message", Some(0), Duration::from_millis(500));

while let Some(event) = messages.next().await {
    tracing::info!("Message: {:?}", event.data);
}

// Stop observing
handle.unsubscribe();
handle2.unsubscribe();

// Get room stats
let stats = client.stream().stats("chat-room-1").await?;

// List all rooms
let rooms = client.stream().list().await?;

// Delete a room
client.stream().delete_room("chat-room-1").await?;
```

### Pub/Sub (Reactive by Default)

Pub/Sub is **reactive by default** - use `subscribe()` for event-driven message consumption.

```rust
use std::collections::HashMap;

// Publish to a topic
let delivered_count = client.pubsub().publish(
    "notifications.email",
    json!({"to": "user@example.com", "subject": "Welcome"}),
    Some(5),    // priority
    None        // headers
).await?;

// ✨ Subscribe to topics (with wildcards) - REST API
let sub_id = client.pubsub().subscribe_topics(
    "user-123",  // subscriber ID
    vec![
        "events.user.*".to_string(),      // single-level wildcard
        "notifications.#".to_string(),    // multi-level wildcard
    ]
).await?;

// ✨ Reactive: Observe messages via WebSocket (recommended)
use futures::StreamExt;
let (mut messages, handle) = client.pubsub()
    .observe("user-123", vec![
        "events.user.*".to_string(),      // single-level wildcard
        "notifications.#".to_string(),    // multi-level wildcard
    ]);

tokio::spawn(async move {
    while let Some(message) = messages.next().await {
        tracing::info!("Received on {}: {:?}", message.topic, message.data);
    }
});

// Stop observing
handle.unsubscribe();

// Or observe a single topic
let (mut events, handle2) = client.pubsub()
    .observe_topic("user-123", "events.user.*");

// Unsubscribe (REST API)
client.pubsub().unsubscribe("user-123", vec![
    "events.user.*".to_string(),
    "notifications.#".to_string(),
]).await?;

// List active topics
let topics = client.pubsub().list_topics().await?;
```

## Configuration

```rust
use synap_sdk::SynapConfig;
use std::time::Duration;

let config = SynapConfig::new("http://localhost:15500")
    .with_timeout(Duration::from_secs(10))
    .with_auth_token("your-api-key")
    .with_max_retries(5);

let client = SynapClient::new(config)?;
```

## Error Handling

```rust
use synap_sdk::SynapError;

match client.kv().get::<String>("key").await {
    Ok(Some(value)) => tracing::info!("Found: {}", value),
    Ok(None) => tracing::info!("Key not found"),
    Err(SynapError::HttpError(e)) => etracing::info!("HTTP error: {}", e),
    Err(SynapError::ServerError(e)) => etracing::info!("Server error: {}", e),
    Err(e) => etracing::info!("Error: {}", e),
}
```

## Reactive Programming (RxJS-style)

The SDK now includes **RxJS-style reactive patterns** via the `rx` module:

```rust
use synap_sdk::rx::{Observable, Subject};

// Observable with operators (like RxJS pipe)
let obs = Observable::from_stream(stream);
obs.filter(|x| *x > 2)
   .map(|x| x * 2)
   .take(10)
   .subscribe_next(|value| {
       tracing::info!("Value: {}", value);
   });

// Subject for multicasting
let subject = Subject::new();
subject.subscribe(|msg| tracing::info!("Sub 1: {}", msg));
subject.subscribe(|msg| tracing::info!("Sub 2: {}", msg));
subject.next("Hello");  // Both subscribers receive it
```

See [`src/rx/README.md`](src/rx/README.md) for complete guide.

## Examples

See the [`examples/`](examples/) directory for more examples:

- [`basic.rs`](examples/basic.rs) - Basic KV operations
- [`queue.rs`](examples/queue.rs) - Task queue pattern (traditional)
- [`reactive_queue.rs`](examples/reactive_queue.rs) - Reactive queue consumption 🔥
- [`stream.rs`](examples/stream.rs) - Event stream (traditional)
- [`reactive_stream.rs`](examples/reactive_stream.rs) - Reactive event consumption 🔥
- [`pubsub.rs`](examples/pubsub.rs) - Pub/Sub messaging
- [`reactive_pubsub.rs`](examples/reactive_pubsub.rs) - Reactive Pub/Sub subscriptions 🔥
- [`rxjs_style.rs`](examples/rxjs_style.rs) - RxJS-style patterns ⭐ NEW

Run an example:

```bash
cargo run --example basic
cargo run --example queue
cargo run --example reactive_queue    # Recommended for queues
cargo run --example reactive_stream   # Recommended for streams
cargo run --example reactive_pubsub   # Recommended for Pub/Sub 🔥
cargo run --example rxjs_style        # RxJS-style API
cargo run --example pubsub
```

## Testing

```bash
# Run tests (requires Synap server running on localhost:15500)
cargo test

# Or use a custom server URL
SYNAP_URL=http://localhost:15500 cargo test
```

## License

MIT License - See [LICENSE](../../LICENSE) for details.

## Links

- [Synap Server](https://github.com/hivellm/synap)
- [Documentation](https://github.com/hivellm/synap/tree/main/docs)
- [TypeScript SDK](../typescript)
- [Python SDK](../python)

