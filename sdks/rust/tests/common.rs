//! Common test utilities

use mockito::{Server, ServerGuard};
use synap_sdk::{SynapClient, SynapConfig};

/// Create a mock Synap server for testing
pub async fn create_mock_server() -> ServerGuard {
    Server::new_async().await
}

/// Setup a test client pointing to a mock server
pub async fn setup_test_client() -> (SynapClient, ServerGuard) {
    let server = create_mock_server().await;
    let config = SynapConfig::new(server.url());
    let client = SynapClient::new(config).unwrap();
    (client, server)
}
