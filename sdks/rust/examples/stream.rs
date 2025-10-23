//! Event Stream Example
//!
//! This example demonstrates real-time event streaming.
//!
//! Usage:
//!   cargo run --example stream

use serde_json::json;
use synap_sdk::{SynapClient, SynapConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    // Create Synap client
    let config = SynapConfig::new("http://localhost:15500");
    let client = SynapClient::new(config)?;

    println!("📡 Synap Rust SDK - Event Stream Example\n");

    // 1. Create a stream room
    println!("1. Creating stream room 'chat-room-1'");
    client
        .stream()
        .create_room("chat-room-1", Some(10000))
        .await?;
    println!("   ✅ Room created\n");

    // 2. Publish events
    println!("2. Publishing events");
    let offset1 = client
        .stream()
        .publish(
            "chat-room-1",
            "message",
            json!({"user": "alice", "text": "Hello everyone!"}),
        )
        .await?;
    println!("   Published event at offset {}", offset1);

    let offset2 = client
        .stream()
        .publish(
            "chat-room-1",
            "message",
            json!({"user": "bob", "text": "Hi Alice!"}),
        )
        .await?;
    println!("   Published event at offset {}", offset2);

    let offset3 = client
        .stream()
        .publish("chat-room-1", "join", json!({"user": "charlie"}))
        .await?;
    println!("   Published event at offset {}\n", offset3);

    // 3. Consume events from beginning
    println!("3. Consuming all events from offset 0");
    let events = client
        .stream()
        .consume("chat-room-1", Some(0), Some(10))
        .await?;
    println!("   Received {} events:", events.len());
    for event in &events {
        println!(
            "   - Offset {}: {} = {:?}",
            event.offset, event.event_type, event.data
        );
    }
    println!();

    // 4. Consume events from specific offset
    println!("4. Consuming events from offset 1");
    let events = client
        .stream()
        .consume("chat-room-1", Some(1), Some(10))
        .await?;
    println!("   Received {} events:", events.len());
    for event in &events {
        println!(
            "   - Offset {}: {} = {:?}",
            event.offset, event.event_type, event.data
        );
    }
    println!();

    // 5. Get room statistics
    println!("5. Getting room statistics");
    let stats = client.stream().stats("chat-room-1").await?;
    println!("   Room: {}", stats.room);
    println!("   Max offset: {}", stats.max_offset);
    println!("   Total events: {}\n", stats.total_events);

    // 6. List all rooms
    println!("6. Listing all stream rooms");
    let rooms = client.stream().list().await?;
    println!("   Rooms: {:?}\n", rooms);

    // 7. Cleanup
    println!("7. Deleting room 'chat-room-1'");
    client.stream().delete_room("chat-room-1").await?;
    println!("   ✅ Room deleted");

    println!("\n✅ Example completed successfully!");

    Ok(())
}
