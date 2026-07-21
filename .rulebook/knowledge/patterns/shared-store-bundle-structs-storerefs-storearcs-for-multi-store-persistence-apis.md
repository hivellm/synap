# Shared store-bundle structs (StoreRefs/StoreArcs) for multi-store persistence APIs

**Category**: rust
**Tags**: clippy, too_many_arguments, parameter-struct, persistence

## Description

When several functions thread the same bundle of stores (kv + optional hash/list/set/zset/queue/stream), define one borrowed struct (StoreRefs<'a>, Copy, all fields pub) and one owned Arc counterpart (StoreArcs with as_refs()/kv_only() helpers) in persistence/apply.rs, re-exported from the persistence mod. apply_operation, maybe_snapshot, create_snapshot and ReplicaNode::new all take the same bundle, so the applier and the snapshot path can never disagree on which stores exist, and call sites use named-field construction instead of 7 positional Option arguments. kv_only() keeps test/bench call sites one-liners.

## When to Use

Any new persistence/replication routine that needs the datatype stores; any function accumulating 7+ positional parameters that form a coherent bundle.

## When NOT to Use

Functions taking 2-3 unrelated parameters — a struct adds indirection without safety gain.
