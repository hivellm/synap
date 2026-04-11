# Synap SDKs

Official client libraries for [Synap](https://github.com/hivellm/synap) — High-Performance In-Memory Key-Value Store & Message Broker.

All SDKs communicate via the **StreamableHTTP** protocol (`POST /api/v1/command`), providing a unified interface across languages.

## SDK Overview

| SDK | Language | Package | Status |
|-----|----------|---------|--------|
| [Rust](rust/) | Rust | `synap-sdk` (crates.io) | Production |
| [TypeScript](typescript/) | TS/JS | `@hivehub/synap` (npm) | Production |
| [Python](python/) | Python 3.11+ | `synap-sdk` (PyPI) | Production |
| [PHP](php/) | PHP 8.1+ | `hivehub/synap` (Packagist) | Production |
| [C#](csharp/) | .NET 8+ | `Synap.SDK` (NuGet) | Production |
| [Go](go/) | Go 1.22+ | `github.com/hivellm/synap-sdk-go` | Production |
| [Java](java/) | Java 17+ | `com.hivellm:synap-sdk` (Maven) | Production |

## Module Coverage

Which data structures / features each SDK implements:

| Module | Rust | TypeScript | Python | PHP | C# | Go | Java |
|--------|:----:|:----------:|:------:|:---:|:--:|:--:|:----:|
| **KV Store** | x | x | x | x | x | x | x |
| **Queue** | x | x | x | x | x | x | x |
| **Stream** | x | x | x | x | x | x | x |
| **Pub/Sub** | x | x | x | x | x | x | x |
| **Hash** | x | x | x | x | x | x | x |
| **List** | x | x | x | x | x | x | x |
| **Set** | x | x | x | x | x | x | x |
| **Sorted Set** | x | x | - | - | - | - | - |
| **Bitmap** | x | x | - | x | x | - | - |
| **Geospatial** | x | x | - | x | x | - | - |
| **HyperLogLog** | x | x | - | x | x | - | - |
| **Scripting** | x | x | - | - | - | - | - |
| **Transactions** | x | x | x | x | x | - | - |

## Test Coverage

| SDK | Unit Tests | Mock Tests | S2S/E2E Tests |
|-----|-----------|------------|---------------|
| Rust | 104 (unit) | 150 (mock) | 44 (S2S) + 8 (E2E) |
| TypeScript | 592 | - | - |
| Python | 394 | - | - |
| PHP | 182 | - | - |
| C# | 94 | - | - |
| Go | 55 | httptest | - |
| Java | 36 | JDK HttpServer | - |

## Transport Support

| Transport | URL Scheme | Rust | Others |
|-----------|-----------|:----:|:------:|
| HTTP/REST | `http://host:15500` | x | x |
| SynapRPC (binary) | `synap://host:15501` | x | - |
| RESP3 (Redis) | `resp3://host:6379` | x | - |

> The Rust SDK is the reference implementation with full multi-transport support. Other SDKs currently use HTTP only — binary transports will be added in future releases.

## Quick Start (all SDKs)

Every SDK follows the same pattern:

```
1. Create a Config with the server URL
2. Create a Client from the Config
3. Access modules: client.kv(), client.queue(), client.pubsub(), etc.
4. Close/dispose the client when done
```

### Rust
```rust
let client = SynapClient::new(SynapConfig::new("http://localhost:15500"))?;
client.kv().set("key", "value", None).await?;
let val: Option<String> = client.kv().get("key").await?;
```

### TypeScript
```typescript
const client = new SynapClient({ url: 'http://localhost:15500' });
await client.kv.set('key', 'value');
const val = await client.kv.get('key');
```

### Python
```python
async with SynapClient(SynapConfig("http://localhost:15500")) as client:
    await client.kv.set("key", "value")
    val = await client.kv.get("key")
```

### Go
```go
client := synap.NewClient(synap.NewConfig("http://localhost:15500"))
err := client.KV().Set(ctx, "key", "value", nil)
val, err := client.KV().Get(ctx, "key")
```

### Java
```java
try (var client = new SynapClient(SynapConfig.builder("http://localhost:15500").build())) {
    client.kv().set("key", "value");
    String val = client.kv().get("key");
}
```

### C#
```csharp
using var client = new SynapClient("http://localhost:15500");
await client.KV.SetAsync("key", "value");
var val = await client.KV.GetAsync("key");
```

### PHP
```php
$client = new SynapClient('http://localhost:15500');
$client->kv()->set('key', 'value');
$val = $client->kv()->get('key');
```

## Authentication

All SDKs support Bearer token and Basic auth:

| Method | Config |
|--------|--------|
| Bearer Token | `.withAuth("token")` / `auth_token="token"` |
| Basic Auth | `.withBasicAuth("user", "pass")` / `basic_auth=("user", "pass")` |

## License

All SDKs are licensed under Apache License 2.0 — see each SDK's LICENSE file.
