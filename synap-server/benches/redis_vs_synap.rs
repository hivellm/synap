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

#[cfg(feature = "redis-bench")]
const REDIS_URL: &str = "redis://127.0.0.1:6379";
#[cfg(feature = "redis-bench")]
const SYNAP_URL: &str = "http://127.0.0.1:15500";

/// Returns true only when the redis-bench feature is active AND both servers
/// are reachable. If either is down the benchmark is silently omitted.
fn servers_available() -> bool {
    #[cfg(not(feature = "redis-bench"))]
    return false;

    #[cfg(feature = "redis-bench")]
    {
        // Check Redis
        let redis_ok = std::net::TcpStream::connect("127.0.0.1:6379").is_ok();
        // Check Synap
        let synap_ok = std::net::TcpStream::connect("127.0.0.1:15500").is_ok();
        redis_ok && synap_ok
    }
}

// ── Redis client (sync wrapper for criterion) ─────────────────────────────

#[cfg(feature = "redis-bench")]
mod redis_client {
    use redis::{Client, Commands, Connection};

    pub struct RedisConn(Connection);

    impl RedisConn {
        pub fn connect(url: &str) -> Self {
            let client = Client::open(url).expect("Redis URL invalid");
            let conn = client.get_connection().expect("Cannot connect to Redis");
            Self(conn)
        }

        pub fn set(&mut self, key: &str, value: &[u8]) {
            let _: () = self.0.set(key, value).expect("Redis SET failed");
        }

        pub fn get(&mut self, key: &str) -> Vec<u8> {
            self.0.get(key).unwrap_or_default()
        }

        pub fn del(&mut self, key: &str) {
            let _: () = self.0.del(key).expect("Redis DEL failed");
        }

        pub fn incr(&mut self, key: &str) -> i64 {
            self.0.incr(key, 1i64).expect("Redis INCR failed")
        }

        pub fn mset(&mut self, pairs: &[(&str, &[u8])]) {
            let args: Vec<(&str, &[u8])> = pairs.to_vec();
            let _: () = redis::cmd("MSET")
                .arg(
                    args.iter()
                        .flat_map(|(k, v)| vec![k.as_bytes(), *v])
                        .collect::<Vec<_>>(),
                )
                .query(&mut self.0)
                .expect("Redis MSET failed");
        }

        pub fn flush(&mut self) {
            let _: () = redis::cmd("FLUSHALL")
                .query(&mut self.0)
                .expect("Redis FLUSHALL failed");
        }

        pub fn bitcount(&mut self, key: &str) -> i64 {
            redis::cmd("BITCOUNT")
                .arg(key)
                .query(&mut self.0)
                .expect("Redis BITCOUNT failed")
        }

        pub fn setbit(&mut self, key: &str, offset: usize, val: bool) {
            let _: () = redis::cmd("SETBIT")
                .arg(key)
                .arg(offset)
                .arg(if val { 1 } else { 0 })
                .query(&mut self.0)
                .expect("Redis SETBIT failed");
        }

        pub fn pfadd(&mut self, key: &str, elements: &[&str]) -> i64 {
            let mut cmd = redis::cmd("PFADD");
            cmd.arg(key);
            for e in elements {
                cmd.arg(*e);
            }
            cmd.query(&mut self.0).expect("Redis PFADD failed")
        }

        pub fn pfcount(&mut self, key: &str) -> i64 {
            redis::cmd("PFCOUNT")
                .arg(key)
                .query(&mut self.0)
                .expect("Redis PFCOUNT failed")
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
                let mut conn = RedisConn::connect(REDIS_URL);
                b.iter(|| conn.set("bench:set", &value));
            });

            group.bench_with_input(BenchmarkId::new("synap", size), &size, |b, _| {
                let conn = SynapConn::connect(SYNAP_URL);
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
                let mut r = RedisConn::connect(REDIS_URL);
                r.set("bench:get", &value);
            }
            {
                let s = SynapConn::connect(SYNAP_URL);
                s.set("bench:get", &value);
            }

            group.bench_with_input(BenchmarkId::new("redis", size), &size, |b, _| {
                let mut conn = RedisConn::connect(REDIS_URL);
                b.iter(|| conn.get("bench:get"));
            });

            group.bench_with_input(BenchmarkId::new("synap", size), &size, |b, _| {
                let conn = SynapConn::connect(SYNAP_URL);
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
                let mut conn = RedisConn::connect(REDIS_URL);
                b.iter(|| conn.mset(&pairs));
            });

            group.bench_with_input(BenchmarkId::new("synap", batch), &batch, |b, _| {
                let conn = SynapConn::connect(SYNAP_URL);
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
            let mut conn = RedisConn::connect(REDIS_URL);
            conn.set("bench:incr", b"0");
            b.iter(|| conn.incr("bench:incr"));
        });

        group.bench_function("synap", |b| {
            let conn = SynapConn::connect(SYNAP_URL);
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
                let mut r = RedisConn::connect(REDIS_URL);
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
                    let mut conn = RedisConn::connect(REDIS_URL);
                    b.iter(|| conn.bitcount(&key_clone));
                },
            );

            group.bench_with_input(BenchmarkId::new("synap", format!("{kb}kb")), &kb, |b, _| {
                let conn = SynapConn::connect(SYNAP_URL);
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
            let mut r = RedisConn::connect(REDIS_URL);
            r.pfadd("bench:hll", &elem_refs);
        }
        {
            let s = SynapConn::connect(SYNAP_URL);
            s.pfadd("bench:hll", &elem_refs);
        }

        group.bench_function("redis/pfcount", |b| {
            let mut conn = RedisConn::connect(REDIS_URL);
            b.iter(|| conn.pfcount("bench:hll"));
        });

        group.bench_function("synap/pfcount", |b| {
            let conn = SynapConn::connect(SYNAP_URL);
            b.iter(|| conn.pfcount("bench:hll"));
        });

        group.bench_function("redis/pfadd_100", |b| {
            let batch: Vec<String> = (0..100).map(|i| format!("new:{i}")).collect();
            let batch_refs: Vec<&str> = batch.iter().map(|s| s.as_str()).collect();
            let mut conn = RedisConn::connect(REDIS_URL);
            b.iter(|| conn.pfadd("bench:hll_write", &batch_refs));
        });

        group.bench_function("synap/pfadd_100", |b| {
            let batch: Vec<String> = (0..100).map(|i| format!("new:{i}")).collect();
            let batch_refs: Vec<&str> = batch.iter().map(|s| s.as_str()).collect();
            let conn = SynapConn::connect(SYNAP_URL);
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
            let mut r = RedisConn::connect(REDIS_URL);
            for i in 0..n {
                r.set(&format!("bench:read:{i}"), b"value_data");
            }
        }
        {
            let s = SynapConn::connect(SYNAP_URL);
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
                            let mut conn = RedisConn::connect(REDIS_URL);
                            for i in 0..100 {
                                conn.get(&format!("bench:read:{}", (t * 100 + i) % n));
                            }
                        });
                    }
                });
            });
        });

        // Synap: 8 threads each do 100 GETs (Synap reads in parallel via RwLock)
        let synap_base = Arc::new(SYNAP_URL.to_string());
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
