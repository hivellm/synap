//! Snapshot reading and writing utilities
//!
//! Implements reading and writing of Synap snapshot format with user-scoped namespace support.

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader, BufWriter};
use tracing::{debug, info};
use uuid::Uuid;

const SNAPSHOT_MAGIC: &[u8] = b"SYNAP002";
const SNAPSHOT_VERSION: u8 = 2;

/// Snapshot metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotMetadata {
    pub version: u8,
    pub timestamp: u64,
    pub wal_offset: u64,
    pub kv_count: u64,
    pub queue_count: u64,
    pub stream_count: u64,
}

/// Snapshot data structure
#[derive(Debug, Clone)]
pub struct SnapshotData {
    pub metadata: SnapshotMetadata,
    pub kv_data: HashMap<String, Vec<u8>>,
    pub queue_data: HashMap<String, Vec<QueueMessage>>,
    pub stream_data: HashMap<String, Vec<StreamEntry>>,
}

/// Queue message structure (simplified)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueMessage {
    pub id: String,
    pub data: Vec<u8>,
    pub timestamp: u64,
}

/// Stream entry structure (simplified)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamEntry {
    pub id: String,
    pub fields: HashMap<String, Vec<u8>>,
    pub timestamp: u64,
}

/// Find the latest snapshot file in a directory
pub async fn find_latest_snapshot(data_dir: &Path) -> Result<Option<PathBuf>> {
    let snapshot_dir = data_dir.join("snapshots");

    if !snapshot_dir.exists() {
        return Ok(None);
    }

    let mut entries = tokio::fs::read_dir(&snapshot_dir)
        .await
        .context("Failed to read snapshot directory")?;

    let mut latest: Option<(PathBuf, u64)> = None;

    while let Some(entry) = entries
        .next_entry()
        .await
        .context("Failed to read directory entry")?
    {
        let path = entry.path();

        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            if name.starts_with("snapshot-v") && name.ends_with(".bin") {
                // Extract timestamp from filename: snapshot-v2-{timestamp}.bin
                if let Some(ts_str) = name
                    .strip_prefix("snapshot-v2-")
                    .and_then(|s| s.strip_suffix(".bin"))
                {
                    if let Ok(timestamp) = ts_str.parse::<u64>() {
                        if latest.is_none() || latest.as_ref().unwrap().1 < timestamp {
                            latest = Some((path.clone(), timestamp));
                        }
                    }
                }
            }
        }
    }

    Ok(latest.map(|(path, _)| path))
}

/// Read snapshot from file
pub async fn read_snapshot(path: &Path) -> Result<SnapshotData> {
    info!("Reading snapshot from {:?}", path);

    let file = File::open(path)
        .await
        .context("Failed to open snapshot file")?;
    let mut reader = BufReader::new(file);

    // Read and validate magic bytes
    let mut magic = [0u8; 8];
    reader
        .read_exact(&mut magic)
        .await
        .context("Failed to read magic bytes")?;

    if magic != SNAPSHOT_MAGIC {
        bail!("Invalid snapshot file: magic bytes mismatch");
    }

    // Read version
    let version = reader.read_u8().await.context("Failed to read version")?;
    if version != SNAPSHOT_VERSION {
        bail!(
            "Unsupported snapshot version: {} (expected {})",
            version,
            SNAPSHOT_VERSION
        );
    }

    // Read metadata
    let timestamp = reader
        .read_u64_le()
        .await
        .context("Failed to read timestamp")?;
    let wal_offset = reader
        .read_u64_le()
        .await
        .context("Failed to read WAL offset")?;

    // Read KV data
    let kv_count = reader
        .read_u64_le()
        .await
        .context("Failed to read KV count")?;

    debug!("Reading {} KV entries", kv_count);
    let mut kv_data = HashMap::new();

    for _ in 0..kv_count {
        let key_len = reader
            .read_u32_le()
            .await
            .context("Failed to read key length")?;

        let mut key_bytes = vec![0u8; key_len as usize];
        reader
            .read_exact(&mut key_bytes)
            .await
            .context("Failed to read key")?;

        let key = String::from_utf8(key_bytes).context("Invalid UTF-8 in key")?;

        let value_len = reader
            .read_u32_le()
            .await
            .context("Failed to read value length")?;

        let mut value = vec![0u8; value_len as usize];
        reader
            .read_exact(&mut value)
            .await
            .context("Failed to read value")?;

        kv_data.insert(key, value);
    }

    // Read queue data
    let queue_count = reader
        .read_u64_le()
        .await
        .context("Failed to read queue count")?;

    debug!("Reading {} queues", queue_count);
    let mut queue_data = HashMap::new();

    for _ in 0..queue_count {
        let queue_name_len = reader
            .read_u32_le()
            .await
            .context("Failed to read queue name length")?;

        let mut queue_name_bytes = vec![0u8; queue_name_len as usize];
        reader
            .read_exact(&mut queue_name_bytes)
            .await
            .context("Failed to read queue name")?;

        let queue_name =
            String::from_utf8(queue_name_bytes).context("Invalid UTF-8 in queue name")?;

        let message_count = reader
            .read_u64_le()
            .await
            .context("Failed to read message count")?;

        let mut messages = Vec::new();
        for _ in 0..message_count {
            let msg_data_len = reader
                .read_u32_le()
                .await
                .context("Failed to read message data length")?;

            let mut msg_data = vec![0u8; msg_data_len as usize];
            reader
                .read_exact(&mut msg_data)
                .await
                .context("Failed to read message data")?;

            let timestamp = reader
                .read_u64_le()
                .await
                .context("Failed to read message timestamp")?;

            messages.push(QueueMessage {
                id: format!("msg_{}", messages.len()),
                data: msg_data,
                timestamp,
            });
        }

        queue_data.insert(queue_name, messages);
    }

    // Read stream data
    let stream_count = reader
        .read_u64_le()
        .await
        .context("Failed to read stream count")?;

    debug!("Reading {} streams", stream_count);
    let mut stream_data = HashMap::new();

    for _ in 0..stream_count {
        let stream_name_len = reader
            .read_u32_le()
            .await
            .context("Failed to read stream name length")?;

        let mut stream_name_bytes = vec![0u8; stream_name_len as usize];
        reader
            .read_exact(&mut stream_name_bytes)
            .await
            .context("Failed to read stream name")?;

        let stream_name =
            String::from_utf8(stream_name_bytes).context("Invalid UTF-8 in stream name")?;

        let entry_count = reader
            .read_u64_le()
            .await
            .context("Failed to read entry count")?;

        let mut entries = Vec::new();
        for _ in 0..entry_count {
            let field_count = reader
                .read_u32_le()
                .await
                .context("Failed to read field count")?;

            let mut fields = HashMap::new();
            for _ in 0..field_count {
                let field_name_len = reader
                    .read_u32_le()
                    .await
                    .context("Failed to read field name length")?;

                let mut field_name_bytes = vec![0u8; field_name_len as usize];
                reader
                    .read_exact(&mut field_name_bytes)
                    .await
                    .context("Failed to read field name")?;

                let field_name =
                    String::from_utf8(field_name_bytes).context("Invalid UTF-8 in field name")?;

                let field_value_len = reader
                    .read_u32_le()
                    .await
                    .context("Failed to read field value length")?;

                let mut field_value = vec![0u8; field_value_len as usize];
                reader
                    .read_exact(&mut field_value)
                    .await
                    .context("Failed to read field value")?;

                fields.insert(field_name, field_value);
            }

            let timestamp = reader
                .read_u64_le()
                .await
                .context("Failed to read entry timestamp")?;

            entries.push(StreamEntry {
                id: format!("entry_{}", entries.len()),
                fields,
                timestamp,
            });
        }

        stream_data.insert(stream_name, entries);
    }

    let metadata = SnapshotMetadata {
        version,
        timestamp,
        wal_offset,
        kv_count,
        queue_count,
        stream_count,
    };

    Ok(SnapshotData {
        metadata,
        kv_data,
        queue_data,
        stream_data,
    })
}

