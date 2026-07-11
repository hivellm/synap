//! Prometheus Metrics for Synap
//!
//! Comprehensive metrics collection for all Synap components:
//! - KV Store operations
//! - Queue operations
//! - Stream operations
//! - Pub/Sub operations
//! - Replication metrics
//! - System metrics
//! - RESP3 TCP protocol
//! - SynapRPC binary protocol

use std::time::Instant;

use lazy_static::lazy_static;
use prometheus::{
    Encoder, HistogramVec, IntCounterVec, IntGaugeVec, TextEncoder, register_histogram_vec,
    register_int_counter_vec, register_int_gauge_vec,
};

// Sub-millisecond buckets for TCP protocol latency (µs–ms range).
const PROTOCOL_LATENCY_BUCKETS: &[f64] = &[
    0.000_025, // 25 µs
    0.000_050, // 50 µs
    0.000_100, // 100 µs
    0.000_250, // 250 µs
    0.000_500, // 500 µs
    0.001_000, // 1 ms
    0.002_500, // 2.5 ms
    0.005_000, // 5 ms
    0.010_000, // 10 ms
    0.050_000, // 50 ms
];

lazy_static! {
    // ============================================================================
    // KV Store Metrics
    // ============================================================================

    /// Total KV operations by type (get, set, delete, scan)
    pub static ref KV_OPS_TOTAL: IntCounterVec = register_int_counter_vec!(
        "synap_kv_operations_total",
        "Total number of KV operations by type",
        &["operation", "status"]
    ).expect("metric registration uses a static, unique name");

    /// KV operation latency in seconds
    pub static ref KV_OP_DURATION: HistogramVec = register_histogram_vec!(
        "synap_kv_operation_duration_seconds",
        "KV operation latency in seconds",
        &["operation"],
        vec![0.00001, 0.0001, 0.001, 0.01, 0.1, 1.0]
    ).expect("metric registration uses a static, unique name");

    /// Current number of keys in store
    pub static ref KV_KEYS_TOTAL: IntGaugeVec = register_int_gauge_vec!(
        "synap_kv_keys_total",
        "Current number of keys in KV store",
        &["shard"]
    ).expect("metric registration uses a static, unique name");

    /// Memory usage in bytes
    pub static ref KV_MEMORY_BYTES: IntGaugeVec = register_int_gauge_vec!(
        "synap_kv_memory_bytes",
        "Memory usage of KV store in bytes",
        &["type"]
    ).expect("metric registration uses a static, unique name");

    /// Accounted memory per datatype toward the shared `maxmemory` budget (audit
    /// M-018). The sum across datatypes is the total the eviction/refusal path uses.
    pub static ref DATATYPE_MEMORY_BYTES: IntGaugeVec = register_int_gauge_vec!(
        "synap_datatype_memory_bytes",
        "Accounted memory in bytes per datatype toward the shared maxmemory budget",
        &["datatype"]
    ).expect("metric registration uses a static, unique name");

    // ============================================================================
    // Queue Metrics
    // ============================================================================

    /// Total queue operations
    pub static ref QUEUE_OPS_TOTAL: IntCounterVec = register_int_counter_vec!(
        "synap_queue_operations_total",
        "Total number of queue operations",
        &["queue", "operation", "status"]
    ).expect("metric registration uses a static, unique name");

    /// Queue depth (pending messages)
    pub static ref QUEUE_DEPTH: IntGaugeVec = register_int_gauge_vec!(
        "synap_queue_depth",
        "Number of pending messages in queue",
        &["queue"]
    ).expect("metric registration uses a static, unique name");

    /// Queue operation latency
    pub static ref QUEUE_OP_DURATION: HistogramVec = register_histogram_vec!(
        "synap_queue_operation_duration_seconds",
        "Queue operation latency in seconds",
        &["queue", "operation"],
        vec![0.0001, 0.001, 0.01, 0.1, 1.0, 10.0]
    ).expect("metric registration uses a static, unique name");

    /// Messages in DLQ
    pub static ref QUEUE_DLQ_TOTAL: IntGaugeVec = register_int_gauge_vec!(
        "synap_queue_dlq_messages",
        "Number of messages in dead letter queue",
        &["queue"]
    ).expect("metric registration uses a static, unique name");

    // ============================================================================
    // Stream Metrics
    // ============================================================================

    /// Total stream operations
    pub static ref STREAM_OPS_TOTAL: IntCounterVec = register_int_counter_vec!(
        "synap_stream_operations_total",
        "Total number of stream operations",
        &["room", "operation", "status"]
    ).expect("metric registration uses a static, unique name");

    /// Stream events published
    pub static ref STREAM_EVENTS_TOTAL: IntCounterVec = register_int_counter_vec!(
        "synap_stream_events_total",
        "Total number of events published to streams",
        &["room", "event_type"]
    ).expect("metric registration uses a static, unique name");

    /// Active subscribers
    pub static ref STREAM_SUBSCRIBERS: IntGaugeVec = register_int_gauge_vec!(
        "synap_stream_subscribers",
        "Number of active subscribers per stream",
        &["room"]
    ).expect("metric registration uses a static, unique name");

    /// Stream buffer size
    pub static ref STREAM_BUFFER_SIZE: IntGaugeVec = register_int_gauge_vec!(
        "synap_stream_buffer_size",
        "Number of events in stream buffer",
        &["room"]
    ).expect("metric registration uses a static, unique name");

    // ============================================================================
    // Pub/Sub Metrics
    // ============================================================================

    /// Total pub/sub operations
    pub static ref PUBSUB_OPS_TOTAL: IntCounterVec = register_int_counter_vec!(
        "synap_pubsub_operations_total",
        "Total number of pub/sub operations",
        &["operation", "status"]
    ).expect("metric registration uses a static, unique name");

    /// Messages published to topics
    pub static ref PUBSUB_MESSAGES_TOTAL: IntCounterVec = register_int_counter_vec!(
        "synap_pubsub_messages_total",
        "Total messages published to topics",
        &["topic"]
    ).expect("metric registration uses a static, unique name");

    /// Active subscriptions
    pub static ref PUBSUB_SUBSCRIPTIONS: IntGaugeVec = register_int_gauge_vec!(
        "synap_pubsub_subscriptions",
        "Number of active subscriptions",
        &["topic"]
    ).expect("metric registration uses a static, unique name");

    // ============================================================================
    // Replication Metrics
    // ============================================================================

    /// Replication lag (offset difference)
    pub static ref REPL_LAG: IntGaugeVec = register_int_gauge_vec!(
        "synap_replication_lag_operations",
        "Replication lag in number of operations",
        &["replica_id"]
    ).expect("metric registration uses a static, unique name");

    /// Replication throughput
    pub static ref REPL_OPS_TOTAL: IntCounterVec = register_int_counter_vec!(
        "synap_replication_operations_total",
        "Total replication operations",
        &["type", "status"]
    ).expect("metric registration uses a static, unique name");

    /// Bytes transferred in replication
    pub static ref REPL_BYTES_TOTAL: IntCounterVec = register_int_counter_vec!(
        "synap_replication_bytes_total",
        "Total bytes transferred in replication",
        &["direction"]
    ).expect("metric registration uses a static, unique name");

    // ============================================================================
    // HTTP Server Metrics
    // ============================================================================

    /// HTTP requests total
    pub static ref HTTP_REQUESTS_TOTAL: IntCounterVec = register_int_counter_vec!(
        "synap_http_requests_total",
        "Total HTTP requests",
        &["method", "path", "status"]
    ).expect("metric registration uses a static, unique name");

    /// HTTP request duration
    pub static ref HTTP_REQUEST_DURATION: HistogramVec = register_histogram_vec!(
        "synap_http_request_duration_seconds",
        "HTTP request latency in seconds",
        &["method", "path"],
        vec![0.001, 0.01, 0.1, 1.0, 10.0]
    ).expect("metric registration uses a static, unique name");

    /// Active connections
    pub static ref HTTP_CONNECTIONS: IntGaugeVec = register_int_gauge_vec!(
        "synap_http_connections",
        "Number of active HTTP connections",
        &["type"]
    ).expect("metric registration uses a static, unique name");

    // ============================================================================
    // RESP3 TCP Protocol Metrics
    // ============================================================================

    /// Total RESP3 commands processed, labelled by command name and status.
    pub static ref RESP3_COMMANDS_TOTAL: IntCounterVec = register_int_counter_vec!(
        "synap_resp3_commands_total",
        "Total RESP3 commands processed",
        &["command", "status"]  // status: ok | err
    ).expect("metric registration uses a static, unique name");

    /// RESP3 command latency from parse-complete to response-sent.
    pub static ref RESP3_COMMAND_DURATION: HistogramVec = register_histogram_vec!(
        "synap_resp3_command_duration_seconds",
        "RESP3 command dispatch latency in seconds",
        &["command"],
        PROTOCOL_LATENCY_BUCKETS.to_vec()
    ).expect("metric registration uses a static, unique name");

    /// Currently open RESP3 TCP connections.
    pub static ref RESP3_CONNECTIONS: IntGaugeVec = register_int_gauge_vec!(
        "synap_resp3_connections",
        "Active RESP3 TCP connections",
        &["state"]  // state: active
    ).expect("metric registration uses a static, unique name");

    /// Bytes received from RESP3 clients.
    pub static ref RESP3_BYTES_READ: IntCounterVec = register_int_counter_vec!(
        "synap_resp3_bytes_read_total",
        "Total bytes read from RESP3 clients",
        &[]
    ).expect("metric registration uses a static, unique name");

    /// Bytes sent to RESP3 clients.
    pub static ref RESP3_BYTES_WRITTEN: IntCounterVec = register_int_counter_vec!(
        "synap_resp3_bytes_written_total",
        "Total bytes written to RESP3 clients",
        &[]
    ).expect("metric registration uses a static, unique name");

    // ============================================================================
    // SynapRPC Binary Protocol Metrics
    // ============================================================================

    /// Total SynapRPC requests processed, labelled by command and status.
    pub static ref SYNAP_RPC_COMMANDS_TOTAL: IntCounterVec = register_int_counter_vec!(
        "synap_rpc_commands_total",
        "Total SynapRPC commands processed",
        &["command", "status"]  // status: ok | err
    ).expect("metric registration uses a static, unique name");

    /// SynapRPC command latency from frame-received to frame-sent.
    pub static ref SYNAP_RPC_COMMAND_DURATION: HistogramVec = register_histogram_vec!(
        "synap_rpc_command_duration_seconds",
        "SynapRPC command dispatch latency in seconds",
        &["command"],
        PROTOCOL_LATENCY_BUCKETS.to_vec()
    ).expect("metric registration uses a static, unique name");

    /// Currently open SynapRPC TCP connections.
    pub static ref SYNAP_RPC_CONNECTIONS: IntGaugeVec = register_int_gauge_vec!(
        "synap_rpc_connections",
        "Active SynapRPC TCP connections",
        &["state"]  // state: active
    ).expect("metric registration uses a static, unique name");

    /// Incoming SynapRPC frame sizes in bytes.
    pub static ref SYNAP_RPC_FRAME_SIZE_IN: HistogramVec = register_histogram_vec!(
        "synap_rpc_frame_size_bytes_in",
        "SynapRPC incoming frame sizes in bytes",
        &[],
        vec![64.0, 128.0, 256.0, 512.0, 1024.0, 4096.0, 16384.0, 65536.0]
    ).expect("metric registration uses a static, unique name");

    /// Outgoing SynapRPC frame sizes in bytes.
    pub static ref SYNAP_RPC_FRAME_SIZE_OUT: HistogramVec = register_histogram_vec!(
        "synap_rpc_frame_size_bytes_out",
        "SynapRPC outgoing frame sizes in bytes",
        &[],
        vec![64.0, 128.0, 256.0, 512.0, 1024.0, 4096.0, 16384.0, 65536.0]
    ).expect("metric registration uses a static, unique name");

    // ============================================================================
    // System Metrics
    // ============================================================================

    /// Process memory usage in bytes (this Synap process only).
    /// `type`: "rss" (resident set size) | "virtual".
    pub static ref PROCESS_MEMORY_BYTES: IntGaugeVec = register_int_gauge_vec!(
        "synap_process_memory_bytes",
        "Memory usage of the Synap process in bytes (rss/virtual)",
        &["type"]
    ).expect("metric registration uses a static, unique name");

    /// CPU usage of the Synap process as a percentage (100 = one full core).
    /// `core`: "process".  Populated from a per-process sampler, NOT host load.
    pub static ref PROCESS_CPU_USAGE: IntGaugeVec = register_int_gauge_vec!(
        "synap_process_cpu_usage_percent",
        "CPU usage of the Synap process as a percentage (100 = one core)",
        &["core"]
    ).expect("metric registration uses a static, unique name");

    // ============================================================================
    // Host Metrics (whole machine — distinct from the per-process gauges above)
    // ============================================================================

    /// Host (whole machine) memory in bytes. `type`: "used" | "total".
    pub static ref HOST_MEMORY_BYTES: IntGaugeVec = register_int_gauge_vec!(
        "synap_host_memory_bytes",
        "Host machine memory in bytes (used/total)",
        &["type"]
    ).expect("metric registration uses a static, unique name");

    /// Host load average * 100. `window`: "1min" | "5min" | "15min".
    pub static ref HOST_LOAD_AVERAGE: IntGaugeVec = register_int_gauge_vec!(
        "synap_host_load_average",
        "Host load average multiplied by 100 (1min/5min/15min)",
        &["window"]
    ).expect("metric registration uses a static, unique name");

    // ============================================================================
    // Broker State Gauges (populated live on each /metrics scrape)
    // ============================================================================

    /// Last (highest) offset published to a stream/room — a stream-length signal.
    pub static ref STREAM_LAST_OFFSET: IntGaugeVec = register_int_gauge_vec!(
        "synap_stream_last_offset",
        "Last published offset per stream/room",
        &["room"]
    ).expect("metric registration uses a static, unique name");

    /// Number of events currently buffered in a partition.
    pub static ref PARTITION_MESSAGES: IntGaugeVec = register_int_gauge_vec!(
        "synap_partition_messages",
        "Number of events currently buffered per topic partition",
        &["topic", "partition"]
    ).expect("metric registration uses a static, unique name");

    /// High-water-mark (last published offset) per topic partition.
    pub static ref PARTITION_END_OFFSET: IntGaugeVec = register_int_gauge_vec!(
        "synap_partition_end_offset",
        "High-water-mark (last published offset) per topic partition",
        &["topic", "partition"]
    ).expect("metric registration uses a static, unique name");

    /// Active members in a consumer group.
    pub static ref CONSUMER_GROUP_MEMBERS: IntGaugeVec = register_int_gauge_vec!(
        "synap_consumer_group_members",
        "Number of active members per consumer group",
        &["group", "topic"]
    ).expect("metric registration uses a static, unique name");

    /// Last committed (acked) offset per consumer group + partition.
    pub static ref CONSUMER_GROUP_COMMITTED_OFFSET: IntGaugeVec = register_int_gauge_vec!(
        "synap_consumer_group_committed_offset",
        "Last committed (acked) offset per consumer group and partition",
        &["group", "topic", "partition"]
    ).expect("metric registration uses a static, unique name");

    /// Consumer lag = partition high-water-mark − committed offset.
    pub static ref CONSUMER_GROUP_LAG: IntGaugeVec = register_int_gauge_vec!(
        "synap_consumer_group_lag",
        "Consumer lag (end offset − committed offset) per group and partition",
        &["group", "topic", "partition"]
    ).expect("metric registration uses a static, unique name");
}

