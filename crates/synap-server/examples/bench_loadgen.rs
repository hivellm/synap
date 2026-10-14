//! Load generator for the performance baseline (`scripts/bench/perf_baseline.py`).
//!
//! Drives a running `synap-server` over RESP3 or SynapRPC with raw TCP
//! clients, so the numbers measure the server and not a client library. Each
//! client is one OS thread with one blocking connection that writes a batch of
//! `--pipeline` requests and then reads their replies; a request's latency is
//! the time from writing its batch to reading its reply (the same definition
//! `redis-benchmark` uses). Every measured latency is kept and the percentiles
//! are exact (nearest rank), not histogram buckets.
//!
//! Subcommands:
//! - `run`: one workload against one listener. Prints one JSON object on
//!   stdout. With `--server-pid` it also samples the server's resident set
//!   size (RSS) while the workload runs.
//! - `host-info`: CPU model, cores, RAM and OS of this machine as JSON.
//!
//! Workloads:
//! - `set`: `SET key:<n> <value>` with `n` uniform in `0..keyspace`.
//! - `get`: `GET key:<n>` after every key in the keyspace has been written.
//! - `list`: `RPUSH` then `LPOP` alternating on one list per client, so the
//!   list stays short and memory stays flat.
//!
//! Pipelined writes are `set` with `--pipeline` above 1.

use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, anyhow, bail};
use clap::{Parser, Subcommand, ValueEnum};
use serde_json::{Value as Json, json};
use sysinfo::{
    CpuRefreshKind, MemoryRefreshKind, Pid, ProcessRefreshKind, ProcessesToUpdate, RefreshKind,
    System,
};
use thunder::wire::encode_frame;
use thunder::{Request, Response, Value};

/// Requests per batch while writing the `get` keyspace before measuring.
const SEED_BATCH: u64 = 512;

#[derive(Parser, Debug)]
#[command(about = "Synap load generator for the performance baseline")]
struct Cli {
    #[command(subcommand)]
    mode: Mode,
}

#[derive(Subcommand, Debug)]
enum Mode {
    /// Run one workload against one listener and print the result as JSON.
    Run(RunArgs),
    /// Print this machine's CPU model, cores, RAM and OS as JSON.
    HostInfo,
}

#[derive(clap::Args, Debug, Clone)]
struct RunArgs {
    /// Wire protocol of the listener at `--addr`.
    #[arg(long, value_enum)]
    protocol: Protocol,
    /// Listener address, e.g. 127.0.0.1:6379.
    #[arg(long)]
    addr: String,
    #[arg(long, value_enum)]
    workload: Workload,
    /// Concurrent connections, one thread each.
    #[arg(long, default_value_t = 4)]
    clients: usize,
    /// Requests written per batch before reading their replies.
    #[arg(long, default_value_t = 1)]
    pipeline: usize,
    /// Measured seconds.
    #[arg(long, default_value_t = 5.0)]
    duration_secs: f64,
    /// Unmeasured seconds of load before the measured window.
    #[arg(long, default_value_t = 1.0)]
    warmup_secs: f64,
    /// Distinct keys the `set` and `get` workloads spread over.
    #[arg(long, default_value_t = 100_000)]
    keyspace: u64,
    /// Bytes per written value.
    #[arg(long, default_value_t = 64)]
    value_size: usize,
    /// Server process to sample RSS from while the workload runs.
    #[arg(long)]
    server_pid: Option<u32>,
    /// Milliseconds between RSS samples.
    #[arg(long, default_value_t = 50)]
    sample_interval_ms: u64,
}

#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
enum Protocol {
    Resp3,
    Synaprpc,
}

#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
enum Workload {
    Set,
    Get,
    List,
}

/// One request a client sends.
enum Op {
    Set(String),
    Get(String),
    RPush(String),
    LPop(String),
}

