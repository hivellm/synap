//! Migration logic for adding user namespace prefixes

use crate::snapshot;
use anyhow::{Context, Result, bail};
use indicatif::{ProgressBar, ProgressStyle};
use std::collections::HashMap;
use std::path::Path;
use synap_server::persistence::Snapshot;
use tracing::{info, warn};
use uuid::Uuid;

/// Migrate standalone Synap data to user-scoped namespaces
pub async fn migrate_to_hub(data_dir: &Path, user_id: &Uuid, dry_run: bool) -> Result<()> {
    info!(
        "Starting migration for user {} (dry_run: {})",
        user_id, dry_run
    );

    // Find and load the latest snapshot
    let (mut snapshot, snapshot_path) = match snapshot::load_latest(data_dir).await? {
        Some(found) => found,
        None => {
            warn!("No snapshot found - checking if data directory is empty");

            // Check if data directory has any content
            let snapshot_dir = data_dir.join("snapshots");
            if !snapshot_dir.exists() {
                info!("No snapshots directory - assuming fresh install");
                if !dry_run {
                    info!("Creating empty migrated snapshot");
                    create_empty_snapshot(data_dir).await?;
                }
                return Ok(());
            }

            bail!("Data directory exists but no valid snapshot found");
        }
    };
    info!("Read snapshot: {:?}", snapshot_path);

    // Display migration plan
    info!("Migration plan:");
    info!("  - KV entries: {}", snapshot.kv_data.len());
    info!("  - Queues: {}", snapshot.queue_data.len());
    info!("  - Streams: {}", snapshot.stream_data.len());

    // Check if already migrated
    let already_scoped = check_if_migrated(&snapshot);
    if already_scoped {
        warn!("Data appears to already be migrated (contains user_ prefixes)");
        if !dry_run {
            bail!("Cannot migrate already-migrated data. Use rollback first if needed.");
        }
        return Ok(());
    }

    if dry_run {
        info!("DRY RUN - No changes will be made");
        return Ok(());
    }

    // Perform migration
    let pb = ProgressBar::new(
        (snapshot.kv_data.len() + snapshot.queue_data.len() + snapshot.stream_data.len()) as u64,
    );
    pb.set_style(
        ProgressStyle::default_bar()
            .template("[{elapsed_precise}] {bar:40.cyan/blue} {pos}/{len} {msg}")?
            .progress_chars("=>-"),
    );

    // Migrate KV data, each key keeping its expiry
    pb.set_message("Migrating KV data...");
    let scope = |name: String| snapshot::scope_resource_name(user_id, &name);
    snapshot.kv_data = snapshot
        .kv_data
        .into_iter()
        .map(|(key, value)| {
            pb.inc(1);
            (scope(key), value)
        })
        .collect();
    snapshot.kv_expires_at_ms = snapshot
        .kv_expires_at_ms
        .into_iter()
        .map(|(key, deadline)| (scope(key), deadline))
        .collect();

    // Migrate queue data
    pb.set_message("Migrating queue data...");
    let mut new_queue_data = HashMap::new();
    for (queue_name, queue) in snapshot.queue_data {
        new_queue_data.insert(scope(queue_name), queue);
        pb.inc(1);
    }
    snapshot.queue_data = new_queue_data;

    // Migrate stream data
    pb.set_message("Migrating stream data...");
    let mut new_stream_data = HashMap::new();
    for (stream_name, entries) in snapshot.stream_data {
        new_stream_data.insert(scope(stream_name), entries);
        pb.inc(1);
    }
    snapshot.stream_data = new_stream_data;

    pb.finish_with_message("Migration complete");

    // Archive the old snapshot first: the server loads the newest snapshot
    // file, and the migrated one must be the only candidate.
    let archive_path = snapshot_path.with_extension("bin.pre-migration");
    info!("Archiving old snapshot to {:?}", archive_path);
    tokio::fs::rename(&snapshot_path, &archive_path)
        .await
        .context("Failed to archive old snapshot")?;

    // Write migrated snapshot
    let migrated_path = snapshot::write(data_dir, snapshot)
        .await
        .context("Failed to write migrated snapshot")?;
    info!("Wrote migrated snapshot to {:?}", migrated_path);

    info!("Migration completed successfully");

    Ok(())
}

/// Check if snapshot data is already migrated
fn check_if_migrated(snapshot: &Snapshot) -> bool {
    snapshot
        .kv_data
        .keys()
        .chain(snapshot.queue_data.keys())
        .chain(snapshot.stream_data.keys())
        .any(|name| snapshot::is_scoped(name))
}

/// Create an empty snapshot for fresh installs
async fn create_empty_snapshot(data_dir: &Path) -> Result<()> {
    let empty_snapshot = Snapshot {
        version: 0,
        timestamp: 0,
        wal_offset: 0,
        kv_data: HashMap::new(),
        queue_data: HashMap::new(),
        stream_data: HashMap::new(),
        list_data: HashMap::new(),
        set_data: HashMap::new(),
        sorted_set_data: HashMap::new(),
        hash_data: HashMap::new(),
        kv_expires_at_ms: HashMap::new(),
        hyperloglog_data: HashMap::new(),
        bitmap_data: HashMap::new(),
    };

    snapshot::write(data_dir, empty_snapshot).await?;

    info!("Created empty snapshot for migrated state");
    Ok(())
}
