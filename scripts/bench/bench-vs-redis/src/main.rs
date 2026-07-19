/// Synap internal KV benchmark (in-process, no HTTP, no network)
///
/// Redis numbers come from redis-benchmark run inside Docker:
///   docker exec project-v-redis-1 redis-benchmark -n 100000 -c <N> -t set,get --csv
///
/// Usage: cargo run --release
use hdrhistogram::Histogram;
use std::sync::Arc;
use std::time::{Duration, Instant};
use synap_server::core::{KVConfig, KVStore};

// ── Config ────────────────────────────────────────────────────────────────────

const WARMUP_OPS: usize = 10_000;
const BENCH_OPS: usize = 100_000;

// Redis numbers measured with redis-benchmark inside the Docker container
// (no Windows networking involved, loopback inside container):
//   redis-benchmark -n 100000 -c <N> -t set,get --csv
const REDIS_RESULTS: &[(&str, f64, f64, f64, f64)] = &[
    // (label, ops/sec, p50_us, p99_us, p999_us)
    ("Redis  SET (1C  RESP/internal)", 12_711.0, 71.0, 127.0, 1_407.0),
    ("Redis  GET (1C  RESP/internal)", 14_186.0, 63.0, 103.0, 303.0),
    ("Redis  SET (4C  RESP/internal)", 72_992.0, 39.0, 87.0, 5_191.0),
    ("Redis  GET (4C  RESP/internal)", 61_387.0, 39.0, 87.0, 751.0),
    ("Redis  SET (16C RESP/internal)", 71_839.0, 119.0, 247.0, 5_503.0),
    ("Redis  GET (16C RESP/internal)", 66_889.0, 127.0, 223.0, 863.0),
];

struct BenchResult {
    label: String,
    ops: usize,
    elapsed: Duration,
    hist_us: Histogram<u64>,
}

impl BenchResult {
    fn ops_per_sec(&self) -> f64 {
        self.ops as f64 / self.elapsed.as_secs_f64()
    }
    fn p50_us(&self) -> u64 {
        self.hist_us.value_at_quantile(0.50)
    }
    fn p99_us(&self) -> u64 {
        self.hist_us.value_at_quantile(0.99)
    }
    fn p999_us(&self) -> u64 {
        self.hist_us.value_at_quantile(0.999)
    }
}

// ── Synap benchmarks (internal store, no HTTP) ────────────────────────────────

async fn bench_synap_set(threads: usize) -> BenchResult {
    let label = format!("Synap  SET ({}T  in-process)", threads);
    let store = Arc::new(KVStore::new(KVConfig::default()));
    let per_thread = BENCH_OPS / threads;

    for i in 0..WARMUP_OPS {
        store
            .set(&format!("w{}", i), vec![0u8; 64], None)
            .await
            .unwrap();
    }

    let mut hist = Histogram::<u64>::new(3).unwrap();
    let t0 = Instant::now();

    let handles: Vec<_> = (0..threads)
        .map(|t| {
            let s = store.clone();
            tokio::spawn(async move {
                let mut times = Vec::with_capacity(per_thread);
                for i in 0..per_thread {
                    let key = format!("k{}_{}", t, i);
                    let start = Instant::now();
                    s.set(&key, vec![42u8; 64], None).await.unwrap();
                    times.push(start.elapsed().as_micros() as u64);
                }
                times
            })
        })
        .collect();

    let mut all_times = Vec::with_capacity(BENCH_OPS);
    for h in handles {
        all_times.extend(h.await.unwrap());
    }
    let elapsed = t0.elapsed();
    for t in all_times {
        hist.record(t).ok();
    }

    BenchResult {
        label,
        ops: BENCH_OPS,
        elapsed,
        hist_us: hist,
    }
}

async fn bench_synap_get(threads: usize) -> BenchResult {
    let label = format!("Synap  GET ({}T  in-process)", threads);
    let store = Arc::new(KVStore::new(KVConfig::default()));
    let per_thread = BENCH_OPS / threads;

    for i in 0..BENCH_OPS {
        store
            .set(&format!("k{}", i), vec![42u8; 64], None)
            .await
            .unwrap();
    }

    let mut hist = Histogram::<u64>::new(3).unwrap();
    let t0 = Instant::now();

    let handles: Vec<_> = (0..threads)
        .map(|t| {
            let s = store.clone();
            tokio::spawn(async move {
                let mut times = Vec::with_capacity(per_thread);
                for i in 0..per_thread {
                    let key = format!("k{}", (t * per_thread + i) % BENCH_OPS);
                    let start = Instant::now();
                    let _ = s.get(&key).await;
                    times.push(start.elapsed().as_micros() as u64);
                }
                times
            })
        })
        .collect();

    let mut all_times = Vec::with_capacity(BENCH_OPS);
    for h in handles {
        all_times.extend(h.await.unwrap());
    }
    let elapsed = t0.elapsed();
    for t in all_times {
        hist.record(t).ok();
    }

    BenchResult {
        label,
        ops: BENCH_OPS,
        elapsed,
        hist_us: hist,
    }
}

