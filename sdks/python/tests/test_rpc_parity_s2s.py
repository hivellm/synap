"""RPC parity S2S tests — queues, streams, pub/sub, transactions, scripts.

Tests run across all three transports: HTTP, SynapRPC (synap://), RESP3 (resp3://).
Requires a running Synap server.  Enable with SYNAP_S2S=true.

Environment variables:
    SYNAP_S2S=true          enable these tests
    SYNAP_HTTP_URL          HTTP base URL (default: http://localhost:15500)
    SYNAP_RPC_URL           SynapRPC URL (default: synap://localhost:15501)
    SYNAP_RESP3_URL         RESP3 URL   (default: resp3://localhost:6379)
"""

from __future__ import annotations

import os
import time
import uuid

import pytest

from synap_sdk import SynapClient, SynapConfig, UnsupportedCommandError

pytestmark = pytest.mark.skipif(
    os.getenv("SYNAP_S2S") != "true",
    reason="S2S tests disabled (set SYNAP_S2S=true to enable)",
)

_HTTP_URL = os.getenv("SYNAP_HTTP_URL", "http://localhost:15500")
_RPC_URL = os.getenv("SYNAP_RPC_URL", "synap://localhost:15501")
_RESP3_URL = os.getenv("SYNAP_RESP3_URL", "resp3://localhost:6379")


def _http_client() -> SynapClient:
    return SynapClient(SynapConfig(_HTTP_URL))


def _rpc_client() -> SynapClient:
    return SynapClient(SynapConfig(_RPC_URL))


def _resp3_client() -> SynapClient:
    return SynapClient(SynapConfig(_RESP3_URL))


def _uid() -> str:
    return str(uuid.uuid4())[:8]


# ──────────────────────────────────────────────────────────────────────────────
# Queue tests
# ──────────────────────────────────────────────────────────────────────────────

class TestQueueParity:
    """Queue operations across transports."""

    @pytest.mark.asyncio
    async def test_create_publish_consume_ack_http(self) -> None:
        async with _http_client() as client:
            await _queue_roundtrip(client)

    @pytest.mark.asyncio
    async def test_create_publish_consume_ack_rpc(self) -> None:
        async with _rpc_client() as client:
            await _queue_roundtrip(client)

    @pytest.mark.asyncio
    async def test_create_publish_consume_ack_resp3(self) -> None:
        async with _resp3_client() as client:
            await _queue_roundtrip(client)

    @pytest.mark.asyncio
    async def test_consume_empty_queue_returns_none_http(self) -> None:
        async with _http_client() as client:
            await _queue_empty(client)

    @pytest.mark.asyncio
    async def test_consume_empty_queue_returns_none_rpc(self) -> None:
        async with _rpc_client() as client:
            await _queue_empty(client)

    @pytest.mark.asyncio
    async def test_list_queues_http(self) -> None:
        async with _http_client() as client:
            await _queue_list(client)

    @pytest.mark.asyncio
    async def test_list_queues_rpc(self) -> None:
        async with _rpc_client() as client:
            await _queue_list(client)


async def _queue_roundtrip(client: SynapClient) -> None:
    name = f"test-q-{_uid()}"
    await client.queue.create_queue(name, max_size=100, message_ttl=60)

    msg_id = await client.queue.publish(name, {"data": "hello"}, priority=5)
    assert isinstance(msg_id, str)
    assert len(msg_id) > 0

    msg = await client.queue.consume(name, consumer_id="worker-1")
    assert msg is not None
    assert msg.payload == {"data": "hello"}
    assert msg.priority == 5

    await client.queue.ack(name, msg.id)


async def _queue_empty(client: SynapClient) -> None:
    name = f"test-q-empty-{_uid()}"
    await client.queue.create_queue(name)
    msg = await client.queue.consume(name, consumer_id="worker-1")
    assert msg is None


async def _queue_list(client: SynapClient) -> None:
    name = f"test-q-list-{_uid()}"
    await client.queue.create_queue(name)
    queues = await client.queue.list()
    assert isinstance(queues, list)
    assert name in queues


# ──────────────────────────────────────────────────────────────────────────────
# Stream tests
# ──────────────────────────────────────────────────────────────────────────────

class TestStreamParity:
    """Stream operations across transports."""

    @pytest.mark.asyncio
    async def test_create_publish_read_http(self) -> None:
        async with _http_client() as client:
            await _stream_roundtrip(client)

    @pytest.mark.asyncio
    async def test_create_publish_read_rpc(self) -> None:
        async with _rpc_client() as client:
            await _stream_roundtrip(client)

    @pytest.mark.asyncio
    async def test_create_publish_read_resp3(self) -> None:
        async with _resp3_client() as client:
            await _stream_roundtrip(client)

    @pytest.mark.asyncio
    async def test_list_rooms_http(self) -> None:
        async with _http_client() as client:
            await _stream_list(client)

    @pytest.mark.asyncio
    async def test_list_rooms_rpc(self) -> None:
        async with _rpc_client() as client:
            await _stream_list(client)