impl Op {
    fn command(&self) -> &'static str {
        match self {
            Op::Set(_) => "SET",
            Op::Get(_) => "GET",
            Op::RPush(_) => "RPUSH",
            Op::LPop(_) => "LPOP",
        }
    }

    fn key(&self) -> &str {
        match self {
            Op::Set(k) | Op::Get(k) | Op::RPush(k) | Op::LPop(k) => k,
        }
    }

    fn carries_value(&self) -> bool {
        matches!(self, Op::Set(_) | Op::RPush(_))
    }
}

/// Per-thread xorshift64* generator: no dependency, distinct per client.
struct Rng(u64);

impl Rng {
    fn for_client(index: usize) -> Self {
        Self(0x9E37_79B9_7F4A_7C15 ^ ((index as u64 + 1) << 17))
    }

    fn below(&mut self, bound: u64) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D) % bound.max(1)
    }
}

impl Workload {
    /// The `seq`-th request of client `client`.
    fn op(self, rng: &mut Rng, seq: u64, client: usize, keyspace: u64) -> Op {
        match self {
            Workload::Set => Op::Set(format!("key:{}", rng.below(keyspace))),
            Workload::Get => Op::Get(format!("key:{}", rng.below(keyspace))),
            Workload::List if seq.is_multiple_of(2) => Op::RPush(format!("list:{client}")),
            Workload::List => Op::LPop(format!("list:{client}")),
        }
    }
}

/// One blocking connection speaking either protocol.
struct Conn {
    protocol: Protocol,
    writer: TcpStream,
    reader: BufReader<TcpStream>,
    out: Vec<u8>,
    line: Vec<u8>,
    body: Vec<u8>,
    next_id: u32,
}

impl Conn {
    fn connect(protocol: Protocol, addr: &str) -> Result<Self> {
        let stream = TcpStream::connect(addr).with_context(|| format!("connect to {addr}"))?;
        stream.set_nodelay(true).context("set TCP_NODELAY")?;
        let reader = BufReader::with_capacity(64 * 1024, stream.try_clone()?);
        Ok(Self {
            protocol,
            writer: stream,
            reader,
            out: Vec::with_capacity(64 * 1024),
            line: Vec::with_capacity(128),
            body: Vec::with_capacity(1024),
            next_id: 1,
        })
    }

    /// Append one encoded request to the pending batch.
    fn queue(&mut self, op: &Op, value: &[u8]) -> Result<()> {
        match self.protocol {
            Protocol::Resp3 => {
                let key = op.key().as_bytes();
                if op.carries_value() {
                    resp_encode(&mut self.out, &[op.command().as_bytes(), key, value])
                } else {
                    resp_encode(&mut self.out, &[op.command().as_bytes(), key])
                }
            }
            Protocol::Synaprpc => {
                let mut args = vec![Value::Str(op.key().to_string())];
                if op.carries_value() {
                    args.push(Value::from(value.to_vec()));
                }
                let request = Request {
                    id: self.next_id,
                    command: op.command().to_string(),
                    args,
                };
                self.next_id = self.next_id.wrapping_add(1);
                let frame = encode_frame(&request).context("encode SynapRPC frame")?;
                self.out.extend_from_slice(&frame);
                Ok(())
            }
        }
    }

    /// Write the pending batch.
    fn flush(&mut self) -> Result<()> {
        self.writer.write_all(&self.out).context("write batch")?;
        self.out.clear();
        Ok(())
    }

    /// Read one reply; `true` when the server answered with an error.
    fn read_reply(&mut self) -> Result<bool> {
        match self.protocol {
            Protocol::Resp3 => read_resp(&mut self.reader, &mut self.line),
            Protocol::Synaprpc => read_rpc(&mut self.reader, &mut self.body),
        }
    }
}

