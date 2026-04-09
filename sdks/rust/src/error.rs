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

    /// TCP transport or I/O error
    #[error("Transport error: {0}")]
    Transport(String),

    /// Command has no native mapping for the active transport.
    ///
    /// Raised when `synap://` or `resp3://` transport is selected and the
    /// command is not in the native-protocol mapper.  Use `http://` transport
    /// for commands that are not yet mapped, or check the transport mapper.
    #[error("command '{command}' is not supported on transport '{transport}'")]
    UnsupportedCommand {
        /// The SDK command name (e.g. `"pubsub.subscribe"`).
        command: String,
        /// The active transport mode (e.g. `"SynapRpc"`, `"Resp3"`).
        transport: String,
    },

    /// Generic error
    #[error("{0}")]
    Other(String),
}
