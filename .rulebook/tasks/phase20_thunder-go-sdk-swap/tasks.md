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

## 4. Carried forward

- [ ] 4.1 Binary values still do not survive a full round trip: responses are handed
      to the module methods as JSON, and `encoding/json` replaces invalid UTF-8 with
      U+FFFD on the way back. The outbound path and the transport are byte-exact; the
      remaining loss is that internal plumbing, across 56 decode sites in eight
      modules. Out of scope for a transport swap — needs its own task
