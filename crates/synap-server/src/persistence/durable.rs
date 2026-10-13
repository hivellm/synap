//! The shared mutation path: every write, on every transport, goes through here.
//!
//! Each function applies one command to its store and then records the
//! command's *effect* through the [`PersistenceLayer`] (replicas first, then
//! the WAL) before the caller acknowledges it. REST, the envelope, RESP3,
//! SynapRPC, MCP and `redis.call` all call these functions instead of the
//! stores' mutators, so the same command produces the same [`Operation`]
//! whatever transport carried it, and no transport can forget to persist.
//!
//! Rules (see the phase21 design notes):
//!
//! - **Resulting effects for derived writes.** INCR, APPEND, SETRANGE,
//!   HINCRBY, ZINCRBY, SPOP, ZPOP* and friends are recorded as what the key
//!   holds afterwards (value plus remaining expiry, removed members, ...),
//!   read by the store under the same lock as the mutation — the model
//!   transactions already use (`CommittedWrite`).
//! - **Only real changes are recorded.** A refused NX/XX SET or an SREM that
//!   removed nothing logs nothing.
//! - **A logging failure fails the command.** The effect is already applied
//!   in memory and is not rolled back; the error says so
//!   ([`write_not_durable_message`]).
//! - **Free when persistence is off.** With no layer each function is the
//!   plain store call — no clone, no `Operation`, no allocation.

use super::layer::{PersistenceLayer, write_not_durable_message};
use super::types::{BitfieldWrite, Operation};
use crate::core::bitmap::{BitfieldOp, BitfieldResult};
use crate::core::sorted_set::{SortedSetStore, ZAddOptions};
use crate::core::{
    Aggregate, BitfieldOperation, BitmapOperation, BitmapStore, Expiry, GeospatialStore, HashStore,
    HyperLogLogStore, KVStore, KeyManager, ListStore, QueueManager, QueueMessage, ScoredMember,
    SetOptions, SetResult, SetStore, SynapError, coordinate_to_score, ttl_secs_from_deadline,
};
use std::collections::HashMap;
use std::sync::Arc;

/// The persistence layer a write is recorded through; `None` when neither a
/// WAL nor a replication master is configured.
pub type Log<'a> = Option<&'a PersistenceLayer>;

type Result<T> = std::result::Result<T, SynapError>;

/// Record one effect, turning a logging failure into the command's error.
async fn record(layer: &PersistenceLayer, op: Operation) -> Result<()> {
    layer
        .log_operation(op)
        .await
        .map_err(|e| SynapError::InternalError(write_not_durable_message(&e)))
}

/// Record the effects of one command that produced several operations.
async fn record_all(layer: &PersistenceLayer, ops: Vec<Operation>) -> Result<()> {
    layer
        .log_operations(ops)
        .await
        .map_err(|e| SynapError::InternalError(write_not_durable_message(&e)))
}

// ── Strings / keys ──────────────────────────────────────────────────────────

/// SET with an optional relative TTL. Keeps the zero-copy `Arc<[u8]>` value
/// path; the value is copied into the record only when a layer exists.
pub async fn kv_set(
    log: Log<'_>,
    kv: &KVStore,
    key: impl Into<String>,
    value: impl Into<Arc<[u8]>>,
    ttl: Option<u64>,
) -> Result<()> {
    let Some(layer) = log else {
        return kv.set(key, value, ttl).await;
    };
    let key: String = key.into();
    let value: Arc<[u8]> = value.into();
    kv.set(key.clone(), Arc::clone(&value), ttl).await?;
    record(
        layer,
        Operation::KVSet {
            key,
            value: value.to_vec(),
            ttl,
        },
    )
    .await
}

/// SET with NX/XX/KEEPTTL/GET options. Records the written value with the
/// expiry the key ended up with (KEEPTTL carries the old deadline forward).
pub async fn kv_set_with_opts(
    log: Log<'_>,
    kv: &KVStore,
    key: &str,
    value: Vec<u8>,
    expiry: Option<Expiry>,
    opts: SetOptions,
) -> Result<SetResult> {
    let Some(layer) = log else {
        return kv.set_with_opts(key, value, expiry, opts).await;
    };
    let logged = value.clone();
    let result = kv.set_with_opts(key, value, expiry, opts).await?;
    if result.written {
        record(
            layer,
            Operation::KVSet {
                key: key.to_owned(),
                value: logged,
                ttl: ttl_secs_from_deadline(result.expires_at_ms),
            },
        )
        .await?;
    }
    Ok(result)
}

/// DEL of one key.
pub async fn kv_delete(log: Log<'_>, kv: &KVStore, key: &str) -> Result<bool> {
    let deleted = kv.delete(key).await?;
    if deleted && let Some(layer) = log {
        record(
            layer,
            Operation::KVDel {
                keys: vec![key.to_owned()],
            },
        )
        .await?;
    }
    Ok(deleted)
}

/// DEL of several keys; returns how many existed.
pub async fn kv_mdel(log: Log<'_>, kv: &KVStore, keys: &[String]) -> Result<usize> {
    let deleted = kv.mdel(keys).await?;
    if deleted > 0
        && let Some(layer) = log
    {
        record(
            layer,
            Operation::KVDel {
                keys: keys.to_vec(),
            },
        )
        .await?;
    }
    Ok(deleted)
}

