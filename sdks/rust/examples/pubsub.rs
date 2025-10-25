//! Pub/Sub Example
//!
//! This example demonstrates topic-based publish/subscribe.
//!
//! Usage:
//!   cargo run --example pubsub

use serde_json::json;
use synap_sdk::{SynapClient, SynapConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    // Create Synap client
    let config = SynapConfig::new("http://localhost:15500");
    let client = SynapClient::new(config)?;

    println!("🔔 Synap Rust SDK - Pub/Sub Example\n");

    // 1. Subscribe to topics with wildcards
    println!("1. Subscribing to topics");
    let sub_id = client
        .pubsub()
        .subscribe_topics(
            "user-123",
            vec![
                "events.user.*".to_string(),   // Single-level wildcard
                "notifications.#".to_string(), // Multi-level wildcard
            ],
        )
        .await?;
    println!("   Subscription ID: {}\n", sub_id);

    // 2. Publish messages to different topics
    println!("2. Publishing messages");

    let count = client
        .pubsub()
        .publish(
            "events.user.login",
            json!({"user_id": 123, "timestamp": 1234567890}),
            Some(5),
            None,
        )
        .await?;
    println!(
        "   Published to events.user.login - delivered to {} subscribers",
        count
    );

    let count = client
        .pubsub()
        .publish(
            "events.user.logout",
            json!({"user_id": 123, "timestamp": 1234567900}),
            Some(5),
            None,
        )
        .await?;
    println!(
        "   Published to events.user.logout - delivered to {} subscribers",
        count
    );

    let count = client
        .pubsub()
        .publish(
            "notifications.email.sent",
            json!({"to": "user@example.com", "subject": "Welcome"}),
            Some(8),
            None,
        )
        .await?;
    println!(
        "   Published to notifications.email.sent - delivered to {} subscribers\n",
        count
    );

    // 3. List active topics
    println!("3. Listing active topics");
    let topics = client.pubsub().list_topics().await?;
    println!("   Active topics: {:?}\n", topics);

    // 4. Unsubscribe
    println!("4. Unsubscribing");
    client
        .pubsub()
        .unsubscribe(
            &sub_id,
            vec!["events.user.*".to_string(), "notifications.#".to_string()],
        )
        .await?;
    println!("   ✅ Unsubscribed from topics");

    println!("\n✅ Example completed successfully!");
    println!("\n📝 Note: To see message delivery in real-time, run multiple");
    println!("   instances of this example in different terminals.");

    Ok(())
}