/// Write snapshot to file
pub async fn write_snapshot(path: &Path, data: &SnapshotData) -> Result<()> {
    info!("Writing snapshot to {:?}", path);

    // Create parent directory
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .context("Failed to create snapshot directory")?;
    }

    let file = File::create(path)
        .await
        .context("Failed to create snapshot file")?;
    let mut writer = BufWriter::new(file);

    // Write header
    writer.write_all(SNAPSHOT_MAGIC).await?;
    writer.write_u8(data.metadata.version).await?;
    writer.write_u64_le(data.metadata.timestamp).await?;
    writer.write_u64_le(data.metadata.wal_offset).await?;

    // Write KV data
    writer.write_u64_le(data.kv_data.len() as u64).await?;

    for (key, value) in &data.kv_data {
        let key_bytes = key.as_bytes();
        writer.write_u32_le(key_bytes.len() as u32).await?;
        writer.write_all(key_bytes).await?;
        writer.write_u32_le(value.len() as u32).await?;
        writer.write_all(value).await?;
    }

    // Write queue data
    writer.write_u64_le(data.queue_data.len() as u64).await?;

    for (queue_name, messages) in &data.queue_data {
        let queue_name_bytes = queue_name.as_bytes();
        writer.write_u32_le(queue_name_bytes.len() as u32).await?;
        writer.write_all(queue_name_bytes).await?;
        writer.write_u64_le(messages.len() as u64).await?;

        for msg in messages {
            writer.write_u32_le(msg.data.len() as u32).await?;
            writer.write_all(&msg.data).await?;
            writer.write_u64_le(msg.timestamp).await?;
        }
    }

    // Write stream data
    writer.write_u64_le(data.stream_data.len() as u64).await?;

    for (stream_name, entries) in &data.stream_data {
        let stream_name_bytes = stream_name.as_bytes();
        writer.write_u32_le(stream_name_bytes.len() as u32).await?;
        writer.write_all(stream_name_bytes).await?;
        writer.write_u64_le(entries.len() as u64).await?;

        for entry in entries {
            writer.write_u32_le(entry.fields.len() as u32).await?;

            for (field_name, field_value) in &entry.fields {
                let field_name_bytes = field_name.as_bytes();
                writer.write_u32_le(field_name_bytes.len() as u32).await?;
                writer.write_all(field_name_bytes).await?;
                writer.write_u32_le(field_value.len() as u32).await?;
                writer.write_all(field_value).await?;
            }

            writer.write_u64_le(entry.timestamp).await?;
        }
    }

    writer.flush().await?;

    info!("Snapshot written successfully");
    Ok(())
}

/// Apply user namespace prefix to resource names
pub fn scope_resource_name(user_id: &Uuid, resource_name: &str) -> String {
    format!("user_{}:{}", user_id.as_simple(), resource_name)
}

/// Check if a resource name is already scoped
pub fn is_scoped(resource_name: &str) -> bool {
    resource_name.starts_with("user_") && resource_name.contains(':')
}
