//! On-disk WAL frame format and the startup scanner shared by the async WAL
//! ([`super::AsyncWAL`]) and the recovery WAL ([`super::WriteAheadLog`]).
//!
//! A frame is `len: u64 BE | crc32(payload): u32 BE | payload`, where the
//! payload is the bincode (legacy config) encoding of a [`WALEntry`].
//!
//! The scanner verifies every frame's length, checksum and payload. The first
//! invalid frame is classified before anything is appended:
//!
//! - **Torn tail** — no complete, valid frame follows it. This is what a crash
//!   in the middle of an append leaves behind. [`repair`] truncates the file to
//!   the end of the last valid frame, so new frames never land after garbage.
//! - **Interior corruption** — a valid frame follows it. Truncating would
//!   discard acknowledged history, so the scan fails with
//!   [`PersistenceError::WALInteriorCorruption`] carrying the byte position.

use super::types::{PersistenceError, Result, WALEntry};
use std::fs::{File, OpenOptions};
use std::io::{self, BufReader, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use tracing::warn;

/// Bytes before the payload: length (u64) + checksum (u32).
pub const FRAME_HEADER_LEN: u64 = 12;

/// Smallest payload any [`WALEntry`] encodes to: offset (u64) + timestamp
/// (u64) + operation variant tag (u32).
const MIN_PAYLOAD_LEN: u64 = 20;

/// Byte range of the operation variant tag inside the payload. bincode's
/// legacy config encodes struct fields in order with fixed-width little-endian
/// integers and enum variants as a `u32`, so the tag follows offset and
/// timestamp.
const TAG_RANGE: std::ops::Range<usize> = 16..20;

/// Exclusive upper bound on the variant tag the corruption search accepts as a
/// frame candidate. `Operation` has far fewer variants, so every frame the
/// writer produces passes; random bytes pass with probability ~2^-22, which
/// keeps the byte-by-byte search from checksumming garbage.
const MAX_OPERATION_TAG: u32 = 1024;

/// Bytes the corruption search reads at a time.
const SEARCH_WINDOW: usize = 1 << 20;

/// Read buffer for the sequential frame pass.
const SCAN_BUFFER: usize = 64 * 1024;

/// Encode `entry` as one complete frame (header + payload).
pub fn encode_frame(entry: &WALEntry) -> Result<Vec<u8>> {
    let mut frame = vec![0u8; FRAME_HEADER_LEN as usize];
    bincode::serde::encode_into_std_write(entry, &mut frame, bincode::config::legacy())?;
    let payload_len = (frame.len() - FRAME_HEADER_LEN as usize) as u64;
    let checksum = crc32fast::hash(&frame[FRAME_HEADER_LEN as usize..]);
    frame[..8].copy_from_slice(&payload_len.to_be_bytes());
    frame[8..12].copy_from_slice(&checksum.to_be_bytes());
    Ok(frame)
}

/// How a scan ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScanEnd {
    /// Every byte of the file belongs to a valid frame.
    Clean,
    /// The bytes from `valid_len` to `file_len` hold an incomplete or invalid
    /// frame with no valid frame after it.
    TornTail {
        valid_len: u64,
        file_len: u64,
        reason: String,
    },
}

/// Result of scanning a WAL file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanReport {
    /// Number of valid frames before [`ScanReport::end`].
    pub valid_frames: u64,
    /// Highest entry offset among the valid frames (0 when there are none).
    pub max_offset: u64,
    /// Where and why the scan stopped.
    pub end: ScanEnd,
}

impl ScanReport {
    /// Offset the next appended entry receives.
    pub fn next_offset(&self) -> u64 {
        self.max_offset + 1
    }
}

/// Scan the WAL at `path`, calling `visit` with every valid entry in file
/// order. A missing file scans as empty and clean.
///
/// Returns [`PersistenceError::WALInteriorCorruption`] when an invalid frame is
/// followed by a valid one. A torn tail is reported in [`ScanReport::end`] and
/// left on disk; use [`repair`] to remove it.
pub fn scan(path: &Path, mut visit: impl FnMut(WALEntry)) -> Result<ScanReport> {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            return Ok(ScanReport {
                valid_frames: 0,
                max_offset: 0,
                end: ScanEnd::Clean,
            });
        }
        Err(e) => return Err(e.into()),
    };
    let file_len = file.metadata()?.len();
    let mut reader = BufReader::with_capacity(SCAN_BUFFER, file);
    let mut report = ScanReport {
        valid_frames: 0,
        max_offset: 0,
        end: ScanEnd::Clean,
    };
    let mut pos = 0u64;

    while pos < file_len {
        match read_frame(&mut reader, file_len - pos)? {
            Frame::Valid { entry, frame_len } => {
                report.max_offset = report.max_offset.max(entry.offset);
                report.valid_frames += 1;
                visit(entry);
                pos += frame_len;
            }
            Frame::Invalid { reason, frame_len } => {
                let mut file = reader.into_inner();
                if let Some(next_valid) = find_valid_frame(&mut file, pos, frame_len, file_len)? {
                    return Err(PersistenceError::WALInteriorCorruption {
                        path: path.to_path_buf(),
                        position: pos,
                        next_valid,
                        reason,
                    });
                }
                report.end = ScanEnd::TornTail {
                    valid_len: pos,
                    file_len,
                    reason,
                };
                return Ok(report);
            }
        }
    }
    Ok(report)
}

