#!/usr/bin/env python3
"""Test a packaged SDK artifact against a built synap-server before publishing.

The SDK test suites run against the source tree. What a registry receives is a
package: a tarball, a wheel, a .nupkg, a .crate. A file left out of the
package, a dependency that only resolves inside the workspace, or an entry
point that points at the wrong build passes every source-tree test and breaks
for the first user. This driver installs the artifact into a scratch project
outside the repository, the way a user would, and runs a smoke client against
a real server over every transport the SDK speaks:

    set/get, INCR/DECR with and without an amount, delete, and a write over
    HTTP read back over SynapRPC and RESP3.

Usage::

    python scripts/release/artifact-smoke/run.py typescript --artifact sdks/typescript/hivehub-synap-1.4.0.tgz
    python scripts/release/artifact-smoke/run.py python     --artifact sdks/python/dist/synap_sdk-1.4.0-py3-none-any.whl
    python scripts/release/artifact-smoke/run.py csharp     --artifact sdks/csharp/artifacts/HiveLLM.Synap.SDK.1.4.0.nupkg
    python scripts/release/artifact-smoke/run.py rust       --artifact target/package/synap-sdk-1.4.0.crate

``--server`` defaults to ``target/release/synap-server``. The server runs from
a temporary directory on free loopback ports, and its WAL and snapshots
stay in that directory.
Exit status is 0 only if every smoke step passed.
"""

from __future__ import annotations

import argparse
import os
import re
import shutil
import socket
import subprocess
import sys
import tarfile
import tempfile
import time
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
HERE = Path(__file__).resolve().parent
EXE = ".exe" if os.name == "nt" else ""


def free_ports(count: int) -> list[int]:
    """Distinct free loopback ports: every socket stays bound until all are picked."""
    sockets = [socket.socket() for _ in range(count)]
    try:
        for s in sockets:
            s.bind(("127.0.0.1", 0))
        return [s.getsockname()[1] for s in sockets]
    finally:
        for s in sockets:
            s.close()


def server_config(http: int, rpc: int, resp3: int) -> str:
    return f"""server:
  host: "127.0.0.1"
  port: {http}
  websocket_enabled: false
synap_rpc:
  enabled: true
  host: "127.0.0.1"
  port: {rpc}
resp3:
  enabled: true
  host: "127.0.0.1"
  port: {resp3}
auth:
  enabled: false
kv_store:
  max_memory_mb: 256
  eviction_policy: "allkeys-lru"
  ttl_cleanup_interval_ms: 100
queue:
  enabled: true
  max_depth: 10000
  ack_deadline_secs: 30
  default_max_retries: 3
  default_priority: 5
logging:
  level: "warn"
  format: "pretty"
protocols:
  streamable_http:
    enabled: true
    path: "/api/v1/command"
  rest:
    enabled: true
    prefix: "/kv"
rate_limit:
  enabled: false
  requests_per_second: 100000
  burst_size: 100000
"""


def wait_for_ports(proc: subprocess.Popen, ports: list[int], log: Path, timeout: float = 60.0) -> None:
    deadline = time.monotonic() + timeout
    pending = list(ports)
    while pending and time.monotonic() < deadline:
        if proc.poll() is not None:
            raise SystemExit(f"synap-server exited with {proc.returncode}:\n{log.read_text(errors='replace')}")
        with socket.socket() as s:
            s.settimeout(0.5)
            if s.connect_ex(("127.0.0.1", pending[0])) == 0:
                pending.pop(0)
                continue
        time.sleep(0.2)
    if pending:
        raise SystemExit(f"synap-server did not open ports {pending} within {timeout:.0f}s")


def run(argv: list[str], cwd: Path, env: dict[str, str] | None = None) -> None:
    print(f"$ {' '.join(argv)}", flush=True)
    subprocess.run(argv, cwd=cwd, env=env, check=True)


def tool(name: str) -> str:
    """Resolve a launcher (npm is npm.cmd on Windows) or fail with a clear message."""
    found = shutil.which(name)
    if not found:
        raise SystemExit(f"{name} is not on PATH")
    return found


def clean_env() -> dict[str, str]:
    """The environment minus anything that could point back at the source tree."""
    env = {k: v for k, v in os.environ.items() if k not in {"PYTHONPATH", "NODE_PATH"}}
    return env


# --------------------------------------------------------------------------
# Consumers: install the artifact into `work` and return the smoke command.
# --------------------------------------------------------------------------
def prepare_typescript(artifact: Path, work: Path) -> list[str]:
    (work / "package.json").write_text(
        '{ "name": "synap-artifact-smoke", "private": true, "type": "module" }\n', encoding="utf-8"
    )
    run([tool("npm"), "install", "--no-audit", "--no-fund", str(artifact)], work)
    shutil.copy(HERE / "typescript" / "smoke.mjs", work / "smoke.mjs")
    return [tool("node"), "smoke.mjs"]


def prepare_python(artifact: Path, work: Path) -> list[str]:
    run([sys.executable, "-m", "venv", str(work / "venv")], work)
    python = work / "venv" / ("Scripts" if os.name == "nt" else "bin") / f"python{EXE}"
    run([str(python), "-m", "pip", "install", "--quiet", str(artifact)], work, clean_env())
    shutil.copy(HERE / "python" / "smoke.py", work / "smoke.py")
    return [str(python), "smoke.py"]