/// INCRBY (negative `amount` decrements). Recorded as the resulting value and
/// the expiry the counter still carries.
pub async fn kv_incr(log: Log<'_>, kv: &KVStore, key: &str, amount: i64) -> Result<i64> {
    let Some(layer) = log else {
        return kv.incr(key, amount).await;
    };
    let (value, expires_at_ms) = kv.incr_with_expiry(key, amount).await?;
    record(
        layer,
        Operation::KVSet {
            key: key.to_owned(),
            value: value.to_string().into_bytes(),
            ttl: ttl_secs_from_deadline(expires_at_ms),
        },
    )
    .await?;
    Ok(value)
}

/// DECRBY, with the same arithmetic as [`KVStore::decr`].
pub async fn kv_decr(log: Log<'_>, kv: &KVStore, key: &str, amount: i64) -> Result<i64> {
    kv_incr(log, kv, key, -amount).await
}

/// MSET: every pair becomes a persistent key.
pub async fn kv_mset(log: Log<'_>, kv: &KVStore, pairs: Vec<(String, Vec<u8>)>) -> Result<()> {
    let Some(layer) = log else {
        return kv.mset(pairs).await;
    };
    let ops = set_ops(&pairs);
    kv.mset(pairs).await?;
    record_all(layer, ops).await
}

/// MSETNX: sets every pair only when none of the keys exists.
pub async fn kv_msetnx(log: Log<'_>, kv: &KVStore, pairs: Vec<(String, Vec<u8>)>) -> Result<bool> {
    let Some(layer) = log else {
        return kv.msetnx(pairs).await;
    };
    let ops = set_ops(&pairs);
    let written = kv.msetnx(pairs).await?;
    if written {
        record_all(layer, ops).await?;
    }
    Ok(written)
}

fn set_ops(pairs: &[(String, Vec<u8>)]) -> Vec<Operation> {
    pairs
        .iter()
        .map(|(key, value)| Operation::KVSet {
            key: key.clone(),
            value: value.clone(),
            ttl: None,
        })
        .collect()
}

/// GETSET: the new value replaces the old one without a TTL.
pub async fn kv_getset(
    log: Log<'_>,
    kv: &KVStore,
    key: &str,
    value: Vec<u8>,
) -> Result<Option<Vec<u8>>> {
    let Some(layer) = log else {
        return kv.getset(key, value).await;
    };
    let logged = value.clone();
    let old = kv.getset(key, value).await?;
    record(
        layer,
        Operation::KVSet {
            key: key.to_owned(),
            value: logged,
            ttl: None,
        },
    )
    .await?;
    Ok(old)
}

/// APPEND, recorded as the whole resulting value and its remaining expiry.
pub async fn kv_append(log: Log<'_>, kv: &KVStore, key: &str, value: Vec<u8>) -> Result<usize> {
    let Some(layer) = log else {
        return kv.append(key, value).await;
    };
    let written = kv.append_returning(key, value).await?;
    record(
        layer,
        Operation::KVSet {
            key: key.to_owned(),
            value: written.value.to_vec(),
            ttl: ttl_secs_from_deadline(written.expires_at_ms),
        },
    )
    .await?;
    Ok(written.len)
}

/// SETRANGE, recorded as the whole resulting value and its remaining expiry.
pub async fn kv_setrange(
    log: Log<'_>,
    kv: &KVStore,
    key: &str,
    offset: usize,
    value: Vec<u8>,
) -> Result<usize> {
    let Some(layer) = log else {
        return kv.setrange(key, offset, value).await;
    };
    let written = kv.setrange_returning(key, offset, value).await?;
    record(
        layer,
        Operation::KVSet {
            key: key.to_owned(),
            value: written.value.to_vec(),
            ttl: ttl_secs_from_deadline(written.expires_at_ms),
        },
    )
    .await?;
    Ok(written.len)
}

/// EXPIRE in seconds.
pub async fn kv_expire(log: Log<'_>, kv: &KVStore, key: &str, secs: u64) -> Result<bool> {
    let changed = kv.expire(key, secs).await?;
    if changed && let Some(layer) = log {
        record(
            layer,
            Operation::KVExpire {
                key: key.to_owned(),
                ttl: Some(secs),
            },
        )
        .await?;
    }
    Ok(changed)
}

/// PERSIST: drop the key's expiry.
pub async fn kv_persist(log: Log<'_>, kv: &KVStore, key: &str) -> Result<bool> {
    let changed = kv.persist(key).await?;
    if changed && let Some(layer) = log {
        record(
            layer,
            Operation::KVExpire {
                key: key.to_owned(),
                ttl: None,
            },
        )
        .await?;
    }
    Ok(changed)
}

/// FLUSHDB (refused unless `allow_flush_commands`).
pub async fn kv_flushdb(log: Log<'_>, kv: &KVStore) -> Result<usize> {
    let removed = kv.flushdb().await?;
    if let Some(layer) = log {
        record(layer, Operation::KVFlush).await?;
    }
    Ok(removed)
}

/// FLUSHALL (refused unless `allow_flush_commands`).
pub async fn kv_flushall(log: Log<'_>, kv: &KVStore) -> Result<usize> {
    let removed = kv.flushall().await?;
    if let Some(layer) = log {
        record(layer, Operation::KVFlush).await?;
    }
    Ok(removed)
}

