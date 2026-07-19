# Proposal: phase18_thunder-go-sdk

Source: https://github.com/hivellm/thunder — `docs/analysis/04-adoption-plan.md` (P5),
README "Packages" table.

## Why

`sdks/go/transport_rpc.go` is the last Synap RPC transport with a Thunder
counterpart. Thunder's Go client is implemented and interop-verified, and the README
lists it as `github.com/hivellm/thunder-go`, "released by git tag".

**That module is not resolvable.** `https://github.com/hivellm/thunder-go` returns
404 and `proxy.golang.org` reports the module as not found — the Go client lives in
the `go/` directory of the `hivellm/thunder` monorepo, whose own `go.mod` declares
`module github.com/hivellm/thunder-go`, a path that does not correspond to any
repository. No Go consumer can currently `go get` it.

So this task has two possible outcomes and both are acceptable deliverables:

1. Thunder publishes the module (own repo, or a module path matching the monorepo
   layout) → the Go SDK swaps like every other language.
2. It does not publish within this release → the Go SDK keeps its own transport, but
   is brought to wire parity with Thunder (`bin` `Bytes` on decode, frame cap
   enforced before allocation), and the swap is deferred to a follow-up task
   blocked on the upstream issue.

Either way, an issue is filed on `hivellm/thunder` first — the blocking gap is real
and belongs upstream, not worked around in silence.

## What Changes

- An issue is filed on `hivellm/thunder`: the Go module path is unresolvable, with
  the reproduction (`go get github.com/hivellm/thunder-go`) and the two viable fixes.
- If the module becomes resolvable: `sdks/go/transport_rpc.go` becomes an adapter
  over Thunder's Go client, with the Synap `Config`, credentials through the client
  options, and pub/sub on the push hook.
- If it does not: `sdks/go/transport_rpc.go` is hardened to Thunder's wire contract
  — decode `Bytes` from both `bin` and int-array, reject an over-cap length prefix
  before allocating — and a follow-up rulebook task is created for the swap,
  referencing the upstream issue.
- `sdks/go/client.go`'s public API is untouched either way.

## Impact
- Affected specs: `.rulebook/tasks/phase18_thunder-go-sdk/specs/go-sdk-transport/spec.md`
- Affected code: `sdks/go/transport_rpc.go`, `sdks/go/go.mod`
- Breaking change: NO — the exported client API is unchanged.
- User benefit: the Go SDK interoperates correctly with a Thunder-based server
  (which emits `bin` `Bytes` from phase12) and gains the frame cap it lacks today.
