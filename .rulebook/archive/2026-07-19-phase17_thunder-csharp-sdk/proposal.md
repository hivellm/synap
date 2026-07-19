# Proposal: phase17_thunder-csharp-sdk

Source: https://github.com/hivellm/thunder — `docs/analysis/04-adoption-plan.md` (P3).

## Why

`sdks/csharp/src/Synap.SDK/SynapRpcTransport.cs` is another hand-port of the shared
wire layer. Thunder's inventory singled out the family's C# transports as the worst
of the three languages: three different serialization strategies across the copies,
including `MessagePack.Typeless` applied to untrusted wire data — a deserializer
that instantiates types named by the payload.

`HiveLLM.Thunder` (NuGet, `net8.0`) uses the `MessagePack` low-level
writer/reader only, never `Typeless`, and brings the same uniform floor as every
other language.

## What Changes

- `sdks/csharp/src/Synap.SDK/Synap.SDK.csproj` adds the `HiveLLM.Thunder` package
  reference and drops the transport's direct MessagePack usage.
- `SynapRpcTransport.cs` becomes an adapter over `ThunderClient`, constructed from
  the Synap `Config` (scheme `synap`, port 15501, `AuthCommand` handshake, push
  enabled).
- Value conversion between the SDK's types and Thunder's `Value` is centralized;
  `Bytes` decoding accepts both MessagePack `bin` and the legacy int-array form.
- `Modules/PubSubManager.cs` consumes push frames from the client's push hook.
- `CommandMapper.cs` and the public `SynapClient` surface are untouched.

## Impact
- Affected specs: `.rulebook/tasks/phase17_thunder-csharp-sdk/specs/csharp-sdk-transport/spec.md`
- Affected code: `sdks/csharp/src/Synap.SDK/SynapRpcTransport.cs`, `sdks/csharp/src/Synap.SDK/Modules/PubSubManager.cs`, `sdks/csharp/src/Synap.SDK/Synap.SDK.csproj`
- Breaking change: NO — the public client API is unchanged.
- User benefit: removes typeless deserialization of untrusted wire data, adds the
  frame cap and timeouts, and pipelines requests on one connection.
