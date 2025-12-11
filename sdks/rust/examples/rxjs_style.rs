//! RxJS-style Reactive Example
//!
//! This example demonstrates RxJS-like patterns in Rust.
//!
//! Usage:
//!   cargo run --example rxjs_style

use std::time::Duration;
use synap_sdk::rx::Subject;
use synap_sdk::{SynapClient, SynapConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    tracing::info!("🎯 Synap Rust SDK - RxJS-style Reactive Example\n");

    // Create client
    let config = SynapConfig::new("http://localhost:15500");
    let client = SynapClient::new(config)?;

    // Example 1: Subject (like RxJS Subject)
    tracing::info!("1. Subject Example (multicast)");
    let subject = Subject::new();

    // Multiple subscribers
    let sub1 = subject.subscribe(|value| {
        tracing::info!("   Subscriber 1 received: {}", value);
    });

    let sub2 = subject.subscribe(|value| {
        tracing::info!("   Subscriber 2 received: {}", value);
    });

    // Emit values
    subject.next("Hello");
    subject.next("World");
    tokio::time::sleep(Duration::from_millis(100)).await;

    tracing::info!("   ✅ Both subscribers received messages\n");

    // Cleanup
    sub1.unsubscribe();
    sub2.unsubscribe();

    // Example 2: Observable with operators (RxJS-style)
    tracing::info!("2. Observable with operators");

    // Create queue
    client.queue().create_queue("rxjs-demo", None, None).await?;

    // Publish test messages
    for i in 1..=10 {
        client
            .queue()
            .publish(
                "rxjs-demo",
                format!("message-{}", i).as_bytes(),
                Some((i % 10) as u8),
                None,
            )
            .await?;
    }

    // Consume messages using StreamExt (Rust-style reactive)
    use futures::StreamExt;

    let (mut stream, handle) =
        client
            .queue()
            .observe_messages("rxjs-demo", "rxjs-worker", Duration::from_millis(100));

    // Chain operators (similar to RxJS pipe)
    tracing::info!("   Processing high-priority messages...");
    let mut count = 0;
    while let Some(msg) = stream.next().await {
        if msg.priority >= 7 {
            let data = String::from_utf8_lossy(&msg.payload);
            tracing::info!("   📨 High priority message: {} (id: {})", data, msg.id);
            count += 1;
            if count >= 5 {
                break;
            }
        }
    }

    // Cleanup
    handle.unsubscribe();
    client.queue().delete_queue("rxjs-demo").await?;

    tracing::info!("\n✅ RxJS-style example completed!");
    tracing::info!("\n💡 Key Features:");
    tracing::info!("   - Subject for multicasting");
    tracing::info!("   - Observable with .map(), .filter(), .take()");
    tracing::info!("   - .subscribe() method (RxJS-like)");
    tracing::info!("   - Subscription.unsubscribe()");

    Ok(())
}
