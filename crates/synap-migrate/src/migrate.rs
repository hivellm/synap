//! Migration of a standalone data directory into one tenant's Hub namespace.
//!
//! The source directory is only read. The whole input (every snapshot file
//! and the WAL) is verified before anything is written, and the result goes
//! to a new or empty output directory, so the source stays the rollback.

use crate::report::{Counts, QueueDefaults, Report};
use crate::scope;
use crate::snapshot;
use crate::source::{self, Layout};
use anyhow::{Context, Result, bail};
use std::path::{Path, PathBuf};
use synap_server::persistence::SnapshotImage;
use tracing::info;
use uuid::Uuid;

/// WAL cutoff of the migrated snapshot. It holds every record the source
/// had, and the output starts with no WAL: the server creates one there and
/// gives its first record offset 1, so replay starts at 1, the cutoff a
/// fresh server's first snapshot gets.
const OUTPUT_WAL_CUTOFF: u64 = 1;

/// What to migrate, where, and for whom.
#[derive(Debug, Clone)]
pub struct Migration {
    /// The standalone data directory (`snapshots/`, `wal/synap.wal`). It is
    /// only read.
    pub source: PathBuf,
    /// Where the migrated data directory is written: a path that does not
    /// exist yet or an empty directory, outside the source.
    pub output: PathBuf,
    /// The Hub tenant (user ID) that owns every migrated resource.
    pub tenant: Uuid,
    pub queue_defaults: QueueDefaults,
    /// Migrate even when every resource name already looks Hub-scoped, for
    /// standalone data whose names only look that way.
    pub allow_scoped_names: bool,
    /// Verify and report without writing the output.
    pub dry_run: bool,
}

/// Run `migration`: check the output location, rebuild the source as the
/// server would recover it (verifying every snapshot file and the whole WAL),
/// rename every resource the way Hub mode names it for the tenant, and write
/// the result as one snapshot under `<output>/snapshots/`.
pub async fn run(migration: &Migration) -> Result<Report> {
    info!(
        "Migrating {:?} into {:?} for tenant {} (dry run: {})",
        migration.source, migration.output, migration.tenant, migration.dry_run
    );
    check_locations(&migration.source, &migration.output)?;

    // Read the directory as the server would recover it: the newest snapshot
    // plus the WAL records written after it. Every snapshot file and the
    // whole WAL are verified here, before anything is written.
    let source = source::read(
        &Layout::of(&migration.source),
        migration.queue_defaults.config.clone(),
    )
    .await?;
    let image = SnapshotImage::capture(source.store_refs())
        .await
        .context("cannot copy the rebuilt stores")?;
    let counts = Counts::of(&image, &source.streams);
    let mut report = Report::new(&source, counts.clone(), &migration.queue_defaults);

    // Data already in a Hub namespace would be scoped twice. A migration
    // writes only Hub-scoped names, but standalone names can look the same
    // (`user_<UUID>:cart`), and scoping those is correct: the tenant reads
    // them under their original names. So only input whose names all look
    // scoped is taken for migrated data.
    let names = scope::names(&image, &source.streams);
    let looks_scoped: Vec<(scope::Kind, &str)> = names
        .iter()
        .copied()
        .filter(|(_, name)| scope::owner(name).is_some())
        .collect();
    if let Some(&(kind, name)) = looks_scoped.first() {
        if looks_scoped.len() == names.len() && !migration.allow_scoped_names {
            bail!(
                "every resource name in the input is already Hub-scoped (for example {kind} {name:?}), as in a directory migrated before: migrating it would scope each name twice. If it is standalone data whose names only look scoped, run again with --allow-scoped-names"
            );
        }
        report.notes.push(format!(
            "{} of {} resource names already look Hub-scoped (for example {kind} {name:?}); they are scoped like every other name, so the tenant reads them under their original names",
            looks_scoped.len(),
            names.len()
        ));
    }

    if migration.dry_run {
        return Ok(report);
    }

    let scoped = scope::image(image, &migration.tenant);
    let streams = scope::streams(source.streams, &migration.tenant);
    let snapshot_dir = Layout::of(&migration.output).snapshot_dir;
    let written = snapshot::write(&snapshot_dir, scoped, streams, OUTPUT_WAL_CUTOFF).await?;
    verify_written(&snapshot_dir, &written, &counts).await?;
    info!("Wrote the migrated snapshot {:?}", written);
    report.written = Some(written);
    Ok(report)
}

