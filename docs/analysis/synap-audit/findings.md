# Synap Deep Audit — Beyond the v1.0.0 Plan

**Date:** 2026-07-08
**Method:** direct source reading of `synap-server/src/**` with file:line evidence.
**Scope:** correctness, durability, security and performance gaps that are NOT already
covered by the v1.0.0 rulebook plan (`phase1..phase8`) and are NOT already shipped.
**Competitor baselines:** Redis (KV/durability/protocol), Kafka (streams), RabbitMQ (queues).

This is the "what still needs to be fixed to be genuinely better than Redis/Kafka/RabbitMQ"
list. Findings are ordered by severity. Each is verified against the actual code, not assumed.

> The v1.0.0 plan already owns: crates/ restructure, dependabot, flaky-test deflake,
> unwrap triage, `Resp3Config::enabled` fix, bind-default unification, pipelining flush check,
> mimalloc flag, parallel MGET/MSET, and the ship/defer decisions for BLPOP/PSUBSCRIBE/SCAN/LFU.
> Nothing below duplicates those.

## Severity summary

| Severity | Count | Findings |
|----------|-------|----------|
| Critical | 7 | M-001 … M-007 |
| High | 6 | M-008 … M-013 |
| Medium | 5 | M-014 … M-018 |

## Index by subsystem

- **Durability / persistence:** M-001 (snapshot omits Hash/List/Set/ZSet), M-002 (snapshot checksum never verified), M-014 (stream WAL replay is a no-op)
- **Security:** M-003 (SynapRPC has no auth), M-004 (RESP3 auth stubbed to always-true), M-009 (unsalted SHA-512 + non-constant-time compare)
- **Availability:** M-005 (replication never wired into the running server)
- **Network hot path / DoS:** M-006 (RESP3 unbounded allocation), M-007 (SynapRPC unbounded frame allocation), M-015 (no max-connections)
- **Transactions:** M-008 (MULTI/EXEC not atomic), M-010 (transactions bypass WAL + replication)
- **Pub/Sub:** M-011 (unbounded per-subscriber buffer)
- **Streams vs Kafka:** M-012 (RAM-only, silent drop of unread events), M-016 (O(n) consume scan)
- **Queues vs RabbitMQ:** M-013 (no prefetch/QoS, consumer count faked), M-017 (global 1 s deadline sweep)
- **KV memory:** M-018 (collections not counted toward maxmemory, full value copy on GET)

---

## Critical

### M-001 — Snapshots silently omit Hash, List, Set and Sorted-Set data → data loss on restart
- **Subsystem:** persistence · **Evidence:** `persistence/snapshot.rs:26-203` (`create_snapshot`) writes only KV, Queue and Stream sections — there is no Hash/List/Set/ZSet section. The loader `persistence/snapshot.rs:338-340` hardcodes `list_data / set_data / sorted_set_data` to empty maps ("Empty for now, will be populated from WAL replay"), and `recovery.rs:107` seeds `HashStore::new()` (empty). Recovery then replays the WAL only from `snapshot.wal_offset` forward (`recovery.rs:128-129`).
- **Failure scenario:** write 1M hash fields → a periodic snapshot runs (`layer.rs:434-469`) and advances `wal_offset` past those writes → process restarts → hashes/lists/sets/zsets written before the snapshot are gone, because they were neither in the snapshot nor in the replayed WAL tail.
- **vs competitor:** Redis RDB/AOF persist every datatype; a snapshot is a complete point-in-time image.
- **Fix:** add Hash/List/Set/SortedSet sections to the streaming snapshot writer and loader (symmetrical with KV), or forbid snapshot-driven WAL truncation for types not covered.

### M-002 — Snapshot checksum is written but never verified on load
- **Subsystem:** persistence · **Evidence:** writer computes a CRC64 over the whole file and appends it (`snapshot.rs:48-58,187-189`); loader reads it into a discarded binding: `snapshot.rs:320` `let _checksum = reader.read_u64().await.unwrap_or(0); // Optional`. It is never compared to a recomputed digest.
- **Failure scenario:** a torn or bit-rotted snapshot loads silently and the server comes up with corrupt data; the integrity mechanism that exists is dead.
- **vs competitor:** Redis validates the RDB CRC64 on load and refuses a corrupt file (unless explicitly disabled).
- **Fix:** recompute the digest while streaming and reject the snapshot on mismatch.

