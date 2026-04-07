//! Prometheus Metrics HTTP Handler

use axum::{http::StatusCode, response::IntoResponse};

/// GET /metrics - Prometheus metrics endpoint
pub async fn metrics_handler() -> impl IntoResponse {
    // Update system metrics before encoding
    update_system_metrics().await;

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

    tracing::info!("Prometheus metrics initialized (17 metric types registered)");
}

/// Update system metrics (called periodically)
pub async fn update_system_metrics() {
    // Update process memory
    if let Ok(usage) = sys_info::mem_info() {
        crate::metrics::PROCESS_MEMORY_BYTES
            .with_label_values(&["used"])
            .set((usage.total - usage.avail) as i64 * 1024);
        crate::metrics::PROCESS_MEMORY_BYTES
            .with_label_values(&["total"])
            .set(usage.total as i64 * 1024);
    }

    // Update CPU usage
    if let Ok(load) = sys_info::loadavg() {
        crate::metrics::PROCESS_CPU_USAGE
            .with_label_values(&["1min"])
            .set((load.one * 100.0) as i64);
        crate::metrics::PROCESS_CPU_USAGE
            .with_label_values(&["5min"])
            .set((load.five * 100.0) as i64);
    }
}
