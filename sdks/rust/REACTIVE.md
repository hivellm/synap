# Reactive Programming in Synap Rust SDK

This document explains the reactive patterns available in the Synap Rust SDK, similar to the RxJS implementation in the TypeScript SDK.

## Overview

The Rust SDK uses Rust's native `futures::Stream` trait to provide reactive programming capabilities. This is the idiomatic way to handle asynchronous streams in Rust, equivalent to RxJS Observables in TypeScript.

## Key Concepts

### MessageStream<T>

A wrapper around `tokio::sync::mpsc::UnboundedReceiver` that implements `futures::Stream`. This allows you to use all the standard stream combinators from the `futures` crate.

```rust
use futures::StreamExt;

let (mut stream, handle) = client.queue()
    .observe_messages("tasks", "worker-1", Duration::from_millis(100));

while let Some(message) = stream.next().await {
    println!("Received: {:?}", message);
}
```

### SubscriptionHandle

A handle for gracefully stopping reactive subscriptions. It automatically cancels the subscription when dropped (RAII pattern).

```rust
let (stream, handle) = client.queue().observe_messages(...);

// Use the stream...

// Explicitly stop
handle.unsubscribe();

// Or let it drop automatically (RAII)
```

## Queue Reactive Methods

### `observe_messages()` - Manual ACK

Stream messages from a queue with manual acknowledgment control.

```rust
use futures::StreamExt;
use std::time::Duration;

let (mut stream, handle) = client.queue()
    .observe_messages("tasks", "worker-1", Duration::from_millis(100));

while let Some(message) = stream.next().await {
    // Process message
    println!("Processing: {:?}", message.id);
    
    // Manual ACK
    client.queue().ack("tasks", &message.id).await?;
}

handle.unsubscribe();
```

**Features:**
- ✅ Manual ACK/NACK control
- ✅ Backpressure handling via Stream
- ✅ Error handling per message
- ✅ Graceful cancellation

### `process_messages()` - Automatic ACK/NACK

Automatically ACK successful processing or NACK failures.

```rust
use std::time::Duration;

let handle = client.queue().process_messages(
    "tasks",
    "worker-1",
    Duration::from_millis(100),
    |message| async move {
        // Process the message
        println!("Processing: {:?}", message.id);
        
        // Result determines ACK/NACK:
        // Ok(()) = ACK (success)
        // Err(_) = NACK (requeue)
        process_task(&message.payload).await
    }
);

// Let it run for a while
tokio::time::sleep(Duration::from_secs(60)).await;

// Stop processing
handle.unsubscribe();
```

**Features:**
- ✅ Automatic ACK on success
- ✅ Automatic NACK on error (requeue)
- ✅ Concurrent processing support
- ✅ Error handling and logging
- ✅ Graceful shutdown

## Stream Reactive Methods

### `observe_events()` - All Events

Stream all events from a room with automatic offset tracking.

```rust
use futures::StreamExt;

let (mut stream, handle) = client.stream()
    .observe_events("chat-room-1", Some(0), Duration::from_millis(100));

while let Some(event) = stream.next().await {
    println!("Event {}: {} = {:?}", event.offset, event.event_type, event.data);
}

handle.unsubscribe();
```

**Features:**
- ✅ Automatic offset tracking
- ✅ Batch consumption (100 events per poll)
- ✅ Configurable poll interval
- ✅ No message loss

### `observe_event()` - Filtered Events

Stream only events of a specific type.

```rust
use futures::StreamExt;

let (mut stream, handle) = client.stream()
    .observe_event("chat-room-1", "message", Some(0), Duration::from_millis(100));

while let Some(event) = stream.next().await {
    println!("Message event: {:?}", event.data);
}

handle.unsubscribe();
```

**Features:**
- ✅ Server-side or client-side filtering
- ✅ Type-specific processing
- ✅ Automatic offset management

## Stream Combinators

Since `MessageStream` implements `futures::Stream`, you can use all standard stream operators:

### Filter

```rust
use futures::StreamExt;

let (stream, handle) = client.queue().observe_messages(...);

let filtered = stream.filter(|msg| {
    futures::future::ready(msg.priority >= 5)
});
```

### Map

```rust
let (stream, handle) = client.stream().observe_events(...);

let mapped = stream.map(|event| {
    event.data["user"].as_str().unwrap_or("unknown").to_string()
});
```

### Take

