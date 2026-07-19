# Synap SDKs — v1.2.0

Official client libraries for [Synap](https://github.com/hivellm/synap) — High-Performance In-Memory Key-Value Store & Message Broker.

All 6 SDKs support **three transports** with auto-detection from URL scheme. **SynapRPC is the default** — lowest latency, binary MessagePack framing over persistent TCP, and since 1.2.0 the same [Thunder](https://github.com/hivellm/thunder) protocol implementation the server runs.

The Go and PHP SDKs live in their own repositories — [`hivellm/synap-sdk-go`](https://github.com/hivellm/synap-sdk-go) and [`hivellm/synap-sdk-php`](https://github.com/hivellm/synap-sdk-php) — and are consumed here as submodules.

## SDK Overview

| SDK | Language | Package | Version |
|-----|----------|---------|:-------:|
| [Rust](rust/) | Rust | `synap-sdk` (crates.io) | 1.2.0 |
| [TypeScript](typescript/) | TS/JS | `@hivehub/synap` (npm) | 1.2.0 |
| [Python](python/) | Python 3.11+ | `synap-sdk` (PyPI) | 1.2.0 |
| [Go](go/) | Go 1.25+ | `github.com/hivellm/synap-sdk-go` | 1.2.0 |
| [PHP](php/) | PHP 8.2+ | `hivellm/synap-sdk` (Packagist) | 1.2.1 |
| [C#](csharp/) | .NET 8+ | `Synap.SDK` (NuGet) | 1.2.0 |

## Transport Support

The transport is auto-detected from the URL scheme:

| Transport | URL Scheme | Default | Description |
|-----------|-----------|:-------:|-------------|
| **SynapRPC** | `synap://host:15501` | **yes** | Binary MessagePack over TCP — lowest latency |
| **RESP3** | `resp3://host:6379` | | Redis-compatible wire protocol — use with Redis tooling |
| **HTTP/REST** | `http://host:15500` | fallback | JSON over HTTP — maximum compatibility |

All 6 SDKs support all 3 transports:

| | Rust | TS | Python | Go | PHP | C# |
|---|:---:|:---:|:---:|:---:|:---:|:---:|
| SynapRPC | x | x | x | x | x | x |
| RESP3 | x | x | x | x | x | x |
| HTTP | x | x | x | x | x | x |

## Module Coverage

| Module | Rust | TS | Python | Go | PHP | C# |
|--------|:----:|:--:|:------:|:--:|:---:|:--:|
| **KV Store** | x | x | x | x | x | x |
| **Queue** | x | x | x | x | x | x |
| **Stream** | x | x | x | x | x | x |
| **Pub/Sub** | x | x | x | x | x | x |
| **Hash** | x | x | x | x | x | x |
| **List** | x | x | x | x | x | x |
| **Set** | x | x | x | x | x | x |
| **KV Watch** | x | x | x | x | x | x |
| **Sorted Set** | x | x | - | - | - | - |
| **Bitmap** | x | x | x | - | x | x |
| **Geospatial** | x | x | x | - | x | x |
| **HyperLogLog** | x | x | x | - | x | x |
| **Scripting** | x | x | - | - | - | - |
| **Transactions** | x | x | x | - | x | x |

## Test Coverage

Totals from the 1.0.0 release audit (suites run against the published
`hivehub/synap:1.0.0` image):

| SDK | Unit/Mock | Integration (real server) | Transports tested |
|-----|:---------:|:------------------------:|:-----------------:|
| Rust | 276 | 44 S2S | HTTP + RPC + RESP3 |
| Go | 55 | 6 integration | HTTP + RPC + RESP3 |
| TypeScript | 467 | SET/GET/DEL verified | HTTP + RPC + RESP3 |
| Python | 177 | SET/GET/DEL verified | HTTP + RPC + RESP3 |
| PHP | 182 | SET/GET/DEL verified | HTTP + RPC + RESP3 |
| C# | 96 | - | build verified |

## Quick Start

Every SDK follows the same pattern — create a config with a URL, create a client, use modules:

### Rust
```rust
let client = SynapClient::new(SynapConfig::new("synap://localhost:15501"))?;
client.kv().set("key", "value", None).await?;
let val: Option<String> = client.kv().get("key").await?;
```

### Go
```go
client := synap.NewClient(synap.NewConfig("synap://localhost:15501"))
_ = client.KV().Set(ctx, "key", "value", 0)
val, _ := client.KV().Get(ctx, "key")
```

### TypeScript
```typescript
const client = new SynapClient({ url: 'synap://localhost:15501' });
await client.kv.set('key', 'value');
const val = await client.kv.get('key');
```

### Python
```python
async with SynapClient(SynapConfig("synap://localhost:15501")) as client:
    await client.kv.set("key", "value")
    val = await client.kv.get("key")
```

### PHP
```php
$client = new SynapClient('synap://localhost:15501');
$client->kv()->set('key', 'value');
$val = $client->kv()->get('key');
```

### C#
```csharp
using var client = new SynapClient("synap://localhost:15501");
await client.KV.SetAsync("key", "value");
var val = await client.KV.GetAsync("key");
```

## KV Watch

Every SDK exposes `kv.watch(pattern)`: a stream of value-carrying change
envelopes (`{ key, event, version, value? }`) over a dedicated `KV.WATCH` push
connection. Requires the `synap://` transport — HTTP clients use the `/kv/ws`
WebSocket endpoint directly (the Rust SDK falls back to it automatically).
`mode: notify` strips the value server-side, per subscription. Closing the
stream issues `KV.UNWATCH`. Semantics and fan-out cost: [`docs/features/kv-watch.md`](../docs/features/kv-watch.md).

```rust
let (mut events, handle) = client.kv().watch("user:*");
while let Some(e) = events.next().await { println!("{} v{} = {:?}", e.key, e.version, e.value); }
handle.unsubscribe();
```

```typescript
const sub = client.kv.watch<Profile>('user:*').subscribe(e => console.log(e.key, e.version, e.value));
sub.unsubscribe();
```

```python
async for e in client.kv.watch("user:*"):
    print(e.key, e.version, e.value)
```

```go
events, stop, _ := client.KV().Watch(ctx, "user:*")
for e := range events { fmt.Println(e.Key, e.Version, e.Value) }
stop()
```

```php
$client->kv()->watch('user:*', fn ($e) => printf("%s v%d\n", $e->key, $e->version));
```

```csharp
await foreach (var e in client.KV.WatchAsync("user:*", ct: cts.Token))
    Console.WriteLine($"{e.Key} v{e.Version} = {e.Value}");
```

## Authentication

All SDKs support Bearer token and Basic auth:

```
synap://localhost:15501          # no auth
synap://localhost:15501          # + .withAuth("api-key-token")
synap://localhost:15501          # + .withBasicAuth("user", "pass")
```

| Language | Bearer Token | Basic Auth |
|----------|-------------|------------|
| Rust | `.with_auth_token("token")` | `.with_basic_auth("user", "pass")` |
| Go | `.WithAuth("token")` | `.WithBasicAuth("user", "pass")` |
| TS | `{ authToken: "token" }` | `{ username, password }` |
| Python | `SynapConfig(auth_token="token")` | `SynapConfig(username="u", password="p")` |
| PHP | `new SynapConfig(authToken: "token")` | `new SynapConfig(username: "u", password: "p")` |
| C# | `new SynapClient(url, authToken: "token")` | `new SynapClient(url, user: "u", pass: "p")` |

## Wire Protocol Reference

### SynapRPC (MessagePack/TCP)

```
Frame: [4-byte LE length] [msgpack body]

Request body (msgpack MAP):
  {"id": uint32, "command": "SET", "args": [{"Str": "key"}, {"Str": "value"}]}

Response body (msgpack ARRAY):
  [id, {"Ok": {"Str": "OK"}}]      — success
  [id, {"Err": "error message"}]    — error

SynapValue (serde externally-tagged):
  null   → "Null"         (bare string)
  string → {"Str": "x"}   (single-key map)
  int    → {"Int": 42}
  float  → {"Float": 1.5}
  bool   → {"Bool": true}
  bytes  → {"Bytes": [1,2,3]}
```

### RESP3 (Redis-compatible)

```
Send:  *3\r\n$3\r\nSET\r\n$3\r\nkey\r\n$5\r\nvalue\r\n
Recv:  +OK\r\n
```

Standard Redis wire protocol. Any Redis client library can connect.

## License

All SDKs are licensed under Apache License 2.0 — see each SDK's [LICENSE](../LICENSE) file.
