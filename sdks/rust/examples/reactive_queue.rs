//! Reactive Queue Example
//!
//! This example demonstrates reactive message processing with Streams.
//!
//! Usage:
//!   cargo run --example reactive_queue

use futures::StreamExt;
use std::time::Duration;
use synap_sdk::{SynapClient, SynapConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    // Create Synap client
    let config = SynapConfig::new("http://localhost:15500");
    let client = SynapClient::new(config)?;

    println!("🔄 Synap Rust SDK - Reactive Queue Example\n");

    // 1. Create a queue
    println!("1. Creating queue 'reactive-tasks'");
    client
        .queue()
        .create_queue("reactive-tasks", Some(10000), Some(30))
        .await?;
    println!("   ✅ Queue created\n");

    // 2. Publish some messages
    println!("2. Publishing 10 messages");
    for i in 1..=10 {
        client
            .queue()
            .publish(
                "reactive-tasks",
                format!("Task {}", i).as_bytes(),
                Some((i % 10) as u8),
                Some(3),
            )
            .await?;
    }
    println!("   ✅ Published 10 messages\n");

    // 3. Observe messages reactively (manual ACK)
    println!("3. Consuming messages reactively (with Stream)");
    let queue = client.queue();
    let (mut stream, handle) =
        queue.observe_messages("reactive-tasks", "worker-1", Duration::from_millis(100));

    let mut count = 0;
    while let Some(message) = stream.next().await {
        println!(
            "   📨 Received message {} (priority {}): {:?}",
            message.id,
            message.priority,
            String::from_utf8_lossy(&message.payload)
        );

        // Manual ACK
        client.queue().ack("reactive-tasks", &message.id).await?;
        println!("      ✅ ACKed");

        count += 1;
        if count >= 5 {
            println!("   Stopping after 5 messages...\n");
            handle.unsubscribe();
            break;
        }
    }

    // 4. Process messages with automatic ACK/NACK
    println!("4. Processing remaining messages (auto ACK/NACK)");

    let handle = client.queue().process_messages(
        "reactive-tasks",
        "worker-2",
        Duration::from_millis(100),
        |message| async move {
            println!(
                "   ⚙️  Processing message {}: {:?}",
                message.id,
                String::from_utf8_lossy(&message.payload)
            );

            // Simulate processing
            tokio::time::sleep(Duration::from_millis(50)).await;

            // Simulate occasional failure
            if message.priority == 3 {
                println!("      ❌ Processing failed (will NACK)");
                Err(synap_sdk::SynapError::Other(
                    "Simulated failure".to_string(),
                ))
            } else {
                println!("      ✅ Processing succeeded (will ACK)");
                Ok(())
            }
        },
    );

    // Let it process for 3 seconds
    tokio::time::sleep(Duration::from_secs(3)).await;
    handle.unsubscribe();
    println!("   Stopped processing\n");

    // 5. Get final stats
    println!("5. Queue statistics");
    let stats = client.queue().stats("reactive-tasks").await?;
    println!("   Depth: {}", stats.depth);
    println!("   Total consumed: {}", stats.total_consumed);
    println!("   Total acked: {}", stats.total_acked);
    println!("   Total nacked: {}\n", stats.total_nacked);

    // 6. Cleanup
    println!("6. Deleting queue");
    client.queue().delete_queue("reactive-tasks").await?;
    println!("   ✅ Queue deleted");

    println!("\n✅ Reactive example completed successfully!");
    println!("\n💡 Key Concepts:");
    println!("   - observe_messages(): Stream-based consumption with manual ACK");
    println!("   - process_messages(): Automatic ACK/NACK based on handler result");
    println!("   - SubscriptionHandle: Graceful cancellation support");

    Ok(())
}
