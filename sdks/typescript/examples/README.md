# Synap TypeScript SDK Examples

This directory contains comprehensive examples for all Synap features, organized by functionality group.

## Prerequisites

- Node.js 18+ installed
- Synap server running (Docker container on `localhost:15500`)

## Quick Start

### Run a specific example:

```bash
npx tsx examples/kv-store.ts
```

### Run all examples:

```bash
npx tsx examples/run-all.ts
```

## Available Examples

### Core Data Structures

1. **`kv-store.ts`** - Key-Value Store Operations
   - SET, GET, DELETE
   - APPEND, STRLEN, GETRANGE
   - TTL support
   - Statistics

2. **`hash.ts`** - Hash Operations
   - HSET, HGET, HGETALL
   - HMSET, HINCRBY
   - HLEN, HDEL
   - Statistics

3. **`list.ts`** - List Operations
   - LPUSH, RPUSH
   - LRANGE, LLEN
   - LPOP, RPOP, LINDEX
   - Statistics

4. **`set.ts`** - Set Operations
   - SADD, SMEMBERS
   - SCARD, SISMEMBER
   - SPOP, SREM
   - Statistics

5. **`sorted-set.ts`** - Sorted Set Operations
   - ZADD, ZCARD
   - ZRANGE, ZRANK, ZSCORE
   - ZINCRBY
   - Statistics

### Messaging & Events

6. **`queue.ts`** - Queue Operations
   - CREATE QUEUE
   - PUBLISH, CONSUME
   - ACK, STATS
   - LIST QUEUES

7. **`stream.ts`** - Stream Operations
   - CREATE ROOM
   - PUBLISH events
   - CONSUME events
   - STATS, LIST ROOMS

8. **`pubsub.ts`** - Pub/Sub Operations
   - PUBLISH to topics
   - STATS
   - LIST TOPICS

### Advanced Features

9. **`transactions.ts`** - Transaction Operations
   - WATCH keys
   - MULTI/EXEC
   - UNWATCH
   - Optimistic locking

10. **`key-management.ts`** - Key Management
    - EXISTS, TYPE
    - RENAME, COPY
    - RANDOMKEY

11. **`hyperloglog.ts`** - HyperLogLog Operations
    - PFADD
    - PFCOUNT
    - PFMERGE

12. **`bitmap.ts`** - Bitmap Operations
    - SETBIT, GETBIT
    - BITCOUNT, BITPOS
    - Statistics

13. **`geospatial.ts`** - Geospatial Operations
    - GEOADD
    - GEODIST, GEORADIUS
    - GEOPOS
    - Statistics

14. **`scripting.ts`** - Lua Scripting
    - EVAL
    - LOAD, EVALSHA
    - EXISTS, FLUSH

## Other Examples

- **`basic-usage.ts`** - Basic usage patterns
- **`queue-worker.ts`** - Queue worker patterns
- **`reactive-patterns.ts`** - Reactive programming with RxJS
- **`pubsub-patterns.ts`** - Pub/Sub patterns
- **`stream-patterns.ts`** - Stream consumption patterns
- **`all-features.ts`** - Complete example with all features (legacy)

## Server Connection

All examples connect to:
- **URL**: `http://localhost:15500`
- **Timeout**: 30000ms

To use a different server, modify the `Synap` constructor in each example:

```typescript
const synap = new Synap({
  url: 'http://your-server:15500',
  timeout: 30000,
});
```

## Running Examples

### Individual Example

```bash
cd sdks/typescript
npx tsx examples/kv-store.ts
```

### All Examples

```bash
cd sdks/typescript
npx tsx examples/run-all.ts
```

This will run all examples sequentially and provide a summary at the end.

## Notes

- Each example is self-contained and can be run independently
- Examples clean up after themselves (close connections)
- Some examples use unique keys/timestamps to avoid conflicts
- Examples demonstrate both SDK methods and direct command usage where needed

