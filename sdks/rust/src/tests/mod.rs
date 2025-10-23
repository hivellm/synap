//! Test utilities and mocks

use mockito::Server;
use std::sync::Arc;

/// Create a mock Synap server for testing
pub async fn create_mock_server() -> Server {
    Server::new_async().await
}

/// Common test utilities
pub mod helpers {
    use super::*;

    /// Setup a test client pointing to a mock server
    pub async fn setup_test_client() -> (crate::SynapClient, Server) {
        let server = create_mock_server().await;
        let config = crate::SynapConfig::new(server.url());
        let client = crate::SynapClient::new(config).unwrap();
        (client, server)
    }
}
