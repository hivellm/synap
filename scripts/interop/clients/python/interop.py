#!/usr/bin/env python3
"""Interop cell: Python SDK (hivellm-thunder) against a Thunder-based server.

Drives ``SynapRpcTransport`` directly rather than the sugar layer -- the matrix
is about the wire, and the transport is where the wire lives.
"""

from __future__ import annotations

import asyncio
import sys
import threading

from synap_sdk.exceptions import SynapException
from synap_sdk.transport_rpc import SynapRpcTransport
from thunder_rpc import Credentials

# A value that is emphatically not valid UTF-8, so a transport that quietly
# round-trips through a string cannot pass this cell.
BINARY = bytes([0xDE, 0xAD, 0xBE, 0xEF])
TOPIC = "interop.python"


def report(step: str, ok: bool, detail: str) -> None:
    print(f"STEP {step} {'PASS' if ok else 'FAIL'} {detail}", flush=True)


async def main() -> int:
    host, port, user, password = sys.argv[1], int(sys.argv[2]), sys.argv[3], sys.argv[4]
    failures = 0

    transport = SynapRpcTransport(
        host, port, timeout=15.0, credentials=Credentials.user_pass(user, password)
    )

    # 1. authenticate -- the handshake happens on the first call.
    #
    # EXISTS rather than PING: the server answers PING before authentication,
    # so a PING probe passes just as happily on a connection that never
    # authenticated -- exactly the bug this column exists to catch.
    try:
        probe = await transport.execute("EXISTS", ["interop:python:probe"])
        report("auth", True, f"EXISTS -> {probe!r}")
    except SynapException as exc:
        report("auth", False, f"{type(exc).__name__}: {exc}")
        await transport.close()
        return 1

    # 2. SET/GET a binary value -- canonical MessagePack bin, byte-exact back.
    try:
        await transport.execute("SET", ["interop:python:bin", BINARY])
        got = await transport.execute("GET", ["interop:python:bin"])
        got_bytes = got.encode("latin-1") if isinstance(got, str) else got
        ok = got_bytes == BINARY
        report("kv_binary", ok, f"{BINARY.hex()} -> {bytes(got_bytes or b'').hex()}")
        failures += 0 if ok else 1
    except SynapException as exc:
        report("kv_binary", False, f"{type(exc).__name__}: {exc}")
        failures += 1

    # 3. SUBSCRIBE then PUBLISH -- the push frame must arrive on the hook.
    received: list[dict] = []
    arrived = threading.Event()
    cancel = None
    try:
        def on_message(msg: dict) -> None:
            received.append(msg)
            arrived.set()

        _subscriber_id, cancel = await transport.subscribe_push([TOPIC], on_message)
        await transport.execute("PUBLISH", [TOPIC, "interop-payload"])

        for _ in range(50):
            if arrived.is_set():
                break
            await asyncio.sleep(0.1)

        ok = bool(received) and received[0]["topic"] == TOPIC
        report("pubsub", ok, f"received={received[:1]}")
        failures += 0 if ok else 1
    except SynapException as exc:
        report("pubsub", False, f"{type(exc).__name__}: {exc}")
        failures += 1
    finally:
        if cancel is not None:
            cancel()

    # 4. Error round-trip -- an unknown command must surface as an exception,
    #    not as a null result, and must not poison the connection.
    try:
        result = await transport.execute("NOSUCHCOMMAND", [])
        report("error", False, f"expected an error, got {result!r}")
        failures += 1
    except SynapException as exc:
        still_alive = await transport.execute("PING", [])
        ok = still_alive == "PONG"
        report("error", ok, f"raised {type(exc).__name__}; connection alive={ok}")
        failures += 0 if ok else 1

    await transport.close()
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(asyncio.run(main()))
