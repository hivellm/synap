//! Prometheus Metrics HTTP Handler

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use axum::{extract::State, http::StatusCode, response::IntoResponse};

use super::handlers::AppState;

/// GET /metrics - Prometheus metrics endpoint
pub async fn metrics_handler(State(state): State<AppState>) -> impl IntoResponse {
    // Refresh system + broker gauges before encoding so the scrape reflects
    // live state (per-process CPU/memory, stream length, consumer lag, …).
    update_system_metrics().await;
    update_broker_metrics(&state).await;

    match crate::metrics::encode_metrics() {
        Ok(metrics) => (
            StatusCode::OK,
            [("content-type", "text/plain; version=0.0.4")],
            metrics,
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to encode metrics: {}", e),
        )
            .into_response(),
    }
}

/// Per-process resource sampler. Held across scrapes so `cpu_usage()` can be
/// computed as a delta between consecutive refreshes of this process.
struct ProcSampler {
    sys: sysinfo::System,
    pid: Option<sysinfo::Pid>,
}

fn proc_sampler() -> &'static Mutex<ProcSampler> {
    static SAMPLER: OnceLock<Mutex<ProcSampler>> = OnceLock::new();
    SAMPLER.get_or_init(|| {
        Mutex::new(ProcSampler {
            sys: sysinfo::System::new(),
            pid: sysinfo::get_current_pid().ok(),
        })
    })
}

/// Initialize metrics with default values
pub fn init_metrics() {
    // Force initialization of all metrics by accessing them
    let _ = &*crate::metrics::KV_OPS_TOTAL;
    let _ = &*crate::metrics::KV_OP_DURATION;
    let _ = &*crate::metrics::KV_KEYS_TOTAL;
    let _ = &*crate::metrics::KV_MEMORY_BYTES;
    let _ = &*crate::metrics::QUEUE_OPS_TOTAL;
    let _ = &*crate::metrics::QUEUE_DEPTH;
    let _ = &*crate::metrics::QUEUE_OP_DURATION;
    let _ = &*crate::metrics::QUEUE_DLQ_TOTAL;
    let _ = &*crate::metrics::STREAM_OPS_TOTAL;
    let _ = &*crate::metrics::STREAM_EVENTS_TOTAL;
    let _ = &*crate::metrics::STREAM_SUBSCRIBERS;
    let _ = &*crate::metrics::STREAM_BUFFER_SIZE;
    let _ = &*crate::metrics::STREAM_LAST_OFFSET;
    let _ = &*crate::metrics::PARTITION_MESSAGES;
    let _ = &*crate::metrics::PARTITION_END_OFFSET;
    let _ = &*crate::metrics::CONSUMER_GROUP_MEMBERS;
    let _ = &*crate::metrics::CONSUMER_GROUP_COMMITTED_OFFSET;
    let _ = &*crate::metrics::CONSUMER_GROUP_LAG;
    let _ = &*crate::metrics::PUBSUB_OPS_TOTAL;
    let _ = &*crate::metrics::PUBSUB_MESSAGES_TOTAL;
    let _ = &*crate::metrics::PUBSUB_SUBSCRIPTIONS;
    let _ = &*crate::metrics::REPL_LAG;
    let _ = &*crate::metrics::REPL_OPS_TOTAL;
    let _ = &*crate::metrics::REPL_BYTES_TOTAL;
    let _ = &*crate::metrics::HTTP_REQUESTS_TOTAL;
    let _ = &*crate::metrics::HTTP_REQUEST_DURATION;
    let _ = &*crate::metrics::HTTP_CONNECTIONS;
    let _ = &*crate::metrics::PROCESS_MEMORY_BYTES;
    let _ = &*crate::metrics::PROCESS_CPU_USAGE;
    let _ = &*crate::metrics::HOST_MEMORY_BYTES;
    let _ = &*crate::metrics::HOST_LOAD_AVERAGE;

    tracing::info!("Prometheus metrics initialized");
}

/// Update process- and host-level system metrics (called on each scrape).
///
/// The `synap_process_*` gauges report **this process only**; the
/// `synap_host_*` gauges report the whole machine. This split fixes the prior
/// behaviour where `synap_process_cpu_usage_percent` was actually the host
/// load average and `synap_process_memory_bytes` was host memory.
pub async fn update_system_metrics() {
    // ── Per-process CPU + memory (this Synap process only) ──
    if let Ok(mut guard) = proc_sampler().lock()
        && let Some(pid) = guard.pid
    {
        guard.sys.refresh_processes_specifics(
            sysinfo::ProcessesToUpdate::Some(&[pid]),
            true,
            sysinfo::ProcessRefreshKind::everything(),
        );
        if let Some(proc_) = guard.sys.process(pid) {
            let rss = proc_.memory();
            let vmem = proc_.virtual_memory();
            let cpu = proc_.cpu_usage() as f64;
            crate::metrics::set_process_metrics(rss, vmem, cpu);
        }
    }

    // ── Host-wide memory + load average (whole machine) ──
    let (used, total) = sys_info::mem_info()
        .map(|m| {
            (
                m.total.saturating_sub(m.avail) as i64 * 1024,
                m.total as i64 * 1024,
            )
        })
        .unwrap_or((0, 0));
    let (l1, l5, l15) = sys_info::loadavg()
        .map(|l| (l.one, l.five, l.fifteen))
        .unwrap_or((0.0, 0.0, 0.0));
    crate::metrics::set_host_metrics(used, total, l1, l5, l15);
}

