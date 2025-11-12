use anyhow::Result;
use clap::Parser;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use synap_server::auth::{ApiKeyManager, UserManager};
use synap_server::core::{HashStore, ListStore, SetStore, SortedSetStore};
use synap_server::monitoring::{ClientListManager, MonitoringManager};
use synap_server::persistence::{PersistenceLayer, recover};
use synap_server::replication::NodeRole;
use synap_server::{
    AppState, ConsumerGroupConfig, ConsumerGroupManager, KVStore, PartitionConfig,
    PartitionManager, PubSubRouter, QueueManager, ScriptManager, ServerConfig, StreamConfig,
    StreamManager, create_router, init_metrics,
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

    // Override auth config from environment variables (Docker support)
    if let Ok(enabled) = std::env::var("SYNAP_AUTH_ENABLED") {
        config.auth.enabled = enabled.parse().unwrap_or(false);
    }
    if let Ok(require_auth) = std::env::var("SYNAP_AUTH_REQUIRE_AUTH") {
        config.auth.require_auth = require_auth.parse().unwrap_or(false);
    }
    if let Ok(root_username) = std::env::var("SYNAP_AUTH_ROOT_USERNAME") {
        config.auth.root.username = root_username;
    }
    if let Ok(root_password) = std::env::var("SYNAP_AUTH_ROOT_PASSWORD") {
        config.auth.root.password = root_password;
    }
    if let Ok(root_enabled) = std::env::var("SYNAP_AUTH_ROOT_ENABLED") {
        config.auth.root.enabled = root_enabled.parse().unwrap_or(true);
    }
    if let Ok(default_ttl) = std::env::var("SYNAP_AUTH_DEFAULT_KEY_TTL") {
        if let Ok(ttl) = default_ttl.parse::<u64>() {
            config.auth.default_key_ttl = ttl;
        }
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

    #[allow(clippy::type_complexity)]
    let (
        kv_store,
        hash_store_recovered,
        list_store_recovered,
        _set_store_recovered,
        _sorted_set_store_recovered,
        queue_manager,
        _wal_offset,
    ): (
        Arc<KVStore>,
        Option<Arc<HashStore>>,
        Option<Arc<ListStore>>,
        Option<Arc<SetStore>>,
        Option<Arc<SortedSetStore>>,
        Option<Arc<QueueManager>>,
        u64,
    ) = if config.persistence.enabled {
        info!("Persistence enabled, attempting recovery...");
        match recover(&config.persistence, kv_config.clone(), queue_config.clone()).await {
            Ok((kv, hs, ls, ss, zs, qm, offset)) => {
                info!("Recovery successful, WAL offset: {}", offset);
                (
                    Arc::new(kv),
                    hs.map(Arc::new),
                    ls.map(Arc::new),
                    ss.map(Arc::new),
                    zs.map(Arc::new),
                    qm.map(Arc::new),
                    offset,
                )
            }
            Err(e) => {
                warn!("Recovery failed: {}, starting fresh", e);
                (
                    Arc::new(KVStore::new(kv_config.clone())),
                    Some(Arc::new(HashStore::new())),
                    Some(Arc::new(ListStore::new())),
                    Some(Arc::new(SetStore::new())),
                    Some(Arc::new(SortedSetStore::new())),
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
            Some(Arc::new(HashStore::new())),
            Some(Arc::new(ListStore::new())),
            Some(Arc::new(SetStore::new())),
            Some(Arc::new(SortedSetStore::new())),
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

    // Initialize partitioned stream manager (Kafka-style)
    let partition_manager = {
        let partition_mgr = Arc::new(PartitionManager::new(PartitionConfig::default()));
        partition_mgr.clone().start_compaction_task();
        info!("Partitioned Stream system enabled (Kafka-style)");
        Some(partition_mgr)
    };

    // Initialize consumer group manager
    let consumer_group_manager = {
        let cg_mgr = Arc::new(ConsumerGroupManager::new(ConsumerGroupConfig::default()));
        cg_mgr.clone().start_rebalance_task();
        info!("Consumer Group system enabled");
        Some(cg_mgr)
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
                layer.clone().start_snapshot_task(
                    kv_store.clone(),
                    queue_manager.clone(),
                    stream_manager.clone(),
                );

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

    // Use recovered hash store
    let hash_store: Arc<synap_server::core::HashStore> =
        hash_store_recovered.unwrap_or_else(|| Arc::new(synap_server::core::HashStore::new()));
    info!("Hash store initialized");

    // Use recovered list store
    let list_store: Arc<synap_server::core::ListStore> =
        list_store_recovered.unwrap_or_else(|| Arc::new(synap_server::core::ListStore::new()));
    info!("List store initialized");

    // Create set store
    let set_store = Arc::new(synap_server::core::SetStore::new());
    info!("Set store initialized");

    // Use sorted set store (fresh or recovered)
    let sorted_set_store = _sorted_set_store_recovered
        .unwrap_or_else(|| Arc::new(synap_server::core::SortedSetStore::new()));
    info!("Sorted set store initialized");

    // Create HyperLogLog store
    use synap_server::core::HyperLogLogStore;
    let hyperloglog_store = Arc::new(HyperLogLogStore::new());
    info!("HyperLogLog store initialized");

    // Create Bitmap store
    use synap_server::core::BitmapStore;
    let bitmap_store = Arc::new(BitmapStore::new());
    info!("Bitmap store initialized");

    // Create Geospatial store (depends on sorted_set_store)
    use synap_server::core::GeospatialStore;
    let geospatial_store = Arc::new(GeospatialStore::new(sorted_set_store.clone()));
    info!("Geospatial store initialized");

    // Create monitoring manager
    let monitoring = Arc::new(MonitoringManager::new(
        kv_store.clone(),
        hash_store.clone(),
        list_store.clone(),
        set_store.clone(),
        sorted_set_store.clone(),
    ));
    info!("Monitoring manager initialized");

    // Create client list manager
    let client_list_manager = Arc::new(ClientListManager::new());
    info!("Client list manager initialized");

    // Create transaction manager
    use synap_server::core::TransactionManager;
    let transaction_manager = Arc::new(TransactionManager::new(
        kv_store.clone(),
        hash_store.clone(),
        list_store.clone(),
        set_store.clone(),
        sorted_set_store.clone(),
    ));
    info!("Transaction manager initialized");

    // Create script manager (Lua scripting)
    let script_manager = Arc::new(ScriptManager::new(Duration::from_secs(5)));
    info!("Script manager initialized (default timeout: 5s)");

    // Initialize authentication managers
    let user_manager = Arc::new(UserManager::new());
    let api_key_manager = Arc::new(ApiKeyManager::new());

    // Initialize root user if authentication is enabled
    if config.auth.enabled {
        info!("Authentication enabled, initializing root user");
        user_manager
            .initialize_root_user(
                &config.auth.root.username,
                &config.auth.root.password,
                config.auth.root.enabled,
            )
            .unwrap_or_else(|e| {
                warn!("Failed to initialize root user: {}", e);
            });

        if config.auth.root.enabled {
            info!(
                "Root user initialized: {} (enabled)",
                config.auth.root.username
            );
        } else {
            warn!(
                "Root user initialized: {} (DISABLED - for security after initial setup)",
                config.auth.root.username
            );
        }
    } else {
        info!("Authentication disabled (development mode)");
    }

    // Create application state with persistence and streams
    let app_state = AppState {
        kv_store,
        hash_store,
        list_store,
        set_store,
        sorted_set_store,
        hyperloglog_store,
        bitmap_store,
        geospatial_store,
        queue_manager,
        stream_manager,
        partition_manager,
        consumer_group_manager,
        pubsub_router,
        persistence,
        monitoring,
        transaction_manager,
        script_manager,
        client_list_manager,
    };

    // Initialize Prometheus metrics
    init_metrics();

    // Create router with rate limiting and authentication
    let app = create_router(
        app_state,
        config.rate_limit.clone(),
        config.mcp.clone(),
        user_manager.clone(),
        api_key_manager.clone(),
        config.auth.enabled,
        config.auth.require_auth,
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
