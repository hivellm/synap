## 1. Unblock

- [ ] 1.1 Confirm `HiveLLM.Thunder` 0.2.0 is on NuGet (hivellm/thunder#8)
- [ ] 1.2 Add the package reference to `Synap.SDK.csproj` at the version matching the other SDKs

## 2. Transport swap

- [ ] 2.1 Rewrite `SynapRpcTransport.cs` as an adapter over `ThunderClient`, built from the Synap `Config`
- [ ] 2.2 Delete the local `MsgPack` helper, the framing and the reader loop
- [ ] 2.3 Route credentials through the client options so the SDK can reach a `require_auth` deployment
- [ ] 2.4 Consume SUBSCRIBE push frames through the push hook in `Modules/PubSubManager.cs`
- [ ] 2.5 Map Thunder's typed errors onto the SDK's exception types

## 3. Tail (docs + tests — check or waive with tailWaiver)

- [ ] 3.1 Update or create documentation covering the implementation — `sdks/csharp/README.md` and `CHANGELOG.md`
- [ ] 3.2 Write tests covering the new behavior — keep `TransportTests.cs` and `RpcParityS2STests.cs` green and add a test asserting an over-cap length prefix is refused without allocating
- [ ] 3.3 Run tests and confirm they pass — `dotnet test`
