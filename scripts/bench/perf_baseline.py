#!/usr/bin/env python3
"""Synap performance baseline: repeated, recorded runs against a real server.

For every run, durability mode, protocol and workload this starts a fresh
``synap-server`` in its own scratch directory, drives it with the
``bench_loadgen`` example (``crates/synap-server/examples/bench_loadgen.rs``)
and records throughput, p50/p95/p99 latency and the server's resident set
size. The report states the durability mode, the build (version, git sha,
profile, toolchain) and the hardware (CPU model, cores, RAM, OS) next to the
numbers, and aggregates the runs into a median plus the spread across runs.

Outputs, under ``--out-dir`` (default ``target/perf-baseline``):
    perf-baseline-<UTC timestamp>.json   machine-readable report
    perf-baseline-<UTC timestamp>.md     the short human summary printed at the end

The exit status is non-zero only when the measurement itself broke: a server
that would not start or died, a load generator that failed, or error replies.
There is no latency or throughput threshold. See scripts/bench/README.md.

Usage::

    python scripts/bench/perf_baseline.py                       # build, 5 runs, everything
    python scripts/bench/perf_baseline.py --runs 1 --duration 1 --warmup 0.2
    python scripts/bench/perf_baseline.py --server-bin /path/to/synap-server-1.3.2 \\
        --server-git-sha <sha> --server-profile release
    python scripts/bench/perf_baseline.py --merge a.json b.json c.json   # one report from several
"""

from __future__ import annotations

import argparse
import datetime as dt
import itertools
import json
import math
import os
import re
import socket
import statistics
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.request
from dataclasses import dataclass
from pathlib import Path

try:
    import yaml
except ImportError:  # pragma: no cover - environment check
    raise SystemExit("perf_baseline.py needs PyYAML: python -m pip install pyyaml")

REPO = Path(__file__).resolve().parents[2]
HERE = Path(__file__).resolve().parent
# The benchmark server config already used for the Redis head-to-head:
# auth, rate limiting, replication and persistence off, RESP3 and SynapRPC on.
# Each cell overrides only the bind addresses, ports, log level and persistence.
BASE_CONFIG = HERE / "bench-config.yml"
EXE = ".exe" if os.name == "nt" else ""
SCHEMA = "synap-perf-baseline/1"

DURABILITY_MODES = ("off", "never", "periodic", "always")
PROTOCOLS = ("resp3", "synaprpc")
RUNNER_CLASSES = {
    "controlled": "dedicated runner; comparable with other controlled runs on the same hardware",
    "shared": "shared CI runner; smoke test of the script only, the numbers are NOT comparable",
    "local": "developer machine; comparable only with runs on the same machine under the same load",
}


@dataclass(frozen=True)
class Workload:
    """One workload: the load generator's workload plus whether it pipelines."""

    loadgen: str
    pipelined: bool
    description: str


WORKLOADS = {
    "get": Workload("get", False, "GET of a random key from a pre-written keyspace, one request per round trip"),
    "set": Workload("set", False, "SET of a random key, one request per round trip"),
    "list": Workload("list", False, "RPUSH then LPOP alternating on one list per client, one request per round trip"),
    "set-pipelined": Workload("set", True, "SET of a random key, --pipeline requests per round trip"),
}

METRICS = {
    "throughput_ops_s": lambda r: r["throughput_ops_s"],
    "p50_us": lambda r: r["latency_us"]["p50"],
    "p95_us": lambda r: r["latency_us"]["p95"],
    "p99_us": lambda r: r["latency_us"]["p99"],
    "rss_peak_bytes": lambda r: (r.get("rss_bytes") or {}).get("peak"),
    "rss_end_bytes": lambda r: (r.get("rss_bytes") or {}).get("end"),
}


class CellError(RuntimeError):
    """One cell could not be measured; the report records why."""


