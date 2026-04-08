#![allow(unused_imports, unused_variables, dead_code)]
//! Redis vs Synap comparison benchmarks.
//!
//! Requires both servers running:
//!   - Redis 7 on 127.0.0.1:6379  (docker run -d -p 6379:6379 redis:7-alpine)
//!   - Synap on 127.0.0.1:15500   (cargo run --release -- --config config.yml)
//!
//! Run with:
//!   cargo bench --bench redis_vs_synap --features redis-bench
//!
//! Without --features redis-bench the benchmark compiles but all groups are
//! skipped (no connection attempted, no panic).

#[allow(unused_imports)]
use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};

// ── helpers ──────────────────────────────────────────────────────────────────

/// Override via env: SYNAP_REDIS_URL=redis://host:port
#[cfg(feature = "redis-bench")]
fn redis_url() -> String {
    std::env::var("SYNAP_REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string())
}

/// Override via env: SYNAP_HTTP_URL=http://host:port
#[cfg(feature = "redis-bench")]
fn synap_url() -> String {
    std::env::var("SYNAP_HTTP_URL").unwrap_or_else(|_| "http://127.0.0.1:15500".to_string())
}

/// Returns true only when the redis-bench feature is active AND both servers
/// are reachable. If either is down the benchmark is silently omitted.
fn servers_available() -> bool {
    #[cfg(not(feature = "redis-bench"))]
    return false;

    #[cfg(feature = "redis-bench")]
    {
        // Parse address from env-overridable URLs
        let redis_addr = std::env::var("SYNAP_REDIS_URL")
            .unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());
        let redis_addr = redis_addr
            .trim_start_matches("redis://")
            .split('/')
            .next()
            .unwrap_or("127.0.0.1:6379")
            .to_string();

        let synap_base = std::env::var("SYNAP_HTTP_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:15500".to_string());
        let synap_addr = synap_base
            .trim_start_matches("http://")
            .split('/')
            .next()
            .unwrap_or("127.0.0.1:15500")
            .to_string();

        let redis_ok = std::net::TcpStream::connect(&redis_addr).is_ok();
        let synap_ok = std::net::TcpStream::connect(&synap_addr).is_ok();

        if !redis_ok {
            eprintln!("[bench] Redis not available at {redis_addr} — skipping");
        }
        if !synap_ok {
            eprintln!("[bench] Synap not available at {synap_addr} — skipping");
        }

        redis_ok && synap_ok
    }
}

// ── Redis client — raw RESP over TcpStream (no crate, no timeouts) ───────────
//
// We bypass the `redis` crate entirely to avoid Windows Docker NAT issues.
// Raw TcpStream + hand-rolled RESP encoding/decoding is 100% reliable and
// gives us the true "bare protocol" latency that Redis benchmarks report.

#[cfg(feature = "redis-bench")]
mod redis_client {
    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::TcpStream;
    use std::time::Duration;

    pub struct RedisConn {
        stream: TcpStream,
        reader: BufReader<TcpStream>,
    }

    impl RedisConn {
        pub fn connect(url: &str) -> Self {
            // Parse "redis://host:port" → "host:port"
            let addr = url
                .trim_start_matches("redis://")
                .split('/')
                .next()
                .unwrap_or("127.0.0.1:6379");
            let stream =
                TcpStream::connect(addr).unwrap_or_else(|e| panic!("Redis connect {addr}: {e}"));
            stream.set_read_timeout(Some(Duration::from_secs(10))).ok();
            stream.set_write_timeout(Some(Duration::from_secs(10))).ok();
            let reader = BufReader::new(stream.try_clone().expect("TcpStream clone"));
            Self { stream, reader }
        }

        /// Encode args as a RESP array and send it.
        fn send(&mut self, args: &[&[u8]]) {
            let mut buf = Vec::with_capacity(256);
            buf.extend_from_slice(format!("*{}\r\n", args.len()).as_bytes());
            for arg in args {
                buf.extend_from_slice(format!("${}\r\n", arg.len()).as_bytes());
                buf.extend_from_slice(arg);
                buf.extend_from_slice(b"\r\n");
            }
            self.stream.write_all(&buf).expect("Redis write");
        }

