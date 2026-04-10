// protocol_bench.rs — Three-protocol latency/throughput comparison
//
// Measures SET + GET round-trip time for:
//   1. HTTP REST   — JSON over HTTP/1.1
//   2. RESP3 TCP   — Redis-compatible text protocol
//   3. SynapRPC    — Binary MessagePack framing
//
// Requires a running Synap server with all three protocols enabled:
//
//   cargo build --release
//   ./target/release/synap-server --config config.yml
//
// Then run:
//   cargo bench --bench protocol_bench
//
// Override endpoints:
//   SYNAP_HTTP_URL=http://127.0.0.1:15500
//   SYNAP_RESP3_ADDR=127.0.0.1:6379
//   SYNAP_RPC_ADDR=127.0.0.1:15501

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::time::Duration;

use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use synap_server::protocol::synap_rpc::types::{Request, Response, SynapValue};

// ── endpoint helpers ──────────────────────────────────────────────────────────

fn http_base() -> String {
    std::env::var("SYNAP_HTTP_URL").unwrap_or_else(|_| "http://127.0.0.1:15500".to_string())
}

fn resp3_addr() -> String {
    std::env::var("SYNAP_RESP3_ADDR").unwrap_or_else(|_| "127.0.0.1:6379".to_string())
}

fn rpc_addr() -> String {
    std::env::var("SYNAP_RPC_ADDR").unwrap_or_else(|_| "127.0.0.1:15501".to_string())
}

fn tcp_reachable(addr: &str) -> bool {
    TcpStream::connect_timeout(
        &addr.parse().expect("valid addr"),
        Duration::from_millis(200),
    )
    .is_ok()
}

fn all_endpoints_up() -> bool {
    let http_host = http_base()
        .trim_start_matches("http://")
        .split('/')
        .next()
        .unwrap_or("127.0.0.1:15500")
        .to_string();
    tcp_reachable(&http_host) && tcp_reachable(&resp3_addr()) && tcp_reachable(&rpc_addr())
}

// ── HTTP REST client ──────────────────────────────────────────────────────────

struct HttpClient {
    base: String,
    agent: ureq::Agent,
}

impl HttpClient {
    fn new(base: &str) -> Self {
        let agent = ureq::AgentBuilder::new()
            .timeout(Duration::from_secs(5))
            .build();
        Self {
            base: base.to_owned(),
            agent,
        }
    }

    fn set(&self, key: &str, value: &[u8]) {
        let url = format!("{}/kv/{}", self.base, key);
        let payload = ureq::json!({"value": std::str::from_utf8(value).unwrap_or("val")});
        let _ = self.agent.post(&url).send_json(payload);
    }

    fn get(&self, key: &str) -> Vec<u8> {
        let url = format!("{}/kv/{}", self.base, key);
        match self.agent.get(&url).call() {
            Ok(resp) => resp.into_string().unwrap_or_default().into_bytes(),
            Err(_) => vec![],
        }
    }
}

// ── RESP3 raw TCP client ──────────────────────────────────────────────────────

struct Resp3Client {
    stream: TcpStream,
    reader: BufReader<TcpStream>,
}

impl Resp3Client {
    fn connect(addr: &str) -> Self {
        let stream = TcpStream::connect(addr).expect("RESP3 connect");
        stream.set_read_timeout(Some(Duration::from_secs(5))).ok();
        stream.set_write_timeout(Some(Duration::from_secs(5))).ok();
        let reader = BufReader::new(stream.try_clone().expect("clone"));
        Self { stream, reader }
    }

    fn send(&mut self, args: &[&[u8]]) {
        let mut buf = format!("*{}\r\n", args.len()).into_bytes();
        for a in args {
            buf.extend_from_slice(format!("${}\r\n", a.len()).as_bytes());
            buf.extend_from_slice(a);
            buf.extend_from_slice(b"\r\n");
        }
        self.stream.write_all(&buf).expect("RESP3 write");
    }

    fn recv(&mut self) -> Vec<u8> {
        let mut line = String::new();
        self.reader.read_line(&mut line).expect("RESP3 read line");
        let line = line.trim_end_matches(['\r', '\n']);
        match line.as_bytes().first() {
            Some(b'+') | Some(b'-') | Some(b':') => line.as_bytes()[1..].to_vec(),
            Some(b'$') => {
                let len: isize = line[1..].parse().unwrap_or(-1);
                if len < 0 {
                    return vec![];
                }
                let mut data = vec![0u8; len as usize + 2];
                self.reader.read_exact(&mut data).expect("RESP3 read bulk");
                data[..len as usize].to_vec()
            }
            _ => line.as_bytes().to_vec(),
        }
    }

    fn set(&mut self, key: &str, value: &[u8]) {
        self.send(&[b"SET", key.as_bytes(), value]);
        self.recv();
    }

    fn get(&mut self, key: &str) -> Vec<u8> {
        self.send(&[b"GET", key.as_bytes()]);
        self.recv()
    }
}

// ── SynapRPC binary client ────────────────────────────────────────────────────

struct RpcClient {
    stream: TcpStream,
}