/// RENAME of a key of any type.
pub async fn key_rename(
    log: Log<'_>,
    keys: &KeyManager,
    source: &str,
    destination: &str,
) -> Result<()> {
    keys.rename(source, destination).await?;
    if source != destination
        && let Some(layer) = log
    {
        record(
            layer,
            Operation::KVRename {
                source: source.to_owned(),
                destination: destination.to_owned(),
            },
        )
        .await?;
    }
    Ok(())
}

/// RENAMENX: renames only when the destination does not exist.
pub async fn key_renamenx(
    log: Log<'_>,
    keys: &KeyManager,
    source: &str,
    destination: &str,
) -> Result<bool> {
    let renamed = keys.renamenx(source, destination).await?;
    if renamed
        && source != destination
        && let Some(layer) = log
    {
        record(
            layer,
            Operation::KVRename {
                source: source.to_owned(),
                destination: destination.to_owned(),
            },
        )
        .await?;
    }
    Ok(renamed)
}

/// COPY of a key of any type.
pub async fn key_copy(
    log: Log<'_>,
    keys: &KeyManager,
    source: &str,
    destination: &str,
    replace: bool,
) -> Result<bool> {
    let copied = keys.copy(source, destination, replace).await?;
    if copied
        && source != destination
        && let Some(layer) = log
    {
        record(
            layer,
            Operation::KeyCopy {
                source: source.to_owned(),
                destination: destination.to_owned(),
                replace,
            },
        )
        .await?;
    }
    Ok(copied)
}

// ── Hashes ──────────────────────────────────────────────────────────────────

/// HSET of one field; returns whether the field was new.
pub async fn hset(
    log: Log<'_>,
    hs: &HashStore,
    key: &str,
    field: &str,
    value: Vec<u8>,
) -> Result<bool> {
    let Some(layer) = log else {
        return hs.hset(key, field, value);
    };
    let created = hs.hset(key, field, value.clone())?;
    record(layer, hash_set_op(key, field, value)).await?;
    Ok(created)
}

/// HMSET of several fields.
pub async fn hmset(
    log: Log<'_>,
    hs: &HashStore,
    key: &str,
    fields: HashMap<String, Vec<u8>>,
) -> Result<()> {
    let Some(layer) = log else {
        return hs.hmset(key, fields);
    };
    let ops = fields
        .iter()
        .map(|(field, value)| hash_set_op(key, field, value.clone()))
        .collect();
    hs.hmset(key, fields)?;
    record_all(layer, ops).await
}

/// HSETNX: sets the field only when it does not exist.
pub async fn hsetnx(
    log: Log<'_>,
    hs: &HashStore,
    key: &str,
    field: &str,
    value: Vec<u8>,
) -> Result<bool> {
    let Some(layer) = log else {
        return hs.hsetnx(key, field, value);
    };
    let set = hs.hsetnx(key, field, value.clone())?;
    if set {
        record(layer, hash_set_op(key, field, value)).await?;
    }
    Ok(set)
}

/// HDEL; returns how many fields were removed.
pub async fn hdel(log: Log<'_>, hs: &HashStore, key: &str, fields: &[String]) -> Result<usize> {
    let removed = hs.hdel(key, fields)?;
    if removed > 0
        && let Some(layer) = log
    {
        record(
            layer,
            Operation::HashDel {
                key: key.to_owned(),
                fields: fields.to_vec(),
            },
        )
        .await?;
    }
    Ok(removed)
}

/// HINCRBY, recorded as the field's resulting value.
pub async fn hincrby(
    log: Log<'_>,
    hs: &HashStore,
    key: &str,
    field: &str,
    increment: i64,
) -> Result<i64> {
    let value = hs.hincrby(key, field, increment)?;
    if let Some(layer) = log {
        // The store renders the field exactly like this.
        record(
            layer,
            hash_set_op(key, field, value.to_string().into_bytes()),
        )
        .await?;
    }
    Ok(value)
}

/// HINCRBYFLOAT, recorded as the field's resulting value.
pub async fn hincrbyfloat(
    log: Log<'_>,
    hs: &HashStore,
    key: &str,
    field: &str,
    increment: f64,
) -> Result<f64> {
    let value = hs.hincrbyfloat(key, field, increment)?;
    if let Some(layer) = log {
        record(
            layer,
            hash_set_op(key, field, value.to_string().into_bytes()),
        )
        .await?;
    }
    Ok(value)
}

fn hash_set_op(key: &str, field: &str, value: Vec<u8>) -> Operation {
    Operation::HashSet {
        key: key.to_owned(),
        field: field.to_owned(),
        value,
    }
}

// ── Lists ───────────────────────────────────────────────────────────────────

/// LPUSH / LPUSHX (`only_if_exists`); returns the new length.
pub async fn lpush(
    log: Log<'_>,
    ls: &ListStore,
    key: &str,
    values: Vec<Vec<u8>>,
    only_if_exists: bool,
) -> Result<usize> {
    push(log, ls, key, values, only_if_exists, true).await
}

/// RPUSH / RPUSHX (`only_if_exists`); returns the new length.
pub async fn rpush(
    log: Log<'_>,
    ls: &ListStore,
    key: &str,
    values: Vec<Vec<u8>>,
    only_if_exists: bool,
) -> Result<usize> {
    push(log, ls, key, values, only_if_exists, false).await
}

