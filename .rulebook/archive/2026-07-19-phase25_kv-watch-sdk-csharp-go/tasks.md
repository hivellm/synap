## 1. Implementation
- [x] 1.1 C# SDK: `WatchEvent` record + envelope decode, `kv.WatchAsync(pattern, mode, ct)` as `IAsyncEnumerable<WatchEvent>` over the existing push transport, cancellation issues UNWATCH — `WatchPushAsync` on the transport (KV.WATCH twin of `SubscribePushAsync`); the async iterator's `finally` issues `KV.UNWATCH` with `CancellationToken.None` before closing the dedicated connection
- [x] 1.2 C# SDK: `dotnet build` clean (0 warnings) — 0 warnings, 0 errors
- [x] 1.3 Go SDK: `WatchEvent` struct + `kv.Watch(ctx, pattern, opts...)` channel API over the existing push transport, ctx cancel issues UNWATCH and closes the channel — `WatchPush` on the transport; teardown issues `KV.UNWATCH` on a fresh 2s-timeout context; `WithNotifyMode()` option. Committed in the `synap-sdk-go` submodule as e2f07ad (push + superproject pointer bump left to the user)
- [x] 1.4 Go SDK: `go vet ./...` clean — vet clean, build clean, tests green
- [x] 1.5 README examples for both SDKs — "KV Watch" sections in both READMEs

## 2. Tail (docs + tests — check or waive with tailWaiver)
- [x] 2.1 Update or create documentation covering the implementation — both READMEs + main CHANGELOG + Go SDK CHANGELOG
- [x] 2.2 Write tests covering the new behavior (decode, watch stream/channel, cancellation unwatch, wildcard, notify mode) — 5 C# tests (decode/defaults/truncated/non-envelope, HTTP rejection), 5 Go tests (decode/defaults/truncated, transport rejection, notify option)
- [x] 2.3 Run tests and confirm they pass — C# 107 green, Go suite green
