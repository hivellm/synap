## 1. Upstream gap

- [ ] 1.1 File a `hivellm/thunder` issue: `github.com/hivellm/thunder-go` is unresolvable (404 / proxy not found), with the reproduction and the two viable fixes
- [ ] 1.2 Run `go list -m github.com/hivellm/thunder-go@latest` and record whether the module resolves

## 2. Wire parity (unconditional)

- [ ] 2.1 Decode `Bytes` from both MessagePack `bin` and the legacy int-array form
- [ ] 2.2 Reject a length prefix above the Synap frame cap before allocating the body
- [ ] 2.3 Verify the Go SDK round-trips binary values against a Thunder-based Synap server

## 3. Client swap (when the module resolves)

- [ ] 3.1 Add the Thunder Go client to `sdks/go/go.mod`
- [ ] 3.2 Rewrite `transport_rpc.go` as an adapter over Thunder's Go client with the Synap `Config`
- [ ] 3.3 Route credentials through the client options and consume push frames from the push hook
- [ ] 3.4 Delete the superseded framing, socket and reconnect code
- [ ] 3.5 If the module is still unresolvable at this point, create the follow-up rulebook task for the swap, blocked on the upstream issue from 1.1

## 4. Tail (docs + tests — check or waive with tailWaiver)

- [ ] 4.1 Update or create documentation covering the implementation — `sdks/go/README.md` and `CHANGELOG.md` (Unreleased) with the outcome and, where applicable, the upstream issue link
- [ ] 4.2 Write tests covering the new behavior — keep `integration_test.go` green and add tests for `bin` `Bytes` decoding and over-cap rejection
- [ ] 4.3 Run tests and confirm they pass — `go vet ./... && go test ./...`