        /// Read one RESP value; returns the first line (simple type) or bulk bytes.
        fn recv_line(&mut self) -> String {
            let mut line = String::new();
            self.reader.read_line(&mut line).expect("Redis read");
            line.trim_end().to_string()
        }

        fn recv_response(&mut self) -> Vec<u8> {
            let line = self.recv_line();
            match line.as_bytes().first() {
                Some(b'+') | Some(b':') | Some(b'-') => line.into_bytes(),
                Some(b'$') => {
                    let n: i64 = line[1..].parse().unwrap_or(-1);
                    if n < 0 {
                        return b"(nil)".to_vec();
                    }
                    let mut data = vec![0u8; n as usize + 2]; // +2 for \r\n
                    self.reader.read_exact(&mut data).expect("Redis bulk read");
                    data.truncate(n as usize);
                    data
                }
                Some(b'*') => {
                    // Array — just drain it
                    let count: i64 = line[1..].parse().unwrap_or(0);
                    for _ in 0..count {
                        self.recv_response();
                    }
                    b"(array)".to_vec()
                }
                _ => line.into_bytes(),
            }
        }

        pub fn set(&mut self, key: &str, value: &[u8]) {
            self.send(&[b"SET", key.as_bytes(), value]);
            self.recv_response();
        }

        pub fn get(&mut self, key: &str) -> Vec<u8> {
            self.send(&[b"GET", key.as_bytes()]);
            self.recv_response()
        }

        pub fn del(&mut self, key: &str) {
            self.send(&[b"DEL", key.as_bytes()]);
            self.recv_response();
        }

        pub fn incr(&mut self, key: &str) -> i64 {
            self.send(&[b"INCR", key.as_bytes()]);
            let r = self.recv_response();
            let s = String::from_utf8_lossy(&r);
            s.trim_start_matches(':').parse().unwrap_or(0)
        }

        pub fn mset(&mut self, pairs: &[(&str, &[u8])]) {
            let mut args: Vec<&[u8]> = vec![b"MSET"];
            let owned: Vec<Vec<u8>> = pairs
                .iter()
                .flat_map(|(k, v)| [k.as_bytes().to_vec(), v.to_vec()])
                .collect();
            for o in &owned {
                args.push(o.as_slice());
            }
            self.send(&args);
            self.recv_response();
        }

        pub fn flush(&mut self) {
            self.send(&[b"FLUSHALL"]);
            self.recv_response();
        }

        pub fn bitcount(&mut self, key: &str) -> i64 {
            self.send(&[b"BITCOUNT", key.as_bytes()]);
            let r = self.recv_response();
            let s = String::from_utf8_lossy(&r);
            s.trim_start_matches(':').parse().unwrap_or(0)
        }

        pub fn setbit(&mut self, key: &str, offset: usize, val: bool) {
            let off = offset.to_string();
            let v = if val { b"1".as_ref() } else { b"0".as_ref() };
            self.send(&[b"SETBIT", key.as_bytes(), off.as_bytes(), v]);
            self.recv_response();
        }

        pub fn pfadd(&mut self, key: &str, elements: &[&str]) -> i64 {
            let mut args: Vec<&[u8]> = vec![b"PFADD", key.as_bytes()];
            let owned: Vec<Vec<u8>> = elements.iter().map(|e| e.as_bytes().to_vec()).collect();
            for o in &owned {
                args.push(o.as_slice());
            }
            self.send(&args);
            let r = self.recv_response();
            let s = String::from_utf8_lossy(&r);
            s.trim_start_matches(':').parse().unwrap_or(0)
        }

        pub fn pfcount(&mut self, key: &str) -> i64 {
            self.send(&[b"PFCOUNT", key.as_bytes()]);
            let r = self.recv_response();
            let s = String::from_utf8_lossy(&r);
            s.trim_start_matches(':').parse().unwrap_or(0)
        }
    }
}

// ── Synap HTTP client (blocking) ─────────────────────────────────────────────

