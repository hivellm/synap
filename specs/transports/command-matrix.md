# Command Parity Matrix — v0.11.0

Legend: ✅ implemented | ❌ missing | N/A not applicable

## KV Store

| Command | HTTP Route | SynapRPC | RESP3 | Notes |
|---------|-----------|----------|-------|-------|
| SET | POST /kv/set | ✅ SET | ✅ SET | |
| GET | GET /kv/get/{key} | ✅ GET | ✅ GET | |
| DEL | DELETE /kv/del/{key} | ✅ DEL | ✅ DEL | |
| EXISTS | GET /kv/{key}/exists | ✅ EXISTS | ✅ EXISTS | |
| EXPIRE | POST /kv/{key}/expire | ✅ EXPIRE | ✅ EXPIRE | |
| TTL | GET /kv/{key}/ttl | ✅ TTL | ✅ TTL | |
| PERSIST | POST /kv/{key}/persist | ✅ PERSIST | ✅ PERSIST | |
| INCR | POST /kv/{key}/incr | ✅ INCR | ✅ INCR | |
| INCRBY | POST /kv/{key}/incrby | ✅ INCRBY | ✅ INCRBY | |
| DECR | POST /kv/{key}/decr | ✅ DECR | ✅ DECR | |
| DECRBY | POST /kv/{key}/decrby | ✅ DECRBY | ✅ DECRBY | |
| MSET | POST /kv/mset | ✅ MSET | ✅ MSET | |
| MGET | POST /kv/mget | ✅ MGET | ✅ MGET | |
| KEYS | GET /kv/keys | ✅ KEYS | ✅ KEYS | |
| SCAN | GET /kv/scan | ✅ SCAN | ✅ SCAN | |
| APPEND | POST /kv/{key}/append | ✅ APPEND | ❌ | RESP3: add APPEND |
| GETRANGE | GET /kv/{key}/getrange | ✅ GETRANGE | ❌ | RESP3: GETRANGE |
| SETRANGE | POST /kv/{key}/setrange | ✅ SETRANGE | ❌ | RESP3: SETRANGE |
| STRLEN | GET /kv/{key}/strlen | ✅ STRLEN | ❌ | RESP3: STRLEN |
| GETSET | POST /kv/{key}/getset | ✅ GETSET | ❌ | RESP3: GETSET |
| MSETNX | POST /kv/msetnx | ✅ MSETNX | ❌ | RESP3: MSETNX |
| DBSIZE | GET /kv/dbsize | ✅ DBSIZE | ❌ | RESP3: DBSIZE |
| FLUSHDB | POST /kv/flushdb | ✅ FLUSHDB | ✅ FLUSHDB | |
| FLUSHALL | POST /kv/flushall | ✅ FLUSHALL | ✅ FLUSHALL | |
| KV.STATS | GET /kv/stats | ✅ KVSTATS | ❌ | RESP3: INFO kv |

## Key Management

| Command | HTTP Route | SynapRPC | RESP3 | Notes |
|---------|-----------|----------|-------|-------|
| TYPE | GET /key/{key}/type | ❌ | ❌ | RESP3: TYPE |
| RENAME | POST /key/{key}/rename | ❌ | ❌ | RESP3: RENAME |
| RENAMENX | POST /key/{key}/renamenx | ❌ | ❌ | RESP3: RENAMENX |
| COPY | POST /key/{key}/copy | ❌ | ❌ | RESP3: COPY |
| RANDOMKEY | GET /key/randomkey | ❌ | ❌ | RESP3: RANDOMKEY |

## Hash

| Command | HTTP Route | SynapRPC | RESP3 | Notes |
|---------|-----------|----------|-------|-------|
| HSET | POST /hash/{key}/set | ✅ HSET | ✅ HSET | |
| HGET | GET /hash/{key}/{field} | ✅ HGET | ✅ HGET | |
| HDEL | DELETE /hash/{key}/{field} | ✅ HDEL | ✅ HDEL | |
| HGETALL | GET /hash/{key}/getall | ✅ HGETALL | ✅ HGETALL | |
| HLEN | GET /hash/{key}/len | ✅ HLEN | ✅ HLEN | |
| HEXISTS | GET /hash/{key}/{field}/exists | ✅ HEXISTS | ✅ HEXISTS | |
| HMSET | POST /hash/{key}/mset | ✅ HMSET | ✅ HMSET | |
| HMGET | POST /hash/{key}/mget | ✅ HMGET | ✅ HMGET | |
| HKEYS | GET /hash/{key}/keys | ✅ HKEYS | ❌ | RESP3: HKEYS |
| HVALS | GET /hash/{key}/vals | ✅ HVALS | ❌ | RESP3: HVALS |
| HINCRBY | POST /hash/{key}/incrby | ❌ | ❌ | RESP3: HINCRBY |
| HINCRBYFLOAT | POST /hash/{key}/incrbyfloat | ❌ | ❌ | RESP3: HINCRBYFLOAT |
| HSETNX | POST /hash/{key}/setnx | ❌ | ❌ | RESP3: HSETNX |

