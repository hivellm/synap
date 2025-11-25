pub mod auth;
pub mod cache;
pub mod cluster;
pub mod compression;
pub mod config;
pub mod core;
pub mod metrics;
pub mod monitoring;
pub mod persistence;
pub mod protocol;
pub mod replication;
pub mod scripting;
pub mod server;

// Re-export commonly used types
pub use auth::{
    Acl, AclRule, Action, ApiKey, ApiKeyManager, AuthContext, Permission, ResourceType, Role, User,
    UserManager,
};
pub use compression::{CompressionAlgorithm, Compressor};
pub use config::ServerConfig;
pub use core::{
    AssignmentStrategy, ConsumerGroupConfig, ConsumerGroupManager, EvictionPolicy, KVConfig,
    KVStore, Message, PartitionConfig, PartitionManager, PubSubRouter, PubSubStats, PublishResult,
    QueueConfig, QueueManager, RetentionPolicy, RoomStats, StreamConfig, StreamManager,
    SubscribeResult, SynapError, TopicInfo,
};
pub use protocol::{Request, Response};
pub use replication::{
    MasterNode, NodeRole, ReplicaNode, ReplicationConfig, ReplicationLog, ReplicationStats,
};
pub use scripting::ScriptManager;
pub use server::{AppState, create_router, get_mcp_tools, handle_mcp_tool, init_metrics};
