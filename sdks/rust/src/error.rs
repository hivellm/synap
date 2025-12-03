//! Error types for Synap SDK

use thiserror::Error;

/// Result type alias for Synap SDK operations
pub type Result<T> = std::result::Result<T, SynapError>;

/// Synap SDK error types
#[derive(Error, Debug)]
pub enum SynapError {
    /// HTTP request error
    #[error("HTTP error: {0}")]
    HttpError(#[from] reqwest::Error),

    /// JSON serialization/deserialization error
    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),

    /// Invalid URL
    #[error("Invalid URL: {0}")]
    InvalidUrl(#[from] url::ParseError),

    /// Server returned an error
    #[error("Server error: {0}")]
    ServerError(String),

    /// Key not found
    #[error("Key not found: {0}")]
    KeyNotFound(String),

    /// Queue not found
    #[error("Queue not found: {0}")]
    QueueNotFound(String),

    /// Stream room not found
    #[error("Stream room not found: {0}")]
    RoomNotFound(String),

    /// Invalid response from server
    #[error("Invalid response: {0}")]
    InvalidResponse(String),

    /// Operation timeout
    #[error("Operation timeout")]
    Timeout,

    /// Generic error
    #[error("{0}")]
    Other(String),
}