fn resp_encode(out: &mut Vec<u8>, args: &[&[u8]]) -> Result<()> {
    write!(out, "*{}\r\n", args.len())?;
    for arg in args {
        write!(out, "${}\r\n", arg.len())?;
        out.extend_from_slice(arg);
        out.extend_from_slice(b"\r\n");
    }
    Ok(())
}

fn parse_resp_len(body: &[u8]) -> Result<i64> {
    std::str::from_utf8(body)?
        .parse::<i64>()
        .map_err(|e| anyhow!("bad RESP length {:?}: {e}", String::from_utf8_lossy(body)))
}

/// Read and discard one RESP2/RESP3 reply; `true` when it is (or holds) an error.
fn read_resp(reader: &mut BufReader<TcpStream>, line: &mut Vec<u8>) -> Result<bool> {
    line.clear();
    reader.read_until(b'\n', line).context("read RESP reply")?;
    if line.len() < 3 || !line.ends_with(b"\r\n") {
        bail!("connection closed or malformed RESP line");
    }
    let kind = line[0];
    let body = &line[1..line.len() - 2];
    match kind {
        b'+' | b':' | b'_' | b',' | b'#' | b'(' => Ok(false),
        b'-' => Ok(true),
        b'$' | b'=' | b'!' => {
            let len = parse_resp_len(body)?;
            if len >= 0 {
                let want = len as u64 + 2;
                let got = io::copy(&mut reader.by_ref().take(want), &mut io::sink())?;
                if got != want {
                    bail!("connection closed inside a bulk reply");
                }
            }
            Ok(kind == b'!')
        }
        b'*' | b'~' | b'>' | b'%' | b'|' => {
            let len = parse_resp_len(body)?.max(0) as u64;
            let items = if matches!(kind, b'%' | b'|') {
                len * 2
            } else {
                len
            };
            let mut error = false;
            for _ in 0..items {
                error |= read_resp(reader, line)?;
            }
            // An attribute map precedes the reply it annotates.
            if kind == b'|' {
                error |= read_resp(reader, line)?;
            }
            Ok(error)
        }
        other => bail!("unknown RESP type byte {:?}", other as char),
    }
}

/// Read one SynapRPC response frame; `true` when it carries an error.
fn read_rpc(reader: &mut BufReader<TcpStream>, body: &mut Vec<u8>) -> Result<bool> {
    loop {
        let mut len = [0u8; 4];
        reader.read_exact(&mut len).context("read SynapRPC frame")?;
        let n = u32::from_le_bytes(len) as usize;
        if n == 0 {
            // Zero-length frame: keep-alive, not a reply.
            continue;
        }
        body.resize(n, 0);
        reader.read_exact(body).context("read SynapRPC body")?;
        let response: Response = rmp_serde::from_slice(body).context("decode SynapRPC reply")?;
        return Ok(response.result.is_err());
    }
}

/// Shared, read-only state of one `run`.
struct Plan {
    args: RunArgs,
    value: Vec<u8>,
    measure_start: Instant,
    measure_end: Instant,
}

/// What one client thread measured.
#[derive(Default)]
struct ClientResult {
    latencies_ns: Vec<u64>,
    errors: u64,
    last_reply: Option<Instant>,
}

fn client_loop(plan: &Plan, client: usize) -> Result<ClientResult> {
    let args = &plan.args;
    let mut conn = Conn::connect(args.protocol, &args.addr)?;
    let mut rng = Rng::for_client(client);
    let mut result = ClientResult::default();
    let mut seq = 0u64;
    while Instant::now() < plan.measure_end {
        for _ in 0..args.pipeline {
            let op = args.workload.op(&mut rng, seq, client, args.keyspace);
            conn.queue(&op, &plan.value)?;
            seq += 1;
        }
        let batch_start = Instant::now();
        conn.flush()?;
        let measured = batch_start >= plan.measure_start;
        for _ in 0..args.pipeline {
            let error = conn.read_reply()?;
            if measured {
                result
                    .latencies_ns
                    .push(batch_start.elapsed().as_nanos() as u64);
                result.errors += u64::from(error);
            }
        }
        if measured {
            result.last_reply = Some(Instant::now());
        }
    }
    Ok(result)
}

