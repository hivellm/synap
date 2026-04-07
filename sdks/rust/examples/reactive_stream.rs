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

    tracing::info!("🔄 Synap Rust SDK - Reactive Event Stream Example\n");

    // 1. Create a stream room
    tracing::info!("1. Creating stream room 'reactive-chat'");
    client
        .stream()
        .create_room("reactive-chat", Some(10000))
        .await?;
    tracing::info!("   ✅ Room created\n");

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
                tracing::info!("   📤 Published event at offset {}", offset);
            }

            tokio::time::sleep(Duration::from_millis(200)).await;
        }
    });

    // 3. Observe ALL events reactively
    tracing::info!("2. Observing all events (reactive Stream)\n");
    let stream_mgr = client.stream();
    let (mut stream, handle) =
        stream_mgr.observe_events("reactive-chat", Some(0), Duration::from_millis(150));

    let mut event_count = 0;
    tokio::time::timeout(Duration::from_secs(5), async {
        while let Some(event) = stream.next().await {
            tracing::info!(
                "   📨 Event {}: {} by {:?}",
                event.offset,
                event.data["text"],
                event.data["user"]
            );

            event_count += 1;
            if event_count >= 10 {
                tracing::info!("   Stopping after 10 events...\n");
                break;
            }
        }
    })
    .await
    .ok();

    handle.unsubscribe();

    // 4. Observe specific event type
    tracing::info!("3. Observing only 'message' events (filtered)\n");
    let stream_mgr2 = client.stream();
    let (mut stream, handle) = stream_mgr2.observe_event(
        "reactive-chat",
        "message",
        Some(0),
        Duration::from_millis(150),
    );

    let mut filtered_count = 0;
    tokio::time::timeout(Duration::from_secs(3), async {
        while let Some(event) = stream.next().await {
            tracing::info!("   📨 Message event {}: {:?}", event.offset, event.data);

            filtered_count += 1;
            if filtered_count >= 5 {
                tracing::info!("   Stopping after 5 filtered events...\n");
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
    tracing::info!("4. Stream statistics");
    let stats = client.stream().stats("reactive-chat").await?;
    tracing::info!("   Room: {}", stats.room);
    tracing::info!("   Total events: {}", stats.total_events);
    tracing::info!("   Max offset: {}\n", stats.max_offset);

    // 6. Cleanup
    tracing::info!("5. Deleting room");
    client.stream().delete_room("reactive-chat").await?;
    tracing::info!("   ✅ Room deleted");

    tracing::info!("\n✅ Reactive stream example completed successfully!");
    tracing::info!("\n💡 Key Concepts:");
    tracing::info!("   - observe_events(): Stream all events from a room");
    tracing::info!("   - observe_event(): Filter events by type");
    tracing::info!("   - SubscriptionHandle: Automatic cleanup on drop");
    tracing::info!("   - Offset tracking: Automatically maintains position");

    Ok(())
}