def csv_choice(allowed: tuple[str, ...] | list[str]):
    def parse(text: str) -> list[str]:
        items = [item.strip() for item in text.split(",") if item.strip()]
        unknown = [item for item in items if item not in allowed]
        if not items or unknown:
            raise argparse.ArgumentTypeError(f"choose from {', '.join(allowed)} (got {text!r})")
        return items

    return parse


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    p = argparse.ArgumentParser(description=__doc__.split("\n\n", 1)[0])
    p.add_argument("--runs", type=int, default=5, help="repetitions of the whole matrix (default 5)")
    p.add_argument("--durability", type=csv_choice(DURABILITY_MODES), default=list(DURABILITY_MODES),
                   help="comma-separated subset of off,never,periodic,always (default all)")
    p.add_argument("--protocols", type=csv_choice(PROTOCOLS), default=list(PROTOCOLS))
    p.add_argument("--workloads", type=csv_choice(list(WORKLOADS)), default=list(WORKLOADS))
    p.add_argument("--clients", type=int, default=4, help="concurrent connections (default 4)")
    p.add_argument("--pipeline", type=int, default=16, help="depth of the pipelined workload (default 16)")
    p.add_argument("--duration", type=float, default=5.0, help="measured seconds per cell (default 5)")
    p.add_argument("--warmup", type=float, default=1.0, help="unmeasured seconds before each cell (default 1)")
    p.add_argument("--keyspace", type=int, default=100_000, help="distinct keys for get/set (default 100000)")
    p.add_argument("--value-size", type=int, default=64, help="bytes per value (default 64)")
    p.add_argument("--server-bin", type=Path, help="measure this synap-server binary instead of building one")
    p.add_argument("--server-git-sha", help="git sha the --server-bin binary was built from")
    p.add_argument("--server-profile", help="cargo profile of --server-bin (e.g. release)")
    p.add_argument("--server-toolchain",
                   help="rustc that built --server-bin (`rustc -vV` release line); "
                        "without it the rustc commit is read from the binary")
    p.add_argument("--loadgen-bin", type=Path, help="prebuilt bench_loadgen binary")
    p.add_argument("--no-build", action="store_true", help="use target/release binaries without running cargo")
    p.add_argument("--runner-class", choices=sorted(RUNNER_CLASSES), default="local",
                   help="where this runs; only 'controlled' marks the numbers comparable (default local)")
    p.add_argument("--out-dir", type=Path, default=REPO / "target" / "perf-baseline")
    p.add_argument("--merge", type=Path, nargs="+", metavar="REPORT",
                   help="measure nothing: combine JSON reports of one server binary into one report, "
                        "runs renumbered in start order (to interleave two binaries, see the README)")
    args = p.parse_args(argv)
    if args.runs < 1 or args.clients < 1 or args.pipeline < 1 or args.keyspace < 1:
        p.error("--runs, --clients, --pipeline and --keyspace must be at least 1")
    if not (math.isfinite(args.duration) and math.isfinite(args.warmup)) or args.duration <= 0 or args.warmup < 0:
        p.error("--duration must be positive and --warmup not negative, both finite")
    return args


# ---------------------------------------------------------------------------
# Build and environment
# ---------------------------------------------------------------------------
def capture(cmd: list[str], cwd: Path = REPO) -> str:
    done = subprocess.run(cmd, cwd=cwd, capture_output=True, text=True, check=False)
    if done.returncode != 0:
        raise SystemExit(f"`{' '.join(cmd)}` failed:\n{done.stderr.strip()}")
    return done.stdout.strip()


def build_binaries(args: argparse.Namespace) -> tuple[Path, Path]:
    """Build what was not supplied, always with the committed Cargo.lock."""
    release = REPO / "target" / "release"
    server = args.server_bin or release / f"synap-server{EXE}"
    loadgen = args.loadgen_bin or release / "examples" / f"bench_loadgen{EXE}"
    targets = [] if args.server_bin else ["--bin", "synap-server"]
    targets += [] if args.loadgen_bin else ["--example", "bench_loadgen"]
    if targets and not args.no_build:
        cmd = ["cargo", "build", "--release", "--locked", "--package", "synap-server", *targets]
        print("+ " + " ".join(cmd), flush=True)
        if subprocess.run(cmd, cwd=REPO, check=False).returncode != 0:
            raise SystemExit("cargo build failed")
    for path in (server, loadgen):
        if not path.is_file():
            raise SystemExit(f"{path} does not exist (build it or drop --no-build)")
    return server.resolve(), loadgen.resolve()


