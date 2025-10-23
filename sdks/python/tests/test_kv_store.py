"""Tests for KVStore."""

from unittest.mock import AsyncMock, MagicMock

import pytest

from synap_sdk.client import SynapClient
from synap_sdk.config import SynapConfig
from synap_sdk.modules.kv_store import KVStore


@pytest.fixture
def mock_client() -> SynapClient:
    """Create a mock client."""
    config = SynapConfig("http://localhost:15500")
    client = SynapClient(config)
    client.execute = AsyncMock()  # type: ignore[method-assign]
    return client


@pytest.fixture
def kv_store(mock_client: SynapClient) -> KVStore:
    """Create a KVStore instance."""
    return KVStore(mock_client)


@pytest.mark.asyncio
async def test_set_sends_correct_request(
    kv_store: KVStore,
    mock_client: SynapClient,
) -> None:
    """Test set sends correct request."""
    mock_client.execute.return_value = {}  # type: ignore[attr-defined]

    await kv_store.set("test-key", "test-value")

    mock_client.execute.assert_called_once()  # type: ignore[attr-defined]
    call_args = mock_client.execute.call_args  # type: ignore[attr-defined]
    assert call_args[0][0] == "kv.set"
    assert call_args[0][1] == "test-key"


@pytest.mark.asyncio
async def test_set_with_ttl(
    kv_store: KVStore,
    mock_client: SynapClient,
) -> None:
    """Test set with TTL."""
    mock_client.execute.return_value = {}  # type: ignore[attr-defined]

    await kv_store.set("test-key", "test-value", ttl=3600)

    call_args = mock_client.execute.call_args  # type: ignore[attr-defined]
    assert call_args[0][2]["ttl"] == 3600


@pytest.mark.asyncio
async def test_get_returns_value(
    kv_store: KVStore,
    mock_client: SynapClient,
) -> None:
    """Test get returns value."""
    mock_client.execute.return_value = {"value": "test-value"}  # type: ignore[attr-defined]

    result = await kv_store.get("test-key")

    assert result == "test-value"


@pytest.mark.asyncio
async def test_get_returns_none_when_not_found(
    kv_store: KVStore,
    mock_client: SynapClient,
) -> None:
    """Test get returns None when not found."""
    mock_client.execute.return_value = {}  # type: ignore[attr-defined]

    result = await kv_store.get("nonexistent-key")

    assert result is None


@pytest.mark.asyncio
async def test_delete_sends_correct_request(
    kv_store: KVStore,
    mock_client: SynapClient,
) -> None:
    """Test delete sends correct request."""
    mock_client.execute.return_value = {}  # type: ignore[attr-defined]

    await kv_store.delete("test-key")

    call_args = mock_client.execute.call_args  # type: ignore[attr-defined]
    assert call_args[0][0] == "kv.delete"
    assert call_args[0][1] == "test-key"


@pytest.mark.asyncio
async def test_exists_returns_true(
    kv_store: KVStore,
    mock_client: SynapClient,
) -> None:
    """Test exists returns True when key exists."""
    mock_client.execute.return_value = {"exists": True}  # type: ignore[attr-defined]

    result = await kv_store.exists("test-key")

    assert result is True


@pytest.mark.asyncio
async def test_incr_returns_new_value(
    kv_store: KVStore,
    mock_client: SynapClient,
) -> None:
    """Test incr returns new value."""
    mock_client.execute.return_value = {"value": 42}  # type: ignore[attr-defined]

    result = await kv_store.incr("counter", delta=5)

    assert result == 42


@pytest.mark.asyncio
async def test_decr_returns_new_value(
    kv_store: KVStore,
    mock_client: SynapClient,
) -> None:
    """Test decr returns new value."""
    mock_client.execute.return_value = {"value": 10}  # type: ignore[attr-defined]

    result = await kv_store.decr("counter", delta=3)

    assert result == 10


@pytest.mark.asyncio
async def test_scan_returns_keys(
    kv_store: KVStore,
    mock_client: SynapClient,
) -> None:
    """Test scan returns keys."""
    mock_client.execute.return_value = {"keys": ["user:1", "user:2", "user:3"]}  # type: ignore[attr-defined]

    result = await kv_store.scan("user:*", limit=100)

    assert result == ["user:1", "user:2", "user:3"]


@pytest.mark.asyncio
async def test_stats_returns_statistics(
    kv_store: KVStore,
    mock_client: SynapClient,
) -> None:
    """Test stats returns statistics."""
    mock_client.execute.return_value = {"total_keys": 100, "memory_usage": 1024}  # type: ignore[attr-defined]

    result = await kv_store.stats()

    assert result["total_keys"] == 100
    assert result["memory_usage"] == 1024