### M-003 — SynapRPC binary protocol has no authentication or ACL enforcement
- **Subsystem:** security · **Evidence:** `protocol/synap_rpc/dispatch/mod.rs:19-47` — `dispatch()` maps commands straight to store operations with no auth check, no user context, no ACL. The listener (default port 15501, and per the v1.0 analysis binds `0.0.0.0`) accepts `SET/GET/DEL/FLUSHALL/KEYS` from any client.
- **Failure scenario:** anyone who can reach the port runs `FLUSHALL` or dumps all keys with `KEYS *`.
- **vs competitor:** Redis enforces ACL/AUTH on every connection including RESP; RabbitMQ requires credentials per connection.
- **Fix:** thread an auth/ACL check into `dispatch()` (HELLO/AUTH handshake + per-command permission), mirroring the HTTP extractor path.

### M-004 — RESP3 authentication is stubbed to always-authenticated
- **Subsystem:** security · **Evidence:** `protocol/resp3/server.rs:75` `let mut authenticated = true;` and `server.rs:212-216` `async fn check_auth(...) -> bool { true }` (always returns true). The AUTH command "succeeds" without validating anything, and the initial state already bypasses the gate.
- **Failure scenario:** the Redis-compatible listener (port 6379) executes every command unauthenticated regardless of the configured users/passwords.
- **vs competitor:** Redis rejects commands with NOAUTH until a valid AUTH.
- **Fix:** wire `AppState` auth into `check_auth` and default `authenticated` to `!auth.required`.

### M-005 — Replication is configured but never instantiated in the running server
- **Subsystem:** availability · **Evidence:** `main.rs` only imports `NodeRole` and sets `config.replication.*` from CLI (`main.rs:10,123-140`); it never constructs `MasterNode`/`ReplicaNode`. `AppState` has no master/replica field (`server/handlers/mod.rs` — no match for `Master`). `MasterNode::new`/`ReplicaNode::new` are called only from `#[cfg(test)]` and `failover.rs` (`grep` across `src`: matches only in `tests.rs`, `master.rs:593/606`, `replica.rs:399/413`). `master.replicate()` is likewise test-only. The write handlers never feed the replication log.
- **Failure scenario:** start with `--role master --replica-listen ...` → the flag is accepted, but no replica listener is spawned and no write is ever replicated; a "replica" only ever gets the one-shot snapshot at best.
- **vs competitor:** Redis/Kafka replication is a first-class, always-on path.
- **Fix:** instantiate and store the master/replica node in `AppState`, spawn its listener from `main.rs`, and call `replicate()` from every write handler (see also M-010).

### M-006 — RESP3 parser allocates attacker-controlled sizes with no cap (OOM DoS)
- **Subsystem:** network · **Evidence:** `protocol/resp3/parser.rs:140` `read_bulk_bytes(reader, len as usize)` → `parser.rs:104` `let mut data = vec![0u8; len];` with `len` parsed from the client (`$<len>`), no maximum. Arrays pre-allocate on a client count: `parser.rs:150` `Vec::with_capacity(count as usize)`. `read_line` (`parser.rs:89-98`) is also unbounded.
- **Failure scenario:** one connection sends `$2000000000\r\n` or `*1000000000\r\n` → multi-GB allocation → OOM kill.
- **vs competitor:** Redis caps bulk length (`proto-max-bulk-len`, default 512 MB) and rejects oversized multibulk.
- **Fix:** enforce a configurable max bulk length and max array/element count; reject frames above it before allocating.

### M-007 — SynapRPC frame reader allocates up to 4 GB from a 4-byte length prefix (OOM DoS)
- **Subsystem:** network · **Evidence:** `protocol/synap_rpc/codec.rs:76-80` reads a 4-byte LE length then `let mut body = vec![0u8; len];` with no ceiling before `read_exact`.
- **Failure scenario:** a client sends `\xff\xff\xff\xff…` → immediate ~4 GB allocation per connection.
- **vs competitor:** every production binary framing (Kafka, gRPC) caps frame size.
- **Fix:** add a max-frame-size guard in `read_frame` and drop the connection when exceeded.

## High

