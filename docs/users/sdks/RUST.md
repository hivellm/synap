---
title: Rust SDK
module: sdks
id: rust-sdk
order: 3
description: Complete Rust SDK guide
tags: [sdk, rust, client, library]
---

# Rust SDK

Complete guide to using the Synap Rust SDK.

## Installation

### From crates.io

```toml
[dependencies]
synap-sdk = "0.8.0"
```

### From Git

```toml
[dependencies]
synap-sdk = { git = "https://github.com/hivellm/synap-sdk-rust.git" }
```

## Quick Start

```rust
use synap_sdk::SynapClient;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create client
    let client = SynapClient::new("http://localhost:15500")?;
    
    // Key-Value operations
    client.kv.set("user:1", "John Doe", Some(3600)).await?;
    let value = client.kv.get("user:1").await?;
    client.kv.delete("user:1").await?;
    
    // Queue operations
    client.queue.create("jobs", 1000, 30).await?;
    client.queue.publish("jobs", b"Hello", 5).await?;
    let message = client.queue.consume("jobs", "worker-1").await?;
    if let Some(msg) = message {
        client.queue.ack("jobs", &msg.message_id).await?;
    }
    
    Ok(())
}
```

## Authentication

### API Key

```rust
let client = SynapClient::new_with_auth(
    "http://localhost:15500",
    Some("sk_live_abc123...".to_string()),
    None,
    None
)?;
```

### Basic Auth

```rust
let client = SynapClient::new_with_auth(
    "http://localhost:15500",
    None,
    Some("admin".to_string()),
    Some("password".to_string())
)?;
```

## Key-Value Store

### Basic Operations

```rust
// Set
client.kv.set("key", "value", None).await?;
client.kv.set("key", "value", Some(3600)).await?;  // With TTL

// Get
let value = client.kv.get("key").await?;

// Delete
client.kv.delete("key").await?;

// Exists
let exists = client.kv.exists("key").await?;
```

### Batch Operations

```rust
// Multiple set
let pairs = vec![
    ("key1", "value1"),
    ("key2", "value2"),
    ("key3", "value3"),
];
client.kv.mset(pairs).await?;

// Multiple get
let keys = vec!["key1", "key2", "key3"];
let values = client.kv.mget(keys).await?;
```

### Atomic Operations

```rust
// Increment
let value = client.kv.incr("counter").await?;
let value2 = client.kv.incrby("counter", 5).await?;

// Decrement
let value3 = client.kv.decr("counter").await?;
let value4 = client.kv.decrby("counter", 3).await?;
```

## Message Queues

### Queue Management

```rust
// Create queue
client.queue.create("jobs", 1000, 30).await?;

// List queues
let queues = client.queue.list().await?;

// Get stats
let stats = client.queue.stats("jobs").await?;
```

### Publishing

```rust
// Publish message
client.queue.publish("jobs", b"Hello", 5).await?;

// With retries
client.queue.publish_with_retries("jobs", b"Hello", 5, 3).await?;
```

### Consuming

```rust
// Consume message
let message = client.queue.consume("jobs", "worker-1").await?;

if let Some(msg) = message {
    // Process message
    process_message(&msg.payload)?;
    
    // ACK
    client.queue.ack("jobs", &msg.message_id).await?;
    
    // Or NACK
    // client.queue.nack("jobs", &msg.message_id).await?;
}
```

## Event Streams

### Stream Management

```rust
// Create stream
client.stream.create("notifications", 1, 24).await?;

// List streams
let streams = client.stream.list().await?;

// Get stats
let stats = client.stream.stats("notifications").await?;
```

### Publishing Events

```rust
// Publish event
client.stream.publish("notifications", "user.signup", "New user").await?;

// With partition key
client.stream.publish_with_key(
    "notifications",
    "user.signup",
    "New user",
    "user-123"
).await?;
```

### Consuming Events

```rust
// Consume events
let events = client.stream.consume("notifications", "user-1", 0, 10).await?;

for event in events {
    println!("Offset: {}, Event: {}, Data: {}", 
        event.offset, event.event, event.data);
}
```

## Pub/Sub

### Publishing

```rust
// Publish to topic
client.pubsub.publish("notifications.email", "New order").await?;
```

### Subscribing

```rust
use futures::StreamExt;

let topics = vec!["notifications.email".to_string()];
let mut subscription = client.pubsub.subscribe(topics).await?;

while let Some(message) = subscription.next().await {
    println!("Topic: {}, Message: {}", message.topic, message.message);
}
```

## Error Handling

```rust
use synap_sdk::SynapError;

match client.kv.get("key").await {
    Ok(value) => println!("Value: {:?}", value),
    Err(SynapError::NotFound) => println!("Key not found"),
    Err(e) => eprintln!("Error: {}", e),
}
```

## Async/Await

All operations are async:

```rust
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = SynapClient::new("http://localhost:15500")?;
    
    // All operations are async
    let value = client.kv.get("key").await?;
    
    Ok(())
}
```

## Related Topics

- [SDKs Overview](./SDKS.md) - SDK comparison
- [API Reference](../api/API_REFERENCE.md) - Complete API documentation

