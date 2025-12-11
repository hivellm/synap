//! Prometheus Metrics for Synap
//!
//! Comprehensive metrics collection for all Synap components:
//! - KV Store operations
//! - Queue operations
//! - Stream operations
//! - Pub/Sub operations
//! - Replication metrics
//! - System metrics

use lazy_static::lazy_static;
use prometheus::{
    Encoder, HistogramVec, IntCounterVec, IntGaugeVec, TextEncoder, register_histogram_vec,
    register_int_counter_vec, register_int_gauge_vec,
};

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