async fn push(
    log: Log<'_>,
    ls: &ListStore,
    key: &str,
    values: Vec<Vec<u8>>,
    only_if_exists: bool,
    left: bool,
) -> Result<usize> {
    let apply = |values| {
        if left {
            ls.lpush(key, values, only_if_exists)
        } else {
            ls.rpush(key, values, only_if_exists)
        }
    };
    let Some(layer) = log else {
        return apply(values);
    };
    let logged = values.clone();
    let len = apply(values)?;
    if len > 0 && !logged.is_empty() {
        record(
            layer,
            Operation::ListPush {
                key: key.to_owned(),
                values: logged,
                left,
            },
        )
        .await?;
    }
    Ok(len)
}

/// LPOP; returns the popped values.
pub async fn lpop(
    log: Log<'_>,
    ls: &ListStore,
    key: &str,
    count: Option<usize>,
) -> Result<Vec<Vec<u8>>> {
    let popped = ls.lpop(key, count)?;
    record_pop(log, key, popped.len(), true).await?;
    Ok(popped)
}

/// RPOP; returns the popped values.
pub async fn rpop(
    log: Log<'_>,
    ls: &ListStore,
    key: &str,
    count: Option<usize>,
) -> Result<Vec<Vec<u8>>> {
    let popped = ls.rpop(key, count)?;
    record_pop(log, key, popped.len(), false).await?;
    Ok(popped)
}

/// BLPOP; returns the key popped from and the value.
pub async fn blpop(
    log: Log<'_>,
    ls: &ListStore,
    keys: Vec<String>,
    timeout_secs: Option<u64>,
) -> Result<(String, Vec<u8>)> {
    let (key, value) = ls.blpop(keys, timeout_secs).await?;
    record_pop(log, &key, 1, true).await?;
    Ok((key, value))
}

/// BRPOP; returns the key popped from and the value.
pub async fn brpop(
    log: Log<'_>,
    ls: &ListStore,
    keys: Vec<String>,
    timeout_secs: Option<u64>,
) -> Result<(String, Vec<u8>)> {
    let (key, value) = ls.brpop(keys, timeout_secs).await?;
    record_pop(log, &key, 1, false).await?;
    Ok((key, value))
}

async fn record_pop(log: Log<'_>, key: &str, count: usize, left: bool) -> Result<()> {
    match log {
        Some(layer) if count > 0 => {
            record(
                layer,
                Operation::ListPop {
                    key: key.to_owned(),
                    count,
                    left,
                },
            )
            .await
        }
        _ => Ok(()),
    }
}

/// RPOPLPUSH; returns the moved value.
pub async fn rpoplpush(
    log: Log<'_>,
    ls: &ListStore,
    source: &str,
    destination: &str,
) -> Result<Vec<u8>> {
    let value = ls.rpoplpush(source, destination)?;
    record_rpoplpush(log, source, destination).await?;
    Ok(value)
}

/// BRPOPLPUSH; returns the moved value.
pub async fn brpoplpush(
    log: Log<'_>,
    ls: &ListStore,
    source: &str,
    destination: &str,
    timeout_secs: Option<u64>,
) -> Result<Vec<u8>> {
    let value = ls.brpoplpush(source, destination, timeout_secs).await?;
    record_rpoplpush(log, source, destination).await?;
    Ok(value)
}

async fn record_rpoplpush(log: Log<'_>, source: &str, destination: &str) -> Result<()> {
    match log {
        Some(layer) => {
            record(
                layer,
                Operation::ListRpoplpush {
                    source: source.to_owned(),
                    destination: destination.to_owned(),
                },
            )
            .await
        }
        None => Ok(()),
    }
}

/// LSET.
pub async fn lset(
    log: Log<'_>,
    ls: &ListStore,
    key: &str,
    index: i64,
    value: Vec<u8>,
) -> Result<()> {
    let Some(layer) = log else {
        return ls.lset(key, index, value);
    };
    ls.lset(key, index, value.clone())?;
    record(
        layer,
        Operation::ListSet {
            key: key.to_owned(),
            index,
            value,
        },
    )
    .await
}

/// LTRIM.
pub async fn ltrim(log: Log<'_>, ls: &ListStore, key: &str, start: i64, stop: i64) -> Result<()> {
    ls.ltrim(key, start, stop)?;
    match log {
        Some(layer) => {
            record(
                layer,
                Operation::ListTrim {
                    key: key.to_owned(),
                    start,
                    stop,
                },
            )
            .await
        }
        None => Ok(()),
    }
}

/// LREM; returns how many elements were removed.
pub async fn lrem(
    log: Log<'_>,
    ls: &ListStore,
    key: &str,
    count: i64,
    value: Vec<u8>,
) -> Result<usize> {
    let Some(layer) = log else {
        return ls.lrem(key, count, value);
    };
    let removed = ls.lrem(key, count, value.clone())?;
    if removed > 0 {
        record(
            layer,
            Operation::ListRem {
                key: key.to_owned(),
                count,
                value,
            },
        )
        .await?;
    }
    Ok(removed)
}