### M-008 — MULTI/EXEC is documented atomic but executes without holding any lock
- **Subsystem:** transactions · **Evidence:** module doc claims "Atomic execution with sorted multi-key locking" (`core/transaction.rs:3-9`), but `execute_transaction` computes `keys_to_lock` then immediately calls `execute_commands` with the honest comment `core/transaction.rs:350-351` "For simplicity, we'll use a single lock on all keys / In production, you'd use sorted locks per key". `execute_commands` (`transaction.rs:403+`) invokes each store op independently, each taking and releasing its own per-shard lock.
- **Failure scenario:** two clients run overlapping EXECs → their commands interleave; a WATCH-based check-and-set is not isolated (version bump happens after execution, `transaction.rs:360-368`), so the guarantee Redis users rely on is violated.
- **vs competitor:** Redis EXEC runs the whole block with no interleaving.
- **Fix:** acquire the sorted per-key locks for the duration of `execute_commands`, or route EXEC through a single serialized executor.

### M-009 — Passwords hashed with unsalted single-round SHA-512 and compared non-constant-time
- **Subsystem:** security · **Evidence:** `auth/user.rs:54-58` `Sha512::new(); update(password); hex::encode(finalize())` — no salt, no iterations. `auth/user.rs:61-64` compares with `hashed == self.password_hash` (short-circuiting `==`, timing-observable). `bcrypt` is already a workspace dependency (`Cargo.toml:33`) but unused here.
- **Failure scenario:** a leaked user table is trivially reversed via rainbow tables; identical passwords collide; login timing leaks prefix matches.
- **vs competitor:** Redis 7 stores SHA-256 but over a random-generated secret; RabbitMQ uses salted hashing. Password auth should use argon2/bcrypt.
- **Fix:** switch to bcrypt/argon2 with per-user salt; compare with a constant-time equality.

### M-010 — Transaction writes bypass the WAL and replication path
- **Subsystem:** durability · **Evidence:** `execute_commands` calls `self.kv_store.set(...)`, `self.hash_store.hset(...)` etc. directly (`core/transaction.rs:411-449`). WAL logging lives in the persistence layer invoked by the HTTP handlers (`persistence/layer.rs:46-59`), not inside the core stores, and transactions do not call it.
- **Failure scenario:** an EXEC that sets 100 keys is acknowledged, the process crashes, and none of it is in the WAL → committed transaction lost. The same writes are also never replicated.
- **vs competitor:** Redis writes the whole MULTI/EXEC to the AOF and to replicas.
- **Fix:** log every transaction command to WAL (and replication) as part of commit, ideally as one atomic WAL batch.

### M-011 — Pub/Sub uses an unbounded per-subscriber channel (slow-consumer OOM)
- **Subsystem:** pubsub · **Evidence:** `core/pubsub.rs:21` `pub type MessageSender = mpsc::UnboundedSender<Message>;`; publish pushes to it non-blocking (`pubsub.rs:441-442`). A subscriber that stops reading never applies backpressure.
- **Failure scenario:** one slow/stuck WebSocket client on a high-rate topic makes its channel grow without limit until the server OOMs — a single client takes down the process.
- **vs competitor:** Redis has `client-output-buffer-limit` for pub/sub; NATS evicts slow consumers.
- **Fix:** bounded channel with a drop-or-disconnect policy and a per-subscriber buffer-limit metric.

### M-012 — Streams are RAM-only and silently drop unread events on overflow
- **Subsystem:** streams · **Evidence:** `core/stream.rs:110` room buffer is a `VecDeque`; on overflow `publish` pops the front regardless of consumer progress (`stream.rs:157-161`), and time compaction drops by timestamp (`stream.rs:206-227`). There is no disk segment/spill; retention is not tied to committed consumer offsets.
- **Failure scenario:** a producer outruns a consumer past `max_buffer_size` → the oldest unread events are discarded with no error → consumer silently misses data.
- **vs competitor:** Kafka persists to segmented logs on disk; retention is size/time based and independent of RAM; consumers can always replay within the retention window.
- **Fix:** spill to disk segments (or bound retention by min committed offset), and surface an explicit drop/lag signal.

### M-013 — Queues have no per-consumer prefetch/QoS and fake the consumer count
- **Subsystem:** queues · **Evidence:** `core/queue.rs:201-224` `consume` just pops one message; there is no consumer registry, prefetch window, or fair dispatch. `queue.rs:217` `self.stats.consumers = 1; // Simplified for now` hardcodes the count.
- **Failure scenario:** multiple consumers on a queue cannot be balanced or rate-limited; monitoring always reports 1 consumer regardless of reality.
- **vs competitor:** RabbitMQ's per-consumer prefetch (QoS) and round-robin fair dispatch are core features.
- **Fix:** model consumers explicitly with a prefetch limit and fair round-robin delivery; report the real count.

