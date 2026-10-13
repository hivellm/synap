# Durability review for the next release

Review date: 2026-10-12. Revalidated against current working tree, including the externally prepared 1.3.2 dependency/version changes. Those changes were preserved.

Method: static analysis of current source, call paths, and transaction documentation. No reproduction, fault-injection test, Cargo command, or benchmark was executed for this report. Acceptance tests below are proposed, not completed. Findings concern current code, not already-fixed omissions from the earlier audit.

## D-01 - P1 - Idle and shutdown paths leave acknowledged WAL data buffered

- Evidence: `crates/synap-server/src/persistence/wal_async.rs:135` receives requests; `crates/synap-server/src/persistence/wal_async.rs:148` enables the sleep branch only while the batch is nonempty; the batch is drained before the next select. `crates/synap-server/src/persistence/wal_async.rs:199` checks the periodic deadline only while processing writes. `crates/synap-server/src/persistence/wal_async.rs:233` exits without a final flush. `crates/synap-server/src/persistence/layer.rs:600` implements `flush()` as a no-op. Defaults at `crates/synap-server/src/persistence/types.rs:313` use a 64 KiB buffer, Periodic mode, and a 1,000 ms interval.
- Scenario: acknowledge a small write before the first fsync deadline, then leave the server idle. No later request triggers a flush. Closing the channel drops the buffered writer without explicitly draining it.
- Impact: acknowledged writes can remain outside the OS indefinitely, exceeding the configured periodic loss window. An orderly layer drop is not itself a durability barrier. Never mode also leaves bytes in the application buffer rather than merely omitting OS fsync.
- Recommendation: add an independent interval, an explicit flush request/barrier, and an awaited shutdown/drain path. Flush application buffers in Never mode while retaining its no-fsync semantics.
- Acceptance: append one small entry, wait beyond the configured interval without new traffic, and recover it from disk. Repeat with orderly shutdown before the interval. Verify Never mode writes bytes without filling the buffer and does not call fsync.

## D-02 - P1 - Flush/fsync failures still produce successful commit replies

- Evidence: `crates/synap-server/src/persistence/wal_async.rs:208` handles group flush; `crates/synap-server/src/persistence/wal_async.rs:210` and `crates/synap-server/src/persistence/wal_async.rs:213` only warn on failure. `crates/synap-server/src/persistence/wal_async.rs:223` sends the prior per-entry result unchanged. Separately, `crates/synap-server/src/server/handlers/kv.rs:1171`, `crates/synap-server/src/server/handlers/admin_cmd.rs:280`, and `crates/synap-server/src/server/mcp_handlers.rs:969` swallow transaction logging errors.
- Scenario: use Always mode and encounter a full device or an injected flush/sync failure. Buffer writes succeeded, so acknowledgements still report success. EXEC handlers also return success when the persistence hook returns an error.
- Impact: clients cannot distinguish durable commits from failed persistence. This contradicts the fsynced-before-success contract in `docs/features/transactions.md:37`.
- Recommendation: fail every affected pending acknowledgement after a group flush/sync error; define a poisoned-writer/recovery policy. Propagate commit errors through each protocol. Explicitly handle the already-mutated memory state rather than implying that an error reply rolls it back.
- Acceptance: inject flush and sync failures independently. Neither single-write nor EXEC paths may return success. Verify subsequent writes cannot silently continue against an invalid WAL and document the resulting in-memory state.

## D-03 - P1 - Transaction WAL batches lack a crash-atomic record boundary

