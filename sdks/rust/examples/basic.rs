//! Basic KV Store Example
//!
//! This example demonstrates basic key-value operations using the Synap SDK.
//!
//! Usage:
//!   cargo run --example basic

use synap_sdk::{SynapClient, SynapConfig};
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    // Create Synap client
    let config = SynapConfig::new("http://localhost:15500");
    let client = SynapClient::new(config)?;

    info!("🚀 Synap Rust SDK - Basic Example\n");

    // 1. SET a string value
    info!("1. Setting key 'greeting' = 'Hello, Synap!'");
    client.kv().set("greeting", "Hello, Synap!", None).await?;

    // 2. GET the value
    info!("2. Getting key 'greeting'");
    let value: Option<String> = client.kv().get("greeting").await?;
    info!("   Value: {:?}\n", value);

    // 3. SET with TTL (10 seconds)
    info!("3. Setting key 'session' with 10s TTL");
    client
        .kv()
        .set("session", "temporary-token", Some(10))
        .await?;

    // 4. Atomic operations
    info!("4. Increment counter");
    client.kv().set("counter", 0, None).await?;
    let val = client.kv().incr("counter").await?;
    info!("   Counter after INCR: {}", val);

    let val = client.kv().incr("counter").await?;
    info!("   Counter after INCR: {}", val);

    let val = client.kv().decr("counter").await?;
    info!("   Counter after DECR: {}\n", val);

    // 5. Check existence
    info!("5. Checking if key exists");
    let exists = client.kv().exists("greeting").await?;
    info!("   'greeting' exists: {}", exists);

    let exists = client.kv().exists("nonexistent").await?;
    info!("   'nonexistent' exists: {}\n", exists);

    // 6. Delete a key
    info!("6. Deleting key 'greeting'");
    let deleted = client.kv().delete("greeting").await?;
    info!("   Deleted: {}\n", deleted);

    // 7. Get statistics
    info!("7. Getting KV store statistics");
    let stats = client.kv().stats().await?;
    info!("   Total keys: {}", stats.total_keys);
    info!("   Total memory: {} bytes", stats.total_memory_bytes);
    info!("   Hit rate: {:.2}%", stats.hit_rate * 100.0);

    info!("\n✅ Example completed successfully!");

    Ok(())
}