/// Encode all metrics to Prometheus text format
pub fn encode_metrics() -> Result<String, Box<dyn std::error::Error>> {
    let encoder = TextEncoder::new();
    let metric_families = prometheus::gather();
    let mut buffer = vec![];
    encoder.encode(&metric_families, &mut buffer)?;
    Ok(String::from_utf8(buffer)?)
}

/// Record KV operation
pub fn record_kv_op(operation: &str, status: &str, duration_secs: f64) {
    KV_OPS_TOTAL.with_label_values(&[operation, status]).inc();
    KV_OP_DURATION
        .with_label_values(&[operation])
        .observe(duration_secs);
}

/// Record queue operation
pub fn record_queue_op(queue: &str, operation: &str, status: &str, duration_secs: f64) {
    QUEUE_OPS_TOTAL
        .with_label_values(&[queue, operation, status])
        .inc();
    QUEUE_OP_DURATION
        .with_label_values(&[queue, operation])
        .observe(duration_secs);
}

/// Record stream operation
pub fn record_stream_op(room: &str, operation: &str, status: &str) {
    STREAM_OPS_TOTAL
        .with_label_values(&[room, operation, status])
        .inc();
}

/// Record stream event
pub fn record_stream_event(room: &str, event_type: &str) {
    STREAM_EVENTS_TOTAL
        .with_label_values(&[room, event_type])
        .inc();
}

