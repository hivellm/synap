#!/usr/bin/env python3
"""Tests for scripts/bench/perf_baseline.py.

They cover the parts that do not need a server: the statistics, the
aggregation of runs into cells, merging reports, the summary, the per-mode
persistence settings, argument checks and reading the rustc commit out of a
binary. The measurement itself (server lifecycle, load generator) is exercised
by running the script; see scripts/bench/README.md.

Usage: python scripts/bench/test_perf_baseline.py   (needs PyYAML)
"""

from __future__ import annotations

import argparse
import contextlib
import importlib.util
import io
import json
import sys
import tempfile
import unittest
from pathlib import Path

SCRIPT = Path(__file__).resolve().parent / "perf_baseline.py"
spec = importlib.util.spec_from_file_location("perf_baseline", SCRIPT)
pb = importlib.util.module_from_spec(spec)
assert spec.loader is not None
# Registered first: dataclasses resolve string annotations through sys.modules.
sys.modules["perf_baseline"] = pb
spec.loader.exec_module(pb)

MATRIX = argparse.Namespace(
    durability=["off", "always"], protocols=["resp3"], workloads=["set", "set-pipelined"],
    pipeline=16, clients=4,
)


def result(throughput: float, p50: float = 10.0, rss: int = 1 << 20) -> dict:
    """One load generator result, the shape `bench_loadgen run` prints."""
    return {
        "ops": int(throughput * 5), "errors": 0, "elapsed_s": 5.0,
        "throughput_ops_s": throughput,
        "latency_us": {"p50": p50, "p95": p50 * 2, "p99": p50 * 3, "p999": p50 * 4, "max": p50 * 5,
                       "mean": p50},
        "rss_bytes": {"start": rss, "end": rss, "peak": rss, "samples": 100},
    }


def report(started_at: str, throughputs: list[float], *, sha: str = "a" * 40,
           failed: dict[int, str] | None = None) -> dict:
    """A measured report whose every cell ran `throughputs[i]` in run i + 1.

    `failed` maps a run number to an error that cell (off, resp3, set) hit
    instead of a result in that run.
    """
    rows = []
    for run, throughput in enumerate(throughputs, 1):
        for mode in MATRIX.durability:
            for protocol in MATRIX.protocols:
                for workload in MATRIX.workloads:
                    row = {"run": run, "durability": mode, "protocol": protocol, "workload": workload}
                    if (failed or {}).get(run) and (mode, workload) == ("off", "set"):
                        row["error"] = failed[run]
                    else:
                        row["result"] = result(throughput)
                    rows.append(row)
    return {
        "schema": pb.SCHEMA,
        "started_at": started_at,
        "finished_at": started_at,
        "runner": {"class": "local", "comparable": False, "note": pb.RUNNER_CLASSES["local"]},
        "build": {
            "server": {"binary": "/bin/synap-server", "version": "1.4.0", "source": "this checkout",
                       "git_sha": sha, "git_dirty": False, "profile": "release", "toolchain": "rustc test"},
            "loadgen": {"binary": "/bin/bench_loadgen", "profile": "release", "git_sha": sha,
                        "git_dirty": False, "toolchain": "rustc test"},
        },
        "hardware": {"cpu_model": "test cpu", "physical_cores": 4, "logical_cores": 8,
                     "ram_bytes": 16 << 30, "os": "test os", "kernel": "1", "arch": "x86_64", "python": "3"},
        "parameters": {"runs": len(throughputs), "clients": 4, "pipeline": 16, "duration_s": 5.0,
                       "warmup_s": 1.0, "keyspace": 100_000, "value_size": 64, "order": "interleaved"},
        "durability_modes": {mode: {"enabled": mode != "off"} for mode in MATRIX.durability},
        "workloads": {name: pb.WORKLOADS[name].description for name in MATRIX.workloads},
        "failures": sum(1 for row in rows if "error" in row),
        "results": pb.aggregate(MATRIX, rows),
    }


def cell(rep: dict, mode: str, workload: str) -> dict:
    return next(c for c in rep["results"] if (c["durability"], c["workload"]) == (mode, workload))


