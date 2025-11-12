//! Authentication Examples for Synap Rust SDK
//!
//! This example demonstrates how to use authentication with the Synap Rust SDK,
//! including Basic Auth and API Key authentication.

use std::time::Duration;
use synap_sdk::client::{SynapClient, SynapConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔐 Synap Rust SDK - Authentication Examples\n");
    println!("{}", "=".repeat(50));
    println!();

    // Example 1: Basic Auth
    example_basic_auth().await?;

    // Example 2: API Key Auth
    example_api_key_auth().await?;

    // Example 3: Builder Pattern
    example_builder_pattern().await?;

    // Example 4: Switch Auth Methods
    example_switch_auth_methods().await?;

    println!("{}", "=".repeat(50));
    println!("✅ All authentication examples completed successfully!");

    Ok(())
}

async fn example_basic_auth() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Basic Auth Example ===\n");

    // Create config with Basic Auth credentials
    let config = SynapConfig::new("http://localhost:15500").with_basic_auth("root", "root");

    let client = SynapClient::new(config)?;

    println!("Testing connection with Basic Auth...");

    // Perform operations - authentication is automatic
    client.kv().set("test:key", "test_value", None).await?;
    let value: Option<Vec<u8>> = client.kv().get("test:key").await?;

    if let Some(bytes) = value {
        let value_str = String::from_utf8_lossy(&bytes);
        println!("✅ Successfully set and retrieved value: {}", value_str);
    }

    // Clean up
    client.kv().delete("test:key").await?;
    println!("✅ Cleaned up test key\n");

    Ok(())
}

async fn example_api_key_auth() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== API Key Authentication Example ===\n");

    // Create config with API Key
    let config = SynapConfig::new("http://localhost:15500").with_auth_token("your-api-key-here"); // Replace with actual API key

    let client = SynapClient::new(config)?;

    println!("Using API key authentication...");

    // Perform operations - API key authentication is automatic
    client.kv().set("test:api_key", "test_value", None).await?;
    let value: Option<Vec<u8>> = client.kv().get("test:api_key").await?;

    if let Some(bytes) = value {
        let value_str = String::from_utf8_lossy(&bytes);
        println!(
            "✅ Successfully set and retrieved value with API key: {}",
            value_str
        );
    }

    // Clean up
    client.kv().delete("test:api_key").await?;
    println!("✅ Cleaned up test key\n");

    Ok(())
}

async fn example_builder_pattern() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Builder Pattern Example ===\n");

    // Create base config
    let config = SynapConfig::new("http://localhost:15500")
        .with_timeout(Duration::from_secs(30))
        .with_basic_auth("root", "root");

    let client = SynapClient::new(config)?;

    client.kv().set("test:builder", "test_value", None).await?;
    let value: Option<Vec<u8>> = client.kv().get("test:builder").await?;

    if let Some(bytes) = value {
        let value_str = String::from_utf8_lossy(&bytes);
        println!("✅ Successfully used builder pattern: {}", value_str);
    }

    // Clean up
    client.kv().delete("test:builder").await?;
    println!("✅ Cleaned up test key\n");

    Ok(())
}

async fn example_switch_auth_methods() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Switching Auth Methods Example ===\n");

    // Start with Basic Auth
    let basic_config = SynapConfig::new("http://localhost:15500").with_basic_auth("root", "root");

    let basic_client = SynapClient::new(basic_config)?;
    basic_client
        .kv()
        .set("test:switch", "basic_auth", None)
        .await?;
    println!("✅ Set value using Basic Auth");

    // Switch to API Key (if you have one)
    // let api_key_config = SynapConfig::new("http://localhost:15500")
    //     .with_auth_token("your-api-key");
    // let api_key_client = SynapClient::new(api_key_config)?;
    // let value = api_key_client.kv().get("test:switch").await?;
    // if let Some(bytes) = value {
    //     let value_str = String::from_utf8_lossy(&bytes);
    //     println!("✅ Retrieved value using API Key: {}", value_str);
    // }

    // Clean up
    basic_client.kv().delete("test:switch").await?;
    println!("✅ Cleaned up test key\n");

    Ok(())
}