/// Record pub/sub operation
pub fn record_pubsub_op(operation: &str, status: &str) {
    PUBSUB_OPS_TOTAL
        .with_label_values(&[operation, status])
        .inc();
}

/// Record pub/sub message
pub fn record_pubsub_message(topic: &str) {
    PUBSUB_MESSAGES_TOTAL.with_label_values(&[topic]).inc();
}

/// Record HTTP request
pub fn record_http_request(method: &str, path: &str, status: u16, duration_secs: f64) {
    HTTP_REQUESTS_TOTAL
        .with_label_values(&[method, path, &status.to_string()])
        .inc();
    HTTP_REQUEST_DURATION
        .with_label_values(&[method, path])
        .observe(duration_secs);
}

/// Update replication lag
pub fn update_replication_lag(replica_id: &str, lag: i64) {
    REPL_LAG.with_label_values(&[replica_id]).set(lag);
}

/// Record replication operation
pub fn record_replication_op(op_type: &str, status: &str, bytes: u64) {
    REPL_OPS_TOTAL.with_label_values(&[op_type, status]).inc();
    REPL_BYTES_TOTAL.with_label_values(&["sent"]).inc_by(bytes);
}

// ── System / host / process snapshot helpers ──────────────────────────────────

/// Set the per-process CPU + memory gauges (this Synap process only).
/// `cpu_percent` is scaled where 100 = one full core.
pub fn set_process_metrics(rss_bytes: u64, virtual_bytes: u64, cpu_percent: f64) {
    PROCESS_MEMORY_BYTES
        .with_label_values(&["rss"])
        .set(rss_bytes as i64);
    PROCESS_MEMORY_BYTES
        .with_label_values(&["virtual"])
        .set(virtual_bytes as i64);
    PROCESS_CPU_USAGE
        .with_label_values(&["process"])
        .set(cpu_percent.round() as i64);
}

