use anyhow::Result;
use clap::Parser;
use std::net::SocketAddr;
use std::sync::Arc;
use synap_server::{KVStore, ServerConfig, create_router};
use tracing::info;

#[derive(Parser, Debug)]
#[command(name = "synap-server")]
#[command(about = "Synap - High-Performance In-Memory Key-Value Store", long_about = None)]
struct Args {
    /// Path to configuration file
    #[arg(short, long, default_value = "config.yml")]
    config: String,

    /// Server host
    #[arg(long)]
    host: Option<String>,

    /// Server port
    #[arg(long)]
    port: Option<u16>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Load configuration
    let mut config = if std::path::Path::new(&args.config).exists() {
        ServerConfig::from_file(&args.config)?
    } else {
        info!("Config file not found, using defaults");
        ServerConfig::default()
    };

    // Override with CLI args
    if let Some(host) = args.host {
        config.server.host = host;
    }
    if let Some(port) = args.port {
        config.server.port = port;
    }

    // Initialize tracing based on config
    let log_level = std::env::var("RUST_LOG").unwrap_or_else(|_| config.logging.level.clone());

    match config.logging.format.as_str() {
        "json" => {
            // JSON format for production (structured logging)
            tracing_subscriber::fmt()
                .json()
                .with_env_filter(tracing_subscriber::EnvFilter::new(log_level))
                .with_target(true)
                .with_thread_ids(true)
                .with_thread_names(true)
                .with_file(true)
                .with_line_number(true)
                .with_current_span(true)
                .init();
        }
        _ => {
            // Pretty format for development (human-readable)
            tracing_subscriber::fmt()
                .pretty()
                .with_env_filter(tracing_subscriber::EnvFilter::new(log_level))
                .with_target(true)
                .with_thread_names(true)
                .with_file(true)
                .with_line_number(true)
                .init();
        }
    }

    info!("Starting Synap Server v{}", env!("CARGO_PKG_VERSION"));
    info!("Configuration loaded from: {}", args.config);
    info!(
        "Log format: {}, level: {}",
        config.logging.format, config.logging.level
    );

    // Create KV store
    let kv_config = config.to_kv_config();
    let store = Arc::new(KVStore::new(kv_config));

    // Start TTL cleanup task
    store.start_ttl_cleanup();

    // Create router
    let app = create_router(store);

    // Bind server
    let addr: SocketAddr = config.server_addr().parse()?;
    info!("Listening on http://{}", addr);

    // Start server
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
