---
title: Key-Value Store
module: kv-store
id: kv-store-index
order: 0
description: Redis-compatible key-value store operations
tags: [kv-store, key-value, redis, storage]
---

# Key-Value Store

Complete guide to Synap's Redis-compatible key-value store.

## Guides

### [Basic Operations](./BASIC.md)

Fundamental key-value operations:

- SET, GET, DELETE
- TTL (Time To Live)
- EXISTS, KEYS
- Basic patterns

### [Advanced Operations](./ADVANCED.md)

Advanced features:

- Batch operations (MSET, MGET)
- Atomic operations (INCR, DECR)
- Expiration management
- Memory optimization

### [Data Structures](./DATA_STRUCTURES.md)

Complex data structures:

- **Hash** - Field-value maps (HSET, HGET, HDEL)
- **List** - Ordered sequences (LPUSH, RPOP, LRANGE)
- **Set** - Unordered unique collections (SADD, SREM, SINTER)
- **Sorted Set** - Scored members with ranking (ZADD, ZRANGE, ZRANK)
- **HyperLogLog** - Probabilistic cardinality estimation
- **Geospatial** - GEO commands (GEOADD, GEORADIUS)
- **Bitmap** - Bit manipulation (SETBIT, GETBIT, BITCOUNT)

### [Complete KV Guide](./KV_STORE.md)

Comprehensive reference:

- All operations
- Performance tips
- Best practices
- Examples

## Quick Start

### Set and Get

```bash
# Set a key
curl -X POST http://localhost:15500/kv/set \
  -H "Content-Type: application/json" \
  -d '{"key":"user:1","value":"John Doe"}'

# Get the key
curl http://localhost:15500/kv/get/user:1
```

### With TTL

```bash
# Set with expiration (1 hour)
curl -X POST http://localhost:15500/kv/set \
  -H "Content-Type: application/json" \
  -d '{"key":"session:abc","value":"data","ttl":3600}'
```

### Delete

```bash
# Delete a key
curl -X DELETE http://localhost:15500/kv/del/user:1
```

## Data Structures

### Hash

```bash
# Set hash field
curl -X POST http://localhost:15500/hash/user:1/hset \
  -H "Content-Type: application/json" \
  -d '{"field":"name","value":"John"}'

# Get hash field
curl http://localhost:15500/hash/user:1/hget/name
```

### List

```bash
# Push to list
curl -X POST http://localhost:15500/list/tasks/lpush \
  -H "Content-Type: application/json" \
  -d '{"value":"task1"}'

# Pop from list
curl -X POST http://localhost:15500/list/tasks/rpop
```

### Set

```bash
# Add to set
curl -X POST http://localhost:15500/set/tags/sadd \
  -H "Content-Type: application/json" \
  -d '{"member":"tag1"}'

# Check membership
curl http://localhost:15500/set/tags/sismember/tag1
```

### Sorted Set

```bash
# Add with score
curl -X POST http://localhost:15500/sortedset/leaderboard/zadd \
  -H "Content-Type: application/json" \
  -d '{"member":"player1","score":100}'

# Get range
curl http://localhost:15500/sortedset/leaderboard/zrange?start=0&stop=9
```

## Related Topics

- [API Reference](../api/API_REFERENCE.md) - Complete API documentation
- [Configuration Guide](../configuration/CONFIGURATION.md) - KV store configuration
- [Use Cases](../use-cases/SESSION_STORE.md) - Session store example

