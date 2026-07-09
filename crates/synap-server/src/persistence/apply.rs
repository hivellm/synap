//! Shared operation applier for WAL recovery and replica replication (phase6j).
//!
//! Both `recovery::recover` (WAL replay) and `ReplicaNode` (applying operations
//! streamed from the master) must turn an [`Operation`] into the equivalent
//! store mutation. Keeping that logic in **one** place structurally prevents the
//! two paths from diverging — previously the replica handled only KV + stream
//! operations, so a replica silently diverged for every other datatype
//! (audit M-005 completion).
//!
//! Streams are the one asymmetry: WAL recovery skips them (they have their own
//! `StreamPersistence`), while a replica must apply them from the stream. This
//! is expressed by the `stream_manager` argument — pass `None` to skip stream
//! ops (recovery), `Some(..)` to apply them (replica).

use crate::core::sorted_set::{SortedSetStore, ZAddOptions};
use crate::core::{
    Aggregate, HashStore, KVStore, ListStore, QueueManager, SetStore, StreamManager, SynapError,
};
use crate::persistence::types::Operation;

/// Apply a single [`Operation`] to the provided stores.
///
/// `kv_store` is always present; the collection/broker stores are optional (a
/// datatype that is disabled is simply skipped). Errors from KV and hash writes
/// propagate; list/set/sorted-set/queue/stream applies are best-effort
/// (idempotent replays), matching WAL recovery semantics.
#[allow(clippy::too_many_arguments)]
pub async fn apply_operation(
    op: Operation,
    kv_store: &KVStore,
    hash_store: Option<&HashStore>,
    list_store: Option<&ListStore>,
    set_store: Option<&SetStore>,
    sorted_set_store: Option<&SortedSetStore>,
    queue_manager: Option<&QueueManager>,
    stream_manager: Option<&StreamManager>,
) -> Result<(), SynapError> {
    match op {
        // ── KV ──────────────────────────────────────────────────────────────
        Operation::KVSet { key, value, ttl } => {
            kv_store.set(key, value, ttl).await?;
        }
        Operation::KVDel { keys } => {
            for key in keys {
                kv_store.delete(&key).await?;
            }
        }
        Operation::KVRename {
            source,
            destination,
        } => {
            if let Some(data) = kv_store.get(&source).await? {
                let ttl = kv_store.ttl(&source).await?;
                kv_store.set(destination, data, ttl).await?;
                kv_store.delete(&source).await?;
            }
        }

        // ── Queue ───────────────────────────────────────────────────────────
        Operation::QueuePublish { queue, message } => {
            if let Some(qm) = queue_manager
                && qm.create_queue(&queue, None).await.is_ok()
            {
                qm.publish(
                    &queue,
                    (*message.payload).clone(),
                    Some(message.priority),
                    Some(message.max_retries),
                )
                .await?;
            }
        }
        Operation::QueueAck { queue, message_id } => {
            if let Some(qm) = queue_manager {
                let _ = qm.ack(&queue, &message_id).await;
            }
        }
        Operation::QueueNack {
            queue,
            message_id,
            requeue,
        } => {
            if let Some(qm) = queue_manager {
                let _ = qm.nack(&queue, &message_id, requeue).await;
            }
        }

        // ── Stream (applied only when a stream manager is provided) ──────────
        Operation::StreamPublish {
            room,
            event_type,
            payload,
        } => {
            if let Some(sm) = stream_manager {
                // Idempotent room creation (synap#165): never errors on an
                // existing room, so replaying the same room is a no-op.
                let _ = sm.get_or_create_room(&room).await;
                let _ = sm.publish(&room, &event_type, payload).await;
            }
        }

        // ── Hash ────────────────────────────────────────────────────────────
        Operation::HashSet { key, field, value } => {
            if let Some(h) = hash_store {
                h.hset(&key, &field, value)?;
            }
        }
        Operation::HashDel { key, fields } => {
            if let Some(h) = hash_store {
                h.hdel(&key, &fields)?;
            }
        }
        Operation::HashIncrBy {
            key,
            field,
            increment,
        } => {
            if let Some(h) = hash_store {
                h.hincrby(&key, &field, increment)?;
            }
        }
        Operation::HashIncrByFloat {
            key,
            field,
            increment,
        } => {
            if let Some(h) = hash_store {
                h.hincrbyfloat(&key, &field, increment)?;
            }
        }

        // ── List ────────────────────────────────────────────────────────────
        Operation::ListPush { key, values, left } => {
            if let Some(l) = list_store {
                if left {
                    l.lpush(&key, values, false)?;
                } else {
                    l.rpush(&key, values, false)?;
                }
            }
        }
        Operation::ListPop { key, count, left } => {
            if let Some(l) = list_store {
                let _ = if left {
                    l.lpop(&key, Some(count))
                } else {
                    l.rpop(&key, Some(count))
                };
            }
        }
        Operation::ListSet { key, index, value } => {
            if let Some(l) = list_store {
                let _ = l.lset(&key, index, value);
            }
        }
        Operation::ListTrim { key, start, stop } => {
            if let Some(l) = list_store {
                let _ = l.ltrim(&key, start, stop);
            }
        }
        Operation::ListRem { key, count, value } => {
            if let Some(l) = list_store {
                let _ = l.lrem(&key, count, value);
            }
        }
        Operation::ListInsert {
            key,
            before,
            pivot,
            value,
        } => {
            if let Some(l) = list_store {
                let _ = l.linsert(&key, before, pivot, value);
            }
        }
        Operation::ListRpoplpush {
            source,
            destination,
        } => {
            if let Some(l) = list_store {
                let _ = l.rpoplpush(&source, &destination);
            }
        }

        // ── Set ─────────────────────────────────────────────────────────────
        Operation::SetAdd { key, members } => {
            if let Some(s) = set_store {
                let _ = s.sadd(&key, members);
            }
        }
        Operation::SetRem { key, members } => {
            if let Some(s) = set_store {
                let _ = s.srem(&key, members);
            }
        }
        Operation::SetMove {
            source,
            destination,
            member,
        } => {
            if let Some(s) = set_store {
                let _ = s.smove(&source, &destination, member);
            }
        }
        Operation::SetInterStore { destination, keys } => {
            if let Some(s) = set_store {
                let _ = s.sinterstore(&destination, &keys);
            }
        }
        Operation::SetUnionStore { destination, keys } => {
            if let Some(s) = set_store {
                let _ = s.sunionstore(&destination, &keys);
            }
        }
        Operation::SetDiffStore { destination, keys } => {
            if let Some(s) = set_store {
                let _ = s.sdiffstore(&destination, &keys);
            }
        }

        // ── Sorted Set ──────────────────────────────────────────────────────
        Operation::ZAdd {
            key,
            member,
            score,
            nx,
            xx,
            gt,
            lt,
        } => {
            if let Some(z) = sorted_set_store {
                let opts = ZAddOptions {
                    nx,
                    xx,
                    gt,
                    lt,
                    ch: false,
                    incr: false,
                };
                let _ = z.zadd(&key, member, score, &opts);
            }
        }
        Operation::ZRem { key, members } => {
            if let Some(z) = sorted_set_store {
                let _ = z.zrem(&key, &members);
            }
        }
        Operation::ZIncrBy {
            key,
            member,
            increment,
        } => {
            if let Some(z) = sorted_set_store {
                let _ = z.zincrby(&key, member, increment);
            }
        }
        Operation::ZRemRangeByRank { key, start, stop } => {
            if let Some(z) = sorted_set_store {
                let _ = z.zremrangebyrank(&key, start, stop);
            }
        }
        Operation::ZRemRangeByScore { key, min, max } => {
            if let Some(z) = sorted_set_store {
                let _ = z.zremrangebyscore(&key, min, max);
            }
        }
        Operation::ZInterStore {
            destination,
            keys,
            weights,
            aggregate,
        } => {
            if let Some(z) = sorted_set_store {
                let key_refs: Vec<&str> = keys.iter().map(|s| s.as_str()).collect();
                let agg = parse_aggregate(&aggregate);
                let _ = z.zinterstore(&destination, &key_refs, weights.as_deref(), agg);
            }
        }
        Operation::ZUnionStore {
            destination,
            keys,
            weights,
            aggregate,
        } => {
            if let Some(z) = sorted_set_store {
                let key_refs: Vec<&str> = keys.iter().map(|s| s.as_str()).collect();
                let agg = parse_aggregate(&aggregate);
                let _ = z.zunionstore(&destination, &key_refs, weights.as_deref(), agg);
            }
        }
        Operation::ZDiffStore { destination, keys } => {
            if let Some(z) = sorted_set_store {
                let key_refs: Vec<&str> = keys.iter().map(|s| s.as_str()).collect();
                let _ = z.zdiffstore(&destination, &key_refs);
            }
        }
    }
    Ok(())
}

fn parse_aggregate(aggregate: &str) -> Aggregate {
    match aggregate.to_lowercase().as_str() {
        "min" => Aggregate::Min,
        "max" => Aggregate::Max,
        _ => Aggregate::Sum,
    }
}
