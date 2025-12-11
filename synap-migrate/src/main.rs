//! Synap Migration Tool
//!
//! Migrates standalone Synap installations to HiveHub.Cloud SaaS mode
//! by adding user namespace prefixes to all resources.

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use tracing::{info, warn};
use uuid::Uuid;

mod backup;
mod migrate;
mod rollback;
mod snapshot;
mod validate;

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
    /// Backup existing Synap data before migration
    Backup {
        /// Path to Synap data directory
        #[arg(short, long, default_value = "./data")]
        data_dir: PathBuf,

        /// Output directory for backup
        #[arg(short, long, default_value = "./backup")]
        output: PathBuf,
    },

    /// Migrate standalone data to user-scoped namespaces
    Migrate {
        /// Path to Synap data directory
        #[arg(short, long, default_value = "./data")]
        data_dir: PathBuf,

        /// User ID to assign ownership to all resources
        #[arg(short, long)]
        user_id: Uuid,

        /// Backup directory (for rollback support)
        #[arg(short, long, default_value = "./backup")]
        backup_dir: PathBuf,

        /// Perform dry-run without making changes
        #[arg(long)]
        dry_run: bool,
    },

    /// Validate migrated data
    Validate {
        /// Path to Synap data directory
        #[arg(short, long, default_value = "./data")]
        data_dir: PathBuf,

        /// Expected user ID
        #[arg(short, long)]
        user_id: Uuid,
    },

    /// Rollback migration to previous state
    Rollback {
        /// Path to Synap data directory
        #[arg(short, long, default_value = "./data")]
        data_dir: PathBuf,

        /// Backup directory to restore from
        #[arg(short, long, default_value = "./backup")]
        backup_dir: PathBuf,

        /// Force rollback without confirmation
        #[arg(short, long)]
        force: bool,
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
        Commands::Backup { data_dir, output } => {
            info!("Creating backup of Synap data");
            backup::create_backup(&data_dir, &output)
                .await
                .context("Failed to create backup")?;
            info!("Backup completed successfully");
        }

        Commands::Migrate {
            data_dir,
            user_id,
            backup_dir,
            dry_run,
        } => {
            if dry_run {
                info!("Running migration in DRY-RUN mode (no changes will be made)");
            } else {
                info!("Starting migration to user-scoped namespaces");
            }

            // Create backup first
            if !dry_run {
                info!("Creating backup before migration...");
                backup::create_backup(&data_dir, &backup_dir)
                    .await
                    .context("Failed to create backup")?;
            }

            // Perform migration
            migrate::migrate_to_hub(&data_dir, &user_id, dry_run)
                .await
                .context("Migration failed")?;

            if dry_run {
                info!("Dry-run completed successfully");
            } else {
                info!("Migration completed successfully");
                info!("Validating migrated data...");

                // Validate migration
                if let Err(e) = validate::validate_migration(&data_dir, &user_id).await {
                    warn!("Validation failed: {}", e);
                    warn!(
                        "You can rollback using: synap-migrate rollback -d {:?} -b {:?}",
                        data_dir, backup_dir
                    );
                    return Err(e);
                }

                info!("Validation passed. Migration successful!");
            }
        }

        Commands::Validate { data_dir, user_id } => {
            info!("Validating migrated data");
            validate::validate_migration(&data_dir, &user_id)
                .await
                .context("Validation failed")?;
            info!("Validation passed successfully");
        }

        Commands::Rollback {
            data_dir,
            backup_dir,
            force,
        } => {
            if !force {
                warn!("This will restore Synap data from backup, overwriting current data.");
                warn!("Use --force to confirm rollback.");
                return Ok(());
            }

            info!("Rolling back migration");
            rollback::rollback_migration(&data_dir, &backup_dir)
                .await
                .context("Rollback failed")?;
            info!("Rollback completed successfully");
        }

        Commands::Status { data_dir } => {
            info!("Checking migration status");
            let status = validate::check_status(&data_dir).await?;
            println!("{}", status);
        }
    }

    Ok(())
}