/// Scan the WAL at `path` and truncate a torn tail to the end of the last valid
/// frame, so the next append starts on a frame boundary. Fails on interior
/// corruption without modifying the file.
pub fn repair(path: &Path) -> Result<ScanReport> {
    let report = scan(path, |_| {})?;
    if let ScanEnd::TornTail {
        valid_len,
        file_len,
        reason,
    } = &report.end
    {
        let file = OpenOptions::new().write(true).open(path)?;
        file.set_len(*valid_len)?;
        file.sync_all()?;
        warn!(
            "WAL {:?}: torn tail of {} bytes at byte {} ({}); truncated to the last valid frame",
            path,
            file_len - valid_len,
            valid_len,
            reason
        );
    }
    Ok(report)
}

/// [`repair`] on the blocking pool, for async callers.
pub async fn repair_async(path: PathBuf) -> Result<ScanReport> {
    tokio::task::spawn_blocking(move || repair(&path))
        .await
        .map_err(|e| PersistenceError::IOError(io::Error::other(e)))?
}

/// Read every valid entry whose offset is at least `from_offset`, on the
/// blocking pool. A torn tail ends the read with a warning (the prefix is
/// returned); interior corruption is an error.
pub async fn read_entries(path: PathBuf, from_offset: u64) -> Result<Vec<WALEntry>> {
    tokio::task::spawn_blocking(move || {
        let mut entries = Vec::new();
        let report = scan(&path, |entry| {
            if entry.offset >= from_offset {
                entries.push(entry);
            }
        })?;
        if let ScanEnd::TornTail {
            valid_len, reason, ..
        } = &report.end
        {
            warn!(
                "WAL {:?}: replay stops at an unrepaired torn tail at byte {} ({})",
                path, valid_len, reason
            );
        }
        Ok(entries)
    })
    .await
    .map_err(|e| PersistenceError::IOError(io::Error::other(e)))?
}

/// One frame read by the sequential pass.
enum Frame {
    Valid {
        entry: WALEntry,
        frame_len: u64,
    },
    /// `frame_len` is the frame's full length when its header was intact and
    /// the declared payload fits in the file, so the next frame boundary is
    /// known; `None` when the header itself is unusable.
    Invalid {
        reason: String,
        frame_len: Option<u64>,
    },
}

/// Read the frame at the reader's position. `remaining` is the number of file
/// bytes from that position to EOF; a declared length beyond it is rejected
/// before anything is allocated.
fn read_frame(reader: &mut impl Read, remaining: u64) -> io::Result<Frame> {
    if remaining < FRAME_HEADER_LEN {
        return Ok(Frame::Invalid {
            reason: format!("incomplete frame header ({remaining} of {FRAME_HEADER_LEN} bytes)"),
            frame_len: None,
        });
    }
    let mut header = [0u8; FRAME_HEADER_LEN as usize];
    reader.read_exact(&mut header)?;
    let (len, checksum) = parse_header(&header);
    let available = remaining - FRAME_HEADER_LEN;
    let Some(payload_len) = usize::try_from(len).ok().filter(|_| len <= available) else {
        return Ok(Frame::Invalid {
            reason: format!("frame declares {len} payload bytes but only {available} remain"),
            frame_len: None,
        });
    };
    let mut payload = vec![0u8; payload_len];
    reader.read_exact(&mut payload)?;
    let frame_len = Some(FRAME_HEADER_LEN + len);
    match decode_payload(&payload, checksum) {
        Ok(entry) => Ok(Frame::Valid {
            entry,
            frame_len: FRAME_HEADER_LEN + len,
        }),
        Err(reason) => Ok(Frame::Invalid { reason, frame_len }),
    }
}

/// Split a frame header into (payload length, checksum).
fn parse_header(header: &[u8]) -> (u64, u32) {
    let mut len = [0u8; 8];
    len.copy_from_slice(&header[..8]);
    let mut checksum = [0u8; 4];
    checksum.copy_from_slice(&header[8..12]);
    (u64::from_be_bytes(len), u32::from_be_bytes(checksum))
}

