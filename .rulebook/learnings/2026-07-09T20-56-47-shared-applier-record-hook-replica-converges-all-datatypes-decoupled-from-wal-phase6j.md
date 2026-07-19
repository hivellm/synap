# Shared applier + record hook: replica converges all datatypes, decoupled from WAL (phase6j)
**Source**: manual
**Date**: 2026-07-09
**Related Task**: phase6j_v1-replica-datatype-apply-and-decouple
**Tags**: replication, persistence, phase6j, analysis:synap-audit, M-005, appstate, rust
Problem (audit M-005 follow-up): the replica's apply_operation only handled KV + stream, so hash/list/set/sorted-set/queue writes reached the replica but were dropped — the replica silently diverged. Propagation also piggybacked on the WAL (needed persistence.enabled), and INFO reported hardcoded replication status.

Solution patterns:
1. Single shared applier. Extracted `persistence::apply::apply_operation(op, kv, hash?, list?, set?, sorted_set?, queue?, stream?)` used by BOTH WAL recovery (stream=None — StreamPersistence owns streams) and ReplicaNode (stream=Some). One match over all ~40 Operation variants → the two paths structurally cannot diverge. recovery.rs shrank ~300 lines; replica gained full-datatype convergence.
2. Decouple replication from WAL via a `record()` hook. PersistenceLayer.wal became Option<Arc<AsyncWAL>>, opened only when `config.enabled && config.wal.enabled`. Every log_* delegates to `record(op)` which ALWAYS maybe_replicate() then appends to WAL only if present. main.rs builds the layer when `persistence.enabled || replication_master.is_some()`, so a master replicates even with persistence off and a replication-only layer creates no WAL file.
3. Live INFO status. Added `ReplicationHandle` enum (Master(Arc<MasterNode>)|Replica(Arc<ReplicaNode>)) to AppState; ReplicationInfo::collect(handle) reads master.list_replicas()/replication_offset() or replica.stats() instead of placeholders.

Gotchas:
- ReplicaNode::new signature order is (config, kv, stream, hash, list, set, sorted_set, queue) — stream stays 3rd (historical), the 5 new stores append after. 11 call sites needed updating (src + tests + benches).
- main.rs ordering: the replica must be constructed AFTER hash/list/set/sorted-set stores exist, so its creation moved out of the early replication block to after the store-init block; the master arm stays early (only needs kv+stream).
- Adding a required AppState field broke 35 struct literals across 29 files. Deterministic fix: a guarded PowerShell pass inserting `replication: None,` only where a `require_auth:` line is the last field (next non-empty line starts with `}`) AND a backward scan confirms the enclosing literal is `AppState {` (this excluded AclRule literals in acl.rs that also end with require_auth). Preserve each file's original EOL via [System.IO.File]::WriteAllText to avoid whole-file CRLF churn.
- End-to-end convergence tests (tests/datatype_replication_tests.rs) drive a live TCP master→replica, call master.replicate(Operation::Hash/List/Set/ZAdd...), and poll the replica store with a 10s deadline (propagation is fire-and-forget over a heartbeat channel — fixed sleeps are racy on CI).