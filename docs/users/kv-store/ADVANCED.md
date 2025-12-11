---
title: Advanced KV Operations
module: kv-store
id: kv-advanced
order: 2
description: Advanced key-value operations and features
tags: [kv-store, advanced, operations, optimization]
---

# Advanced KV Operations

Advanced key-value store operations and optimization techniques.

## Batch Operations

### Multiple Set (MSET)

```bash
curl -X POST http://localhost:15500/kv/mset \
  -H "Content-Type: application/json" \
  -d '{
    "pairs": [
      {"key": "user:1", "value": "John"},
      {"key": "user:2", "value": "Jane"},
      {"key": "user:3", "value": "Bob"}
    ]
  }'
```

### Multiple Get (MGET)

```bash
curl -X POST http://localhost:15500/kv/mget \
  -H "Content-Type: application/json" \
  -d '{
    "keys": ["user:1", "user:2", "user:3"]
  }'
```

**Response:**
```json
{
  "values": ["John", "Jane", "Bob"]
}
```

### Set if Not Exists (MSETNX)

```bash
curl -X POST http://localhost:15500/kv/msetnx \
  -H "Content-Type: application/json" \
  -d '{
    "pairs": [
      {"key": "user:1", "value": "John"},
      {"key": "user:2", "value": "Jane"}
    ]
  }'
```

Returns `true` only if all keys were set (none existed).

## Atomic Operations

### Increment (INCR)

```bash
curl -X POST http://localhost:15500/kv/incr/counter
```

**Response:**
```json
{
  "value": 1
}
```

### Decrement (DECR)

```bash
curl -X POST http://localhost:15500/kv/decr/counter
```

### Increment By (INCRBY)

```bash
curl -X POST http://localhost:15500/kv/incrby/counter \
  -H "Content-Type: application/json" \
  -d '{"amount": 5}'
```

### Decrement By (DECRBY)

```bash
curl -X POST http://localhost:15500/kv/decrby/counter \
  -H "Content-Type: application/json" \
  -d '{"amount": 3}'
```

## Expiration Management

### Set Expiration (EXPIRE)

```bash
curl -X POST http://localhost:15500/kv/expire/user:1 \
  -H "Content-Type: application/json" \
  -d '{"ttl": 3600}'
```

### Get Time To Live (TTL)

```bash
curl http://localhost:15500/kv/ttl/user:1
```

**Response:**
```json
{
  "ttl": 3599
}
```

Returns `-1` if key has no expiration, `-2` if key doesn't exist.

### Remove Expiration (PERSIST)

```bash
curl -X POST http://localhost:15500/kv/persist/user:1
```

## String Operations

### Append

```bash
curl -X POST http://localhost:15500/kv/append/user:1 \
  -H "Content-Type: application/json" \
  -d '{"value": " World"}'
```

### Get Range (GETRANGE)

```bash
curl "http://localhost:15500/kv/getrange/user:1?start=0&end=4"
```

### Set Range (SETRANGE)

```bash
curl -X POST http://localhost:15500/kv/setrange/user:1 \
  -H "Content-Type: application/json" \
  -d '{
    "offset": 0,
    "value": "Hello"
  }'
```

### Get and Set (GETSET)

```bash
curl -X POST http://localhost:15500/kv/getset/user:1 \
  -H "Content-Type: application/json" \
  -d '{"value": "New Value"}'
```

Returns old value and sets new value atomically.

## Memory Operations

### Get Memory Usage

```bash
curl http://localhost:15500/memory/user:1/usage
```

**Response:**
```json
{
  "bytes": 1024
}
```

## Performance Tips

### Use Batch Operations

Batch operations are more efficient than individual operations:

```python
# Good: Batch operation
client.kv.mset([
    ("user:1", "John"),
    ("user:2", "Jane"),
    ("user:3", "Bob")
])

# Less efficient: Individual operations
client.kv.set("user:1", "John")
client.kv.set("user:2", "Jane")
client.kv.set("user:3", "Bob")
```

### Use Appropriate TTL

Set TTL for temporary data to prevent memory bloat:

```python
# Session data with 1 hour TTL
client.kv.set("session:abc", session_data, ttl=3600)

# Cache with 5 minute TTL
client.kv.set("cache:product:123", product_data, ttl=300)
```

### Monitor Memory Usage

```bash
# Get overall stats
curl http://localhost:15500/kv/stats

# Get specific key memory usage
curl http://localhost:15500/memory/user:1/usage
```

## Related Topics

- [Basic Operations](./BASIC.md) - Basic KV operations
- [Data Structures](./DATA_STRUCTURES.md) - Hash, List, Set, etc.
- [Complete KV Guide](./KV_STORE.md) - Comprehensive reference

