# Design: phase21_durable-writes-on-every-transport

## 1. Inventory before this change (item 1.1)

How each mutating command was persisted (WAL + replication both hang off
`PersistenceLayer`, so "not logged" means "lost on restart and never sent to
replicas"). Read from the dispatchers at the base commit (2a3a9df).

Legend: **L** logged correctly, **L!** logged but wrong, **E** logged but the
logging error was swallowed (client told "OK"), **-** not logged at all,
**n/a** command not offered on that transport.

### Strings / keys

| Command | REST | Envelope | RESP3 | SynapRPC | MCP | Script (`redis.call`) |
|---|---|---|---|---|---|---|
| SET (+EX/PX/KEEPTTL/NX/XX) | L! E — TTL always logged as `None` (expiry was already converted to an absolute deadline), Always mode logs before the NX/XX check | L E | - | - | L E | - |
| DEL / MDEL | L E | L E | - | - | L E | - |
| INCR / INCRBY / DECR / DECRBY | n/a | L! E — resulting value logged without the key's TTL | - | - | n/a | - |
| MSET | n/a | L E | - | - | n/a | n/a |
| MSETNX | L E | L E | - | - | n/a | n/a |
| GETSET | L E | L E | - | - | n/a | n/a |
| APPEND | L! E — empty value, no TTL (D-08) | L! E — empty value (D-08) | - | - | L! E — the operand, not the result | n/a |
| SETRANGE | L! E — empty value, no TTL (D-08) | L! E — empty value (D-08) | - | - | n/a | n/a |
| EXPIRE / PERSIST | n/a | - | - | - | n/a | - |
| FLUSHDB / FLUSHALL | n/a | - | - (RESP3 also replied OK when the flush was refused) | - | n/a | n/a |
| RENAME | L! E — `KVRename` replays only KV keys | L! E (same) | n/a | n/a | - | n/a |
| RENAMENX / COPY | - | - | n/a | n/a | n/a | n/a |

### Collections

| Command | REST | Envelope | RESP3 | SynapRPC | MCP | Script |
|---|---|---|---|---|---|---|
| HSET / HMSET / HSETNX | - | - | - | - | - | - |
| HDEL | - | - | - | - | n/a | - |
| HINCRBY / HINCRBYFLOAT | - | - | - | - | n/a | - |
| LPUSH / RPUSH / LPUSHX / RPUSHX | - | - | - | - | - | - |
| LPOP / RPOP / BLPOP / BRPOP | - | - | - | - | - | - |
| LSET / LTRIM / LREM / LINSERT / RPOPLPUSH / BRPOPLPUSH | - | - | - (BRPOPLPUSH) | - (BRPOPLPUSH) | n/a | n/a |
| SADD / SREM / SPOP / SMOVE | - | - | - | - | - (SADD) | - (SREM) |
| ZADD / ZREM / ZINCRBY | - | - | - | - | - (ZADD) | - |
| ZPOPMIN / ZPOPMAX / BZPOPMIN / BZPOPMAX | - | - | - (BZPOP*) | - (BZPOP*) | n/a | - (ZPOP*) |
| ZREMRANGEBYRANK / ZREMRANGEBYSCORE | - | - | n/a | n/a | n/a | - |
| ZINTERSTORE / ZUNIONSTORE / ZDIFFSTORE | - | - | n/a | n/a | n/a | n/a |
| GEOADD (stored in sorted sets) | - | - | - | - | n/a | n/a |
| PFADD / PFMERGE | - (no `Operation`) | - | - | - | n/a | n/a |
| SETBIT / BITOP / BITFIELD | - (no `Operation`) | - | - (SETBIT) | - (SETBIT) | n/a | n/a |

### Brokers and transactions

| Command | REST | Envelope | RESP3 | SynapRPC | MCP |
|---|---|---|---|---|---|
| QPUBLISH / QACK / QNACK | L E | L E | - | - | - (publish) |
| QCREATE / QDELETE / QPURGE | - | - | - | - | n/a |
| Stream publish / create / delete | not in the KV WAL by design (audit M-014: `StreamPersistence` and the v3 snapshot section own streams) | same | same | same | n/a |
| Pub/Sub publish, Kafka-style partitions, consumer groups | not persisted state | same | same | same | n/a |
| EXEC | L (fails the command, phase20) | L | L | L | L |

UMICP has no dispatcher of its own: it converts the envelope into an MCP tool
call (`server/umicp/handlers.rs`), so it inherits the MCP column.

Two more gaps found while reading the recovery path:

- `main.rs` discarded the recovered set store (`_set_store_recovered`) and
  started with an empty one, so even a correctly logged `SADD` was lost on
  restart.
- HyperLogLog and bitmap stores were never handed to recovery or to the
  replica applier.

## 2. The shared mutation path (item 1.2)

`persistence::durable` holds one function per persisted effect. Each one
applies the mutation to the store and then records the resulting `Operation`
through `PersistenceLayer` (replication first, then the WAL), returning an
error when recording fails. Every transport calls these functions instead of
the stores' mutators:

```text
REST / envelope / RESP3 / SynapRPC / MCP / redis.call
        │  (parse args, authorize, scope keys, render reply — per transport)
        ▼
persistence::durable::<command>(log, &store, ...)   apply in memory, read the effect
        │
        ▼
PersistenceLayer::log_operation / log_operations   replicate + WAL append
```

Each function takes the optional layer (`durable::Log<'_>`, i.e.
`state.persistence.as_deref()`) and the store it mutates, and mirrors that
store method's signature, so a call site changes from
`state.hash_store.hset(k, f, v)` to
`durable::hset(state.persistence.as_deref(), &state.hash_store, k, f, v).await`.
Scripts pass the layer from their `ScriptExecContext`, which now carries it,
and the queue WebSocket passes the `AppState` it was opened with.

Rules the functions follow:

- **Same effect on every transport.** A command produces the same
  `Operation` whatever carried it.
- **Resulting effects for derived writes** (the transaction model,
  `CommittedWrite`): INCR/DECR log the resulting value plus the expiry the
  counter still has; APPEND/SETRANGE log the whole resulting value plus expiry,
  returned by `KVStore::append_returning` / `setrange_returning` from inside
  the shard write lock; HINCRBY/HINCRBYFLOAT log the resulting field value;
  ZINCRBY logs the resulting score; SPOP/ZPOPMIN/ZPOPMAX/BZPOP* log the members
  actually removed; SET with KEEPTTL logs the deadline it carried forward
  (`SetResult::expires_at_ms`); GEOADD logs the sorted-set scores it wrote.
- **Only real changes are logged**: a refused NX/XX SET, an LPUSHX on a missing
  key, an SREM that removed nothing, etc. record nothing.
- **A logging failure fails the command.** The effect is already in memory and
  is not rolled back, so the error says so
  (`layer::write_not_durable_message`, same wording style as phase20's
  `exec_not_durable_message`). This closes phase20's known gap where
  single-write handlers only logged the WAL error.
- **No cost when persistence is off.** With `AppState.persistence == None`
  each function is the plain store call: no clone, no `Operation`, no extra
  allocation. RESP3/SynapRPC `SET` keep their zero-copy `Arc<[u8]>` path; the
  value is copied into the `Operation` only when a layer exists.
- **Multi-op commands** (MSET, MSETNX, HMSET, multi-member ZADD, GEOADD) are
  recorded with `log_operations`, i.e. one WAL batch.

### New `Operation` variants

Appended at the end of the enum so WAL files and replication streams written
by older builds still decode (bincode encodes the variant index):

| Variant | Recorded by | Applied as |
|---|---|---|
| `KVExpire { key, ttl }` | EXPIRE (`Some(secs)`), PERSIST (`None`) | `expire` / `persist` |
| `KVFlush` | FLUSHDB / FLUSHALL | `KVStore::clear` (no config gate: the master already accepted it) |
| `KeyCopy { source, destination, replace }` | COPY | copy across KV/hash/list/set/sorted set |
| `HllAdd { key, elements, ttl }` | PFADD | `pfadd` |
| `HllMerge { destination, sources }` | PFMERGE | `pfmerge` |
| `BitmapSetBit { key, offset, value }` | SETBIT | `setbit` |
| `BitmapOp { operation, destination, sources }` | BITOP | `bitop` |
| `BitmapField { key, writes }` | BITFIELD SET/INCRBY | `bitfield` SET of each resulting field value |

`KVRename` keeps its encoding but its applier now moves the key in whichever
store holds it (KV, hash, list, set, sorted set), so a renamed hash or list
survives replay; old WALs replay the same way for KV keys.

`StoreRefs`/`StoreArcs` gain `hyperloglog_store` and `bitmap_store`;
`recover_all` returns every store (including the two new ones) and `recover`
keeps its tuple for existing callers.

## 3. What is deliberately out of scope

- **Same-key ordering between concurrent writers** — phase23. The shared path
  makes it easier: there is now one place per command where the WAL sequence
  can be taken under the shard lock.
- **Snapshots of HyperLogLog and bitmap data** — the v3 snapshot has no section
  for them, and the snapshot format is phase23's (v4). Until then their state
  is durable through the WAL tail only; after a snapshot, recovery skips the
  WAL entries before the snapshot's offset, so phase23's v4 must add both
  sections.
- **Absolute expiry** — the WAL keeps relative TTL seconds (phase23 D-07).
- **Queue identity on replay** — phase24. QCREATE/QDELETE/QPURGE stay
  unlogged: there is no queue-lifecycle record and phase24 redesigns queue
  replay; QPUBLISH/QACK/QNACK are routed through the shared path so the native
  transports match REST.
- **Streams** stay in `StreamPersistence` (audit M-014).
