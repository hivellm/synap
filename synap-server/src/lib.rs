pub mod auth;
pub mod compression;
pub mod config;
pub mod core;
pub mod persistence;
pub mod protocol;
pub mod server;

// Re-export commonly used types
pub use auth::{
    Acl, AclRule, Action, ApiKey, ApiKeyManager, AuthContext, Permission, ResourceType, Role, User,
    UserManager,
};
pub use compression::{CompressionAlgorithm, Compressor};
pub use config::ServerConfig;
pub use core::{
    EvictionPolicy, KVConfig, KVStore, Message, PubSubRouter, PubSubStats, PublishResult,
    QueueConfig, QueueManager, RoomStats, StreamConfig, StreamManager, SubscribeResult, SynapError,
    TopicInfo,
};
pub use protocol::{Request, Response};
pub use server::{AppState, create_router, get_mcp_tools, handle_mcp_tool};
