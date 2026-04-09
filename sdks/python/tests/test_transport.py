"""Unit tests for synap_sdk.transport.

Covers:
- _to_wire / _from_wire pure conversion
- _map_command pure mapping
- _map_response pure shape conversion
- SynapRpcTransport against a local asyncio test server
- Resp3Transport against a local asyncio test server
"""

from __future__ import annotations

import asyncio
import struct
from typing import Any

import msgpack
import pytest

from synap_sdk.transport import (
    Resp3Transport,
    SynapRpcTransport,
    _from_wire,
    _map_command,
    _map_response,
    _to_wire,
)


# ── _to_wire / _from_wire ──────────────────────────────────────────────────────


class TestToWire:
    def test_none_becomes_null_string(self) -> None:
        assert _to_wire(None) == "Null"

    def test_true_becomes_bool_envelope(self) -> None:
        assert _to_wire(True) == {"Bool": True}

    def test_false_becomes_bool_envelope(self) -> None:
        assert _to_wire(False) == {"Bool": False}

    def test_int_becomes_int_envelope(self) -> None:
        assert _to_wire(42) == {"Int": 42}

    def test_negative_int(self) -> None:
        assert _to_wire(-7) == {"Int": -7}

    def test_float_becomes_float_envelope(self) -> None:
        assert _to_wire(3.14) == {"Float": 3.14}

    def test_str_becomes_str_envelope(self) -> None:
        assert _to_wire("hello") == {"Str": "hello"}

    def test_bytes_becomes_bytes_envelope(self) -> None:
        result = _to_wire(b"\x01\x02\x03")
        # bytes are kept as-is in the envelope (raw bytes object)
        assert "Bytes" in result
        assert result["Bytes"] == b"\x01\x02\x03"


class TestFromWire:
    def test_null_string_becomes_none(self) -> None:
        assert _from_wire("Null") is None

    def test_none_becomes_none(self) -> None:
        assert _from_wire(None) is None

    def test_bool_envelope_true(self) -> None:
        assert _from_wire({"Bool": True}) is True

    def test_bool_envelope_false(self) -> None:
        assert _from_wire({"Bool": False}) is False

    def test_int_envelope(self) -> None:
        assert _from_wire({"Int": 99}) == 99

    def test_float_envelope(self) -> None:
        assert _from_wire({"Float": 1.5}) == 1.5

    def test_str_envelope(self) -> None:
        assert _from_wire({"Str": "world"}) == "world"

    def test_bytes_envelope(self) -> None:
        raw = {"Bytes": b"\xde\xad"}
        assert _from_wire(raw) == b"\xde\xad"

    def test_roundtrip_none(self) -> None:
        assert _from_wire(_to_wire(None)) is None

    def test_roundtrip_bool(self) -> None:
        assert _from_wire(_to_wire(True)) is True

    def test_roundtrip_int(self) -> None:
        assert _from_wire(_to_wire(123)) == 123

    def test_roundtrip_float(self) -> None:
        assert _from_wire(_to_wire(2.71)) == pytest.approx(2.71)

    def test_roundtrip_str(self) -> None:
        assert _from_wire(_to_wire("synap")) == "synap"

    def test_passthrough_plain_value(self) -> None:
        # Non-envelope, non-null value passes through as-is.
        assert _from_wire(42) == 42


# ── _map_command ───────────────────────────────────────────────────────────────


