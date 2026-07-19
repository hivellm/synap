## 1. Unblock

- [x] 1.1 Confirm `HiveLLM.Thunder` 0.2.0 is on NuGet (hivellm/thunder#8) — published and indexed
- [x] 1.2 Add the package reference to `Synap.SDK.csproj` at the version matching the other SDKs (0.2.0)

## 2. Transport swap

- [x] 2.1 Rewrite `SynapRpcTransport.cs` as an adapter over `ThunderClient`, built from the Synap `Config`
- [x] 2.2 Delete the local `MsgPack` helper, the framing and the reader loop — the helper moved to the test project rather than being deleted outright, because the transport tests use it to decode frames independently of Thunder, which is what makes them prove wire compatibility rather than self-consistency. It is gone from the shipped package either way.
- [x] 2.3 Route credentials through the client options so the SDK can reach a `require_auth` deployment
- [x] 2.4 Consume SUBSCRIBE push frames through the push hook in `Modules/PubSubManager.cs` — the transport keeps its `IAsyncEnumerable` signature, now fed by Thunder's push hook through a channel, so `PubSubManager` needed no change
- [x] 2.5 Map Thunder's typed errors onto the SDK's exception types

## 3. Tail (docs + tests — check or waive with tailWaiver)

- [x] 3.1 Update or create documentation covering the implementation — `sdks/csharp/README.md` (new "The `synap://` transport is Thunder" section) and `CHANGELOG.md`
- [x] 3.2 Write tests covering the new behavior — the `WireValue` tests were rewritten against Thunder's `Value` (including the UTF-8-vs-binary `Bytes` split and the array/map cases the old suite never covered), a `SynapConfig` test pins the seven values the server declares independently, and a new socket-level test asserts an over-cap length prefix is refused without allocating
- [x] 3.3 Run tests and confirm they pass — `dotnet build` clean with `TreatWarningsAsErrors` on (0 warnings, 0 errors) and `dotnet test` green: 102 passed, 0 failed, 48 requiring a live server not run

## 4. Bugs found by the swap

- [x] 4.1 The old `SubscribePushAsync` sent SUBSCRIBE with `id = 0xFFFFFFFF` — the reserved push sentinel. A Thunder server refuses a request carrying that id, so pub/sub over RPC would have failed outright against the 1.1.0 server. Thunder allocates a normal request id and routes push frames by the sentinel, which is what the sentinel is for.