def git_state() -> dict:
    sha = capture(["git", "rev-parse", "HEAD"])
    dirty = capture(["git", "status", "--porcelain", "--untracked-files=no"]) != ""
    return {"git_sha": sha, "git_dirty": dirty}


def toolchain() -> str:
    """`rustc -vV` of this checkout (rust-toolchain.toml pins it)."""
    fields = dict(
        line.split(": ", 1) for line in capture(["rustc", "-vV"]).splitlines()[1:] if ": " in line
    )
    release = capture(["rustc", "--version"])
    return f"{release} host={fields.get('host', 'unknown')} llvm={fields.get('LLVM version', 'unknown')}"


def embedded_rustc(binary: Path) -> str | None:
    """The rustc commit a binary was built with, read from its std source paths.

    rustc remaps the standard library's paths to `/rustc/<commit>/` in every
    binary it builds (panic locations keep them), so the compiler of a binary
    whose build was not recorded can still be identified.
    """
    commits = {m.group(1).decode() for m in re.finditer(rb"/rustc/([0-9a-f]{40})/", binary.read_bytes())}
    return f"rustc commit {commits.pop()} (read from the binary)" if len(commits) == 1 else None


def build_info(args: argparse.Namespace, server: Path, loadgen: Path, version: str | None) -> dict:
    checkout = {**git_state(), "toolchain": toolchain()}
    if args.server_bin:
        server_build = {
            "source": "external binary (--server-bin)",
            "git_sha": args.server_git_sha or "unknown",
            "git_dirty": None,
            "profile": args.server_profile or "unknown",
            "toolchain": args.server_toolchain or embedded_rustc(server) or "unknown",
        }
    else:
        server_build = {
            "source": "this checkout" + ("" if not args.no_build else " (not rebuilt: --no-build)"),
            "git_sha": checkout["git_sha"],
            "git_dirty": checkout["git_dirty"],
            "profile": "release",
            "toolchain": checkout["toolchain"],
        }
    return {
        "server": {"binary": str(server), "version": version or "unknown", **server_build},
        "loadgen": {"binary": str(loadgen), "profile": "release", **checkout},
    }


def hardware(loadgen: Path) -> dict:
    info = json.loads(capture([str(loadgen), "host-info"]))
    info["python"] = sys.version.split()[0]
    return info


# ---------------------------------------------------------------------------
# Server lifecycle
# ---------------------------------------------------------------------------
def free_port() -> int:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as s:
        s.bind(("127.0.0.1", 0))
        return s.getsockname()[1]


def persistence_block(mode: str, workdir: Path, template: dict) -> dict:
    """The template's persistence settings with this mode's WAL and fsync."""
    enabled = mode != "off"
    wal = {**template["wal"], "enabled": enabled, "path": str(workdir / "wal" / "synap.wal")}
    if enabled:
        wal["fsync_mode"] = mode
    snapshot = {**template["snapshot"], "enabled": enabled, "directory": str(workdir / "snapshots")}
    return {"enabled": enabled, "wal": wal, "snapshot": snapshot}


def server_config(mode: str, workdir: Path, ports: dict[str, int]) -> dict:
    cfg = yaml.safe_load(BASE_CONFIG.read_text(encoding="utf-8"))
    cfg["server"].update(host="127.0.0.1", port=ports["http"])
    cfg["resp3"].update(enabled=True, host="127.0.0.1", port=ports["resp3"])
    cfg["synap_rpc"].update(enabled=True, host="127.0.0.1", port=ports["synaprpc"])
    cfg["logging"]["level"] = "warn"
    cfg["persistence"] = persistence_block(mode, workdir, cfg["persistence"])
    return cfg


