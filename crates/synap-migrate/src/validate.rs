//! Validation logic for migration

use crate::snapshot;
use crate::source::{self, Layout, Source};
use anyhow::{Context, Result, bail};
use std::path::Path;
use synap_server::QueueConfig;
use synap_server::persistence::SnapshotImage;
use tracing::info;
use uuid::Uuid;

/// The KV, queue and stream names of the data directory `data_dir`, read as
/// the server would recover it.
async fn read_names(data_dir: &Path) -> Result<(Vec<String>, Vec<String>, Vec<String>)> {
    let source: Source = source::read(&Layout::of(data_dir), QueueConfig::default()).await?;
    let image = SnapshotImage::capture(source.store_refs())
        .await
        .context("cannot copy the rebuilt stores")?;
    let kv = image.kv.into_iter().map(|(key, _, _)| key).collect();
    let queues = image.queues.into_keys().collect();
    let streams = source.streams.into_keys().collect();
    Ok((kv, queues, streams))
}

/// Validate that migration was successful
pub async fn validate_migration(data_dir: &Path, user_id: &Uuid) -> Result<()> {
    info!("Validating migration for user {}", user_id);

    let (kv_keys, queue_names, stream_names) =
        read_names(data_dir).await.context("Failed to read data")?;

    let expected_prefix = format!("user_{}:", user_id.as_simple());

    // Validate KV keys
    for key in &kv_keys {
        if !key.starts_with(&expected_prefix) {
            bail!(
                "KV key '{}' does not have expected user prefix '{}'",
                key,
                expected_prefix
            );
        }
    }

    // Validate queue names
    for queue_name in &queue_names {
        if !queue_name.starts_with(&expected_prefix) {
            bail!(
                "Queue '{}' does not have expected user prefix '{}'",
                queue_name,
                expected_prefix
            );
        }
    }

    // Validate stream names
    for stream_name in &stream_names {
        if !stream_name.starts_with(&expected_prefix) {
            bail!(
                "Stream '{}' does not have expected user prefix '{}'",
                stream_name,
                expected_prefix
            );
        }
    }

    info!("Validation successful:");
    info!("  - {} KV entries validated", kv_keys.len());
    info!("  - {} queues validated", queue_names.len());
    info!("  - {} streams validated", stream_names.len());

    Ok(())
}

/// Check migration status and return statistics
pub async fn check_status(data_dir: &Path) -> Result<String> {
    info!("Checking migration status");

    let (kv_keys, queue_names, stream_names) = read_names(data_dir).await?;

    // Check for scoped resources
    let mut scoped_count = 0;
    let mut unscoped_count = 0;
    let mut user_ids = std::collections::HashSet::new();

    for key in &kv_keys {
        if snapshot::is_scoped(key) {
            scoped_count += 1;
            if let Some(user_id_str) = extract_user_id_from_key(key) {
                user_ids.insert(user_id_str);
            }
        } else {
            unscoped_count += 1;
        }
    }

    for queue_name in &queue_names {
        if snapshot::is_scoped(queue_name) {
            scoped_count += 1;
            if let Some(user_id_str) = extract_user_id_from_key(queue_name) {
                user_ids.insert(user_id_str);
            }
        } else {
            unscoped_count += 1;
        }
    }

    for stream_name in &stream_names {
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
    if key.starts_with("user_")
        && let Some(colon_pos) = key.find(':')
    {
        return Some(key[5..colon_pos].to_string());
    }
    None
}
