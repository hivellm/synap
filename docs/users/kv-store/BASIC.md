---
title: Basic KV Operations
module: kv-store
id: kv-basic
order: 1
description: Fundamental key-value operations
tags: [kv-store, basic, operations, tutorial]
---

# Basic KV Operations

Fundamental key-value store operations in Synap.

## Set Key-Value

### Basic Set

```bash
curl -X POST http://localhost:15500/kv/set \
  -H "Content-Type: application/json" \
  -d '{
    "key": "user:1",
    "value": "John Doe"
  }'
```

### Set with TTL

```bash
curl -X POST http://localhost:15500/kv/set \
  -H "Content-Type: application/json" \
  -d '{
    "key": "session:abc",
    "value": "session-data",
    "ttl": 3600
  }'
```

TTL is in seconds. Key will expire after specified time.

## Get Value

### Get Single Key

```bash
curl http://localhost:15500/kv/get/user:1
```

**Response:**
```
"John Doe"
```

### Get Non-Existent Key

```bash
curl http://localhost:15500/kv/get/notfound
```

**Response:**
```
null
```

## Delete Key

```bash
curl -X DELETE http://localhost:15500/kv/del/user:1
```

**Response:**
```json
{
  "deleted": true
}
```

## Check Existence

```bash
curl http://localhost:15500/kv/exists/user:1
```

**Response:**
```json
{
  "exists": true
}
```

## List Keys

```bash
# List all keys
curl http://localhost:15500/kv/keys

# List keys with pattern
curl "http://localhost:15500/kv/keys?pattern=user:*"
```

**Response:**
```json
{
  "keys": ["user:1", "user:2", "user:3"]
}
```

## Get Statistics

```bash
curl http://localhost:15500/kv/stats
```

**Response:**
```json
{
  "total_keys": 42,
  "memory_bytes": 8192,
  "eviction_policy": "lru"
}
```

## Using SDKs

### Python

```python
from synap_sdk import SynapClient

client = SynapClient("http://localhost:15500")

# Set
client.kv.set("user:1", "John Doe", ttl=3600)

# Get
value = client.kv.get("user:1")

# Delete
client.kv.delete("user:1")

# Exists
exists = client.kv.exists("user:1")
```

### TypeScript

```typescript
import { Synap } from "@hivehub/synap";

const client = new SynapClient("http://localhost:15500");

// Set
await client.kv.set("user:1", "John Doe", { ttl: 3600 });

// Get
const value = await client.kv.get("user:1");

// Delete
await client.kv.delete("user:1");

// Exists
const exists = await client.kv.exists("user:1");
```

### Rust

```rust
use synap_sdk::SynapClient;

let client = SynapClient::new("http://localhost:15500")?;

// Set
client.kv.set("user:1", "John Doe", Some(3600)).await?;

// Get
let value = client.kv.get("user:1").await?;

// Delete
client.kv.delete("user:1").await?;

// Exists
let exists = client.kv.exists("user:1").await?;
```

## Best Practices

1. **Use Namespaced Keys**: `user:1`, `session:abc`, `cache:product:123`
2. **Set Appropriate TTL**: Use TTL for temporary data
3. **Check Before Get**: Use `exists` to check before expensive operations
4. **Batch Operations**: Use MSET/MGET for multiple keys

## Related Topics

- [Advanced Operations](./ADVANCED.md) - Advanced KV operations
- [Data Structures](./DATA_STRUCTURES.md) - Hash, List, Set, etc.
- [Complete KV Guide](./KV_STORE.md) - Comprehensive reference

