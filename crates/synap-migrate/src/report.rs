//! What a migration read and wrote, for the user.

use crate::source::Source;
use std::collections::HashMap;
use std::fmt;
use std::path::PathBuf;
use synap_server::QueueConfig;
use synap_server::persistence::SnapshotImage;
use synap_server::persistence::types::StreamEvent;

/// The configuration queues get when the input does not record theirs
/// (snapshots older than v5, queues created implicitly by a replayed
/// publish), and where it came from.
#[derive(Debug, Clone)]
pub struct QueueDefaults {
    pub config: QueueConfig,
    pub origin: String,
}

impl Default for QueueDefaults {
    fn default() -> Self {
        Self {
            config: QueueConfig::default(),
            origin: "the server's built-in queue defaults".to_string(),
        }
    }
}

/// How many resources of each structure a data directory holds.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Counts {
    pub kv_keys: usize,
    /// KV keys with an expiry.
    pub kv_expiring: usize,
    pub hashes: usize,
    pub lists: usize,
    pub sets: usize,
    /// Sorted sets, geospatial indexes included.
    pub sorted_sets: usize,
    pub hyperloglogs: usize,
    pub bitmaps: usize,
    pub queues: usize,
    /// Messages waiting or delivered but not acknowledged.
    pub queue_messages: usize,
    pub dead_letters: usize,
    pub streams: usize,
    pub stream_events: usize,
}

impl Counts {
    /// Count the resources of `image` and `streams`.
    pub fn of(image: &SnapshotImage, streams: &HashMap<String, Vec<StreamEvent>>) -> Self {
        Self {
            kv_keys: image.kv.len(),
            kv_expiring: image.kv.iter().filter(|(_, _, e)| e.is_some()).count(),
            hashes: image.hashes.len(),
            lists: image.lists.len(),
            sets: image.sets.len(),
            sorted_sets: image.sorted_sets.len(),
            hyperloglogs: image.hyperloglogs.len(),
            bitmaps: image.bitmaps.len(),
            queues: image.queues.len(),
            queue_messages: image
                .queues
                .values()
                .map(|q| q.ready.len() + q.in_flight.len())
                .sum(),
            dead_letters: image.queues.values().map(|q| q.dead_letter.len()).sum(),
            streams: streams.len(),
            stream_events: streams.values().map(Vec::len).sum(),
        }
    }
}

impl fmt::Display for Counts {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "  KV keys:       {} ({} with expiry)",
            self.kv_keys, self.kv_expiring
        )?;
        writeln!(f, "  hashes:        {}", self.hashes)?;
        writeln!(f, "  lists:         {}", self.lists)?;
        writeln!(f, "  sets:          {}", self.sets)?;
        writeln!(
            f,
            "  sorted sets:   {} (geospatial included)",
            self.sorted_sets
        )?;
        writeln!(f, "  HyperLogLogs:  {}", self.hyperloglogs)?;
        writeln!(f, "  bitmaps:       {}", self.bitmaps)?;
        writeln!(
            f,
            "  queues:        {} ({} messages, {} dead letters)",
            self.queues, self.queue_messages, self.dead_letters
        )?;
        write!(
            f,
            "  streams:       {} ({} events)",
            self.streams, self.stream_events
        )
    }
}

/// The report of one migration run.
#[derive(Debug, Clone)]
pub struct Report {
    /// The snapshot the state started from: path and format version.
    pub snapshot: Option<(PathBuf, u32)>,
    /// The WAL, its format, and its records replayed after the snapshot.
    pub wal: String,
    pub counts: Counts,
    /// What the input could not carry, and other facts the user must know.
    pub notes: Vec<String>,
    /// The migrated snapshot; `None` for a dry run.
    pub written: Option<PathBuf>,
}

impl Report {
    /// The report for `source` (read with `queue_defaults`) holding `counts`.
    pub fn new(source: &Source, counts: Counts, queue_defaults: &QueueDefaults) -> Self {
        let mut notes = Vec::new();
        if let Some(read) = &source.snapshot {
            if read.version < 4 && counts.kv_keys > 0 {
                notes.push(format!(
                    "snapshot {} is format v{}, which stores no key expiry: the keys it holds load without expiry, as the server loads them (keys written to the WAL after it keep their deadline)",
                    read.path.display(),
                    read.version
                ));
            }
            if read.version < 5 && counts.queues > 0 {
                notes.push(format!(
                    "snapshot {} is format v{}, which stores no queue configuration and no in-flight or dead-lettered messages: its queues get {}",
                    read.path.display(),
                    read.version,
                    queue_defaults.origin
                ));
            }
        }
        let wal = &source.wal;
        if let Some(torn) = &wal.torn_tail {
            notes.push(format!(
                "WAL {}: {}; the server discards it at startup, and so did this migration",
                wal.path.display(),
                torn
            ));
        }
        if wal.stream_records > 0 {
            notes.push(format!(
                "WAL {}: {} stream publish records were not replayed; the server never replays them (streams come from the snapshot)",
                wal.path.display(),
                wal.stream_records
            ));
        }
        Self {
            snapshot: source
                .snapshot
                .as_ref()
                .map(|read| (read.path.clone(), read.version)),
            wal: format!(
                "{} ({}): {} records, {} replayed after the snapshot",
                wal.path.display(),
                match wal.format.version() {
                    Some(version) => format!("format v{version}"),
                    None => "empty or absent".to_string(),
                },
                wal.records,
                wal.replayed
            ),
            counts,
            notes,
            written: None,
        }
    }
}

impl fmt::Display for Report {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.snapshot {
            Some((path, version)) => {
                writeln!(f, "Snapshot: {} (format v{})", path.display(), version)?
            }
            None => writeln!(f, "Snapshot: none (the state comes from the WAL alone)")?,
        }
        writeln!(f, "WAL: {}", self.wal)?;
        writeln!(f, "Read:")?;
        writeln!(f, "{}", self.counts)?;
        for note in &self.notes {
            writeln!(f, "Note: {note}")?;
        }
        match &self.written {
            Some(path) => write!(f, "Wrote: {}", path.display()),
            None => write!(f, "Dry run: nothing was written"),
        }
    }
}
