# Spec — Transports & Connection URLs

Version: 0.11.0 | Status: Authoritative

## 1. URL Schemes

The SDK accepts a single connection URL on every client constructor.
The URL scheme determines the active transport; no other field is
required in the recommended API surface.

| Scheme | Transport | Default Port | Wire Format |
|--------|-----------|-------------|------------|
| `http://` | REST over HTTP/1.1 | 15500 | JSON |
| `https://` | REST over TLS | 15500 | JSON |
| `synap://` | SynapRPC over TCP | 15501 | MessagePack |
| `resp3://` | RESP3 over TCP | 6379 | Redis text protocol |

```
// Rust
let client = SynapClient::new("synap://127.0.0.1:15501");
let client = SynapClient::new("resp3://127.0.0.1:6379");
let client = SynapClient::new("http://127.0.0.1:15500");

// TypeScript
const synap = new Synap("synap://127.0.0.1:15501");

// Python
client = SynapClient("resp3://127.0.0.1:6379")
```

Given a URL with an unrecognised scheme, the constructor MUST raise a
typed configuration error (`InvalidSchemeError` or equivalent) before
any network I/O occurs.

## 2. No Silent HTTP Fallback

When the active transport is `synap://` or `resp3://`, the SDK MUST NOT
transparently fall back to HTTP for unimplemented commands. If a
command is not implemented on the active transport the SDK MUST raise
`UnsupportedCommandError` (language-appropriate type) carrying:

- `command` — the command name that was invoked
- `transport` — the active transport mode (`synap` | `resp3`)

MCP and UMICP endpoints remain HTTP-only and are explicitly exempt from
this rule.

## 3. Command Parity Contract

For every command listed in `command-matrix.md` the server SHALL
implement the same semantics on all three transports. "Same semantics"
means: given equivalent input, all three produce equivalent output
(byte-for-byte where the wire format allows, semantically equivalent
otherwise).

A command "exists on HTTP" implies it MUST exist on SynapRPC and RESP3
before version 0.11.0 ships.

## 4. SynapRPC Wire Format

### 4.1 Regular Request/Response (existing)

All frames are length-prefixed MessagePack over a persistent TCP
connection. Requests and responses may be interleaved on the same
connection (multiplexed by `id`).

```
// Request frame (rmp-serde encoded)
Request {
    id:      u32,      // client-chosen, monotonically increasing
    command: String,   // uppercase command name, e.g. "SET"
    args:    Vec<SynapValue>,
}

// Response frame
Response {
    id:     u32,
    result: Result<SynapValue, String>,
}
```

`SynapValue` variants:
- `Null` — Redis nil / SQL NULL
- `Bool(bool)`
- `Int(i64)`
- `Float(f64)`
- `Bytes(Vec<u8>)` — binary-safe, no base64
- `Str(String)` — UTF-8 text
- `Array(Vec<SynapValue>)` — heterogeneous list
- `Map(Vec<(SynapValue, SynapValue)>)` — ordered key-value pairs

### 4.2 Server-Push Frame (streaming — new in 0.11.0)

Server-push frames are used for reactive subscriptions (pub/sub,
stream observation, reactive queue consumer). The server sends these
frames unprompted after a SUBSCRIBE/OBSERVE command.

```
// Push frame — same MessagePack envelope, id = 0 for pushes
PushFrame {
    id:      u32,    // ALWAYS 0 for server-initiated pushes
    command: String, // push type: "EVENT", "MESSAGE", "DELIVERY"
    args:    Vec<SynapValue>,
}
```

Push type payload conventions:

| push `command` | `args[0]` | `args[1]` | `args[2]` |
|---|---|---|---|
| `EVENT` | topic/room (Str) | event id (Str) | payload (Bytes\|Str\|Map) |
| `MESSAGE` | queue name (Str) | delivery tag (Str) | payload (Bytes) |
| `DELIVERY` | subscriber id (Str) | sequence (Int) | data (Bytes) |

The client library uses `id == 0` as the discriminant to route push
frames to the appropriate subscriber callback rather than to the
pending-request map.

### 4.3 Subscription Commands

`SUBSCRIBE <topic>` — subscribes and enables push frames for that topic.
`UNSUBSCRIBE <topic>` — stops push frames for that topic.
`OBSERVE <room>` — subscribes to stream room events.
`QCONSUME <queue>` — starts reactive queue consumption.

The server responds with a normal `Response` (`id` echoed, `result`
`Ok(Null)`) to confirm the subscription, then sends push frames as
events arrive.

## 5. RESP3 Streaming

On RESP3, streaming uses native Redis semantics:

- Pub/Sub: `SUBSCRIBE channel [channel ...]` / `PSUBSCRIBE pattern`
  → server sends `["message", channel, payload]` push frames
- Streams: `XREAD BLOCK 0 STREAMS room $` → blocking read
- Queue: `QCONSUME queue` → streaming list-pop equivalent

## 6. Deprecated Builder Methods

The previous multi-field builder APIs (`with_synap_rpc_transport`,
`with_rpc_addr`, `WithSynapRpcTransport`, etc.) remain available in
0.11.0 but MUST be marked `#[deprecated]` (Rust), `@deprecated` (TS/Python),
`[Obsolete]` (C#), or `@deprecated` (PHP) and redirect to the
URL-scheme path internally. They will be removed in 0.12.0.

## 7. Version

This specification ships with Synap `0.11.0`. The version is bumped
across the workspace and all five SDKs simultaneously.
