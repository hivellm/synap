//! Migration of a standalone data directory into one tenant's Hub namespace.

use crate::report::{Counts, QueueDefaults, Report};
use crate::scope;
use crate::snapshot;
use crate::source::{self, Layout};
use anyhow::{Context, Result, bail};
use std::path::Path;
use synap_server::persistence::SnapshotImage;
use tracing::info;
use uuid::Uuid;

/// Migrate the data directory `data_dir` into the Hub namespace of `tenant`:
/// rebuild it as the server would recover it, rename every resource the way
/// Hub mode names it for that tenant, and write the result as one snapshot.
pub async fn migrate_to_hub(
    data_dir: &Path,
    tenant: &Uuid,
    dry_run: bool,
    queue_defaults: &QueueDefaults,
) -> Result<Report> {
    info!(
        "Starting migration for tenant {} (dry_run: {})",
        tenant, dry_run
    );

    // Read the directory as the server would recover it: the newest snapshot
    // plus the WAL records written after it.
    let layout = Layout::of(data_dir);
    let source = source::read(&layout, queue_defaults.config.clone()).await?;
    let image = SnapshotImage::capture(source.store_refs())
        .await
        .context("cannot copy the rebuilt stores")?;
    let mut report = Report::new(&source, Counts::of(&image, &source.streams), queue_defaults);

    // Data already in a Hub namespace would be scoped twice.
    if let Some((kind, name)) = scope::names(&image, &source.streams)
        .into_iter()
        .find(|(_, name)| scope::owner(name).is_some())
    {
        bail!("the input already holds Hub-scoped names ({kind} {name:?}): it was migrated before");
    }

    if dry_run {
        return Ok(report);
    }

    let scoped = scope::image(image, tenant);
    let streams = scope::streams(source.streams, tenant);

    // The migrated snapshot holds the WAL records too, so its replay starts
    // past them; it gets the next sequence number and is the one the server
    // loads.
    let written = snapshot::write(
        &layout.snapshot_dir,
        scoped,
        streams,
        source.stores.wal_offset,
    )
    .await?;
    info!("Wrote migrated snapshot to {:?}", written);

    // Archive the snapshot the migration read
    if let Some(read) = source.snapshot {
        let archive_path = read.path.with_extension("bin.pre-migration");
        info!("Archiving old snapshot to {:?}", archive_path);
        tokio::fs::rename(&read.path, &archive_path)
            .await
            .context("Failed to archive old snapshot")?;
    }
    report.written = Some(written);
    Ok(report)
}