- Evidence: `crates/synap-server/src/persistence/wal_async.rs:182` writes each batch operation separately. `crates/synap-server/src/persistence/wal_async.rs:255` serializes each as an ordinary WALEntry with its own length/checksum. `crates/synap-server/src/persistence/wal.rs:158` stops at incomplete data, while `crates/synap-server/src/persistence/wal.rs:184` accepts complete preceding entries. No transaction envelope or commit marker distinguishes a complete transaction from its prefix. `docs/features/transactions.md:33` explicitly promises crash all-or-nothing behavior.
- Scenario: a crash or failed write leaves the first entry of a two-operation EXEC complete and the second incomplete, before successful batch acknowledgement. Recovery accepts the first effect by itself.
- Impact: recovery can expose a partial transaction. Contiguous writes prevent interleaving but do not provide atomic recovery. This does not claim that a successfully fsynced complete batch normally loses an entry; the gap is handling incomplete batches. Replication at `crates/synap-server/src/persistence/layer.rs:107` also sends separate operations without transaction grouping.
- Recommendation: introduce a versioned atomic batch record or transaction framing with a durable commit marker. Replay must discard incomplete groups. Define and test transaction boundaries for replication rather than assuming contiguous sends provide them.
- Acceptance: truncate a multi-operation transaction WAL at every byte boundary and recover either all effects or none. Include a large transaction exceeding the writer buffer and injected failure between operations. Test replica disconnect during a transaction against the chosen replication contract.

## D-04 - P1 - Snapshot data is not tied to its recorded WAL cutoff

- Evidence: `crates/synap-server/src/persistence/layer.rs:399` samples the next WAL offset before calling snapshot creation at `crates/synap-server/src/persistence/layer.rs:402`. Live data is read later: KV at `crates/synap-server/src/persistence/snapshot.rs:74`, hash at `crates/synap-server/src/persistence/snapshot.rs:198`, and list at `crates/synap-server/src/persistence/snapshot.rs:205`. `crates/synap-server/src/persistence/recovery.rs:137` replays from that cutoff, inclusively through `crates/synap-server/src/persistence/wal.rs:184`.
- Scenario: snapshot captures cutoff N; a concurrent LPUSH or HINCRBY is applied and logged at offset N or later before that collection is dumped. The snapshot contains its effect, and recovery applies its WAL entry again.
- Impact: duplicated list elements or counter increments after restart. Sequential store/shard dumps can also produce a mixed-time image. The concrete finding needs only concurrent non-idempotent operations; it does not assume stronger cross-store isolation than documented. Other interleavings depend on each handler's memory/WAL order and cannot be repaired by merely moving the offset read after the dumps.
- Recommendation: establish a coherent state image paired with its committed WAL position, using a shared commit/snapshot barrier or versioned immutable state. Coordinate memory application and WAL sequence assignment. Keep the critical section bounded by capturing a stable image before serializing it.
- Acceptance: pause snapshot creation immediately after cutoff capture, complete a logged LPUSH/HINCRBY, resume the dump, then recover. State must match exactly, with no duplicate effect. Exercise opposite ordering, multi-shard data, and transactions separately.

## D-05 - P1 - Queue replay changes message identity and cannot honor persisted ACKs

- Evidence: `crates/synap-server/src/persistence/apply.rs:131` replays QueuePublish through ordinary `publish`; `crates/synap-core/src/core/queue/manager.rs:136` constructs a fresh QueueMessage, whose ID is generated at `crates/synap-core/src/core/queue/mod.rs:72`. `crates/synap-server/src/persistence/apply.rs:144` replays ACK using the original ID and ignores errors. `crates/synap-core/src/core/queue/mod.rs:273` removes ACKed messages only from pending state. Snapshot restoration also republishes messages at `crates/synap-server/src/persistence/recovery.rs:63`.
- Scenario: publish, consume, ACK, then recover from WAL. Replay creates a new ID in ready state; the persisted ACK cannot remove it. Restoring a snapshot followed by a later ACK has the same identity/state problem.
- Impact: acknowledged messages become deliverable again, and message metadata/identity changes on recovery. Replication uses the shared apply path, so mirrored IDs diverge and ACK application is also affected.
- Recommendation: add restoration primitives preserving message IDs and required metadata. Apply durable acknowledgements/tombstones against the reconstructed message regardless of ready/pending state, or persist enough lifecycle state to make replay equivalent. Do not hide unexpected replay failures.
- Acceptance: WAL-only and snapshot-plus-WAL recovery after ACK must leave no deliverable copy. An unacknowledged restored message retains its ID. Master and replica IDs and queue contents remain equivalent through publish/consume/ACK.

