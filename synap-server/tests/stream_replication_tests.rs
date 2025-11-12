//! Event Streams Replication Integration Tests
//!
//! Tests for Event Streams with master-slave replication:
//! - Full sync with stream data
//! - Partial sync with stream events
//! - Multiple rooms replication
//! - Stream recovery after failover

use std::sync::Arc;
use std::sync::atomic::{AtomicU16, Ordering};
use std::time::Duration;
use synap_server::core::{StreamConfig, StreamManager};
use synap_server::replication::{MasterNode, NodeRole, ReplicaNode, ReplicationConfig};
use synap_server::{KVConfig, KVStore};
use tokio::time::sleep;

static STREAM_TEST_PORT: AtomicU16 = AtomicU16::new(40000);

fn next_port() -> u16 {
    STREAM_TEST_PORT.fetch_add(1, Ordering::SeqCst)
}

async fn create_stream_master() -> (
    Arc<MasterNode>,
    Arc<KVStore>,
    Arc<StreamManager>,
    std::net::SocketAddr,
) {
    let port = next_port();
    let addr: std::net::SocketAddr = format!("127.0.0.1:{}", port).parse().unwrap();

    let config = ReplicationConfig {
        enabled: true,
        role: NodeRole::Master,
        replica_listen_address: Some(addr),
        heartbeat_interval_ms: 100,
        ..Default::default()
    };

    let kv = Arc::new(KVStore::new(KVConfig::default()));
    let stream_mgr = Arc::new(StreamManager::new(StreamConfig::default()));

    let master = Arc::new(
        MasterNode::new(config, Arc::clone(&kv), Some(Arc::clone(&stream_mgr)))
            .await
            .unwrap(),
    );

    sleep(Duration::from_millis(100)).await;

    (master, kv, stream_mgr, addr)
}

