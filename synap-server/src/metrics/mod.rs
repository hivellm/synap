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
    ).unwrap();

    /// KV operation latency in seconds
    pub static ref KV_OP_DURATION: HistogramVec = register_histogram_vec!(
        "synap_kv_operation_duration_seconds",
        "KV operation latency in seconds",
        &["operation"],
        vec![0.00001, 0.0001, 0.001, 0.01, 0.1, 1.0]
    ).unwrap();

    /// Current number of keys in store
    pub static ref KV_KEYS_TOTAL: IntGaugeVec = register_int_gauge_vec!(
        "synap_kv_keys_total",
        "Current number of keys in KV store",
        &["shard"]
    ).unwrap();

    /// Memory usage in bytes
    pub static ref KV_MEMORY_BYTES: IntGaugeVec = register_int_gauge_vec!(
        "synap_kv_memory_bytes",
        "Memory usage of KV store in bytes",
        &["type"]
    ).unwrap();

    // ============================================================================
    // Queue Metrics
    // ============================================================================

    /// Total queue operations
    pub static ref QUEUE_OPS_TOTAL: IntCounterVec = register_int_counter_vec!(
        "synap_queue_operations_total",
        "Total number of queue operations",
        &["queue", "operation", "status"]
    ).unwrap();

    /// Queue depth (pending messages)
    pub static ref QUEUE_DEPTH: IntGaugeVec = register_int_gauge_vec!(
        "synap_queue_depth",
        "Number of pending messages in queue",
        &["queue"]
    ).unwrap();

    /// Queue operation latency
    pub static ref QUEUE_OP_DURATION: HistogramVec = register_histogram_vec!(
        "synap_queue_operation_duration_seconds",
        "Queue operation latency in seconds",
        &["queue", "operation"],
        vec![0.0001, 0.001, 0.01, 0.1, 1.0, 10.0]
    ).unwrap();

    /// Messages in DLQ
    pub static ref QUEUE_DLQ_TOTAL: IntGaugeVec = register_int_gauge_vec!(
        "synap_queue_dlq_messages",
        "Number of messages in dead letter queue",
        &["queue"]
    ).unwrap();

    // ============================================================================
    // Stream Metrics
    // ============================================================================

    /// Total stream operations
    pub static ref STREAM_OPS_TOTAL: IntCounterVec = register_int_counter_vec!(
        "synap_stream_operations_total",
        "Total number of stream operations",
        &["room", "operation", "status"]
    ).unwrap();

    /// Stream events published
    pub static ref STREAM_EVENTS_TOTAL: IntCounterVec = register_int_counter_vec!(
        "synap_stream_events_total",
        "Total number of events published to streams",
        &["room", "event_type"]
    ).unwrap();

    /// Active subscribers
    pub static ref STREAM_SUBSCRIBERS: IntGaugeVec = register_int_gauge_vec!(
        "synap_stream_subscribers",
        "Number of active subscribers per stream",
        &["room"]
    ).unwrap();

    /// Stream buffer size
    pub static ref STREAM_BUFFER_SIZE: IntGaugeVec = register_int_gauge_vec!(
        "synap_stream_buffer_size",
        "Number of events in stream buffer",
        &["room"]
    ).unwrap();

    // ============================================================================
    // Pub/Sub Metrics
    // ============================================================================

    /// Total pub/sub operations
    pub static ref PUBSUB_OPS_TOTAL: IntCounterVec = register_int_counter_vec!(
        "synap_pubsub_operations_total",
        "Total number of pub/sub operations",
        &["operation", "status"]
    ).unwrap();

    /// Messages published to topics
    pub static ref PUBSUB_MESSAGES_TOTAL: IntCounterVec = register_int_counter_vec!(
        "synap_pubsub_messages_total",
        "Total messages published to topics",
        &["topic"]
    ).unwrap();

    /// Active subscriptions
    pub static ref PUBSUB_SUBSCRIPTIONS: IntGaugeVec = register_int_gauge_vec!(
        "synap_pubsub_subscriptions",
        "Number of active subscriptions",
        &["topic"]
    ).unwrap();

    // ============================================================================
    // Replication Metrics
    // ============================================================================

    /// Replication lag (offset difference)
    pub static ref REPL_LAG: IntGaugeVec = register_int_gauge_vec!(
        "synap_replication_lag_operations",
        "Replication lag in number of operations",
        &["replica_id"]
    ).unwrap();

    /// Replication throughput
    pub static ref REPL_OPS_TOTAL: IntCounterVec = register_int_counter_vec!(
        "synap_replication_operations_total",
        "Total replication operations",
        &["type", "status"]
    ).unwrap();

    /// Bytes transferred in replication
    pub static ref REPL_BYTES_TOTAL: IntCounterVec = register_int_counter_vec!(
        "synap_replication_bytes_total",
        "Total bytes transferred in replication",
        &["direction"]
    ).unwrap();

    // ============================================================================
    // HTTP Server Metrics
    // ============================================================================

    /// HTTP requests total
    pub static ref HTTP_REQUESTS_TOTAL: IntCounterVec = register_int_counter_vec!(
        "synap_http_requests_total",
        "Total HTTP requests",
        &["method", "path", "status"]
    ).unwrap();

    /// HTTP request duration
    pub static ref HTTP_REQUEST_DURATION: HistogramVec = register_histogram_vec!(
        "synap_http_request_duration_seconds",
        "HTTP request latency in seconds",
        &["method", "path"],
        vec![0.001, 0.01, 0.1, 1.0, 10.0]
    ).unwrap();

    /// Active connections
    pub static ref HTTP_CONNECTIONS: IntGaugeVec = register_int_gauge_vec!(
        "synap_http_connections",
        "Number of active HTTP connections",
        &["type"]
    ).unwrap();

    // ============================================================================
    // RESP3 TCP Protocol Metrics
    // ============================================================================

    /// Total RESP3 commands processed, labelled by command name and status.
    pub static ref RESP3_COMMANDS_TOTAL: IntCounterVec = register_int_counter_vec!(
        "synap_resp3_commands_total",
        "Total RESP3 commands processed",
        &["command", "status"]  // status: ok | err
    ).unwrap();

    /// RESP3 command latency from parse-complete to response-sent.
    pub static ref RESP3_COMMAND_DURATION: HistogramVec = register_histogram_vec!(
        "synap_resp3_command_duration_seconds",
        "RESP3 command dispatch latency in seconds",
        &["command"],
        PROTOCOL_LATENCY_BUCKETS.to_vec()
    ).unwrap();

    /// Currently open RESP3 TCP connections.
    pub static ref RESP3_CONNECTIONS: IntGaugeVec = register_int_gauge_vec!(
        "synap_resp3_connections",
        "Active RESP3 TCP connections",
        &["state"]  // state: active
    ).unwrap();

    /// Bytes received from RESP3 clients.
    pub static ref RESP3_BYTES_READ: IntCounterVec = register_int_counter_vec!(
        "synap_resp3_bytes_read_total",
        "Total bytes read from RESP3 clients",
        &[]
    ).unwrap();

    /// Bytes sent to RESP3 clients.
    pub static ref RESP3_BYTES_WRITTEN: IntCounterVec = register_int_counter_vec!(
        "synap_resp3_bytes_written_total",
        "Total bytes written to RESP3 clients",
        &[]
    ).unwrap();

    // ============================================================================
    // SynapRPC Binary Protocol Metrics
    // ============================================================================

    /// Total SynapRPC requests processed, labelled by command and status.
    pub static ref SYNAP_RPC_COMMANDS_TOTAL: IntCounterVec = register_int_counter_vec!(
        "synap_rpc_commands_total",
        "Total SynapRPC commands processed",
        &["command", "status"]  // status: ok | err
    ).unwrap();

    /// SynapRPC command latency from frame-received to frame-sent.
    pub static ref SYNAP_RPC_COMMAND_DURATION: HistogramVec = register_histogram_vec!(
        "synap_rpc_command_duration_seconds",
        "SynapRPC command dispatch latency in seconds",
        &["command"],
        PROTOCOL_LATENCY_BUCKETS.to_vec()
    ).unwrap();

    /// Currently open SynapRPC TCP connections.
    pub static ref SYNAP_RPC_CONNECTIONS: IntGaugeVec = register_int_gauge_vec!(
        "synap_rpc_connections",
        "Active SynapRPC TCP connections",
        &["state"]  // state: active
    ).unwrap();

    /// Incoming SynapRPC frame sizes in bytes.
    pub static ref SYNAP_RPC_FRAME_SIZE_IN: HistogramVec = register_histogram_vec!(
        "synap_rpc_frame_size_bytes_in",
        "SynapRPC incoming frame sizes in bytes",
        &[],
        vec![64.0, 128.0, 256.0, 512.0, 1024.0, 4096.0, 16384.0, 65536.0]
    ).unwrap();

    /// Outgoing SynapRPC frame sizes in bytes.
    pub static ref SYNAP_RPC_FRAME_SIZE_OUT: HistogramVec = register_histogram_vec!(
        "synap_rpc_frame_size_bytes_out",
        "SynapRPC outgoing frame sizes in bytes",
        &[],
        vec![64.0, 128.0, 256.0, 512.0, 1024.0, 4096.0, 16384.0, 65536.0]
    ).unwrap();

    // ============================================================================
    // System Metrics
    // ============================================================================

    /// Process memory usage
    pub static ref PROCESS_MEMORY_BYTES: IntGaugeVec = register_int_gauge_vec!(
        "synap_process_memory_bytes",
        "Process memory usage in bytes",
        &["type"]
    ).unwrap();

    /// Process CPU usage (percentage * 100)
    pub static ref PROCESS_CPU_USAGE: IntGaugeVec = register_int_gauge_vec!(
        "synap_process_cpu_usage_percent",
        "Process CPU usage percentage",
        &["core"]
    ).unwrap();
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

// ── RESP3 helpers ─────────────────────────────────────────────────────────────

/// Record one RESP3 command completion.
pub fn record_resp3_command(command: &str, ok: bool, duration_secs: f64) {
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
        if let Some(threshold) = self.slow_threshold_secs {
            if elapsed > threshold {
                tracing::warn!(
                    command = self.label,
                    elapsed_ms = elapsed * 1_000.0,
                    threshold_ms = threshold * 1_000.0,
                    "slow command detected"
                );
            }
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

        let metrics = encode_metrics().unwrap();
        assert!(metrics.contains("synap_kv_operations_total"));
        assert!(metrics.contains("synap_kv_operation_duration_seconds"));
    }

    #[test]
    fn test_record_queue_op() {
        record_queue_op("test-queue", "publish", "success", 0.005);

        let metrics = encode_metrics().unwrap();
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
}