class TestMapCommand:
    def test_kv_get(self) -> None:
        result = _map_command("kv.get", {"key": "foo"})
        assert result == ("GET", ["foo"])

    def test_kv_set_no_ttl(self) -> None:
        result = _map_command("kv.set", {"key": "foo", "value": {"Str": "bar"}})
        assert result == ("SET", ["foo", {"Str": "bar"}])

    def test_kv_set_with_ttl(self) -> None:
        result = _map_command("kv.set", {"key": "foo", "value": "v", "ttl": 60})
        assert result == ("SET", ["foo", "v", "EX", 60])

    def test_kv_del(self) -> None:
        result = _map_command("kv.del", {"key": "foo"})
        assert result == ("DEL", ["foo"])

    def test_kv_exists(self) -> None:
        result = _map_command("kv.exists", {"key": "foo"})
        assert result == ("EXISTS", ["foo"])

    def test_kv_incr(self) -> None:
        # kv.incr maps to INCR (no delta arg in this version)
        result = _map_command("kv.incr", {"key": "foo"})
        assert result is not None
        assert result[0] == "INCR"
        assert result[1][0] == "foo"

    def test_kv_incrby(self) -> None:
        result = _map_command("kv.incrby", {"key": "foo", "amount": 5})
        assert result == ("INCRBY", ["foo", 5])

    def test_hash_get(self) -> None:
        result = _map_command("hash.get", {"key": "h", "field": "f1"})
        assert result == ("HGET", ["h", "f1"])

    def test_hash_set(self) -> None:
        result = _map_command("hash.set", {"key": "h", "field": "f1", "value": "v1"})
        assert result == ("HSET", ["h", "f1", "v1"])

    def test_list_lpush(self) -> None:
        result = _map_command("list.lpush", {"key": "mylist", "value": "item"})
        assert result == ("LPUSH", ["mylist", "item"])

    def test_set_add(self) -> None:
        result = _map_command("set.add", {"key": "myset", "value": "member"})
        assert result == ("SADD", ["myset", "member"])

    def test_queue_publish_maps_to_qpublish(self) -> None:
        result = _map_command("queue.publish", {"queue": "q", "payload": "msg"})
        assert result is not None
        assert result[0] == "QPUBLISH"

    def test_stream_publish_maps_to_spublish(self) -> None:
        result = _map_command("stream.publish", {"room": "s", "event": "evt", "data": {}})
        assert result is not None
        assert result[0] == "SPUBLISH"

    def test_unknown_command_returns_none(self) -> None:
        result = _map_command("unknown.command", {})
        assert result is None


# ── _map_response ──────────────────────────────────────────────────────────────


class TestMapResponse:
    def test_kv_get(self) -> None:
        assert _map_response("kv.get", "bar") == {"value": "bar"}

    def test_kv_del_deleted(self) -> None:
        assert _map_response("kv.del", 1) == {"deleted": True}

    def test_kv_del_not_deleted(self) -> None:
        assert _map_response("kv.del", 0) == {"deleted": False}

    def test_kv_exists_true(self) -> None:
        assert _map_response("kv.exists", 1) == {"exists": True}

    def test_kv_exists_false(self) -> None:
        assert _map_response("kv.exists", 0) == {"exists": False}

    def test_kv_incr(self) -> None:
        assert _map_response("kv.incr", 42) == {"value": 42}

    def test_kv_incrby(self) -> None:
        assert _map_response("kv.incrby", 10) == {"value": 10}

    def test_hash_getall_list_pairs(self) -> None:
        raw = ["f1", "v1", "f2", "v2"]
        result = _map_response("hash.getall", raw)
        assert result == {"fields": {"f1": "v1", "f2": "v2"}}

    def test_hash_getall_dict(self) -> None:
        raw = {"f1": "v1"}
        result = _map_response("hash.getall", raw)
        assert result == {"fields": {"f1": "v1"}}

    def test_kv_set_ok(self) -> None:
        assert _map_response("kv.set", "OK") == {"success": True}

    def test_kv_set_not_ok(self) -> None:
        assert _map_response("kv.set", "ERR") == {"success": False}

    def test_unknown_command_dict_passthrough(self) -> None:
        raw = {"custom": 123}
        result = _map_response("custom.cmd", raw)
        assert result == {"custom": 123}

    def test_unknown_command_scalar(self) -> None:
        result = _map_response("custom.cmd", 99)
        assert result == {"result": 99}


# ── SynapRpcTransport with local asyncio server ────────────────────────────────


async def _run_rpc_server(
    reader: asyncio.StreamReader,
    writer: asyncio.StreamWriter,
    response_payload: Any,
) -> None:
    """Handle one RPC request and send back response_payload."""
    try:
        len_bytes = await reader.readexactly(4)
        frame_len = struct.unpack_from("<I", len_bytes)[0]
        body = await reader.readexactly(frame_len)
        decoded = msgpack.unpackb(body, raw=False)
        req_id = decoded[0]
        # Send response
        resp = msgpack.packb([req_id, response_payload], use_bin_type=True)
        writer.write(struct.pack("<I", len(resp)) + resp)
        await writer.drain()
    finally:
        writer.close()


