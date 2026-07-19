#!/usr/bin/env python3
"""Cross-SDK interop matrix for the Thunder-based SynapRPC transport.

One server build, every SDK, the same four steps. The point is not to test each
SDK again -- each has its own suite -- but to prove the thing no single-language
suite can: that a Thunder-based Synap server and every Synap client still agree
on the wire, including the two SDKs with no Thunder package (PHP, Java) and a
pre-Thunder client still in the wild.

Every client is a standalone program under ``clients/`` that speaks the same
contract:

    argv:   <host> <port> <user> <pass>
    stdout: one ``STEP <name> PASS|FAIL <detail>`` line per step
    exit:   0 if every step passed

The driver owns the server, runs the clients, and renders the matrix. It never
looks inside a client, so adding a language is adding a directory.

Usage::

    python scripts/interop/run-matrix.py            # every cell
    python scripts/interop/run-matrix.py rust go     # a subset
    python scripts/interop/run-matrix.py --list      # what exists
"""

from __future__ import annotations

import argparse
import json
import os
import re
import shutil
import socket
import subprocess
import sys
import tempfile
import time
from dataclasses import dataclass, field, replace
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
HERE = Path(__file__).resolve().parent
CLIENTS = HERE / "clients"

# The four steps every cell must clear, in the order a client runs them.
STEPS = ["auth", "kv_binary", "pubsub", "error"]

STEP_TITLES = {
    "auth": "authenticate",
    "kv_binary": "SET/GET binary",
    "pubsub": "SUBSCRIBE/PUBLISH",
    "error": "error round-trip",
}

SERVER_CONFIG = HERE / "server-config.yml"


def _from_config(pattern: str) -> str:
    """Pull one scalar out of the checked-in server config.

    The credentials and the RPC port have to agree between the server and every
    client. Reading them back from the one config file means a port change is a
    one-line edit instead of a silent mismatch the matrix would report as a
    connection failure.
    """
    match = re.search(pattern, SERVER_CONFIG.read_text(encoding="utf-8"), re.MULTILINE)
    if not match:
        raise SystemExit(f"{SERVER_CONFIG.name} has no match for {pattern!r}")
    return match.group(1)


RPC_PORT = int(_from_config(r"^synap_rpc:(?:\n.*?)*?^  port:\s*(\d+)"))
USER = _from_config(r'^    username:\s*"([^"]+)"')
PASSWORD = _from_config(r'^    password:\s*"([^"]+)"')


# ---------------------------------------------------------------------------
# Cell definitions
# ---------------------------------------------------------------------------
@dataclass
class Cell:
    """One language's client program and how to run it."""

    name: str
    # Command template; ``{host} {port} {user} {pass}`` are appended by the driver.
    command: list[str]
    cwd: Path
    # Optional one-shot build/restore, run once before the client.
    setup: list[list[str]] = field(default_factory=list)
    # Executable that must resolve for this cell to be runnable at all.
    requires: str | None = None
    note: str = ""
    env: dict[str, str] = field(default_factory=dict)


def discover_cells() -> list[Cell]:
    """Describe every cell. Presence of the toolchain is checked separately."""
    return [
        Cell(
            name="rust",
            command=[
                "cargo", "run", "--quiet", "--release",
                "--package", "synap-sdk", "--example", "interop", "--",
            ],
            cwd=REPO,
            requires="cargo",
            note="synap-sdk example, transport = thunder-rpc",
        ),
        Cell(
            name="typescript",
            command=["node", str(CLIENTS / "typescript" / "interop.mjs")],
            # The client imports the SDK's built `dist/` by relative path, so
            # node resolves @hivehub/thunder from the SDK's own node_modules --
            # no package.json or install step of its own.
            cwd=REPO / "sdks" / "typescript",
            requires="node",
            note="@hivehub/thunder via the SDK's dist build",
        ),
        Cell(
            name="python",
            command=[sys.executable, str(CLIENTS / "python" / "interop.py")],
            cwd=CLIENTS / "python",
            requires=sys.executable,
            note="hivellm-thunder via synap_sdk",
            env={"PYTHONPATH": str(REPO / "sdks" / "python")},
        ),
        Cell(
            name="csharp",
            command=["dotnet", "run", "--project", str(CLIENTS / "csharp"), "-c", "Release", "--"],
            cwd=CLIENTS / "csharp",
            requires="dotnet",
            note="HiveLLM.Thunder via Synap.SDK",
        ),
        Cell(
            name="go",
            # Inside the SDK's own module, so `go run` resolves the package
            # without a replace directive.
            command=["go", "run", "./cmd/interop"],
            cwd=REPO / "sdks" / "go",
            requires="go",
            note="hand-written transport at wire parity (thunder#9)",
        ),
        Cell(
            name="php",
            command=["php", str(CLIENTS / "php" / "interop.php")],
            cwd=CLIENTS / "php",
            requires="php",
            note="hand-written transport, no Thunder package",
        ),
        Cell(
            name="java",
            command=["java", "-cp", "target/classes:target/dependency/*",
                     "-Dinterop.main=1", "InteropMain"],
            cwd=CLIENTS / "java",
            setup=[["mvn", "-q", "-B", "package", "dependency:copy-dependencies"]],
            requires="mvn",
            note="hand-written transport, no Thunder package",
        ),
        Cell(
            name="legacy",
            command=[sys.executable, str(CLIENTS / "legacy" / "interop_legacy.py")],
            cwd=CLIENTS / "legacy",
            requires=sys.executable,
            note="pre-Thunder wire replay: int-array Bytes, map-shaped frames",
        ),
    ]


