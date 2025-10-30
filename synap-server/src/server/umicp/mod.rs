//! UMICP Protocol Integration for Synap
//!
//! This module provides UMICP protocol support for Synap,
//! enabling high-performance streaming communication over HTTP.
//!
//! Version 0.2.3: Native JSON types + Tool Discovery

use axum::response::Json;
use serde_json::Value;

pub mod discovery;
pub mod handlers;
pub mod transport;

pub use discovery::SynapDiscoveryService;

/// UMICP server state
#[derive(Clone)]
pub struct UmicpState {
    /// Synap AppState reference
    pub app_state: std::sync::Arc<crate::server::AppState>,
    /// MCP configuration for tool selection
    pub mcp_config: crate::config::McpConfig,
}

/// Health check for UMICP endpoint
pub async fn health_check() -> Json<Value> {
    Json(serde_json::json!({
        "protocol": "UMICP",
        "version": "0.2.3",
        "transport": "streamable-http",
        "status": "ok",
        "synap_version": env!("CARGO_PKG_VERSION")
    }))
}