## List

| Command | HTTP Route | SynapRPC | RESP3 | Notes |
|---------|-----------|----------|-------|-------|
| LPUSH | POST /list/{key}/lpush | ✅ | ✅ | |
| RPUSH | POST /list/{key}/rpush | ✅ | ✅ | |
| LPOP | POST /list/{key}/lpop | ✅ | ✅ | |
| RPOP | POST /list/{key}/rpop | ✅ | ✅ | |
| LRANGE | GET /list/{key}/range | ✅ | ✅ | |
| LLEN | GET /list/{key}/len | ✅ | ✅ | |
| LINDEX | GET /list/{key}/index | ❌ | ❌ | RESP3: LINDEX |
| LSET | POST /list/{key}/set | ❌ | ❌ | RESP3: LSET |
| LTRIM | POST /list/{key}/trim | ❌ | ❌ | RESP3: LTRIM |
| LREM | POST /list/{key}/rem | ❌ | ❌ | RESP3: LREM |
| LINSERT | POST /list/{key}/insert | ❌ | ❌ | RESP3: LINSERT |
| LPUSHX | POST /list/{key}/lpushx | ❌ | ❌ | RESP3: LPUSHX |
| RPUSHX | POST /list/{key}/rpushx | ❌ | ❌ | RESP3: RPUSHX |
| RPOPLPUSH | POST /list/{source}/rpoplpush | ❌ | ❌ | RESP3: RPOPLPUSH |

## Set

| Command | HTTP Route | SynapRPC | RESP3 | Notes |
|---------|-----------|----------|-------|-------|
| SADD | POST /set/{key}/add | ✅ | ✅ | |
| SREM | POST /set/{key}/rem | ✅ | ✅ | |
| SISMEMBER | GET /set/{key}/ismember | ✅ | ✅ | |
| SMEMBERS | GET /set/{key}/members | ✅ | ✅ | |
| SCARD | GET /set/{key}/card | ✅ | ✅ | |
| SPOP | POST /set/{key}/pop | ❌ | ❌ | RESP3: SPOP |
| SRANDMEMBER | GET /set/{key}/randmember | ❌ | ❌ | RESP3: SRANDMEMBER |
| SMOVE | POST /set/{source}/move/{dest} | ❌ | ❌ | RESP3: SMOVE |
| SINTER | POST /set/inter | ❌ | ❌ | RESP3: SINTER |
| SUNION | POST /set/union | ❌ | ❌ | RESP3: SUNION |
| SDIFF | POST /set/diff | ❌ | ❌ | RESP3: SDIFF |

## Sorted Set

| Command | HTTP Route | SynapRPC | RESP3 | Notes |
|---------|-----------|----------|-------|-------|
| ZADD | POST /sortedset/{key}/zadd | ✅ | ✅ | |
| ZREM | POST /sortedset/{key}/zrem | ✅ | ✅ | |
| ZSCORE | GET /sortedset/{key}/zscore | ✅ | ✅ | |
| ZCARD | GET /sortedset/{key}/zcard | ✅ | ✅ | |
| ZRANGE | GET /sortedset/{key}/zrange | ✅ | ✅ | |
| ZRANK | GET /sortedset/{key}/zrank | ❌ | ❌ | |
| ZREVRANK | GET /sortedset/{key}/zrevrank | ❌ | ❌ | |
| ZREVRANGE | GET /sortedset/{key}/zrevrange | ❌ | ❌ | |
| ZCOUNT | GET /sortedset/{key}/zcount | ❌ | ❌ | |
| ZMSCORE | POST /sortedset/{key}/zmscore | ❌ | ❌ | |
| ZRANGEBYSCORE | GET /sortedset/{key}/zrangebyscore | ❌ | ❌ | |
| ZPOPMIN | POST /sortedset/{key}/zpopmin | ❌ | ❌ | |
| ZPOPMAX | POST /sortedset/{key}/zpopmax | ❌ | ❌ | |
| ZREMRANGEBYRANK | POST /sortedset/{key}/zremrangebyrank | ❌ | ❌ | |
| ZREMRANGEBYSCORE | POST /sortedset/{key}/zremrangebyscore | ❌ | ❌ | |
| ZINTERSTORE | POST /sortedset/{dest}/zinterstore | ❌ | ❌ | |
| ZUNIONSTORE | POST /sortedset/{dest}/zunionstore | ❌ | ❌ | |
| ZDIFFSTORE | POST /sortedset/{dest}/zdiffstore | ❌ | ❌ | |

