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

    #[error("Queue not found: {0}")]
    QueueNotFound(String),

    #[error("Queue is full: {0}")]
    QueueFull(String),

    #[error("Message not found: {0}")]
    MessageNotFound(String),

    #[error("Consumer not found: {0}")]
    ConsumerNotFound(String),

    #[error("IO error: {0}")]
    IoError(String),
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
            Self::QueueNotFound(_) | Self::MessageNotFound(_) | Self::ConsumerNotFound(_) => {
                StatusCode::NOT_FOUND
            }
            Self::QueueFull(_) => StatusCode::INSUFFICIENT_STORAGE,
            Self::IoError(_) => StatusCode::INTERNAL_SERVER_ERROR,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_status_codes() {
        assert_eq!(
            SynapError::KeyNotFound("test".to_string()).status_code(),
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            SynapError::KeyExists("test".to_string()).status_code(),
            StatusCode::CONFLICT
        );
        assert_eq!(
            SynapError::InvalidValue("test".to_string()).status_code(),
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            SynapError::MemoryLimitExceeded.status_code(),
            StatusCode::INSUFFICIENT_STORAGE
        );
        assert_eq!(
            SynapError::CASFailed {
                expected: "1".to_string(),
                actual: "2".to_string()
            }
            .status_code(),
            StatusCode::CONFLICT
        );
        assert_eq!(
            SynapError::QueueFull("test".to_string()).status_code(),
            StatusCode::INSUFFICIENT_STORAGE
        );
    }

    #[test]
    fn test_error_display() {
        let err = SynapError::KeyNotFound("mykey".to_string());
        assert_eq!(err.to_string(), "Key not found: mykey");

        let err = SynapError::CASFailed {
            expected: "1".to_string(),
            actual: "2".to_string(),
        };
        assert_eq!(err.to_string(), "CAS failed - expected: 1, actual: 2");
    }

    #[test]
    fn test_error_into_response() {
        let err = SynapError::KeyNotFound("test".to_string());
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[test]
    fn test_all_error_variants() {
        // Test that all error variants can be created
        let _ = SynapError::KeyNotFound("key".to_string());
        let _ = SynapError::KeyExists("key".to_string());
        let _ = SynapError::InvalidValue("val".to_string());
        let _ = SynapError::MemoryLimitExceeded;
        let _ = SynapError::TTLInvalid("ttl".to_string());
        let _ = SynapError::CASFailed {
            expected: "1".to_string(),
            actual: "2".to_string(),
        };
        let _ = SynapError::UnknownCommand("cmd".to_string());
        let _ = SynapError::InvalidRequest("req".to_string());
        let _ = SynapError::SerializationError("err".to_string());
        let _ = SynapError::InternalError("err".to_string());
        let _ = SynapError::QueueNotFound("q".to_string());
        let _ = SynapError::QueueFull("q".to_string());
        let _ = SynapError::MessageNotFound("msg".to_string());
        let _ = SynapError::ConsumerNotFound("consumer".to_string());
    }
}
