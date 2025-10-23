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

pub mod client;
pub mod error;
pub mod kv;
#[cfg(test)]
mod kv_test;
pub mod pubsub;
#[cfg(test)]
mod pubsub_test;
pub mod queue;
mod queue_reactive;
#[cfg(test)]
mod queue_test;
pub mod reactive;
pub mod stream;
mod stream_reactive;
#[cfg(test)]
mod stream_test;
#[cfg(test)]
mod tests;
pub mod types;

pub use client::{SynapClient, SynapConfig};
pub use error::{Result, SynapError};
pub use kv::KVStore;
pub use pubsub::PubSubManager;
pub use queue::QueueManager;
pub use reactive::{MessageStream, SubscriptionHandle};
pub use stream::StreamManager;
