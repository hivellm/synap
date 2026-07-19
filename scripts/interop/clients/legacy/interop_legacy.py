#!/usr/bin/env python3
"""Interop cell: a pre-Thunder client against a Thunder-based server.

This is the compatibility cell. It deliberately does *not* import any Synap
SDK -- it hand-writes the wire exactly as the SDKs emitted it before the
Thunder swap, so it keeps testing the old encoding even after every SDK has
moved on and no pre-Thunder build is left to check out:

  * requests are **map-shaped** (``{"id", "command", "args"}``) rather than the
    array form Thunder emits (WIRE-012/013 decode tolerance);
  * ``Bytes`` are an **array of integers** (rmp_serde's ``Vec<u8>`` form)
    rather than MessagePack ``bin`` (WIRE-010/011);
  * there is no frame cap and no handshake beyond ``AUTH``.

A green cell here means an old client in the wild keeps working against a
1.1.0 server. A red one means the release breaks deployed software.
"""

from __future__ import annotations

import socket
import struct
import sys

import msgpack

BINARY = bytes([0xDE, 0xAD, 0xBE, 0xEF])
TOPIC = "interop.legacy"

# The reserved push id. A legacy client used it as the *request* id for
# SUBSCRIBE, which a Thunder server refuses -- see the pubsub step.
PUSH_ID = 0xFFFFFFFF


def report(step: str, ok: bool, detail: str) -> None:
    print(f"STEP {step} {'PASS' if ok else 'FAIL'} {detail}", flush=True)


class LegacyConn:
    """The pre-Thunder framing: 4-byte LE length prefix + MessagePack body."""

    def __init__(self, host: str, port: int) -> None:
        self.sock = socket.create_connection((host, port), timeout=15)
        self.next_id = 0

    def send(self, cmd: str, args: list, *, frame_id: int | None = None) -> int:
        if frame_id is None:
            self.next_id += 1
            frame_id = self.next_id
        # Map-shaped request: what the old SDKs emitted.
        body = msgpack.packb(
            {"id": frame_id, "command": cmd.upper(), "args": args},
            use_bin_type=True,
        )
        self.sock.sendall(struct.pack("<I", len(body)) + body)
        return frame_id

    def recv(self) -> tuple[int, dict]:
        header = self._read_exactly(4)
        (length,) = struct.unpack("<I", header)
        decoded = msgpack.unpackb(self._read_exactly(length), raw=False, strict_map_key=False)
        # Responses are array-shaped: [id, {"Ok"|"Err": ...}]
        if isinstance(decoded, list):
            return decoded[0], decoded[1]
        return decoded["id"], decoded["result"]

    def call(self, cmd: str, args: list) -> dict:
        self.send(cmd, args)
        _frame_id, result = self.recv()
        return result

    def _read_exactly(self, n: int) -> bytes:
        buf = b""
        while len(buf) < n:
            chunk = self.sock.recv(n - len(buf))
            if not chunk:
                raise ConnectionError("server closed the connection")
            buf += chunk
        return buf

    def close(self) -> None:
        self.sock.close()


def wire_str(s: str) -> dict:
    return {"Str": s}


def wire_bytes_legacy(b: bytes) -> dict:
    """`Bytes` as an array of integers -- the pre-1.1.0 encoding."""
    return {"Bytes": list(b)}


def main() -> int:
    host, port, user, password = sys.argv[1], int(sys.argv[2]), sys.argv[3], sys.argv[4]
    failures = 0
    conn = LegacyConn(host, port)

    # 1. AUTH, map-shaped, as the old clients sent it.
    result = conn.call("AUTH", [wire_str(user), wire_str(password)])
    ok = "Ok" in result
    report("auth", ok, f"map-shaped AUTH -> {result}")
    failures += 0 if ok else 1
    if not ok:
        conn.close()
        return 1

    # 2. SET a binary value in the legacy int-array form, GET it back. The
    #    server answers in the canonical `bin` form, which msgpack hands us as
    #    Python bytes -- an old client's decoder accepted both.
    conn.call("SET", [wire_str("interop:legacy:bin"), wire_bytes_legacy(BINARY)])
    got = conn.call("GET", [wire_str("interop:legacy:bin")])
    raw = got.get("Ok", {}) if isinstance(got, dict) else {}
    value = raw.get("Bytes") if isinstance(raw, dict) else None
    if isinstance(value, list):
        value = bytes(value)
    elif isinstance(value, str):
        value = value.encode("latin-1")
    ok = value == BINARY
    report("kv_binary", ok, f"sent int-array, got {bytes(value or b'').hex()}")
    failures += 0 if ok else 1

    # 3. SUBSCRIBE. The old clients sent this with id == PUSH_ID, the reserved
    #    push sentinel. A Thunder server refuses a *request* carrying it, which
    #    is correct -- the sentinel identifies server-to-client frames. The cell
    #    therefore asserts the refusal is clean (an error reply or a closed
    #    connection), not that the subscription works: a legacy client's pub/sub
    #    over RPC is a known casualty of the reserved id, documented in the
    #    matrix rather than silently passed.
    try:
        conn.send("SUBSCRIBE", [wire_str(TOPIC)], frame_id=PUSH_ID)
        _id, result = conn.recv()
        refused = "Err" in result
        report("pubsub", refused, f"reserved-id SUBSCRIBE -> {result}")
        failures += 0 if refused else 1
    except (ConnectionError, OSError) as exc:
        # A closed connection is also a clean refusal.
        report("pubsub", True, f"reserved-id SUBSCRIBE closed the connection: {exc}")
        conn = LegacyConn(host, port)
        conn.call("AUTH", [wire_str(user), wire_str(password)])

    # 4. Error round-trip: an unknown command must come back as `Err`, and the
    #    connection must stay usable.
    result = conn.call("NOSUCHCOMMAND", [])
    errored = "Err" in result
    alive = "Ok" in conn.call("EXISTS", [wire_str("interop:legacy:probe")])
    ok = errored and alive
    report("error", ok, f"{result}; connection alive={alive}")
    failures += 0 if ok else 1

    conn.close()
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