/// Verify `payload` against `checksum` and decode it.
fn decode_payload(payload: &[u8], checksum: u32) -> std::result::Result<WALEntry, String> {
    let actual = crc32fast::hash(payload);
    if actual != checksum {
        return Err(format!(
            "checksum mismatch (stored {checksum:#010x}, computed {actual:#010x})"
        ));
    }
    bincode::serde::decode_from_slice::<WALEntry, _>(payload, bincode::config::legacy())
        .map(|(entry, _)| entry)
        .map_err(|e| format!("undecodable payload: {e}"))
}

/// Find the position of a complete, valid frame after the invalid frame at
/// `pos`, or `None` when the rest of the file holds none.
///
/// When the invalid frame's length was usable, its natural successor is
/// checked first. A corrupt length hides the boundary, so the rest of the file
/// is then searched byte by byte.
fn find_valid_frame(
    file: &mut File,
    pos: u64,
    frame_len: Option<u64>,
    file_len: u64,
) -> io::Result<Option<u64>> {
    if let Some(len) = frame_len {
        let next = pos + len;
        if next < file_len && frame_is_valid_at(file, next, file_len)? {
            return Ok(Some(next));
        }
    }

    let min_frame = FRAME_HEADER_LEN + MIN_PAYLOAD_LEN;
    let mut window: Vec<u8> = Vec::new();
    let mut window_start = pos + 1;
    let mut candidate = pos + 1;
    while file_len.saturating_sub(candidate) >= min_frame {
        if candidate + min_frame > window_start + window.len() as u64 {
            window_start = candidate;
            let take = (file_len - candidate).min(SEARCH_WINDOW as u64) as usize;
            window.resize(take, 0);
            file.seek(SeekFrom::Start(candidate))?;
            file.read_exact(&mut window)?;
        }
        let at = (candidate - window_start) as usize;
        if let Some((len, checksum)) = candidate_header(&window[at..], file_len - candidate) {
            let payload_start = at + FRAME_HEADER_LEN as usize;
            let in_window = payload_start as u64 + len <= window.len() as u64;
            let valid = if in_window {
                let payload = &window[payload_start..payload_start + len as usize];
                decode_payload(payload, checksum).is_ok()
            } else {
                frame_is_valid_at(file, candidate, file_len)?
            };
            if valid {
                return Ok(Some(candidate));
            }
        }
        candidate += 1;
    }
    Ok(None)
}

/// Cheap plausibility test for a frame starting at `bytes[0]`: the declared
/// length fits in the file and the operation tag is in range. Every frame the
/// writer produces passes. Returns (payload length, checksum).
fn candidate_header(bytes: &[u8], remaining: u64) -> Option<(u64, u32)> {
    let (len, checksum) = parse_header(&bytes[..FRAME_HEADER_LEN as usize]);
    if len < MIN_PAYLOAD_LEN || len > remaining - FRAME_HEADER_LEN {
        return None;
    }
    let tag_bytes = &bytes[FRAME_HEADER_LEN as usize..][TAG_RANGE];
    let mut tag = [0u8; 4];
    tag.copy_from_slice(tag_bytes);
    (u32::from_le_bytes(tag) < MAX_OPERATION_TAG).then_some((len, checksum))
}