async def _stream_roundtrip(client: SynapClient) -> None:
    room = f"test-room-{_uid()}"
    await client.stream.create_room(room)

    off0 = await client.stream.publish(room, "user.created", {"userId": "u1"})
    off1 = await client.stream.publish(room, "user.updated", {"userId": "u1", "name": "Alice"})
    assert isinstance(off0, int)
    assert isinstance(off1, int)
    assert off1 > off0

    events = await client.stream.read(room, offset=0)
    assert len(events) >= 2
    assert events[0].event == "user.created"
    assert events[1].event == "user.updated"


async def _stream_list(client: SynapClient) -> None:
    room = f"test-room-list-{_uid()}"
    await client.stream.create_room(room)
    rooms = await client.stream.list_rooms()
    assert isinstance(rooms, list)
    assert room in rooms


# ──────────────────────────────────────────────────────────────────────────────
# Pub/Sub tests
# ──────────────────────────────────────────────────────────────────────────────

class TestPubSubParity:
    """Pub/Sub operations across transports."""

    @pytest.mark.asyncio
    async def test_publish_http(self) -> None:
        async with _http_client() as client:
            await _pubsub_publish(client)

    @pytest.mark.asyncio
    async def test_publish_rpc(self) -> None:
        async with _rpc_client() as client:
            await _pubsub_publish(client)

    @pytest.mark.asyncio
    async def test_publish_resp3(self) -> None:
        async with _resp3_client() as client:
            await _pubsub_publish(client)


async def _pubsub_publish(client: SynapClient) -> None:
    topic = f"test.pub.{_uid()}"
    result = await client.pubsub.publish(topic, {"msg": "hello"})
    assert isinstance(result, int)
    assert result >= 0


# ──────────────────────────────────────────────────────────────────────────────
# Transaction tests
# ──────────────────────────────────────────────────────────────────────────────

class TestTransactionParity:
    """Transaction operations across transports."""

    @pytest.mark.asyncio
    async def test_multi_exec_http(self) -> None:
        async with _http_client() as client:
            await _txn_roundtrip(client)

    @pytest.mark.asyncio
    async def test_multi_exec_rpc(self) -> None:
        async with _rpc_client() as client:
            await _txn_roundtrip(client)

    @pytest.mark.asyncio
    async def test_multi_discard_http(self) -> None:
        async with _http_client() as client:
            await _txn_discard(client)

    @pytest.mark.asyncio
    async def test_multi_discard_rpc(self) -> None:
        async with _rpc_client() as client:
            await _txn_discard(client)


async def _txn_roundtrip(client: SynapClient) -> None:
    client_id = f"txn-{_uid()}"
    key = f"tx:test:{_uid()}"

    await client.transaction.multi(client_id=client_id)
    await client.send_command("kv.set", {"key": key, "value": "txn-value", "client_id": client_id})
    result = await client.transaction.exec(client_id=client_id)

    assert result.get("success") is True
    value = await client.kv.get(key)
    assert value == "txn-value"


async def _txn_discard(client: SynapClient) -> None:
    client_id = f"txn-discard-{_uid()}"
    key = f"tx:discard:{_uid()}"

    await client.transaction.multi(client_id=client_id)
    await client.send_command("kv.set", {"key": key, "value": "should-not-exist", "client_id": client_id})
    result = await client.transaction.discard(client_id=client_id)

    assert result.get("success") is True
    value = await client.kv.get(key)
    assert value is None


# ──────────────────────────────────────────────────────────────────────────────
# Script tests
# ──────────────────────────────────────────────────────────────────────────────

class TestScriptParity:
    """Scripting operations across transports."""

    @pytest.mark.asyncio
    async def test_eval_http(self) -> None:
        async with _http_client() as client:
            await _script_eval(client)

    @pytest.mark.asyncio
    async def test_eval_rpc(self) -> None:
        async with _rpc_client() as client:
            await _script_eval(client)


async def _script_eval(client: SynapClient) -> None:
    # Simple script: return a constant
    response = await client.send_command(
        "script.eval",
        {"script": "return 42", "keys": [], "args": []},
    )
    assert response is not None


# ──────────────────────────────────────────────────────────────────────────────
# UnsupportedCommandError regression
# ──────────────────────────────────────────────────────────────────────────────

class TestUnsupportedCommandRegression:
    """Native transports raise UnsupportedCommandError for unmapped commands."""

    @pytest.mark.asyncio
    async def test_rpc_raises_for_unmapped_command(self) -> None:
        async with _rpc_client() as client:
            with pytest.raises(UnsupportedCommandError) as exc_info:
                await client.send_command("bitmap.setbit", {"key": "bm", "offset": 7, "value": 1})
            assert "bitmap.setbit" in str(exc_info.value)

    @pytest.mark.asyncio
    async def test_resp3_raises_for_unmapped_command(self) -> None:
        async with _resp3_client() as client:
            with pytest.raises(UnsupportedCommandError) as exc_info:
                await client.send_command("bitmap.setbit", {"key": "bm", "offset": 7, "value": 1})
            assert "bitmap.setbit" in str(exc_info.value)

    @pytest.mark.asyncio
    async def test_http_succeeds_for_bitmap(self) -> None:
        """HTTP transport routes everything through the server — no UnsupportedCommandError."""
        async with _http_client() as client:
            # Should not raise; may succeed or fail with a server error
            try:
                await client.send_command("bitmap.setbit", {"key": f"bm:{_uid()}", "offset": 7, "value": 1})
            except UnsupportedCommandError:
                pytest.fail("HTTP transport must not raise UnsupportedCommandError")
