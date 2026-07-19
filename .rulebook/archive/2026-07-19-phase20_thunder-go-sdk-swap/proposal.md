# Proposal: phase20_thunder-go-sdk-swap

Blocked on: https://github.com/hivellm/thunder/issues/9
Follow-up to: `phase18_thunder-go-sdk`

## Why

phase18 brought the Go SDK to Thunder *wire* parity — it decodes the canonical
`bin` `Bytes` form alongside the legacy int-array one, and enforces the same
512 MiB frame cap the server does — but it is still the one Synap SDK carrying
its own copy of the protocol. That copy is exactly what Thunder exists to end:
the 18-implementation duplication, the drift, and the per-language feature gaps
(this transport has no multiplexing, no push hook, and no `AUTH`).

The swap could not happen because the Thunder Go client is not consumable:

```
$ go list -m github.com/hivellm/thunder-go@latest
go: module github.com/hivellm/thunder-go: ... exit status 128
$ curl -s -o /dev/null -w "%{http_code}" https://github.com/hivellm/thunder-go
404
```

The code exists in the Thunder monorepo's `go/` directory, but its `go.mod`
declares a module path that corresponds to no repository, so no consumer can
fetch it. Filed as hivellm/thunder#9 with two suggested fixes.

## What Changes

Once the module resolves:

- `sdks/go/go.mod` requires the Thunder Go client.
- `sdks/go/transport_rpc.go` becomes an adapter over it: the framing, socket
  handling, reconnect loop and the request-id counter are deleted.
- Commands pipeline over one connection instead of serializing behind the
  transport's mutex.
- Credentials travel in the handshake, so the SDK can reach a `require_auth`
  deployment on 15501 — which it cannot today.
- Pub/sub consumes push frames through the client's push hook.
- `sdks/go/client.go`'s public API is untouched.

## Impact
- Affected specs: reuses `phase18_thunder-go-sdk/specs/go-sdk-transport/spec.md`
- Affected code: `sdks/go/transport_rpc.go`, `sdks/go/pubsub.go`, `sdks/go/go.mod`
- Breaking change: NO — the exported client API is unchanged.
- User benefit: pipelining, enforced timeouts, RPC authentication, and one less
  hand-maintained copy of the wire protocol.