## D-06 - P1 - A tolerated torn WAL tail remains in front of later writes

- Evidence: `crates/synap-server/src/persistence/wal_async.rs:47` opens in append mode. `crates/synap-server/src/persistence/wal_async.rs:98` logs that an incomplete entry is being truncated but only breaks the scan. The synchronous scanner has the same behavior at `crates/synap-server/src/persistence/wal.rs:71`. Neither path truncates to the last valid byte position before appending.
- Scenario: a crash leaves a partial frame. Startup tolerates the tail; new acknowledged records are appended after it. On the next startup, parsing the original partial frame consumes bytes from the appended records or stops before reaching them.
- Impact: later successful writes become unrecoverable, or recovery fails checksum/deserialization checks. Tail-tolerant startup therefore does not establish a safe append point.
- Recommendation: validate the existing prefix, retain its final valid byte position, and truncate a confirmed torn tail before accepting writes. Distinguish recoverable tail damage from interior corruption; validate checksums and bound frame lengths during scanning.
- Acceptance: construct partial length, checksum, and payload tails. Reopen, append a successful record, reopen again, and recover both the original valid prefix and the new record. Interior corruption must fail explicitly rather than silently discard history.

## D-07 - P1 - Restart loses or renews key expiry

- Evidence: `crates/synap-core/src/core/kv_store/store.rs:1493` dumps key/value bytes without expiry. `crates/synap-server/src/persistence/recovery.rs:56` restores snapshot entries using `None` TTL. For WAL, `crates/synap-server/src/persistence/apply.rs:112` passes the original relative TTL to SET; `crates/synap-server/src/persistence/recovery.rs:140` passes only the operation to replay, ignoring the WALEntry timestamp recorded at `crates/synap-server/src/persistence/wal_async.rs:246`.
- Scenario: snapshot a key with a finite lifetime and restart: it becomes permanent. Without a snapshot, replay renews the original TTL from restart time, including keys whose lifetime elapsed during downtime.
- Impact: stale cache/session/coordination data survives beyond its configured expiration. Snapshot and WAL recovery disagree with uninterrupted execution.
- Recommendation: persist absolute expiry in a versioned format, preserve it through snapshot and replication, and omit expired entries during recovery. Define migration behavior for legacy relative-TTL records.
- Acceptance: with a controlled clock, restart before and after expiry through snapshot-only, WAL-only, and snapshot-plus-WAL paths. Remaining lifetime must never reset, and elapsed keys must stay absent.

## D-08 - P1 - REST APPEND and SETRANGE persist empty values

- Evidence: `crates/synap-server/src/server/handlers/kv.rs:400` logs successful APPEND as `log_kv_set(scoped_key.into_owned(), vec![], None)`; SETRANGE does the same at `crates/synap-server/src/server/handlers/kv.rs:481`. The handlers modify the real value first, but never reconstruct the value their comments promise. `crates/synap-server/src/persistence/apply.rs:112` faithfully applies the empty SET during recovery/replication.
- Scenario: SET a key, then successfully call REST APPEND or SETRANGE. Recover from a WAL containing that operation, or inspect a replica receiving it.
- Impact: the resulting value is replaced by empty bytes after restart or on replicas. Existing expiry is also removed by the logged `None` TTL.
- Recommendation: durably record the actual resulting value and its expiration under the same mutation/ordering boundary, or introduce replayable APPEND/SETRANGE operations. A separate unlocked GET after mutation is insufficient under concurrent writers.
- Acceptance: REST APPEND and SETRANGE on existing and new keys preserve exact binary bytes after restart and on replicas. Include zero-padding, existing expiry, and concurrent writes to verify ordering.