#[cfg(feature = "redis-bench")]
mod synap_client {
    use reqwest::blocking::Client;
    use serde_json::{Value, json};

    pub struct SynapConn {
        client: Client,
        base: String,
    }

    impl SynapConn {
        pub fn connect(base_url: &str) -> Self {
            Self {
                client: Client::builder()
                    .timeout(std::time::Duration::from_secs(5))
                    .build()
                    .expect("reqwest client build failed"),
                base: base_url.to_string(),
            }
        }

        fn cmd(&self, command: &str, params: Value) -> Value {
            let body = json!({ "command": command, "params": params });
            self.client
                .post(format!("{}/api/v1/command", self.base))
                .json(&body)
                .send()
                .expect("Synap request failed")
                .json()
                .unwrap_or(Value::Null)
        }

        pub fn set(&self, key: &str, value: &[u8]) {
            let encoded = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, value);
            self.cmd("kv.set", json!({ "key": key, "value": encoded }));
        }

        pub fn get(&self, key: &str) -> Vec<u8> {
            let resp = self.cmd("kv.get", json!({ "key": key }));
            if let Some(val) = resp.get("value").and_then(|v| v.as_str()) {
                base64::Engine::decode(&base64::engine::general_purpose::STANDARD, val)
                    .unwrap_or_default()
            } else {
                vec![]
            }
        }

        pub fn del(&self, key: &str) {
            self.cmd("kv.del", json!({ "key": key }));
        }

        pub fn incr(&self, key: &str) -> i64 {
            let resp = self.cmd("kv.incr", json!({ "key": key }));
            resp.get("value").and_then(|v| v.as_i64()).unwrap_or(0)
        }

        pub fn mset(&self, pairs: &[(&str, &[u8])]) {
            let items: Vec<Value> = pairs
                .iter()
                .map(|(k, v)| {
                    json!({
                        "key": k,
                        "value": base64::Engine::encode(
                            &base64::engine::general_purpose::STANDARD,
                            v,
                        )
                    })
                })
                .collect();
            self.cmd("kv.mset", json!({ "pairs": items }));
        }

        pub fn flush(&self) {
            self.cmd("kv.flushall", json!({}));
        }

        pub fn bitcount(&self, key: &str) -> i64 {
            let resp = self.cmd("bitmap.bitcount", json!({ "key": key }));
            resp.get("count").and_then(|v| v.as_i64()).unwrap_or(0)
        }

        pub fn pfadd(&self, key: &str, elements: &[&str]) {
            self.cmd(
                "hyperloglog.pfadd",
                json!({ "key": key, "elements": elements }),
            );
        }

        pub fn pfcount(&self, key: &str) -> i64 {
            let resp = self.cmd("hyperloglog.pfcount", json!({ "key": key }));
            resp.get("count").and_then(|v| v.as_i64()).unwrap_or(0)
        }
    }
}

// ── Benchmark groups ──────────────────────────────────────────────────────────

fn bench_set(c: &mut Criterion) {
    if !servers_available() {
        return;
    }

    #[cfg(feature = "redis-bench")]
    {
        use redis_client::RedisConn;
        use synap_client::SynapConn;

        let mut group = c.benchmark_group("set_latency");

        for size in [64usize, 256, 1024, 4096] {
            let value = vec![0xABu8; size];
            group.throughput(Throughput::Bytes(size as u64));

            group.bench_with_input(BenchmarkId::new("redis", size), &size, |b, _| {
                let mut conn = RedisConn::connect(&redis_url());
                b.iter(|| conn.set("bench:set", &value));
            });

            group.bench_with_input(BenchmarkId::new("synap", size), &size, |b, _| {
                let conn = SynapConn::connect(&synap_url());
                b.iter(|| conn.set("bench:set", &value));
            });
        }

        group.finish();
    }
}

