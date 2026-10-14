//! Hub-mode names for every persisted structure.
//!
//! Hub mode keeps each tenant's resources under `user_<tenant>:<name>`
//! (`synap_server::hub::MultiTenant`). The handlers name KV keys and every
//! key-based structure (hashes, lists, sets, sorted sets and the geospatial
//! indexes stored in them, HyperLogLogs, bitmaps) with `scope_kv_key`, queues
//! with `scope_queue_name` and streams with `scope_stream_name`; renaming a
//! standalone image with the same functions makes the migrated data exactly
//! what that tenant reads in Hub mode.

use std::collections::HashMap;
use std::fmt;
use synap_server::hub::MultiTenant;
use synap_server::persistence::SnapshotImage;
use synap_server::persistence::types::StreamEvent;
use uuid::Uuid;

/// Rename every resource of `image` to the name Hub mode gives it for
/// `tenant`. Values, expiry and queue contents (configuration and every
/// message with its ID and metadata) move unchanged.
pub fn image(image: SnapshotImage, tenant: &Uuid) -> SnapshotImage {
    let SnapshotImage {
        kv,
        queues,
        hashes,
        lists,
        sets,
        sorted_sets,
        hyperloglogs,
        bitmaps,
    } = image;
    SnapshotImage {
        kv: kv
            .into_iter()
            .map(|(key, value, expires_at_ms)| (key_name(tenant, &key), value, expires_at_ms))
            .collect(),
        queues: queues
            .into_iter()
            .map(|(name, queue)| (MultiTenant::scope_queue_name(Some(tenant), &name), queue))
            .collect(),
        hashes: keyed(hashes, tenant),
        lists: keyed(lists, tenant),
        sets: keyed(sets, tenant),
        sorted_sets: keyed(sorted_sets, tenant),
        hyperloglogs: keyed(hyperloglogs, tenant),
        bitmaps: keyed(bitmaps, tenant),
    }
}

/// Rename every stream room for `tenant`; each event keeps its ID, offset,
/// type, data and timestamp.
pub fn streams(
    streams: HashMap<String, Vec<StreamEvent>>,
    tenant: &Uuid,
) -> HashMap<String, Vec<StreamEvent>> {
    streams
        .into_iter()
        .map(|(room, events)| (MultiTenant::scope_stream_name(Some(tenant), &room), events))
        .collect()
}

fn key_name(tenant: &Uuid, key: &str) -> String {
    MultiTenant::scope_kv_key(Some(tenant), key).into_owned()
}

fn keyed<V>(map: HashMap<String, V>, tenant: &Uuid) -> HashMap<String, V> {
    map.into_iter()
        .map(|(key, value)| (key_name(tenant, &key), value))
        .collect()
}

/// A kind of persisted structure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Kv,
    Hash,
    List,
    Set,
    SortedSet,
    HyperLogLog,
    Bitmap,
    Queue,
    Stream,
}

impl fmt::Display for Kind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Kind::Kv => "KV key",
            Kind::Hash => "hash",
            Kind::List => "list",
            Kind::Set => "set",
            Kind::SortedSet => "sorted set",
            Kind::HyperLogLog => "HyperLogLog",
            Kind::Bitmap => "bitmap",
            Kind::Queue => "queue",
            Kind::Stream => "stream",
        })
    }
}

/// Every resource name in `image` and `streams`, with its kind.
pub fn names<'a>(
    image: &'a SnapshotImage,
    streams: &'a HashMap<String, Vec<StreamEvent>>,
) -> Vec<(Kind, &'a str)> {
    let mut names: Vec<(Kind, &str)> = image
        .kv
        .iter()
        .map(|(key, _, _)| (Kind::Kv, key.as_str()))
        .collect();
    let mut add = |kind: Kind, keys: Vec<&'a String>| {
        names.extend(keys.into_iter().map(|key| (kind, key.as_str())));
    };
    add(Kind::Hash, image.hashes.keys().collect());
    add(Kind::List, image.lists.keys().collect());
    add(Kind::Set, image.sets.keys().collect());
    add(Kind::SortedSet, image.sorted_sets.keys().collect());
    add(Kind::HyperLogLog, image.hyperloglogs.keys().collect());
    add(Kind::Bitmap, image.bitmaps.keys().collect());
    add(Kind::Queue, image.queues.keys().collect());
    add(Kind::Stream, streams.keys().collect());
    names
}

