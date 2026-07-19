# Proposal: phase21_thunder-csharp-sdk-swap

Blocked on: https://github.com/hivellm/thunder/issues/8
Supersedes: `phase17_thunder-csharp-sdk`

## Why

`sdks/csharp/src/Synap.SDK/SynapRpcTransport.cs` hand-rolls both the frame
handling and the MessagePack codec (in a local `MsgPack` helper — the SDK has no
MessagePack package reference at all). It carries the same unbounded-allocation
hole the TypeScript and Python transports had:

```csharp
var msgLen = BinaryPrimitives.ReadUInt32LittleEndian(lenBuf);
var msgBuf = new byte[msgLen];   // whatever a remote peer's 4 bytes claimed
```

The swap could not happen because the package is not available:

```
$ curl -s https://api.nuget.org/v3-flatcontainer/hivellm.thunder/index.json
{"versions":["0.1.0"]}
```

Thunder's repo declares `<Version>0.2.0</Version>`, and crates.io, npm and PyPI
all carry 0.2.0 — NuGet stopped at 0.1.0. Pinning 0.1.0 would put Synap's C#
SDK a version behind its own Rust, TypeScript and Python SDKs and miss the
zero-length-frame / keep-alive handling from thunder#6, which is a live interop
difference rather than a cosmetic one. Filed as hivellm/thunder#8.

> **Correction to phase17's premise.** That task's proposal asserted the SDK used
> `MessagePack.Typeless` on untrusted wire data, generalizing from Thunder's
> family-wide C# analysis. That is not true of Synap: `grep -rn Typeless
> sdks/csharp/` returns nothing. The real defect here is the unbounded
> allocation, not typeless deserialization.

## What Changes

Once `HiveLLM.Thunder` 0.2.0 is on NuGet:

- `Synap.SDK.csproj` adds the package reference.
- `SynapRpcTransport.cs` becomes an adapter over `ThunderClient`; the local
  `MsgPack` helper, the framing and the reader loop are deleted.
- Credentials travel in the handshake, so the SDK can reach a `require_auth`
  deployment on 15501 — which it cannot today.
- `Modules/PubSubManager.cs` consumes push frames from the client's push hook.
- `CommandMapper.cs` and the public `SynapClient` surface are untouched.

## Impact
- Affected specs: reuses `phase17_thunder-csharp-sdk/specs/csharp-sdk-transport/spec.md`
- Affected code: `sdks/csharp/src/Synap.SDK/SynapRpcTransport.cs`, `Modules/PubSubManager.cs`, `Synap.SDK.csproj`
- Breaking change: NO — the public client API is unchanged.
- User benefit: closes the unbounded-allocation hole, adds pipelining, timeouts
  and RPC authentication, and removes a hand-written MessagePack codec.
