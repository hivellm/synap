pub mod auth;
pub mod compression;
pub mod config;
pub mod core;
pub mod protocol;
pub mod server;

// Re-export commonly used types
pub use auth::{
    Acl, AclRule, Action, ApiKey, ApiKeyManager, AuthContext, Permission, ResourceType, Role, User,
    UserManager,
};
pub use compression::{CompressionAlgorithm, Compressor};
pub use config::ServerConfig;
pub use core::{KVConfig, KVStore, QueueConfig, QueueManager, SynapError};
pub use protocol::{Request, Response};
pub use server::{AppState, create_router};
