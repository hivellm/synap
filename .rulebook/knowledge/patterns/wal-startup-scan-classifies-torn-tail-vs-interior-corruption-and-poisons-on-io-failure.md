# WAL startup scan classifies torn tail vs interior corruption; I/O failure poisons the writer

**Category**: persistence
**Tags**: persistence, wal, fsync, durability, phase20

## Description

An append-only log needs a safe append point before its first write. Scan every
frame (length must fit in the remaining file bytes, CRC32 must match, payload
must decode) and classify the first invalid frame: if no complete valid frame
follows it, it is a torn tail from a crash, so truncate the file to the last
valid frame before opening for append; if a valid frame follows, it is interior
corruption, so fail startup with the byte position instead of silently
discarding acknowledged history. A corrupt length hides the next boundary, so
search the rest of the file byte by byte, with a cheap sound prefilter
(plausible length + operation tag range) before checksumming a candidate.

On the write side, a failed write, flush or fsync must fail every
acknowledgement in its group and poison the writer (write-once `OnceLock`
reason): after a failed fsync the kernel may have dropped dirty pages, so the
file contents are unknown and only a restart plus rescan is safe. Expose the
poison through health (503) so supervisors restart the process.

Put the file behind a small sink trait (`write_all`/`flush`/`sync`, RPITIT
`impl Future + Send`) so tests can wrap the production sink and inject
failures without touching the filesystem.

## Example

```rust
match read_frame(&mut reader, file_len - pos)? {
    Frame::Valid { entry, frame_len } => { visit(entry); pos += frame_len; }
    Frame::Invalid { reason, frame_len } => {
        if let Some(next) = find_valid_frame(&mut file, pos, frame_len, file_len)? {
            return Err(WALInteriorCorruption { position: pos, next_valid: next, .. });
        }
        return Ok(TornTail { valid_len: pos, .. }); // repair() truncates here
    }
}
```

## When to Use

Any append-only file (WAL, AOF, stream segment) that is reopened for append
after a crash, and any group-commit writer that acknowledges callers.

## When NOT to Use

Files rewritten atomically via temp-file + rename (snapshots), where a torn
write never becomes the live file.

## Anti-pattern this replaces

`break` on the first bad frame and then open in append mode: new frames land
after the garbage and the next restart cannot reach them. Also: logging a
`warn!` on a failed group fsync while sending the already-`Ok` per-entry
results.
