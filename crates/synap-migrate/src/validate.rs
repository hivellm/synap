//! Checks of a data directory's Hub scoping.

use crate::scope::{self, Kind};
use crate::source::{self, Layout};
use anyhow::{Context, Result, bail};
use std::collections::{BTreeSet, HashMap};
use std::path::Path;
use synap_server::QueueConfig;
use synap_server::hub::MultiTenant;
use synap_server::persistence::SnapshotImage;
use synap_server::persistence::types::StreamEvent;
use tracing::info;
use uuid::Uuid;

/// The data directory `data_dir`, read as the server would recover it, as an
/// image plus its stream section.
async fn read_image(data_dir: &Path) -> Result<(SnapshotImage, HashMap<String, Vec<StreamEvent>>)> {
    if !data_dir.is_dir() {
        bail!(
            "the data directory {} does not exist or is not a directory",
            data_dir.display()
        );
    }
    let source = source::read(&Layout::of(data_dir), QueueConfig::default()).await?;
    let image = SnapshotImage::capture(source.store_refs())
        .await
        .context("cannot copy the rebuilt stores")?;
    Ok((image, source.streams))
}

/// Check that every resource of every structure in `data_dir` lives in the
/// Hub namespace of `tenant`.
pub async fn validate_migration(data_dir: &Path, tenant: &Uuid) -> Result<()> {
    info!("Validating migration for tenant {}", tenant);
    let (image, streams) = read_image(data_dir).await?;
    let expected_prefix = MultiTenant::get_user_prefix(tenant);
    let names = scope::names(&image, &streams);
    for (kind, name) in &names {
        if scope::owner(name) != Some(*tenant) {
            bail!("{kind} {name:?} does not have the expected prefix {expected_prefix:?}");
        }
    }
    info!("Validation successful: {} resources validated", names.len());
    Ok(())
}

/// Describe the Hub scoping of `data_dir`: migrated, not migrated, or mixed.
pub async fn check_status(data_dir: &Path) -> Result<String> {
    info!("Checking migration status");
    let (image, streams) = read_image(data_dir).await?;
    let names = scope::names(&image, &streams);
    if names.is_empty() {
        return Ok("Status: EMPTY (no resources; likely a fresh install)".to_string());
    }

    let mut tenants = BTreeSet::new();
    let mut unscoped: Vec<(Kind, &str)> = Vec::new();
    for (kind, name) in &names {
        match scope::owner(name) {
            Some(tenant) => {
                tenants.insert(tenant);
            }
            None => unscoped.push((*kind, name)),
        }
    }
    let total = names.len();
    let scoped = total - unscoped.len();
    let tenant_list = tenants
        .iter()
        .map(Uuid::to_string)
        .collect::<Vec<_>>()
        .join(", ");

    Ok(if unscoped.is_empty() {
        format!(
            "Status: MIGRATED\n\
             Total resources: {total}\n\
             Scoped resources: {scoped} (100%)\n\
             Tenants: {} ({tenant_list})",
            tenants.len()
        )
    } else if scoped == 0 {
        format!(
            "Status: NOT MIGRATED\n\
             Total resources: {total}\n\
             All resources are unscoped (standalone mode)"
        )
    } else {
        let (kind, name) = unscoped[0];
        format!(
            "Status: PARTIALLY MIGRATED (WARNING)\n\
             Total resources: {total}\n\
             Scoped resources: {scoped} ({:.1}%)\n\
             Unscoped resources: {} ({:.1}%), for example {kind} {name:?}\n\
             This is an inconsistent state!",
            (scoped as f64 / total as f64) * 100.0,
            unscoped.len(),
            (unscoped.len() as f64 / total as f64) * 100.0
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::migrate::{self, Migration};
    use crate::report::QueueDefaults;
    use crate::snapshot;
    use synap_server::persistence::StoreRefs;
    use synap_server::{KVConfig, KVStore, QueueManager};

    fn tenant() -> Uuid {
        Uuid::parse_str("2c4e6a8b-1d3f-4a5b-8c7d-9e0f1a2b3c4d").unwrap()
    }

    /// A standalone data directory with a KV key and a queue.
    async fn standalone(dir: &Path) {
        let kv = KVStore::new(KVConfig::default());
        kv.set("k", b"v".to_vec(), None).await.unwrap();
        let queues = QueueManager::new(QueueConfig::default());
        queues.create_queue("q", None).await.unwrap();
        snapshot::manager(&Layout::of(dir).snapshot_dir)
            .create_snapshot(
                StoreRefs {
                    queue_manager: Some(&queues),
                    ..StoreRefs::kv_only(&kv)
                },
                1,
            )
            .await
            .unwrap();
    }

    /// Status tells a standalone directory from a migrated one, and validate
    /// accepts only the tenant the directory was migrated for.
    #[tokio::test]
    async fn status_and_validate_follow_the_migration() {
        let source = tempfile::tempdir().unwrap();
        let scratch = tempfile::tempdir().unwrap();
        let output = scratch.path().join("migrated");
        standalone(source.path()).await;

        let status = check_status(source.path()).await.unwrap();
        assert!(status.starts_with("Status: NOT MIGRATED"), "{status}");
        assert!(status.contains("Total resources: 2"), "{status}");
        let error = validate_migration(source.path(), &tenant())
            .await
            .unwrap_err();
        assert!(
            format!("{error:#}").contains("expected prefix"),
            "{error:#}"
        );

        migrate::run(&Migration {
            source: source.path().to_path_buf(),
            output: output.clone(),
            tenant: tenant(),
            queue_defaults: QueueDefaults::default(),
            dry_run: false,
        })
        .await
        .unwrap();

        let status = check_status(&output).await.unwrap();
        assert!(status.starts_with("Status: MIGRATED"), "{status}");
        assert!(status.contains(&tenant().to_string()), "{status}");
        validate_migration(&output, &tenant()).await.unwrap();
        let other = Uuid::parse_str("3d5f7b9c-2e4a-4b6c-9d8e-0f1a2b3c4d5e").unwrap();
        assert!(validate_migration(&output, &other).await.is_err());

        let empty = scratch.path().join("empty");
        let error = check_status(&empty).await.unwrap_err();
        assert!(format!("{error:#}").contains("does not exist"), "{error:#}");
        std::fs::create_dir_all(&empty).unwrap();
        let status = check_status(&empty).await.unwrap();
        assert!(status.starts_with("Status: EMPTY"), "{status}");
    }
}
