//! Integration tests for broker-level Prometheus metrics (GitHub issue #196).
//!
//! Verifies that `/metrics` exposes per-stream length, per-partition
//! high-water-mark, consumer-group lag/committed-offset, and queue depth after
//! a scrape, plus the corrected process/host system gauges.
//!
//! All assertions live in a single test: the Prometheus registry is a
//! process-global, and `reset_broker_gauges()` clears every broker series, so
//! parallel test functions in this binary would race on shared gauge state.

mod app_state_helper;

use std::sync::Arc;

use synap_server::core::{HashStore, ListStore, SetStore, SortedSetStore};
use synap_server::metrics::encode_metrics;
use synap_server::server::metrics_handler::{update_broker_metrics, update_system_metrics};
use synap_server::{
    AppState, ConsumerGroupConfig, ConsumerGroupManager, KVConfig, KVStore, PartitionConfig,
    PartitionManager, QueueConfig, QueueManager, StreamConfig, StreamManager,
};

/// Build a base `AppState` with empty broker managers.
fn base_state() -> AppState {
    let kv = Arc::new(KVStore::new(KVConfig::default()));
    let hash = Arc::new(HashStore::new());
    let list = Arc::new(ListStore::new());
    let set = Arc::new(SetStore::new());
    let zset = Arc::new(SortedSetStore::new());
    app_state_helper::create_test_app_state_with_stores(kv, hash, list, set, zset)
}

/// Find the value of a metric line matching `name` and containing every label
/// fragment in `labels` (order-independent). Returns the numeric suffix.
fn metric_value(text: &str, name: &str, labels: &[&str]) -> Option<i64> {
    text.lines()
        .filter(|l| l.starts_with(name) && labels.iter().all(|frag| l.contains(frag)))
        .find_map(|l| l.rsplit(' ').next().and_then(|v| v.parse::<i64>().ok()))
}

#[tokio::test]
async fn test_metrics_endpoint_exposes_broker_and_system_gauges() {
    let mut state = base_state();

    // ── Stream: 3 events in room "events" → length 3, last offset 2 ──
    let sm = Arc::new(StreamManager::new(StreamConfig::default()));
    sm.create_room("events").await.unwrap();
    for i in 0..3 {
        sm.publish("events", "evt", format!("m{i}").into_bytes())
            .await
            .unwrap();
    }
    state.stream_manager = Some(sm.clone());

    // ── Partition: single-partition topic "orders" with 5 events → end 4 ──
    let pm = Arc::new(PartitionManager::new(PartitionConfig {
        num_partitions: 1,
        ..Default::default()
    }));
    pm.create_topic("orders", None).await.unwrap();
    for i in 0..5 {
        pm.publish("orders", "order", None, format!("o{i}").into_bytes())
            .await
            .unwrap();
    }
    state.partition_manager = Some(pm);

    // ── Consumer group on "orders", committed offset 2 → lag = 4 − 2 = 2 ──
    let cg = Arc::new(ConsumerGroupManager::new(ConsumerGroupConfig::default()));
    cg.create_group("g1", "orders", 1, None).await.unwrap();
    cg.join_group("g1", 30).await.unwrap();
    cg.commit_offset("g1", 0, 2).await.unwrap();
    state.consumer_group_manager = Some(cg);

    // ── Queue: 2 pending messages → depth 2 ──
    let qm = Arc::new(QueueManager::new(QueueConfig::default()));
    qm.create_queue("q1", None).await.unwrap();
    qm.publish("q1", b"a".to_vec(), None, None).await.unwrap();
    qm.publish("q1", b"b".to_vec(), None, None).await.unwrap();
    state.queue_manager = Some(qm);

    // Scrape.
    update_broker_metrics(&state).await;
    let enc = encode_metrics().unwrap();

    // Stream length + last offset.
    assert_eq!(
        metric_value(&enc, "synap_stream_buffer_size", &["room=\"events\""]),
        Some(3),
        "stream buffer size\n{enc}"
    );
    assert_eq!(
        metric_value(&enc, "synap_stream_last_offset", &["room=\"events\""]),
        Some(2),
        "stream last offset"
    );

    // Partition high-water-mark.
    assert_eq!(
        metric_value(
            &enc,
            "synap_partition_end_offset",
            &["topic=\"orders\"", "partition=\"0\""]
        ),
        Some(4),
        "partition end offset"
    );

    // Consumer-group committed offset + lag + members.
    assert_eq!(
        metric_value(
            &enc,
            "synap_consumer_group_committed_offset",
            &["group=\"g1\"", "partition=\"0\""]
        ),
        Some(2),
        "committed offset"
    );
    assert_eq!(
        metric_value(
            &enc,
            "synap_consumer_group_lag",
            &["group=\"g1\"", "partition=\"0\""]
        ),
        Some(2),
        "consumer lag"
    );
    assert_eq!(
        metric_value(&enc, "synap_consumer_group_members", &["group=\"g1\""]),
        Some(1),
        "group members"
    );

    // Queue depth.
    assert_eq!(
        metric_value(&enc, "synap_queue_depth", &["queue=\"q1\""]),
        Some(2),
        "queue depth"
    );

    // ── Stale-series clearing: delete the room, re-scrape, gauge must vanish ──
    sm.delete_room("events").await.unwrap();
    update_broker_metrics(&state).await;
    let enc = encode_metrics().unwrap();
    assert_eq!(
        metric_value(&enc, "synap_stream_last_offset", &["room=\"events\""]),
        None,
        "deleted stream must not report a stale gauge",
    );

    // ── System metrics: process-scoped, with host stats under host_ gauges ──
    update_system_metrics().await;
    let enc = encode_metrics().unwrap();
    let rss = metric_value(&enc, "synap_process_memory_bytes", &["type=\"rss\""]);
    assert!(
        rss.is_some_and(|v| v >= 0),
        "process rss gauge missing:\n{enc}"
    );
    assert!(
        enc.contains("synap_process_cpu_usage_percent") && enc.contains("core=\"process\""),
        "process cpu should be labelled core=\"process\", not host load average",
    );
    assert!(
        enc.contains("synap_host_memory_bytes"),
        "host memory gauge missing",
    );
    assert!(
        enc.contains("synap_host_load_average"),
        "host load average gauge missing",
    );
}
