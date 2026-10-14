//! Synap Migration Tool
//!
//! Migrates a standalone Synap data directory into one tenant's namespace for
//! HiveHub.Cloud (Hub) mode, writing the result to a new directory.

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use tracing::info;
use uuid::Uuid;

use synap_migrate::migrate::{self, Migration};
use synap_migrate::report::QueueDefaults;
use synap_migrate::validate;

#[derive(Parser)]
#[command(name = "synap-migrate")]
#[command(about = "Migration tool for Synap standalone to HiveHub.Cloud SaaS mode", long_about = None)]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Enable verbose logging
    #[arg(short, long, global = true)]
    verbose: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Migrate a standalone data directory into a tenant's Hub namespace,
    /// writing the result to a new directory (the source is only read)
    Migrate {
        /// Path to the standalone Synap data directory (`snapshots/`,
        /// `wal/synap.wal`); it is not modified
        #[arg(short, long, default_value = "./data")]
        data_dir: PathBuf,

        /// Directory to write the migrated data to; it must not exist yet or
        /// be empty, and must not be inside the data directory
        #[arg(short, long)]
        output: PathBuf,

        /// Hub user ID (tenant) that owns every migrated resource
        #[arg(short, long)]
        user_id: Uuid,

        /// Configuration file of the server that wrote the data; its queue
        /// settings configure the queues the data does not record a
        /// configuration for (snapshots older than v5)
        #[arg(long)]
        server_config: Option<PathBuf>,

        /// Verify the input and report what would be migrated, without
        /// writing anything
        #[arg(long)]
        dry_run: bool,
    },

    /// Check that every resource of a data directory belongs to one tenant
    Validate {
        /// Path to Synap data directory
        #[arg(short, long, default_value = "./data")]
        data_dir: PathBuf,

        /// Expected user ID
        #[arg(short, long)]
        user_id: Uuid,
    },

    /// Show migration status and statistics
    Status {
        /// Path to Synap data directory
        #[arg(short, long, default_value = "./data")]
        data_dir: PathBuf,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize logging
    let log_level = if cli.verbose { "debug" } else { "info" };
    tracing_subscriber::fmt()
        .with_env_filter(log_level)
        .with_target(false)
        .init();

    match cli.command {
        Commands::Migrate {
            data_dir,
            output,
            user_id,
            server_config,
            dry_run,
        } => {
            let queue_defaults = match server_config {
                Some(path) => QueueDefaults {
                    config: synap_server::ServerConfig::from_file(&path)
                        .with_context(|| {
                            format!("cannot read the server configuration {}", path.display())
                        })?
                        .to_queue_config(),
                    origin: format!("the queue settings of {}", path.display()),
                },
                None => QueueDefaults::default(),
            };
            let migration = Migration {
                source: data_dir,
                output,
                tenant: user_id,
                queue_defaults,
                dry_run,
            };
            let report = migrate::run(&migration)
                .await
                .context("Migration failed (the source directory is unchanged)")?;
            println!("{report}");
            if !dry_run {
                println!(
                    "The source directory {} is unchanged and stays the rollback. Point the Hub-mode server's persistence at {} (snapshots/ and wal/synap.wal).",
                    migration.source.display(),
                    migration.output.display()
                );
            }
        }

        Commands::Validate { data_dir, user_id } => {
            info!("Validating migrated data");
            validate::validate_migration(&data_dir, &user_id)
                .await
                .context("Validation failed")?;
            info!("Validation passed successfully");
        }

        Commands::Status { data_dir } => {
            let status = validate::check_status(&data_dir).await?;
            println!("{}", status);
        }
    }

    Ok(())
}
