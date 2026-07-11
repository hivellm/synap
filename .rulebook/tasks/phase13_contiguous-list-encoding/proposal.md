# Proposal: phase13_contiguous-list-encoding

> **Scope extension (2026-07-11):** the same small-collection encoding applies to
> SETs — the multi-key SADD gap (0.66 after ahash) is the per-new-key
> `HashSet`+`SetValue` allocation vs Redis's ~20-byte listpack. Add a small-set
> contiguous encoding alongside the list one, same thresholds/upgrade model.

Source: docs/analysis/redis-parity-deep-dive.md (item J, plan round 6)

## Why

RPUSH is the weakest remaining op vs Redis (~0.74 after phase12). Synap stores
lists as `VecDeque<Vec<u8>>` — **one heap allocation per element**, elements
scattered across the heap. Redis (`t_list.c`) stores small lists as
**listpacks**: elements packed into one contiguous byte blob inside a quicklist,
so a push is an append into a hot buffer with no per-element allocation and
excellent cache locality. The per-element alloc + pointer-chasing is the
structural reason Synap's list writes trail while its LPUSH wins are
client-quirk-driven.

## What Changes

1. Add a contiguous small-list encoding to `ListValue`: elements stored in one
   growable byte buffer with length-prefixed entries (listpack-style), used
   while the list stays under thresholds (e.g. ≤128 elements and ≤64B per
   element — mirror Redis's `list-max-listpack-size` defaults).
2. Automatic upgrade to the current `VecDeque<Vec<u8>>` representation when a
   threshold is crossed (equivalent of listpack → quicklist conversion);
   downgrade is not required (Redis doesn't either).
3. All list ops (push/pop/range/index/set/trim/rem/insert) handle both
   encodings; conversions covered by tests including boundary sizes.
4. Serde/persistence: the on-disk snapshot format for lists must round-trip
   both encodings (serialize as the logical element sequence, encoding is a
   runtime detail).
5. Re-run the sweep: target RPUSH/LPOP/RPOP ≥ 0.9 of Redis.

## Impact

- Affected specs: none (list command semantics unchanged)
- Affected code: crates/synap-core/src/core/list.rs (ListValue + all ops),
  persistence snapshot round-trip for lists
- Breaking change: NO (snapshots serialize the logical sequence)
- User benefit: list write throughput near Redis parity; lower memory per small
  list (no per-element Vec header/capacity overhead)