impl RpcClient {
    fn connect(addr: &str) -> Self {
        let stream = TcpStream::connect(addr).expect("RPC connect");
        stream.set_read_timeout(Some(Duration::from_secs(5))).ok();
        stream.set_write_timeout(Some(Duration::from_secs(5))).ok();
        Self { stream }
    }

    fn send_request(&mut self, req: &Request) {
        let body = rmp_serde::to_vec(req).expect("rmp-serde encode");
        let len = (body.len() as u32).to_le_bytes();
        self.stream.write_all(&len).expect("RPC write len");
        self.stream.write_all(&body).expect("RPC write body");
    }

    fn recv_response(&mut self) -> Response {
        let mut len_buf = [0u8; 4];
        self.stream.read_exact(&mut len_buf).expect("RPC read len");
        let len = u32::from_le_bytes(len_buf) as usize;
        let mut body = vec![0u8; len];
        self.stream.read_exact(&mut body).expect("RPC read body");
        rmp_serde::from_slice(&body).expect("rmp-serde decode response")
    }

    fn set(&mut self, key: &str, value: &[u8]) {
        let req = Request {
            id: 1,
            command: "SET".into(),
            args: vec![
                SynapValue::Str(key.into()),
                SynapValue::Bytes(value.to_vec()),
            ],
        };
        self.send_request(&req);
        self.recv_response();
    }

    fn get(&mut self, key: &str) -> Vec<u8> {
        let req = Request {
            id: 2,
            command: "GET".into(),
            args: vec![SynapValue::Str(key.into())],
        };
        self.send_request(&req);
        match self.recv_response().result {
            Ok(SynapValue::Bytes(b)) => b,
            Ok(SynapValue::Str(s)) => s.into_bytes(),
            _ => vec![],
        }
    }
}

// ── benchmark groups ──────────────────────────────────────────────────────────

fn bench_set_get(c: &mut Criterion) {
    if !all_endpoints_up() {
        eprintln!("⚠ protocol_bench: Synap not reachable — skipping");
        return;
    }

    let payload = b"benchmark_value_12345".as_slice();
    let key = "bench_proto_key";

    let mut group = c.benchmark_group("protocol_set_get");
    group.throughput(Throughput::Elements(1));
    group.warm_up_time(Duration::from_millis(500));
    group.measurement_time(Duration::from_secs(5));

    // ── HTTP REST ─────────────────────────────────────────────────────────────
    let http = HttpClient::new(&http_base());
    group.bench_function("http_rest", |b| {
        b.iter(|| {
            http.set(key, payload);
            http.get(key);
        });
    });

    // ── RESP3 ─────────────────────────────────────────────────────────────────
    let mut resp3 = Resp3Client::connect(&resp3_addr());
    group.bench_function("resp3_tcp", |b| {
        b.iter(|| {
            resp3.set(key, payload);
            resp3.get(key);
        });
    });

    // ── SynapRPC ──────────────────────────────────────────────────────────────
    let mut rpc = RpcClient::connect(&rpc_addr());
    group.bench_function("synap_rpc", |b| {
        b.iter(|| {
            rpc.set(key, payload);
            rpc.get(key);
        });
    });

    group.finish();
}

fn bench_set_only(c: &mut Criterion) {
    if !all_endpoints_up() {
        return;
    }

    let payload = b"val".as_slice();
    let mut group = c.benchmark_group("protocol_set_only");
    group.throughput(Throughput::Elements(1));
    group.warm_up_time(Duration::from_millis(500));
    group.measurement_time(Duration::from_secs(5));

    let http = HttpClient::new(&http_base());
    group.bench_function("http_rest", |b| {
        b.iter(|| http.set("bench_set_key", payload));
    });

    let mut resp3 = Resp3Client::connect(&resp3_addr());
    group.bench_function("resp3_tcp", |b| {
        b.iter(|| resp3.set("bench_set_key", payload));
    });

    let mut rpc = RpcClient::connect(&rpc_addr());
    group.bench_function("synap_rpc", |b| {
        b.iter(|| rpc.set("bench_set_key", payload));
    });

    group.finish();
}

fn bench_get_only(c: &mut Criterion) {
    if !all_endpoints_up() {
        return;
    }

    let payload = b"val".as_slice();
    let mut group = c.benchmark_group("protocol_get_only");
    group.throughput(Throughput::Elements(1));
    group.warm_up_time(Duration::from_millis(500));
    group.measurement_time(Duration::from_secs(5));

    // Pre-seed key
    let mut resp3_seed = Resp3Client::connect(&resp3_addr());
    resp3_seed.set("bench_get_key", payload);

    let http = HttpClient::new(&http_base());
    group.bench_function("http_rest", |b| {
        b.iter(|| http.get("bench_get_key"));
    });

    let mut resp3 = Resp3Client::connect(&resp3_addr());
    group.bench_function("resp3_tcp", |b| {
        b.iter(|| resp3.get("bench_get_key"));
    });

    let mut rpc = RpcClient::connect(&rpc_addr());
    group.bench_function("synap_rpc", |b| {
        b.iter(|| rpc.get("bench_get_key"));
    });

    group.finish();
}

criterion_group!(benches, bench_set_get, bench_set_only, bench_get_only);
criterion_main!(benches);
