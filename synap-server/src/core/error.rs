use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::json;
use thiserror::Error;

/// Main error type for Synap operations
#[derive(Debug, Error)]
pub enum SynapError {
    #[error("Key not found: {0}")]
    KeyNotFound(String),

    #[error("Key already exists: {0}")]
    KeyExists(String),

    #[error("Invalid value: {0}")]
    InvalidValue(String),

    #[error("Memory limit exceeded")]
    MemoryLimitExceeded,

    #[error("Invalid TTL: {0}")]
    TTLInvalid(String),

    #[error("CAS failed - expected: {expected}, actual: {actual}")]
    CASFailed { expected: String, actual: String },

    #[error("Unknown command: {0}")]
    UnknownCommand(String),

    #[error("Invalid request: {0}")]
    InvalidRequest(String),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("Internal error: {0}")]
    InternalError(String),
}

impl SynapError {
    /// Get HTTP status code for this error
    pub fn status_code(&self) -> StatusCode {
        match self {
            Self::KeyNotFound(_) => StatusCode::NOT_FOUND,
            Self::KeyExists(_) => StatusCode::CONFLICT,
            Self::InvalidValue(_) | Self::TTLInvalid(_) | Self::InvalidRequest(_) => {
                StatusCode::BAD_REQUEST
            }
            Self::MemoryLimitExceeded => StatusCode::INSUFFICIENT_STORAGE,
            Self::CASFailed { .. } => StatusCode::CONFLICT,
            Self::UnknownCommand(_) => StatusCode::BAD_REQUEST,
            Self::SerializationError(_) | Self::InternalError(_) => {
                StatusCode::INTERNAL_SERVER_ERROR
            }
        }
    }
}

/// Implement IntoResponse for Axum integration
impl IntoResponse for SynapError {
    fn into_response(self) -> Response {
        let status = self.status_code();
        let body = Json(json!({
            "error": self.to_string(),
            "code": status.as_u16(),
        }));

        (status, body).into_response()
    }
}

/// Result type alias for Synap operations
pub type Result<T> = std::result::Result<T, SynapError>;
