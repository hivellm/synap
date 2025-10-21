use super::types::{PersistenceError, Result};
use crate::core::stream::StreamManager;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::fs::{File, OpenOptions};
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufWriter};
use tokio::sync::{Mutex, mpsc, oneshot};
use tracing::{debug, info, warn};

/// Stream event for persistence (Kafka-style)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamEvent {
    pub room: String,
    pub event_type: String,
    pub payload: Vec<u8>,
    pub offset: u64,
    pub timestamp: u64,
}

/// Stream Persistence Layer (Kafka-style append-only log)
///
/// Features:
/// - Append-only log per room (like Kafka partitions)
/// - Offset-based consumption
/// - Durable storage (survives crashes)
/// - Compaction support (future)
#[derive(Clone)]
pub struct StreamPersistence {
    base_dir: PathBuf,
    writer_tx: mpsc::UnboundedSender<StreamWriteCommand>,
    current_offset: Arc<AtomicU64>,
    stats: Arc<Mutex<StreamStats>>,
}

#[derive(Debug, Default)]
struct StreamStats {
    total_events: u64,
    total_bytes: u64,
    rooms_active: u64,
}

enum StreamWriteCommand {
    Append {
        event: StreamEvent,
        response_tx: oneshot::Sender<Result<u64>>,
    },
    Flush {
        room: String,
        response_tx: oneshot::Sender<Result<()>>,
    },
}

impl StreamPersistence {
    /// Create new stream persistence layer
    pub async fn new(base_dir: PathBuf) -> Result<Self> {
        tokio::fs::create_dir_all(&base_dir).await?;

        let current_offset = Arc::new(AtomicU64::new(0));
        let stats = Arc::new(Mutex::new(StreamStats::default()));
        let (writer_tx, writer_rx) = mpsc::unbounded_channel();

        // Spawn background writer task
        let base_dir_clone = base_dir.clone();
        let offset_clone = Arc::clone(&current_offset);
        let stats_clone = Arc::clone(&stats);

        tokio::spawn(Self::writer_loop(
            base_dir_clone,
            writer_rx,
            offset_clone,
            stats_clone,
        ));

        info!("Stream persistence initialized at {:?}", base_dir);

        Ok(Self {
            base_dir,
            writer_tx,
            current_offset,
            stats,
        })
    }

    /// Background writer loop (Kafka-style batching)
    async fn writer_loop(
        base_dir: PathBuf,
        mut rx: mpsc::UnboundedReceiver<StreamWriteCommand>,
        current_offset: Arc<AtomicU64>,
        stats: Arc<Mutex<StreamStats>>,
    ) {
        // Keep writers open per room (Kafka keeps partition files open)
        let mut room_writers: HashMap<String, BufWriter<File>> = HashMap::new();

        while let Some(cmd) = rx.recv().await {
            match cmd {
                StreamWriteCommand::Append { event, response_tx } => {
                    // Get or create writer for this room
                    let writer = match room_writers.get_mut(&event.room) {
                        Some(w) => w,
                        None => {
                            let room_path = base_dir.join(format!("{}.log", event.room));
                            match OpenOptions::new()
                                .create(true)
                                .append(true)
                                .open(&room_path)
                                .await
                            {
                                Ok(file) => {
                                    room_writers.insert(
                                        event.room.clone(),
                                        BufWriter::with_capacity(64 * 1024, file),
                                    );
                                    room_writers.get_mut(&event.room).unwrap()
                                }
                                Err(e) => {
                                    let _ = response_tx.send(Err(e.into()));
                                    continue;
                                }
                            }
                        }
                    };

                    // Write event
                    let result = Self::write_event(writer, &event, &current_offset, &stats).await;
                    let _ = response_tx.send(result);
                }
                StreamWriteCommand::Flush { room, response_tx } => {
                    if let Some(writer) = room_writers.get_mut(&room) {
                        let result = writer.flush().await.map_err(|e| e.into());
                        let _ = response_tx.send(result);
                    } else {
                        let _ = response_tx.send(Ok(()));
                    }
                }
            }
        }

        // Flush all on shutdown
        for (room, mut writer) in room_writers {
            if let Err(e) = writer.flush().await {
                warn!("Failed to flush room {} on shutdown: {}", room, e);
            }
        }

        info!("Stream writer loop terminated");
    }

    /// Write a single event (Kafka-style)
    async fn write_event(
        writer: &mut BufWriter<File>,
        event: &StreamEvent,
        current_offset: &Arc<AtomicU64>,
        stats: &Arc<Mutex<StreamStats>>,
    ) -> Result<u64> {
        let offset = current_offset.fetch_add(1, Ordering::SeqCst);

        // Create versioned event with offset
        let event_with_offset = StreamEvent {
            room: event.room.clone(),
            event_type: event.event_type.clone(),
            payload: event.payload.clone(),
            offset,
            timestamp: event.timestamp,
        };

        // Serialize
        let data = bincode::serialize(&event_with_offset)?;
        let size = data.len() as u64;
        let checksum = crc32fast::hash(&data);

        // Write: [size (8)][checksum (4)][data]
        writer.write_u64(size).await?;
        writer.write_u32(checksum).await?;
        writer.write_all(&data).await?;

        // Update stats
        let mut stats_guard = stats.lock().await;
        stats_guard.total_events += 1;
        stats_guard.total_bytes += size + 12;

        Ok(offset)
    }