## Medium

### M-014 — Stream operations are logged to the WAL but their replay is a no-op
- **Subsystem:** persistence · **Evidence:** `persistence/layer.rs:408-431` logs `StreamPublish` to the WAL, but `recovery.rs:193-202` explicitly does nothing on replay ("Stream operations are not replayed from WAL … here to prevent compilation errors"). Streams rely on a separate `StreamPersistence`.
- **Impact:** WAL space is spent on entries that are never used; two persistence paths for streams risk divergence. Either stop logging streams to the WAL or actually replay them.

### M-015 — No maximum-connections / accept-rate limit on the listeners
- **Subsystem:** network · **Evidence:** RESP3/SynapRPC accept loops spawn a task per connection with no cap (`protocol/resp3/server.rs`, `protocol/synap_rpc/server.rs` accept paths); no `maxclients` equivalent in config.
- **Impact:** connection-flood exhausts FDs/memory. Redis has `maxclients`. Add a semaphore-bounded accept limit + idle-timeout.

### M-016 — Stream consume is an O(n) linear scan of the whole buffer
- **Subsystem:** streams · **Evidence:** `core/stream.rs:169-177` filters the entire `VecDeque` by `offset >= from_offset` on every consume. With a large buffer and many consumers this is O(buffer × consumers) per poll.
- **Impact:** consume latency grows with backlog. Index by offset (offset − min_offset is the VecDeque index since offsets are contiguous) for O(1) seek.

### M-017 — ACK-deadline sweep locks every queue globally once per second
- **Subsystem:** queues · **Evidence:** `core/queue.rs:307-322` takes a single `queues.write()` and iterates all queues calling `check_expired_pending`, which itself scans all pending entries (`queue.rs:274-286`) — every second, under one global write lock.
- **Impact:** with many queues / large pending sets this stalls all queue ops periodically. Use a per-queue timer wheel or a min-heap of deadlines instead of a full scan.

### M-018 — Collection memory is not counted toward maxmemory, and GET copies the full value
- **Subsystem:** KV memory · **Evidence:** memory accounting/eviction is KV-only — `total_memory_bytes` is maintained in `kv_store/store.rs` (54 hits) and eviction runs there; Hash/List/Set/ZSet/Stream/Queue sizes are not added to the budget (grep shows accounting confined to `kv_store` + one hit in `hash.rs`). Also every GET clones the whole value: `store.rs:454,492` `value.data().to_vec()`.
- **Impact:** `maxmemory` can be blown far past the limit by collections/brokers (eviction only sheds KV strings); large-value reads double memory traffic. Count all datatypes toward the budget; consider `Arc<[u8]>`/`Bytes` to return values without a full copy.

---

## Materialized rulebook tasks

| Task | Findings | Priority |
|------|----------|----------|
| `phase6a_v1-durability-integrity` | M-001, M-002, M-014 | critical |
| `phase6b_v1-protocol-auth-hardening` | M-003, M-004, M-009 | critical |
| `phase6c_v1-network-dos-limits` | M-006, M-007, M-011, M-015 | critical |
| `phase6d_v1-transaction-atomicity` | M-008, M-010 | high |
| `phase6e_v1-replication-wiring` | M-005 | critical |
| `phase6f_v1-broker-parity` | M-012, M-013, M-016, M-017 | high/medium |
| `phase6g_v1-memory-accounting` | M-018 | medium |

All seven run in phase 6 (after the crates/ restructure and stability hardening, before the
phase 7 Redis benchmark) so correctness/security/durability are fixed before performance is
measured or the 1.0 tag is cut.

## Notes on scope

- Findings deliberately exclude anything already in `phase1..phase8` or already shipped
  (RESP3 dispatch, SynapRPC, eviction policies, GET AtomicU32 fast path, WAL group commit,
  snapshot streaming format, metrics). See `docs/analysis/synap-v1-release/`.
- Several criticals (M-003, M-004, M-005) suggest security and replication should each get a
  dedicated hardening task before the 1.0 tag, ahead of the performance/benchmark phase.
