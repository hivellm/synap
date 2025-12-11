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

    tracing::info!("📡 Synap Rust SDK - Event Stream Example\n");

    // 1. Create a stream room
    tracing::info!("1. Creating stream room 'chat-room-1'");
    client
        .stream()
        .create_room("chat-room-1", Some(10000))
        .await?;
    tracing::info!("   ✅ Room created\n");

    // 2. Publish events
    tracing::info!("2. Publishing events");
    let offset1 = client
        .stream()
        .publish(
            "chat-room-1",
            "message",
            json!({"user": "alice", "text": "Hello everyone!"}),
        )
        .await?;
    tracing::info!("   Published event at offset {}", offset1);

    let offset2 = client
        .stream()
        .publish(
            "chat-room-1",
            "message",
            json!({"user": "bob", "text": "Hi Alice!"}),
        )
        .await?;
    tracing::info!("   Published event at offset {}", offset2);

    let offset3 = client
        .stream()
        .publish("chat-room-1", "join", json!({"user": "charlie"}))
        .await?;
    tracing::info!("   Published event at offset {}\n", offset3);

    // 3. Consume events from beginning
    tracing::info!("3. Consuming all events from offset 0");
    let events = client
        .stream()
        .consume("chat-room-1", Some(0), Some(10))
        .await?;
    tracing::info!("   Received {} events:", events.len());
    for event in &events {
        tracing::info!(
            "   - Offset {}: {} = {:?}",
            event.offset,
            event.event,
            event.data
        );
    }

    // 4. Consume events from specific offset
    tracing::info!("4. Consuming events from offset 1");
    let events = client
        .stream()
        .consume("chat-room-1", Some(1), Some(10))
        .await?;
    tracing::info!("   Received {} events:", events.len());
    for event in &events {
        tracing::info!(
            "   - Offset {}: {} = {:?}",
            event.offset,
            event.event,
            event.data
        );
    }

    // 5. Get room statistics
    tracing::info!("5. Getting room statistics");
    let stats = client.stream().stats("chat-room-1").await?;
    tracing::info!("   Room: {}", stats.room);
    tracing::info!("   Max offset: {}", stats.max_offset);
    tracing::info!("   Total events: {}\n", stats.total_events);

    // 6. List all rooms
    tracing::info!("6. Listing all stream rooms");
    let rooms = client.stream().list().await?;
    tracing::info!("   Rooms: {:?}\n", rooms);

    // 7. Cleanup
    tracing::info!("7. Deleting room 'chat-room-1'");
    client.stream().delete_room("chat-room-1").await?;
    tracing::info!("   ✅ Room deleted");

    tracing::info!("\n✅ Example completed successfully!");

    Ok(())
}