def durability_description(mode: str) -> dict:
    """The persistence block a mode runs with, scratch paths elided."""
    template = yaml.safe_load(BASE_CONFIG.read_text(encoding="utf-8"))["persistence"]
    block = persistence_block(mode, Path("<scratch>"), template)
    block["wal"]["path"] = "<scratch>/wal/synap.wal"
    block["snapshot"]["directory"] = "<scratch>/snapshots"
    return block


class Server:
    """A synap-server in a scratch directory, stopped and removed on exit.

    Both binary listeners are configured, but readiness waits only for the HTTP
    port (it serves the version) and the listener under test, so a binary that
    lacks the other protocol can still be measured on this one.
    """

    def __init__(self, binary: Path, mode: str, protocol: str) -> None:
        self.binary = binary
        self.mode = mode
        self.protocol = protocol
        self.ports = {"http": free_port(), "resp3": free_port(), "synaprpc": free_port()}
        self._dir = tempfile.TemporaryDirectory(prefix="synap-perf-", ignore_cleanup_errors=True)
        self.workdir = Path(self._dir.name)
        self.log = self.workdir / "server.log"
        self.proc: subprocess.Popen | None = None

    def __enter__(self) -> "Server":
        config = self.workdir / "config.yml"
        config.write_text(yaml.safe_dump(server_config(self.mode, self.workdir, self.ports)), encoding="utf-8")
        with open(self.log, "wb") as log:
            self.proc = subprocess.Popen(
                [str(self.binary), "--config", str(config)],
                cwd=self.workdir, stdout=log, stderr=subprocess.STDOUT,
            )
        try:
            self._wait_ready(timeout=60.0)
        except BaseException:
            # `with` only calls __exit__ after a successful __enter__.
            self.__exit__(None, None, None)
            raise
        return self

    def _wait_ready(self, timeout: float) -> None:
        deadline = time.monotonic() + timeout
        for name in ("http", self.protocol):
            port = self.ports[name]
            while True:
                self.check_alive()
                try:
                    socket.create_connection(("127.0.0.1", port), timeout=1.0).close()
                    break
                except OSError:
                    if time.monotonic() > deadline:
                        raise CellError(f"server did not open its {name} port {port}: {self.log_tail()}")
                    time.sleep(0.1)

    def check_alive(self) -> None:
        if self.proc is not None and self.proc.poll() is not None:
            raise CellError(f"server exited with status {self.proc.returncode}: {self.log_tail()}")

    def log_tail(self, lines: int = 20) -> str:
        try:
            text = self.log.read_text(encoding="utf-8", errors="replace")
        except OSError:
            return "(no server log)"
        return "\n".join(text.splitlines()[-lines:]) or "(empty server log)"

    def __exit__(self, *exc: object) -> None:
        if self.proc is not None and self.proc.poll() is None:
            self.proc.terminate()
            try:
                self.proc.wait(timeout=15)
            except subprocess.TimeoutExpired:
                self.proc.kill()
                self.proc.wait()
        self._dir.cleanup()


def server_version(port: int) -> str | None:
    """The package version the server reports on `GET /health`."""
    try:
        with urllib.request.urlopen(f"http://127.0.0.1:{port}/health", timeout=5.0) as reply:
            return json.loads(reply.read()).get("version")
    except urllib.error.HTTPError as exc:  # 503 still carries the version
        try:
            return json.loads(exc.read()).get("version")
        except ValueError:
            return None
    except (OSError, ValueError):
        return None


