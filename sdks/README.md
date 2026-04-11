# Synap SDKs — v0.11.1

Official client libraries for [Synap](https://github.com/hivellm/synap) — High-Performance In-Memory Key-Value Store & Message Broker.

All 7 SDKs support **three transports** with auto-detection from URL scheme. **SynapRPC is the default** — lowest latency, binary MessagePack framing over persistent TCP.

## SDK Overview

| SDK | Language | Package | Version |
|-----|----------|---------|:-------:|
| [Rust](rust/) | Rust | `synap-sdk` (crates.io) | 0.11.1 |
| [TypeScript](typescript/) | TS/JS | `@hivehub/synap` (npm) | 0.11.1 |
| [Python](python/) | Python 3.11+ | `synap-sdk` (PyPI) | 0.11.1 |
| [Go](go/) | Go 1.22+ | `github.com/hivellm/synap/sdks/go` | 0.11.1 |
| [Java](java/) | Java 17+ | `com.hivellm:synap-sdk` (Maven) | 0.11.1 |
| [PHP](php/) | PHP 8.1+ | `hivellm/synap-sdk` (Packagist) | 0.11.1 |
| [C#](csharp/) | .NET 8+ | `Synap.SDK` (NuGet) | 0.11.1 |

## Transport Support

The transport is auto-detected from the URL scheme:

| Transport | URL Scheme | Default | Description |
|-----------|-----------|:-------:|-------------|
| **SynapRPC** | `synap://host:15501` | **yes** | Binary MessagePack over TCP — lowest latency |
| **RESP3** | `resp3://host:6379` | | Redis-compatible wire protocol — use with Redis tooling |
| **HTTP/REST** | `http://host:15500` | fallback | JSON over HTTP — maximum compatibility |

All 7 SDKs support all 3 transports:

| | Rust | TS | Python | Go | Java | PHP | C# |
|---|:---:|:---:|:---:|:---:|:---:|:---:|:---:|
| SynapRPC | x | x | x | x | x | x | x |
| RESP3 | x | x | x | x | x | x | x |
| HTTP | x | x | x | x | x | x | x |

## Module Coverage

| Module | Rust | TS | Python | Go | Java | PHP | C# |
|--------|:----:|:--:|:------:|:--:|:----:|:---:|:--:|
| **KV Store** | x | x | x | x | x | x | x |
| **Queue** | x | x | x | x | x | x | x |
| **Stream** | x | x | x | x | x | x | x |
| **Pub/Sub** | x | x | x | x | x | x | x |
| **Hash** | x | x | x | x | x | x | x |
| **List** | x | x | x | x | x | x | x |
| **Set** | x | x | x | x | x | x | x |
| **Sorted Set** | x | x | - | - | - | - | - |
| **Bitmap** | x | x | - | - | - | x | x |
| **Geospatial** | x | x | - | - | - | x | x |
| **HyperLogLog** | x | x | - | - | - | x | x |
| **Scripting** | x | x | - | - | - | - | - |
| **Transactions** | x | x | x | - | - | x | x |

## Test Coverage

| SDK | Unit/Mock | Integration (real server) | Transports tested |
|-----|:---------:|:------------------------:|:-----------------:|
| Rust | 254 | 42 S2S + 8 E2E | HTTP + RPC + RESP3 |
| Go | 55 | 12 integration | HTTP + RPC + RESP3 |
| TypeScript | 592 | SET/GET/DEL verified | HTTP + RPC + RESP3 |
| Python | 394 | SET/GET/DEL verified | HTTP + RPC + RESP3 |
| PHP | 182 | SET/GET/DEL verified | HTTP + RPC + RESP3 |
| Java | 36 | - (needs JDK 17) | code verified |
| C# | 94 | - | build verified |

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

### Java
```java
try (var client = new SynapClient(SynapConfig.builder("synap://localhost:15501").build())) {
    client.kv().set("key", "value");
    String val = client.kv().get("key");
}
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
| Java | `.authToken("token")` | `.basicAuth("user", "pass")` |
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