/// Set the whole-machine (host) memory + load-average gauges.
pub fn set_host_metrics(used_bytes: i64, total_bytes: i64, load1: f64, load5: f64, load15: f64) {
    HOST_MEMORY_BYTES
        .with_label_values(&["used"])
        .set(used_bytes);
    HOST_MEMORY_BYTES
        .with_label_values(&["total"])
        .set(total_bytes);
    HOST_LOAD_AVERAGE
        .with_label_values(&["1min"])
        .set((load1 * 100.0) as i64);
    HOST_LOAD_AVERAGE
        .with_label_values(&["5min"])
        .set((load5 * 100.0) as i64);
    HOST_LOAD_AVERAGE
        .with_label_values(&["15min"])
        .set((load15 * 100.0) as i64);
}

// ── Broker-state snapshot helpers (populated live on each /metrics scrape) ─────

/// Clear all live broker-state gauges before repopulating them from a fresh
/// snapshot, so streams/groups/queues that were deleted stop reporting stale
/// values.
pub fn reset_broker_gauges() {
    STREAM_BUFFER_SIZE.reset();
    STREAM_SUBSCRIBERS.reset();
    STREAM_LAST_OFFSET.reset();
    PARTITION_MESSAGES.reset();
    PARTITION_END_OFFSET.reset();
    CONSUMER_GROUP_MEMBERS.reset();
    CONSUMER_GROUP_COMMITTED_OFFSET.reset();
    CONSUMER_GROUP_LAG.reset();
    QUEUE_DEPTH.reset();
    QUEUE_DLQ_TOTAL.reset();
    DATATYPE_MEMORY_BYTES.reset();
}