# ---------------------------------------------------------------------------
# Measurement
# ---------------------------------------------------------------------------
def run_cell(args: argparse.Namespace, server: Server, loadgen: Path, protocol: str, workload: str) -> dict:
    spec = WORKLOADS[workload]
    cmd = [
        str(loadgen), "run",
        "--protocol", protocol,
        "--addr", f"127.0.0.1:{server.ports[protocol]}",
        "--workload", spec.loadgen,
        "--clients", str(args.clients),
        "--pipeline", str(args.pipeline if spec.pipelined else 1),
        "--duration-secs", str(args.duration),
        "--warmup-secs", str(args.warmup),
        "--keyspace", str(args.keyspace),
        "--value-size", str(args.value_size),
        "--server-pid", str(server.proc.pid),
    ]
    try:
        done = subprocess.run(cmd, capture_output=True, text=True, timeout=args.duration + args.warmup + 900)
    except subprocess.TimeoutExpired as exc:
        raise CellError(f"load generator timed out after {exc.timeout:.0f}s") from exc
    server.check_alive()
    if done.returncode != 0:
        raise CellError(f"load generator failed: {done.stderr.strip()}")
    try:
        result = json.loads(done.stdout)
    except ValueError as exc:
        raise CellError(f"load generator printed no JSON report: {done.stdout[:200]!r}") from exc
    if result["errors"]:
        raise CellError(f"{result['errors']} error replies out of {result['ops']} requests")
    return result


def measure(args: argparse.Namespace, server_bin: Path, loadgen: Path) -> tuple[list[dict], str | None]:
    """Every (run, durability, protocol, workload) cell, interleaved by run."""
    rows: list[dict] = []
    version: str | None = None
    matrix = itertools.product(range(1, args.runs + 1), args.durability, args.protocols, args.workloads)
    for run, mode, protocol, workload in matrix:
        row = {"run": run, "durability": mode, "protocol": protocol, "workload": workload}
        label = f"run {run}/{args.runs} {mode:<8} {protocol:<8} {workload:<13}"
        try:
            with Server(server_bin, mode, protocol) as server:
                version = version or server_version(server.ports["http"])
                row["result"] = run_cell(args, server, loadgen, protocol, workload)
            r = row["result"]
            print(f"{label} {r['throughput_ops_s']:>12,.0f} ops/s  p50 {r['latency_us']['p50']:>8.1f} us"
                  f"  p99 {r['latency_us']['p99']:>8.1f} us", flush=True)
        except CellError as exc:
            row["error"] = str(exc)
            print(f"{label} FAILED: {exc}", flush=True)
        rows.append(row)
    return rows, version


def spread(values: list[float]) -> dict | None:
    if not values:
        return None
    median = statistics.median(values)
    low, high = min(values), max(values)
    return {
        "median": round(median, 2),
        "min": round(low, 2),
        "max": round(high, 2),
        "spread_pct": round((high - low) / median * 100, 2) if median else None,
        "stdev": round(statistics.stdev(values), 2) if len(values) > 1 else 0.0,
        "n": len(values),
    }


def aggregate(args: argparse.Namespace, rows: list[dict]) -> list[dict]:
    cells = []
    for mode in args.durability:
        for protocol in args.protocols:
            for workload in args.workloads:
                mine = [r for r in rows if (r["durability"], r["protocol"], r["workload"]) == (mode, protocol, workload)]
                ok = [r["result"] for r in mine if "result" in r]
                cells.append({
                    "durability": mode,
                    "protocol": protocol,
                    "workload": workload,
                    "pipeline": args.pipeline if WORKLOADS[workload].pipelined else 1,
                    "clients": args.clients,
                    "failed_runs": [{"run": r["run"], "error": r["error"]} for r in mine if "error" in r],
                    "summary": {
                        name: spread([v for v in (get(r) for r in ok) if v is not None])
                        for name, get in METRICS.items()
                    },
                    "runs": [{"run": r["run"], **r["result"]} for r in mine if "result" in r],
                })
    return cells


# ---------------------------------------------------------------------------
# Merge
# ---------------------------------------------------------------------------
# What must be identical for reports to merge: one binary, measured the same
# way, on the same machine, over the same matrix.
MERGE_KEYS = ("schema", "runner", "build", "hardware", "durability_modes", "workloads")


def cell_keys(report: dict) -> list[tuple[str, str, str]]:
    return [(c["durability"], c["protocol"], c["workload"]) for c in report["results"]]