# ---------------------------------------------------------------------------
# Server lifecycle
# ---------------------------------------------------------------------------
def server_binary() -> Path:
    exe = ".exe" if os.name == "nt" else ""
    path = REPO / "target" / "release" / f"synap-server{exe}"
    if not path.exists():
        raise SystemExit(
            f"server binary not found at {path}\n"
            "build it first: cargo build --release -p synap-server"
        )
    return path


def wait_for_port(port: int, timeout: float = 30.0) -> None:
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        with socket.socket() as s:
            s.settimeout(0.5)
            if s.connect_ex(("127.0.0.1", port)) == 0:
                return
        time.sleep(0.2)
    raise SystemExit(f"server did not open port {port} within {timeout:.0f}s")


class Server:
    """The one server build every cell is measured against."""

    def __init__(self, workdir: Path) -> None:
        self.workdir = workdir
        self.proc: subprocess.Popen | None = None
        self.log = workdir / "server.log"
        self._handle = None

    def __enter__(self) -> "Server":
        # Copied rather than read in place: the server writes WAL and snapshots
        # relative to its working directory, and that belongs in the temp dir.
        config = self.workdir / "config.yml"
        config.write_text(SERVER_CONFIG.read_text(encoding="utf-8"), encoding="utf-8")
        handle = self.log.open("w", encoding="utf-8")
        self._handle = handle
        self.proc = subprocess.Popen(
            [str(server_binary()), "--config", str(config)],
            cwd=self.workdir,
            stdout=handle,
            stderr=subprocess.STDOUT,
        )
        try:
            wait_for_port(RPC_PORT)
        except SystemExit:
            self.__exit__(None, None, None)
            sys.stderr.write(self.log.read_text(encoding="utf-8", errors="replace"))
            raise
        return self

    def __exit__(self, *_exc: object) -> None:
        if self.proc and self.proc.poll() is None:
            self.proc.terminate()
            try:
                self.proc.wait(timeout=10)
            except subprocess.TimeoutExpired:
                self.proc.kill()
                self.proc.wait(timeout=10)
        # Windows keeps the directory locked until the log handle is released.
        if self._handle is not None:
            self._handle.close()
            self._handle = None


# ---------------------------------------------------------------------------
# Running one cell
# ---------------------------------------------------------------------------
@dataclass
class Result:
    name: str
    steps: dict[str, tuple[str, str]]  # step -> (PASS|FAIL|MISS, detail)
    skipped_reason: str = ""
    output: str = ""

    @property
    def green(self) -> bool:
        return not self.skipped_reason and all(
            self.steps.get(step, ("MISS", ""))[0] == "PASS" for step in STEPS
        )


def parse_steps(stdout: str) -> dict[str, tuple[str, str]]:
    steps: dict[str, tuple[str, str]] = {}
    for line in stdout.splitlines():
        parts = line.strip().split(maxsplit=3)
        if len(parts) >= 3 and parts[0] == "STEP":
            steps[parts[1]] = (parts[2], parts[3] if len(parts) > 3 else "")
    return steps