    /// Append event to stream
    pub async fn append_event(
        &self,
        room: String,
        event_type: String,
        payload: Vec<u8>,
    ) -> Result<u64> {
        let event = StreamEvent {
            room: room.clone(),
            event_type,
            payload,
            offset: 0, // Will be assigned in write_event
            timestamp: Self::current_timestamp(),
        };

        let (response_tx, response_rx) = oneshot::channel();
        self.writer_tx
            .send(StreamWriteCommand::Append { event, response_tx })
            .map_err(|_| {
                PersistenceError::IOError(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "Writer closed",
                ))
            })?;

        response_rx.await.map_err(|_| {
            PersistenceError::IOError(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Response lost",
            ))
        })?
    }

    /// Flush events for a specific room
    pub async fn flush_room(&self, room: String) -> Result<()> {
        let (response_tx, response_rx) = oneshot::channel();
        self.writer_tx
            .send(StreamWriteCommand::Flush { room, response_tx })
            .map_err(|_| {
                PersistenceError::IOError(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "Writer closed",
                ))
            })?;

        response_rx.await.map_err(|_| {
            PersistenceError::IOError(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Response lost",
            ))
        })?
    }

    /// Read events from a room's log (Kafka-style sequential read)
    pub async fn read_events(
        &self,
        room: &str,
        from_offset: u64,
        limit: usize,
    ) -> Result<Vec<StreamEvent>> {
        let room_path = self.base_dir.join(format!("{}.log", room));

        if !tokio::fs::metadata(&room_path).await.is_ok() {
            return Ok(vec![]);
        }

        let mut file = File::open(&room_path).await?;
        let mut events = Vec::new();
        let mut current_offset = 0u64;

        loop {
            if events.len() >= limit {
                break;
            }

            let size = match file.read_u64().await {
                Ok(s) => s,
                Err(_) => break,
            };

            let expected_checksum = match file.read_u32().await {
                Ok(c) => c,
                Err(_) => break,
            };

            let mut data = vec![0u8; size as usize];
            if file.read_exact(&mut data).await.is_err() {
                break;
            }

            let actual_checksum = crc32fast::hash(&data);
            if actual_checksum != expected_checksum {
                warn!("Checksum mismatch in stream log");
                break;
            }

            match bincode::deserialize::<StreamEvent>(&data) {
                Ok(event) => {
                    let event_offset = event.offset;
                    // Only include events >= from_offset
                    if event_offset >= from_offset {
                        events.push(event);
                    }
                    current_offset = event_offset;
                }
                Err(e) => {
                    warn!("Failed to deserialize stream event: {}", e);
                    break;
                }
            }
        }

        debug!(
            "Read {} events from room {} starting at offset {}",
            events.len(),
            room,
            from_offset
        );

        Ok(events)
    }

    /// Recover stream state from logs
    pub async fn recover_room(&self, stream_manager: &StreamManager, room: &str) -> Result<u64> {
        info!("Recovering stream room: {}", room);

        let events = self.read_events(room, 0, usize::MAX).await?;
        let count = events.len();

        if count == 0 {
            return Ok(0);
        }

        // Recreate room
        stream_manager.create_room(room).await.map_err(|e| {
            PersistenceError::RecoveryFailed(format!("Failed to create room: {}", e))
        })?;

        // Replay events
        for event in events {
            stream_manager
                .publish(room, &event.event_type, event.payload)
                .await
                .map_err(|e| {
                    PersistenceError::RecoveryFailed(format!("Failed to publish: {}", e))
                })?;
        }

        info!("Recovered {} events for room {}", count, room);
        Ok(count as u64)
    }

    fn current_timestamp() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }
}

use std::collections::HashMap;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::StreamConfig;

    #[tokio::test]
    async fn test_stream_persistence_append() {
        let temp_dir = std::env::temp_dir().join("synap_stream_test");
        let _ = tokio::fs::remove_dir_all(&temp_dir).await;
        tokio::fs::create_dir_all(&temp_dir).await.unwrap();

        let stream_persist = StreamPersistence::new(temp_dir.clone()).await.unwrap();

        // Append events
        for i in 0..10 {
            stream_persist
                .append_event(
                    "test_room".to_string(),
                    "test_event".to_string(),
                    format!("payload_{}", i).into_bytes(),
                )
                .await
                .unwrap();
        }

        // Flush
        stream_persist
            .flush_room("test_room".to_string())
            .await
            .unwrap();

        // Read back
        let events = stream_persist
            .read_events("test_room", 0, 100)
            .await
            .unwrap();

        assert_eq!(events.len(), 10);
        assert_eq!(events[0].offset, 0);
        assert_eq!(events[9].offset, 9);

        // Cleanup
        let _ = tokio::fs::remove_dir_all(&temp_dir).await;
    }

    #[tokio::test]
    async fn test_stream_offset_based_read() {
        let temp_dir = std::env::temp_dir().join("synap_stream_offset_test");
        let _ = tokio::fs::remove_dir_all(&temp_dir).await;

        let stream_persist = StreamPersistence::new(temp_dir.clone()).await.unwrap();

        // Append 20 events
        for i in 0..20 {
            stream_persist
                .append_event(
                    "offset_room".to_string(),
                    "event".to_string(),
                    vec![i as u8],
                )
                .await
                .unwrap();
        }

        stream_persist
            .flush_room("offset_room".to_string())
            .await
            .unwrap();

        // Read from offset 10
        let events = stream_persist
            .read_events("offset_room", 10, 5)
            .await
            .unwrap();

        assert_eq!(events.len(), 5);
        assert_eq!(events[0].offset, 10);
        assert_eq!(events[4].offset, 14);

        // Cleanup
        let _ = tokio::fs::remove_dir_all(&temp_dir).await;
    }
}
