//! On-disk WAL file format and the startup scanner shared by the async WAL
//! ([`super::AsyncWAL`]) and the recovery WAL ([`super::WriteAheadLog`]).
//!
//! A WAL file is a 12-byte header followed by frames:
//!
//! - **Header** — `magic "SYNAPWAL" (8 bytes) | format version: u32 BE`. This
//!   server writes and reads format version [`WAL_FORMAT_VERSION`]; a header
//!   declaring a newer version fails the scan with
//!   [`PersistenceError::WALUnsupportedVersion`].
//! - **Frame** — `len: u64 BE | crc32(payload): u32 BE | payload`, where the
//!   payload is the bincode (legacy config) encoding of a [`WALEntry`]. A
//!   committed transaction is one frame holding an
//!   [`Operation::Transaction`](super::types::Operation::Transaction), so it is
//!   covered by one checksum and survives a crash whole or not at all.
//!
//! Files written by Synap 1.3.x have no header: frames start at byte 0. They
//! are read as format version [`LEGACY_FORMAT_VERSION`]. Before the first
//! append, [`prepare`] upgrades such a file in place to the current format by
//! writing the header plus the unchanged frames to a temporary file and
//! renaming it over the original, so a crash during the upgrade leaves either
//! the untouched legacy file or the complete upgraded one. A legacy file can
//! never be mistaken for a versioned one: its first 8 bytes are a frame length,
//! and no frame is `0x53594E415057414C` bytes long.
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
use std::io::{self, BufReader, BufWriter, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use tracing::{info, warn};

/// Bytes that open every versioned WAL file.
pub const WAL_MAGIC: [u8; 8] = *b"SYNAPWAL";

/// Format version of the headerless WAL files written by Synap 1.3.x and
/// earlier.
pub const LEGACY_FORMAT_VERSION: u32 = 1;

/// Format version this server writes, and the newest it reads. Version 2 adds
/// the file header and the single-frame transaction record.
pub const WAL_FORMAT_VERSION: u32 = 2;

/// File header length: magic (8 bytes) + format version (u32).
pub const FILE_HEADER_LEN: u64 = 12;

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

/// The header of a versioned WAL file at the current format version.
pub fn encode_file_header() -> [u8; FILE_HEADER_LEN as usize] {
    let mut header = [0u8; FILE_HEADER_LEN as usize];
    header[..8].copy_from_slice(&WAL_MAGIC);
    header[8..].copy_from_slice(&WAL_FORMAT_VERSION.to_be_bytes());
    header
}

/// Container format of a WAL file, as found by a scan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WalFormat {
    /// No header and no frames: a missing or empty file, or a header torn by a
    /// crash while the file was being created.
    Empty,
    /// Headerless file written by Synap 1.3.x (format version 1); frames start
    /// at byte 0.
    Legacy,
    /// File whose header declares this format version; frames start after the
    /// header.
    Versioned(u32),
}

impl WalFormat {
    /// The file's format version, or `None` for an empty file.
    pub fn version(self) -> Option<u32> {
        match self {
            WalFormat::Empty => None,
            WalFormat::Legacy => Some(LEGACY_FORMAT_VERSION),
            WalFormat::Versioned(version) => Some(version),
        }
    }