/// LINSERT; returns the list's length afterwards.
pub async fn linsert(
    log: Log<'_>,
    ls: &ListStore,
    key: &str,
    before: bool,
    pivot: Vec<u8>,
    value: Vec<u8>,
) -> Result<usize> {
    let Some(layer) = log else {
        return ls.linsert(key, before, pivot, value);
    };
    let len = ls.linsert(key, before, pivot.clone(), value.clone())?;
    record(
        layer,
        Operation::ListInsert {
            key: key.to_owned(),
            before,
            pivot,
            value,
        },
    )
    .await?;
    Ok(len)
}

// ── Sets ────────────────────────────────────────────────────────────────────

/// SADD; returns how many members were new.
pub async fn sadd(log: Log<'_>, ss: &SetStore, key: &str, members: Vec<Vec<u8>>) -> Result<usize> {
    let Some(layer) = log else {
        return ss.sadd(key, members);
    };
    let added = ss.sadd(key, members.clone())?;
    if added > 0 {
        record(
            layer,
            Operation::SetAdd {
                key: key.to_owned(),
                members,
            },
        )
        .await?;
    }
    Ok(added)
}

/// SREM; returns how many members were removed.
pub async fn srem(log: Log<'_>, ss: &SetStore, key: &str, members: Vec<Vec<u8>>) -> Result<usize> {
    let Some(layer) = log else {
        return ss.srem(key, members);
    };
    let removed = ss.srem(key, members.clone())?;
    if removed > 0 {
        record(
            layer,
            Operation::SetRem {
                key: key.to_owned(),
                members,
            },
        )
        .await?;
    }
    Ok(removed)
}

/// SPOP, recorded as the removal of the members actually popped.
pub async fn spop(
    log: Log<'_>,
    ss: &SetStore,
    key: &str,
    count: Option<usize>,
) -> Result<Vec<Vec<u8>>> {
    let popped = ss.spop(key, count)?;
    if !popped.is_empty()
        && let Some(layer) = log
    {
        record(
            layer,
            Operation::SetRem {
                key: key.to_owned(),
                members: popped.clone(),
            },
        )
        .await?;
    }
    Ok(popped)
}

/// SMOVE; returns whether the member moved.
pub async fn smove(
    log: Log<'_>,
    ss: &SetStore,
    source: &str,
    destination: &str,
    member: Vec<u8>,
) -> Result<bool> {
    let Some(layer) = log else {
        return ss.smove(source, destination, member);
    };
    let moved = ss.smove(source, destination, member.clone())?;
    if moved {
        record(
            layer,
            Operation::SetMove {
                source: source.to_owned(),
                destination: destination.to_owned(),
                member,
            },
        )
        .await?;
    }
    Ok(moved)
}

/// SINTERSTORE; returns the destination's size.
pub async fn sinterstore(
    log: Log<'_>,
    ss: &SetStore,
    destination: &str,
    keys: &[String],
) -> Result<usize> {
    let size = ss.sinterstore(destination, keys)?;
    if let Some(layer) = log {
        record(
            layer,
            Operation::SetInterStore {
                destination: destination.to_owned(),
                keys: keys.to_vec(),
            },
        )
        .await?;
    }
    Ok(size)
}

/// SUNIONSTORE; returns the destination's size.
pub async fn sunionstore(
    log: Log<'_>,
    ss: &SetStore,
    destination: &str,
    keys: &[String],
) -> Result<usize> {
    let size = ss.sunionstore(destination, keys)?;
    if let Some(layer) = log {
        record(
            layer,
            Operation::SetUnionStore {
                destination: destination.to_owned(),
                keys: keys.to_vec(),
            },
        )
        .await?;
    }
    Ok(size)
}

/// SDIFFSTORE; returns the destination's size.
pub async fn sdiffstore(
    log: Log<'_>,
    ss: &SetStore,
    destination: &str,
    keys: &[String],
) -> Result<usize> {
    let size = ss.sdiffstore(destination, keys)?;
    if let Some(layer) = log {
        record(
            layer,
            Operation::SetDiffStore {
                destination: destination.to_owned(),
                keys: keys.to_vec(),
            },
        )
        .await?;
    }
    Ok(size)
}

// ── Sorted sets (and geospatial, which stores in sorted sets) ──────────────

/// ZADD of one member; returns `(added, changed)` like the store. Recorded
/// with its NX/XX/GT/LT conditions, which replay to the same result.
pub async fn zadd(
    log: Log<'_>,
    zs: &SortedSetStore,
    key: &str,
    member: Vec<u8>,
    score: f64,
    opts: &ZAddOptions,
) -> Result<(usize, usize)> {
    let Some(layer) = log else {
        return Ok(zs.zadd(key, member, score, opts));
    };
    let op = zadd_op(key, member.clone(), score, opts);
    let result = zs.zadd(key, member, score, opts);
    record(layer, op).await?;
    Ok(result)
}

fn zadd_op(key: &str, member: Vec<u8>, score: f64, opts: &ZAddOptions) -> Operation {
    Operation::ZAdd {
        key: key.to_owned(),
        member,
        score,
        nx: opts.nx,
        xx: opts.xx,
        gt: opts.gt,
        lt: opts.lt,
    }
}

/// ZREM; returns how many members were removed.
pub async fn zrem(
    log: Log<'_>,
    zs: &SortedSetStore,
    key: &str,
    members: &[Vec<u8>],
) -> Result<usize> {
    let removed = zs.zrem(key, members);
    if removed > 0
        && let Some(layer) = log
    {
        record(
            layer,
            Operation::ZRem {
                key: key.to_owned(),
                members: members.to_vec(),
            },
        )
        .await?;
    }
    Ok(removed)
}

