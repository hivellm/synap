# Proposal: phase25_kv-watch-sdk-csharp-go

Source: docs/analysis/kv-watch-observable/ (F-009, F-010)

## Why

Watch parity for the remaining SDKs. C# (`SynapRpcTransport.cs` + `Modules/PubSubManager.cs`)
and Go (`transport_rpc.go` + `pubsub.go`) already have the push-subscribe primitive. The
analysis explicitly flagged Go so it is not left inconsistent — "todas as SDKs" includes it.

## What Changes

- C# SDK: `kv.WatchAsync(pattern, mode, ct)` returning `IAsyncEnumerable<WatchEvent>`,
  mirroring `PubSubManager`'s consumption style. `WatchEvent` record with Key, Event,
  Version, Value, Truncated. Cancellation issues UNWATCH.
- Go SDK: `kv.Watch(ctx, pattern, opts...) (<-chan WatchEvent, error)` mirroring
  `pubsub.go`'s channel style. Context cancellation issues UNWATCH and closes the channel.
- Both: envelope decode matches the phase-23 reference; reconnect follows each SDK's
  existing pub/sub semantics.

## Impact

- Affected specs: specs/kv-watch-sdk/spec.md (ADDED C#/Go requirements)
- Affected code: sdks/csharp/src/Synap.SDK/ (KV module + WatchEvent),
  sdks/go/ (kv watch + WatchEvent)
- Breaking change: NO (additive API)
- User benefit: idiomatic key observation in C# (IAsyncEnumerable) and Go (channel), closing
  full six-SDK parity.
