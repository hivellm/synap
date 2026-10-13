"""Artifact smoke test for the Python SDK.

Run by scripts/release/artifact-smoke/run.py with the interpreter of a fresh
virtual environment that installed the built wheel, from a directory outside
the repository, so ``synap_sdk`` resolves to the artifact about to be
published, not to the source tree.

    argv:   <http-url> <synap-url> <resp3-url>
    stdout: one ``STEP <transport>.<name> PASS|FAIL <detail>`` line per step
    exit:   0 if every step passed
"""

from __future__ import annotations

import asyncio
import sys
from collections.abc import Awaitable, Callable

from synap_sdk import SynapClient, SynapConfig

failures = 0


def report(step: str, ok: bool, detail: str) -> None:
    global failures
    print(f"STEP {step} {'PASS' if ok else 'FAIL'} {detail}", flush=True)
    if not ok:
        failures += 1


async def check(step: str, run: Callable[[], Awaitable[tuple[bool, str]]]) -> None:
    try:
        ok, detail = await run()
    except Exception as exc:  # noqa: BLE001 - a crash is reported as this step's failure
        report(step, False, f"threw {type(exc).__name__}: {exc}")
        return
    report(step, ok, detail)


async def transport_suite(name: str, url: str) -> None:
    key = f"smoke:py:{name}:key"
    counter = f"smoke:py:{name}:counter"
    async with SynapClient(SynapConfig(url, timeout=5)) as client:

        async def set_get() -> tuple[bool, str]:
            await client.kv.set(key, f"value-{name}")
            got = await client.kv.get(key)
            return got == f"value-{name}", f"got {got!r}"

        async def counter_amounts() -> tuple[bool, str]:
            await client.kv.set(counter, "0")
            seen = [
                await client.kv.incr(counter, 10),
                await client.kv.decr(counter, 4),
                await client.kv.incr(counter),
            ]
            return seen == [10, 6, 7], f"incr 10, decr 4, incr -> {seen}"

        async def delete() -> tuple[bool, str]:
            await client.kv.delete(key)
            await client.kv.delete(counter)
            got = await client.kv.get(key)
            return got is None, f"get after delete -> {got!r}"

        await check(f"{name}.set_get", set_get)
        await check(f"{name}.counter_amounts", counter_amounts)
        await check(f"{name}.delete", delete)


async def cross_transport(http_url: str, rpc_url: str, resp3_url: str) -> None:
    key = "smoke:py:cross"
    async with (
        SynapClient(SynapConfig(http_url, timeout=5)) as http,
        SynapClient(SynapConfig(rpc_url, timeout=5)) as rpc,
        SynapClient(SynapConfig(resp3_url, timeout=5)) as resp3,
    ):

        async def http_to_native() -> tuple[bool, str]:
            await http.kv.set(key, "from-http")
            seen = [await rpc.kv.get(key), await resp3.kv.get(key)]
            await http.kv.delete(key)
            return all(v == "from-http" for v in seen), f"synaprpc, resp3 -> {seen}"

        await check("cross.http_to_native", http_to_native)


async def main(http_url: str, rpc_url: str, resp3_url: str) -> int:
    await transport_suite("http", http_url)
    await transport_suite("synaprpc", rpc_url)
    await transport_suite("resp3", resp3_url)
    await cross_transport(http_url, rpc_url, resp3_url)
    return 0 if failures == 0 else 1


if __name__ == "__main__":
    if len(sys.argv) != 4:
        print("usage: smoke.py <http-url> <synap-url> <resp3-url>", file=sys.stderr)
        sys.exit(2)
    sys.exit(asyncio.run(main(*sys.argv[1:])))
