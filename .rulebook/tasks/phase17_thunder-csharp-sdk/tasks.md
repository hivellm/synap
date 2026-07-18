## 1. Dependency swap

- [ ] 1.1 Add the `HiveLLM.Thunder` package reference to `Synap.SDK.csproj` and remove the transport's direct MessagePack usage
- [ ] 1.2 Add a static holder exposing the Synap `Config` (scheme `synap`, port 15501, `AuthCommand`, push enabled)
- [ ] 1.3 `dotnet build` clean

## 2. Transport rewrite

- [ ] 2.1 Rewrite `SynapRpcTransport.cs` as an adapter over `ThunderClient`
- [ ] 2.2 Centralize SDK-value ↔ `Value` conversion, decoding `Bytes` from both `bin` and the legacy int-array form
- [ ] 2.3 Route credentials through `ClientConfig`/`Credentials` instead of a hand-written AUTH frame
- [ ] 2.4 Map Thunder's typed errors onto the SDK's exception types
- [ ] 2.5 Delete the superseded framing, socket and reconnect code — and confirm no `Typeless` deserialization remains anywhere in the SDK

## 3. Push path

- [ ] 3.1 Consume SUBSCRIBE push frames through the client's push hook in `Modules/PubSubManager.cs`

## 4. Tail (docs + tests — check or waive with tailWaiver)

- [ ] 4.1 Update or create documentation covering the implementation — `sdks/csharp/README.md` and `CHANGELOG.md` (Unreleased → Changed)
- [ ] 4.2 Write tests covering the new behavior — keep `TransportTests.cs` and `RpcParityS2STests.cs` green and add an over-cap length-prefix test
- [ ] 4.3 Run tests and confirm they pass — `dotnet test`