/// Set the accounted memory (bytes) for one datatype (audit M-018).
pub fn set_datatype_memory(datatype: &str, bytes: i64) {
    DATATYPE_MEMORY_BYTES
        .with_label_values(&[datatype])
        .set(bytes);
}

/// Set the live gauges for one stream/room.
pub fn set_stream_gauges(room: &str, message_count: i64, last_offset: i64, subscribers: i64) {
    STREAM_BUFFER_SIZE
        .with_label_values(&[room])
        .set(message_count);
    STREAM_LAST_OFFSET
        .with_label_values(&[room])
        .set(last_offset);
    STREAM_SUBSCRIBERS
        .with_label_values(&[room])
        .set(subscribers);
}

/// Set the live gauges for one topic partition.
pub fn set_partition_gauges(topic: &str, partition: &str, messages: i64, end_offset: i64) {
    PARTITION_MESSAGES
        .with_label_values(&[topic, partition])
        .set(messages);
    PARTITION_END_OFFSET
        .with_label_values(&[topic, partition])
        .set(end_offset);
}

/// Set the member-count gauge for one consumer group.
pub fn set_consumer_group_members(group: &str, topic: &str, members: i64) {
    CONSUMER_GROUP_MEMBERS
        .with_label_values(&[group, topic])
        .set(members);
}