## HyperLogLog

| Command | HTTP Route | SynapRPC | RESP3 | Notes |
|---------|-----------|----------|-------|-------|
| PFADD | POST /hyperloglog/{key}/pfadd | ✅ | ✅ | |
| PFCOUNT | GET /hyperloglog/{key}/pfcount | ✅ | ✅ | |
| PFMERGE | POST /hyperloglog/{dest}/pfmerge | ✅ PFMERGE | ❌ | RESP3: add PFMERGE |
| HLL.STATS | GET /hyperloglog/stats | ✅ HLLSTATS | ❌ | RESP3: add |

## Bitmap

| Command | HTTP Route | SynapRPC | RESP3 | Notes |
|---------|-----------|----------|-------|-------|
| SETBIT | POST /bitmap/{key}/setbit | ✅ | ✅ | |
| GETBIT | GET /bitmap/{key}/getbit | ✅ | ✅ | |
| BITCOUNT | GET /bitmap/{key}/bitcount | ✅ | ✅ | |
| BITPOS | GET /bitmap/{key}/bitpos | ❌ | ❌ | |
| BITOP | POST /bitmap/{dest}/bitop | ❌ | ❌ | |
| BITFIELD | POST /bitmap/{key}/bitfield | ❌ | ❌ | |

## Geospatial

| Command | HTTP Route | SynapRPC | RESP3 | Notes |
|---------|-----------|----------|-------|-------|
| GEOADD | POST /geospatial/{key}/geoadd | ✅ GEOADD | ❌ | RESP3: add |
| GEODIST | GET /geospatial/{key}/geodist | ✅ GEODIST | ❌ | RESP3: add |
| GEORADIUS | GET /geospatial/{key}/georadius | ✅ GEORADIUS | ❌ | RESP3: add |
| GEORADIUSBYMEMBER | GET /geospatial/{key}/georadiusbymember | ✅ GEORADIUSBYMEMBER | ❌ | RESP3: add |
| GEOPOS | GET /geospatial/{key}/geopos | ✅ GEOPOS | ❌ | RESP3: add |
| GEOHASH | GET /geospatial/{key}/geohash | ✅ GEOHASH | ❌ | RESP3: add |
| GEOSEARCH | POST /geospatial/{key}/geosearch | ✅ GEOSEARCH | ❌ | RESP3: add |
| GEO.STATS | GET /geospatial/stats | ✅ GEOSTATS | ❌ | RESP3: add |

## Queue

| Command | HTTP Route | SynapRPC | RESP3 | Notes |
|---------|-----------|----------|-------|-------|
| QUEUE.CREATE | POST /queue/{name} | ✅ QCREATE | ❌ | RESP3: add |
| QUEUE.DELETE | DELETE /queue/{name} | ✅ QDELETE | ❌ | RESP3: add |
| QUEUE.LIST | GET /queue/list | ✅ QLIST | ❌ | RESP3: add |
| QUEUE.PUBLISH | POST /queue/{name}/publish | ✅ QPUBLISH | ❌ | RESP3: add |
| QUEUE.CONSUME | POST /queue/{name}/consume | ✅ QCONSUME | ❌ | RESP3: add QCONSUME |
| QUEUE.ACK | POST /queue/{name}/ack | ✅ QACK | ❌ | RESP3: add |
| QUEUE.NACK | POST /queue/{name}/nack | ✅ QNACK | ❌ | RESP3: add |
| QUEUE.STATS | GET /queue/{name}/stats | ✅ QSTATS | ❌ | RESP3: add |
| QUEUE.PURGE | POST /queue/{name}/purge | ✅ QPURGE | ❌ | RESP3: add |

## Stream

| Command | HTTP Route | SynapRPC | RESP3 | Notes |
|---------|-----------|----------|-------|-------|
| STREAM.CREATE | POST /stream/{room} | ✅ SCREATE | ❌ | RESP3: XADD-create |
| STREAM.PUBLISH | POST /stream/{room}/publish | ✅ SPUBLISH | ❌ | RESP3: XADD |
| STREAM.READ | POST /stream/{room}/consume | ✅ SREAD | ❌ | RESP3: XREAD |
| STREAM.DELETE | DELETE /stream/{room} | ✅ SDELETE | ❌ | RESP3: add |
| STREAM.LIST | GET /stream/list | ✅ SLIST | ❌ | RESP3: add |
| STREAM.STATS | GET /stream/{room}/stats | ✅ SSTATS | ❌ | RESP3: XINFO |
| STREAM.REPLAY | POST /stream/{room}/replay | ❌ | ❌ | RESP3: XRANGE |
| XACK | (group ack) | ❌ | ❌ | RESP3: XACK |