/// ZINCRBY, recorded as the member's resulting score.
pub async fn zincrby(
    log: Log<'_>,
    zs: &SortedSetStore,
    key: &str,
    member: Vec<u8>,
    increment: f64,
) -> Result<f64> {
    let Some(layer) = log else {
        return Ok(zs.zincrby(key, member, increment));
    };
    let score = zs.zincrby(key, member.clone(), increment);
    record(layer, zadd_op(key, member, score, &ZAddOptions::default())).await?;
    Ok(score)
}

/// ZPOPMIN, recorded as the removal of the members popped.
pub async fn zpopmin(
    log: Log<'_>,
    zs: &SortedSetStore,
    key: &str,
    count: usize,
) -> Result<Vec<ScoredMember>> {
    let popped = zs.zpopmin(key, count);
    record_zpop(log, key, &popped).await?;
    Ok(popped)
}

/// ZPOPMAX, recorded as the removal of the members popped.
pub async fn zpopmax(
    log: Log<'_>,
    zs: &SortedSetStore,
    key: &str,
    count: usize,
) -> Result<Vec<ScoredMember>> {
    let popped = zs.zpopmax(key, count);
    record_zpop(log, key, &popped).await?;
    Ok(popped)
}

async fn record_zpop(log: Log<'_>, key: &str, popped: &[ScoredMember]) -> Result<()> {
    match log {
        Some(layer) if !popped.is_empty() => {
            record(
                layer,
                Operation::ZRem {
                    key: key.to_owned(),
                    members: popped.iter().map(|m| m.member.clone()).collect(),
                },
            )
            .await
        }
        _ => Ok(()),
    }
}

/// BZPOPMIN; returns `(key, member, score)`.
pub async fn bzpopmin(
    log: Log<'_>,
    zs: &SortedSetStore,
    keys: Vec<String>,
    timeout_secs: Option<u64>,
) -> Result<(String, Vec<u8>, f64)> {
    let popped = zs.bzpopmin(keys, timeout_secs).await?;
    record_bzpop(log, &popped).await?;
    Ok(popped)
}

/// BZPOPMAX; returns `(key, member, score)`.
pub async fn bzpopmax(
    log: Log<'_>,
    zs: &SortedSetStore,
    keys: Vec<String>,
    timeout_secs: Option<u64>,
) -> Result<(String, Vec<u8>, f64)> {
    let popped = zs.bzpopmax(keys, timeout_secs).await?;
    record_bzpop(log, &popped).await?;
    Ok(popped)
}

async fn record_bzpop(log: Log<'_>, popped: &(String, Vec<u8>, f64)) -> Result<()> {
    match log {
        Some(layer) => {
            record(
                layer,
                Operation::ZRem {
                    key: popped.0.clone(),
                    members: vec![popped.1.clone()],
                },
            )
            .await
        }
        None => Ok(()),
    }
}

/// ZREMRANGEBYRANK; returns how many members were removed.
pub async fn zremrangebyrank(
    log: Log<'_>,
    zs: &SortedSetStore,
    key: &str,
    start: i64,
    stop: i64,
) -> Result<usize> {
    let removed = zs.zremrangebyrank(key, start, stop);
    if removed > 0
        && let Some(layer) = log
    {
        record(
            layer,
            Operation::ZRemRangeByRank {
                key: key.to_owned(),
                start,
                stop,
            },
        )
        .await?;
    }
    Ok(removed)
}

/// ZREMRANGEBYSCORE; returns how many members were removed.
pub async fn zremrangebyscore(
    log: Log<'_>,
    zs: &SortedSetStore,
    key: &str,
    min: f64,
    max: f64,
) -> Result<usize> {
    let removed = zs.zremrangebyscore(key, min, max);
    if removed > 0
        && let Some(layer) = log
    {
        record(
            layer,
            Operation::ZRemRangeByScore {
                key: key.to_owned(),
                min,
                max,
            },
        )
        .await?;
    }
    Ok(removed)
}

fn aggregate_name(aggregate: Aggregate) -> String {
    match aggregate {
        Aggregate::Sum => "sum",
        Aggregate::Min => "min",
        Aggregate::Max => "max",
    }
    .to_owned()
}

/// ZINTERSTORE; returns the destination's size.
pub async fn zinterstore(
    log: Log<'_>,
    zs: &SortedSetStore,
    destination: &str,
    keys: &[&str],
    weights: Option<&[f64]>,
    aggregate: Aggregate,
) -> Result<usize> {
    let size = zs.zinterstore(destination, keys, weights, aggregate);
    if let Some(layer) = log {
        record(
            layer,
            Operation::ZInterStore {
                destination: destination.to_owned(),
                keys: keys.iter().map(|k| (*k).to_owned()).collect(),
                weights: weights.map(<[f64]>::to_vec),
                aggregate: aggregate_name(aggregate),
            },
        )
        .await?;
    }
    Ok(size)
}

