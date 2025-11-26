//! Message Queue Example
//!
//! This example demonstrates queue operations with ACK/NACK.
//!
//! Usage:
//!   cargo run --example queue

use synap_sdk::{SynapClient, SynapConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    // Create Synap client
    let config = SynapConfig::new("http://localhost:15500");
    let client = SynapClient::new(config)?;

    tracing::info!("📨 Synap Rust SDK - Message Queue Example\n");

    // 1. Create a queue
    tracing::info!("1. Creating queue 'tasks'");
    client
        .queue()
        .create_queue("tasks", Some(10000), Some(30))
        .await?;
    tracing::info!("   ✅ Queue created\n");

    // 2. Publish messages with different priorities
    tracing::info!("2. Publishing messages with priorities");
    let msg1 = client
        .queue()
        .publish("tasks", b"low-priority-task", Some(1), Some(3))
        .await?;
    tracing::info!("   Published message {} (priority 1)", msg1);

    let msg2 = client
        .queue()
        .publish("tasks", b"high-priority-task", Some(9), Some(3))
        .await?;
    tracing::info!("   Published message {} (priority 9)", msg2);

    let msg3 = client
        .queue()
        .publish("tasks", b"medium-priority-task", Some(5), Some(3))
        .await?;
    tracing::info!("   Published message {} (priority 5)\n", msg3);

    // 3. Get queue stats
    tracing::info!("3. Queue statistics");
    let stats = client.queue().stats("tasks").await?;
    tracing::info!("   Depth: {}", stats.depth);
    tracing::info!("   Pending: {}", stats.pending);
    tracing::info!("   Total published: {}\n", stats.total_published);

    // 4. Consume and ACK messages (consumed in priority order)
    tracing::info!("4. Consuming messages (priority order)");
    for i in 1..=3 {
        if let Some(message) = client.queue().consume("tasks", "worker-1").await? {
            tracing::info!(
                "   Consumed message {} (priority {})",
                message.id,
                message.priority
            );
            tracing::info!(
                "   Payload: {:?}",
                String::from_utf8_lossy(&message.payload)
            );

            // Acknowledge the message
            client.queue().ack("tasks", &message.id).await?;
            tracing::info!("   ✅ ACKed message {}\n", message.id);
        } else {
            tracing::info!("   No more messages ({})", i);
            break;
        }
    }

    // 5. Test NACK (requeue)
    tracing::info!("5. Testing NACK (requeue)");
    let msg_id = client
        .queue()
        .publish("tasks", b"requeue-test", Some(5), Some(3))
        .await?;
    tracing::info!("   Published message {}", msg_id);

    if let Some(message) = client.queue().consume("tasks", "worker-2").await? {
        tracing::info!("   Consumed message {}", message.id);

        // Simulate processing failure - NACK to requeue
        client.queue().nack("tasks", &message.id).await?;
        tracing::info!("   ✅ NACKed message (requeued)\n");
    }

    // 6. Consume the requeued message
    tracing::info!("6. Consuming requeued message");
    if let Some(message) = client.queue().consume("tasks", "worker-3").await? {
        tracing::info!(
            "   Consumed message {} (retry count: {})",
            message.id,
            message.retry_count
        );
        client.queue().ack("tasks", &message.id).await?;
        tracing::info!("   ✅ ACKed\n");
    }

    // 7. List all queues
    tracing::info!("7. Listing all queues");
    let queues = client.queue().list().await?;
    tracing::info!("   Queues: {:?}\n", queues);

    // 8. Cleanup
    tracing::info!("8. Deleting queue 'tasks'");
    client.queue().delete_queue("tasks").await?;
    tracing::info!("   ✅ Queue deleted");

    tracing::info!("\n✅ Example completed successfully!");

    Ok(())
}
