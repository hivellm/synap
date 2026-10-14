#!/usr/bin/env python3
"""Fail CI when a component's line coverage drops below its recorded floor.

The floors live in ``.github/coverage-floors.json``. Each component names the
report its CI job writes (LCOV ``.info`` or Cobertura ``.xml``, glob allowed)
and, for a report that covers several components, the path prefix of the
component's source files. The floor is the coverage measured when the gate was
introduced, or a higher value someone raised it to; the 95% policy is the
target, and the gap between the two is printed on every run.

Usage::

    python scripts/ci/coverage_gate.py synap-core synap-server synap-sdk
    python scripts/ci/coverage_gate.py --list

Exit status is 1 when any named component is below its floor or its report is
missing or holds no lines for it; the failing component is named in the output.
"""

from __future__ import annotations

import argparse
import glob
import json
import os
import sys
import xml.etree.ElementTree as ET
from dataclasses import dataclass
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
FLOORS = REPO / ".github" / "coverage-floors.json"


@dataclass
class Measured:
    covered: int
    total: int

    @property
    def percent(self) -> float:
        return 100.0 * self.covered / self.total if self.total else 0.0


def _normalized(path: str) -> str:
    """Forward slashes and a leading slash, so prefixes match whole segments."""
    return "/" + path.strip().replace("\\", "/").lstrip("/")


def _in_component(source: str, include: str | None) -> bool:
    return include is None or f"/{include.strip('/')}/" in _normalized(source)


def measure_lcov(report: Path, include: str | None) -> Measured:
    covered = total = 0
    selected = False
    for line in report.read_text(encoding="utf-8").splitlines():
        if line.startswith("SF:"):
            selected = _in_component(line[3:], include)
        elif selected and line.startswith("LF:"):
            total += int(line[3:])
        elif selected and line.startswith("LH:"):
            covered += int(line[3:])
    return Measured(covered, total)


def measure_cobertura(report: Path, include: str | None) -> Measured:
    root = ET.parse(report).getroot()
    if include is None:
        return Measured(int(root.get("lines-covered", 0)), int(root.get("lines-valid", 0)))
    covered = total = 0
    for cls in root.iter("class"):
        if not _in_component(cls.get("filename", ""), include):
            continue
        for line in cls.iter("line"):
            total += 1
            if int(line.get("hits", 0)) > 0:
                covered += 1
    return Measured(covered, total)


def resolve_report(pattern: str) -> list[Path]:
    matches = sorted(glob.glob(str(REPO / pattern), recursive=True))
    return [Path(m) for m in matches]


def measure(component: str, spec: dict) -> tuple[Measured | None, str]:
    reports = resolve_report(spec["report"])
    if not reports:
        return None, f"no report matches {spec['report']}"
    if len(reports) > 1:
        names = ", ".join(os.path.relpath(r, REPO) for r in reports)
        return None, f"{spec['report']} matches {len(reports)} reports ({names}); expected one"
    report = reports[0]
    include = spec.get("include")
    if report.suffix == ".xml":
        result = measure_cobertura(report, include)
    else:
        result = measure_lcov(report, include)
    if result.total == 0:
        where = f" under {include}" if include else ""
        return None, f"{os.path.relpath(report, REPO)} has no lines{where}"
    return result, os.path.relpath(report, REPO)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("components", nargs="*", help="components to check (default: none)")
    parser.add_argument("--list", action="store_true", help="list components, floors and reports")
    args = parser.parse_args()

    config = json.loads(FLOORS.read_text(encoding="utf-8"))
    policy = float(config["policy"])
    components: dict[str, dict] = config["components"]

    if args.list:
        for name, spec in components.items():
            print(f"{name:<16} floor {spec['floor']:6.2f}%  {spec['report']}  ({spec['mode']})")
        return 0
    if not args.components:
        parser.error("name at least one component, or pass --list")
    unknown = [c for c in args.components if c not in components]
    if unknown:
        print(f"unknown component(s): {', '.join(unknown)}; see {FLOORS.relative_to(REPO)}", file=sys.stderr)
        return 1

    failed: list[str] = []
    print(f"{'component':<16} {'lines':>15} {'measured':>9} {'floor':>7} {'policy':>7}  result")
    for name in args.components:
        spec = components[name]
        floor = float(spec["floor"])
        result, source = measure(name, spec)
        if result is None:
            print(f"{name:<16} {'-':>15} {'-':>9} {floor:6.2f}% {policy:6.2f}%  FAIL: {source}")
            print(f"::error title=coverage {name}::{source}")
            failed.append(name)
            continue
        lines = f"{result.covered}/{result.total}"
        pct = result.percent
        if pct + 1e-9 < floor:
            verdict = f"FAIL: below the floor by {floor - pct:.2f} points"
            print(f"::error title=coverage {name}::{name} line coverage is {pct:.2f}%, below its floor of {floor:.2f}%")
            failed.append(name)
        elif pct + 1e-9 < policy:
            verdict = f"ok, {policy - pct:.2f} points short of the {policy:g}% policy"
        else:
            verdict = "ok, meets the policy"
        print(f"{name:<16} {lines:>15} {pct:8.2f}% {floor:6.2f}% {policy:6.2f}%  {verdict}  [{source}]")
        if pct >= floor + 1.0:
            print(
                f"::notice title=coverage {name}::{name} is at {pct:.2f}%, a point or more above its "
                f"floor of {floor:.2f}%; raise the floor in {FLOORS.relative_to(REPO).as_posix()} to keep the gain"
            )

    if failed:
        print(f"\ncoverage gate FAILED for: {', '.join(failed)}")
        return 1
    print("\ncoverage gate passed")
    return 0


if __name__ == "__main__":
    sys.exit(main())
