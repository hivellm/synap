#!/usr/bin/env python3
"""Structural assertions for .github/workflows/release.yml.

Guards the release-artifact contract (Nexus/Vectorizer pattern):
triggers, both binaries, all five targets, sha256 checksums, native
arm64 runner, no cross-compile leftovers.

Usage: python scripts/release/test-release-workflow.py
"""

import sys
from pathlib import Path

import yaml

WORKFLOW = Path(__file__).resolve().parents[2] / ".github/workflows/release.yml"

EXPECTED_TARGETS = {
    "x86_64-unknown-linux-gnu",
    "aarch64-unknown-linux-gnu",
    "x86_64-apple-darwin",
    "aarch64-apple-darwin",
    "x86_64-pc-windows-msvc",
}

failures = []


def check(label: str, ok: bool) -> None:
    print(f"  {'ok' if ok else 'FAIL'}: {label}")
    if not ok:
        failures.append(label)


doc = yaml.safe_load(WORKFLOW.read_text(encoding="utf-8"))

# YAML 1.1 parses the bare key `on` as boolean True.
triggers = doc.get("on", doc.get(True, {}))
check("triggers on release:published", triggers.get("release", {}).get("types") == ["published"])
check(
    "workflow_dispatch has required tag input",
    triggers.get("workflow_dispatch", {}).get("inputs", {}).get("tag", {}).get("required") is True,
)
check("resolves RELEASE_TAG env", "RELEASE_TAG" in doc.get("env", {}))
check("cold-CI incremental off", doc.get("env", {}).get("CARGO_INCREMENTAL") == "0")
check("can write release assets", doc.get("permissions", {}).get("contents") == "write")

jobs = doc["jobs"]
uploads = []  # (job, bin, target, checksum)
for job_name, job in jobs.items():
    matrix_targets = [
        inc.get("target")
        for inc in job.get("strategy", {}).get("matrix", {}).get("include", [])
    ]
    for step in job.get("steps", []):
        if "upload-rust-binary-action" in str(step.get("uses", "")):
            with_ = step.get("with", {})
            target = with_.get("target")
            targets = matrix_targets if str(target).startswith("${{") else [target]
            for t in targets:
                uploads.append((job_name, with_.get("bin"), t, with_.get("checksum")))

built_targets = {t for (_, _, t, _) in uploads}
check(f"all five targets covered ({len(built_targets)}/5)", built_targets == EXPECTED_TARGETS)

for binary in ("synap-server", "synap-cli"):
    covered = {t for (_, b, t, _) in uploads if b == binary}
    check(f"{binary} ships on all targets", covered == EXPECTED_TARGETS)

check("every upload has sha256 checksum", all(c == "sha256" for (_, _, _, c) in uploads))
check(
    "every upload pins ref to the release tag",
    all(
        "refs/tags/" in str(step.get("with", {}).get("ref", ""))
        for job in jobs.values()
        for step in job.get("steps", [])
        if "upload-rust-binary-action" in str(step.get("uses", ""))
    ),
)

arm_jobs = [name for name, job in jobs.items() if job.get("runs-on") == "ubuntu-24.04-arm"]
check("aarch64-linux builds on a native arm runner", len(arm_jobs) == 1)

raw = WORKFLOW.read_text(encoding="utf-8")
check("no cross gcc leftovers", "gcc-aarch64-linux-gnu" not in raw)
check(
    "no vendored-openssl build flag in release builds",
    "--features vendored-openssl" not in raw
    and not any(
        "vendored-openssl" in str(step.get("with", {}).get("features", ""))
        for job in jobs.values()
        for step in job.get("steps", [])
    ),
)

print()
if failures:
    print(f"{len(failures)} release-workflow assertion(s) failed")
    sys.exit(1)
print("All release-workflow assertions passed")
