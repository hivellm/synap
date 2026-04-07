use super::types::{Operation, PersistenceConfig, Result};
use super::{SnapshotManager, WriteAheadLog};
use crate::core::QueueConfig;
use crate::core::hash::HashStore;
use crate::core::kv_store::KVStore;
use crate::core::list::ListStore;
use crate::core::queue::QueueManager;
use crate::core::set::SetStore;
use crate::core::sorted_set::{SortedSetStore, ZAddOptions};
use crate::core::types::KVConfig;
use tracing::info;

/// Recover system state from persistence
pub async fn recover(
    config: &PersistenceConfig,
    kv_config: KVConfig,
    queue_config: QueueConfig,
) -> Result<(
    KVStore,
    Option<HashStore>,
    Option<ListStore>,
    Option<SetStore>,
    Option<SortedSetStore>,
    Option<QueueManager>,
    u64,
)> {
    if !config.enabled {
        info!("Persistence disabled, starting with fresh state");
        return Ok((
            KVStore::new(kv_config),
            Some(HashStore::new()),
            Some(ListStore::new()),
            Some(SetStore::new()),
            Some(SortedSetStore::new()),
            Some(QueueManager::new(queue_config)),
            0,
        ));
    }

    info!("Starting recovery process...");

    let snapshot_mgr = SnapshotManager::new(config.snapshot.clone());
    let wal = WriteAheadLog::open(config.wal.clone()).await?;

    // Step 1: Load latest snapshot (if exists)
    let (kv_store, hash_store, list_store, set_store, sorted_set_store, queue_manager, last_offset) =
        if let Some((snapshot, path)) = snapshot_mgr.load_latest().await? {
            info!(
                "Loaded snapshot from {:?} at offset {}",
                path, snapshot.wal_offset
            );

            // Restore KV store
            let kv = KVStore::new(kv_config);
            for (key, value) in snapshot.kv_data {
                kv.set(&key, value, None).await?;
            }

            // Restore Queue manager
            let queues = QueueManager::new(queue_config);
            for (queue_name, messages) in snapshot.queue_data {
                queues.create_queue(&queue_name, None).await?;
                for message in messages {
                    queues
                        .publish(
                            &queue_name,
                            (*message.payload).clone(), // Convert Arc<Vec<u8>> to Vec<u8>
                            Some(message.priority),
                            Some(message.max_retries),
                        )
                        .await?;
                }
            }

            // Restore List store
            let lists = ListStore::new();
            for (key, list_value) in snapshot.list_data {
                // Restore list by pushing all elements
                let elements: Vec<Vec<u8>> = list_value.elements.into_iter().collect();
                if !elements.is_empty() {
                    lists.rpush(&key, elements, false)?;
                }
            }

            // Restore Set store
            let sets = SetStore::new();
            for (key, set_value) in snapshot.set_data {
                // Restore set by adding all members
                let members: Vec<Vec<u8>> = set_value.members.into_iter().collect();
                if !members.is_empty() {
                    sets.sadd(&key, members)?;
                }
            }

            // Restore Sorted Set store
            let sorted_sets = SortedSetStore::new();
            for (key, members_scores) in snapshot.sorted_set_data {
                // Restore sorted set by adding all members with their scores
                let opts = ZAddOptions::default();
                for (member, score) in members_scores {
                    sorted_sets.zadd(&key, member, score, &opts);
                }
            }

            (
                kv,
                Some(HashStore::new()),
                Some(lists),
                Some(sets),
                Some(sorted_sets),
                Some(queues),
                snapshot.wal_offset,
            )
        } else {
            info!("No snapshot found, starting fresh");
            (
                KVStore::new(kv_config),
                Some(HashStore::new()),
                Some(ListStore::new()),
                Some(SetStore::new()),
                Some(SortedSetStore::new()),
                Some(QueueManager::new(queue_config)),
                0,
            )
        };

    // Step 2: Replay WAL from snapshot offset
    info!("Replaying WAL from offset {}...", last_offset);
    let entries = wal.replay(last_offset).await?;
    let mut replayed = 0;

    for entry in entries {
        match entry.operation {
            Operation::KVSet { key, value, ttl } => {
                kv_store.set(&key, value, ttl).await?;
                replayed += 1;
            }
            Operation::KVDel { keys } => {
                for key in keys {
                    kv_store.delete(&key).await?;
                }
                replayed += 1;
            }
            Operation::KVRename {
                source,
                destination,
            } => {
                // Get value and TTL from source key
                let value = kv_store.get(&source).await?;
                if let Some(data) = value {
                    // Get TTL from source key
                    let ttl = kv_store.ttl(&source).await?;

                    // Set destination with same value and TTL
                    kv_store.set(&destination, data, ttl).await?;

                    // Delete source
                    kv_store.delete(&source).await?;
                }
                replayed += 1;
            }
            Operation::QueuePublish { queue, message } => {
                if let Some(ref qm) = queue_manager {
                    // Ensure queue exists
                    if qm.create_queue(&queue, None).await.is_ok() {
                        qm.publish(
                            &queue,
                            (*message.payload).clone(), // Convert Arc<Vec<u8>> to Vec<u8>
                            Some(message.priority),
                            Some(message.max_retries),
                        )
                        .await?;
                    }
                }
                replayed += 1;
            }
            Operation::QueueAck { queue, message_id } => {
                if let Some(ref qm) = queue_manager {
                    let _ = qm.ack(&queue, &message_id).await;
                }
                replayed += 1;
            }
            Operation::QueueNack {
                queue,
                message_id,
                requeue,
            } => {
                if let Some(ref qm) = queue_manager {
                    let _ = qm.nack(&queue, &message_id, requeue).await;
                }
                replayed += 1;
            }
            Operation::StreamPublish {
                room: _,
                event_type: _,
                payload: _,
            } => {
                // Stream operations are not replayed from WAL
                // They use their own persistence layer (StreamPersistence)
                // This is here to prevent compilation errors
                replayed += 1;
            }
            Operation::HashSet { key, field, value } => {
                if let Some(ref hash_store) = hash_store {
                    hash_store.hset(&key, &field, value)?;
                }
                replayed += 1;
            }
            Operation::HashDel { key, fields } => {
                if let Some(ref hash_store) = hash_store {
                    hash_store.hdel(&key, &fields)?;
                }
                replayed += 1;
            }
            Operation::HashIncrBy {
                key,
                field,
                increment,
            } => {
                if let Some(ref hash_store) = hash_store {
                    hash_store.hincrby(&key, &field, increment)?;
                }
                replayed += 1;
            }
            Operation::HashIncrByFloat {
                key,
                field,
                increment,
            } => {
                if let Some(ref hash_store) = hash_store {
                    hash_store.hincrbyfloat(&key, &field, increment)?;
                }
                replayed += 1;
            }
            Operation::ListPush { key, values, left } => {
                if let Some(ref list_store) = list_store {
                    if left {
                        list_store.lpush(&key, values, false)?;
                    } else {
                        list_store.rpush(&key, values, false)?;
                    }
                }
                replayed += 1;
            }
            Operation::ListPop { key, count, left } => {
                if let Some(ref list_store) = list_store {
                    let _ = if left {
                        list_store.lpop(&key, Some(count))
                    } else {
                        list_store.rpop(&key, Some(count))
                    };
                }
                replayed += 1;
            }
            Operation::ListSet { key, index, value } => {
                if let Some(ref list_store) = list_store {
                    let _ = list_store.lset(&key, index, value);
                }
                replayed += 1;
            }
            Operation::ListTrim { key, start, stop } => {
                if let Some(ref list_store) = list_store {
                    let _ = list_store.ltrim(&key, start, stop);
                }
                replayed += 1;
            }
            Operation::ListRem { key, count, value } => {
                if let Some(ref list_store) = list_store {
                    let _ = list_store.lrem(&key, count, value);
                }
                replayed += 1;
            }
            Operation::ListInsert {
                key,
                before,
                pivot,
                value,
            } => {
                if let Some(ref list_store) = list_store {
                    let _ = list_store.linsert(&key, before, pivot, value);
                }
                replayed += 1;
            }
            Operation::ListRpoplpush {
                source,
                destination,
            } => {
                if let Some(ref list_store) = list_store {
                    let _ = list_store.rpoplpush(&source, &destination);
                }
                replayed += 1;
            }
            Operation::SetAdd { key, members } => {
                if let Some(ref set_store) = set_store {
                    let _ = set_store.sadd(&key, members);
                }
                replayed += 1;
            }
            Operation::SetRem { key, members } => {
                if let Some(ref set_store) = set_store {
                    let _ = set_store.srem(&key, members);
                }
                replayed += 1;
            }
            Operation::SetMove {
                source,
                destination,
                member,
            } => {
                if let Some(ref set_store) = set_store {
                    let _ = set_store.smove(&source, &destination, member);
                }
                replayed += 1;
            }
            Operation::SetInterStore { destination, keys } => {
                if let Some(ref set_store) = set_store {
                    let _ = set_store.sinterstore(&destination, &keys);
                }
                replayed += 1;
            }
            Operation::SetUnionStore { destination, keys } => {
                if let Some(ref set_store) = set_store {
                    let _ = set_store.sunionstore(&destination, &keys);
                }
                replayed += 1;
            }
            Operation::SetDiffStore { destination, keys } => {
                if let Some(ref set_store) = set_store {
                    let _ = set_store.sdiffstore(&destination, &keys);
                }
                replayed += 1;
            }
            Operation::ZAdd {
                key,
                member,
                score,
                nx,
                xx,
                gt,
                lt,
            } => {
                if let Some(ref sorted_set_store) = sorted_set_store {
                    let opts = ZAddOptions {
                        nx,
                        xx,
                        gt,
                        lt,
                        ch: false,
                        incr: false,
                    };
                    let _ = sorted_set_store.zadd(&key, member, score, &opts);
                }
                replayed += 1;
            }
            Operation::ZRem { key, members } => {
                if let Some(ref sorted_set_store) = sorted_set_store {
                    let _ = sorted_set_store.zrem(&key, &members);
                }
                replayed += 1;
            }
            Operation::ZIncrBy {
                key,
                member,
                increment,
            } => {
                if let Some(ref sorted_set_store) = sorted_set_store {
                    let _ = sorted_set_store.zincrby(&key, member, increment);
                }
                replayed += 1;
            }
            Operation::ZRemRangeByRank { key, start, stop } => {
                if let Some(ref sorted_set_store) = sorted_set_store {
                    let _ = sorted_set_store.zremrangebyrank(&key, start, stop);
                }
                replayed += 1;
            }
            Operation::ZRemRangeByScore { key, min, max } => {
                if let Some(ref sorted_set_store) = sorted_set_store {
                    let _ = sorted_set_store.zremrangebyscore(&key, min, max);
                }
                replayed += 1;
            }
            Operation::ZInterStore {
                destination,
                keys,
                weights,
                aggregate,
            } => {
                if let Some(ref sorted_set_store) = sorted_set_store {
                    let key_refs: Vec<&str> = keys.iter().map(|s| s.as_str()).collect();
                    let agg = match aggregate.to_lowercase().as_str() {
                        "min" => crate::core::Aggregate::Min,
                        "max" => crate::core::Aggregate::Max,
                        _ => crate::core::Aggregate::Sum,
                    };
                    let _ = sorted_set_store.zinterstore(
                        &destination,
                        &key_refs,
                        weights.as_deref(),
                        agg,
                    );
                }
                replayed += 1;
            }
            Operation::ZUnionStore {
                destination,
                keys,
                weights,
                aggregate,
            } => {
                if let Some(ref sorted_set_store) = sorted_set_store {
                    let key_refs: Vec<&str> = keys.iter().map(|s| s.as_str()).collect();
                    let agg = match aggregate.to_lowercase().as_str() {
                        "min" => crate::core::Aggregate::Min,
                        "max" => crate::core::Aggregate::Max,
                        _ => crate::core::Aggregate::Sum,
                    };
                    let _ = sorted_set_store.zunionstore(
                        &destination,
                        &key_refs,
                        weights.as_deref(),
                        agg,
                    );
                }
                replayed += 1;
            }
            Operation::ZDiffStore { destination, keys } => {
                if let Some(ref sorted_set_store) = sorted_set_store {
                    let key_refs: Vec<&str> = keys.iter().map(|s| s.as_str()).collect();
                    let _ = sorted_set_store.zdiffstore(&destination, &key_refs);
                }
                replayed += 1;
            }
        }
    }

    info!("Recovery complete. Replayed {} operations", replayed);

    let final_offset = last_offset + replayed;

    Ok((
        kv_store,
        hash_store,
        list_store,
        set_store,
        sorted_set_store,
        queue_manager,
        final_offset,
    ))
}

