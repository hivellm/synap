pub mod auth;
pub mod config;
pub mod hub;
pub mod metrics;
pub mod monitoring;
pub mod persistence;
pub mod protocol;
pub mod replication;
pub mod scripting;
pub mod server;

// Engine modules live in the `synap-core` crate. Re-export them under their
// original paths so existing `crate::core`, `crate::cluster`, `crate::cache`,
// `crate::compression` and `crate::simd` references keep resolving unchanged.
pub use synap_core::{cache, cluster, compression, core, simd};

pub use hub::{HubClient, HubConfig, QuotaManager, ResourceNaming, UsageReporter};

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
pub use replication::{
    MasterNode, NodeRole, ReplicaNode, ReplicationConfig, ReplicationLog, ReplicationStats,
};
pub use scripting::ScriptManager;
pub use server::{AppState, create_router, get_mcp_tools, handle_mcp_tool, init_metrics};
pub use synap_protocol::{Request, Response};