/// Write every key of the `get` keyspace so each GET finds a value.
fn seed_keyspace(args: &RunArgs, value: &[u8]) -> Result<()> {
    let mut conn = Conn::connect(args.protocol, &args.addr)?;
    let mut next = 0u64;
    while next < args.keyspace {
        let end = (next + SEED_BATCH).min(args.keyspace);
        for n in next..end {
            conn.queue(&Op::Set(format!("key:{n}")), value)?;
        }
        conn.flush()?;
        for _ in next..end {
            if conn.read_reply()? {
                bail!("the server rejected a SET while seeding the keyspace");
            }
        }
        next = end;
    }
    Ok(())
}

/// Resident set size samples of the server process.
#[derive(Default)]
struct RssSamples {
    start: Option<u64>,
    end: Option<u64>,
    peak: u64,
    count: u64,
}

fn read_rss(sys: &mut System, pid: Pid) -> Option<u64> {
    sys.refresh_processes_specifics(
        ProcessesToUpdate::Some(&[pid]),
        true,
        ProcessRefreshKind::nothing().with_memory(),
    );
    sys.process(pid).map(|p| p.memory())
}

/// Sample the server's RSS every `interval` until `stop` is set.
fn sample_rss(pid: u32, interval: Duration, stop: &AtomicBool) -> Result<RssSamples> {
    let pid = Pid::from_u32(pid);
    let mut sys = System::new();
    let mut samples = RssSamples::default();
    loop {
        let stopping = stop.load(Ordering::Acquire);
        let rss = read_rss(&mut sys, pid)
            .ok_or_else(|| anyhow!("server process {pid} is gone; it exited during the run"))?;
        samples.start.get_or_insert(rss);
        samples.end = Some(rss);
        samples.peak = samples.peak.max(rss);
        samples.count += 1;
        if stopping {
            return Ok(samples);
        }
        thread::sleep(interval);
    }
}

/// Nearest-rank percentile of an ascending slice, in microseconds.
fn percentile_us(sorted_ns: &[u64], quantile: f64) -> f64 {
    if sorted_ns.is_empty() {
        return 0.0;
    }
    let rank = (quantile * sorted_ns.len() as f64).ceil() as usize;
    let index = rank.clamp(1, sorted_ns.len()) - 1;
    sorted_ns[index] as f64 / 1_000.0
}

fn round2(x: f64) -> f64 {
    (x * 100.0).round() / 100.0
}

fn latency_json(sorted_ns: &[u64]) -> Json {
    let mean_ns = if sorted_ns.is_empty() {
        0.0
    } else {
        sorted_ns.iter().map(|&n| n as f64).sum::<f64>() / sorted_ns.len() as f64
    };
    json!({
        "p50": round2(percentile_us(sorted_ns, 0.50)),
        "p95": round2(percentile_us(sorted_ns, 0.95)),
        "p99": round2(percentile_us(sorted_ns, 0.99)),
        "p999": round2(percentile_us(sorted_ns, 0.999)),
        "max": round2(percentile_us(sorted_ns, 1.0)),
        "mean": round2(mean_ns / 1_000.0),
    })
}

fn rss_json(samples: Option<RssSamples>) -> Json {
    match samples {
        None => Json::Null,
        Some(s) => json!({
            "start": s.start,
            "end": s.end,
            "peak": s.peak,
            "samples": s.count,
        }),
    }
}

fn validate(args: &RunArgs) -> Result<()> {
    if args.clients == 0 || args.pipeline == 0 {
        bail!("--clients and --pipeline must be at least 1");
    }
    // `Duration::from_secs_f64` panics on NaN and infinity, so both must be finite.
    let finite = args.duration_secs.is_finite() && args.warmup_secs.is_finite();
    if !finite || args.duration_secs <= 0.0 || args.warmup_secs < 0.0 {
        bail!("--duration-secs must be positive and --warmup-secs not negative, both finite");
    }
    if args.keyspace == 0 {
        bail!("--keyspace must be at least 1");
    }
    Ok(())
}