class StatisticsTests(unittest.TestCase):
    def test_spread_reports_median_range_and_stdev(self) -> None:
        s = pb.spread([100.0, 300.0, 200.0])
        self.assertEqual((s["median"], s["min"], s["max"], s["n"]), (200.0, 100.0, 300.0, 3))
        self.assertEqual(s["spread_pct"], 100.0)
        self.assertEqual(s["stdev"], 100.0)

    def test_spread_edge_cases(self) -> None:
        self.assertIsNone(pb.spread([]))
        self.assertEqual(pb.spread([5.0])["stdev"], 0.0)
        self.assertIsNone(pb.spread([0.0, 0.0])["spread_pct"])

    def test_aggregate_keeps_failed_runs_out_of_the_summary(self) -> None:
        rep = report("2026-01-01T00:00:00+00:00", [100.0, 200.0, 300.0], failed={2: "server exited"})
        broken = cell(rep, "off", "set")
        self.assertEqual(broken["failed_runs"], [{"run": 2, "error": "server exited"}])
        self.assertEqual([r["run"] for r in broken["runs"]], [1, 3])
        self.assertEqual(broken["summary"]["throughput_ops_s"]["median"], 200.0)
        self.assertEqual(broken["summary"]["throughput_ops_s"]["n"], 2)
        whole = cell(rep, "always", "set-pipelined")
        self.assertEqual((whole["pipeline"], whole["clients"]), (16, 4))
        self.assertEqual(whole["summary"]["throughput_ops_s"]["n"], 3)
        self.assertEqual(cell(rep, "off", "set")["pipeline"], 1)


class MergeTests(unittest.TestCase):
    def setUp(self) -> None:
        self._dir = tempfile.TemporaryDirectory()
        self.root = Path(self._dir.name)

    def tearDown(self) -> None:
        self._dir.cleanup()

    def write(self, name: str, rep: dict) -> Path:
        path = self.root / name
        path.write_text(json.dumps(rep), encoding="utf-8")
        return path

    def test_merge_renumbers_runs_in_start_order_and_aggregates_like_one_measurement(self) -> None:
        late = self.write("late.json", report("2026-01-01T02:00:00+00:00", [300.0]))
        early = self.write("early.json", report("2026-01-01T00:00:00+00:00", [100.0, 200.0],
                                                failed={1: "load generator failed"}))
        merged = pb.merge_reports([late, early])
        self.assertEqual(merged["parameters"]["runs"], 3)
        self.assertEqual(merged["started_at"], "2026-01-01T00:00:00+00:00")
        self.assertEqual([m["runs"] for m in merged["merged_from"]], [2, 1])
        self.assertEqual(merged["failures"], 1)
        target = cell(merged, "off", "set")
        self.assertEqual([r["run"] for r in target["runs"]], [2, 3])
        self.assertEqual(target["failed_runs"], [{"run": 1, "error": "load generator failed"}])
        other = cell(merged, "always", "set")
        self.assertEqual([(r["run"], r["throughput_ops_s"]) for r in other["runs"]],
                         [(1, 100.0), (2, 200.0), (3, 300.0)])
        one_measurement = report("x", [100.0, 200.0, 300.0])
        self.assertEqual(other["summary"], cell(one_measurement, "always", "set")["summary"])

    def test_merge_refuses_reports_of_another_binary_or_setup(self) -> None:
        base = self.write("a.json", report("2026-01-01T00:00:00+00:00", [100.0]))
        other_build = self.write("b.json", report("2026-01-01T01:00:00+00:00", [100.0], sha="b" * 40))
        with self.assertRaisesRegex(SystemExit, "`build` differs"):
            pb.merge_reports([base, other_build])
        changed = report("2026-01-01T01:00:00+00:00", [100.0])
        changed["parameters"]["clients"] = 8
        with self.assertRaisesRegex(SystemExit, "`parameters` differ"):
            pb.merge_reports([base, self.write("c.json", changed)])
        smaller = report("2026-01-01T01:00:00+00:00", [100.0])
        smaller["results"] = smaller["results"][1:]
        with self.assertRaisesRegex(SystemExit, "matrix of cells differs"):
            pb.merge_reports([base, self.write("d.json", smaller)])

    def test_merge_refuses_a_merged_report_and_an_unknown_schema(self) -> None:
        a = self.write("a.json", report("2026-01-01T00:00:00+00:00", [100.0]))
        b = self.write("b.json", report("2026-01-01T01:00:00+00:00", [200.0]))
        merged = self.write("m.json", pb.merge_reports([a, b]))
        with self.assertRaisesRegex(SystemExit, "not a merged report"):
            pb.merge_reports([merged, a])
        old = report("2026-01-01T00:00:00+00:00", [100.0])
        old["schema"] = "synap-perf-baseline/0"
        with self.assertRaisesRegex(SystemExit, "expected"):
            pb.merge_reports([self.write("old.json", old)])

    def test_main_merge_writes_a_separate_merged_report(self) -> None:
        a = self.write("a.json", report("2026-01-01T00:00:00+00:00", [100.0]))
        b = self.write("b.json", report("2026-01-01T01:00:00+00:00", [200.0]))
        out = self.root / "out"
        with contextlib.redirect_stdout(io.StringIO()) as printed:
            code = pb.main(["--merge", str(a), str(b), "--out-dir", str(out)])
        self.assertEqual(code, 0)
        written = json.loads((out / "perf-baseline-20260101T000000Z-merged.json").read_text(encoding="utf-8"))
        self.assertEqual(written["parameters"]["runs"], 2)
        summary = (out / "perf-baseline-20260101T000000Z-merged.md").read_text(encoding="utf-8")
        self.assertIn("Merged from 2 reports", summary)
        self.assertIn("| off | resp3 | set | 150 (66.7%) |", summary)
        self.assertIn(summary, printed.getvalue())


