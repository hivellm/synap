# Spec: KV Watch core notifier

## ADDED Requirements

### Requirement: Value-carrying watch notification
The system SHALL publish a watch event to channel `__watch@0__:<key>` on every KV mutation
(set, del, setex-family, expired, expire, persist, append, setrange, incr), carrying the
post-mutation value, independent of the `notify_keyspace_events` configuration.

#### Scenario: Set publishes value to watchers
Given a subscriber on channel `__watch@0__:user:1`
When `SET user:1 "alice"` executes
Then the subscriber receives an envelope with key `user:1`, event `set`, and value `alice`

#### Scenario: Watch works with keyspace notifications disabled
Given `notify_keyspace_events` is empty (default)
And a subscriber on `__watch@0__:k`
When `SET k v` executes
Then the subscriber receives the watch event

#### Scenario: Partial mutation ships the resulting value
Given key `k` holds `"ab"` and a subscriber on `__watch@0__:k`
When `APPEND k "cd"` executes
Then the received envelope value is `abcd`

### Requirement: Per-key monotonic version
Each watch envelope MUST include a per-key monotonically increasing `version` so clients
can detect missed events after a slow-consumer disconnect.

#### Scenario: Versions increase across mutations
Given a subscriber on `__watch@0__:k`
When `SET k v1` then `SET k v2` execute
Then the second envelope's version is greater than the first's

### Requirement: Inline value cap with notify-only degradation
The notifier MUST omit the value and set `truncated: true` in the envelope when the
post-mutation value exceeds the configured inline cap (default 65536 bytes).

#### Scenario: Oversized value degrades to notify-only
Given the inline cap is 65536 bytes and a subscriber on `__watch@0__:big`
When a 1 MiB value is SET on `big`
Then the envelope has no value and `truncated` is true

### Requirement: No-subscriber fast path
The notifier MUST NOT serialize or publish an envelope when no subscriber matches the
key's watch channel.

#### Scenario: Idle keys cost near zero
Given no subscriber matches `__watch@0__:k`
When `SET k v` executes
Then no envelope is serialized or published for the watch channel
