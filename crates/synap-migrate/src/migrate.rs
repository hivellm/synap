//! Migration logic for adding user namespace prefixes

use crate::snapshot;
use crate::source::{self, Layout, Source};
use anyhow::{Context, Result, bail};
use std::path::Path;
use synap_server::QueueConfig;
use synap_server::persistence::SnapshotImage;
use tracing::{info, warn};
use uuid::Uuid;

/// Migrate standalone Synap data to user-scoped namespaces
pub async fn migrate_to_hub(data_dir: &Path, user_id: &Uuid, dry_run: bool) -> Result<()> {
    info!(
        "Starting migration for user {} (dry_run: {})",
        user_id, dry_run
    );

    // Read the directory as the server would recover it: the newest snapshot
    // plus the WAL records written after it.
    let layout = Layout::of(data_dir);
    let source = source::read(&layout, QueueConfig::default()).await?;
    log_source(&source);
    let mut image = SnapshotImage::capture(source.store_refs())
        .await
        .context("cannot copy the rebuilt stores")?;
    let mut streams = source.streams;

    // Display migration plan
    info!("Migration plan:");
    info!("  - KV entries: {}", image.kv.len());
    info!("  - Queues: {}", image.queues.len());
    info!("  - Streams: {}", streams.len());

    // Check if already migrated
    let already_scoped = image
        .kv
        .iter()
        .map(|(key, _, _)| key)
        .chain(image.queues.keys())
        .chain(streams.keys())
        .any(|name| snapshot::is_scoped(name));
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

    // Scope every name, each KV key keeping its value and expiry.
    let scope = |name: String| snapshot::scope_resource_name(user_id, &name);
    image.kv = image
        .kv
        .into_iter()
        .map(|(key, value, expires_at_ms)| (scope(key), value, expires_at_ms))
        .collect();
    image.queues = image
        .queues
        .into_iter()
        .map(|(name, queue)| (scope(name), queue))
        .collect();
    streams = streams
        .into_iter()
        .map(|(name, events)| (scope(name), events))
        .collect();

    // The migrated snapshot holds the WAL records too, so its replay starts
    // past them; it gets the next sequence number and is the one the server
    // loads.
    let migrated_path = snapshot::write(
        &layout.snapshot_dir,
        image,
        streams,
        source.stores.wal_offset,
    )
    .await
    .context("Failed to write migrated snapshot")?;
    info!("Wrote migrated snapshot to {:?}", migrated_path);

    // Archive the snapshot the migration read
    if let Some(read) = source.snapshot {
        let archive_path = read.path.with_extension("bin.pre-migration");
        info!("Archiving old snapshot to {:?}", archive_path);
        tokio::fs::rename(&read.path, &archive_path)
            .await
            .context("Failed to archive old snapshot")?;
    }

    info!("Migration completed successfully");

    Ok(())
}

/// Log which snapshot and WAL records a read used.
pub fn log_source(source: &Source) {
    match &source.snapshot {
        Some(read) => info!(
            "Snapshot {:?} (format v{}, WAL replay from offset {})",
            read.path, read.version, read.wal_offset
        ),
        None => info!("No snapshot: the state comes from the WAL alone"),
    }
    let wal = &source.wal;
    info!(
        "WAL {:?} ({:?}): {} records, {} replayed after the snapshot",
        wal.path, wal.format, wal.records, wal.replayed
    );
    if let Some(torn) = &wal.torn_tail {
        warn!(
            "WAL {:?}: {}; the server discards it at startup, and so does this read",
            wal.path, torn
        );
    }
}