    /// Byte position of the first frame.
    fn data_start(self) -> u64 {
        match self {
            WalFormat::Versioned(_) => FILE_HEADER_LEN,
            WalFormat::Empty | WalFormat::Legacy => 0,
        }
    }
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
    /// Container format of the file.
    pub format: WalFormat,
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
/// Returns [`PersistenceError::WALUnsupportedVersion`] when the header declares
/// a version newer than [`WAL_FORMAT_VERSION`],
/// [`PersistenceError::WALCorrupted`] when the header is damaged, and
/// [`PersistenceError::WALInteriorCorruption`] when an invalid frame is
/// followed by a valid one. A torn tail (including a torn header) is reported
/// in [`ScanReport::end`] and left on disk; use [`repair`] to remove it.
pub fn scan(path: &Path, mut visit: impl FnMut(WALEntry)) -> Result<ScanReport> {
    let mut file = match File::open(path) {
        Ok(file) => file,
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            return Ok(ScanReport {
                format: WalFormat::Empty,
                valid_frames: 0,
                max_offset: 0,
                end: ScanEnd::Clean,
            });
        }
        Err(e) => return Err(e.into()),
    };
    let file_len = file.metadata()?.len();
    let format = match read_header(&mut file, file_len, path)? {
        Header::Format(format) => format,
        Header::Torn { reason } => {
            return Ok(ScanReport {
                format: WalFormat::Empty,
                valid_frames: 0,
                max_offset: 0,
                end: ScanEnd::TornTail {
                    valid_len: 0,
                    file_len,
                    reason,
                },
            });
        }
    };
    let mut pos = format.data_start();
    file.seek(SeekFrom::Start(pos))?;
    let mut reader = BufReader::with_capacity(SCAN_BUFFER, file);
    let mut report = ScanReport {
        format,
        valid_frames: 0,
        max_offset: 0,
        end: ScanEnd::Clean,
    };

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

/// Make the WAL at `path` ready for appends at the current format version:
/// [`repair`] it, then write the header of a missing or empty file, or upgrade
/// a legacy (1.3.x, headerless) file in place.
///
/// The returned report describes the frames found by the scan; its
/// [`ScanReport::format`] is the format on disk afterwards. Fails without
/// modifying the file on interior corruption, a damaged header, or a format
/// version newer than this server reads.
pub fn prepare(path: &Path) -> Result<ScanReport> {
    let mut report = repair(path)?;
    match report.format {
        WalFormat::Versioned(_) => return Ok(report),
        WalFormat::Empty => write_new_file(path)?,
        WalFormat::Legacy => upgrade_legacy(path, &report)?,
    }
    report.format = WalFormat::Versioned(WAL_FORMAT_VERSION);
    Ok(report)
}

/// [`prepare`] on the blocking pool, for async callers.
pub async fn prepare_async(path: PathBuf) -> Result<ScanReport> {
    tokio::task::spawn_blocking(move || prepare(&path))
        .await
        .map_err(|e| PersistenceError::IOError(io::Error::other(e)))?
}

/// Create (or reset an empty) WAL file holding only the current header, and
/// make it durable before any frame can be appended.
fn write_new_file(path: &Path) -> Result<()> {
    let mut file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(path)?;
    file.write_all(&encode_file_header())?;
    file.sync_all()?;
    sync_parent_dir(path)?;
    Ok(())
}

/// Rewrite a repaired legacy WAL as header + the same frames, atomically.
///
/// The frames are copied byte for byte (their encoding did not change) to a
/// temporary file that is fsynced and then renamed over the original, so a
/// crash at any point leaves either the untouched legacy file or the complete
/// upgraded one. A leftover temporary file is overwritten by the next attempt.
fn upgrade_legacy(path: &Path, report: &ScanReport) -> Result<()> {
    let mut temp_name = path.file_name().unwrap_or_default().to_os_string();
    temp_name.push(".upgrade");
    let temp = path.with_file_name(temp_name);

    let mut source = File::open(path)?;
    let legacy_len = source.metadata()?.len();
    let target = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&temp)?;
    let mut writer = BufWriter::with_capacity(SCAN_BUFFER, target);
    writer.write_all(&encode_file_header())?;
    io::copy(&mut source, &mut writer)?;
    let target = writer.into_inner().map_err(|e| e.into_error())?;
    target.sync_all()?;
    drop(target);
    drop(source);

    std::fs::rename(&temp, path)?;
    sync_parent_dir(path)?;
    info!(
        "WAL {:?}: upgraded from format version {} (Synap 1.3.x, no header) to version {} ({} frames, {} bytes); Synap 1.3.x cannot read the upgraded file",
        path, LEGACY_FORMAT_VERSION, WAL_FORMAT_VERSION, report.valid_frames, legacy_len
    );
    Ok(())
}

