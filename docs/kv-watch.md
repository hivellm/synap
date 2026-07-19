# KV Watch — value-carrying key change notifications

Watch lets a client observe a key and receive **the new value** when it changes,
instead of learning only that something happened and having to `GET` again.

This document covers the core notifier (`synap-core`) and the server surfaces
that ride on it: the `KV.WATCH` SynapRPC command and the `/kv/ws` WebSocket
endpoint. The Rust and TypeScript SDKs expose it as `kv.watch()` (see each
SDK's README); the remaining SDKs are separate phases.

## Why it is not keyspace notifications

Synap already has Redis-compatible keyspace notifications, and they fire on
every KV mutation. They are the wrong foundation for watch on two counts:

- **They carry the event name only.** The post-mutation value is in scope at the
  notify site but is not forwarded, so every watcher would have to `GET` after
  every event — an extra round trip and a race window in which the value can
  change again.
- **They default to off.** `notify-keyspace-events` is empty unless configured,
  so a watch built on them would silently do nothing at the default
  configuration. Extending the payload with values would also break the Redis
  semantics the channels exist to provide.

So watch has its own channel family, `__watch@0__:<key>`, published through the
same `PubSubRouter` — which already solves fan-out, wildcard matching and
slow-consumer backpressure. Watch is a composition layer, not a new subsystem.

## The envelope

```json
{
  "key": "user:1",
  "event": "set",
  "version": 7,
  "value": "alice"
}
```

| Field | Meaning |
|---|---|
| `key` | The key that changed. |
| `event` | `set`, `del`, `expired`, `evicted`, `expire`, `persist`, `append`, `setrange`, `incrby`, `decrby`. |
| `version` | Per-key monotonic counter, starting at 1. |
| `value` | The post-mutation value. Omitted, not null, when absent. |
| `truncated` | Present and `true` only when the value was withheld. |

For a **partial mutation** — `APPEND`, `SETRANGE`, `INCR`/`DECR` — the value is
the **resulting** value, never the operand. `APPEND k "cd"` on a key holding
`"ab"` delivers `abcd`. A watcher never has to re-`GET` to learn what the key
now holds.

`expire` and `persist` carry no `value`: they change the TTL, not the value, so
the watcher already holds the latest one.

`expired` fires however the key actually leaves — through the active expiration
cycle or lazily on a read — and `evicted` fires when the `maxmemory` policy
removes a key. Both are terminal: the version counter resets like on `del`.

`FLUSHDB` publishes no per-key events (Redis parity), but it does reset every
version counter, so a flushed key's next incarnation starts at version 1.

## Semantics

**Best-effort, latest-value.** A watcher that cannot keep up is disconnected by
the router's existing bounded-channel policy; it must re-`GET` and re-subscribe.
Replay is a streams feature, and streams are the right tool when you need it.

**`version` is how you detect that.** It increases by one per delivered event
for that key, so a client that sees 7 then 9 knows it missed one. Versions reset
when a key is deleted or expires — the terminal `del`/`expired` event carries
its own version first, so the reset is never ambiguous in context.

**Versions are assigned after the shard lock is released**, so two writers
racing on the same key can publish their events out of order — the event with
the higher version is not guaranteed to carry the later value. A client should
treat `version` as a gap and staleness signal, not as a linearization order;
when the true latest value matters, re-`GET` (or use CAS / key locks on the
write side). This is the price of keeping the watch lookup off the locked
mutation path.

**Values are withheld above a cap.** Broadcasting a value to N watchers
multiplies bandwidth by value size × N, so above the inline cap (64 KiB by
default) the event is delivered with no `value` and `truncated: true`. The
client knows the key changed and re-`GET`s if it wants the payload.

**Non-UTF-8 values are also notify-only.** The envelope's `value` is a string,
and re-encoding arbitrary bytes lossily would hand the watcher something the key
does not hold. Such a value is withheld with `truncated: true` for the same
reason an oversized one is.

## Cost when nobody is watching

The notifier fires on every KV mutation, so its idle cost is what matters. It is
one `PubSubRouter::has_subscriber` lookup: no envelope is built, nothing is
serialized, nothing is published, and the key does not even get a version
counter. The counter map is therefore bounded by the *watched* keyspace, not by
the store, and entries are dropped when a key is deleted.

`APPEND` and `SETRANGE` need the merged bytes to ship the resulting value, and
that copy happens only when a notifier is attached.

## Watching over SynapRPC

```
KV.WATCH <pattern> [mode]      → { subscriber_id, channel, mode }
KV.UNWATCH <subscriber_id> [pattern ...]
```

Named `KV.WATCH` because plain `WATCH` is the transaction command (optimistic
locking), whose `WATCH client_id key` shape would be indistinguishable.

`mode` is `value` (default) or `notify`. A `notify` subscription receives the
envelope without `value`/`truncated` — the strip happens server-side, per
subscription, so a client that only wants change signals does not pay value
bandwidth. Envelopes arrive as ordinary push frames on the same connection,
exactly like `SUBSCRIBE` — same bridge, same slow-consumer policy.

Wildcards glob within the key: `KV.WATCH user:*` sees every `user:`-prefixed
key. (`*` in a pub/sub topic still matches one `.`-segment; a `*` embedded in a
segment globs inside it, which is what `:`-style keys need.)

`KV.UNWATCH` with no patterns drops every subscription of that subscriber.

## Watching over WebSocket

```
GET /kv/ws?keys=user:1,session:*
```

Each key (wildcards allowed) becomes its `__watch@0__:<key>` channel and the
connection speaks the ordinary pub/sub socket protocol: one `connected` welcome
frame, then `{"type": "message", "topic": ..., "payload": <envelope>}` frames.
Slow consumers are disconnected by the same bounded-channel policy.

## Configuration

```yaml
watch:
  max_inline_value_bytes: 65536   # SYNAP_WATCH_MAX_INLINE_VALUE_BYTES
```

There is no enable flag — watch is always on, and an unwatched deployment pays
one router lookup per mutation. Embedders wire the notifier themselves:

```rust
use std::sync::Arc;
use synap_core::core::{KVConfig, KVStore, KeyWatchNotifier, PubSubRouter};

let router = Arc::new(PubSubRouter::new());
let notifier = Arc::new(KeyWatchNotifier::new(Arc::clone(&router), 0));
let store = KVStore::new(KVConfig::default())
    .with_watch_notifier(Some(notifier));
```

Override the inline cap with `KeyWatchNotifier::with_inline_cap`. A store with
no notifier attached behaves exactly as before, at no cost.