def merge_reports(paths: list[Path]) -> dict:
    """One report from several reports of the same server binary.

    Comparing two binaries fairly means alternating them, so a drift of the
    machine lands on both: run each with ``--runs 1`` in turn, then merge each
    binary's reports. The runs are renumbered in the order the reports
    started, and the cells are aggregated exactly as one ``--runs N``
    measurement would be.
    """
    loaded = [(path, json.loads(path.read_text(encoding="utf-8"))) for path in paths]
    loaded.sort(key=lambda item: item[1]["started_at"])
    first = loaded[0][1]
    if first.get("schema") != SCHEMA:
        raise SystemExit(f"{loaded[0][0]}: schema {first.get('schema')!r}, expected {SCHEMA!r}")
    if any("merged_from" in report for _, report in loaded):
        raise SystemExit("merge the measured reports themselves, not a merged report")
    same_parameters = {k: v for k, v in first["parameters"].items() if k != "runs"}
    for path, report in loaded[1:]:
        for key in MERGE_KEYS:
            if report.get(key) != first.get(key):
                raise SystemExit(f"{path}: `{key}` differs from {loaded[0][0]}; "
                                 "only reports of one binary measured the same way merge")
        if {k: v for k, v in report["parameters"].items() if k != "runs"} != same_parameters:
            raise SystemExit(f"{path}: `parameters` differ from {loaded[0][0]}")
        if cell_keys(report) != cell_keys(first):
            raise SystemExit(f"{path}: the matrix of cells differs from {loaded[0][0]}")

    rows: list[dict] = []
    offset = 0
    for _, report in loaded:
        for cell in report["results"]:
            key = {"durability": cell["durability"], "protocol": cell["protocol"], "workload": cell["workload"]}
            for run in cell["runs"]:
                result = {name: value for name, value in run.items() if name != "run"}
                rows.append({"run": offset + run["run"], **key, "result": result})
            for failed in cell["failed_runs"]:
                rows.append({"run": offset + failed["run"], **key, "error": failed["error"]})
        offset += report["parameters"]["runs"]

    cells = cell_keys(first)
    matrix = argparse.Namespace(
        durability=list(dict.fromkeys(mode for mode, _, _ in cells)),
        protocols=list(dict.fromkeys(protocol for _, protocol, _ in cells)),
        workloads=list(dict.fromkeys(workload for _, _, workload in cells)),
        pipeline=first["parameters"]["pipeline"],
        clients=first["parameters"]["clients"],
    )
    return {
        "schema": SCHEMA,
        "started_at": first["started_at"],
        "finished_at": loaded[-1][1]["finished_at"],
        "runner": first["runner"],
        "build": first["build"],
        "hardware": first["hardware"],
        "parameters": {
            **first["parameters"],
            "runs": offset,
            "order": f"{len(loaded)} reports merged in start order, so run k is the k-th one measured; "
                     f"within each report: {first['parameters']['order']}",
        },
        "merged_from": [{"started_at": report["started_at"], "runs": report["parameters"]["runs"]}
                        for _, report in loaded],
        "durability_modes": first["durability_modes"],
        "workloads": first["workloads"],
        "failures": sum(1 for row in rows if "error" in row),
        "results": aggregate(matrix, rows),
    }


# ---------------------------------------------------------------------------
# Report
# ---------------------------------------------------------------------------
def mib(value: float | None) -> str:
    return "-" if value is None else f"{value / 1048576:.1f} MiB"


