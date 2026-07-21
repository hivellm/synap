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

from thunder_rpc import Value

from synap_sdk.transport_rpc import MAX_FRAME_BYTES

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
    """`_to_wire` now produces Thunder `Value`s rather than hand-rolled
    serde envelopes; the wire bytes are unchanged, the in-process type is not."""

    def test_none_becomes_null(self) -> None:
        assert _to_wire(None) == Value.null()

    def test_true_becomes_bool(self) -> None:
        assert _to_wire(True) == Value.bool(True)

    def test_false_becomes_bool(self) -> None:
        assert _to_wire(False) == Value.bool(False)

    def test_bool_is_not_encoded_as_int(self) -> None:
        # `bool` is an `int` subclass in Python — checking int first would send
        # `Int(1)` where the server's dispatch tree expects `Bool(true)`.
        assert _to_wire(True).kind == "bool"

    def test_int_becomes_int(self) -> None:
        assert _to_wire(42) == Value.int(42)

    def test_negative_int(self) -> None:
        assert _to_wire(-7) == Value.int(-7)

    def test_float_becomes_float(self) -> None:
        assert _to_wire(3.14) == Value.float(3.14)

    def test_str_becomes_str(self) -> None:
        assert _to_wire("hello") == Value.str("hello")

    def test_bytes_becomes_bytes(self) -> None:
        assert _to_wire(bytes([1, 2, 3])) == Value.bytes(bytes([1, 2, 3]))


class TestFromWire:
    def test_null_becomes_none(self) -> None:
        assert _from_wire(Value.null()) is None

    def test_none_becomes_none(self) -> None:
        assert _from_wire(None) is None

    def test_bool_true(self) -> None:
        assert _from_wire(Value.bool(True)) is True

    def test_bool_false(self) -> None:
        assert _from_wire(Value.bool(False)) is False

    def test_int(self) -> None:
        assert _from_wire(Value.int(99)) == 99

    def test_float(self) -> None:
        assert _from_wire(Value.float(1.5)) == 1.5

    def test_str(self) -> None:
        assert _from_wire(Value.str("world")) == "world"

    def test_bytes(self) -> None:
        # 0xDE 0xAD happens to be valid UTF-8 (U+07AD) and the SDK contract
        # decodes UTF-8-looking bytes to str...
        assert _from_wire(Value.bytes(bytes([0xDE, 0xAD]))) == chr(0x07AD)
        # ...while invalid UTF-8 stays raw bytes.
        invalid = bytes([0xFF, 0xFE])
        assert _from_wire(Value.bytes(invalid)) == invalid

    def test_array(self) -> None:
        wire = Value.array([Value.int(1), Value.str("two")])
        assert _from_wire(wire) == [1, "two"]

    def test_map(self) -> None:
        wire = Value.map([(Value.str("k"), Value.int(7))])
        assert _from_wire(wire) == {"k": 7}

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
        # A non-`Value` passes through as-is.
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

    def test_write_with_client_id_wraps_into_txqueue(self) -> None:
        result = _map_command(
            "kv.set", {"key": "foo", "value": "bar", "client_id": "tx1"}
        )
        assert result == ("TXQUEUE", ["tx1", "SET", "foo", "bar"])

    def test_unqueueable_with_client_id_returns_none(self) -> None:
        result = _map_command(
            "sorted_set.add",
            {"key": "z", "member": "m", "score": 1.0, "client_id": "tx1"},
        )
        assert result is None

    def test_transaction_commands_not_wrapped(self) -> None:
        result = _map_command("transaction.exec", {"client_id": "tx1"})
        assert result == ("EXEC", ["tx1"])

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
        # Requests are array-encoded structs: [id, command, args] (WIRE-012).
        decoded = msgpack.unpackb(body, raw=False)
        got_request["id"], got_request["cmd"], got_request["args"] = decoded
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
        # Decoded with msgpack directly, independently of Thunder: the args
        # really are externally-tagged on the wire, unchanged by the swap.
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


# ── Frame cap ──────────────────────────────────────────────────────────────────


@pytest.mark.asyncio
async def test_synaprpc_transport_refuses_over_cap_length_prefix() -> None:
    """A length prefix above the cap is refused before the body is allocated.

    The pre-Thunder transport called ``readexactly(frame_len)`` with whatever a
    remote peer's 4-byte prefix claimed, so a tiny message could drive an
    unbounded allocation. Thunder validates against ``max_frame_bytes`` first.
    """
    over_cap = MAX_FRAME_BYTES + 1

    async def handler(r: asyncio.StreamReader, w: asyncio.StreamWriter) -> None:
        # Read the request, then answer with a header claiming more than the
        # cap — and send only the header, so a client that allocated first
        # would wait forever for a body that never arrives.
        len_bytes = await r.readexactly(4)
        frame_len = struct.unpack_from("<I", len_bytes)[0]
        await r.readexactly(frame_len)
        w.write(struct.pack("<I", over_cap))
        await w.drain()
        w.close()

    server = await asyncio.start_server(handler, "127.0.0.1", 0)
    port = server.sockets[0].getsockname()[1]

    transport = SynapRpcTransport("127.0.0.1", port, timeout=5.0)
    try:
        with pytest.raises(Exception):  # noqa: B017, PT011 - any refusal is correct
            await asyncio.wait_for(transport.execute("GET", ["k"]), timeout=10.0)
    finally:
        await transport.close()
        server.close()
        await server.wait_closed()
