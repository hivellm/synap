# Spec: KV Watch server endpoints

## ADDED Requirements

### Requirement: WATCH command over SynapRPC
The system SHALL accept a `WATCH <pattern> [mode]` command on the SynapRPC port that
subscribes the connection to `__watch@0__:<pattern>` and delivers watch envelopes as
Thunder push frames, and an `UNWATCH <pattern>` command that stops delivery.

#### Scenario: Watch then set delivers a push frame
Given an RPC connection that issued `WATCH user:1`
When another connection executes `SET user:1 "alice"`
Then the watcher receives a push frame whose envelope has key `user:1` and value `alice`

#### Scenario: Unwatch stops delivery
Given an RPC connection watching `user:1`
When it issues `UNWATCH user:1` and `SET user:1 v2` executes
Then no further push frame is delivered for `user:1`

#### Scenario: Wildcard watch
Given an RPC connection that issued `WATCH user:*`
When `SET user:42 v` executes
Then the watcher receives the envelope for `user:42`

### Requirement: Notify-only subscription mode
The `WATCH` command MUST accept mode `notify`, in which delivered envelopes omit the
inline value while keeping key, event, and version.

#### Scenario: Notify mode strips the value
Given an RPC connection that issued `WATCH k notify`
When `SET k v` executes
Then the received envelope contains key and version but no value

### Requirement: KV watch over WebSocket
The `/kv/ws` endpoint SHALL accept `?keys=<k1,k2,...>` (wildcards allowed), subscribe the
socket to the corresponding watch channels, and stream watch envelopes as JSON frames.
It MUST no longer return 501.

#### Scenario: WebSocket watch receives updates
Given a WebSocket connected to `/kv/ws?keys=session:*`
When `SET session:9 v` executes
Then the socket receives a JSON frame with key `session:9` and value `v`

### Requirement: Configurable inline cap
The server MUST expose `watch.max_inline_value_bytes` (default 65536) in config.yml with
environment override `SYNAP_WATCH_MAX_INLINE_VALUE_BYTES`.

#### Scenario: Env override applies
Given `SYNAP_WATCH_MAX_INLINE_VALUE_BYTES=1024`
When the server starts
Then values larger than 1024 bytes are delivered truncated (notify-only envelope)