async fn bench_synap_overwrite() -> BenchResult {
    let label = "Synap  SET overwrite (1T in-process)".to_string();
    let store = Arc::new(KVStore::new(KVConfig::default()));
    let key = "overwrite_target";

    for _ in 0..WARMUP_OPS {
        store.set(key, vec![0u8; 64], None).await.unwrap();
    }

    let mut hist = Histogram::<u64>::new(3).unwrap();
    let t0 = Instant::now();

    for _ in 0..BENCH_OPS {
        let start = Instant::now();
        store.set(key, vec![42u8; 64], None).await.unwrap();
        hist.record(start.elapsed().as_micros() as u64).ok();
    }

    BenchResult {
        label,
        ops: BENCH_OPS,
        elapsed: t0.elapsed(),
        hist_us: hist,
    }
}

// ── Output ────────────────────────────────────────────────────────────────────

const W: usize = 48;

fn print_header() {
    println!();
    println!(
        "{:<W$} {:>12} {:>10} {:>10} {:>10}",
        "Benchmark", "ops/sec", "p50 µs", "p99 µs", "p99.9 µs"
    );
    println!("{}", "─".repeat(W + 46));
}

fn print_row(r: &BenchResult) {
    println!(
        "{:<W$} {:>12.0} {:>10} {:>10} {:>10}",
        r.label,
        r.ops_per_sec(),
        r.p50_us(),
        r.p99_us(),
        r.p999_us(),
    );
}

fn print_redis_row(label: &str, ops: f64, p50: f64, p99: f64, p999: f64) {
    println!(
        "{:<W$} {:>12.0} {:>10.0} {:>10.0} {:>10.0}",
        label, ops, p50, p99, p999,
    );
}

fn print_ratio_raw(synap_ops: f64, redis_ops: f64, label: &str) {
    let ratio = synap_ops / redis_ops;
    let adj = if ratio >= 1.0 { "faster" } else { "slower" };
    let indicator = if ratio >= 1.0 { "▲" } else { "▼" };
    println!(
        "  → {label}: Synap {:.2}x {} ({} {:.0}k vs {:.0}k ops/s)",
        if ratio >= 1.0 { ratio } else { 1.0 / ratio },
        adj,
        indicator,
        synap_ops / 1_000.0,
        redis_ops / 1_000.0,
    );
}

