## 1. Unblock

- [x] 1.1 Confirmed: `github.com/hivellm/thunder-go` v0.2.0 and v0.2.1 resolve through the Go module proxy (hivellm/thunder#9 closed in 0.2.1)
- [x] 1.2 Added at v0.2.1, matching the server and the Rust, TypeScript, Python and C# SDKs. Note it raises the module's minimum Go from 1.22 to 1.25

## 2. Transport swap

- [x] 2.1 Rewritten over `thunder.Client`, with `synapConfig()` mirroring the server's `synap_config()`
- [x] 2.2 Credentials flow through `ClientConfig.Credentials`; verified against a `require_auth` server — the `auth` cell of the interop matrix went from red to green
- [x] 2.3 Added `PubSubManager.Observe`, streaming push frames off Thunder's hook. The SDK had no push support of any kind before, so this is new capability rather than a port
- [x] 2.4 Deleted: framing, `net.Conn` handling, the reconnect loop, the request-id counter and the hand-written externally-tagged codec
- [x] 2.5 Dual `bin` / int-array tolerance is now Thunder's, and the legacy cell of the interop matrix still passes against this SDK's server

## 3. Tail (docs + tests — check or waive with tailWaiver)

- [x] 3.1 README and CHANGELOG rewritten for the Thunder transport, including the module-path break, the Go 1.25 floor and the remaining binary-value limitation
- [x] 3.2 `transport_rpc_test.go` rewritten against the new internals, including `TestConcurrentCommandsShareOneConnection` (32 concurrent calls, each response matched to its own request, 32 distinct ids seen server-side)
- [x] 3.3 `go vet ./...` clean and `go test ./...` green; interop matrix re-run with `auth`, `pubsub` and `error` green

## 4. Carried forward — resolved

- [x] 4.1 Binary values now survive a full round trip. This was written as a
      carry-forward: responses reached the module methods as JSON, and
      `encoding/json` replaces invalid UTF-8 with U+FFFD, so `deadbeef` came back
      as `deadefbfbdefbfbd` — the outbound path and the transport were byte-exact,
      but that internal plumbing was not. Fixed in the SDK by `d9a0950` (released
      in Go SDK 1.1.1), which added the `response` seam in `response.go`: replies
      on `synap://` travel to the caller as typed Go values and never pass through
      JSON, so the fix is one seam rather than the 56 decode sites this item
      feared. HTTP and RESP3, which genuinely speak JSON, are unchanged.
      Verified end to end by the SDK's interop client, which round-trips
      `0xDEADBEEF` through SET/GET and asserts byte equality. No follow-up task is
      needed. Residual, documented in `response.go`: `Raw()` still re-encodes on
      the RPC path and carries the same UTF-8 caveat — no module method uses it,
      and its doc comment steers callers to `Decode`
