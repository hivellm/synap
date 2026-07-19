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

### How values are encoded

Every REST endpoint that writes a value follows one rule: a **JSON string is
stored as raw UTF-8**, and any other JSON value (object, array, number,
boolean) is stored in its JSON form. So `{"value": "cd"}` writes the two bytes
`cd`, not the four bytes `"cd"`.

This matters when you combine operations. Appending `cd` to a key holding `ab`
gives exactly `abcd` — the operand is never wrapped in quotes on the way in:

```bash
curl -X POST http://localhost:15500/kv/set \
  -H "Content-Type: application/json" \
  -d '{"key": "user:1", "value": "ab"}'

curl -X POST http://localhost:15500/kv/user:1/append \
  -H "Content-Type: application/json" \
  -d '{"value": "cd"}'

curl http://localhost:15500/kv/get/user:1   # "abcd"
```

Reads apply the inverse: bytes that parse as JSON come back decoded, and bytes
that do not come back as the string they are. A value that is not valid UTF-8
cannot be represented in a JSON response — use the RESP3 or SynapRPC transport
for binary values.

### Append

```bash
curl -X POST http://localhost:15500/kv/user:1/append \
  -H "Content-Type: application/json" \
  -d '{"value": " World"}'
```

### Get Range (GETRANGE)

```bash
curl "http://localhost:15500/kv/user:1/getrange?start=0&end=4"
```

### Set Range (SETRANGE)

```bash
curl -X POST http://localhost:15500/kv/user:1/setrange \
  -H "Content-Type: application/json" \
  -d '{
    "offset": 0,
    "value": "Hello"
  }'
```

### Get and Set (GETSET)

```bash
curl -X POST http://localhost:15500/kv/user:1/getset \
  -H "Content-Type: application/json" \
  -d '{"value": "New Value"}'
```

Returns old value and sets new value atomically.

### Reading the previous value on SET

`SET` with `"get": true` returns whatever the key held before, under
`old_value`. The field is **omitted** when the key did not exist, so an absent
`old_value` means "there was nothing here" — not "the value could not be
decoded".

```bash
curl -X POST http://localhost:15500/kv/set \
  -H "Content-Type: application/json" \
  -d '{"key": "user:1", "value": "second", "get": true}'
# {"success":true,"key":"user:1","written":true,"old_value":"first"}
```

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

