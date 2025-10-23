//! Test utilities and mocks

use mockito::{Server, ServerGuard};

/// Create a mock Synap server for testing
pub async fn create_mock_server() -> ServerGuard {
    Server::new_async().await
}

/// Common test utilities
pub mod helpers {
    use super::*;

    /// Setup a test client pointing to a mock server
    pub async fn setup_test_client() -> (crate::SynapClient, ServerGuard) {
        let server = create_mock_server().await;
        let config = crate::SynapConfig::new(server.url());
        let client = crate::SynapClient::new(config).unwrap();
        (client, server)
    }
}
