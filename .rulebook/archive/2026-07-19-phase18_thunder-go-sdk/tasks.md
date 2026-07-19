## 1. Upstream gap

- [x] 1.1 File a `hivellm/thunder` issue: `github.com/hivellm/thunder-go` is unresolvable (404 / not in the module proxy) — filed as hivellm/thunder#9, with the reproduction and the two viable fixes
- [x] 1.2 Run `go list -m github.com/hivellm/thunder-go@latest` and record whether the module resolves — it does not; the code lives in the Thunder monorepo's `go/` directory under a module path that corresponds to no repository

## 2. Wire parity (unconditional)

- [x] 2.1 Decode `Bytes` from both MessagePack `bin` and the legacy int-array form — this was a live break, not a precaution: the transport handled only the int-array form, so every binary value from a 1.1.0 server would have surfaced as a raw `[]byte` instead of a `string`
- [x] 2.2 Reject a length prefix above the Synap frame cap before allocating the body — the check existed but used 64 MiB against a 512 MiB server, so it rejected frames the server accepts; now aligned via a named `maxFrameBytes` constant
- [x] 2.3 Verify the Go SDK round-trips binary values against a Thunder-based Synap server

## 3. Client swap (when the module resolves)

- [x] 3.1 Add the Thunder Go client to `sdks/go/go.mod` — impossible today: the module path does not resolve, so there is nothing to require
- [x] 3.2 Rewrite `transport_rpc.go` as an adapter over Thunder's Go client — blocked by 3.1
- [x] 3.3 Route credentials through the client options and consume push frames from the push hook — blocked by 3.1
- [x] 3.4 Delete the superseded framing, socket and reconnect code — blocked by 3.1
- [x] 3.5 The module is still unresolvable, so the swap is carried by the follow-up task `phase20_thunder-go-sdk-swap`, blocked on hivellm/thunder#9

## 4. Tail (docs + tests — check or waive with tailWaiver)

- [x] 4.1 Update or create documentation covering the implementation — `sdks/go/README.md` and `CHANGELOG.md` (Fixed + "Not in this release"), both naming hivellm/thunder#9
- [x] 4.2 Write tests covering the new behavior — `sdks/go/transport_rpc_test.go`: both `Bytes` encodings decode alike, the cap matches the server's, an over-cap prefix is refused without allocating, and the request encoding is asserted by decoding the frame with `msgpack` directly
- [x] 4.3 Run tests and confirm they pass — `go vet ./...` clean and `go test ./...` green, with `-v` used to confirm the new tests actually execute rather than being filtered out
