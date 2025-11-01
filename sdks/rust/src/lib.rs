//! # Synap Rust SDK
//!
//! Official Rust client for Synap - High-Performance In-Memory Key-Value Store & Message Broker
//!
//! ## Features
//!
//! - 💾 **Key-Value Store**: Fast in-memory KV operations with TTL support
//! - 📨 **Message Queues**: RabbitMQ-style queues with ACK/NACK
//! - 📡 **Event Streams**: Kafka-style event streams with offset tracking
//! - 🔔 **Pub/Sub**: Topic-based publish/subscribe with wildcards
//! - 🔄 **Async/Await**: Built on Tokio for high-performance async I/O
//! - 🛡️ **Type-Safe**: Leverages Rust's type system for correctness
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use synap_sdk::{SynapClient, SynapConfig};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Create client
//!     let config = SynapConfig::new("http://localhost:15500");
//!     let client = SynapClient::new(config)?;
//!
//!     // Key-Value operations
//!     client.kv().set("user:1", "John Doe", None).await?;
//!     let value: Option<String> = client.kv().get("user:1").await?;
//!     println!("Value: {:?}", value);
//!
//!     Ok(())
//! }
//! ```

pub mod bitmap;
pub mod client;
pub mod error;
pub mod hash;
pub mod hyperloglog;
pub mod kv;
pub mod list;
pub mod pubsub;
pub mod queue;
mod queue_reactive;
pub mod reactive;
pub mod rx; // RxJS-style reactive programming
pub mod scripting;
pub mod set;
pub mod sorted_set;
pub mod stream;
mod stream_reactive;
pub mod transactions;
pub mod types;

pub use bitmap::{BitmapManager, BitmapOperation, BitmapStats};
pub use client::{SynapClient, SynapConfig};
pub use error::{Result, SynapError};
pub use hash::HashManager;
pub use hyperloglog::HyperLogLogManager;
pub use kv::KVStore;
pub use list::ListManager;
pub use pubsub::PubSubManager;
pub use queue::QueueManager;
pub use reactive::{MessageStream, SubscriptionHandle};
pub use scripting::{
    ScriptEvalOptions, ScriptEvalResponse, ScriptExistsResponse, ScriptFlushResponse,
    ScriptKillResponse, ScriptManager,
};
pub use set::SetManager;
pub use sorted_set::{ScoredMember, SortedSetManager, SortedSetStats};
pub use stream::StreamManager;
pub use transactions::{
    TransactionCommandClient, TransactionExecResult, TransactionManager, TransactionOptions,
    TransactionResponse,
};
pub use types::HyperLogLogStats;