@pytest.mark.asyncio
async def test_synaprpc_transport_ok_response() -> None:
    """SynapRpcTransport.execute returns unwrapped value on Ok."""
    # {"Ok": {"Str": "testvalue"}} → _from_wire → "testvalue"
    response_payload = {"Ok": {"Str": "testvalue"}}
    got_request: dict[str, Any] = {}

    async def handler(r: asyncio.StreamReader, w: asyncio.StreamWriter) -> None:
        len_bytes = await r.readexactly(4)
        frame_len = struct.unpack_from("<I", len_bytes)[0]
        body = await r.readexactly(frame_len)
        decoded = msgpack.unpackb(body, raw=False)
        got_request["id"] = decoded[0]
        got_request["cmd"] = decoded[1]
        got_request["args"] = decoded[2]
        resp = msgpack.packb([decoded[0], response_payload], use_bin_type=True)
        w.write(struct.pack("<I", len(resp)) + resp)
        await w.drain()
        w.close()

    server = await asyncio.start_server(handler, "127.0.0.1", 0)
    port = server.sockets[0].getsockname()[1]

    transport = SynapRpcTransport("127.0.0.1", port, timeout=5.0)
    try:
        result = await transport.execute("GET", ["testkey"])
        assert result == "testvalue"
        assert got_request["cmd"] == "GET"
    finally:
        await transport.close()
        server.close()
        await server.wait_closed()


@pytest.mark.asyncio
async def test_synaprpc_transport_error_response() -> None:
    """SynapRpcTransport.execute raises Exception on Err response."""

    async def handler(r: asyncio.StreamReader, w: asyncio.StreamWriter) -> None:
        len_bytes = await r.readexactly(4)
        frame_len = struct.unpack_from("<I", len_bytes)[0]
        body = await r.readexactly(frame_len)
        decoded = msgpack.unpackb(body, raw=False)
        resp = msgpack.packb([decoded[0], {"Err": "not found"}], use_bin_type=True)
        w.write(struct.pack("<I", len(resp)) + resp)
        await w.drain()
        w.close()

    server = await asyncio.start_server(handler, "127.0.0.1", 0)
    port = server.sockets[0].getsockname()[1]

    transport = SynapRpcTransport("127.0.0.1", port, timeout=5.0)
    try:
        with pytest.raises(Exception, match="not found"):
            await transport.execute("GET", ["missingkey"])
    finally:
        await transport.close()
        server.close()
        await server.wait_closed()


@pytest.mark.asyncio
async def test_synaprpc_transport_wire_args_encoded() -> None:
    """SynapRpcTransport wraps args with _to_wire before sending."""
    received_args: list[Any] = []

    async def handler(r: asyncio.StreamReader, w: asyncio.StreamWriter) -> None:
        len_bytes = await r.readexactly(4)
        frame_len = struct.unpack_from("<I", len_bytes)[0]
        body = await r.readexactly(frame_len)
        decoded = msgpack.unpackb(body, raw=False)
        received_args.extend(decoded[2])
        resp = msgpack.packb([decoded[0], {"Ok": "Null"}], use_bin_type=True)
        w.write(struct.pack("<I", len(resp)) + resp)
        await w.drain()
        w.close()

    server = await asyncio.start_server(handler, "127.0.0.1", 0)
    port = server.sockets[0].getsockname()[1]

    transport = SynapRpcTransport("127.0.0.1", port, timeout=5.0)
    try:
        await transport.execute("SET", ["mykey", "myvalue"])
        # First arg "mykey" → {"Str": "mykey"}, second → {"Str": "myvalue"}
        assert received_args[0] == {"Str": "mykey"}
        assert received_args[1] == {"Str": "myvalue"}
    finally:
        await transport.close()
        server.close()
        await server.wait_closed()


# ── Resp3Transport with local asyncio server ───────────────────────────────────