class SummaryAndConfigTests(unittest.TestCase):
    def test_summary_marks_a_cell_with_no_successful_run_failed(self) -> None:
        rep = report("2026-01-01T00:00:00+00:00", [100.0], failed={1: "boom"})
        text = pb.render_summary(rep)
        self.assertIn("| off | resp3 | set | FAILED | | | | |", text)
        self.assertIn("1 cell run(s) FAILED", text)
        self.assertIn("| always | resp3 | set-pipelined | 100 (0.0%) | 10.0 | 20.0 | 30.0 | 1.0 MiB |", text)

    def test_persistence_block_per_durability_mode(self) -> None:
        template = {"wal": {"enabled": False, "fsync_mode": "periodic", "fsync_interval_ms": 10},
                    "snapshot": {"enabled": False, "interval_secs": 300}}
        workdir = Path("scratch")
        off = pb.persistence_block("off", workdir, template)
        self.assertFalse(off["enabled"] or off["wal"]["enabled"] or off["snapshot"]["enabled"])
        always = pb.persistence_block("always", workdir, template)
        self.assertTrue(always["enabled"] and always["wal"]["enabled"] and always["snapshot"]["enabled"])
        self.assertEqual(always["wal"]["fsync_mode"], "always")
        self.assertEqual(always["wal"]["fsync_interval_ms"], 10)
        self.assertEqual(Path(always["wal"]["path"]), workdir / "wal" / "synap.wal")
        self.assertEqual(Path(always["snapshot"]["directory"]), workdir / "snapshots")
        self.assertEqual(template["wal"]["fsync_mode"], "periodic", "the template must not change")

    def test_argument_checks(self) -> None:
        args = pb.parse_args(["--durability", "off,always", "--workloads", "set-pipelined"])
        self.assertEqual((args.durability, args.workloads), (["off", "always"], ["set-pipelined"]))
        for bad in (["--durability", "fast"], ["--runs", "0"], ["--duration", "nan"], ["--protocols", ""]):
            with self.subTest(argv=bad), contextlib.redirect_stderr(io.StringIO()):
                with self.assertRaises(SystemExit):
                    pb.parse_args(bad)

    def test_embedded_rustc_needs_exactly_one_commit(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            binary = Path(tmp) / "server"
            one, two = "1" * 40, "2" * 40
            binary.write_bytes(f"x/rustc/{one}/library/core\0y/rustc/{one}/library/std".encode())
            self.assertEqual(pb.embedded_rustc(binary), f"rustc commit {one} (read from the binary)")
            binary.write_bytes(f"/rustc/{one}/a\0/rustc/{two}/b".encode())
            self.assertIsNone(pb.embedded_rustc(binary))
            binary.write_bytes(b"no compiler paths")
            self.assertIsNone(pb.embedded_rustc(binary))


if __name__ == "__main__":
    unittest.main(verbosity=2)
