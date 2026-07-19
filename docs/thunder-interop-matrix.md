# Thunder interop matrix — recorded run

Cross-SDK verification of the Thunder-based SynapRPC transport, run as the gate
for 1.1.0. How to re-run it: [`scripts/interop/README.md`](../scripts/interop/README.md).

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
| `php` | ✅ | ✅ | ✅ | ✅ | hand-written (no Thunder package) |
| `go` | ✅ | ❌ | ✅ | ✅ | hand-written (thunder#9) |
| `java` | — | — | — | — | hand-written; **not run**, no toolchain |
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

### Open

**`go` / SET-GET binary — red.** The Go SDK cannot carry a binary value over
`synap://`. The cause is not the wire: `sendRPC` marshals the payload to JSON to
extract its fields, and Go's `encoding/json` replaces invalid UTF-8 with U+FFFD.
The value is destroyed client-side, before framing:

```
in=deadbeef  json={"value":"ޭ��"}  out=deadefbfbdefbfbd
```

A JSON round-trip in the middle of a binary transport is the real defect, and
fixing it means the module methods must build wire arguments directly rather
than going through `encoding/json`. That is the rewrite `phase20` performs when
Go moves onto Thunder, so it is tracked there rather than patched twice.

The two Go fixes above are independent of it and stand on their own.

**`java` — not run.** The Java cell needs Maven and JDK 17; this machine has
neither (JDK 11 only, and Maven is absent from the winget source). Reading the
transport shows it has no `AUTH` path either, so it is expected to fail the
`auth` column the same way Go and PHP did — but that is a code reading, not a
measurement, and it is recorded as unverified rather than presented as a result.
Fixing it without being able to run it would be worse than leaving it.

## Compatibility

The `legacy` cell replays the wire exactly as the SDKs emitted it before the
Thunder swap — map-shaped request frames and `Bytes` as an array of integers —
and it is green. A pre-1.1.0 client in the wild keeps working against a 1.1.0
server.

Its `pubsub` column asserts a clean *refusal*, not a working subscription: a
legacy client sent `SUBSCRIBE` with the reserved push id, and the server
correctly rejects it with `ERR request id u32::MAX is reserved for server push
frames`. Pub/sub over SynapRPC from an un-upgraded client is a known casualty of
that reserved id, recorded here rather than silently passed.
