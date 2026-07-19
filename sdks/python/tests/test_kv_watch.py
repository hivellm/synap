"""Tests for KV watch (envelope decode, iteration, unwatch, modes)."""

from __future__ import annotations

import asyncio
import contextlib
from unittest.mock import AsyncMock, MagicMock

import pytest

from synap_sdk.exceptions import SynapException
from synap_sdk.modules.kv_store import KVStore
from synap_sdk.types import WatchEvent


class FakeRpc:
    """Records watch_push calls and lets tests drive envelopes in."""

    def __init__(self) -> None:
        self.pattern: str | None = None
        self.mode: str | None = None
        self.on_event = None
        self.cancelled = False

    async def watch_push(self, pattern, mode, on_event):  # noqa: ANN001, ANN201
        self.pattern = pattern
        self.mode = mode
        self.on_event = on_event

        def cancel() -> None:
            self.cancelled = True

        return "sub-1", cancel


@pytest.fixture
def rpc() -> FakeRpc:
    return FakeRpc()


@pytest.fixture
def kv(rpc: FakeRpc) -> KVStore:
    client = MagicMock()
    client.send_command = AsyncMock()
    client.synap_rpc_transport = MagicMock(return_value=rpc)
    return KVStore(client)


@pytest.mark.asyncio
async def test_watch_decodes_envelopes(kv: KVStore, rpc: FakeRpc) -> None:
    """Envelopes become WatchEvent instances with defaults for omitted fields."""
    iterator = kv.watch("user:1")
    first = asyncio.ensure_future(anext(iterator))
    await asyncio.sleep(0)  # let watch() register the push callback

    rpc.on_event({"key": "user:1", "event": "set", "version": 1, "value": "alice"})
    event = await first

    assert event == WatchEvent(key="user:1", event="set", version=1, value="alice")
    assert event.truncated is False

    second = asyncio.ensure_future(anext(iterator))
    await asyncio.sleep(0)
    rpc.on_event({"key": "user:1", "event": "del", "version": 2})
    event = await second

    assert event.value is None
    assert event.event == "del"

    await iterator.aclose()


@pytest.mark.asyncio
async def test_watch_passes_pattern_and_default_mode(kv: KVStore, rpc: FakeRpc) -> None:
    iterator = kv.watch("user:*")
    task = asyncio.ensure_future(anext(iterator))
    await asyncio.sleep(0)

    assert rpc.pattern == "user:*"
    assert rpc.mode == "value"

    task.cancel()
    with contextlib.suppress(asyncio.CancelledError):
        await task
    await iterator.aclose()


@pytest.mark.asyncio
async def test_watch_passes_notify_mode(kv: KVStore, rpc: FakeRpc) -> None:
    iterator = kv.watch("user:*", mode="notify")
    task = asyncio.ensure_future(anext(iterator))
    await asyncio.sleep(0)

    assert rpc.mode == "notify"

    task.cancel()
    with contextlib.suppress(asyncio.CancelledError):
        await task
    await iterator.aclose()


@pytest.mark.asyncio
async def test_closing_the_iterator_cancels_the_watch(kv: KVStore, rpc: FakeRpc) -> None:
    """aclose() must run the finally block, which issues KV.UNWATCH."""
    iterator = kv.watch("k")
    task = asyncio.ensure_future(anext(iterator))
    await asyncio.sleep(0)
    assert rpc.cancelled is False

    task.cancel()
    with contextlib.suppress(asyncio.CancelledError):
        await task
    await iterator.aclose()

    assert rpc.cancelled is True


@pytest.mark.asyncio
async def test_truncated_envelope_keeps_the_flag(kv: KVStore, rpc: FakeRpc) -> None:
    iterator = kv.watch("big")
    task = asyncio.ensure_future(anext(iterator))
    await asyncio.sleep(0)

    rpc.on_event({"key": "big", "event": "set", "version": 1, "truncated": True})
    event = await task

    assert event.truncated is True
    assert event.value is None

    await iterator.aclose()


@pytest.mark.asyncio
async def test_watch_requires_the_rpc_transport() -> None:
    client = MagicMock()
    client.synap_rpc_transport = MagicMock(return_value=None)
    kv = KVStore(client)

    with pytest.raises(SynapException, match="synap://"):
        await anext(kv.watch("k"))