/// Populate the live broker-state gauges (stream length, consumer lag, queue
/// depth, …) from a fresh snapshot of the shared broker state on each scrape.
///
/// Gauges are reset first so streams/groups/queues that were deleted stop
/// reporting stale values.
pub async fn update_broker_metrics(state: &AppState) {
    crate::metrics::reset_broker_gauges();

    // ── Streams / rooms: buffered length, last offset, subscribers ──
    if let Some(sm) = &state.stream_manager {
        for room in sm.list_rooms().await {
            if let Ok(s) = sm.room_stats(&room).await {
                crate::metrics::set_stream_gauges(
                    &room,
                    s.message_count as i64,
                    s.max_offset as i64,
                    s.subscriber_count as i64,
                );
            }
        }
    }

    // ── Partitioned topics: messages + end offset (high-water-mark) per
    //    partition. The end offset also anchors consumer-group lag below. ──
    let mut partition_end: HashMap<(String, usize), u64> = HashMap::new();
    if let Some(pm) = &state.partition_manager {
        for topic in pm.list_topics().await {
            if let Ok(parts) = pm.topic_stats(&topic).await {
                for p in parts {
                    let partition = p.partition_id.to_string();
                    crate::metrics::set_partition_gauges(
                        &topic,
                        &partition,
                        p.message_count as i64,
                        p.max_offset as i64,
                    );
                    partition_end.insert((topic.clone(), p.partition_id), p.max_offset);
                }
            }
        }
    }

    // ── Consumer groups: members, committed offset, and lag =
    //    (last-published offset − committed offset), clamped at 0. ──
    if let Some(cg) = &state.consumer_group_manager {
        for group_id in cg.list_groups().await {
            let Ok(stats) = cg.group_stats(&group_id).await else {
                continue;
            };
            crate::metrics::set_consumer_group_members(
                &group_id,
                &stats.topic,
                stats.member_count as i64,
            );
            for partition in 0..stats.partition_count {
                let committed = cg.get_offset(&group_id, partition).await.ok().flatten();
                let end = partition_end
                    .get(&(stats.topic.clone(), partition))
                    .copied();
                let partition_label = partition.to_string();
                match (end, committed) {
                    // Topic is partition-backed: report committed (0 if never
                    // committed) and the resulting lag.
                    (Some(end_offset), committed) => {
                        let committed = committed.unwrap_or(0);
                        crate::metrics::set_consumer_group_partition(
                            &group_id,
                            &stats.topic,
                            &partition_label,
                            committed as i64,
                            end_offset.saturating_sub(committed) as i64,
                        );
                    }
                    // No high-water-mark available: report committed only.
                    (None, Some(committed)) => {
                        crate::metrics::set_consumer_group_partition(
                            &group_id,
                            &stats.topic,
                            &partition_label,
                            committed as i64,
                            0,
                        );
                    }
                    (None, None) => {}
                }
            }
        }
    }

    // ── Queues: ready depth + dead-letter count ──
    if let Some(qm) = &state.queue_manager
        && let Ok(queues) = qm.list_queues().await
    {
        for q in queues {
            if let Ok(s) = qm.stats(&q).await {
                crate::metrics::set_queue_gauges(&q, s.depth as i64, s.dead_lettered as i64);
            }
        }
    }

    // ── Per-datatype memory toward the shared maxmemory budget (audit M-018).
    //    The sum is the total the eviction/refusal path uses. ──
    crate::metrics::set_datatype_memory(
        "kv",
        state.kv_store.stats().await.total_memory_bytes.max(0),
    );
    crate::metrics::set_datatype_memory("hash", state.hash_store.memory_bytes() as i64);
    crate::metrics::set_datatype_memory("list", state.list_store.memory_bytes() as i64);
    crate::metrics::set_datatype_memory("set", state.set_store.memory_bytes() as i64);
    crate::metrics::set_datatype_memory("sorted_set", state.sorted_set_store.memory_bytes() as i64);
    if let Some(sm) = &state.stream_manager {
        crate::metrics::set_datatype_memory("stream", sm.memory_bytes() as i64);
    }
    if let Some(qm) = &state.queue_manager {
        crate::metrics::set_datatype_memory("queue", qm.memory_bytes() as i64);
    }
}