/// Set the committed-offset + lag gauges for one consumer group partition.
pub fn set_consumer_group_partition(
    group: &str,
    topic: &str,
    partition: &str,
    committed: i64,
    lag: i64,
) {
    CONSUMER_GROUP_COMMITTED_OFFSET
        .with_label_values(&[group, topic, partition])
        .set(committed);
    CONSUMER_GROUP_LAG
        .with_label_values(&[group, topic, partition])
        .set(lag);
}

/// Set the depth + dead-letter gauges for one queue.
pub fn set_queue_gauges(queue: &str, depth: i64, dlq: i64) {
    QUEUE_DEPTH.with_label_values(&[queue]).set(depth);
    QUEUE_DLQ_TOTAL.with_label_values(&[queue]).set(dlq);
}

// ── RESP3 helpers ─────────────────────────────────────────────────────────────

/// Pre-resolved metric handles for one command (`with_label_values` children are
/// cheap clonable Arcs). Resolving per call costs a label hash + `RwLock` read
/// inside prometheus ×3 per command; caching turns the hot path into one
/// lock-free HashMap lookup + plain atomic bumps.
struct CmdMetricHandles {
    ok: prometheus::IntCounter,
    err: prometheus::IntCounter,
    duration: prometheus::Histogram,
}

/// Commands worth pre-resolving (the hot set). Anything else falls back to the
/// dynamic prometheus lookup — same behaviour, just slower.
const HOT_COMMANDS: &[&str] = &[
    "GET",
    "SET",
    "DEL",
    "EXISTS",
    "INCR",
    "DECR",
    "INCRBY",
    "DECRBY",
    "MGET",
    "MSET",
    "EXPIRE",
    "TTL",
    "PING",
    "LPUSH",
    "RPUSH",
    "LPOP",
    "RPOP",
    "LRANGE",
    "LLEN",
    "SADD",
    "SREM",
    "SISMEMBER",
    "SMEMBERS",
    "SCARD",
    "SPOP",
    "HSET",
    "HGET",
    "HDEL",
    "HGETALL",
    "ZADD",
    "ZRANGE",
    "ZSCORE",
    "ZREM",
    "APPEND",
    "GETSET",
    "STRLEN",
];

fn hot_handles(
    counters: &prometheus::IntCounterVec,
    durations: &prometheus::HistogramVec,
) -> std::collections::HashMap<&'static str, CmdMetricHandles> {
    HOT_COMMANDS
        .iter()
        .map(|&cmd| {
            (
                cmd,
                CmdMetricHandles {
                    ok: counters.with_label_values(&[cmd, "ok"]),
                    err: counters.with_label_values(&[cmd, "err"]),
                    duration: durations.with_label_values(&[cmd]),
                },
            )
        })
        .collect()
}

