---
title: SDKs Overview
module: sdks
id: sdks-overview
order: 0
description: Quick comparison and overview of all SDKs
tags: [sdks, overview, comparison, clients]
---

# SDKs Overview

Quick comparison and overview of all Synap SDKs.

## Available SDKs

### Python SDK

**Installation:**
```bash
pip install synap-sdk
```

**Quick Start:**
```python
from synap_sdk import SynapClient

client = SynapClient("http://localhost:15500")
client.kv.set("key", "value")
```

**Documentation:** [Python SDK Guide](./PYTHON.md)

### TypeScript/JavaScript SDK

**Installation:**
```bash
npm install @hivehub/synap
```

**Quick Start:**
```typescript
import { Synap } from "@hivehub/synap";

const client = new SynapClient("http://localhost:15500");
await client.kv.set("key", "value");
```

**Documentation:** [TypeScript SDK Guide](./TYPESCRIPT.md)

### Rust SDK

**Installation:**
```toml
[dependencies]
synap-sdk = "0.8.0"
```

**Quick Start:**
```rust
use synap_sdk::SynapClient;

let client = SynapClient::new("http://localhost:15500")?;
client.kv.set("key", "value", None).await?;
```

**Documentation:** [Rust SDK Guide](./RUST.md)

## Feature Comparison

| Feature | Python | TypeScript | Rust |
|---------|--------|------------|------|
| Key-Value | ✅ | ✅ | ✅ |
| Queues | ✅ | ✅ | ✅ |
| Streams | ✅ | ✅ | ✅ |
| Pub/Sub | ✅ | ✅ | ✅ |
| Async Support | ✅ | ✅ | ✅ |
| Type Safety | ✅ | ✅ | ✅ |
| Error Handling | ✅ | ✅ | ✅ |

## Installation Quick Reference

### Python

```bash
pip install synap-sdk
```

### TypeScript/JavaScript

```bash
npm install @hivehub/synap
# or
yarn add @hivehub/synap
# or
pnpm add @hivehub/synap
```

### Rust

```toml
[dependencies]
synap-sdk = "0.8.0"
```

## Usage Examples

### Key-Value Operations

**Python:**
```python
client.kv.set("key", "value", ttl=3600)
value = client.kv.get("key")
```

**TypeScript:**
```typescript
await client.kv.set("key", "value", { ttl: 3600 });
const value = await client.kv.get("key");
```

**Rust:**
```rust
client.kv.set("key", "value", Some(3600)).await?;
let value = client.kv.get("key").await?;
```

### Queue Operations

**Python:**
```python
client.queue.publish("jobs", b"Hello", priority=5)
message = client.queue.consume("jobs", "worker-1")
```

**TypeScript:**
```typescript
await client.queue.publish("jobs", Buffer.from("Hello"), { priority: 5 });
const message = await client.queue.consume("jobs", "worker-1");
```

**Rust:**
```rust
client.queue.publish("jobs", b"Hello", 5).await?;
let message = client.queue.consume("jobs", "worker-1").await?;
```

## Choosing an SDK

### Use Python SDK if:
- Building Python applications
- Need quick prototyping
- Using data science tools

### Use TypeScript SDK if:
- Building web applications
- Using Node.js backend
- Need browser support

### Use Rust SDK if:
- Building high-performance applications
- Need maximum performance
- Using Rust ecosystem

## Related Topics

- [Python SDK Guide](./PYTHON.md) - Complete Python guide
- [TypeScript SDK Guide](./TYPESCRIPT.md) - Complete TypeScript guide
- [Rust SDK Guide](./RUST.md) - Complete Rust guide
- [API Reference](../api/API_REFERENCE.md) - REST API documentation

