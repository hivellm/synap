//! UMICP Transport - Simple HTTP handler
//! Updated for v0.2.3: Native JSON types + Tool Discovery

use axum::Json;
use axum::extract::{Request, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;
use tracing::{debug, error, info};
use umicp_core::{DiscoverableService, Envelope};

use super::{SynapDiscoveryService, UmicpState};

/// Main UMICP HTTP handler
pub async fn umicp_handler(State(state): State<UmicpState>, request: Request) -> Response {
    info!("🔌 UMICP request received");

    // The router's auth layer attaches the caller's identity to every request
    // (an admin context when authentication is disabled); without one the
    // request is refused rather than run unrestricted.
    let tenant = request
        .extensions()
        .get::<crate::hub::HubUserContext>()
        .map(|hub| hub.user_id);
    let caller = match request.extensions().get::<crate::auth::AuthContext>() {
        Some(auth) => crate::auth::OperationContext::from_auth_context(auth).with_tenant(tenant),
        None => {
            return (
                StatusCode::FORBIDDEN,
                r#"{"error":"NOPERM the request carries no identity"}"#,
            )
                .into_response();
        }
    };

    // Read the body up to the configured limit. A declared length above it is
    // refused before anything is read; a chunked body stops being read as soon
    // as it passes the limit, so no more than the limit is ever buffered.
    let limit = state.mcp_config.umicp_max_body_bytes;
    if declared_length(&request).is_some_and(|length| length > limit as u64) {
        return payload_too_large(limit);
    }
    let body_bytes = match axum::body::to_bytes(request.into_body(), limit).await {
        Ok(bytes) => bytes,
        Err(e) if exceeds_limit(&e) => return payload_too_large(limit),
        Err(e) => {
            error!("Failed to read body: {}", e);
            return (
                StatusCode::BAD_REQUEST,
                format!(r#"{{"error":"Failed to read body: {}"}}"#, e),
            )
                .into_response();
        }
    };

    let body_str = match std::str::from_utf8(&body_bytes) {
        Ok(s) => s,
        Err(e) => {
            error!("Invalid UTF-8: {}", e);
            return (StatusCode::BAD_REQUEST, r#"{"error":"Invalid UTF-8"}"#).into_response();
        }
    };

    debug!("Received body: {} bytes", body_str.len());

    // Parse envelope
    let envelope = match Envelope::deserialize(body_str) {
        Ok(env) => env,
        Err(e) => {
            error!("Failed to parse envelope: {}", e);
            return (
                StatusCode::BAD_REQUEST,
                format!(r#"{{"error":"Invalid UMICP envelope: {}"}}"#, e),
            )
                .into_response();
        }
    };

    // Validate
    if let Err(e) = envelope.validate() {
        error!("Envelope validation failed: {}", e);
        return (
            StatusCode::BAD_REQUEST,
            format!(r#"{{"error":"Validation failed: {}"}}"#, e),
        )
            .into_response();
    }

    debug!(
        "✅ Valid envelope - from: {}, to: {}, op: {:?}",
        envelope.from(),
        envelope.to(),
        envelope.operation()
    );

    // Process request
    let response_envelope = super::handlers::handle_umicp_request(state, envelope, &caller).await;

    match response_envelope {
        Ok(response) => match response.serialize() {
            Ok(json_str) => (StatusCode::OK, json_str).into_response(),
            Err(e) => {
                error!("Failed to serialize response: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    r#"{"error":"Failed to serialize response"}"#,
                )
                    .into_response()
            }
        },
        Err(e) => {
            error!("Handler error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!(r#"{{"error":"{}"}}"#, e),
            )
                .into_response()
        }
    }
}

/// The `Content-Length` the request declares, if any. An unparsable value is
/// left to the body reader, which still enforces the limit.
fn declared_length(request: &Request) -> Option<u64> {
    request
        .headers()
        .get(axum::http::header::CONTENT_LENGTH)?
        .to_str()
        .ok()?
        .parse()
        .ok()
}

/// Whether reading the body failed because it passed the limit.
fn exceeds_limit(error: &axum::Error) -> bool {
    std::error::Error::source(error)
        .is_some_and(|source| source.is::<http_body_util::LengthLimitError>())
}

/// 413 naming the limit the body exceeded.
fn payload_too_large(limit: usize) -> Response {
    debug!("UMICP body exceeds the {} byte limit", limit);
    (
        StatusCode::PAYLOAD_TOO_LARGE,
        format!(r#"{{"error":"UMICP request body exceeds the {limit} byte limit"}}"#),
    )
        .into_response()
}

/// UMICP Discovery Handler - Returns all available operations
/// Endpoint: GET /umicp/discover
pub async fn umicp_discover_handler(State(state): State<UmicpState>) -> Json<serde_json::Value> {
    info!("🔍 UMICP discovery request received");

    let service = SynapDiscoveryService {
        mcp_config: state.mcp_config.clone(),
    };
    let server_info = service.server_info();
    let operations = service.list_operations();

    // Operations are already serializable OperationSchema structs
    let operations_json: Vec<serde_json::Value> = operations
        .iter()
        .map(|op| serde_json::to_value(op).expect("OperationSchema serializes to JSON"))
        .collect();

    Json(json!({
        "protocol": "UMICP",
        "version": "0.2.3",
        "server_info": {
            "server": server_info.server,
            "version": server_info.version,
            "protocol": server_info.protocol,
            "features": server_info.features,
            "operations_count": server_info.operations_count,
            "mcp_compatible": server_info.mcp_compatible,
            "metadata": server_info.metadata,
        },
        "operations": operations_json,
        "total_operations": operations.len(),
    }))
}