static RESP3_CMD_HANDLES: std::sync::OnceLock<
    std::collections::HashMap<&'static str, CmdMetricHandles>,
> = std::sync::OnceLock::new();

/// Record one RESP3 command completion.
pub fn record_resp3_command(command: &str, ok: bool, duration_secs: f64) {
    let cache = RESP3_CMD_HANDLES
        .get_or_init(|| hot_handles(&RESP3_COMMANDS_TOTAL, &RESP3_COMMAND_DURATION));
    if let Some(h) = cache.get(command) {
        if ok {
            h.ok.inc()
        } else {
            h.err.inc()
        }
        h.duration.observe(duration_secs);
        return;
    }
    let status = if ok { "ok" } else { "err" };
    RESP3_COMMANDS_TOTAL
        .with_label_values(&[command, status])
        .inc();
    RESP3_COMMAND_DURATION
        .with_label_values(&[command])
        .observe(duration_secs);
}

/// Track an opened RESP3 connection (call on accept).
pub fn resp3_connection_open() {
    RESP3_CONNECTIONS.with_label_values(&["active"]).inc();
}

/// Track a closed RESP3 connection (call on disconnect).
pub fn resp3_connection_close() {
    RESP3_CONNECTIONS.with_label_values(&["active"]).dec();
}

/// Record bytes flowing through RESP3.
pub fn resp3_bytes(read: usize, written: usize) {
    if read > 0 {
        RESP3_BYTES_READ
            .with_label_values(&[] as &[&str; 0])
            .inc_by(read as u64);
    }
    if written > 0 {
        RESP3_BYTES_WRITTEN
            .with_label_values(&[] as &[&str; 0])
            .inc_by(written as u64);
    }
}

// ── SynapRPC helpers ──────────────────────────────────────────────────────────

/// Record one SynapRPC command completion.
pub fn record_synap_rpc_command(command: &str, ok: bool, duration_secs: f64) {
    static HANDLES: std::sync::OnceLock<std::collections::HashMap<&'static str, CmdMetricHandles>> =
        std::sync::OnceLock::new();
    let cache =
        HANDLES.get_or_init(|| hot_handles(&SYNAP_RPC_COMMANDS_TOTAL, &SYNAP_RPC_COMMAND_DURATION));
    if let Some(h) = cache.get(command) {
        if ok {
            h.ok.inc()
        } else {
            h.err.inc()
        }
        h.duration.observe(duration_secs);
        return;
    }
    let status = if ok { "ok" } else { "err" };
    SYNAP_RPC_COMMANDS_TOTAL
        .with_label_values(&[command, status])
        .inc();
    SYNAP_RPC_COMMAND_DURATION
        .with_label_values(&[command])
        .observe(duration_secs);
}

/// Track an opened SynapRPC connection.
pub fn synap_rpc_connection_open() {
    SYNAP_RPC_CONNECTIONS.with_label_values(&["active"]).inc();
}

/// Track a closed SynapRPC connection.
pub fn synap_rpc_connection_close() {
    SYNAP_RPC_CONNECTIONS.with_label_values(&["active"]).dec();
}

/// Record incoming + outgoing SynapRPC frame sizes.
pub fn synap_rpc_frame_sizes(in_bytes: usize, out_bytes: usize) {
    if in_bytes > 0 {
        SYNAP_RPC_FRAME_SIZE_IN
            .with_label_values(&[] as &[&str; 0])
            .observe(in_bytes as f64);
    }
    if out_bytes > 0 {
        SYNAP_RPC_FRAME_SIZE_OUT
            .with_label_values(&[] as &[&str; 0])
            .observe(out_bytes as f64);
    }
}

// ── PerfTimer — drop-based latency recorder ───────────────────────────────────

