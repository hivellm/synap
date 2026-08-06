//! Guards against re-introducing the span-entry bug that killed the server.
//!
//! `tracing::Span::enter()` returns a *thread-local* guard. Held across an
//! `.await`, the span stays entered on the worker thread when the task yields,
//! so the next task scheduled there runs inside a span that belongs to someone
//! else. Observed live on 1.3.1: RESP3 connection spans for four different
//! peers nested inside one another, and then
//!
//! ```text
//! thread 'tokio-rt-worker' panicked at tracing-subscriber/src/registry/sharded.rs:317:
//! assertion `left != right` failed: tried to clone a span (Id(..)) that already closed
//! ```
//!
//! Each panic takes a worker thread with it. Enough of them and the runtime has
//! nothing left to poll: every listener still accepts TCP, nothing ever
//! answers, and the periodic snapshot task stops too — which is exactly how the
//! failure presented before the cause was known.
//!
//! The fix is `tracing::Instrument`: `future.instrument(span).await` enters the
//! span per poll and exits it on every yield. This test keeps the async
//! protocol paths honest about that.

use std::path::{Path, PathBuf};

/// Source files that run inside the tokio runtime and must never hold an
/// `enter()` guard.
const ASYNC_PATHS: &[&str] = &[
    "src/protocol/resp3",
    "src/protocol/synap_rpc",
    "src/server",
    "src/persistence",
    "src/replication",
];

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

#[test]
fn async_protocol_paths_instrument_spans_instead_of_entering_them() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));

    let mut files = Vec::new();
    for rel in ASYNC_PATHS {
        rust_files(&root.join(rel), &mut files);
    }
    assert!(
        !files.is_empty(),
        "no sources found to check — did the layout change?"
    );

    let mut offenders = Vec::new();
    for file in &files {
        let source = std::fs::read_to_string(file).expect("read source");
        for (idx, line) in source.lines().enumerate() {
            let code = line.split("//").next().unwrap_or(line);
            if code.contains(".enter()") {
                offenders.push(format!(
                    "{}:{}: {}",
                    file.strip_prefix(root).unwrap_or(file).display(),
                    idx + 1,
                    line.trim()
                ));
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "span.enter() in async code re-introduces the worker-killing panic \
         (see this file's header). Use `future.instrument(span).await` instead.\n  {}",
        offenders.join("\n  ")
    );
}
