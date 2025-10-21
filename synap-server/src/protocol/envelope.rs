use serde::{Deserialize, Serialize};

/// StreamableHTTP request envelope
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Request {
    /// Command to execute (e.g., "kv.set", "kv.get")
    pub command: String,
    /// Unique request identifier
    pub request_id: String,
    /// Command payload
    pub payload: serde_json::Value,
}

/// StreamableHTTP response envelope
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    /// Whether the operation succeeded
    pub success: bool,
    /// Matching request identifier
    pub request_id: String,
    /// Response payload (if successful)
    pub payload: Option<serde_json::Value>,
    /// Error message (if failed)
    pub error: Option<String>,
}

impl Request {
    /// Create a new request
    pub fn new(command: impl Into<String>, payload: serde_json::Value) -> Self {
        Self {
            command: command.into(),
            request_id: uuid::Uuid::new_v4().to_string(),
            payload,
        }
    }
}

impl Response {
    /// Create a successful response
    pub fn success(request_id: String, payload: serde_json::Value) -> Self {
        Self {
            success: true,
            request_id,
            payload: Some(payload),
            error: None,
        }
    }

    /// Create an error response
    pub fn error(request_id: String, error: impl Into<String>) -> Self {
        Self {
            success: false,
            request_id,
            payload: None,
            error: Some(error.into()),
        }
    }
}
