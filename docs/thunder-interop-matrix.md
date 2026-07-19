# Thunder interop matrix — recorded run

Cross-SDK verification of the Thunder-based SynapRPC transport, run as the gate
for 1.2.0. How to re-run it: [`scripts/interop/README.md`](../scripts/interop/README.md).

Run twice against the same code: once with a locally built
`target/release/synap-server`, and once against the **release Docker image**
(`synap:interop`, 18.5 MB, reporting `healthy`) with its ports published. Both
runs produced the table below — the container is not a separate result, it is
the same result reached through a published port.

Server: `thunder-rpc 0.2.0`, authentication required.

| SDK | authenticate | SET/GET binary | SUBSCRIBE/PUBLISH | error round-trip | Transport |
|---|---|---|---|---|---|
| `rust` | ✅ | ✅ | ✅ | ✅ | `thunder-rpc` |
| `typescript` | ✅ | ✅ | ✅ | ✅ | `@hivehub/thunder` |
| `python` | ✅ | ✅ | ✅ | ✅ | `hivellm-thunder` |
| `csharp` | ✅ | ✅ | ✅ | ✅ | `HiveLLM.Thunder` |
| `php` | ✅ | ✅ | ✅ | ✅ | `hivellm/thunder` |
| `go` | ✅ | ✅ | ✅ | ✅ | `thunder-go` |
| `legacy` | ✅ | ✅ | ✅ | ✅ | pre-Thunder wire replay |

The `php` row was run directly rather than through the driver: a
winget-installed PHP resolves to a Windows App Execution Alias that cannot be
spawned from a subprocess. Same client, same server, same four steps.

## What the matrix found

Every red cell below was a real defect. None of them was introduced by the
Thunder swap — the matrix is simply the first thing that looked.

### Fixed

| SDK | Defect | Fix |
|---|---|---|
| rust | `set(k, vec![…])` then `get::<Vec<u8>>(k)` failed on a value the SDK itself wrote. The transport JSON-encodes a structured value into a string on the way out and nothing re-parsed it on the way back. | `KVStore::get` retries as JSON only after the direct decode has already failed, so a value that genuinely is a string is untouched. |
| typescript | Binary values came back corrupted and unrecoverable: `Bytes` were decoded as UTF-8 unconditionally, so every invalid sequence became U+FFFD and `deadbeef` read back as `adfdfd`. | Decode UTF-8 only when the bytes are valid UTF-8; otherwise return a `Buffer`. |
| go | Could not reach a `require_auth` server at all — the RPC transport never sent `AUTH`. | Credentials attached to the transport and sent as `AUTH` on every connect, including reconnects. |
| go | A non-UTF-8 Go string was packed as MessagePack `str`, which the server decoded lossily. | Strings that are not valid UTF-8 travel as `bin`. |
| php | Could not reach a `require_auth` server at all — same missing `AUTH`. | `AUTH` sent on connect, on the command socket and the dedicated push socket. |
| php | Pub/sub over SynapRPC could not work: `SUBSCRIBE` was sent with `id = 0xFFFFFFFF`, the reserved push sentinel, which the server refuses outright. | Ordinary request id. The sentinel identifies frames coming *from* the server. |
| php | Non-UTF-8 strings were packed as `str`, and the server closed the connection. | Same `bin` treatment as Go. |

The C# SDK had the identical `AUTH` and reserved-sentinel defects; both were
fixed when it moved onto Thunder, before this matrix ran.

### Found by running the image, not the matrix

Two defects only a container run could surface, both release blockers:

- **The image did not build.** The `Dockerfile` still copied
  `crates/synap-protocol/`, dissolved earlier in this release, so every build
  failed at that `COPY`. No CI job builds the image, so nothing caught it.
- **An authenticated deployment reported itself `unhealthy`.** `/health` and
  `/metrics` are declared "always public" at their route definitions, but the
  auth middleware is a layer over the whole router, so under `require_auth`
  they answered `401`. The container's HEALTHCHECK probes `/health`
  unauthenticated, so the container stayed `unhealthy` forever and an
  orchestrator would restart a server that was serving perfectly.

Both are fixed, with regression tests in `auth_edge_cases_tests.rs`. The
standing lesson matches the matrix's: a CI job should build and run the image,
for the same reason the matrix exists.

### Closed since

**`go` / SET-GET binary — now green.** The Go SDK moved onto Thunder
(`phase20`), and with it the JSON round trip that sat in the middle of a binary
transport was removed in both directions. `encoding/json` replaces invalid UTF-8
with U+FFFD, so the value was destroyed inside the client, before framing:

```
in=deadbeef  json={"value":"ޭ��"}  out=deadefbfbdefbfbd
```

Command payloads are now read by reflection instead of being marshalled just to
reach their fields by name, and replies reach the module methods as typed values
instead of being re-encoded. HTTP and RESP3, which genuinely speak JSON, are
untouched. Shipped in the Go SDK's v1.1.1.

Every cell is green. There is nothing open.

## Compatibility

The `legacy` cell replays the wire exactly as the SDKs emitted it before the
Thunder swap — map-shaped request frames and `Bytes` as an array of integers —
and it is green. A pre-1.2.0 client in the wild keeps working against a 1.2.0
server.

Its `pubsub` column asserts a clean *refusal*, not a working subscription: a
legacy client sent `SUBSCRIBE` with the reserved push id, and the server
correctly rejects it with `ERR request id u32::MAX is reserved for server push
frames`. Pub/sub over SynapRPC from an un-upgraded client is a known casualty of
that reserved id, recorded here rather than silently passed.
