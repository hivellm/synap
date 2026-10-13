#!/usr/bin/env python3
"""Structural assertions for the pull-request and release gates.

Guards the wiring docs/development/CI_GATES.md describes, so a later edit
cannot quietly drop a gate:
  - Cargo.lock is tracked, and every cargo invocation in CI, release, SDK
    publishing and the Dockerfile builds with --locked
  - rust-toolchain.toml pins a dated nightly, and every workflow installs it
    through .github/actions/setup-rust instead of naming a toolchain itself
  - pull requests run the SDK, GUI, contract and interop jobs
  - every component with a coverage floor is checked by some workflow step
  - every SDK publish job tests its packaged artifact before publishing

Usage: python scripts/ci/test_ci_gates.py   (needs PyYAML)
"""

from __future__ import annotations

import json
import re
import subprocess
import sys
from pathlib import Path

import yaml

REPO = Path(__file__).resolve().parents[2]
WORKFLOWS = REPO / ".github" / "workflows"

failures: list[str] = []


def check(label: str, ok: bool) -> None:
    print(f"  {'ok' if ok else 'FAIL'}: {label}")
    if not ok:
        failures.append(label)


def load(name: str) -> dict:
    return yaml.safe_load((WORKFLOWS / name).read_text(encoding="utf-8"))


def triggers(doc: dict) -> dict:
    # YAML 1.1 parses the bare key `on` as boolean True.
    return doc.get("on", doc.get(True, {})) or {}


def steps(doc: dict):
    for job_name, job in doc.get("jobs", {}).items():
        for step in job.get("steps", []):
            yield job_name, step


def git(*args: str) -> subprocess.CompletedProcess:
    return subprocess.run(["git", *args], cwd=REPO, capture_output=True, text=True, check=False)


workflow_docs = {path.name: yaml.safe_load(path.read_text(encoding="utf-8")) for path in sorted(WORKFLOWS.glob("*.yml"))}

# --- Lockfile ---------------------------------------------------------------
print("lockfile")
check("Cargo.lock is tracked", git("ls-files", "--error-unmatch", "Cargo.lock").returncode == 0)
check("Cargo.lock is not ignored", git("check-ignore", "-q", "Cargo.lock").returncode == 1)

CARGO = re.compile(r"\bcargo\s+(build|check|test|test-full|clippy|bench|llvm-cov|package|publish|run|deny)\b")
for name, doc in workflow_docs.items():
    for job_name, step in steps(doc):
        for line in str(step.get("run", "")).splitlines():
            match = CARGO.search(line)
            if match and "--locked" not in line:
                check(f"{name}:{job_name} runs `{line.strip()}` with --locked", False)
        if "upload-rust-binary-action" in str(step.get("uses", "")):
            check(
                f"{name}:{job_name} uploads {step['with'].get('bin')} built with locked: true",
                step.get("with", {}).get("locked") is True,
            )
dockerfile = (REPO / "Dockerfile").read_text(encoding="utf-8")
check("Dockerfile builds with --locked", re.search(r"cargo build[^\n]*--locked", dockerfile) is not None)
check("Dockerfile copies Cargo.lock", re.search(r"COPY [^\n]*Cargo\.lock", dockerfile) is not None)

# --- Toolchain --------------------------------------------------------------
print("toolchain")
toolchain = (REPO / "rust-toolchain.toml").read_text(encoding="utf-8")
channel = re.search(r'^channel\s*=\s*"([^"]+)"', toolchain, re.MULTILINE)
check(
    "rust-toolchain.toml pins a dated nightly",
    channel is not None and re.fullmatch(r"nightly-\d{4}-\d{2}-\d{2}", channel.group(1)) is not None,
)
for name in workflow_docs:
    raw = (WORKFLOWS / name).read_text(encoding="utf-8")
    check(f"{name} names no toolchain of its own", "dtolnay/rust-toolchain" not in raw and "+nightly" not in raw)
