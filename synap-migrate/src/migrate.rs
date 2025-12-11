//! Migration logic for adding user namespace prefixes

use crate::snapshot::{self, SnapshotData, SnapshotMetadata};
use anyhow::{Context, Result, bail};
use indicatif::{ProgressBar, ProgressStyle};
use std::collections::HashMap;
use std::path::Path;
use tracing::{info, warn};
use uuid::Uuid;

/// Migrate standalone Synap data to user-scoped namespaces
pub async fn migrate_to_hub(data_dir: &Path, user_id: &Uuid, dry_run: bool) -> Result<()> {
    info!(
        "Starting migration for user {} (dry_run: {})",
        user_id, dry_run
    );

    // Find latest snapshot
    let snapshot_path = snapshot::find_latest_snapshot(data_dir)
        .await
        .context("Failed to find snapshot")?;

    let snapshot_path = match snapshot_path {
        Some(path) => path,
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

    info!("Reading snapshot: {:?}", snapshot_path);
    let mut snapshot = snapshot::read_snapshot(&snapshot_path)
        .await
        .context("Failed to read snapshot")?;

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

    // Migrate KV data
    pb.set_message("Migrating KV data...");
    let mut new_kv_data = HashMap::new();
    for (key, value) in snapshot.kv_data {
        let scoped_key = snapshot::scope_resource_name(user_id, &key);
        new_kv_data.insert(scoped_key, value);
        pb.inc(1);
    }
    snapshot.kv_data = new_kv_data;

    // Migrate queue data
    pb.set_message("Migrating queue data...");
    let mut new_queue_data = HashMap::new();
    for (queue_name, messages) in snapshot.queue_data {
        let scoped_name = snapshot::scope_resource_name(user_id, &queue_name);
        new_queue_data.insert(scoped_name, messages);
        pb.inc(1);
    }
    snapshot.queue_data = new_queue_data;

    // Migrate stream data
    pb.set_message("Migrating stream data...");
    let mut new_stream_data = HashMap::new();
    for (stream_name, entries) in snapshot.stream_data {
        let scoped_name = snapshot::scope_resource_name(user_id, &stream_name);
        new_stream_data.insert(scoped_name, entries);
        pb.inc(1);
    }
    snapshot.stream_data = new_stream_data;

    pb.finish_with_message("Migration complete");

    // Update metadata
    snapshot.metadata.kv_count = snapshot.kv_data.len() as u64;
    snapshot.metadata.queue_count = snapshot.queue_data.len() as u64;
    snapshot.metadata.stream_count = snapshot.stream_data.len() as u64;

    // Write migrated snapshot
    let migrated_path = data_dir.join("snapshots").join(format!(
        "snapshot-v2-{}.bin",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    ));

    info!("Writing migrated snapshot to {:?}", migrated_path);
    snapshot::write_snapshot(&migrated_path, &snapshot)
        .await
        .context("Failed to write migrated snapshot")?;

    // Archive old snapshot
    let archive_path = snapshot_path.with_extension("bin.pre-migration");
    info!("Archiving old snapshot to {:?}", archive_path);
    tokio::fs::rename(&snapshot_path, &archive_path)
        .await
        .context("Failed to archive old snapshot")?;

    info!("Migration completed successfully");

    Ok(())
}

/// Check if snapshot data is already migrated
fn check_if_migrated(snapshot: &SnapshotData) -> bool {
    // Check KV keys
    for key in snapshot.kv_data.keys() {
        if snapshot::is_scoped(key) {
            return true;
        }
    }

    // Check queue names
    for queue_name in snapshot.queue_data.keys() {
        if snapshot::is_scoped(queue_name) {
            return true;
        }
    }

    // Check stream names
    for stream_name in snapshot.stream_data.keys() {
        if snapshot::is_scoped(stream_name) {
            return true;
        }
    }

    false
}

/// Create an empty snapshot for fresh installs
async fn create_empty_snapshot(data_dir: &Path) -> Result<()> {
    let snapshot_path = data_dir.join("snapshots").join(format!(
        "snapshot-v2-{}.bin",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    ));

    let empty_snapshot = SnapshotData {
        metadata: SnapshotMetadata {
            version: 2,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            wal_offset: 0,
            kv_count: 0,
            queue_count: 0,
            stream_count: 0,
        },
        kv_data: HashMap::new(),
        queue_data: HashMap::new(),
        stream_data: HashMap::new(),
    };

    snapshot::write_snapshot(&snapshot_path, &empty_snapshot).await?;

    info!("Created empty snapshot for migrated state");
    Ok(())
}