/// ZUNIONSTORE; returns the destination's size.
pub async fn zunionstore(
    log: Log<'_>,
    zs: &SortedSetStore,
    destination: &str,
    keys: &[&str],
    weights: Option<&[f64]>,
    aggregate: Aggregate,
) -> Result<usize> {
    let size = zs.zunionstore(destination, keys, weights, aggregate);
    if let Some(layer) = log {
        record(
            layer,
            Operation::ZUnionStore {
                destination: destination.to_owned(),
                keys: keys.iter().map(|k| (*k).to_owned()).collect(),
                weights: weights.map(<[f64]>::to_vec),
                aggregate: aggregate_name(aggregate),
            },
        )
        .await?;
    }
    Ok(size)
}

/// ZDIFFSTORE; returns the destination's size.
pub async fn zdiffstore(
    log: Log<'_>,
    zs: &SortedSetStore,
    destination: &str,
    keys: &[&str],
) -> Result<usize> {
    let size = zs.zdiffstore(destination, keys);
    if let Some(layer) = log {
        record(
            layer,
            Operation::ZDiffStore {
                destination: destination.to_owned(),
                keys: keys.iter().map(|k| (*k).to_owned()).collect(),
            },
        )
        .await?;
    }
    Ok(size)
}

/// GEOADD, recorded as the sorted-set ZADDs it performs (member, geohash
/// score, NX/XX), so replay and replicas rebuild the same sorted set.
pub async fn geoadd(
    log: Log<'_>,
    geo: &GeospatialStore,
    key: &str,
    locations: Vec<(f64, f64, Vec<u8>)>,
    nx: bool,
    xx: bool,
    ch: bool,
) -> Result<usize> {
    let Some(layer) = log else {
        return geo.geoadd(key, locations, nx, xx, ch);
    };
    let opts = ZAddOptions {
        nx,
        xx,
        ..ZAddOptions::default()
    };
    let ops = locations
        .iter()
        .map(|(lat, lon, member)| {
            zadd_op(key, member.clone(), coordinate_to_score(*lat, *lon), &opts)
        })
        .collect();
    let added = geo.geoadd(key, locations, nx, xx, ch)?;
    record_all(layer, ops).await?;
    Ok(added)
}

// ── HyperLogLog ─────────────────────────────────────────────────────────────

/// PFADD; returns how many registers changed.
pub async fn pfadd(
    log: Log<'_>,
    hll: &HyperLogLogStore,
    key: &str,
    elements: Vec<Vec<u8>>,
    ttl: Option<u64>,
) -> Result<usize> {
    let Some(layer) = log else {
        return hll.pfadd(key, elements, ttl);
    };
    let added = hll.pfadd(key, elements.clone(), ttl)?;
    record(
        layer,
        Operation::HllAdd {
            key: key.to_owned(),
            elements,
            ttl,
        },
    )
    .await?;
    Ok(added)
}

/// PFMERGE; returns the destination's estimated cardinality.
pub async fn pfmerge(
    log: Log<'_>,
    hll: &HyperLogLogStore,
    destination: &str,
    sources: Vec<String>,
) -> Result<u64> {
    let Some(layer) = log else {
        return hll.pfmerge(destination, sources);
    };
    let count = hll.pfmerge(destination, sources.clone())?;
    record(
        layer,
        Operation::HllMerge {
            destination: destination.to_owned(),
            sources,
        },
    )
    .await?;
    Ok(count)
}

// ── Bitmaps ─────────────────────────────────────────────────────────────────

/// SETBIT; returns the previous bit.
pub async fn setbit(
    log: Log<'_>,
    bm: &BitmapStore,
    key: &str,
    offset: usize,
    value: u8,
) -> Result<u8> {
    let old = bm.setbit(key, offset, value)?;
    if let Some(layer) = log {
        record(
            layer,
            Operation::BitmapSetBit {
                key: key.to_owned(),
                offset: offset as u64,
                value,
            },
        )
        .await?;
    }
    Ok(old)
}

/// BITOP; returns the destination's length in bytes.
pub async fn bitop(
    log: Log<'_>,
    bm: &BitmapStore,
    operation: BitmapOperation,
    destination: &str,
    sources: &[String],
) -> Result<usize> {
    let len = bm.bitop(operation, destination, sources)?;
    if let Some(layer) = log {
        let name = match operation {
            BitmapOperation::And => "AND",
            BitmapOperation::Or => "OR",
            BitmapOperation::Xor => "XOR",
            BitmapOperation::Not => "NOT",
        };
        record(
            layer,
            Operation::BitmapOp {
                operation: name.to_owned(),
                destination: destination.to_owned(),
                sources: sources.to_vec(),
            },
        )
        .await?;
    }
    Ok(len)
}

/// BITFIELD. Its SET and INCRBY sub-operations are recorded as the value each
/// written field holds afterwards (a SET stores the requested value, an
/// INCRBY reports the value it left after overflow handling).
pub async fn bitfield(
    log: Log<'_>,
    bm: &BitmapStore,
    key: &str,
    operations: &[BitfieldOperation],
) -> Result<Vec<BitfieldResult>> {
    let results = bm.bitfield(key, operations)?;
    if let Some(layer) = log {
        let writes: Vec<BitfieldWrite> = operations
            .iter()
            .zip(&results)
            .filter_map(|(op, result)| match (op, result.operation) {
                (
                    BitfieldOperation::Set {
                        offset,
                        width,
                        signed,
                        value,
                    },
                    BitfieldOp::Set,
                ) => Some(field_write(*offset, *width, *signed, *value)),
                (
                    BitfieldOperation::IncrBy {
                        offset,
                        width,
                        signed,
                        ..
                    },
                    BitfieldOp::IncrBy,
                ) => Some(field_write(*offset, *width, *signed, result.value)),
                _ => None,
            })
            .collect();
        if !writes.is_empty() {
            record(
                layer,
                Operation::BitmapField {
                    key: key.to_owned(),
                    writes,
                },
            )
            .await?;
        }
    }
    Ok(results)
}