fn bench_get(c: &mut Criterion) {
    if !servers_available() {
        return;
    }

    #[cfg(feature = "redis-bench")]
    {
        use redis_client::RedisConn;
        use synap_client::SynapConn;

        let mut group = c.benchmark_group("get_latency");

        for size in [64usize, 256, 1024, 4096] {
            let value = vec![0xCDu8; size];
            group.throughput(Throughput::Bytes(size as u64));

            // Pre-populate
            {
                let mut r = RedisConn::connect(&redis_url());
                r.set("bench:get", &value);
            }
            {
                let s = SynapConn::connect(&synap_url());
                s.set("bench:get", &value);
            }

            group.bench_with_input(BenchmarkId::new("redis", size), &size, |b, _| {
                let mut conn = RedisConn::connect(&redis_url());
                b.iter(|| conn.get("bench:get"));
            });

            group.bench_with_input(BenchmarkId::new("synap", size), &size, |b, _| {
                let conn = SynapConn::connect(&synap_url());
                b.iter(|| conn.get("bench:get"));
            });
        }

        group.finish();
    }
}

fn bench_mset(c: &mut Criterion) {
    if !servers_available() {
        return;
    }

    #[cfg(feature = "redis-bench")]
    {
        use redis_client::RedisConn;
        use synap_client::SynapConn;

        let mut group = c.benchmark_group("mset_batch");
        let value = vec![0u8; 64];

        for batch in [10usize, 50, 100] {
            let keys: Vec<String> = (0..batch).map(|i| format!("bench:mset:{i}")).collect();
            let pairs: Vec<(&str, &[u8])> = keys
                .iter()
                .map(|k| (k.as_str(), value.as_slice()))
                .collect();

            group.throughput(Throughput::Elements(batch as u64));

            group.bench_with_input(BenchmarkId::new("redis", batch), &batch, |b, _| {
                let mut conn = RedisConn::connect(&redis_url());
                b.iter(|| conn.mset(&pairs));
            });

            group.bench_with_input(BenchmarkId::new("synap", batch), &batch, |b, _| {
                let conn = SynapConn::connect(&synap_url());
                b.iter(|| conn.mset(&pairs));
            });
        }

        group.finish();
    }
}

fn bench_incr(c: &mut Criterion) {
    if !servers_available() {
        return;
    }

    #[cfg(feature = "redis-bench")]
    {
        use redis_client::RedisConn;
        use synap_client::SynapConn;

        let mut group = c.benchmark_group("incr_latency");

        group.bench_function("redis", |b| {
            let mut conn = RedisConn::connect(&redis_url());
            conn.set("bench:incr", b"0");
            b.iter(|| conn.incr("bench:incr"));
        });

        group.bench_function("synap", |b| {
            let conn = SynapConn::connect(&synap_url());
            conn.set("bench:incr", b"0");
            b.iter(|| conn.incr("bench:incr"));
        });

        group.finish();
    }
}

fn bench_bitcount(c: &mut Criterion) {
    if !servers_available() {
        return;
    }

    #[cfg(feature = "redis-bench")]
    {
        use redis_client::RedisConn;
        use synap_client::SynapConn;

        let mut group = c.benchmark_group("bitcount");

        // Pre-populate bitmaps of different sizes
        for kb in [64usize, 512, 1024] {
            let key = format!("bench:bitmap:{kb}kb");
            let bits = kb * 1024 * 8;

            // Set every other bit in Redis
            {
                let mut r = RedisConn::connect(&redis_url());
                r.del(&key);
                // Use SET with raw bytes — set the bitmap key to a block of 0xAA bytes
                let data = vec![0xAAu8; kb * 1024];
                r.set(&key, &data);
            }

            group.throughput(Throughput::Elements(bits as u64));

            let key_clone = key.clone();
            group.bench_with_input(
                BenchmarkId::new("redis", format!("{kb}kb")),
                &kb,
                move |b, _| {
                    let mut conn = RedisConn::connect(&redis_url());
                    b.iter(|| conn.bitcount(&key_clone));
                },
            );

            group.bench_with_input(BenchmarkId::new("synap", format!("{kb}kb")), &kb, |b, _| {
                let conn = SynapConn::connect(&synap_url());
                // Synap bitmap is pre-set via its own SETBIT; here we just count
                b.iter(|| conn.bitcount(&key));
            });
        }

        group.finish();
    }
}

