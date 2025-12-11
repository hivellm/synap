//! Rollback functionality to restore from backup

use anyhow::{Context, Result, bail};
use std::path::Path;
use tracing::{info, warn};
use walkdir::WalkDir;

/// Rollback migration by restoring from backup
pub async fn rollback_migration(data_dir: &Path, backup_dir: &Path) -> Result<()> {
    info!("Rolling back migration from {:?}", backup_dir);

    // Check backup exists
    if !backup_dir.exists() {
        bail!("Backup directory does not exist: {:?}", backup_dir);
    }

    // Check if backup has content
    let backup_entries: Vec<_> = WalkDir::new(backup_dir)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
        .collect();

    if backup_entries.len() <= 1 {
        // Only the backup_dir itself
        bail!("Backup directory is empty: {:?}", backup_dir);
    }

    info!("Backup found with {} entries", backup_entries.len());

    // Clear existing data directory (except backups)
    if data_dir.exists() {
        warn!("Removing existing data from {:?}", data_dir);

        for entry in WalkDir::new(data_dir)
            .follow_links(false)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();

            // Skip the data directory itself and the backup directory
            if path == data_dir {
                continue;
            }

            // Don't delete the backup directory if it's inside data_dir
            if let Ok(stripped) = path.strip_prefix(data_dir) {
                if stripped.starts_with("backups") || stripped.starts_with("backup") {
                    continue;
                }
            }

            if path.is_file() {
                tokio::fs::remove_file(path)
                    .await
                    .with_context(|| format!("Failed to remove file: {:?}", path))?;
            } else if path.is_dir() && path != data_dir {
                // Only remove empty directories (non-empty will be removed after their contents)
                let _ = tokio::fs::remove_dir(path).await; // Ignore errors for non-empty dirs
            }
        }
    } else {
        // Create data directory if it doesn't exist
        tokio::fs::create_dir_all(data_dir)
            .await
            .context("Failed to create data directory")?;
    }

    // Restore from backup
    info!("Restoring from backup...");

    let mut file_count = 0;
    let mut total_bytes = 0u64;

    for entry in WalkDir::new(backup_dir)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();

        if path.is_file() {
            // Calculate relative path
            let rel_path = path
                .strip_prefix(backup_dir)
                .context("Failed to strip backup prefix")?;

            let dest_path = data_dir.join(rel_path);

            // Create parent directory
            if let Some(parent) = dest_path.parent() {
                tokio::fs::create_dir_all(parent)
                    .await
                    .context("Failed to create parent directory")?;
            }

            // Copy file
            let metadata = tokio::fs::metadata(path)
                .await
                .context("Failed to read file metadata")?;

            tokio::fs::copy(path, &dest_path)
                .await
                .with_context(|| format!("Failed to copy file: {:?}", path))?;

            file_count += 1;
            total_bytes += metadata.len();
        }
    }

    info!(
        "Rollback completed: {} files restored, {:.2} MB",
        file_count,
        total_bytes as f64 / 1_048_576.0
    );

    Ok(())
}
