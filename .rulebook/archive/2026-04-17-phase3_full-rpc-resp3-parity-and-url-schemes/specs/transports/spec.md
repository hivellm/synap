# Spec — Transports & Connection URLs

## 1. URL schemes

The SDK SHALL accept a single connection URL on every client constructor.
The URL scheme determines the active transport and there SHALL be no
other way to choose a transport in the recommended API surface:

- `http://host:port` / `https://host:port` — REST transport
- `synap://host:port` — SynapRPC binary transport (MessagePack over TCP)
- `resp3://host:port` — RESP3 Redis-compatible transport

Given a URL with a scheme the SDK does not recognise, the constructor
MUST raise a typed configuration error before any network I/O occurs.

## 2. No silent HTTP fallback

When the active transport is `synap://` or `resp3://`, the SDK MUST NOT
transparently fall back to HTTP for unknown commands. If a command is
not implemented on the active transport the SDK MUST raise
`UnsupportedCommandError` (or the language equivalent) carrying the
command name and the transport mode in its message.

## 3. Command parity

For every command listed in `command-matrix.md` the server SHALL
implement the same behaviour on all three transports. "Same behaviour"
means: given equivalent input, all three produce equivalent output
(byte-for-byte where the wire format allows, semantically equivalent
otherwise).

Given a command exists on the HTTP router, When the same command is
issued over SynapRPC or RESP3, Then the server SHALL dispatch to the
identical core handler and return the same result envelope translated
into the target wire format.

## 4. Deprecation of legacy builders

The previous multi-field builder methods (`with_synap_rpc_transport`,
`with_rpc_addr`, `WithSynapRpcTransport`, etc.) SHALL remain available
for one release but MUST be marked deprecated and redirect to the
URL-scheme path internally.

## 5. Streaming frames

Reactive subscriptions on SynapRPC (pub/sub, stream observe, reactive
queue consumer) SHALL use a server-push frame variant of the SynapRPC
envelope. The frame layout is defined in
`../command-matrix.md#server-push-frames`.

On RESP3 the same subscriptions SHALL use the native Redis
`SUBSCRIBE` / `PSUBSCRIBE` / `XREAD BLOCK` semantics.

## 6. Version bump

This change SHALL ship as `0.11.0` across the workspace and all SDKs
(Rust, TypeScript, Python, PHP, C#).
