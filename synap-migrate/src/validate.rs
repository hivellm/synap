//! Validation logic for migration

use crate::snapshot;
use anyhow::{Context, Result, bail};
use std::path::Path;
use tracing::info;
use uuid::Uuid;

/// Validate that migration was successful
pub async fn validate_migration(data_dir: &Path, user_id: &Uuid) -> Result<()> {
    info!("Validating migration for user {}", user_id);

    // Find latest snapshot
    let snapshot_path = snapshot::find_latest_snapshot(data_dir)
        .await
        .context("Failed to find snapshot")?;

    let snapshot_path = match snapshot_path {
        Some(path) => path,
        None => bail!("No snapshot found"),
    };

    // Read snapshot
    let snapshot = snapshot::read_snapshot(&snapshot_path)
        .await
        .context("Failed to read snapshot")?;

    let expected_prefix = format!("user_{}:", user_id.as_simple());

    // Validate KV keys
    for key in snapshot.kv_data.keys() {
        if !key.starts_with(&expected_prefix) {
            bail!(
                "KV key '{}' does not have expected user prefix '{}'",
                key,
                expected_prefix
            );
        }
    }

    // Validate queue names
    for queue_name in snapshot.queue_data.keys() {
        if !queue_name.starts_with(&expected_prefix) {
            bail!(
                "Queue '{}' does not have expected user prefix '{}'",
                queue_name,
                expected_prefix
            );
        }
    }

    // Validate stream names
    for stream_name in snapshot.stream_data.keys() {
        if !stream_name.starts_with(&expected_prefix) {
            bail!(
                "Stream '{}' does not have expected user prefix '{}'",
                stream_name,
                expected_prefix
            );
        }
    }

    info!("Validation successful:");
    info!("  - {} KV entries validated", snapshot.kv_data.len());
    info!("  - {} queues validated", snapshot.queue_data.len());
    info!("  - {} streams validated", snapshot.stream_data.len());

    Ok(())
}

/// Check migration status and return statistics
pub async fn check_status(data_dir: &Path) -> Result<String> {
    info!("Checking migration status");

    // Find latest snapshot
    let snapshot_path = snapshot::find_latest_snapshot(data_dir).await?;

    let snapshot_path = match snapshot_path {
        Some(path) => path,
        None => {
            return Ok("Status: No snapshot found (likely fresh install)".to_string());
        }
    };

    // Read snapshot
    let snapshot = snapshot::read_snapshot(&snapshot_path).await?;

    // Check for scoped resources
    let mut scoped_count = 0;
    let mut unscoped_count = 0;
    let mut user_ids = std::collections::HashSet::new();

    for key in snapshot.kv_data.keys() {
        if snapshot::is_scoped(key) {
            scoped_count += 1;
            if let Some(user_id_str) = extract_user_id_from_key(key) {
                user_ids.insert(user_id_str);
            }
        } else {
            unscoped_count += 1;
        }
    }

    for queue_name in snapshot.queue_data.keys() {
        if snapshot::is_scoped(queue_name) {
            scoped_count += 1;
            if let Some(user_id_str) = extract_user_id_from_key(queue_name) {
                user_ids.insert(user_id_str);
            }
        } else {
            unscoped_count += 1;
        }
    }

    for stream_name in snapshot.stream_data.keys() {
        if snapshot::is_scoped(stream_name) {
            scoped_count += 1;
            if let Some(user_id_str) = extract_user_id_from_key(stream_name) {
                user_ids.insert(user_id_str);
            }
        } else {
            unscoped_count += 1;
        }
    }

    let total_count = scoped_count + unscoped_count;

    let status = if unscoped_count == 0 && scoped_count > 0 {
        format!(
            "Status: MIGRATED\n\
             Total resources: {}\n\
             Scoped resources: {} (100%)\n\
             Unique users: {}\n\
             User IDs: {:?}",
            total_count,
            scoped_count,
            user_ids.len(),
            user_ids
        )
    } else if scoped_count == 0 {
        format!(
            "Status: NOT MIGRATED\n\
             Total resources: {}\n\
             All resources are unscoped (standalone mode)",
            total_count
        )
    } else {
        format!(
            "Status: PARTIALLY MIGRATED (WARNING)\n\
             Total resources: {}\n\
             Scoped resources: {} ({:.1}%)\n\
             Unscoped resources: {} ({:.1}%)\n\
             This is an inconsistent state!",
            total_count,
            scoped_count,
            (scoped_count as f64 / total_count as f64) * 100.0,
            unscoped_count,
            (unscoped_count as f64 / total_count as f64) * 100.0
        )
    };

    Ok(status)
}

/// Extract user ID from a scoped resource name
fn extract_user_id_from_key(key: &str) -> Option<String> {
    if key.starts_with("user_") {
        if let Some(colon_pos) = key.find(':') {
            return Some(key[5..colon_pos].to_string());
        }
    }
    None
}
