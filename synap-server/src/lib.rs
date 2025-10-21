pub mod compression;
pub mod config;
pub mod core;
pub mod protocol;
pub mod server;

// Re-export commonly used types
pub use compression::{CompressionAlgorithm, Compressor};
pub use config::ServerConfig;
pub use core::{KVConfig, KVStore, QueueManager, SynapError};
pub use protocol::{Request, Response};
pub use server::{AppState, create_router};
