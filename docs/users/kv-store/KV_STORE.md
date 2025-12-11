---
title: Complete KV Store Guide
module: kv-store
id: kv-store-complete
order: 4
description: Comprehensive key-value store reference
tags: [kv-store, reference, complete, guide]
---

# Complete KV Store Guide

Comprehensive reference for Synap's key-value store operations.

## Overview

Synap provides a Redis-compatible key-value store with:
- **High Performance**: 12M+ reads/sec, 44K+ writes/sec
- **Low Latency**: < 1ms (87ns typical for GET)
- **TTL Support**: Automatic expiration
- **Data Structures**: Hash, List, Set, Sorted Set, HyperLogLog, Geospatial, Bitmap
- **Atomic Operations**: INCR, DECR, and more

## Basic Operations

### SET

Set a key-value pair.

```bash
curl -X POST http://localhost:15500/kv/set \
  -H "Content-Type: application/json" \
  -d '{"key":"user:1","value":"John Doe"}'
```

### GET

Get value by key.

```bash
curl http://localhost:15500/kv/get/user:1
```

### DELETE

Delete a key.

```bash
curl -X DELETE http://localhost:15500/kv/del/user:1
```

### EXISTS

Check if key exists.

```bash
curl http://localhost:15500/kv/exists/user:1
```

## TTL Operations

### SET with TTL

```bash
curl -X POST http://localhost:15500/kv/set \
  -H "Content-Type: application/json" \
  -d '{"key":"session:abc","value":"data","ttl":3600}'
```

### EXPIRE

Set expiration on existing key.

```bash
curl -X POST http://localhost:15500/kv/expire/user:1 \
  -H "Content-Type: application/json" \
  -d '{"ttl":3600}'
```

### TTL

Get time to live.

```bash
curl http://localhost:15500/kv/ttl/user:1
```

### PERSIST

Remove expiration.

```bash
curl -X POST http://localhost:15500/kv/persist/user:1
```

## Batch Operations

### MSET

Set multiple key-value pairs.

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

### MGET

Get multiple values.

```bash
curl -X POST http://localhost:15500/kv/mget \
  -H "Content-Type: application/json" \
  -d '{"keys":["user:1","user:2","user:3"]}'
```

### MSETNX

Set multiple keys only if none exist.

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

## Atomic Operations

### INCR

Increment integer value.

```bash
curl -X POST http://localhost:15500/kv/incr/counter
```

### DECR

Decrement integer value.

```bash
curl -X POST http://localhost:15500/kv/decr/counter
```

### INCRBY

Increment by amount.

```bash
curl -X POST http://localhost:15500/kv/incrby/counter \
  -H "Content-Type: application/json" \
  -d '{"amount":5}'
```

### DECRBY

Decrement by amount.

```bash
curl -X POST http://localhost:15500/kv/decrby/counter \
  -H "Content-Type: application/json" \
  -d '{"amount":3}'
```

## String Operations

### APPEND

Append to string value.

```bash
curl -X POST http://localhost:15500/kv/append/user:1 \
  -H "Content-Type: application/json" \
  -d '{"value":" World"}'
```

### GETRANGE

Get substring.

```bash
curl "http://localhost:15500/kv/getrange/user:1?start=0&end=4"
```

### SETRANGE

Set substring.

```bash
curl -X POST http://localhost:15500/kv/setrange/user:1 \
  -H "Content-Type: application/json" \
  -d '{"offset":0,"value":"Hello"}'
```

### GETSET

Get and set atomically.

```bash
curl -X POST http://localhost:15500/kv/getset/user:1 \
  -H "Content-Type: application/json" \
  -d '{"value":"New Value"}'
```

## Data Structures

### Hash

Field-value maps within keys.

```bash
# HSET
curl -X POST http://localhost:15500/hash/user:1/hset \
  -H "Content-Type: application/json" \
  -d '{"field":"name","value":"John"}'

# HGET
curl http://localhost:15500/hash/user:1/hget/name

# HGETALL
curl http://localhost:15500/hash/user:1/hgetall

# HDEL
curl -X DELETE http://localhost:15500/hash/user:1/hdel/name
```

### List

Ordered sequences.

```bash
# LPUSH
curl -X POST http://localhost:15500/list/tasks/lpush \
  -H "Content-Type: application/json" \
  -d '{"value":"task1"}'

# RPOP
curl -X POST http://localhost:15500/list/tasks/rpop

# LRANGE
curl "http://localhost:15500/list/tasks/lrange?start=0&stop=-1"
```

### Set

Unordered unique collections.

```bash
# SADD
curl -X POST http://localhost:15500/set/tags/sadd \
  -H "Content-Type: application/json" \
  -d '{"member":"tag1"}'

# SMEMBERS
curl http://localhost:15500/set/tags/smembers

# SISMEMBER
curl http://localhost:15500/set/tags/sismember/tag1
```

### Sorted Set

Scored members with ranking.

```bash
# ZADD
curl -X POST http://localhost:15500/sortedset/leaderboard/zadd \
  -H "Content-Type: application/json" \
  -d '{"member":"player1","score":100}'

# ZRANGE
curl "http://localhost:15500/sortedset/leaderboard/zrange?start=0&stop=9"

# ZRANK
curl http://localhost:15500/sortedset/leaderboard/zrank/player1
```

## Statistics

### Get Stats

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

### Memory Usage

```bash
curl http://localhost:15500/memory/user:1/usage
```

## Best Practices

### Key Naming

- Use namespaces: `user:1`, `session:abc`, `cache:product:123`
- Be consistent with separators
- Keep keys short but descriptive

### TTL Management

- Set TTL for temporary data
- Use appropriate expiration times
- Monitor TTL usage

### Batch Operations

- Use MSET/MGET for multiple operations
- Reduces network round-trips
- Improves performance

## Related Topics

- [Basic Operations](./BASIC.md) - Basic KV operations
- [Advanced Operations](./ADVANCED.md) - Advanced features
- [Data Structures](./DATA_STRUCTURES.md) - Hash, List, Set, etc.