fn field_write(offset: usize, width: usize, signed: bool, value: i64) -> BitfieldWrite {
    BitfieldWrite {
        offset: offset as u64,
        width: width as u64,
        signed,
        value,
    }
}

// ── Queues ──────────────────────────────────────────────────────────────────

/// Queue PUBLISH; returns the published message.
pub async fn queue_publish(
    log: Log<'_>,
    qm: &QueueManager,
    queue: &str,
    payload: Vec<u8>,
    priority: Option<u8>,
    max_retries: Option<u32>,
) -> Result<QueueMessage> {
    let message = qm
        .publish_with_message(queue, payload, priority, max_retries)
        .await?;
    if let Some(layer) = log {
        record(
            layer,
            Operation::QueuePublish {
                queue: queue.to_owned(),
                message: message.clone(),
            },
        )
        .await?;
    }
    Ok(message)
}

/// Queue ACK.
pub async fn queue_ack(
    log: Log<'_>,
    qm: &QueueManager,
    queue: &str,
    message_id: &str,
) -> Result<()> {
    qm.ack(queue, message_id).await?;
    if let Some(layer) = log {
        record(
            layer,
            Operation::QueueAck {
                queue: queue.to_owned(),
                message_id: message_id.to_owned(),
            },
        )
        .await?;
    }
    Ok(())
}

/// Queue NACK.
pub async fn queue_nack(
    log: Log<'_>,
    qm: &QueueManager,
    queue: &str,
    message_id: &str,
    requeue: bool,
) -> Result<()> {
    qm.nack(queue, message_id, requeue).await?;
    if let Some(layer) = log {
        record(
            layer,
            Operation::QueueNack {
                queue: queue.to_owned(),
                message_id: message_id.to_owned(),
                requeue,
            },
        )
        .await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::KVConfig;
    use crate::persistence::layer::test_support::faulty_layer;
    use crate::persistence::recover_all;

    /// A write that cannot be logged fails with the not-durable message, and
    /// the in-memory effect stays (it is not rolled back).
    #[tokio::test]
    async fn a_logging_failure_fails_the_write() {
        let (layer, faults) = faulty_layer("durable_write_fails").await;
        let kv = KVStore::new(KVConfig::default());
        faults.fail_next_flush();
        let err = kv_set(Some(&layer), &kv, "k", b"v".to_vec(), None)
            .await
            .unwrap_err();
        assert!(err.to_string().contains("not durable"), "{err}");
        assert_eq!(kv.get("k").await.unwrap(), Some(b"v".to_vec()));
        // The WAL is poisoned now: every later write fails as well.
        let hs = HashStore::new();
        assert!(
            hset(Some(&layer), &hs, "h", "f", b"v".to_vec())
                .await
                .is_err()
        );
    }

    /// APPEND, SETRANGE and INCR are logged as the value the key ends up
    /// with and the expiry it still carries, so a WAL replay rebuilds them.
    #[tokio::test]
    async fn derived_writes_replay_as_their_resulting_value() {
        let dir = "./target/durable_derived_writes";
        let _ = std::fs::remove_dir_all(dir);
        let mut config = crate::persistence::PersistenceConfig::default();
        config.wal.fsync_mode = crate::persistence::types::FsyncMode::Always;
        config.wal.path = std::path::PathBuf::from(format!("{dir}/wal.log"));
        config.snapshot.enabled = false;
        config.snapshot.directory = std::path::PathBuf::from(format!("{dir}/snap"));
        let layer = PersistenceLayer::new(config.clone()).await.unwrap();
        let kv = KVStore::new(KVConfig::default());
        let log = Some(&layer);

        kv_set(log, &kv, "a", b"ab".to_vec(), Some(300))
            .await
            .unwrap();
        assert_eq!(kv_append(log, &kv, "a", b"cd".to_vec()).await.unwrap(), 4);
        assert_eq!(
            kv_setrange(log, &kv, "pad", 3, b"x".to_vec())
                .await
                .unwrap(),
            4
        );
        kv_set(log, &kv, "n", b"1".to_vec(), Some(300))
            .await
            .unwrap();
        assert_eq!(kv_incr(log, &kv, "n", 4).await.unwrap(), 5);
        layer.shutdown().await.unwrap();

        let r = recover_all(
            &config,
            KVConfig::default(),
            crate::core::QueueConfig::default(),
        )
        .await
        .unwrap();
        assert_eq!(r.kv_store.get("a").await.unwrap(), Some(b"abcd".to_vec()));
        assert!(matches!(r.kv_store.ttl("a").await, Ok(Some(t)) if t > 0));
        assert_eq!(
            r.kv_store.get("pad").await.unwrap(),
            Some(b"\0\0\0x".to_vec())
        );
        assert_eq!(r.kv_store.get("n").await.unwrap(), Some(b"5".to_vec()));
        assert!(matches!(r.kv_store.ttl("n").await, Ok(Some(t)) if t > 0));
        let _ = std::fs::remove_dir_all(dir);
    }
}