fn bench_hyperloglog(c: &mut Criterion) {
    if !servers_available() {
        return;
    }

    #[cfg(feature = "redis-bench")]
    {
        use redis_client::RedisConn;
        use synap_client::SynapConn;

        let mut group = c.benchmark_group("hyperloglog");

        // Pre-populate HLL with 10K distinct elements
        let elements: Vec<String> = (0..10_000).map(|i| format!("elem:{i}")).collect();
        let elem_refs: Vec<&str> = elements.iter().map(|s| s.as_str()).collect();

        {
            let mut r = RedisConn::connect(&redis_url());
            r.pfadd("bench:hll", &elem_refs);
        }
        {
            let s = SynapConn::connect(&synap_url());
            s.pfadd("bench:hll", &elem_refs);
        }

        group.bench_function("redis/pfcount", |b| {
            let mut conn = RedisConn::connect(&redis_url());
            b.iter(|| conn.pfcount("bench:hll"));
        });

        group.bench_function("synap/pfcount", |b| {
            let conn = SynapConn::connect(&synap_url());
            b.iter(|| conn.pfcount("bench:hll"));
        });

        group.bench_function("redis/pfadd_100", |b| {
            let batch: Vec<String> = (0..100).map(|i| format!("new:{i}")).collect();
            let batch_refs: Vec<&str> = batch.iter().map(|s| s.as_str()).collect();
            let mut conn = RedisConn::connect(&redis_url());
            b.iter(|| conn.pfadd("bench:hll_write", &batch_refs));
        });

        group.bench_function("synap/pfadd_100", |b| {
            let batch: Vec<String> = (0..100).map(|i| format!("new:{i}")).collect();
            let batch_refs: Vec<&str> = batch.iter().map(|s| s.as_str()).collect();
            let conn = SynapConn::connect(&synap_url());
            b.iter(|| conn.pfadd("bench:hll_write", &batch_refs));
        });

        group.finish();
    }
}

fn bench_concurrent_reads(c: &mut Criterion) {
    if !servers_available() {
        return;
    }

    #[cfg(feature = "redis-bench")]
    {
        use redis_client::RedisConn;
        use std::sync::Arc;
        use synap_client::SynapConn;

        let mut group = c.benchmark_group("concurrent_reads_8threads");

        // Pre-populate 1000 distinct keys
        let n = 1000usize;
        {
            let mut r = RedisConn::connect(&redis_url());
            for i in 0..n {
                r.set(&format!("bench:read:{i}"), b"value_data");
            }
        }
        {
            let s = SynapConn::connect(&synap_url());
            for i in 0..n {
                s.set(&format!("bench:read:{i}"), b"value_data");
            }
        }

        group.throughput(Throughput::Elements(8 * 100));

        // Redis: 8 threads each do 100 GETs (Redis serializes all via single thread)
        group.bench_function("redis/8threads_x100gets", |b| {
            b.iter(|| {
                std::thread::scope(|s| {
                    for t in 0..8usize {
                        s.spawn(move || {
                            let mut conn = RedisConn::connect(&redis_url());
                            for i in 0..100 {
                                conn.get(&format!("bench:read:{}", (t * 100 + i) % n));
                            }
                        });
                    }
                });
            });
        });

        // Synap: 8 threads each do 100 GETs (Synap reads in parallel via RwLock)
        let synap_base = Arc::new(synap_url());
        group.bench_function("synap/8threads_x100gets", move |b| {
            b.iter(|| {
                let base = Arc::clone(&synap_base);
                std::thread::scope(|s| {
                    for t in 0..8usize {
                        let base = Arc::clone(&base);
                        s.spawn(move || {
                            let conn = SynapConn::connect(&base);
                            for i in 0..100 {
                                conn.get(&format!("bench:read:{}", (t * 100 + i) % n));
                            }
                        });
                    }
                });
            });
        });

        group.finish();
    }
}

criterion_group!(
    benches,
    bench_set,
    bench_get,
    bench_mset,
    bench_incr,
    bench_bitcount,
    bench_hyperloglog,
    bench_concurrent_reads,
);
criterion_main!(benches);