for name, doc in workflow_docs.items():
    for job_name, job in doc.get("jobs", {}).items():
        runs_cargo = any(CARGO.search(str(step.get("run", ""))) for step in job.get("steps", []))
        builds_rust = runs_cargo or any(
            "upload-rust-binary-action" in str(step.get("uses", "")) for step in job.get("steps", [])
        )
        if builds_rust:
            check(
                f"{name}:{job_name} installs the pinned toolchain",
                any(step.get("uses") == "./.github/actions/setup-rust" for step in job.get("steps", [])),
            )
action = (REPO / ".github" / "actions" / "setup-rust" / "action.yml").read_text(encoding="utf-8")
check("setup-rust reads rust-toolchain.toml", "rust-toolchain.toml" in action)
check("Dockerfile installs the channel from rust-toolchain.toml", "rust-toolchain.toml)" in dockerfile)

# --- Pull-request gates -----------------------------------------------------
print("pull-request gates")
pr = load("sdk-gui-contract.yml")
check("sdk-gui-contract.yml runs on pull requests", "pull_request" in triggers(pr))
jobs = pr.get("jobs", {})
for job in ("sdk-typescript", "sdk-python", "sdk-csharp", "sdk-php", "gui", "contract", "server", "gate-scripts"):
    check(f"job {job} exists", job in jobs)


def runs(job: str) -> str:
    return "\n".join(str(step.get("run", "")) for step in jobs.get(job, {}).get("steps", []))


gui = runs("gui")
for command in ("npm run type-check", "npm run lint", "npm run build", "npm test"):
    check(f"gui runs `{command}`", command in gui)
check("gui lint does not --fix", "--fix" not in json.loads((REPO / "gui" / "package.json").read_text())["scripts"]["lint"])
contract = runs("contract")
check("contract runs the TypeScript E2E suite", "src/__tests__/e2e.test.ts" in contract)
check("contract runs the interop matrix strictly", "run-matrix.py --strict" in contract)
check("contract runs the S2S suites", "npm run test:s2s" in contract and "pytest" in contract)
check("sdk-php checks out the submodule", jobs.get("sdk-php", {}).get("steps", [{}])[0].get("with", {}).get("submodules") is True)
check("sdk-php runs phpunit", "phpunit" in runs("sdk-php"))
e2e = (REPO / "sdks" / "typescript" / "src" / "__tests__" / "e2e.test.ts").read_text(encoding="utf-8")
check("the E2E suite asserts an INCR amount on every transport", "synap.kv.incr(counter, 10)" in e2e)

# --- Coverage ---------------------------------------------------------------
print("coverage")
floors = json.loads((REPO / ".github" / "coverage-floors.json").read_text(encoding="utf-8"))
gated: set[str] = set()
for doc in workflow_docs.values():
    for _, step in steps(doc):
        for line in str(step.get("run", "")).splitlines():
            if "scripts/ci/coverage_gate.py" in line:
                gated.update(line.split("coverage_gate.py", 1)[1].split())
for component in floors["components"]:
    check(f"coverage floor of {component} is enforced by a workflow", component in gated)

# --- Release artifacts --------------------------------------------------------
print("release artifacts")
publish = load("sdk-publish.yml")
for job_name, job in publish["jobs"].items():
    if not job_name.startswith("publish-"):
        continue
    names = [str(step.get("run", "")) + str(step.get("uses", "")) for step in job.get("steps", [])]
    smoke = next((i for i, s in enumerate(names) if "artifact-smoke/run.py" in s), None)
    publish_at = next(
        (i for i, s in enumerate(names) if re.search(r"cargo publish|npm publish|gh-action-pypi-publish|nuget push", s)),
        None,
    )
    check(
        f"{job_name} tests its artifact before publishing",
        smoke is not None and publish_at is not None and smoke < publish_at,
    )
    check(f"{job_name} waits for the server build", job.get("needs") == "server")

print()
if failures:
    print(f"{len(failures)} CI gate assertion(s) failed")
    sys.exit(1)
print("All CI gate assertions passed")