def prepare_csharp(artifact: Path, work: Path) -> list[str]:
    match = re.fullmatch(r"HiveLLM\.Synap\.SDK\.(\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?)\.nupkg", artifact.name)
    if not match:
        raise SystemExit(f"{artifact.name} is not a HiveLLM.Synap.SDK.<version>.nupkg")
    version = match.group(1)
    for name in ("Program.cs", "Smoke.csproj"):
        shutil.copy(HERE / "csharp" / name, work / name)
    # Package source mapping: HiveLLM.Synap.SDK may only come from the
    # artifact's directory, so a same-version package on nuget.org can never
    # stand in for the one under test. Its dependencies come from nuget.org.
    (work / "nuget.config").write_text(
        f"""<?xml version="1.0" encoding="utf-8"?>
<configuration>
  <packageSources>
    <clear />
    <add key="artifact" value="{artifact.parent}" />
    <add key="nuget.org" value="https://api.nuget.org/v3/index.json" />
  </packageSources>
  <packageSourceMapping>
    <packageSource key="artifact">
      <package pattern="HiveLLM.Synap.SDK" />
    </packageSource>
    <packageSource key="nuget.org">
      <package pattern="*" />
    </packageSource>
  </packageSourceMapping>
  <config>
    <add key="globalPackagesFolder" value="{work / 'packages'}" />
  </config>
</configuration>
""",
        encoding="utf-8",
    )
    dotnet = tool("dotnet")
    run([dotnet, "build", "--configuration", "Release", f"-p:SynapSdkVersion={version}"], work)
    return [dotnet, "run", "--no-build", "--configuration", "Release", "--"]


def prepare_rust(artifact: Path, work: Path) -> list[str]:
    match = re.fullmatch(r"synap-sdk-(.+)\.crate", artifact.name)
    if not match:
        raise SystemExit(f"{artifact.name} is not a synap-sdk-<version>.crate")
    with tarfile.open(artifact, "r:gz") as crate:
        crate.extractall(work / "unpacked", filter="data")
    sdk = work / "unpacked" / f"synap-sdk-{match.group(1)}"
    consumer = work / "consumer"
    (consumer / "src").mkdir(parents=True)
    shutil.copy(HERE / "rust" / "main.rs", consumer / "src" / "main.rs")
    (consumer / "Cargo.toml").write_text(
        f"""[package]
name = "synap-artifact-smoke"
version = "0.0.0"
edition = "2024"
publish = false

[dependencies]
synap-sdk = {{ path = {str(sdk.as_posix())!r} }}
serde_json = "1"
tokio = {{ version = "1", features = ["macros", "rt-multi-thread"] }}

# Standalone: never adopted by a workspace further up the directory tree.
[workspace]
""",
        encoding="utf-8",
    )
    # Outside the repository rust-toolchain.toml does not apply; pin the
    # consumer to the same toolchain the release builds use.
    toolchain = re.search(
        r'^channel\s*=\s*"([^"]+)"', (REPO / "rust-toolchain.toml").read_text(encoding="utf-8"), re.MULTILINE
    )
    env = clean_env()
    if toolchain:
        env["RUSTUP_TOOLCHAIN"] = toolchain.group(1)
    run([tool("cargo"), "build", "--quiet"], consumer, env)
    return [str(consumer / "target" / "debug" / f"synap-artifact-smoke{EXE}")]


PREPARE = {
    "typescript": prepare_typescript,
    "python": prepare_python,
    "csharp": prepare_csharp,
    "rust": prepare_rust,
}


def main() -> int:
    if hasattr(sys.stdout, "reconfigure"):
        sys.stdout.reconfigure(encoding="utf-8", errors="replace")
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("sdk", choices=sorted(PREPARE))
    parser.add_argument("--artifact", type=Path, required=True, help="the packaged SDK to test")
    parser.add_argument(
        "--server",
        type=Path,
        default=REPO / "target" / "release" / f"synap-server{EXE}",
        help="synap-server binary (default: target/release/synap-server)",
    )
    args = parser.parse_args()

    artifact = args.artifact.resolve()
    if not artifact.is_file():
        raise SystemExit(f"artifact not found: {artifact}")
    server = args.server.resolve()
    if not server.is_file():
        raise SystemExit(
            f"server binary not found: {server}\nbuild it first: cargo build --release --locked -p synap-server"
        )

    with tempfile.TemporaryDirectory(prefix=f"synap-artifact-{args.sdk}-", ignore_cleanup_errors=True) as tmp:
        tmp_path = Path(tmp)
        work = tmp_path / "consumer-project"
        work.mkdir()
        print(f"installing {artifact.name} into a scratch {args.sdk} project", flush=True)
        command = PREPARE[args.sdk](artifact, work)

        http, rpc, resp3 = free_ports(3)
        server_dir = tmp_path / "server"
        server_dir.mkdir()
        (server_dir / "config.yml").write_text(server_config(http, rpc, resp3), encoding="utf-8")
        log = server_dir / "server.log"
        with log.open("w", encoding="utf-8") as handle:
            proc = subprocess.Popen(
                [str(server), "--config", "config.yml"], cwd=server_dir, stdout=handle, stderr=subprocess.STDOUT
            )
            try:
                wait_for_ports(proc, [http, rpc, resp3], log)
                urls = [f"http://127.0.0.1:{http}", f"synap://127.0.0.1:{rpc}", f"resp3://127.0.0.1:{resp3}"]
                print(f"$ {' '.join(command + urls)}", flush=True)
                done = subprocess.run(command + urls, cwd=work, env=clean_env(), timeout=300, check=False)
            finally:
                proc.terminate()
                try:
                    proc.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    proc.kill()
                    proc.wait(timeout=10)
        if done.returncode != 0:
            print(f"\nartifact smoke FAILED for {args.sdk} ({artifact.name}); server log:")
            print(log.read_text(encoding="utf-8", errors="replace")[-4000:])
            return 1
    print(f"\nartifact smoke passed for {args.sdk} ({artifact.name})")
    return 0


if __name__ == "__main__":
    sys.exit(main())
