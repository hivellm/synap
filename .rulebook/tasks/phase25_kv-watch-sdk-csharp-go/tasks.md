## 1. Implementation
- [ ] 1.1 C# SDK: `WatchEvent` record + envelope decode, `kv.WatchAsync(pattern, mode, ct)` as `IAsyncEnumerable<WatchEvent>` over the existing push transport, cancellation issues UNWATCH
- [ ] 1.2 C# SDK: `dotnet build` clean (0 warnings)
- [ ] 1.3 Go SDK: `WatchEvent` struct + `kv.Watch(ctx, pattern, opts...)` channel API over the existing push transport, ctx cancel issues UNWATCH and closes the channel
- [ ] 1.4 Go SDK: `go vet ./...` clean
- [ ] 1.5 README examples for both SDKs

## 2. Tail (docs + tests — check or waive with tailWaiver)
- [ ] 2.1 Update or create documentation covering the implementation
- [ ] 2.2 Write tests covering the new behavior (decode, watch stream/channel, cancellation unwatch, wildcard, notify mode)
- [ ] 2.3 Run tests and confirm they pass