def render_summary(report: dict) -> str:
    server, hw, p = report["build"]["server"], report["hardware"], report["parameters"]
    runner = report["runner"]
    lines = [
        f"# Synap performance baseline ({report['started_at']})",
        "",
        f"- Server: {server['version']} ({server['git_sha'][:12]}{' dirty' if server['git_dirty'] else ''}, "
        f"{server['profile']}, {server['toolchain']})",
        f"- Hardware: {hw['cpu_model']}, {hw['physical_cores']} physical / {hw['logical_cores']} logical cores, "
        f"{hw['ram_bytes'] / 1073741824:.1f} GiB RAM, {hw['os']}",
        f"- Runner: {runner['class']} - {runner['note']}",
        f"- {p['runs']} run(s); {p['clients']} clients; {p['duration_s']} s measured after {p['warmup_s']} s warmup; "
        f"keyspace {p['keyspace']}; {p['value_size']}-byte values; pipelined depth {p['pipeline']}",
        *([f"- Merged from {len(report['merged_from'])} reports; run k is the k-th one measured"]
          if "merged_from" in report else []),
        "- Medians across runs; the spread is (max - min) / median. RSS is the server's, sampled.",
        "",
        "| durability | protocol | workload | ops/s (spread) | p50 us | p95 us | p99 us | peak RSS |",
        "|---|---|---|---|---|---|---|---|",
    ]
    for cell in report["results"]:
        s = cell["summary"]
        if s["throughput_ops_s"] is None:
            lines.append(f"| {cell['durability']} | {cell['protocol']} | {cell['workload']} | FAILED | | | | |")
            continue
        tp = s["throughput_ops_s"]
        tp_spread = "-" if tp["spread_pct"] is None else f"{tp['spread_pct']:.1f}%"
        lines.append(
            f"| {cell['durability']} | {cell['protocol']} | {cell['workload']} | "
            f"{tp['median']:,.0f} ({tp_spread}) | {s['p50_us']['median']:.1f} | "
            f"{s['p95_us']['median']:.1f} | {s['p99_us']['median']:.1f} | "
            f"{mib(s['rss_peak_bytes']['median'] if s['rss_peak_bytes'] else None)} |"
        )
    if report["failures"]:
        lines += ["", f"{report['failures']} cell run(s) FAILED; see `failed_runs` in the JSON report."]
    return "\n".join(lines) + "\n"


def measured_report(args: argparse.Namespace) -> dict:
    started = dt.datetime.now(dt.timezone.utc)
    server_bin, loadgen = build_binaries(args)
    rows, version = measure(args, server_bin, loadgen)
    cells = aggregate(args, rows)
    return {
        "schema": SCHEMA,
        "started_at": started.isoformat(timespec="seconds"),
        "finished_at": dt.datetime.now(dt.timezone.utc).isoformat(timespec="seconds"),
        "runner": {"class": args.runner_class, "comparable": args.runner_class == "controlled",
                   "note": RUNNER_CLASSES[args.runner_class]},
        "build": build_info(args, server_bin, loadgen, version),
        "hardware": hardware(loadgen),
        "parameters": {
            "runs": args.runs, "clients": args.clients, "pipeline": args.pipeline,
            "duration_s": args.duration, "warmup_s": args.warmup,
            "keyspace": args.keyspace, "value_size": args.value_size,
            "order": "runs interleaved: every cell of run 1, then of run 2, ...; a fresh server per cell",
        },
        "durability_modes": {mode: durability_description(mode) for mode in args.durability},
        "workloads": {name: WORKLOADS[name].description for name in args.workloads},
        "failures": sum(1 for r in rows if "error" in r),
        "results": cells,
    }


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    report = merge_reports(args.merge) if args.merge else measured_report(args)
    started = dt.datetime.fromisoformat(report["started_at"])
    # A merged report starts when its earliest source did; the suffix keeps it
    # from overwriting that source in the same directory.
    suffix = "-merged" if args.merge else ""
    args.out_dir.mkdir(parents=True, exist_ok=True)
    stem = args.out_dir / f"perf-baseline-{started.strftime('%Y%m%dT%H%M%SZ')}{suffix}"
    summary = render_summary(report)
    stem.with_suffix(".json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    stem.with_suffix(".md").write_text(summary, encoding="utf-8")
    print("\n" + summary)
    print(f"JSON report: {stem.with_suffix('.json')}\nSummary:     {stem.with_suffix('.md')}")
    return 1 if report["failures"] else 0


if __name__ == "__main__":
    sys.exit(main())
