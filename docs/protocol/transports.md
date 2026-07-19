# Transport Reference — Synap v0.11.0

This document describes the three wire transports supported by the Synap server and all five official SDKs (Rust, TypeScript, Python, PHP, C#), including the URL-scheme API, the full command-parity matrix, and the SynapRPC server-push frame reference.

---

## 1. URL Schemes

Every SDK client constructor accepts a single connection URL. The scheme determines the active transport; no extra builder options are required.

| URL scheme | Transport | Default port | Wire format |
|------------|-----------|-------------|-------------|
| `synap://` | SynapRPC over TCP | `15501` | MessagePack (binary) |
| `resp3://` | RESP3 over TCP | `6379` | Redis text protocol |
| `http://` | REST over HTTP/1.1 | `15500` | JSON |
| `https://` | REST over TLS | `15500` | JSON |

```
synap://127.0.0.1:15501   → SynapRPC
resp3://127.0.0.1:6379    → RESP3
http://127.0.0.1:15500    → HTTP/REST
```

An unrecognised scheme causes the constructor to raise a typed configuration error before any network I/O.

### Quick examples by language

```rust
// Rust
let client = SynapClient::new(SynapConfig::new("synap://127.0.0.1:15501"))?;
```

```typescript
// TypeScript
const client = new SynapClient("synap://127.0.0.1:15501");
```

```python
# Python
client = SynapClient(SynapConfig("synap://127.0.0.1:15501"))
```

```php
// PHP
$client = new SynapClient(new SynapConfig("synap://127.0.0.1:15501"));
```

```csharp
// C#
var client = new SynapClient(SynapConfig.Create("synap://127.0.0.1:15501"));
```

---

## 2. No Silent HTTP Fallback

When the active transport is `synap://` or `resp3://`, the SDK **never** transparently falls back to HTTP for unmapped commands. Instead it raises a typed error:

| Language | Error type |
|----------|-----------|
| Rust | `SynapError::UnsupportedCommand { command, transport }` |
| TypeScript | `UnsupportedCommandError` |
| Python | `UnsupportedCommandError` |
| PHP | `UnsupportedCommandException` |
| C# | `UnsupportedCommandException` |

HTTP-only endpoints (MCP, auth/admin, cluster management, health, metrics) are explicitly exempt.

---

## 3. SynapRPC Wire Format

### 3.1 Regular request/response

All frames are 4-byte LE length-prefixed MessagePack over a persistent TCP connection. Requests and responses may be interleaved on the same connection (multiplexed by `id`).

```
// Request frame
[id: u32, command: String, args: Vec<SynapValue>]

// Response frame
[id: u32, result: {"Ok": SynapValue} | {"Err": String}]
```

`SynapValue` variants:

| Variant | Description |
|---------|-------------|
| `"Null"` | Redis nil / SQL NULL |
| `{"Bool": bool}` | Boolean |
| `{"Int": i64}` | 64-bit signed integer |
| `{"Float": f64}` | 64-bit float |
| `{"Str": String}` | UTF-8 text |
| `{"Bytes": bytes}` | Binary-safe byte array (no base64) |
| `{"Array": [...]}` | Heterogeneous list |
| `{"Map": [[k, v], ...]}` | Ordered key-value pairs |

### 3.2 Server-push frames

Server-push frames are used for reactive subscriptions (pub/sub, stream observation, reactive queue consumption). The server sends these unprompted after a `SUBSCRIBE` command on a dedicated TCP connection.

**Push frame sentinel:** `id == 0xFFFFFFFF` (`u32::MAX`)

The push connection is a separate TCP socket opened by the SDK exclusively for receiving server-initiated frames. The regular request/response connection is not shared.

```
// Push frame — same MessagePack envelope but id = 0xFFFFFFFF
[id: u32 = 0xFFFFFFFF, command: String, args: Vec<SynapValue>]
```

Push `command` payload conventions:

| `command` | `args[0]` | `args[1]` | `args[2]` |
|-----------|-----------|-----------|-----------|
| `"EVENT"` | topic / room (Str) | event id (Str) | payload (Bytes \| Str \| Map) |
| `"MESSAGE"` | queue name (Str) | delivery tag (Str) | payload (Bytes) |
| `"DELIVERY"` | subscriber id (Str) | sequence (Int) | data (Bytes) |

The client library uses `id == 0xFFFFFFFF` as the discriminant to route push frames to the registered subscriber callback.

### 3.3 Subscription handshake

1. Client opens a **dedicated** TCP socket to the SynapRPC port.
2. Client sends a `SUBSCRIBE` frame with `id = 0xFFFFFFFF` and `args = [topic, ...]`.
3. Server responds with a normal response frame (echoing `id = 0xFFFFFFFF`, `result = Ok(Null)`) to confirm the subscription.
4. Server sends push frames as events arrive.

```
// Step 2: client → server
[0xFFFFFFFF, "SUBSCRIBE", [{"Str": "news.*"}, {"Str": "alerts"}]]

// Step 3: server → client (ack)
[0xFFFFFFFF, {"Ok": "Null"}]

// Step 4: server → client (push)
[0xFFFFFFFF, "EVENT", [{"Str": "news.*"}, {"Str": "id-001"}, {"Str": "breaking!"}]]
```

---

## 4. RESP3 Wire Format

RESP3 uses the standard Redis text protocol on port 6379. The SDK sends a `HELLO 3` handshake on connect, then issues commands as RESP arrays.

Streaming on RESP3 uses native Redis semantics:
- **Pub/Sub:** `SUBSCRIBE channel [...]` / `PSUBSCRIBE pattern` → server sends `["message", channel, payload]` push frames.
- **Streams:** `XREAD BLOCK 0 STREAMS room $` → blocking read.
- **Queue:** `QCONSUME queue` → streaming list-pop equivalent.

---

## 5. Command Parity Matrix

Legend: ✅ implemented · ❌ not yet · N/A not applicable

### KV Store

| Command | HTTP | SynapRPC | RESP3 |
|---------|------|----------|-------|
| SET | ✅ | ✅ SET | ✅ SET |
| GET | ✅ | ✅ GET | ✅ GET |
| DEL | ✅ | ✅ DEL | ✅ DEL |
| EXISTS | ✅ | ✅ EXISTS | ✅ EXISTS |
| EXPIRE | ✅ | ✅ EXPIRE | ✅ EXPIRE |
| TTL | ✅ | ✅ TTL | ✅ TTL |
| PERSIST | ✅ | ✅ PERSIST | ✅ PERSIST |
| INCR/INCRBY | ✅ | ✅ INCR/INCRBY | ✅ |
| DECR/DECRBY | ✅ | ✅ DECR/DECRBY | ✅ |
| MSET/MGET | ✅ | ✅ MSET/MGET | ✅ |
| KEYS | ✅ | ✅ KEYS | ✅ KEYS |
| SCAN | ✅ | ✅ SCAN | ✅ SCAN |
| APPEND | ✅ | ✅ APPEND | ❌ |
| GETRANGE/SETRANGE | ✅ | ✅ | ❌ |
| STRLEN | ✅ | ✅ STRLEN | ❌ |
| GETSET | ✅ | ✅ GETSET | ❌ |
| MSETNX | ✅ | ✅ MSETNX | ❌ |
| DBSIZE | ✅ | ✅ DBSIZE | ❌ |
| FLUSHDB/FLUSHALL | ✅ | ✅ | ✅ |
| KV.STATS | ✅ | ✅ KVSTATS | ❌ |

### Hash

| Command | HTTP | SynapRPC | RESP3 |
|---------|------|----------|-------|
| HSET/HGET/HDEL | ✅ | ✅ | ✅ |
| HGETALL/HLEN/HEXISTS | ✅ | ✅ | ✅ |
| HMSET/HMGET | ✅ | ✅ | ✅ |
| HKEYS | ✅ | ✅ HKEYS | ❌ |
| HVALS | ✅ | ✅ HVALS | ❌ |
| HINCRBY/HINCRBYFLOAT | ✅ | ❌ | ❌ |
| HSETNX | ✅ | ❌ | ❌ |

### List

| Command | HTTP | SynapRPC | RESP3 |
|---------|------|----------|-------|
| LPUSH/RPUSH | ✅ | ✅ | ✅ |
| LPOP/RPOP | ✅ | ✅ | ✅ |
| LRANGE/LLEN | ✅ | ✅ | ✅ |
| LINDEX/LSET/LTRIM/LREM | ✅ | ❌ | ❌ |
| LINSERT/LPUSHX/RPUSHX | ✅ | ❌ | ❌ |

### Set

| Command | HTTP | SynapRPC | RESP3 |
|---------|------|----------|-------|
| SADD/SREM/SISMEMBER | ✅ | ✅ | ✅ |
| SMEMBERS/SCARD | ✅ | ✅ | ✅ |
| SPOP/SRANDMEMBER/SMOVE | ✅ | ❌ | ❌ |
| SINTER/SUNION/SDIFF | ✅ | ❌ | ❌ |

### Sorted Set

| Command | HTTP | SynapRPC | RESP3 |
|---------|------|----------|-------|
| ZADD/ZREM/ZSCORE | ✅ | ✅ | ✅ |
| ZCARD/ZRANGE | ✅ | ✅ | ✅ |
| ZRANK/ZREVRANK/ZREVRANGE | ✅ | ❌ | ❌ |
| ZCOUNT/ZRANGEBYSCORE | ✅ | ❌ | ❌ |
| ZPOPMIN/ZPOPMAX | ✅ | ❌ | ❌ |

### HyperLogLog

| Command | HTTP | SynapRPC | RESP3 |
|---------|------|----------|-------|
| PFADD | ✅ | ✅ PFADD | ✅ PFADD |
| PFCOUNT | ✅ | ✅ PFCOUNT | ✅ PFCOUNT |
| PFMERGE | ✅ | ✅ PFMERGE | ❌ |
| HLL.STATS | ✅ | ✅ HLLSTATS | ❌ |

### Geospatial

| Command | HTTP | SynapRPC | RESP3 |
|---------|------|----------|-------|
| GEOADD | ✅ | ✅ GEOADD | ❌ |
| GEOPOS | ✅ | ✅ GEOPOS | ❌ |
| GEODIST | ✅ | ✅ GEODIST | ❌ |
| GEOHASH | ✅ | ✅ GEOHASH | ❌ |
| GEORADIUS | ✅ | ✅ GEORADIUS | ❌ |
| GEORADIUSBYMEMBER | ✅ | ✅ GEORADIUSBYMEMBER | ❌ |
| GEOSEARCH | ✅ | ✅ GEOSEARCH | ❌ |
| GEO.STATS | ✅ | ✅ GEOSTATS | ❌ |

### Queue

| Command | HTTP | SynapRPC | RESP3 |
|---------|------|----------|-------|
| queue.create | ✅ | ✅ QCREATE | ❌ |
| queue.delete | ✅ | ✅ QDELETE | ❌ |
| queue.list | ✅ | ✅ QLIST | ❌ |
| queue.publish | ✅ | ✅ QPUBLISH | ❌ |
| queue.consume | ✅ | ✅ QCONSUME | ❌ |
| queue.ack | ✅ | ✅ QACK | ❌ |
| queue.nack | ✅ | ✅ QNACK | ❌ |
| queue.stats | ✅ | ✅ QSTATS | ❌ |
| queue.purge | ✅ | ✅ QPURGE | ❌ |

### Stream

| Command | HTTP | SynapRPC | RESP3 |
|---------|------|----------|-------|
| stream.create_room | ✅ | ✅ SCREATE | ❌ |
| stream.publish | ✅ | ✅ SPUBLISH | ❌ |
| stream.read | ✅ | ✅ SREAD | ❌ |
| stream.delete_room | ✅ | ✅ SDELETE | ❌ |
| stream.list_rooms | ✅ | ✅ SLIST | ❌ |
| stream.stats | ✅ | ✅ SSTATS | ❌ |
| stream.replay | ✅ | ❌ | ❌ |

### Pub/Sub

| Command | HTTP | SynapRPC | RESP3 |
|---------|------|----------|-------|
| pubsub.publish | ✅ | ✅ PUBLISH | ❌ |
| pubsub.subscribe | ✅ (WS) | ✅ SUBSCRIBE + push | ❌ |
| pubsub.unsubscribe | ✅ | ✅ UNSUBSCRIBE | ❌ |
| pubsub.topics | ✅ | ✅ TOPICS | ❌ |
| pubsub.stats | ✅ | ✅ PSSTATS | ❌ |

### Transactions

| Command | HTTP | SynapRPC | RESP3 |
|---------|------|----------|-------|
| transaction.multi | ✅ | ✅ MULTI | ❌ |
| transaction.exec | ✅ | ✅ EXEC | ❌ |
| transaction.discard | ✅ | ✅ DISCARD | ❌ |
| transaction.watch | ✅ | ✅ WATCH | ❌ |
| transaction.unwatch | ✅ | ✅ UNWATCH | ❌ |

> **Note:** RPC transaction commands carry `client_id` as the first argument.

### Scripting

| Command | HTTP | SynapRPC | RESP3 |
|---------|------|----------|-------|
| script.eval | ✅ | ✅ EVAL | ❌ |
| script.evalsha | ✅ | ✅ EVALSHA | ❌ |
| script.load | ✅ | ✅ SCRIPT.LOAD | ❌ |
| script.exists | ✅ | ✅ SCRIPT.EXISTS | ❌ |
| script.flush | ✅ | ✅ SCRIPT.FLUSH | ❌ |
| script.kill | ✅ | ✅ SCRIPT.KILL | ❌ |

---

## 6. Deprecated Builder Methods

The previous multi-field builder APIs (e.g. `with_synap_rpc_transport`, `WithSynapRpcTransport`) remain available in v0.11.0 but are deprecated:

| Language | Deprecation mechanism |
|----------|-----------------------|
| Rust | `#[deprecated(since = "0.11.0")]` |
| TypeScript | `@deprecated` JSDoc |
| Python | `warnings.warn(DeprecationWarning)` |
| PHP | `trigger_error(E_USER_DEPRECATED)` |
| C# | `[Obsolete]` |

They will be removed in v0.12.0. Migrate to URL-scheme construction shown in §1.

---

## 7. HTTP-only Endpoints

The following are explicitly HTTP-only and exempt from the `UnsupportedCommand` rule:

- MCP (`/mcp`)
- UMICP (`/umicp`)
- Auth / Admin (`/auth/*`)
- Cluster management (`/cluster/*`)
- Health check (`/health`)
- Metrics (`/metrics`)
- HiveHub integration (`/hivehub/*`)

---

## 8. Summary Counts (v0.11.0)

| Subsystem | HTTP | SynapRPC | RESP3 |
|-----------|------|----------|-------|
| KV | ✅ 23 | ✅ 18 | ✅ 10 |
| Hash | ✅ 12 | ✅ 10 | ✅ 8 |
| List | ✅ 15 | ✅ 5 | ✅ 5 |
| Set | ✅ 11 | ✅ 5 | ✅ 5 |
| Sorted Set | ✅ 19 | ✅ 5 | ✅ 5 |
| HyperLogLog | ✅ 4 | ✅ 4 | ✅ 2 |
| Bitmap | ✅ 6 | ✅ 3 | ✅ 3 |
| Geo | ✅ 8 | ✅ 8 | ❌ 0 |
| Queue | ✅ 9 | ✅ 9 | ❌ 0 |
| Stream | ✅ 8 | ✅ 6 | ❌ 0 |
| Pub/Sub | ✅ 6 | ✅ 5 | ❌ 0 |
| Transactions | ✅ 5 | ✅ 5 | ❌ 0 |
| Scripting | ✅ 6 | ✅ 6 | ❌ 0 |