async fn create_stream_replica(
    master_addr: std::net::SocketAddr,
) -> (Arc<ReplicaNode>, Arc<KVStore>, Arc<StreamManager>) {
    let config = ReplicationConfig {
        enabled: true,
        role: NodeRole::Replica,
        master_address: Some(master_addr),
        auto_reconnect: true,
        reconnect_delay_ms: 100,
        ..Default::default()
    };

    let kv = Arc::new(KVStore::new(KVConfig::default()));
    let stream_mgr = Arc::new(StreamManager::new(StreamConfig::default()));

    let replica = ReplicaNode::new(config, Arc::clone(&kv), Some(Arc::clone(&stream_mgr)))
        .await
        .unwrap();

    sleep(Duration::from_millis(50)).await;

    (replica, kv, stream_mgr)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_stream_full_sync() {
    // Create master with stream data
    let (_master, _master_kv, master_stream, master_addr) = create_stream_master().await;

    // Create rooms and publish events BEFORE replica connects
    master_stream.create_room("room1").await.unwrap();
    master_stream.create_room("room2").await.unwrap();

    // Publish events to room1
    for i in 0..10 {
        let data = format!("event_{}", i).into_bytes();
        master_stream
            .publish("room1", "test_event", data)
            .await
            .unwrap();
    }

    // Publish events to room2
    for i in 0..5 {
        let data = format!("room2_event_{}", i).into_bytes();
        master_stream
            .publish("room2", "test_event", data)
            .await
            .unwrap();
    }

    sleep(Duration::from_millis(100)).await;

    // Create replica - should receive full sync with stream data
    let (_replica, _replica_kv, replica_stream) = create_stream_replica(master_addr).await;

    // Wait for full sync
    sleep(Duration::from_secs(2)).await;

    // Verify rooms were created
    let rooms = replica_stream.list_rooms().await;
    assert!(rooms.contains(&"room1".to_string()), "room1 not replicated");
    assert!(rooms.contains(&"room2".to_string()), "room2 not replicated");

    // Verify events in room1
    let events = replica_stream
        .consume("room1", "test_consumer", 0, 100)
        .await
        .unwrap();
    assert_eq!(events.len(), 10, "room1 events not fully replicated");

    // Verify events in room2
    let events = replica_stream
        .consume("room2", "test_consumer", 0, 100)
        .await
        .unwrap();
    assert_eq!(events.len(), 5, "room2 events not fully replicated");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_stream_partial_sync() {
    // Create master and replica (both with streams)
    let (master, _master_kv, master_stream, master_addr) = create_stream_master().await;

    // Create replica first
    let (_replica, _replica_kv, replica_stream) = create_stream_replica(master_addr).await;

    // Wait for initial sync
    sleep(Duration::from_secs(1)).await;

    // Now create room and publish events (after replica is connected)
    master_stream.create_room("live_room").await.unwrap();

    // Publish events - these should be replicated incrementally
    for i in 0..20 {
        let data = format!("live_event_{}", i).into_bytes();
        let offset = master_stream
            .publish("live_room", "live_event", data.clone())
            .await
            .unwrap();

        // Replicate via replication log
        master.replicate(synap_server::persistence::types::Operation::StreamPublish {
            room: "live_room".to_string(),
            event_type: "live_event".to_string(),
            payload: data,
        });

        // Small delay
        sleep(Duration::from_millis(10)).await;

        eprintln!("Published event {} at offset {}", i, offset);
    }

    // Wait for replication
    sleep(Duration::from_secs(2)).await;

    // Verify room exists on replica
    let rooms = replica_stream.list_rooms().await;
    assert!(
        rooms.contains(&"live_room".to_string()),
        "live_room not replicated"
    );

    // Verify events were replicated
    let events = replica_stream
        .consume("live_room", "test_consumer", 0, 100)
        .await
        .unwrap();

    eprintln!("Replica received {} events", events.len());
    assert!(
        events.len() >= 15,
        "Expected at least 15 events, got {}",
        events.len()
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_stream_multiple_rooms_sync() {
    let (_master, _master_kv, master_stream, master_addr) = create_stream_master().await;

    // Create multiple rooms with different amounts of data
    for room_id in 0..5 {
        let room_name = format!("room_{}", room_id);
        master_stream.create_room(&room_name).await.unwrap();

        // Different number of events per room
        for i in 0..(room_id + 1) * 10 {
            let data = format!("event_{}_{}", room_id, i).into_bytes();
            master_stream
                .publish(&room_name, "event", data)
                .await
                .unwrap();
        }
    }

    sleep(Duration::from_millis(100)).await;

    // Create replica
    let (_replica, _replica_kv, replica_stream) = create_stream_replica(master_addr).await;

    // Wait for full sync
    sleep(Duration::from_secs(2)).await;

    // Verify all rooms were replicated
    let rooms = replica_stream.list_rooms().await;
    assert_eq!(rooms.len(), 5, "Not all rooms replicated");

    // Verify event counts
    for room_id in 0..5 {
        let room_name = format!("room_{}", room_id);
        let events = replica_stream
            .consume(&room_name, "test", 0, 1000)
            .await
            .unwrap();

        let expected_count = (room_id + 1) * 10;
        assert_eq!(
            events.len(),
            expected_count,
            "room_{} has wrong event count: expected {}, got {}",
            room_id,
            expected_count,
            events.len()
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_stream_kv_combined_sync() {
    // Test that both KV and Streams sync together
    let (_master, master_kv, master_stream, master_addr) = create_stream_master().await;

    // Add KV data
    for i in 0..50 {
        let key = format!("key_{}", i);
        let value = format!("value_{}", i).into_bytes();
        master_kv.set(&key, value, None).await.unwrap();
    }

    // Add stream data
    master_stream.create_room("combined_room").await.unwrap();
    for i in 0..30 {
        let data = format!("stream_event_{}", i).into_bytes();
        master_stream
            .publish("combined_room", "event", data)
            .await
            .unwrap();
    }

    sleep(Duration::from_millis(100)).await;

    // Create replica
    let (_replica, replica_kv, replica_stream) = create_stream_replica(master_addr).await;

    // Wait for full sync
    sleep(Duration::from_secs(2)).await;

    // Verify KV data
    let kv_keys = replica_kv.keys().await.unwrap();
    assert_eq!(kv_keys.len(), 50, "KV data not fully synced");

    // Verify random KV values
    assert_eq!(
        replica_kv.get("key_10").await.unwrap(),
        Some(b"value_10".to_vec())
    );
    assert_eq!(
        replica_kv.get("key_25").await.unwrap(),
        Some(b"value_25".to_vec())
    );

    // Verify stream data
    let rooms = replica_stream.list_rooms().await;
    assert_eq!(rooms.len(), 1, "Stream room not synced");

    let events = replica_stream
        .consume("combined_room", "test", 0, 100)
        .await
        .unwrap();
    assert_eq!(events.len(), 30, "Stream events not fully synced");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_stream_offset_preservation() {
    // Test that stream offsets are preserved during replication
    let (_master, _master_kv, master_stream, master_addr) = create_stream_master().await;

    master_stream.create_room("offset_room").await.unwrap();

    // Publish events and track offsets
    let mut expected_offsets = Vec::new();
    for i in 0..15 {
        let data = format!("offset_event_{}", i).into_bytes();
        let offset = master_stream
            .publish("offset_room", "event", data)
            .await
            .unwrap();
        expected_offsets.push(offset);
    }

    sleep(Duration::from_millis(100)).await;

    // Create replica
    let (_replica, _replica_kv, replica_stream) = create_stream_replica(master_addr).await;
    sleep(Duration::from_secs(2)).await;

    // Verify offsets match
    let events = replica_stream
        .consume("offset_room", "test", 0, 100)
        .await
        .unwrap();

    for (i, event) in events.iter().enumerate() {
        assert_eq!(
            event.offset, expected_offsets[i],
            "Offset mismatch at position {}: expected {}, got {}",
            i, expected_offsets[i], event.offset
        );
    }
}
