---
title: TypeScript SDK
module: sdks
id: typescript-sdk
order: 2
description: TypeScript/JavaScript SDK guide
tags: [sdk, typescript, javascript, client, library]
---

# TypeScript SDK

Complete guide to using the Synap TypeScript/JavaScript SDK.

## Installation

### npm

```bash
npm install @hivehub/synap
```

### yarn

```bash
yarn add @hivehub/synap
```

### pnpm

```bash
pnpm add @hivehub/synap
```

## Quick Start

```typescript
import { Synap } from "@hivehub/synap";

// Create client
const client = new SynapClient("http://localhost:15500");

// Key-Value operations
await client.kv.set("user:1", "John Doe", { ttl: 3600 });
const value = await client.kv.get("user:1");
await client.kv.delete("user:1");

// Queue operations
await client.queue.create("jobs", { maxDepth: 1000 });
await client.queue.publish("jobs", Buffer.from("Hello"), { priority: 5 });
const message = await client.queue.consume("jobs", "worker-1");
await client.queue.ack("jobs", message.messageId);
```

## Authentication

### API Key

```typescript
const client = new SynapClient("http://localhost:15500", {
  apiKey: "sk_live_abc123..."
});
```

### Basic Auth

```typescript
const client = new SynapClient("http://localhost:15500", {
  username: "admin",
  password: "password"
});
```

## Key-Value Store

### Basic Operations

```typescript
// Set
await client.kv.set("key", "value");
await client.kv.set("key", "value", { ttl: 3600 });

// Get
const value = await client.kv.get("key");

// Delete
await client.kv.delete("key");

// Exists
const exists = await client.kv.exists("key");
```

### Batch Operations

```typescript
// Multiple set
await client.kv.mset([
  { key: "key1", value: "value1" },
  { key: "key2", value: "value2" },
  { key: "key3", value: "value3" }
]);

// Multiple get
const values = await client.kv.mget(["key1", "key2", "key3"]);
```

### Atomic Operations

```typescript
// Increment
const value = await client.kv.incr("counter");
const value2 = await client.kv.incrby("counter", 5);

// Decrement
const value3 = await client.kv.decr("counter");
const value4 = await client.kv.decrby("counter", 3);
```

## Message Queues

### Queue Management

```typescript
// Create queue
await client.queue.create("jobs", {
  maxDepth: 1000,
  ackDeadlineSecs: 30
});

// List queues
const queues = await client.queue.list();

// Get stats
const stats = await client.queue.stats("jobs");
```

### Publishing

```typescript
// Publish message
await client.queue.publish("jobs", Buffer.from("Hello"), { priority: 5 });

// With retries
await client.queue.publish("jobs", Buffer.from("Hello"), {
  priority: 5,
  maxRetries: 3
});
```

### Consuming

```typescript
// Consume message
const message = await client.queue.consume("jobs", "worker-1");

// Process message
processMessage(message.payload);

// Acknowledge
await client.queue.ack("jobs", message.messageId);

// Or reject
await client.queue.nack("jobs", message.messageId);
```

## Event Streams

### Stream Management

```typescript
// Create stream
await client.stream.create("notifications", {
  partitions: 1,
  retentionHours: 24
});

// List streams
const streams = await client.stream.list();

// Get stats
const stats = await client.stream.stats("notifications");
```

### Publishing Events

```typescript
// Publish event
await client.stream.publish("notifications", "user.signup", "New user registered");
```

### Consuming Events

```typescript
// Consume events
const events = await client.stream.consume("notifications", "user-1", {
  fromOffset: 0,
  limit: 10
});

for (const event of events) {
  console.log(`Event: ${event.event}, Data: ${event.data}`);
}
```

## Pub/Sub

### Publishing

```typescript
// Publish to topic
await client.pubsub.publish("notifications.email", "New order received");
```

### Subscribing

```typescript
// Subscribe to topics
const subscription = client.pubsub.subscribe(["notifications.email"]);

for await (const message of subscription) {
  console.log(`Topic: ${message.topic}, Message: ${message.message}`);
}
```

## Error Handling

```typescript
import { SynapError } from "@hivehub/synap";

try {
  const value = await client.kv.get("key");
} catch (error) {
  if (error instanceof SynapError) {
    console.error(`Error: ${error.message}, Code: ${error.statusCode}`);
  }
}
```

## Browser Usage

```typescript
// Works in browser (requires CORS configuration)
const client = new SynapClient("https://synap.example.com", {
  apiKey: "sk_live_abc123..."
});
```

## Related Topics

- [SDKs Overview](./SDKS.md) - SDK comparison
- [API Reference](../api/API_REFERENCE.md) - Complete API documentation