/// Test recovery without actually loading data (validation only)
pub async fn validate_recovery(config: &PersistenceConfig) -> Result<RecoveryInfo> {
    let snapshot_mgr = SnapshotManager::new(config.snapshot.clone());
    let wal = WriteAheadLog::open(config.wal.clone()).await?;

    let snapshot_info = if let Some((snapshot, path)) = snapshot_mgr.load_latest().await? {
        Some(SnapshotInfo {
            path,
            offset: snapshot.wal_offset,
            timestamp: snapshot.timestamp,
            kv_count: snapshot.kv_data.len(),
            queue_count: snapshot.queue_data.len(),
        })
    } else {
        None
    };

    let wal_offset = wal.current_offset();
    let wal_entries = if let Some(ref si) = snapshot_info {
        wal.replay(si.offset).await?.len()
    } else {
        wal.replay(0).await?.len()
    };

    Ok(RecoveryInfo {
        snapshot: snapshot_info,
        wal_offset,
        wal_entries_to_replay: wal_entries,
    })
}

/// Information about recovery state
#[derive(Debug)]
pub struct RecoveryInfo {
    pub snapshot: Option<SnapshotInfo>,
    pub wal_offset: u64,
    pub wal_entries_to_replay: usize,
}

/// Information about a snapshot
#[derive(Debug)]
pub struct SnapshotInfo {
    pub path: std::path::PathBuf,
    pub offset: u64,
    pub timestamp: u64,
    pub kv_count: usize,
    pub queue_count: usize,
}
