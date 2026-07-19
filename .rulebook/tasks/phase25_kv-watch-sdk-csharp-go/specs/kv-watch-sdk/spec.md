# Spec: KV Watch SDK surface — C# & Go

## ADDED Requirements

### Requirement: Async-stream watch in the C# SDK
The C# SDK SHALL expose `kv.WatchAsync(pattern, mode, cancellationToken)` returning
`IAsyncEnumerable<WatchEvent>` (Key, Event, Version, Value, Truncated) decoded from the
server envelope.

#### Scenario: Await foreach yields events
Given a C# client iterating `await foreach (var e in kv.WatchAsync("user:1"))`
When the server applies `SET user:1 "alice"`
Then the stream yields a WatchEvent with Key `user:1` and Value `alice`

#### Scenario: Cancellation unwatches
Given an active WatchAsync enumeration
When the cancellation token is cancelled
Then the SDK issues UNWATCH for the pattern

### Requirement: Channel-based watch in the Go SDK
The Go SDK SHALL expose `kv.Watch(ctx, pattern, opts...)` returning a receive-only channel
of `WatchEvent`, closed when the context is cancelled, with UNWATCH issued on cancellation.

#### Scenario: Channel receives events
Given a Go client with `ch, _ := kv.Watch(ctx, "user:*")`
When the server applies `SET user:42 v`
Then a WatchEvent for `user:42` is received on `ch`

#### Scenario: Context cancel closes the channel
Given an active watch channel
When `ctx` is cancelled
Then the SDK issues UNWATCH and the channel is closed

### Requirement: Mode selection
Both SDKs MUST support watch mode `value` (default) and `notify`.

#### Scenario: Notify mode yields events without value
Given a watch in notify mode
When the watched key is set
Then the delivered event has no value and version is present
