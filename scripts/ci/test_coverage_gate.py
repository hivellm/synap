#!/usr/bin/env python3
"""Tests for scripts/ci/coverage_gate.py.

Each test writes reports and a floors file into a scratch directory and points
the gate at it, so the real .github/coverage-floors.json is never touched.

Usage: python scripts/ci/test_coverage_gate.py
"""

from __future__ import annotations

import contextlib
import importlib.util
import io
import json
import sys
import tempfile
import unittest
from pathlib import Path

GATE_PATH = Path(__file__).resolve().parent / "coverage_gate.py"
spec = importlib.util.spec_from_file_location("coverage_gate", GATE_PATH)
gate = importlib.util.module_from_spec(spec)
assert spec.loader is not None
# Registered first: dataclasses resolve string annotations through sys.modules.
sys.modules["coverage_gate"] = gate
spec.loader.exec_module(gate)

# Two crates in one LCOV report, the shape `cargo llvm-cov --lcov` writes:
# absolute SF paths, LF/LH totals per file.
LCOV = """TN:
SF:/home/runner/work/synap/synap/crates/synap-core/src/lib.rs
DA:1,1
LF:80
LH:60
end_of_record
SF:/home/runner/work/synap/synap/crates/synap-core/src/kv.rs
LF:20
LH:20
end_of_record
SF:/home/runner/work/synap/synap/crates/synap-server/src/main.rs
LF:100
LH:25
end_of_record
"""

COBERTURA = """<?xml version="1.0"?>
<coverage line-rate="0.5" lines-covered="3" lines-valid="6" version="1.9">
  <packages><package name="sdk"><classes>
    <class name="A" filename="src/Sdk/A.cs"><lines>
      <line number="1" hits="1"/><line number="2" hits="0"/><line number="3" hits="2"/>
    </lines></class>
    <class name="B" filename="src/Other/B.cs"><lines>
      <line number="1" hits="0"/><line number="2" hits="1"/><line number="3" hits="0"/>
    </lines></class>
  </classes></package></packages>
</coverage>
"""


class GateTest(unittest.TestCase):
    def setUp(self) -> None:
        self._tmp = tempfile.TemporaryDirectory()
        self.root = Path(self._tmp.name)
        (self.root / "lcov.info").write_text(LCOV, encoding="utf-8")
        (self.root / "cs").mkdir()
        (self.root / "cs" / "coverage.cobertura.xml").write_text(COBERTURA, encoding="utf-8")
        self._saved = (gate.REPO, gate.FLOORS)
        gate.REPO = self.root
        gate.FLOORS = self.root / "floors.json"

    def tearDown(self) -> None:
        gate.REPO, gate.FLOORS = self._saved
        self._tmp.cleanup()

    def floors(self, **components: dict) -> None:
        gate.FLOORS.write_text(json.dumps({"policy": 95.0, "components": components}), encoding="utf-8")

    def run_gate(self, *argv: str) -> tuple[int, str]:
        out, err = io.StringIO(), io.StringIO()
        saved_argv = sys.argv
        sys.argv = ["coverage_gate.py", *argv]
        try:
            with contextlib.redirect_stdout(out), contextlib.redirect_stderr(err):
                code = gate.main()
        finally:
            sys.argv = saved_argv
        return code, out.getvalue() + err.getvalue()

    def test_lcov_is_split_by_component_prefix(self) -> None:
        core = gate.measure_lcov(self.root / "lcov.info", "crates/synap-core")
        server = gate.measure_lcov(self.root / "lcov.info", "crates/synap-server")
        self.assertEqual((core.covered, core.total), (80, 100))
        self.assertEqual((server.covered, server.total), (25, 100))
        # A prefix only matches whole path segments.
        self.assertEqual(gate.measure_lcov(self.root / "lcov.info", "crates/synap").total, 0)

    def test_component_at_its_floor_passes_and_reports_the_policy_gap(self) -> None:
        self.floors(core={"report": "lcov.info", "include": "crates/synap-core", "floor": 80.0, "mode": "unit"})
        code, out = self.run_gate("core")
        self.assertEqual(code, 0, out)
        self.assertIn("15.00 points short of the 95% policy", out)

    def test_drop_below_the_floor_fails_naming_the_component(self) -> None:
        self.floors(
            core={"report": "lcov.info", "include": "crates/synap-core", "floor": 80.0, "mode": "unit"},
            server={"report": "lcov.info", "include": "crates/synap-server", "floor": 25.1, "mode": "unit"},
        )
        code, out = self.run_gate("core", "server")
        self.assertEqual(code, 1, out)
        self.assertIn("coverage gate FAILED for: server", out)
        self.assertIn("server line coverage is 25.00%, below its floor of 25.10%", out)
        self.assertNotIn("FAILED for: core", out)

    def test_missing_report_fails(self) -> None:
        self.floors(ts={"report": "sdks/typescript/coverage/lcov.info", "floor": 1.0, "mode": "unit"})
        code, out = self.run_gate("ts")
        self.assertEqual(code, 1, out)
        self.assertIn("no report matches", out)

    def test_report_without_lines_for_the_component_fails(self) -> None:
        self.floors(cli={"report": "lcov.info", "include": "crates/synap-cli", "floor": 1.0, "mode": "unit"})
        code, out = self.run_gate("cli")
        self.assertEqual(code, 1, out)
        self.assertIn("has no lines under crates/synap-cli", out)

    def test_cobertura_totals_and_glob(self) -> None:
        self.floors(cs={"report": "cs/*.xml", "floor": 50.0, "mode": "unit"})
        code, out = self.run_gate("cs")
        self.assertEqual(code, 0, out)
        self.assertIn("3/6", out)
        filtered = gate.measure_cobertura(self.root / "cs" / "coverage.cobertura.xml", "src/Sdk")
        self.assertEqual((filtered.covered, filtered.total), (2, 3))

    def test_unknown_component_fails(self) -> None:
        self.floors(core={"report": "lcov.info", "floor": 1.0, "mode": "unit"})
        code, out = self.run_gate("nope")
        self.assertEqual(code, 1, out)
        self.assertIn("unknown component(s): nope", out)

    def test_checked_in_floors_name_existing_reports_and_sane_values(self) -> None:
        config = json.loads(self._saved[1].read_text(encoding="utf-8"))
        self.assertEqual(config["policy"], 95.0)
        for name, component in config["components"].items():
            with self.subTest(component=name):
                self.assertTrue(0.0 < float(component["floor"]) <= 100.0)
                self.assertTrue(component["report"])
                self.assertTrue(component["mode"])


if __name__ == "__main__":
    unittest.main(verbosity=2)