/// Zero-overhead RAII timer.  On drop it records elapsed time to a
/// `HistogramVec` and optionally emits a WARN log when the threshold is exceeded.
///
/// # Example
/// ```ignore
/// let _t = PerfTimer::new(&RESP3_COMMAND_DURATION, &["GET"], 1.0);
/// // … do work …
/// // timer records on drop
/// ```
pub struct PerfTimer<'a> {
    start: Instant,
    histogram: &'a HistogramVec,
    label: &'a str,
    /// Emit a WARN log if elapsed > this many seconds.  `None` = never.
    slow_threshold_secs: Option<f64>,
}

impl<'a> PerfTimer<'a> {
    /// Create a new timer.  `slow_ms` — if >0, log a warning when elapsed exceeds it.
    pub fn new(histogram: &'a HistogramVec, label: &'a str, slow_ms: f64) -> Self {
        Self {
            start: Instant::now(),
            histogram,
            label,
            slow_threshold_secs: if slow_ms > 0.0 {
                Some(slow_ms / 1_000.0)
            } else {
                None
            },
        }
    }

    /// Elapsed time so far (without recording).
    pub fn elapsed_secs(&self) -> f64 {
        self.start.elapsed().as_secs_f64()
    }
}

impl Drop for PerfTimer<'_> {
    fn drop(&mut self) {
        let elapsed = self.start.elapsed().as_secs_f64();
        self.histogram
            .with_label_values(&[self.label])
            .observe(elapsed);
        if let Some(threshold) = self.slow_threshold_secs
            && elapsed > threshold
        {
            tracing::warn!(
                command = self.label,
                elapsed_ms = elapsed * 1_000.0,
                threshold_ms = threshold * 1_000.0,
                "slow command detected"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_record_kv_op() {
        record_kv_op("get", "success", 0.001);
        record_kv_op("set", "success", 0.002);

        let metrics = encode_metrics().expect("metric registration uses a static, unique name");
        assert!(metrics.contains("synap_kv_operations_total"));
        assert!(metrics.contains("synap_kv_operation_duration_seconds"));
    }

    #[test]
    fn test_record_queue_op() {
        record_queue_op("test-queue", "publish", "success", 0.005);

        let metrics = encode_metrics().expect("metric registration uses a static, unique name");
        assert!(metrics.contains("synap_queue_operations_total"));
    }

    #[test]
    fn test_encode_metrics() {
        // Record some metrics first
        record_kv_op("get", "success", 0.001);

        let result = encode_metrics();
        assert!(result.is_ok());

        let metrics = result.unwrap();
        // Should contain Prometheus format (may be empty if no metrics recorded)
        assert!(!metrics.is_empty() || metrics.contains("# HELP"));
    }

    #[test]
    fn test_all_recording_functions_are_exposed() {
        // Exercise every record/set/gauge helper so the metric families register
        // and appear in the exposition output.
        record_stream_op("room", "publish", "success");
        record_stream_event("room", "evt");
        record_pubsub_op("publish", "success");
        record_pubsub_message("topic");
        record_http_request("GET", "/api/kv", 200, 0.003);
        update_replication_lag("replica-1", 5);
        record_replication_op("write", "success", 1024);
        set_process_metrics(1_000_000, 2_000_000, 12.5);
        set_host_metrics(500, 1000, 0.1, 0.2, 0.3);
        set_datatype_memory("hash", 4096);
        set_stream_gauges("room", 10, 9, 2);
        set_partition_gauges("topic", "0", 100, 99);
        set_consumer_group_members("g", "topic", 3);
        set_queue_gauges("q", 7, 1);
        record_resp3_command("GET", true, 0.001);
        resp3_connection_open();
        resp3_bytes(64, 128);
        resp3_connection_close();
        record_synap_rpc_command("GET", true, 0.001);
        synap_rpc_connection_open();
        synap_rpc_frame_sizes(32, 48);
        synap_rpc_connection_close();

        // reset then repopulate broker gauges (scrape-time snapshot pattern).
        reset_broker_gauges();
        set_stream_gauges("room", 1, 0, 1);
        set_datatype_memory("hash", 4096);

        let out = encode_metrics().unwrap();
        assert!(out.contains("synap_http_requests_total"));
        assert!(out.contains("synap_resp3_commands_total"));
        assert!(out.contains("synap_process_memory_bytes"));
        assert!(out.contains("synap_datatype_memory_bytes"));
    }
}