/// Whether a complete, valid frame starts at byte `at`.
fn frame_is_valid_at(file: &mut File, at: u64, file_len: u64) -> io::Result<bool> {
    file.seek(SeekFrom::Start(at))?;
    let mut reader = BufReader::with_capacity(SCAN_BUFFER, &mut *file);
    Ok(matches!(
        read_frame(&mut reader, file_len - at)?,
        Frame::Valid { .. }
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::persistence::types::Operation;

    fn entry(offset: u64, key: &str) -> WALEntry {
        WALEntry {
            offset,
            timestamp: 1_700_000_000,
            operation: Operation::KVSet {
                key: key.to_string(),
                value: key.as_bytes().to_vec(),
                ttl: None,
            },
        }
    }

    fn fresh_dir(name: &str) -> PathBuf {
        let dir = PathBuf::from(format!("./target/wal_frame_{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Frames for offsets 1..=n, concatenated, plus each frame's start byte.
    fn frames(n: u64) -> (Vec<u8>, Vec<u64>) {
        let mut bytes = Vec::new();
        let mut starts = Vec::new();
        for offset in 1..=n {
            starts.push(bytes.len() as u64);
            bytes.extend(encode_frame(&entry(offset, &format!("key{offset}"))).unwrap());
        }
        (bytes, starts)
    }

    fn keys(path: &Path) -> (Vec<String>, ScanReport) {
        let mut keys = Vec::new();
        let report = scan(path, |e| {
            if let Operation::KVSet { key, .. } = e.operation {
                keys.push(key);
            }
        })
        .unwrap();
        (keys, report)
    }

    /// Every frame the writer produces passes the search prefilter, for small
    /// and large operation variants alike.
    #[test]
    fn writer_frames_pass_the_search_prefilter() {
        let ops = [
            Operation::KVSet {
                key: "k".into(),
                value: vec![],
                ttl: None,
            },
            Operation::ZDiffStore {
                destination: "d".into(),
                keys: vec![],
            },
        ];
        for operation in ops {
            let frame = encode_frame(&WALEntry {
                offset: 0,
                timestamp: 0,
                operation,
            })
            .unwrap();
            assert!(candidate_header(&frame, frame.len() as u64).is_some());
        }
    }

    #[test]
    fn clean_file_scans_every_frame() {
        let dir = fresh_dir("clean");
        let path = dir.join("wal.log");
        std::fs::write(&path, frames(3).0).unwrap();
        let (keys, report) = keys(&path);
        assert_eq!(keys, ["key1", "key2", "key3"]);
        assert_eq!(report.end, ScanEnd::Clean);
        assert_eq!(report.next_offset(), 4);
    }

    #[test]
    fn missing_file_scans_empty() {
        let dir = fresh_dir("missing");
        let report = repair(&dir.join("absent.wal")).unwrap();
        assert_eq!(report.valid_frames, 0);
        assert_eq!(report.next_offset(), 1);
        assert!(!dir.join("absent.wal").exists());
    }

    /// Every torn-tail shape is truncated to the end of the last valid frame.
    #[test]
    fn torn_tail_shapes_are_truncated_to_the_last_valid_frame() {
        let (valid, _) = frames(2);
        let torn = encode_frame(&entry(3, "torn")).unwrap();
        let mut huge_len = (1u64 << 60).to_be_bytes().to_vec();
        huge_len.extend_from_slice(&[0xAB; 40]);
        let mut bad_checksum = torn.clone();
        bad_checksum[8] ^= 0xFF;
        let shapes: Vec<(&str, Vec<u8>)> = vec![
            ("partial_length", torn[..5].to_vec()),
            ("partial_checksum", torn[..10].to_vec()),
            ("partial_payload", torn[..torn.len() - 3].to_vec()),
            ("corrupt_huge_length", huge_len),
            ("bad_checksum", bad_checksum),
            ("zero_fill", vec![0u8; 4096]),
        ];
        for (name, tail) in shapes {
            let dir = fresh_dir(&format!("torn_{name}"));
            let path = dir.join("wal.log");
            let mut bytes = valid.clone();
            bytes.extend_from_slice(&tail);
            std::fs::write(&path, &bytes).unwrap();

            let report = repair(&path).unwrap();
            assert!(
                matches!(report.end, ScanEnd::TornTail { valid_len, .. } if valid_len == valid.len() as u64),
                "{name}: {:?}",
                report.end
            );
            assert_eq!(std::fs::metadata(&path).unwrap().len(), valid.len() as u64);
            let (keys, after) = keys(&path);
            assert_eq!(keys, ["key1", "key2"], "{name}");
            assert_eq!(after.end, ScanEnd::Clean, "{name}");
        }
    }

    /// A checksum mismatch followed by a valid frame fails with the byte
    /// position of the bad frame and leaves the file untouched.
    #[test]
    fn interior_checksum_mismatch_fails_with_position() {
        let dir = fresh_dir("interior_checksum");
        let path = dir.join("wal.log");
        let (mut bytes, starts) = frames(3);
        bytes[starts[1] as usize + FRAME_HEADER_LEN as usize + 2] ^= 0xFF;
        std::fs::write(&path, &bytes).unwrap();

        let err = repair(&path).unwrap_err();
        match err {
            PersistenceError::WALInteriorCorruption {
                position,
                next_valid,
                ..
            } => {
                assert_eq!(position, starts[1]);
                assert_eq!(next_valid, starts[2]);
            }
            other => panic!("expected interior corruption, got {other:?}"),
        }
        assert!(err_message_has_position(&path, starts[1]));
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
    }

    /// A corrupt length hides the next boundary; the byte search still finds
    /// the valid frame after it.
    #[test]
    fn interior_corrupt_length_is_found_by_search() {
        let dir = fresh_dir("interior_length");
        let path = dir.join("wal.log");
        let (mut bytes, starts) = frames(3);
        let at = starts[1] as usize;
        bytes[at..at + 8].copy_from_slice(&(1u64 << 60).to_be_bytes());
        std::fs::write(&path, &bytes).unwrap();

        match repair(&path).unwrap_err() {
            PersistenceError::WALInteriorCorruption {
                position,
                next_valid,
                ..
            } => {
                assert_eq!(position, starts[1]);
                assert_eq!(next_valid, starts[2]);
            }
            other => panic!("expected interior corruption, got {other:?}"),
        }
    }

    fn err_message_has_position(path: &Path, position: u64) -> bool {
        repair(path)
            .unwrap_err()
            .to_string()
            .contains(&format!("at byte {position}"))
    }
}
