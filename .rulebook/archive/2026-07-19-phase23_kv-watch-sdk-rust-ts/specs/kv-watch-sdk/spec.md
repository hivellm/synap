# Spec: KV Watch SDK surface — Rust & TypeScript

## ADDED Requirements

### Requirement: Reactive watch in the Rust SDK
The Rust SDK SHALL expose `kv.watch(pattern)` returning an `Observable<WatchEvent>` from
the SDK's `rx` module, where `WatchEvent` carries key, event, version, optional value, and
the truncated flag decoded from the server envelope.

#### Scenario: Observing a key yields events
Given a Rust client with `kv.watch("user:1")` subscribed
When the server applies `SET user:1 "alice"`
Then the Observable emits a WatchEvent with key `user:1` and value `alice`

#### Scenario: Dropping the subscription unwatches
Given a subscribed watch Observable
When the subscription is dropped
Then the SDK issues UNWATCH for the pattern

### Requirement: Reactive watch in the TypeScript SDK
The TypeScript SDK SHALL expose `kv.watch<T>(pattern, opts?)` returning an rxjs
`Observable<WatchEvent<T>>`, consistent with the pub/sub module's Observable API.

#### Scenario: Observing with wildcard
Given a TS client with `kv.watch("user:*")` subscribed
When the server applies `SET user:42 v`
Then the Observable emits a WatchEvent for `user:42`

#### Scenario: Truncated envelope triggers value fetch helper
Given a TS client using the `withValueFetch` option
When a truncated envelope arrives for key `big`
Then the SDK issues a GET for `big` and emits the event with the fetched value

### Requirement: Mode selection
Both SDKs MUST allow selecting watch mode `value` (default) or `notify` at subscribe time.

#### Scenario: Notify mode emits without value
Given a watch subscription in notify mode
When the watched key is set
Then the emitted event has no value and version is present