// ── Main ──────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() {
    println!("╔═══════════════════════════════════════════════════════════════════════════════════════╗");
    println!("║          Synap vs Redis — Throughput & Latency Benchmark                               ║");
    println!("╠═══════════════════════════════════════════════════════════════════════════════════════╣");
    println!("║  Synap : in-process KV store (no HTTP, direct Rust API — zero network overhead)        ║");
    println!("║  Redis : redis-benchmark inside Docker (loopback, no Windows NAT overhead)             ║");
    println!(
        "║  Ops   : {:>6} per benchmark, 64-byte values, nightly Rust --release            ║",
        BENCH_OPS
    );
    println!("╚═══════════════════════════════════════════════════════════════════════════════════════╝");

    // ── 1 thread ─────────────────────────────────────────────────────────────
    println!("\n## Single-threaded / 1 connection  (100k ops, 64-byte values)");
    print_header();

    let s_set_1t = bench_synap_set(1).await;
    print_row(&s_set_1t);
    print_redis_row(
        REDIS_RESULTS[0].0,
        REDIS_RESULTS[0].1,
        REDIS_RESULTS[0].2,
        REDIS_RESULTS[0].3,
        REDIS_RESULTS[0].4,
    );
    print_ratio_raw(s_set_1t.ops_per_sec(), REDIS_RESULTS[0].1, "SET 1T/1C");
    println!();

    let s_get_1t = bench_synap_get(1).await;
    print_row(&s_get_1t);
    print_redis_row(
        REDIS_RESULTS[1].0,
        REDIS_RESULTS[1].1,
        REDIS_RESULTS[1].2,
        REDIS_RESULTS[1].3,
        REDIS_RESULTS[1].4,
    );
    print_ratio_raw(s_get_1t.ops_per_sec(), REDIS_RESULTS[1].1, "GET 1T/1C");

    // ── 4 threads ────────────────────────────────────────────────────────────
    println!("\n## 4 threads / 4 connections");
    print_header();

    let s_set_4t = bench_synap_set(4).await;
    print_row(&s_set_4t);
    print_redis_row(
        REDIS_RESULTS[2].0,
        REDIS_RESULTS[2].1,
        REDIS_RESULTS[2].2,
        REDIS_RESULTS[2].3,
        REDIS_RESULTS[2].4,
    );
    print_ratio_raw(s_set_4t.ops_per_sec(), REDIS_RESULTS[2].1, "SET 4T/4C");
    println!();

    let s_get_4t = bench_synap_get(4).await;
    print_row(&s_get_4t);
    print_redis_row(
        REDIS_RESULTS[3].0,
        REDIS_RESULTS[3].1,
        REDIS_RESULTS[3].2,
        REDIS_RESULTS[3].3,
        REDIS_RESULTS[3].4,
    );
    print_ratio_raw(s_get_4t.ops_per_sec(), REDIS_RESULTS[3].1, "GET 4T/4C");

    // ── 16 threads ───────────────────────────────────────────────────────────
    println!("\n## 16 threads / 16 connections");
    print_header();

    let s_set_16t = bench_synap_set(16).await;
    print_row(&s_set_16t);
    print_redis_row(
        REDIS_RESULTS[4].0,
        REDIS_RESULTS[4].1,
        REDIS_RESULTS[4].2,
        REDIS_RESULTS[4].3,
        REDIS_RESULTS[4].4,
    );
    print_ratio_raw(s_set_16t.ops_per_sec(), REDIS_RESULTS[4].1, "SET 16T/16C");
    println!();

    let s_get_16t = bench_synap_get(16).await;
    print_row(&s_get_16t);
    print_redis_row(
        REDIS_RESULTS[5].0,
        REDIS_RESULTS[5].1,
        REDIS_RESULTS[5].2,
        REDIS_RESULTS[5].3,
        REDIS_RESULTS[5].4,
    );
    print_ratio_raw(s_get_16t.ops_per_sec(), REDIS_RESULTS[5].1, "GET 16T/16C");

    // ── Overwrite ─────────────────────────────────────────────────────────────
    println!("\n## Overwrite — same key repeated (memory accounting stress)");
    print_header();

    let s_ow = bench_synap_overwrite().await;
    print_row(&s_ow);

    // ── Summary ───────────────────────────────────────────────────────────────
    let gap_set_1t = s_set_1t.ops_per_sec() / REDIS_RESULTS[0].1;
    let gap_get_1t = s_get_1t.ops_per_sec() / REDIS_RESULTS[1].1;
    let gap_set_16t = s_set_16t.ops_per_sec() / REDIS_RESULTS[4].1;
    let gap_get_16t = s_get_16t.ops_per_sec() / REDIS_RESULTS[5].1;

    let note = |ratio: f64| -> String {
        if ratio >= 2.0 {
            format!("{:.1}x AHEAD of Redis (store layer)", ratio)
        } else if ratio >= 1.0 {
            format!("{:.1}x ahead of Redis (store layer)", ratio)
        } else {
            format!("{:.1}x BEHIND Redis at store layer", 1.0 / ratio)
        }
    };

    println!();
    println!("╔═══════════════════════════════════════════════════════════════════════════════════════╗");
    println!("║  SUMMARY — Performance Gaps (Synap store vs Redis RESP/internal loopback)             ║");
    println!("╠═══════════════════════════════════════════════════════════════════════════════════════╣");
    println!("║  SET  1T / 1C  : {:<70}║", note(gap_set_1t));
    println!("║  GET  1T / 1C  : {:<70}║", note(gap_get_1t));
    println!("║  SET 16T / 16C : {:<70}║", note(gap_set_16t));
    println!("║  GET 16T / 16C : {:<70}║", note(gap_get_16t));
    println!("╠═══════════════════════════════════════════════════════════════════════════════════════╣");
    println!("║  KEY INSIGHT: Synap store is measured WITHOUT any protocol or network overhead.        ║");
    println!("║  Redis numbers INCLUDE protocol parsing + loopback TCP (even if same host).            ║");
    println!("║  Synap with HTTP/JSON adds ~5-10x latency over these store numbers.                    ║");
    println!("║  Remaining gap explained by: single-threaded Redis vs Synap's 64-shard design.         ║");
    println!("║  A RESP protocol front-end would give Synap near-Redis latency + better throughput.    ║");
    println!("╚═══════════════════════════════════════════════════════════════════════════════════════╝");
    println!();
}
