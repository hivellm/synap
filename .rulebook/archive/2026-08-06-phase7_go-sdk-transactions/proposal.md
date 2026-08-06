# Proposal: phase7_go-sdk-transactions

## Why

Five of the six SDKs can run a transaction: TypeScript, Python, PHP, C# and
Rust all expose `MULTI`/`EXEC`/`DISCARD`/`WATCH`, and since server 1.3.0 (ADR
005) their writes travel as `TXQUEUE <client_id> <CMD> <args...>` so the server
queues them into the open `MULTI` instead of executing them immediately. The Go
SDK has no transaction surface at all — no `TransactionManager`, no `client_id`
plumbing, no `MULTI` in its command map. A Go user cannot write an atomic
multi-key update, and there is no error telling them so: the feature is simply
absent.

This surfaced while syncing the submodule SDKs for 1.3.1 (phase6). It is not a
fix — it is a missing module — so it does not belong in a patch release, but it
is the largest remaining parity gap in the SDK matrix and needs to be tracked
rather than rediscovered.

## What Changes

- A `TransactionManager` on the Go client mirroring the other SDKs:
  `Multi`, `Exec`, `Discard`, `Watch`, `Unwatch`, keyed by a `client_id` the
  SDK generates and threads through the transaction's writes.
- Command-map support: `transaction.*` maps to `MULTI`/`EXEC`/`DISCARD`/
  `WATCH`/`UNWATCH`, and any write carrying a `client_id` is wrapped as
  `TXQUEUE`, refusing (rather than silently executing) commands outside the
  server's queueable set — `SET`/`DEL`/`INCR[BY]`/`DECR[BY]`/`HSET`/`HDEL`/
  `HINCRBY`/`LPUSH`/`RPUSH`/`LPOP`/`RPOP`/`SADD`/`SREM`.
- Reply translation for `EXEC` (result list) and the control commands.
- Unit tests pinning the wire names and the TXQUEUE wrapping, plus an S2S test
  proving a transaction is atomic on HTTP and on SynapRPC.

## Impact

- Affected specs: none in this repository (the code lives in
  `hivellm/synap-sdk-go`)
- Affected code: `client.go` (command map + reply translation),
  a new `transaction.go`, `command_map_test.go`
- Breaking change: NO — purely additive.
- User benefit: Go users get the same transactional guarantees as every other
  SDK, on both HTTP and the native transports.
