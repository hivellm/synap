---
title: Data Structures
module: kv-store
id: kv-data-structures
order: 3
description: Hash, List, Set, Sorted Set, and other data structures
tags: [kv-store, data-structures, hash, list, set, sorted-set]
---

# Data Structures

Complete guide to advanced data structures in Synap's key-value store.

## Overview

Synap supports multiple Redis-compatible data structures:
- **Hash**: Field-value maps within keys
- **List**: Ordered sequences
- **Set**: Unordered unique collections
- **Sorted Set**: Scored members with ranking
- **HyperLogLog**: Probabilistic cardinality estimation
- **Geospatial**: GEO commands
- **Bitmap**: Bit manipulation

## Hash

Field-value maps within keys (Redis HSET, HGET, etc.).

### HSET - Set Hash Field

```bash
curl -X POST http://localhost:15500/hash/user:1/hset \
  -H "Content-Type: application/json" \
  -d '{"field":"name","value":"John"}'
```

### HGET - Get Hash Field

```bash
curl http://localhost:15500/hash/user:1/hget/name
```

### HGETALL - Get All Fields

```bash
curl http://localhost:15500/hash/user:1/hgetall
```

**Response:**
```json
{
  "name": "John",
  "age": "30",
  "email": "john@example.com"
}
```

### HDEL - Delete Hash Field

```bash
curl -X DELETE http://localhost:15500/hash/user:1/hdel/name
```

### HMSET - Set Multiple Fields

```bash
curl -X POST http://localhost:15500/hash/user:1/mset \
  -H "Content-Type: application/json" \
  -d '{
    "fields": {
      "name": "John",
      "age": "30",
      "email": "john@example.com"
    }
  }'
```

### HKEYS - Get All Keys

```bash
curl http://localhost:15500/hash/user:1/hkeys
```

### HVALS - Get All Values

```bash
curl http://localhost:15500/hash/user:1/hvals
```

### HEXISTS - Check Field Exists

```bash
curl http://localhost:15500/hash/user:1/hexists/name
```

## List

Ordered sequences (Redis LPUSH, RPOP, LRANGE, etc.).

### LPUSH - Push to Left

```bash
curl -X POST http://localhost:15500/list/tasks/lpush \
  -H "Content-Type: application/json" \
  -d '{"value":"task1"}'
```

### RPUSH - Push to Right

```bash
curl -X POST http://localhost:15500/list/tasks/rpush \
  -H "Content-Type: application/json" \
  -d '{"value":"task2"}'
```

### LPOP - Pop from Left

```bash
curl -X POST http://localhost:15500/list/tasks/lpop
```

### RPOP - Pop from Right

```bash
curl -X POST http://localhost:15500/list/tasks/rpop
```

### LRANGE - Get Range

```bash
curl "http://localhost:15500/list/tasks/lrange?start=0&stop=-1"
```

### LLEN - Get Length

```bash
curl http://localhost:15500/list/tasks/llen
```

### LINDEX - Get by Index

```bash
curl http://localhost:15500/list/tasks/lindex/0
```

## Set

Unordered unique collections (Redis SADD, SREM, SINTER, etc.).

### SADD - Add Member

```bash
curl -X POST http://localhost:15500/set/tags/sadd \
  -H "Content-Type: application/json" \
  -d '{"member":"tag1"}'
```

### SREM - Remove Member

```bash
curl -X DELETE http://localhost:15500/set/tags/srem/tag1
```

### SMEMBERS - Get All Members

```bash
curl http://localhost:15500/set/tags/smembers
```

### SISMEMBER - Check Membership

```bash
curl http://localhost:15500/set/tags/sismember/tag1
```

### SCARD - Get Cardinality

```bash
curl http://localhost:15500/set/tags/scard
```

### SINTER - Intersection

```bash
curl -X POST http://localhost:15500/set/sinter \
  -H "Content-Type: application/json" \
  -d '{"keys":["tags1","tags2"]}'
```

### SUNION - Union

```bash
curl -X POST http://localhost:15500/set/sunion \
  -H "Content-Type: application/json" \
  -d '{"keys":["tags1","tags2"]}'
```

### SDIFF - Difference

```bash
curl -X POST http://localhost:15500/set/sdiff \
  -H "Content-Type: application/json" \
  -d '{"keys":["tags1","tags2"]}'
```

## Sorted Set

Scored members with ranking (Redis ZADD, ZRANGE, ZRANK, etc.).

### ZADD - Add Member with Score

```bash
curl -X POST http://localhost:15500/sortedset/leaderboard/zadd \
  -H "Content-Type: application/json" \
  -d '{"member":"player1","score":100}'
```

### ZRANGE - Get Range by Rank

```bash
curl "http://localhost:15500/sortedset/leaderboard/zrange?start=0&stop=9"
```

### ZREVRANGE - Get Range by Rank (Reverse)

```bash
curl "http://localhost:15500/sortedset/leaderboard/zrevrange?start=0&stop=9"
```

### ZRANK - Get Rank

```bash
curl http://localhost:15500/sortedset/leaderboard/zrank/player1
```

### ZSCORE - Get Score

```bash
curl http://localhost:15500/sortedset/leaderboard/zscore/player1
```

### ZCARD - Get Cardinality

```bash
curl http://localhost:15500/sortedset/leaderboard/zcard
```

### ZCOUNT - Count by Score Range

```bash
curl "http://localhost:15500/sortedset/leaderboard/zcount?min=100&max=200"
```

### ZINTER - Intersection

```bash
curl -X POST http://localhost:15500/sortedset/zinter \
  -H "Content-Type: application/json" \
  -d '{
    "keys": ["leaderboard1", "leaderboard2"],
    "weights": [1, 1],
    "aggregate": "sum"
  }'
```

