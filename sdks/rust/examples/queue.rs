//! Message Queue Example
//!
//! This example demonstrates queue operations with ACK/NACK.
//!
//! Usage:
//!   cargo run --example queue

use std::time::Duration;
use synap_sdk::{SynapClient, SynapConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    // Create Synap client
    let config = SynapConfig::new("http://localhost:15500");
    let client = SynapClient::new(config)?;

    println!("📨 Synap Rust SDK - Message Queue Example\n");

    // 1. Create a queue
    println!("1. Creating queue 'tasks'");
    client
        .queue()
        .create_queue("tasks", Some(10000), Some(30))
        .await?;
    println!("   ✅ Queue created\n");

    // 2. Publish messages with different priorities
    println!("2. Publishing messages with priorities");
    let msg1 = client
        .queue()
        .publish("tasks", b"low-priority-task", Some(1), Some(3))
        .await?;
    println!("   Published message {} (priority 1)", msg1);

    let msg2 = client
        .queue()
        .publish("tasks", b"high-priority-task", Some(9), Some(3))
        .await?;
    println!("   Published message {} (priority 9)", msg2);

    let msg3 = client
        .queue()
        .publish("tasks", b"medium-priority-task", Some(5), Some(3))
        .await?;
    println!("   Published message {} (priority 5)\n", msg3);

    // 3. Get queue stats
    println!("3. Queue statistics");
    let stats = client.queue().stats("tasks").await?;
    println!("   Depth: {}", stats.depth);
    println!("   Pending: {}", stats.pending);
    println!("   Total published: {}\n", stats.total_published);

    // 4. Consume and ACK messages (consumed in priority order)
    println!("4. Consuming messages (priority order)");
    for i in 1..=3 {
        if let Some(message) = client.queue().consume("tasks", "worker-1").await? {
            println!(
                "   Consumed message {} (priority {})",
                message.id, message.priority
            );
            println!(
                "   Payload: {:?}",
                String::from_utf8_lossy(&message.payload)
            );

            // Acknowledge the message
            client.queue().ack("tasks", &message.id).await?;
            println!("   ✅ ACKed message {}\n", message.id);
        } else {
            println!("   No more messages ({})", i);
            break;
        }
    }

    // 5. Test NACK (requeue)
    println!("5. Testing NACK (requeue)");
    let msg_id = client
        .queue()
        .publish("tasks", b"requeue-test", Some(5), Some(3))
        .await?;
    println!("   Published message {}", msg_id);

    if let Some(message) = client.queue().consume("tasks", "worker-2").await? {
        println!("   Consumed message {}", message.id);

        // Simulate processing failure - NACK to requeue
        client.queue().nack("tasks", &message.id).await?;
        println!("   ✅ NACKed message (requeued)\n");
    }

    // 6. Consume the requeued message
    println!("6. Consuming requeued message");
    if let Some(message) = client.queue().consume("tasks", "worker-3").await? {
        println!(
            "   Consumed message {} (retry count: {})",
            message.id, message.retry_count
        );
        client.queue().ack("tasks", &message.id).await?;
        println!("   ✅ ACKed\n");
    }

    // 7. List all queues
    println!("7. Listing all queues");
    let queues = client.queue().list().await?;
    println!("   Queues: {:?}\n", queues);

    // 8. Cleanup
    println!("8. Deleting queue 'tasks'");
    client.queue().delete_queue("tasks").await?;
    println!("   ✅ Queue deleted");

    println!("\n✅ Example completed successfully!");

    Ok(())
}
