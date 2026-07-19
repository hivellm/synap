## 1. Unblock

- [ ] 1.1 Confirm the Thunder Go client resolves: `go list -m <path>@latest` succeeds (hivellm/thunder#9)
- [ ] 1.2 Add it to `sdks/go/go.mod` at the version matching the other SDKs

## 2. Transport swap

- [ ] 2.1 Rewrite `sdks/go/transport_rpc.go` as an adapter over Thunder's Go client, built from the Synap `Config`
- [ ] 2.2 Route credentials through the client options so the SDK can reach a `require_auth` deployment
- [ ] 2.3 Consume SUBSCRIBE push frames through the push hook in `sdks/go/pubsub.go`
- [ ] 2.4 Delete the superseded framing, socket, reconnect and request-id code
- [ ] 2.5 Keep `unwrapSynapValue`'s dual `Bytes` tolerance behavior intact, now provided by Thunder

## 3. Tail (docs + tests — check or waive with tailWaiver)

- [ ] 3.1 Update or create documentation covering the implementation — `sdks/go/README.md` and `CHANGELOG.md`
- [ ] 3.2 Write tests covering the new behavior — port `transport_rpc_test.go` to the new internals and add a pipelining test proving concurrent commands share one connection
- [ ] 3.3 Run tests and confirm they pass — `go vet ./... && go test ./...`