```rust
let (stream, handle) = client.queue().observe_messages(...);

let first_10 = stream.take(10);

// Process only first 10 messages
while let Some(msg) = first_10.next().await {
    process(msg).await;
}
```

### Buffer

```rust
use futures::StreamExt;

let (stream, handle) = client.queue().observe_messages(...);

let batched = stream.chunks(10);

// Process in batches of 10
while let Some(batch) = batched.next().await {
    process_batch(batch).await;
}
```

### Timeout

```rust
use tokio::time::Duration;
use futures::StreamExt;

let (stream, handle) = client.queue().observe_messages(...);

let with_timeout = stream.timeout(Duration::from_secs(5));

while let Ok(Some(msg)) = with_timeout.next().await {
    process(msg).await;
}
```

### Retry Logic

```rust
use futures::StreamExt;

let handle = client.queue().process_messages(
    "tasks",
    "worker",
    Duration::from_millis(100),
    |message| async move {
        // Retry logic
        let mut retries = 0;
        loop {
            match process_task(&message.payload).await {
                Ok(result) => return Ok(result),
                Err(e) if retries < 3 => {
                    retries += 1;
                    tokio::time::sleep(Duration::from_millis(100 * retries)).await;
                }
                Err(e) => return Err(e),
            }
        }
    }
);
```

## Comparison with TypeScript SDK

| TypeScript (RxJS) | Rust (futures::Stream) |
|-------------------|------------------------|
| `Observable<T>` | `impl Stream<Item = T>` |
| `subscribe()` | `stream.next().await` |
| `unsubscribe()` | `handle.unsubscribe()` |
| `pipe(filter(...))` | `stream.filter(...)` |
| `pipe(map(...))` | `stream.map(...)` |
| `pipe(take(n))` | `stream.take(n)` |
| `pipe(bufferTime(...))` | `stream.chunks_timeout(...)` |
| `catchError()` | `stream.then(\|result\| ...)` |
| `retry()` | Custom retry in handler |
| `share()` | `stream.shared()` (with futures_util) |

## Advanced Patterns

### Concurrent Processing

```rust
use futures::stream::{self, StreamExt};

let (stream, handle) = client.queue().observe_messages(...);

// Process up to 10 messages concurrently
stream
    .for_each_concurrent(10, |message| async move {
        process_message(message).await;
    })
    .await;
```

### Merge Multiple Streams

```rust
use futures::stream::{self, StreamExt};

let (stream1, _) = client.queue().observe_messages("queue1", ...);
let (stream2, _) = client.queue().observe_messages("queue2", ...);

let merged = stream::select(stream1, stream2);

while let Some(message) = merged.next().await {
    process(message).await;
}
```

### Error Recovery

```rust
use futures::StreamExt;

let (stream, handle) = client.queue().observe_messages(...);

let with_recovery = stream.then(|message| async move {
    match process(message).await {
        Ok(result) => Some(result),
        Err(e) => {
            eprintln!("Error: {}", e);
            None // Skip failed items
        }
    }
}).filter_map(|x| futures::future::ready(x));
```

## Best Practices

1. **Always unsubscribe**: Use `SubscriptionHandle` to gracefully stop streams
2. **Handle errors**: Don't panic in stream processing, log and continue
3. **Use backpressure**: Stream naturally provides backpressure via `poll_next`
4. **Concurrent processing**: Use `for_each_concurrent` for parallel processing
5. **Timeout**: Always add timeouts to prevent hanging
6. **Drop cleanup**: `SubscriptionHandle` auto-unsubscribes on drop (RAII)

## Performance Considerations

- **Poll interval**: Balance between latency and CPU usage
  - High frequency (10-50ms): Low latency, higher CPU
  - Low frequency (500-1000ms): Lower CPU, higher latency
  
- **Batch size**: Consume in batches for better throughput
  - Events: 100 per poll (automatic in `observe_events`)
  - Messages: 1 per poll (can be batched manually)

- **Concurrency**: Use `for_each_concurrent` for I/O-bound tasks

## See Also

- [examples/reactive_queue.rs](examples/reactive_queue.rs) - Complete reactive queue example
- [examples/reactive_stream.rs](examples/reactive_stream.rs) - Complete reactive stream example
- [Tokio Streams](https://docs.rs/tokio-stream) - Tokio stream utilities
- [futures-rs](https://docs.rs/futures) - Stream combinators

