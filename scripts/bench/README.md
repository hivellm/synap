# Benchmarks

| File | What it is |
|---|---|
| [`perf_baseline.py`](perf_baseline.py) | The performance baseline: repeated, recorded runs against a real `synap-server` (below). |
| [`bench-config.yml`](bench-config.yml) | Benchmark server config: auth, rate limiting, replication and persistence off; RESP3 and SynapRPC on. The baseline starts every server from it. |
| [`bench-vs-redis/`](bench-vs-redis/) | In-process KV store numbers next to `redis-benchmark` figures. |
| [`bench-redis-comparison.sh`](bench-redis-comparison.sh) | Starts Redis and runs the `redis_vs_synap` criterion bench. |
| [`test-performance.sh`](test-performance.sh), [`test-performance.ps1`](test-performance.ps1) | Release build, tests, then the criterion suites. |
| [`Dockerfile.bench`](Dockerfile.bench) | Benchmark-only image for the Redis head-to-head. |

The criterion suites themselves live in `crates/synap-server/benches/`; CI only
compiles them (`cargo bench --no-run`).

## Performance baseline

`perf_baseline.py` measures a `synap-server` binary the way a client sees it:
over loopback TCP, through RESP3 and SynapRPC. For every run it records, per
durability mode, protocol and workload:

- throughput (ops/s) and p50 / p95 / p99 latency (also p99.9, max and mean);
- the server's resident set size (RSS): at the start, at the end and the
  highest sample, read every 50 ms;
- the workload, the client count and the pipeline depth;
- the durability mode and the exact persistence settings it ran with;
- the build: server version, git sha (and whether the tree was dirty), cargo
  profile, rustc version;
- the hardware: CPU model, physical and logical cores, RAM, OS and kernel.

It repeats the whole matrix `--runs` times and reports, per cell, the median
across runs, the minimum, the maximum, the spread ((max - min) / median) and
the standard deviation, plus every individual run.

```bash
# Build the server and the load generator (cargo --locked), 5 runs of everything
python scripts/bench/perf_baseline.py

# Quick check that the script works end to end
python scripts/bench/perf_baseline.py --runs 1 --duration 1 --warmup 0.2

# Only part of the matrix
python scripts/bench/perf_baseline.py --durability off,always --protocols resp3 --workloads set,set-pipelined

# Another server binary, e.g. 1.3.2, measured the same way. Its git sha and
# profile cannot be read from the binary, so pass them; the rustc commit is
# read from the binary unless --server-toolchain names the compiler.
python scripts/bench/perf_baseline.py --server-bin /path/to/synap-server-1.3.2 \
    --server-git-sha <sha> --server-profile release
```

A binary that cannot run a cell (a listener it lacks, a config key or command
it rejects) fails that cell only: the run goes on, the summary marks the cell
`FAILED`, and `failed_runs` in the JSON holds the reason (the server's last log
lines, or the load generator's error).

It needs Python 3.10+ with PyYAML (`python -m pip install pyyaml`) and the
toolchain from `rust-toolchain.toml`.

### The matrix

| Dimension | Values |
|---|---|
| Durability (`--durability`) | `off` (persistence disabled), `never`, `periodic`, `always` (WAL on with that `fsync_mode`, snapshots on with the config's interval and threshold) |
| Protocol (`--protocols`) | `resp3`, `synaprpc` |
| Workload (`--workloads`) | `get` (random key of a pre-written keyspace), `set` (random key), `list` (RPUSH then LPOP alternating on one list per client), `set-pipelined` (`set` with `--pipeline` requests per round trip, default 16) |

Defaults: 4 clients, 5 s measured after 1 s of warmup, keyspace 100 000,
64-byte values. Every cell gets a fresh server in its own scratch directory
(free ports, WAL and snapshots inside it), so one cell's data, WAL or memory
never leaks into the next. The runs are interleaved, run 1 of every cell before
run 2 of any, so a slow drift of the machine spreads over all cells instead of
landing on one.

### Output

`target/perf-baseline/perf-baseline-<UTC timestamp>.json` (schema
`synap-perf-baseline/1`) and a `.md` copy of the summary printed at the end.
The JSON holds `runner`, `build`, `hardware`, `parameters`,
`durability_modes`, `workloads` and `results`; each entry of `results` is one
(durability, protocol, workload) cell with `summary` (median / min / max /
spread / stdev per metric) and `runs` (every run's raw numbers).

`--runner-class` labels where the numbers came from: `controlled` (a dedicated
runner; the only class marked `comparable: true`), `shared` (a shared CI
runner; the numbers only show that the script works) or `local` (the default).

The exit status is non-zero only when the measurement broke: a server that did
not start or exited, a load generator failure, or error replies. No latency or
throughput threshold exists; comparing two reports is a human decision.

### Design

- **Language.** The driver is Python, like the other scripts that start a
  server and orchestrate clients (`scripts/interop/run-matrix.py`,
  `scripts/release/artifact-smoke/run.py`). The load itself comes from a Rust
  program, `crates/synap-server/examples/bench_loadgen.rs`, because a Python
  client cannot saturate the server and would add its own interpreter time to
  every latency sample. The load generator uses raw sockets for both protocols
  (no client library in the measured path), keeps every latency sample and
  reports exact nearest-rank percentiles. It also reads the server's RSS and
  the host facts through the `sysinfo` crate the server already depends on, so
  the same code works on Linux, macOS and Windows. Being a cargo example, it
  builds with the workspace's `Cargo.lock` and is type-checked and linted with
  `--all-targets` in CI.
- **Latency** of a request is the time from writing its batch to reading its
  reply, as in `redis-benchmark`. With `--pipeline 16`, the 16 requests of a
  batch share a start time.
- **Memory** is the server process's RSS (`sysinfo` `Process::memory`, the
  working set on Windows), not host memory.
- **The load generator runs on the same machine as the server** and competes
  with it for CPU. Numbers compare between runs on the same hardware, not
  across machines.

The scheduled workflow that runs this script is described in
[docs/development/CI_GATES.md](../../docs/development/CI_GATES.md#performance-baseline).