### ZUNION - Union

```bash
curl -X POST http://localhost:15500/sortedset/zunion \
  -H "Content-Type: application/json" \
  -d '{
    "keys": ["leaderboard1", "leaderboard2"],
    "weights": [1, 1],
    "aggregate": "sum"
  }'
```

## HyperLogLog

Probabilistic cardinality estimation.

### PFADD - Add Elements

```bash
curl -X POST http://localhost:15500/hyperloglog/visitors/pfadd \
  -H "Content-Type: application/json" \
  -d '{"elements":["user1","user2","user3"]}'
```

### PFCOUNT - Get Cardinality

```bash
curl http://localhost:15500/hyperloglog/visitors/pfcount
```

### PFMERGE - Merge HyperLogLogs

```bash
curl -X POST http://localhost:15500/hyperloglog/pfmerge \
  -H "Content-Type: application/json" \
  -d '{"dest":"visitors","sources":["visitors1","visitors2"]}'
```

## Geospatial

Redis-compatible GEO commands.

### GEOADD - Add Location

```bash
curl -X POST http://localhost:15500/geo/locations/geoadd \
  -H "Content-Type: application/json" \
  -d '{
    "member": "restaurant1",
    "longitude": -122.4194,
    "latitude": 37.7749
  }'
```

### GEORADIUS - Find Within Radius

```bash
curl "http://localhost:15500/geo/locations/georadius?longitude=-122.4194&latitude=37.7749&radius=1000&unit=km"
```

### GEOSEARCH - Search by Box

```bash
curl "http://localhost:15500/geo/locations/geosearch?longitude=-122.4194&latitude=37.7749&width=1000&height=1000&unit=km"
```

### GEODIST - Get Distance

```bash
curl "http://localhost:15500/geo/locations/geodist?member1=restaurant1&member2=restaurant2&unit=km"
```

## Bitmap

Bit manipulation operations.

### SETBIT - Set Bit

```bash
curl -X POST http://localhost:15500/bitmap/user:1:online/setbit \
  -H "Content-Type: application/json" \
  -d '{"offset":100,"value":1}'
```

### GETBIT - Get Bit

```bash
curl http://localhost:15500/bitmap/user:1:online/getbit/100
```

### BITCOUNT - Count Set Bits

```bash
curl "http://localhost:15500/bitmap/user:1:online/bitcount?start=0&end=-1"
```

### BITOP - Bitwise Operations

```bash
curl -X POST http://localhost:15500/bitmap/bitop \
  -H "Content-Type: application/json" \
  -d '{
    "operation": "AND",
    "dest": "result",
    "keys": ["bitmap1", "bitmap2"]
  }'
```

## Using SDKs

### Python

```python
from synap_sdk import SynapClient

client = SynapClient("http://localhost:15500")

# Hash
client.hash.hset("user:1", "name", "John")
name = client.hash.hget("user:1", "name")

# List
client.list.lpush("tasks", "task1")
task = client.list.rpop("tasks")

# Set
client.set.sadd("tags", "tag1")
members = client.set.smembers("tags")

# Sorted Set
client.sortedset.zadd("leaderboard", "player1", 100)
top_players = client.sortedset.zrange("leaderboard", 0, 9)
```

### TypeScript

```typescript
import { Synap } from "@hivehub/synap";

const client = new SynapClient("http://localhost:15500");

// Hash
await client.hash.hset("user:1", "name", "John");
const name = await client.hash.hget("user:1", "name");

// List
await client.list.lpush("tasks", "task1");
const task = await client.list.rpop("tasks");

// Set
await client.set.sadd("tags", "tag1");
const members = await client.set.smembers("tags");

// Sorted Set
await client.sortedset.zadd("leaderboard", "player1", 100);
const topPlayers = await client.sortedset.zrange("leaderboard", 0, 9);
```

### Rust

```rust
use synap_sdk::SynapClient;

let client = SynapClient::new("http://localhost:15500")?;

// Hash
client.hash.hset("user:1", "name", "John").await?;
let name = client.hash.hget("user:1", "name").await?;

// List
client.list.lpush("tasks", "task1").await?;
let task = client.list.rpop("tasks").await?;

// Set
client.set.sadd("tags", "tag1").await?;
let members = client.set.smembers("tags").await?;

// Sorted Set
client.sortedset.zadd("leaderboard", "player1", 100.0).await?;
let top_players = client.sortedset.zrange("leaderboard", 0, 9).await?;
```

## Best Practices

### Choose Appropriate Structure

- **Hash**: For objects with multiple fields
- **List**: For ordered sequences (queues, timelines)
- **Set**: For unique collections (tags, followers)
- **Sorted Set**: For ranked data (leaderboards, time-series)

### Use Hash for Objects

```python
# Good: Use hash for user object
client.hash.hset("user:1", "name", "John")
client.hash.hset("user:1", "age", "30")
client.hash.hset("user:1", "email", "john@example.com")

# Less efficient: Multiple keys
client.kv.set("user:1:name", "John")
client.kv.set("user:1:age", "30")
client.kv.set("user:1:email", "john@example.com")
```

### Use Sorted Sets for Rankings

```python
# Leaderboard
client.sortedset.zadd("leaderboard", "player1", 100)
client.sortedset.zadd("leaderboard", "player2", 200)

# Get top 10
top_10 = client.sortedset.zrevrange("leaderboard", 0, 9)
```

## Related Topics

- [Basic Operations](./BASIC.md) - Basic KV operations
- [Advanced Operations](./ADVANCED.md) - Advanced features
- [Complete KV Guide](./KV_STORE.md) - Comprehensive reference

