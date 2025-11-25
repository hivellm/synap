//! Reactive Pub/Sub Example
//!
//! This example demonstrates reactive Pub/Sub subscriptions using WebSocket.
//!
//! Usage:
//!   cargo run --example reactive_pubsub

use futures::StreamExt;
use serde_json::json;
use std::time::Duration;
use synap_sdk::{SynapClient, SynapConfig};
use tokio::time::sleep;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    // Create Synap client
    let config = SynapConfig::new("http://localhost:15500");
    let client = SynapClient::new(config)?;

    println!("🔔 Synap Rust SDK - Reactive Pub/Sub Example\n");

    // Subscribe to topics reactively
    println!("1. Subscribing to topics reactively");
    let (mut stream, handle) = client.pubsub().observe(
        "subscriber-1",
        vec![
            "user.*".to_string(),          // Single-level wildcard
            "notifications.#".to_string(), // Multi-level wildcard
        ],
    );

    println!("   ✅ Subscribed! Waiting for messages...\n");

    // Spawn a task to publish messages
    let publisher = client.pubsub();
    tokio::spawn(async move {
        sleep(Duration::from_secs(1)).await;

        println!("2. Publishing messages");

        let count = publisher
            .publish(
                "user.login",
                json!({"user_id": 123, "action": "login"}),
                Some(5),
                None,
            )
            .await
            .unwrap();
        println!(
            "   Published to user.login - delivered to {} subscribers",
            count
        );

        sleep(Duration::from_millis(500)).await;

        let count = publisher
            .publish(
                "user.logout",
                json!({"user_id": 123, "action": "logout"}),
                Some(5),
                None,
            )
            .await
            .unwrap();
        println!(
            "   Published to user.logout - delivered to {} subscribers",
            count
        );

        sleep(Duration::from_millis(500)).await;

        let count = publisher
            .publish(
                "notifications.email.sent",
                json!({"to": "user@example.com", "subject": "Welcome"}),
                Some(8),
                None,
            )
            .await
            .unwrap();
        println!(
            "   Published to notifications.email.sent - delivered to {} subscribers\n",
            count
        );

        sleep(Duration::from_secs(2)).await;
    });

    // Process messages reactively
    println!("3. Processing messages reactively");
    let mut message_count = 0;
    while let Some(message) = stream.next().await {
        message_count += 1;
        println!("   📨 Received on {}: {:?}", message.topic, message.data);

        if message_count >= 3 {
            println!("\n4. Unsubscribing");
            handle.unsubscribe();
            break;
        }
    }

    println!("   ✅ Received {} messages", message_count);
    println!("\n✅ Example completed successfully!");

    Ok(())
}
