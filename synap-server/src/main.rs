use anyhow::Result;
use clap::Parser;
use std::net::SocketAddr;
use std::sync::Arc;
use synap_server::persistence::{PersistenceLayer, recover};
use synap_server::replication::{MasterNode, NodeRole, ReplicaNode, ReplicationConfig};
use synap_server::{
    AppState, KVStore, PubSubRouter, QueueManager, ServerConfig, StreamConfig, StreamManager,
    create_router,
};
use tracing::{info, warn};

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

    /// Replication role: master, replica, or standalone
    #[arg(long, value_parser = ["master", "replica", "standalone"])]
    role: Option<String>,

    /// Master address for replica nodes (e.g., "127.0.0.1:5500")
    #[arg(long)]
    master_address: Option<String>,

    /// Listen address for replica connections (master only)
    #[arg(long)]
    replica_listen: Option<String>,

    /// Enable auto-reconnect on replica disconnect
    #[arg(long, default_value_t = true)]
    auto_reconnect: bool,
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

    // Configure replication from CLI args
    if let Some(role_str) = &args.role {
        config.replication.enabled = role_str != "standalone";
        config.replication.role = match role_str.as_str() {
            "master" => NodeRole::Master,
            "replica" => NodeRole::Replica,
            _ => NodeRole::Standalone,
        };

        if let Some(master_addr) = &args.master_address {
            config.replication.master_address = master_addr.parse().ok();
        }

        if let Some(replica_listen) = &args.replica_listen {
            config.replication.replica_listen_address = replica_listen.parse().ok();
        }

        config.replication.auto_reconnect = args.auto_reconnect;
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

    // Recover from persistence or create fresh
    let kv_config = config.to_kv_config();
    let queue_config = config.to_queue_config();

    let (kv_store, queue_manager, _wal_offset) = if config.persistence.enabled {
        info!("Persistence enabled, attempting recovery...");
        match recover(&config.persistence, kv_config.clone(), queue_config.clone()).await {
            Ok((kv, qm, offset)) => {
                info!("Recovery successful, WAL offset: {}", offset);
                (Arc::new(kv), qm.map(Arc::new), offset)
            }
            Err(e) => {
                warn!("Recovery failed: {}, starting fresh", e);
                (
                    Arc::new(KVStore::new(kv_config.clone())),
                    if config.queue.enabled {
                        Some(Arc::new(QueueManager::new(queue_config.clone())))
                    } else {
                        None
                    },
                    0,
                )
            }
        }
    } else {
        info!("Persistence disabled, starting fresh");
        (
            Arc::new(KVStore::new(kv_config.clone())),
            if config.queue.enabled {
                Some(Arc::new(QueueManager::new(queue_config.clone())))
            } else {
                None
            },
            0,
        )
    };

    // Start TTL cleanup task
    kv_store.start_ttl_cleanup();

    // Start queue deadline checker if queue enabled
    if let Some(ref qm) = queue_manager {
        qm.start_deadline_checker();
        info!("Queue system enabled");
    } else {
        info!("Queue system disabled");
    }

    // Initialize stream manager (enabled by default for now)
    let stream_manager = {
        let stream_mgr = Arc::new(StreamManager::new(StreamConfig::default()));
        stream_mgr.clone().start_compaction_task();
        info!("Event Stream system enabled");
        Some(stream_mgr)
    };

    // Initialize Pub/Sub router (enabled by default for now)
    let pubsub_router = {
        let router = Arc::new(PubSubRouter::new());
        info!("Pub/Sub system enabled");
        Some(router)
    };

    // Create persistence layer if enabled
    let persistence = if config.persistence.enabled {
        match PersistenceLayer::new(config.persistence.clone()).await {
            Ok(layer) => {
                let layer = Arc::new(layer);

                // Start background snapshot task
                layer
                    .clone()
                    .start_snapshot_task(kv_store.clone(), queue_manager.clone());

                info!("Persistence layer initialized (WAL + Snapshots)");
                Some(layer)
            }
            Err(e) => {
                warn!("Failed to initialize persistence: {}", e);
                None
            }
        }
    } else {
        None
    };

    // Create application state with persistence and streams
    let app_state = AppState {
        kv_store,
        queue_manager,
        stream_manager,
        pubsub_router,
        persistence,
    };

    // Create router with rate limiting
    let app = create_router(
        app_state,
        config.rate_limit.enabled,
        config.rate_limit.requests_per_second,
    );

    if config.rate_limit.enabled {
        info!(
            "Rate limiting enabled: {} req/s (burst: {})",
            config.rate_limit.requests_per_second, config.rate_limit.burst_size
        );
    }

    // Bind server
    let addr: SocketAddr = config.server_addr().parse()?;
    info!("Listening on http://{}", addr);

    // Start server
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