fn run(args: RunArgs) -> Result<Json> {
    validate(&args)?;
    let value = vec![b'x'; args.value_size];
    if args.workload == Workload::Get {
        seed_keyspace(&args, &value)?;
    }

    let stop = Arc::new(AtomicBool::new(false));
    let sampler = args.server_pid.map(|pid| {
        let stop = Arc::clone(&stop);
        let interval = Duration::from_millis(args.sample_interval_ms.max(1));
        thread::spawn(move || sample_rss(pid, interval, &stop))
    });

    let start = Instant::now();
    let measure_start = start + Duration::from_secs_f64(args.warmup_secs);
    let plan = Arc::new(Plan {
        measure_end: measure_start + Duration::from_secs_f64(args.duration_secs),
        measure_start,
        value,
        args: args.clone(),
    });
    let handles: Vec<_> = (0..args.clients)
        .map(|client| {
            let plan = Arc::clone(&plan);
            thread::spawn(move || client_loop(&plan, client))
        })
        .collect();

    let mut latencies = Vec::new();
    let mut errors = 0u64;
    let mut last_reply = measure_start;
    let mut failure = None;
    for handle in handles {
        match handle
            .join()
            .map_err(|_| anyhow!("a client thread panicked"))?
        {
            Ok(r) => {
                latencies.extend(r.latencies_ns);
                errors += r.errors;
                last_reply = last_reply.max(r.last_reply.unwrap_or(measure_start));
            }
            Err(e) => failure = Some(e),
        }
    }
    stop.store(true, Ordering::Release);
    let rss = match sampler {
        Some(h) => Some(
            h.join()
                .map_err(|_| anyhow!("the RSS sampler panicked"))??,
        ),
        None => None,
    };
    if let Some(e) = failure {
        return Err(e.context("a client connection failed"));
    }

    latencies.sort_unstable();
    let elapsed = last_reply.duration_since(measure_start).as_secs_f64();
    let throughput = if elapsed > 0.0 {
        latencies.len() as f64 / elapsed
    } else {
        0.0
    };
    Ok(json!({
        "protocol": format!("{:?}", args.protocol).to_lowercase(),
        "workload": format!("{:?}", args.workload).to_lowercase(),
        "clients": args.clients,
        "pipeline": args.pipeline,
        "duration_s": args.duration_secs,
        "warmup_s": args.warmup_secs,
        "keyspace": args.keyspace,
        "value_size": args.value_size,
        "ops": latencies.len(),
        "errors": errors,
        "elapsed_s": round2(elapsed * 1_000.0) / 1_000.0,
        "throughput_ops_s": round2(throughput),
        "latency_us": latency_json(&latencies),
        "rss_bytes": rss_json(rss),
    }))
}

fn host_info() -> Json {
    let sys = System::new_with_specifics(
        RefreshKind::nothing()
            .with_cpu(CpuRefreshKind::nothing())
            .with_memory(MemoryRefreshKind::nothing().with_ram()),
    );
    json!({
        "cpu_model": sys.cpus().first().map(|c| c.brand().trim().to_string()),
        "physical_cores": System::physical_core_count(),
        "logical_cores": sys.cpus().len(),
        "ram_bytes": sys.total_memory(),
        "os": System::long_os_version(),
        "kernel": System::kernel_version(),
        "arch": System::cpu_arch(),
    })
}

fn main() -> Result<()> {
    let output = match Cli::parse().mode {
        Mode::Run(args) => run(args)?,
        Mode::HostInfo => host_info(),
    };
    println!("{output}");
    Ok(())
}
