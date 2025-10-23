//! Reactive Event Stream Example
//!
//! This example demonstrates reactive event processing with Streams.
//!
//! Usage:
//!   cargo run --example reactive_stream

use futures::StreamExt;
use serde_json::json;
use std::time::Duration;
use synap_sdk::{SynapClient, SynapConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    // Create Synap client
    let config = SynapConfig::new("http://localhost:15500");
    let client = SynapClient::new(config)?;

    println!("🔄 Synap Rust SDK - Reactive Event Stream Example\n");

    // 1. Create a stream room
    println!("1. Creating stream room 'reactive-chat'");
    client
        .stream()
        .create_room("reactive-chat", Some(10000))
        .await?;
    println!("   ✅ Room created\n");

    // 2. Publish events in background
    let publish_client = client.clone();
    let publish_handle = tokio::spawn(async move {
        for i in 1..=20 {
            let offset = publish_client
                .stream()
                .publish(
                    "reactive-chat",
                    "message",
                    json!({
                        "user": format!("user{}", i % 3),
                        "text": format!("Message {}", i),
                        "timestamp": i
                    }),
                )
                .await;

            if let Ok(offset) = offset {
                println!("   📤 Published event at offset {}", offset);
            }

            tokio::time::sleep(Duration::from_millis(200)).await;
        }
    });

    // 3. Observe ALL events reactively
    println!("2. Observing all events (reactive Stream)\n");
    let (mut stream, handle) =
        client
            .stream()
            .observe_events("reactive-chat", Some(0), Duration::from_millis(150));

    let mut event_count = 0;
    tokio::time::timeout(Duration::from_secs(5), async {
        while let Some(event) = stream.next().await {
            println!(
                "   📨 Event {}: {} by {:?}",
                event.offset, event.data["text"], event.data["user"]
            );

            event_count += 1;
            if event_count >= 10 {
                println!("   Stopping after 10 events...\n");
                break;
            }
        }
    })
    .await
    .ok();

    handle.unsubscribe();

    // 4. Observe specific event type
    println!("3. Observing only 'message' events (filtered)\n");
    let (mut stream, handle) = client.stream().observe_event(
        "reactive-chat",
        "message",
        Some(0),
        Duration::from_millis(150),
    );

    let mut filtered_count = 0;
    tokio::time::timeout(Duration::from_secs(3), async {
        while let Some(event) = stream.next().await {
            println!("   📨 Message event {}: {:?}", event.offset, event.data);

            filtered_count += 1;
            if filtered_count >= 5 {
                println!("   Stopping after 5 filtered events...\n");
                break;
            }
        }
    })
    .await
    .ok();

    handle.unsubscribe();

    // Wait for publisher to finish
    let _ = publish_handle.await;

    // 5. Get final stats
    println!("4. Stream statistics");
    let stats = client.stream().stats("reactive-chat").await?;
    println!("   Room: {}", stats.room);
    println!("   Total events: {}", stats.total_events);
    println!("   Max offset: {}\n", stats.max_offset);

    // 6. Cleanup
    println!("5. Deleting room");
    client.stream().delete_room("reactive-chat").await?;
    println!("   ✅ Room deleted");

    println!("\n✅ Reactive stream example completed successfully!");
    println!("\n💡 Key Concepts:");
    println!("   - observe_events(): Stream all events from a room");
    println!("   - observe_event(): Filter events by type");
    println!("   - SubscriptionHandle: Automatic cleanup on drop");
    println!("   - Offset tracking: Automatically maintains position");

    Ok(())
}
