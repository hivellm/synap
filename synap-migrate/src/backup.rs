//! Backup functionality for Synap data

use anyhow::{Context, Result};
use std::path::Path;
use tracing::info;
use walkdir::WalkDir;

/// Create a full backup of Synap data directory
pub async fn create_backup(data_dir: &Path, backup_dir: &Path) -> Result<()> {
    info!("Creating backup from {:?} to {:?}", data_dir, backup_dir);

    // Create backup directory
    tokio::fs::create_dir_all(backup_dir)
        .await
        .context("Failed to create backup directory")?;

    // Check if data directory exists
    if !data_dir.exists() {
        info!("Data directory does not exist, skipping backup");
        return Ok(());
    }

    // Copy all files recursively
    let mut file_count = 0;
    let mut total_bytes = 0u64;

    for entry in WalkDir::new(data_dir)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();

        if path.is_file() {
            // Calculate relative path
            let rel_path = path
                .strip_prefix(data_dir)
                .context("Failed to strip prefix")?;

            let dest_path = backup_dir.join(rel_path);

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
        "Backup completed: {} files, {:.2} MB",
        file_count,
        total_bytes as f64 / 1_048_576.0
    );

    Ok(())
}