/// Check that the source is a directory and that `output` can receive the
/// migration without touching it: `output` does not exist yet or is an empty
/// directory, and it is not inside the source.
fn check_locations(source: &Path, output: &Path) -> Result<()> {
    if !source.is_dir() {
        bail!(
            "the data directory {} does not exist or is not a directory",
            source.display()
        );
    }
    match std::fs::read_dir(output) {
        Ok(mut entries) => {
            if entries.next().is_some() {
                bail!(
                    "the output {} is not empty: the migration writes only to a new or empty directory",
                    output.display()
                );
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => bail!("cannot use {} as the output: {e}", output.display()),
    }
    let source_dir = std::fs::canonicalize(source)
        .with_context(|| format!("cannot resolve the data directory {}", source.display()))?;
    if resolve(output)?.starts_with(&source_dir) {
        bail!(
            "the output {} is inside the data directory {}, which the migration must leave unchanged",
            output.display(),
            source.display()
        );
    }
    Ok(())
}

/// `path` as an absolute path whose existing part is canonical, so it
/// compares with a canonical directory even when it does not exist yet.
fn resolve(path: &Path) -> Result<PathBuf> {
    let absolute =
        std::path::absolute(path).with_context(|| format!("cannot resolve {}", path.display()))?;
    let mut existing = absolute.as_path();
    let mut missing = Vec::new();
    while !existing.exists() {
        let (Some(name), Some(parent)) = (existing.file_name(), existing.parent()) else {
            break;
        };
        missing.push(name.to_os_string());
        existing = parent;
    }
    let mut resolved = std::fs::canonicalize(existing)
        .with_context(|| format!("cannot resolve {}", existing.display()))?;
    resolved.extend(missing.iter().rev());
    Ok(resolved)
}

/// Load the snapshot just written with the server's loader and check that it
/// holds what was meant to be written.
async fn verify_written(snapshot_dir: &Path, written: &Path, counts: &Counts) -> Result<()> {
    let snapshot = snapshot::manager(snapshot_dir)
        .load(written)
        .await
        .with_context(|| format!("the migrated snapshot {} does not load", written.display()))?;
    let found = Counts {
        kv_keys: snapshot.kv_data.len(),
        kv_expiring: snapshot.kv_expires_at_ms.len(),
        hashes: snapshot.hash_data.len(),
        lists: snapshot.list_data.len(),
        sets: snapshot.set_data.len(),
        sorted_sets: snapshot.sorted_set_data.len(),
        hyperloglogs: snapshot.hyperloglog_data.len(),
        bitmaps: snapshot.bitmap_data.len(),
        queues: snapshot.queue_data.len(),
        queue_messages: snapshot
            .queue_data
            .values()
            .map(|q| q.ready.len() + q.in_flight.len())
            .sum(),
        dead_letters: snapshot
            .queue_data
            .values()
            .map(|q| q.dead_letter.len())
            .sum(),
        streams: snapshot.stream_data.len(),
        stream_events: snapshot.stream_data.values().map(Vec::len).sum(),
    };
    if &found != counts {
        bail!(
            "the migrated snapshot {} does not hold what was written (expected\n{counts}\nfound\n{found}); delete it and report this",
            written.display()
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use synap_server::hub::MultiTenant;
    use synap_server::persistence::wal_frame;
    use synap_server::persistence::{Operation, StoreRefs, WALEntry};
    use synap_server::{KVConfig, KVStore};

    fn tenant() -> Uuid {
        Uuid::parse_str("0f1e2d3c-4b5a-4968-8776-a5b4c3d2e1f0").unwrap()
    }

    /// Every file under `dir` with its bytes.
    fn tree(dir: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
        let mut files = BTreeMap::new();
        let mut pending = vec![dir.to_path_buf()];
        while let Some(current) = pending.pop() {
            for entry in std::fs::read_dir(&current).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    pending.push(path);
                } else {
                    let bytes = std::fs::read(&path).unwrap();
                    files.insert(path.strip_prefix(dir).unwrap().to_path_buf(), bytes);
                }
            }
        }
        files
    }

    fn set(offset: u64, key: &str) -> WALEntry {
        WALEntry {
            offset,
            timestamp: 1_700_000_000,
            operation: Operation::KVSet {
                key: key.into(),
                value: key.as_bytes().to_vec(),
                ttl: None,
            },
        }
    }

    /// A source directory with two snapshots (cutoffs 1 and 2) and a
    /// versioned WAL holding records 1 to 3; returns the WAL bytes.
    async fn source_dir(dir: &Path) -> Vec<u8> {
        let layout = Layout::of(dir);
        let manager = snapshot::manager(&layout.snapshot_dir);
        let kv = KVStore::new(KVConfig::default());
        manager
            .create_snapshot(StoreRefs::kv_only(&kv), 1)
            .await
            .unwrap();
        kv.set("first", b"first".to_vec(), None).await.unwrap();
        manager
            .create_snapshot(StoreRefs::kv_only(&kv), 2)
            .await
            .unwrap();
        let mut wal = wal_frame::encode_file_header().to_vec();
        for entry in [set(1, "first"), set(2, "second"), set(3, "third")] {
            wal.extend_from_slice(&wal_frame::encode_frame(&entry).unwrap());
        }
        std::fs::create_dir_all(layout.wal_path.parent().unwrap()).unwrap();
        std::fs::write(&layout.wal_path, &wal).unwrap();
        wal
    }

    fn migration(source: &Path, output: &Path) -> Migration {
        Migration {
            source: source.to_path_buf(),
            output: output.to_path_buf(),
            tenant: tenant(),
            queue_defaults: QueueDefaults::default(),
            allow_scoped_names: false,
            dry_run: false,
        }
    }

    async fn failure(migration: &Migration) -> String {
        format!("{:#}", run(migration).await.unwrap_err())
    }

    /// The migration writes one snapshot, in the server's current format and
    /// naming, to the output only; the source is byte-for-byte unchanged.
    #[tokio::test]
    async fn migration_writes_only_the_output() {
        let source = tempfile::tempdir().unwrap();
        let scratch = tempfile::tempdir().unwrap();
        let output = scratch.path().join("migrated");
        source_dir(source.path()).await;
        let before = tree(source.path());

        let report = run(&migration(source.path(), &output)).await.unwrap();

        assert_eq!(tree(source.path()), before);
        let written = report.written.unwrap();
        assert_eq!(
            written,
            output
                .join("snapshots")
                .join("snapshot-v5-00000000000000000001.bin")
        );
        assert_eq!(tree(&output).len(), 1, "only the migrated snapshot");
        assert_eq!(report.counts.kv_keys, 3);
        let snapshot = snapshot::manager(&output.join("snapshots"))
            .load(&written)
            .await
            .unwrap();
        assert_eq!(snapshot.wal_offset, OUTPUT_WAL_CUTOFF);
        for key in ["first", "second", "third"] {
            let scoped = MultiTenant::scope_kv_key(Some(&tenant()), key).into_owned();
            assert_eq!(snapshot.kv_data[&scoped], key.as_bytes().to_vec());
        }

        // A second run over the migrated data is refused: every name is
        // scoped already.
        let again = scratch.path().join("again");
        let error = failure(&migration(&output, &again)).await;
        assert!(error.contains("migrated before"), "{error}");
        assert!(error.contains("--allow-scoped-names"), "{error}");
        assert!(!again.exists());
    }

    /// Standalone names shaped like Hub-scoped ones (`user_<UUID>:cart`) are
    /// scoped like any other, so the tenant reads them under their original
    /// names. Input whose names all look scoped is refused as migrated data
    /// unless the caller says it is standalone data.
    #[tokio::test]
    async fn standalone_names_that_look_scoped_are_migrated() {
        let looks_scoped = "user_0123456789abcdef0123456789abcdef:cart";
        let scratch = tempfile::tempdir().unwrap();
        let source = scratch.path().join("source");
        let mut wal = wal_frame::encode_file_header().to_vec();
        for entry in [set(1, looks_scoped), set(2, "plain")] {
            wal.extend_from_slice(&wal_frame::encode_frame(&entry).unwrap());
        }
        let wal_path = Layout::of(&source).wal_path;
        std::fs::create_dir_all(wal_path.parent().unwrap()).unwrap();
        std::fs::write(&wal_path, &wal).unwrap();

        let output = scratch.path().join("mixed");
        let report = run(&migration(&source, &output)).await.unwrap();
        assert!(
            report
                .notes
                .iter()
                .any(|n| n.contains("1 of 2 resource names")),
            "{:?}",
            report.notes
        );
        let snapshot = snapshot::manager(&output.join("snapshots"))
            .load(report.written.as_ref().unwrap())
            .await
            .unwrap();
        for key in [looks_scoped, "plain"] {
            let scoped = MultiTenant::scope_kv_key(Some(&tenant()), key).into_owned();
            assert_eq!(snapshot.kv_data[&scoped], key.as_bytes().to_vec(), "{key}");
        }

        // Only names that look scoped: taken for migrated data, unless allowed.
        let mut wal = wal_frame::encode_file_header().to_vec();
        wal.extend_from_slice(&wal_frame::encode_frame(&set(1, looks_scoped)).unwrap());
        std::fs::write(&wal_path, &wal).unwrap();
        let refused = scratch.path().join("refused");
        let error = failure(&migration(&source, &refused)).await;
        assert!(error.contains("already Hub-scoped"), "{error}");
        assert!(!refused.exists());
        let allowed = scratch.path().join("allowed");
        let report = run(&Migration {
            allow_scoped_names: true,
            ..migration(&source, &allowed)
        })
        .await
        .unwrap();
        assert_eq!(report.counts.kv_keys, 1);
        assert!(report.written.is_some());
    }

    /// A damaged snapshot file, even one older than the newest, fails the run
    /// before anything is written, naming the file and the reason.
    #[tokio::test]
    async fn a_damaged_snapshot_is_rejected_before_writing() {
        let source = tempfile::tempdir().unwrap();
        let scratch = tempfile::tempdir().unwrap();
        let output = scratch.path().join("migrated");
        source_dir(source.path()).await;
        let oldest = snapshot::manager(&Layout::of(source.path()).snapshot_dir)
            .list_snapshots()
            .await
            .unwrap()
            .remove(0);
        let mut bytes = std::fs::read(&oldest).unwrap();
        let last = bytes.len() - 1;
        bytes[last] ^= 0xFF;
        std::fs::write(&oldest, &bytes).unwrap();
        let before = tree(source.path());

        let error = failure(&migration(source.path(), &output)).await;
        let name = oldest.file_name().unwrap().to_string_lossy().into_owned();
        assert!(error.contains(&name), "{error}");
        assert!(error.contains("checksum mismatch"), "{error}");
        assert!(!output.exists());
        assert_eq!(tree(source.path()), before);
    }

    /// Interior WAL corruption, a WAL format and a snapshot format newer than
    /// this release fail the run before anything is written.
    #[tokio::test]
    async fn corrupt_or_unsupported_input_is_rejected_before_writing() {
        // Interior corruption: a damaged record followed by a valid one.
        let source = tempfile::tempdir().unwrap();
        let scratch = tempfile::tempdir().unwrap();
        let output = scratch.path().join("migrated");
        let mut wal = source_dir(source.path()).await;
        let second_record = wal_frame::FILE_HEADER_LEN as usize
            + wal_frame::encode_frame(&set(1, "first")).unwrap().len();
        wal[second_record + 20] ^= 0xFF;
        let wal_path = Layout::of(source.path()).wal_path;
        std::fs::write(&wal_path, &wal).unwrap();
        let error = failure(&migration(source.path(), &output)).await;
        assert!(error.contains("synap.wal"), "{error}");
        assert!(error.contains("interior corruption"), "{error}");
        assert!(!output.exists());

        // A WAL header declaring a newer format.
        let source = tempfile::tempdir().unwrap();
        let mut wal = source_dir(source.path()).await;
        wal[8..12].copy_from_slice(&(wal_frame::WAL_FORMAT_VERSION + 1).to_be_bytes());
        std::fs::write(Layout::of(source.path()).wal_path, &wal).unwrap();
        let error = failure(&migration(source.path(), &output)).await;
        assert!(error.contains("format version 3"), "{error}");
        assert!(!output.exists());

        // A snapshot written by a newer release.
        let source = tempfile::tempdir().unwrap();
        source_dir(source.path()).await;
        let newest = snapshot::manager(&Layout::of(source.path()).snapshot_dir)
            .list_snapshots()
            .await
            .unwrap()
            .pop()
            .unwrap();
        let mut bytes = std::fs::read(&newest).unwrap();
        bytes[..8].copy_from_slice(b"SYNAP006");
        std::fs::write(&newest, &bytes).unwrap();
        let error = failure(&migration(source.path(), &output)).await;
        assert!(error.contains("SYNAP006"), "{error}");
        assert!(error.contains("newer release"), "{error}");
        assert!(!output.exists());
    }

    /// The output must be new or empty and outside the source; an empty
    /// existing directory is accepted, and a dry run writes nothing.
    #[tokio::test]
    async fn the_output_must_be_new_or_empty_and_outside_the_source() {
        let source = tempfile::tempdir().unwrap();
        let scratch = tempfile::tempdir().unwrap();
        source_dir(source.path()).await;

        let full = scratch.path().join("full");
        std::fs::create_dir_all(&full).unwrap();
        std::fs::write(full.join("keep.txt"), b"x").unwrap();
        let error = failure(&migration(source.path(), &full)).await;
        assert!(error.contains("not empty"), "{error}");
        assert_eq!(std::fs::read_dir(&full).unwrap().count(), 1);

        let inside = source.path().join("migrated");
        let error = failure(&migration(source.path(), &inside)).await;
        assert!(error.contains("inside the data directory"), "{error}");
        assert!(!inside.exists());

        let missing = scratch.path().join("no-such-source");
        let error = failure(&migration(&missing, &full)).await;
        assert!(error.contains("does not exist"), "{error}");

        let empty = scratch.path().join("empty");
        std::fs::create_dir_all(&empty).unwrap();
        let dry = Migration {
            dry_run: true,
            ..migration(source.path(), &empty)
        };
        let report = run(&dry).await.unwrap();
        assert!(report.written.is_none());
        assert_eq!(std::fs::read_dir(&empty).unwrap().count(), 0);
        run(&migration(source.path(), &empty)).await.unwrap();
        assert_eq!(tree(&empty).len(), 1);
    }
}
