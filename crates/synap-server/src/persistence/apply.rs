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

/// Borrowed bundle of every store a persistence routine can touch.
///
/// `kv_store` is always present; the collection/broker stores are optional (a
/// datatype that is disabled is simply skipped). Shared by the operation
/// applier ([`apply_operation`]) and the snapshot path so the two never
/// disagree on which stores exist.
#[derive(Clone, Copy)]
pub struct StoreRefs<'a> {
    pub kv_store: &'a KVStore,
    pub hash_store: Option<&'a HashStore>,
    pub list_store: Option<&'a ListStore>,
    pub set_store: Option<&'a SetStore>,
    pub sorted_set_store: Option<&'a SortedSetStore>,
    pub queue_manager: Option<&'a QueueManager>,
    pub stream_manager: Option<&'a StreamManager>,
}

impl<'a> StoreRefs<'a> {
    /// A bundle with only the KV store (collection/broker stores absent).
    pub fn kv_only(kv_store: &'a KVStore) -> Self {
        Self {
            kv_store,
            hash_store: None,
            list_store: None,
            set_store: None,
            sorted_set_store: None,
            queue_manager: None,
            stream_manager: None,
        }
    }
}

/// Owned (`Arc`) counterpart of [`StoreRefs`] for consumers that must hold the
/// stores across `'static` boundaries (background snapshot task, replica node).
#[derive(Clone)]
pub struct StoreArcs {
    pub kv_store: std::sync::Arc<KVStore>,
    pub hash_store: Option<std::sync::Arc<HashStore>>,
    pub list_store: Option<std::sync::Arc<ListStore>>,
    pub set_store: Option<std::sync::Arc<SetStore>>,
    pub sorted_set_store: Option<std::sync::Arc<SortedSetStore>>,
    pub queue_manager: Option<std::sync::Arc<QueueManager>>,
    pub stream_manager: Option<std::sync::Arc<StreamManager>>,
}

impl StoreArcs {
    /// A bundle with only the KV store (collection/broker stores absent).
    pub fn kv_only(kv_store: std::sync::Arc<KVStore>) -> Self {
        Self {
            kv_store,
            hash_store: None,
            list_store: None,
            set_store: None,
            sorted_set_store: None,
            queue_manager: None,
            stream_manager: None,
        }
    }

    /// Borrow the bundle as a [`StoreRefs`] for the snapshot/apply APIs.
    pub fn as_refs(&self) -> StoreRefs<'_> {
        StoreRefs {
            kv_store: &self.kv_store,
            hash_store: self.hash_store.as_deref(),
            list_store: self.list_store.as_deref(),
            set_store: self.set_store.as_deref(),
            sorted_set_store: self.sorted_set_store.as_deref(),
            queue_manager: self.queue_manager.as_deref(),
            stream_manager: self.stream_manager.as_deref(),
        }
    }
}