def run_cell(cell: Cell, timeout: float) -> Result:
    # `SYNAP_INTEROP_<CELL>` overrides the launcher for one cell, e.g.
    # `SYNAP_INTEROP_PHP=C:\php\php.exe`. Needed when the name on PATH is not
    # something that can actually be spawned: a winget-installed PHP resolves
    # to an App Execution Alias, which fails to launch with "Access denied".
    override = os.environ.get(f"SYNAP_INTEROP_{cell.name.upper()}")
    if override:
        cell = replace(cell, command=[override, *cell.command[1:]], requires=override)

    if cell.requires and not (
        shutil.which(cell.requires) or Path(cell.requires).exists()
    ):
        return Result(cell.name, {}, skipped_reason=f"{cell.requires} not on PATH")

    env = {**os.environ, **cell.env}
    transcript: list[str] = []

    for step in cell.setup:
        if not shutil.which(step[0]):
            return Result(cell.name, {}, skipped_reason=f"{step[0]} not on PATH")
        done = subprocess.run(
            step, cwd=cell.cwd, env=env, capture_output=True, text=True, timeout=timeout
        )
        transcript.append(f"$ {' '.join(step)}\n{done.stdout}{done.stderr}")
        if done.returncode != 0:
            return Result(
                cell.name, {},
                skipped_reason=f"setup failed: {' '.join(step)}",
                output="\n".join(transcript),
            )

    argv = [*cell.command, "127.0.0.1", str(RPC_PORT), USER, PASSWORD]
    try:
        done = subprocess.run(
            argv, cwd=cell.cwd, env=env, capture_output=True, text=True, timeout=timeout
        )
    except subprocess.TimeoutExpired:
        return Result(cell.name, {}, skipped_reason=f"timed out after {timeout:.0f}s",
                      output="\n".join(transcript))

    transcript.append(f"$ {' '.join(argv)}\n{done.stdout}{done.stderr}")
    return Result(cell.name, parse_steps(done.stdout), output="\n".join(transcript))


# ---------------------------------------------------------------------------
# Rendering
# ---------------------------------------------------------------------------
def render(results: list[Result], cells: dict[str, Cell]) -> str:
    header = "| SDK | " + " | ".join(STEP_TITLES[s] for s in STEPS) + " | Transport |"
    sep = "|" + "|".join(["---"] * (len(STEPS) + 2)) + "|"
    rows = [header, sep]
    for r in results:
        if r.skipped_reason:
            cells_text = " | ".join(["—"] * len(STEPS))
            rows.append(f"| `{r.name}` | {cells_text} | not run: {r.skipped_reason} |")
            continue
        marks = []
        for step in STEPS:
            status = r.steps.get(step, ("MISS", ""))[0]
            marks.append({"PASS": "✅", "FAIL": "❌"}.get(status, "⚠️ not reported"))
        rows.append(f"| `{r.name}` | " + " | ".join(marks) + f" | {cells[r.name].note} |")
    return "\n".join(rows)


def main() -> int:
    # The matrix renders as markdown with ✅/❌; a cp1252 console would abort on
    # the first tick mark.
    if hasattr(sys.stdout, "reconfigure"):
        sys.stdout.reconfigure(encoding="utf-8", errors="replace")

    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("only", nargs="*", help="run only these cells")
    parser.add_argument("--list", action="store_true", help="list cells and exit")
    parser.add_argument("--timeout", type=float, default=300.0)
    parser.add_argument("--json", type=Path, help="also write raw results here")
    args = parser.parse_args()

    cells = discover_cells()
    if args.list:
        for cell in cells:
            print(f"{cell.name:<12} {cell.note}")
        return 0
    if args.only:
        wanted = set(args.only)
        unknown = wanted - {c.name for c in cells}
        if unknown:
            raise SystemExit(f"unknown cell(s): {', '.join(sorted(unknown))}")
        cells = [c for c in cells if c.name in wanted]

    by_name = {c.name: c for c in cells}
    results: list[Result] = []

    with tempfile.TemporaryDirectory(
        prefix="synap-interop-", ignore_cleanup_errors=True
    ) as tmp:
        with Server(Path(tmp)):
            for cell in cells:
                print(f"--- {cell.name} ---", flush=True)
                result = run_cell(cell, args.timeout)
                results.append(result)
                if result.skipped_reason:
                    print(f"    not run: {result.skipped_reason}", flush=True)
                else:
                    for step in STEPS:
                        status, detail = result.steps.get(step, ("MISS", "not reported"))
                        print(f"    {step:<10} {status} {detail}", flush=True)
                if not result.green and result.output:
                    print(result.output, flush=True)

    print()
    print(render(results, by_name))

    if args.json:
        args.json.write_text(
            json.dumps(
                [
                    {
                        "name": r.name,
                        "green": r.green,
                        "skipped_reason": r.skipped_reason,
                        "steps": {k: {"status": v[0], "detail": v[1]} for k, v in r.steps.items()},
                    }
                    for r in results
                ],
                indent=2,
            ),
            encoding="utf-8",
        )

    red = [r.name for r in results if not r.green and not r.skipped_reason]
    not_run = [r.name for r in results if r.skipped_reason]
    if not_run:
        print(f"\nnot run: {', '.join(not_run)}")
    if red:
        print(f"\nFAILED: {', '.join(red)}")
        return 1
    print("\nevery cell that ran is green")
    return 0


if __name__ == "__main__":
    sys.exit(main())
