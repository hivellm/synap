use anyhow::Result;
use std::net::SocketAddr;
use std::sync::Arc;
use synap_server::{KVConfig, KVStore, create_router};
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    info!("Starting Synap Server v{}", env!("CARGO_PKG_VERSION"));

    // Create KV store
    let config = KVConfig::default();
    let store = Arc::new(KVStore::new(config));

    // Start TTL cleanup task
    store.start_ttl_cleanup();

    // Create router
    let app = create_router(store);

    // Bind server
    let addr = SocketAddr::from(([0, 0, 0, 0], 15500));
    info!("Listening on http://{}", addr);

    // Start server
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