async def _resp3_server_handler(reader: asyncio.StreamReader, writer: asyncio.StreamWriter) -> None:
    """Minimal RESP3 server: handles HELLO handshake then serves GET commands."""
    try:
        while True:
            line = await asyncio.wait_for(reader.readline(), timeout=5.0)
            if not line:
                break
            line_str = line.decode("utf-8").strip()
            if not line_str.startswith("*"):
                continue
            count = int(line_str[1:])
            parts: list[str] = []
            for _ in range(count):
                hdr = await reader.readline()
                hdr_str = hdr.decode("utf-8").strip()
                if hdr_str.startswith("$"):
                    length = int(hdr_str[1:])
                    data = await reader.readexactly(length + 2)
                    parts.append(data[:-2].decode("utf-8"))
            if not parts:
                continue
            cmd = parts[0].upper()
            if cmd == "HELLO":
                writer.write(b"%1\r\n+server\r\n+synap-test\r\n")
            elif cmd == "GET":
                key = parts[1] if len(parts) > 1 else ""
                if key == "testkey":
                    payload = b"testvalue"
                    writer.write(b"$" + str(len(payload)).encode() + b"\r\n" + payload + b"\r\n")
                else:
                    writer.write(b"_\r\n")
            else:
                writer.write(b"+OK\r\n")
            await writer.drain()
    except (asyncio.IncompleteReadError, ConnectionResetError, asyncio.TimeoutError):
        pass
    finally:
        writer.close()


@pytest.mark.asyncio
async def test_resp3_transport_get_existing_key() -> None:
    """Resp3Transport.execute returns string value for existing key."""
    server = await asyncio.start_server(_resp3_server_handler, "127.0.0.1", 0)
    port = server.sockets[0].getsockname()[1]

    transport = Resp3Transport("127.0.0.1", port, timeout=5.0)
    try:
        result = await transport.execute("GET", ["testkey"])
        assert result == "testvalue"
    finally:
        await transport.close()
        server.close()
        await server.wait_closed()


@pytest.mark.asyncio
async def test_resp3_transport_get_missing_key_returns_none() -> None:
    """Resp3Transport.execute returns None for missing key (null bulk string / '_')."""
    server = await asyncio.start_server(_resp3_server_handler, "127.0.0.1", 0)
    port = server.sockets[0].getsockname()[1]

    transport = Resp3Transport("127.0.0.1", port, timeout=5.0)
    try:
        result = await transport.execute("GET", ["missing"])
        assert result is None
    finally:
        await transport.close()
        server.close()
        await server.wait_closed()


@pytest.mark.asyncio
async def test_resp3_transport_error_response() -> None:
    """Resp3Transport.execute raises Exception on RESP3 error reply (- prefix)."""

    async def error_handler(reader: asyncio.StreamReader, writer: asyncio.StreamWriter) -> None:
        try:
            while True:
                line = await asyncio.wait_for(reader.readline(), timeout=5.0)
                if not line:
                    break
                line_str = line.decode("utf-8").strip()
                if not line_str.startswith("*"):
                    continue
                count = int(line_str[1:])
                parts: list[str] = []
                for _ in range(count):
                    hdr = await reader.readline()
                    hdr_str = hdr.decode("utf-8").strip()
                    if hdr_str.startswith("$"):
                        length = int(hdr_str[1:])
                        data = await reader.readexactly(length + 2)
                        parts.append(data[:-2].decode("utf-8"))
                if not parts:
                    continue
                cmd = parts[0].upper() if parts else ""
                if cmd == "HELLO":
                    writer.write(b"%1\r\n+server\r\n+synap-test\r\n")
                else:
                    writer.write(b"-ERR command failed\r\n")
                await writer.drain()
        except (asyncio.IncompleteReadError, ConnectionResetError, asyncio.TimeoutError):
            pass
        finally:
            writer.close()

    server = await asyncio.start_server(error_handler, "127.0.0.1", 0)
    port = server.sockets[0].getsockname()[1]

    transport = Resp3Transport("127.0.0.1", port, timeout=5.0)
    try:
        with pytest.raises(Exception, match="ERR command failed"):
            await transport.execute("GET", ["anykey"])
    finally:
        await transport.close()
        server.close()
        await server.wait_closed()
