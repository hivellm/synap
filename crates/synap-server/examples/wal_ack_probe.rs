//! Where a write's WAL acknowledgement time goes.
//!
//! The instrument behind the regression analysis in
//! `docs/benchmarks/BASELINE_1_4_0.md`. It drives the server's own
//! [`AsyncWAL`] in a scratch directory, with a timing wrapper around its
//! [`FileSink`] (the [`AsyncWAL::open_with_sink`] seam), and prints latency
//! percentiles for:
//!
//! - A: one append awaited at a time (a client with one write in flight), with
//!   the share spent in the sink's flush (the group's write handed to the OS)
//!   and, when the mode fsyncs, in the fsync;
//! - B: a batch of 16 appends on one connection, acknowledgements awaited one
//!   by one (B1, what RESP3 does with a pipelined batch) or all queued first
//!   and awaited together (B2, what SynapRPC gets from a task per request);
//! - C: four tasks appending one at a time each (the baseline's
//!   non-pipelined cells), with the frames written per group;
//! - D: the bare write syscall of one frame on the calling thread;
//! - E: an empty `spawn_blocking` round trip, the hop `tokio::fs` pays.
//!
//! Run: `cargo run --release --locked -p synap-server --example wal_ack_probe`.
//! The WAL goes to the system temp directory, so the fsync numbers are that
//! drive's.

use std::io::{self, Write};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use synap_server::persistence::types::{FsyncMode, Operation, WALConfig};
use synap_server::persistence::{AsyncWAL, FileSink, WalSink};

/// What the sink saw since the last [`take`].
#[derive(Default)]
struct Timings {
    flush_ns: Vec<u64>,
    sync_ns: Vec<u64>,
    frames: u64,
    groups: u64,
}

/// The production sink with every flush and fsync timed. A flush that
/// carried frames closes one group commit.
struct TimingSink {
    inner: FileSink,
    timings: Arc<Mutex<Timings>>,
    frames: u64,
}

impl WalSink for TimingSink {
    async fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        self.frames += 1;
        self.inner.write_all(buf).await
    }

    async fn flush(&mut self) -> io::Result<()> {
        let start = Instant::now();
        let result = self.inner.flush().await;
        let ns = elapsed_ns(start);
        let mut timings = self.timings.lock().expect("timings lock");
        if self.frames > 0 {
            timings.flush_ns.push(ns);
            timings.frames += self.frames;
            timings.groups += 1;
        }
        self.frames = 0;
        result
    }

    async fn sync(&mut self) -> io::Result<()> {
        let start = Instant::now();
        let result = self.inner.sync().await;
        let ns = elapsed_ns(start);
        self.timings.lock().expect("timings lock").sync_ns.push(ns);
        result
    }
}

fn elapsed_ns(start: Instant) -> u64 {
    u64::try_from(start.elapsed().as_nanos()).unwrap_or(u64::MAX)
}

/// Nearest-rank percentile of an ascending slice, in microseconds.
fn percentile_us(sorted: &[u64], quantile: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let rank = ((quantile * sorted.len() as f64).ceil() as usize).clamp(1, sorted.len());
    sorted[rank - 1] as f64 / 1_000.0
}

fn report(label: &str, mut samples: Vec<u64>) {
    samples.sort_unstable();
    let mean = samples.iter().sum::<u64>() as f64 / samples.len().max(1) as f64 / 1_000.0;
    println!(
        "{label:<62} n={:<6} p50={:>9.1}us p90={:>9.1}us p99={:>9.1}us mean={:>9.1}us",
        samples.len(),
        percentile_us(&samples, 0.50),
        percentile_us(&samples, 0.90),
        percentile_us(&samples, 0.99),
        mean
    );
}

fn frames_per_group(timings: &Timings) -> f64 {
    timings.frames as f64 / timings.groups.max(1) as f64
}

/// A SET record shaped like the baseline's: `key:<n>` and a 64-byte value.
fn set_op(i: u64) -> Operation {
    Operation::KVSet {
        key: format!("key:{}", i % 100_000),
        value: vec![b'x'; 64],
        ttl: None,
    }
}

fn take(timings: &Arc<Mutex<Timings>>) -> Timings {
    std::mem::take(&mut *timings.lock().expect("timings lock"))
}

