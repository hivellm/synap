//! Prometheus Metrics HTTP Handler

use axum::{http::StatusCode, response::IntoResponse};

/// GET /metrics - Prometheus metrics endpoint
pub async fn metrics_handler() -> impl IntoResponse {
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

