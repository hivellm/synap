//! Basic KV Store Example
//!
//! This example demonstrates basic key-value operations using the Synap SDK.
//!
//! Usage:
//!   cargo run --example basic

use synap_sdk::{SynapClient, SynapConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    // Create Synap client
    let config = SynapConfig::new("http://localhost:15500");
    let client = SynapClient::new(config)?;

    println!("🚀 Synap Rust SDK - Basic Example\n");

    // 1. SET a string value
    println!("1. Setting key 'greeting' = 'Hello, Synap!'");
    client.kv().set("greeting", "Hello, Synap!", None).await?;

    // 2. GET the value
    println!("2. Getting key 'greeting'");
    let value: Option<String> = client.kv().get("greeting").await?;
    println!("   Value: {:?}\n", value);

    // 3. SET with TTL (10 seconds)
    println!("3. Setting key 'session' with 10s TTL");
    client
        .kv()
        .set("session", "temporary-token", Some(10))
        .await?;

    // 4. Atomic operations
    println!("4. Increment counter");
    client.kv().set("counter", 0, None).await?;
    let val = client.kv().incr("counter").await?;
    println!("   Counter after INCR: {}", val);

    let val = client.kv().incr("counter").await?;
    println!("   Counter after INCR: {}", val);

    let val = client.kv().decr("counter").await?;
    println!("   Counter after DECR: {}\n", val);

    // 5. Check existence
    println!("5. Checking if key exists");
    let exists = client.kv().exists("greeting").await?;
    println!("   'greeting' exists: {}", exists);

    let exists = client.kv().exists("nonexistent").await?;
    println!("   'nonexistent' exists: {}\n", exists);

    // 6. Delete a key
    println!("6. Deleting key 'greeting'");
    let deleted = client.kv().delete("greeting").await?;
    println!("   Deleted: {}\n", deleted);

    // 7. Get statistics
    println!("7. Getting KV store statistics");
    let stats = client.kv().stats().await?;
    println!("   Total keys: {}", stats.total_keys);
    println!("   Total memory: {} bytes", stats.total_memory_bytes);
    println!("   Hit rate: {:.2}%", stats.hit_rate * 100.0);

    println!("\n✅ Example completed successfully!");

    Ok(())
}
