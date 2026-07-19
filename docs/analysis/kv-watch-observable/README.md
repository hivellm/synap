# Analysis: KV Watch/Observable — broadcast key updates to all listeners

**Slug:** `kv-watch-observable`
**Date:** 2026-07-19
**Status:** Approved — materialized into rulebook tasks
**Scope:**

1. Server-side "watch a key → broadcast the new value on change" primitive, reusing the
   existing `PubSubRouter` fan-out and the Thunder RPC push bridge.
2. `kv.watch(pattern)` in all six SDKs (rust, typescript, python, php, csharp, go),
   returning each SDK's native reactive idiom (Observable / async iterator / channel).
3. Efficiency target: millions of concurrent connections — achieved by riding the existing
   bounded-channel push path with slow-consumer drop, not by building a new fan-out engine.

## Documents

- [findings.md](findings.md) — 11 numbered findings (F-001..F-011) across 3 workstreams,
  with evidence and confidence.
- [execution-plan.md](execution-plan.md) — 6 phases mapped 1:1 to rulebook tasks
  (phase21..phase26), plus risk register.

## Method

Direct source reading of `synap-core` (pubsub, keyspace, kv_store), `synap-server`
(SynapRPC push bridge, WebSocket handlers, config) and all six SDK pub/sub modules.

## Headline conclusions

1. **The hard infrastructure already exists.** Server-push pub/sub over Thunder RPC is
   shipping (F-001), keyspace notifications already fire on every KV mutation (F-002), and
   `PubSubRouter` already solves fan-out + slow-consumer backpressure (F-004). Watch is a
   thin composition layer, not a new subsystem.
2. **The single real server gap:** keyspace notifications carry the event name only — the
   new value is in scope at the notify site but not forwarded (F-002). Fix: a dedicated
   always-on `__watch@0__:<key>` channel carrying the post-mutation value, independent of
   the Redis-compat `notify_keyspace_events` flag which defaults OFF (F-006).
3. **The `/kv/ws` WATCH endpoint is already routed but returns 501** — its stated blocker
   is now solved; complete it by mirroring the working pub/sub WebSocket handler (F-003).
4. **All six SDKs already have `subscribePush`** — `kv.watch()` is a thin wrapper returning
   each SDK's existing reactive type (F-009, F-010). Wildcard watch is essentially free (F-011).
5. **Semantics:** best-effort latest-value with per-key `version` for gap detection; replay
   belongs to streams (F-007). Inline-value cap with notify-only degradation for large
   values (F-008).

## Materialized rulebook tasks

| Task | Findings |
|------|----------|
| `phase21_kv-watch-core-notifier` | F-002, F-004..F-008 |
| `phase22_kv-watch-server-endpoints` | F-001, F-003 |
| `phase23_kv-watch-sdk-rust-ts` | F-009..F-011 |
| `phase24_kv-watch-sdk-python-php` | F-009, F-010 |
| `phase25_kv-watch-sdk-csharp-go` | F-009, F-010 |
| `phase26_kv-watch-interop-bench-docs` | all |
