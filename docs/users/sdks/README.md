---
title: SDKs
module: sdks
id: sdks-index
order: 0
description: Client libraries and SDKs
tags: [sdks, clients, libraries, api]
---

# SDKs

Client libraries and SDKs for Synap.

## Available SDKs

### [Python SDK](./PYTHON.md)

Complete Python SDK guide:

- Installation
- Basic usage
- All operations
- Examples

### [TypeScript SDK](./TYPESCRIPT.md)

TypeScript/JavaScript SDK guide:

- Installation
- Basic usage
- All operations
- Examples

### [Rust SDK](./RUST.md)

Complete Rust SDK guide:

- Installation
- Basic usage
- All operations
- Examples

### [SDKs Overview](./SDKS.md)

Quick comparison and overview:

- Feature comparison
- Installation quick reference
- Usage examples

## Quick Start

### Python

```python
from synap_sdk import SynapClient

client = SynapClient("http://localhost:15500")
client.kv.set("key", "value")
value = client.kv.get("key")
```

### TypeScript

```typescript
import { Synap } from "@hivehub/synap";

const client = new SynapClient("http://localhost:15500");
await client.kv.set("key", "value");
const value = await client.kv.get("key");
```

### Rust

```rust
use synap_sdk::SynapClient;

let client = SynapClient::new("http://localhost:15500")?;
client.kv.set("key", "value").await?;
let value = client.kv.get("key").await?;
```

## Related Topics

- [API Reference](../api/API_REFERENCE.md) - Complete API documentation
- [Quick Start Guide](../getting-started/QUICK_START.md) - Get started quickly