/// Apply a single [`Operation`] to the provided stores.
///
/// Errors from KV and hash writes propagate; list/set/sorted-set/queue/stream
/// applies are best-effort (idempotent replays), matching WAL recovery
/// semantics.
pub async fn apply_operation(op: Operation, stores: StoreRefs<'_>) -> Result<(), SynapError> {
    let StoreRefs {
        kv_store,
        hash_store,
        list_store,
        set_store,
        sorted_set_store,
        queue_manager,
        stream_manager,
    } = stores;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::KVConfig;
    use crate::core::StreamConfig;
    use crate::core::queue::QueueConfig;

    struct Stores {
        kv: KVStore,
        hash: HashStore,
        list: ListStore,
        set: SetStore,
        zset: SortedSetStore,
        queue: QueueManager,
        stream: StreamManager,
    }

    fn stores() -> Stores {
        Stores {
            kv: KVStore::new(KVConfig::default()),
            hash: HashStore::new(),
            list: ListStore::new(),
            set: SetStore::new(),
            zset: SortedSetStore::new(),
            queue: QueueManager::new(QueueConfig::default()),
            stream: StreamManager::new(StreamConfig::default()),
        }
    }

    /// Apply an op with every store present (replica-style: stream applied).
    async fn apply(s: &Stores, op: Operation) {
        apply_operation(
            op,
            StoreRefs {
                kv_store: &s.kv,
                hash_store: Some(&s.hash),
                list_store: Some(&s.list),
                set_store: Some(&s.set),
                sorted_set_store: Some(&s.zset),
                queue_manager: Some(&s.queue),
                stream_manager: Some(&s.stream),
            },
        )
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn applies_kv_operations() {
        let s = stores();
        apply(
            &s,
            Operation::KVSet {
                key: "a".into(),
                value: b"1".to_vec(),
                ttl: None,
            },
        )
        .await;
        assert_eq!(s.kv.get("a").await.unwrap(), Some(b"1".to_vec()));

        apply(
            &s,
            Operation::KVRename {
                source: "a".into(),
                destination: "b".into(),
            },
        )
        .await;
        assert_eq!(s.kv.get("b").await.unwrap(), Some(b"1".to_vec()));
        assert_eq!(s.kv.get("a").await.unwrap(), None);

        apply(
            &s,
            Operation::KVDel {
                keys: vec!["b".into()],
            },
        )
        .await;
        assert_eq!(s.kv.get("b").await.unwrap(), None);
    }

    #[tokio::test]
    async fn applies_hash_operations() {
        let s = stores();
        apply(
            &s,
            Operation::HashSet {
                key: "h".into(),
                field: "f".into(),
                value: b"v".to_vec(),
            },
        )
        .await;
        assert_eq!(s.hash.hget("h", "f").unwrap(), Some(b"v".to_vec()));

        apply(
            &s,
            Operation::HashIncrBy {
                key: "h".into(),
                field: "n".into(),
                increment: 4,
            },
        )
        .await;
        apply(
            &s,
            Operation::HashIncrByFloat {
                key: "h".into(),
                field: "fl".into(),
                increment: 1.5,
            },
        )
        .await;
        apply(
            &s,
            Operation::HashDel {
                key: "h".into(),
                fields: vec!["f".into()],
            },
        )
        .await;
        assert_eq!(s.hash.hget("h", "f").unwrap(), None);
        assert_eq!(s.hash.hget("h", "n").unwrap(), Some(b"4".to_vec()));
    }

    #[tokio::test]
    async fn applies_list_operations() {
        let s = stores();
        apply(
            &s,
            Operation::ListPush {
                key: "l".into(),
                values: vec![b"a".to_vec(), b"b".to_vec(), b"c".to_vec()],
                left: false,
            },
        )
        .await;
        apply(
            &s,
            Operation::ListPush {
                key: "l".into(),
                values: vec![b"z".to_vec()],
                left: true,
            },
        )
        .await;
        apply(
            &s,
            Operation::ListSet {
                key: "l".into(),
                index: 0,
                value: b"Z".to_vec(),
            },
        )
        .await;
        apply(
            &s,
            Operation::ListInsert {
                key: "l".into(),
                before: true,
                pivot: b"a".to_vec(),
                value: b"a0".to_vec(),
            },
        )
        .await;
        apply(
            &s,
            Operation::ListRem {
                key: "l".into(),
                count: 1,
                value: b"a0".to_vec(),
            },
        )
        .await;
        apply(
            &s,
            Operation::ListTrim {
                key: "l".into(),
                start: 0,
                stop: 3,
            },
        )
        .await;
        apply(
            &s,
            Operation::ListPop {
                key: "l".into(),
                count: 1,
                left: true,
            },
        )
        .await;
        apply(
            &s,
            Operation::ListRpoplpush {
                source: "l".into(),
                destination: "l2".into(),
            },
        )
        .await;
        // Both lists exist and the applies did not error.
        assert!(!s.list.lrange("l", 0, -1).unwrap().is_empty());
    }

    #[tokio::test]
    async fn applies_set_operations() {
        let s = stores();
        apply(
            &s,
            Operation::SetAdd {
                key: "s1".into(),
                members: vec![b"a".to_vec(), b"b".to_vec()],
            },
        )
        .await;
        apply(
            &s,
            Operation::SetAdd {
                key: "s2".into(),
                members: vec![b"b".to_vec(), b"c".to_vec()],
            },
        )
        .await;
        apply(
            &s,
            Operation::SetRem {
                key: "s1".into(),
                members: vec![b"a".to_vec()],
            },
        )
        .await;
        apply(
            &s,
            Operation::SetMove {
                source: "s2".into(),
                destination: "s1".into(),
                member: b"c".to_vec(),
            },
        )
        .await;
        apply(
            &s,
            Operation::SetInterStore {
                destination: "si".into(),
                keys: vec!["s1".into(), "s2".into()],
            },
        )
        .await;
        apply(
            &s,
            Operation::SetUnionStore {
                destination: "su".into(),
                keys: vec!["s1".into(), "s2".into()],
            },
        )
        .await;
        apply(
            &s,
            Operation::SetDiffStore {
                destination: "sd".into(),
                keys: vec!["s1".into(), "s2".into()],
            },
        )
        .await;
        assert!(s.set.sismember("s1", b"c".to_vec()).unwrap());
    }

    #[tokio::test]
    async fn applies_sorted_set_operations() {
        let s = stores();
        for (m, score) in [("a", 1.0), ("b", 2.0), ("c", 3.0)] {
            apply(
                &s,
                Operation::ZAdd {
                    key: "z1".into(),
                    member: m.as_bytes().to_vec(),
                    score,
                    nx: false,
                    xx: false,
                    gt: false,
                    lt: false,
                },
            )
            .await;
        }
        apply(
            &s,
            Operation::ZIncrBy {
                key: "z1".into(),
                member: b"a".to_vec(),
                increment: 10.0,
            },
        )
        .await;
        apply(
            &s,
            Operation::ZRem {
                key: "z1".into(),
                members: vec![b"b".to_vec()],
            },
        )
        .await;
        apply(
            &s,
            Operation::ZRemRangeByRank {
                key: "z1".into(),
                start: 0,
                stop: 0,
            },
        )
        .await;
        apply(
            &s,
            Operation::ZRemRangeByScore {
                key: "z1".into(),
                min: 100.0,
                max: 200.0,
            },
        )
        .await;
        // Build a second set for the store ops.
        apply(
            &s,
            Operation::ZAdd {
                key: "z2".into(),
                member: b"a".to_vec(),
                score: 5.0,
                nx: false,
                xx: false,
                gt: false,
                lt: false,
            },
        )
        .await;
        apply(
            &s,
            Operation::ZInterStore {
                destination: "zi".into(),
                keys: vec!["z1".into(), "z2".into()],
                weights: None,
                aggregate: "sum".into(),
            },
        )
        .await;
        apply(
            &s,
            Operation::ZUnionStore {
                destination: "zu".into(),
                keys: vec!["z1".into(), "z2".into()],
                weights: Some(vec![1.0, 2.0]),
                aggregate: "max".into(),
            },
        )
        .await;
        apply(
            &s,
            Operation::ZDiffStore {
                destination: "zd".into(),
                keys: vec!["z1".into(), "z2".into()],
            },
        )
        .await;
        assert_eq!(s.zset.zscore("z1", b"a"), Some(11.0));
    }

    #[tokio::test]
    async fn applies_queue_and_stream_operations() {
        let s = stores();
        let msg = crate::core::queue::QueueMessage::new(b"payload".to_vec(), 0, 3);
        apply(
            &s,
            Operation::QueuePublish {
                queue: "q".into(),
                message: msg,
            },
        )
        .await;
        apply(
            &s,
            Operation::QueueAck {
                queue: "q".into(),
                message_id: "missing".into(),
            },
        )
        .await;
        apply(
            &s,
            Operation::QueueNack {
                queue: "q".into(),
                message_id: "missing".into(),
                requeue: true,
            },
        )
        .await;

        apply(
            &s,
            Operation::StreamPublish {
                room: "r".into(),
                event_type: "e".into(),
                payload: b"data".to_vec(),
            },
        )
        .await;
        assert!(s.stream.list_rooms().await.contains(&"r".to_string()));
    }

    /// With `stream_manager = None` (WAL recovery), a StreamPublish is skipped.
    #[tokio::test]
    async fn stream_publish_skipped_without_manager() {
        let s = stores();
        apply_operation(
            Operation::StreamPublish {
                room: "r".into(),
                event_type: "e".into(),
                payload: b"data".to_vec(),
            },
            StoreRefs {
                kv_store: &s.kv,
                hash_store: None,
                list_store: None,
                set_store: None,
                sorted_set_store: None,
                queue_manager: None,
                stream_manager: None,
            },
        )
        .await
        .unwrap();
        assert!(s.stream.list_rooms().await.is_empty());
    }

    #[test]
    fn parse_aggregate_maps_all() {
        assert!(matches!(parse_aggregate("min"), Aggregate::Min));
        assert!(matches!(parse_aggregate("MAX"), Aggregate::Max));
        assert!(matches!(parse_aggregate("sum"), Aggregate::Sum));
        assert!(matches!(parse_aggregate("other"), Aggregate::Sum));
    }
}