/// The tenant a Hub-scoped name belongs to: the name starts with `user_`, 32
/// hexadecimal digits and `:`, as `MultiTenant` writes it.
pub fn owner(name: &str) -> Option<Uuid> {
    let (prefix, _) = name.strip_prefix("user_")?.split_once(':')?;
    if prefix.len() != 32 || !prefix.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    Uuid::parse_str(prefix).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use synap_server::core::{
        BitmapStore, HashStore, HyperLogLogStore, ListStore, SetStore, SortedSetStore,
    };
    use synap_server::persistence::StoreRefs;
    use synap_server::{KVConfig, KVStore, QueueConfig, QueueManager};

    fn tenant() -> Uuid {
        Uuid::parse_str("7d3f2a10-5b8c-4e2f-9a61-0c4d8e7f1b23").unwrap()
    }

    /// Every structure is renamed the way the Hub handlers name it, and its
    /// contents, expiry and queue metadata are untouched.
    #[tokio::test]
    async fn every_structure_gets_the_hub_name_and_keeps_its_contents() {
        let kv = KVStore::new(KVConfig::default());
        kv.set("k", b"v".to_vec(), Some(600)).await.unwrap();
        let hashes = HashStore::new();
        hashes.hset("h", "f", b"1".to_vec()).unwrap();
        let lists = ListStore::new();
        lists.rpush("l", vec![b"a".to_vec()], false).unwrap();
        let sets = SetStore::new();
        sets.sadd("s", vec![b"m".to_vec()]).unwrap();
        let sorted_sets = SortedSetStore::new();
        sorted_sets.zadd("z", b"m".to_vec(), 2.5, &Default::default());
        let hll = HyperLogLogStore::new();
        hll.pfadd("p", vec![b"x".to_vec()], None).unwrap();
        let bitmaps = BitmapStore::new();
        bitmaps.setbit("b", 3, 1).unwrap();
        let queues = QueueManager::new(QueueConfig::default());
        queues.create_queue("q", None).await.unwrap();
        let id = queues
            .publish("q", b"m".to_vec(), Some(9), Some(4))
            .await
            .unwrap();
        let captured = SnapshotImage::capture(StoreRefs {
            kv_store: &kv,
            hash_store: Some(&hashes),
            list_store: Some(&lists),
            set_store: Some(&sets),
            sorted_set_store: Some(&sorted_sets),
            queue_manager: Some(&queues),
            stream_manager: None,
            hyperloglog_store: Some(&hll),
            bitmap_store: Some(&bitmaps),
        })
        .await
        .unwrap();
        let deadline = captured.kv[0].2;
        assert!(deadline.is_some());

        let t = tenant();
        let scoped = image(captured, &t);
        let name = |n: &str| MultiTenant::scope_kv_key(Some(&t), n).into_owned();
        assert_eq!(scoped.kv.len(), 1);
        assert_eq!(scoped.kv[0].0, name("k"));
        assert_eq!(scoped.kv[0].1, Arc::<[u8]>::from(b"v".to_vec()));
        assert_eq!(scoped.kv[0].2, deadline);
        assert_eq!(scoped.hashes[&name("h")]["f"], b"1".to_vec());
        assert!(scoped.lists.contains_key(&name("l")));
        assert!(scoped.sets.contains_key(&name("s")));
        assert_eq!(scoped.sorted_sets[&name("z")], vec![(b"m".to_vec(), 2.5)]);
        assert!(scoped.hyperloglogs.contains_key(&name("p")));
        assert!(scoped.bitmaps.contains_key(&name("b")));
        let queue = &scoped.queues[&MultiTenant::scope_queue_name(Some(&t), "q")];
        assert_eq!(queue.ready.len(), 1);
        assert_eq!(queue.ready[0].id, id);
        assert_eq!(queue.ready[0].priority, 9);
        assert_eq!(queue.ready[0].max_retries, 4);

        let room = streams(
            HashMap::from([(
                "room".to_string(),
                vec![StreamEvent {
                    id: "e".into(),
                    offset: 4,
                    event_type: "t".into(),
                    data: vec![1],
                    timestamp: 9,
                }],
            )]),
            &t,
        );
        let events = &room[&MultiTenant::scope_stream_name(Some(&t), "room")];
        assert_eq!((events[0].id.as_str(), events[0].offset), ("e", 4));

        let all = names(&scoped, &room);
        assert_eq!(all.len(), 9);
        assert!(all.iter().all(|(_, n)| owner(n) == Some(t)));
    }

    /// Only the exact prefix Hub mode writes counts as scoped.
    #[test]
    fn owner_recognizes_only_hub_prefixes() {
        let t = tenant();
        assert_eq!(owner(&MultiTenant::scope_kv_key(Some(&t), "a:b")), Some(t));
        assert_eq!(owner("user_settings:theme"), None);
        assert_eq!(owner("user_7d3f2a10:x"), None);
        assert_eq!(owner("plain"), None);
        assert_eq!(owner("user_7d3f2a105b8c4e2f9a610c4d8e7f1b2z:x"), None);
    }
}