## Pub/Sub

| Command | HTTP Route | SynapRPC | RESP3 | Notes |
|---------|-----------|----------|-------|-------|
| PUBLISH | POST /pubsub/{topic}/publish | ✅ PUBLISH | ❌ | RESP3: PUBLISH |
| SUBSCRIBE | POST /pubsub/subscribe (WS) | ✅ SUBSCRIBE | ❌ | RPC returns subscriber_id; push via connection layer |
| UNSUBSCRIBE | POST /pubsub/unsubscribe | ✅ UNSUBSCRIBE | ❌ | RESP3: UNSUBSCRIBE |
| TOPICS | GET /pubsub/topics | ✅ TOPICS | ❌ | RESP3: PUBSUB CHANNELS |
| PSUBSCRIBE | N/A | ❌ | ❌ | RESP3: PSUBSCRIBE |
| PUBSUB.STATS | GET /pubsub/stats | ✅ PSSTATS | ❌ | RESP3: add |

## Transactions

| Command | HTTP Route | SynapRPC | RESP3 | Notes |
|---------|-----------|----------|-------|-------|
| MULTI | POST /transaction/multi | ✅ MULTI | ❌ | RPC: client_id as arg[0] |
| EXEC | POST /transaction/exec | ✅ EXEC | ❌ | RESP3: EXEC |
| DISCARD | POST /transaction/discard | ✅ DISCARD | ❌ | RESP3: DISCARD |
| WATCH | POST /transaction/watch | ✅ WATCH | ❌ | RESP3: WATCH |
| UNWATCH | POST /transaction/unwatch | ✅ UNWATCH | ❌ | RESP3: UNWATCH |

## Scripting

| Command | HTTP Route | SynapRPC | RESP3 | Notes |
|---------|-----------|----------|-------|-------|
| EVAL | POST /script/eval | ✅ EVAL | ❌ | RESP3: EVAL |
| EVALSHA | POST /script/evalsha | ✅ EVALSHA | ❌ | RESP3: EVALSHA |
| SCRIPT LOAD | POST /script/load | ✅ SCRIPT.LOAD | ❌ | RESP3: SCRIPT LOAD |
| SCRIPT EXISTS | POST /script/exists | ✅ SCRIPT.EXISTS | ❌ | RESP3: SCRIPT EXISTS |
| SCRIPT FLUSH | POST /script/flush | ✅ SCRIPT.FLUSH | ❌ | RESP3: SCRIPT FLUSH |
| SCRIPT KILL | POST /script/kill | ✅ SCRIPT.KILL | ❌ | RESP3: SCRIPT KILL |

## Out of scope (HTTP-only)

These endpoints are explicitly HTTP-only in v0.11.0:

- MCP (`/mcp`)
- UMICP (`/umicp`)
- Auth/Admin (`/auth/*`)
- Cluster management (`/cluster/*`)
- Health check (`/health`)
- Metrics (`/metrics`)
- HiveHub integration (`/hivehub/*`)

## Summary

| Subsystem | HTTP routes | RPC (v0.11.0) | RESP3 today | Remaining (RPC) | Remaining (RESP3) |
|-----------|------------|---------------|-------------|-----------------|-------------------|
| KV | 11 | 18 | 10 | 0 | 9 |
| Key Mgmt | 6 | 0 | 0 | 5 | 5 |
| Hash | 14 | 10 | 8 | 3 | 6 |
| List | 15 | 5 | 5 | 9 | 9 |
| Set | 12 | 5 | 5 | 6 | 6 |
| Sorted Set | 19 | 5 | 5 | 13 | 13 |
| HLL | 4 | 4 | 2 | 0 | 2 |
| Bitmap | 7 | 3 | 3 | 4 | 4 |
| Geo | 8 | 8 | 0 | 0 | 8 |
| Queue | 9 | 9 | 0 | 0 | 9 |
| Stream | 8 | 6 | 0 | 2 | 8 |
| Pub/Sub | 7 | 5 | 0 | 1 | 6 |
| Transactions | 5 | 5 | 0 | 0 | 5 |
| Scripting | 6 | 6 | 0 | 0 | 6 |
| **Total** | **131** | **89** | **38** | **43** | **96** |