async fn scenario(mode: FsyncMode, iterations: u64) {
    println!("\n== fsync mode {mode:?} ==");
    let dir = tempfile::tempdir().expect("scratch directory");
    let timings = Arc::new(Mutex::new(Timings::default()));
    let config = WALConfig {
        enabled: true,
        path: dir.path().join("wal").join("synap.wal"),
        buffer_size_kb: 64,
        fsync_mode: mode,
        fsync_interval_ms: 1000,
        max_size_mb: 1024,
    };
    let sink_timings = Arc::clone(&timings);
    let wal = AsyncWAL::open_with_sink(config, move |inner| TimingSink {
        inner,
        timings: sink_timings,
        frames: 0,
    })
    .await
    .expect("open the WAL");
    for i in 0..200 {
        wal.append(set_op(i)).await.expect("warmup append");
    }
    take(&timings);

    // A. One append in flight at a time.
    let mut latencies = Vec::new();
    for i in 0..iterations {
        let start = Instant::now();
        wal.append(set_op(i)).await.expect("append");
        latencies.push(elapsed_ns(start));
    }
    let a = take(&timings);
    report(
        "A. one append awaited at a time: acknowledgement",
        latencies,
    );
    report(
        "   of which the group's flush (write handed to the OS)",
        a.flush_ns,
    );
    if !a.sync_ns.is_empty() {
        report("   of which the group's fsync", a.sync_ns);
    }

    // B. A batch of 16 writes on one connection.
    let batches = (iterations / 16).max(10);
    let mut one_by_one = Vec::new();
    for b in 0..batches {
        let start = Instant::now();
        for j in 0..16 {
            wal.append(set_op(b * 16 + j)).await.expect("append");
        }
        one_by_one.push(elapsed_ns(start));
    }
    let b1 = take(&timings);
    let mut together = Vec::new();
    for b in 0..batches {
        let start = Instant::now();
        let acks: Vec<_> = (0..16)
            .map(|j| wal.enqueue(set_op(b * 16 + j)).expect("enqueue"))
            .collect();
        for ack in acks {
            ack.wait().await.expect("acknowledgement");
        }
        together.push(elapsed_ns(start));
    }
    let b2 = take(&timings);
    report(
        "B1. 16 writes, acknowledgements awaited one by one: batch",
        one_by_one,
    );
    println!("    frames per group: {:.2}", frames_per_group(&b1));
    report(
        "B2. 16 writes queued, then awaited together: batch",
        together,
    );
    println!("    frames per group: {:.2}", frames_per_group(&b2));

    // C. Four connections with one write in flight each.
    let start = Instant::now();
    let mut tasks = Vec::new();
    for client in 0..4u64 {
        let wal = wal.clone();
        tasks.push(tokio::spawn(async move {
            for i in 0..iterations {
                wal.append(set_op(client * 1_000_000 + i))
                    .await
                    .expect("append");
            }
        }));
    }
    for task in tasks {
        task.await.expect("append task");
    }
    let seconds = start.elapsed().as_secs_f64();
    let c = take(&timings);
    println!(
        "C. 4 tasks x {iterations} appends, one in flight each: {:.0} appends/s, {:.2} frames per group",
        (4 * iterations) as f64 / seconds,
        frames_per_group(&c)
    );
    wal.shutdown().await.expect("shutdown");
}

/// D. The bare write syscall the flush stands for: one ~110-byte frame
/// appended on the calling thread, without tokio's blocking pool.
fn bare_write(iterations: u64) {
    let dir = tempfile::tempdir().expect("scratch directory");
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.path().join("bare.wal"))
        .expect("open");
    let frame = vec![b'x'; 110];
    let mut latencies = Vec::new();
    for _ in 0..iterations {
        let start = Instant::now();
        file.write_all(&frame).expect("write");
        latencies.push(elapsed_ns(start));
    }
    println!();
    report(
        "D. std::fs write of one frame on the calling thread",
        latencies,
    );
}

/// E. One empty `spawn_blocking` round trip: the thread hand-off `tokio::fs`
/// makes for every write and flush.
async fn blocking_hop(iterations: u64) {
    let mut latencies = Vec::new();
    for _ in 0..iterations {
        let start = Instant::now();
        tokio::task::spawn_blocking(|| ())
            .await
            .expect("blocking task");
        latencies.push(elapsed_ns(start));
    }
    report("E. empty spawn_blocking round trip", latencies);
}

#[tokio::main]
async fn main() {
    scenario(FsyncMode::Never, 20_000).await;
    scenario(FsyncMode::Periodic, 20_000).await;
    scenario(FsyncMode::Always, 400).await;
    bare_write(20_000);
    blocking_hop(20_000).await;
}
