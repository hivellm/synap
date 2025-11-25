//! Common test utilities

use mockito::{Server, ServerGuard};
use synap_sdk::{SynapClient, SynapConfig};

/// Create a mock Synap server for testing
#[allow(dead_code)] // Used by other test modules
pub async fn create_mock_server() -> ServerGuard {
    Server::new_async().await
}

/// Setup a test client pointing to a mock server
#[allow(dead_code)] // Used by other test modules
pub async fn setup_test_client() -> (SynapClient, ServerGuard) {
    let server = create_mock_server().await;
    let config = SynapConfig::new(server.url()).with_timeout(std::time::Duration::from_secs(5)); // Timeout adequado para testes
    let client = SynapClient::new(config).unwrap();
    (client, server)
}

/// Setup a test client for S2S tests (requires running server)
#[allow(dead_code)] // Used by S2S test modules
pub fn setup_s2s_client() -> SynapClient {
    let url = std::env::var("SYNAP_URL").unwrap_or_else(|_| "http://localhost:15500".to_string());
    let config = SynapConfig::new(url).with_timeout(std::time::Duration::from_secs(10));
    SynapClient::new(config).expect("Failed to create S2S client")
}