/// Make a create or rename in the directory holding `path` durable.
/// Directories can only be fsynced on Unix; on Windows, NTFS journals the
/// metadata change itself.
pub(crate) fn sync_parent_dir(path: &Path) -> io::Result<()> {
    #[cfg(unix)]
    if let Some(parent) = path.parent() {
        let dir = if parent.as_os_str().is_empty() {
            Path::new(".")
        } else {
            parent
        };
        File::open(dir)?.sync_all()?;
    }
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
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

/// What the first bytes of a WAL file say about its format.
enum Header {
    Format(WalFormat),
    /// A prefix of a header and nothing else: the file was being created when
    /// the process crashed, so it holds no frames.
    Torn {
        reason: String,
    },
}

/// Classify the file from its first bytes (the cursor is left anywhere).
fn read_header(file: &mut File, file_len: u64, path: &Path) -> Result<Header> {
    if file_len == 0 {
        return Ok(Header::Format(WalFormat::Empty));
    }
    let take = file_len.min(FILE_HEADER_LEN) as usize;
    let mut bytes = [0u8; FILE_HEADER_LEN as usize];
    file.read_exact(&mut bytes[..take])?;
    let magic_len = take.min(WAL_MAGIC.len());
    if bytes[..magic_len] != WAL_MAGIC[..magic_len] {
        return Ok(Header::Format(WalFormat::Legacy));
    }
    if take < FILE_HEADER_LEN as usize {
        return Ok(Header::Torn {
            reason: format!("incomplete file header ({take} of {FILE_HEADER_LEN} bytes)"),
        });
    }
    let mut version = [0u8; 4];
    version.copy_from_slice(&bytes[8..12]);
    let version = u32::from_be_bytes(version);
    if version > WAL_FORMAT_VERSION {
        return Err(PersistenceError::WALUnsupportedVersion {
            path: path.to_path_buf(),
            found: version,
            supported: WAL_FORMAT_VERSION,
        });
    }
    if version <= LEGACY_FORMAT_VERSION {
        return Err(PersistenceError::WALCorrupted {
            offset: 8,
            reason: format!(
                "WAL {path:?} header declares format version {version}, but versioned headers start at version {}",
                LEGACY_FORMAT_VERSION + 1
            ),
        });
    }
    Ok(Header::Format(WalFormat::Versioned(version)))
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
            Operation::Transaction {
                operations: vec![Operation::KVDel { keys: vec![] }],
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

    /// `frames` behind the current file header.
    fn versioned(frames: &[u8]) -> Vec<u8> {
        let mut bytes = encode_file_header().to_vec();
        bytes.extend_from_slice(frames);
        bytes
    }

    fn header_with_version(version: u32) -> Vec<u8> {
        let mut bytes = WAL_MAGIC.to_vec();
        bytes.extend_from_slice(&version.to_be_bytes());
        bytes
    }

    #[test]
    fn versioned_file_scans_the_frames_after_its_header() {
        let dir = fresh_dir("versioned");
        let path = dir.join("wal.log");
        std::fs::write(&path, versioned(&frames(3).0)).unwrap();
        let (keys, report) = keys(&path);
        assert_eq!(keys, ["key1", "key2", "key3"]);
        assert_eq!(report.format, WalFormat::Versioned(WAL_FORMAT_VERSION));
        assert_eq!(report.end, ScanEnd::Clean);
    }

    /// A header from a newer format fails with both versions in the error and
    /// leaves the file untouched.
    #[test]
    fn newer_format_version_fails_naming_both_versions() {
        let dir = fresh_dir("newer_version");
        let path = dir.join("wal.log");
        let mut bytes = header_with_version(WAL_FORMAT_VERSION + 1);
        bytes.extend(frames(1).0);
        std::fs::write(&path, &bytes).unwrap();

        let err = prepare(&path).unwrap_err();
        match &err {
            PersistenceError::WALUnsupportedVersion {
                found, supported, ..
            } => {
                assert_eq!(*found, WAL_FORMAT_VERSION + 1);
                assert_eq!(*supported, WAL_FORMAT_VERSION);
            }
            other => panic!("expected an unsupported version, got {other:?}"),
        }
        let message = err.to_string();
        assert!(message.contains("format version 3"), "{message}");
        assert!(message.contains("versions up to 2"), "{message}");
        assert!(err.refuses_startup());
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
    }

    /// Only headerless files are version 1; a header claiming 0 or 1 is damage.
    #[test]
    fn header_below_version_two_is_corruption() {
        for version in [0, LEGACY_FORMAT_VERSION] {
            let dir = fresh_dir(&format!("header_v{version}"));
            let path = dir.join("wal.log");
            std::fs::write(&path, header_with_version(version)).unwrap();
            let err = prepare(&path).unwrap_err();
            assert!(
                matches!(err, PersistenceError::WALCorrupted { offset: 8, .. }),
                "{err:?}"
            );
            assert!(err.refuses_startup());
        }
    }

    #[test]
    fn prepare_creates_a_header_only_file() {
        let dir = fresh_dir("prepare_new");
        let path = dir.join("wal.log");
        let report = prepare(&path).unwrap();
        assert_eq!(report.format, WalFormat::Versioned(WAL_FORMAT_VERSION));
        assert_eq!(report.next_offset(), 1);
        assert_eq!(std::fs::read(&path).unwrap(), encode_file_header());
        // Idempotent.
        prepare(&path).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), encode_file_header());
    }

    /// A crash while a new file's header was being written leaves a prefix of
    /// it; the next start writes a complete header.
    #[test]
    fn torn_header_is_replaced_by_a_complete_one() {
        let header = encode_file_header();
        for len in 1..header.len() {
            let dir = fresh_dir(&format!("torn_header_{len}"));
            let path = dir.join("wal.log");
            std::fs::write(&path, &header[..len]).unwrap();
            let (_, before) = keys(&path);
            assert!(
                matches!(before.end, ScanEnd::TornTail { valid_len: 0, .. }),
                "{len}: {:?}",
                before.end
            );
            prepare(&path).unwrap();
            assert_eq!(std::fs::read(&path).unwrap(), header, "{len}");
        }
    }

    /// A 1.3.x file (no header) is repaired, then upgraded to header + the
    /// same frame bytes, with no temporary file left behind.
    #[test]
    fn prepare_upgrades_a_legacy_file_in_place() {
        let dir = fresh_dir("upgrade");
        let path = dir.join("wal.log");
        let (legacy, _) = frames(3);
        let mut with_torn_tail = legacy.clone();
        with_torn_tail.extend_from_slice(&encode_frame(&entry(4, "torn")).unwrap()[..7]);
        std::fs::write(&path, &with_torn_tail).unwrap();
        assert_eq!(keys(&path).1.format, WalFormat::Legacy);

        let report = prepare(&path).unwrap();
        assert_eq!(report.valid_frames, 3);
        assert_eq!(report.next_offset(), 4);
        assert_eq!(report.format, WalFormat::Versioned(WAL_FORMAT_VERSION));
        assert_eq!(std::fs::read(&path).unwrap(), versioned(&legacy));
        assert!(!dir.join("wal.log.upgrade").exists());

        let (keys, after) = keys(&path);
        assert_eq!(keys, ["key1", "key2", "key3"]);
        assert_eq!(after.end, ScanEnd::Clean);
        prepare(&path).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), versioned(&legacy));
    }

    /// A transaction record cut at any byte is discarded whole: the scan sees
    /// the frame before it and nothing of the transaction.
    #[test]
    fn transaction_frame_cut_at_any_byte_is_discarded_whole() {
        let dir = fresh_dir("txn_cut");
        let path = dir.join("wal.log");
        let txn = WALEntry {
            offset: 2,
            timestamp: 1_700_000_000,
            operation: Operation::Transaction {
                operations: vec![
                    entry(0, "t1").operation,
                    entry(0, "t2").operation,
                    entry(0, "t3").operation,
                ],
            },
        };
        let mut full = versioned(&encode_frame(&entry(1, "before")).unwrap());
        let boundary = full.len();
        full.extend(encode_frame(&txn).unwrap());

        for cut in boundary..=full.len() {
            std::fs::write(&path, &full[..cut]).unwrap();
            let mut seen = Vec::new();
            let report = repair(&path).unwrap();
            scan(&path, |e| seen.push(e.operation)).unwrap();
            if cut == full.len() {
                assert_eq!(report.end, ScanEnd::Clean);
                assert_eq!(seen.len(), 2);
                assert!(
                    matches!(&seen[1], Operation::Transaction { operations } if operations.len() == 3)
                );
            } else {
                assert_eq!(seen.len(), 1, "cut at {cut}");
                assert!(matches!(&seen[0], Operation::KVSet { key, .. } if key == "before"));
                assert_eq!(std::fs::metadata(&path).unwrap().len(), boundary as u64);
            }
        }
    }

    fn err_message_has_position(path: &Path, position: u64) -> bool {
        repair(path)
            .unwrap_err()
            .to_string()
            .contains(&format!("at byte {position}"))
    }
}
