# Proposal: phase13_int-encoding-counters

Source: docs/analysis/redis-parity-deep-dive.md (item F, plan round 6)

## Why

INCR is the last KV command materially behind Redis (0.84 after phase12; was
0.51). Synap's INCR still allocates twice per op — format the new value
(`int_to_bytes` String) + `set_data` `Vec → Arc` copy — while Redis stores
numeric strings **as a `long` inside `robj->ptr`** (`object.c` int encoding):
its INCR is an integer add with **zero allocations**, formatting only on read
via `ll2string` into a stack buffer, plus shared objects for 0–9999. Matching
this requires an integer representation inside `StoredValue`.

## What Changes

1. Add an integer-encoded variant to `StoredValue` (e.g. `Int(i64)` for
   persistent numeric values; expiring counterpart or a flag on the existing
   variants — design decision during implementation).
2. INCR/DECR fast path: if the entry is int-encoded, the op is a checked add —
   no parse, no format, no alloc. SET of a short numeric string may opt into int
   encoding (like Redis `tryObjectEncoding`), or only INCR-created values are
   int-encoded (smaller blast radius) — decide by measuring.
3. Read paths (`data()`, `data_arc()`, `estimate_entry_size`, snapshots,
   replication, `get_shared`) must handle the variant: format on demand. Since
   `data()` returns `&[u8]` (a borrow), the int variant either carries a small
   inline byte cache (e.g. `[u8; 20]` + len, formatted lazily once) or read
   sites are migrated to a `Cow`-returning accessor — key design constraint,
   resolve during implementation with the inline-cache approach preferred.
4. Re-run the sweep; success = INCR ≥ 0.95 of Redis with GET on int-encoded
   values not regressing.

## Impact

- Affected specs: none (wire behaviour unchanged — INCR/GET semantics identical)
- Affected code: crates/synap-core/src/core/types.rs (StoredValue), every
  `data()`/`data_arc()` borrower (store.rs, snapshots, replication, cache),
  kv_store/store.rs incr path
- Breaking change: NO (internal representation; serialization of snapshots must
  stay compatible or be versioned — verify in 1.x persistence tests)
- User benefit: INCR/DECR at Redis parity; counters cost near-zero allocations
