"""Tests for Hash Manager."""

from __future__ import annotations

from unittest.mock import AsyncMock, MagicMock

import pytest

from synap_sdk.modules.hash import HashManager


@pytest.fixture
def mock_client() -> MagicMock:
    """Create a mock Synap client."""
    client = MagicMock()
    client.send_command = AsyncMock()
    return client


@pytest.fixture
def hash_manager(mock_client: MagicMock) -> HashManager:
    """Create a HashManager instance."""
    return HashManager(mock_client)


@pytest.mark.asyncio
async def test_hash_set(hash_manager: HashManager, mock_client: MagicMock) -> None:
    """Test hash set operation."""
    mock_client.send_command.return_value = {"success": True}
    
    result = await hash_manager.set("user:1", "name", "Alice")
    
    assert result is True
    mock_client.send_command.assert_called_once_with(
        "hash.set", {"key": "user:1", "field": "name", "value": "Alice"}
    )


@pytest.mark.asyncio
async def test_hash_get(hash_manager: HashManager, mock_client: MagicMock) -> None:
    """Test hash get operation."""
    mock_client.send_command.return_value = {"value": "Alice"}
    
    result = await hash_manager.get("user:1", "name")
    
    assert result == "Alice"
    mock_client.send_command.assert_called_once_with(
        "hash.get", {"key": "user:1", "field": "name"}
    )


@pytest.mark.asyncio
async def test_hash_get_all(hash_manager: HashManager, mock_client: MagicMock) -> None:
    """Test hash get_all operation."""
    mock_client.send_command.return_value = {"fields": {"name": "Alice", "age": "30"}}
    
    result = await hash_manager.get_all("user:1")
    
    assert result == {"name": "Alice", "age": "30"}
    mock_client.send_command.assert_called_once_with("hash.getall", {"key": "user:1"})


@pytest.mark.asyncio
async def test_hash_delete(hash_manager: HashManager, mock_client: MagicMock) -> None:
    """Test hash delete operation."""
    mock_client.send_command.return_value = {"deleted": 1}
    
    result = await hash_manager.delete("user:1", "name")
    
    assert result == 1
    mock_client.send_command.assert_called_once_with(
        "hash.del", {"key": "user:1", "field": "name"}
    )


@pytest.mark.asyncio
async def test_hash_exists(hash_manager: HashManager, mock_client: MagicMock) -> None:
    """Test hash exists operation."""
    mock_client.send_command.return_value = {"exists": True}
    
    result = await hash_manager.exists("user:1", "name")
    
    assert result is True
    mock_client.send_command.assert_called_once_with(
        "hash.exists", {"key": "user:1", "field": "name"}
    )


@pytest.mark.asyncio
async def test_hash_keys(hash_manager: HashManager, mock_client: MagicMock) -> None:
    """Test hash keys operation."""
    mock_client.send_command.return_value = {"fields": ["name", "age"]}
    
    result = await hash_manager.keys("user:1")
    
    assert result == ["name", "age"]
    mock_client.send_command.assert_called_once_with("hash.keys", {"key": "user:1"})


@pytest.mark.asyncio
async def test_hash_values(hash_manager: HashManager, mock_client: MagicMock) -> None:
    """Test hash values operation."""
    mock_client.send_command.return_value = {"values": ["Alice", "30"]}
    
    result = await hash_manager.values("user:1")
    
    assert result == ["Alice", "30"]
    mock_client.send_command.assert_called_once_with("hash.values", {"key": "user:1"})


@pytest.mark.asyncio
async def test_hash_len(hash_manager: HashManager, mock_client: MagicMock) -> None:
    """Test hash len operation."""
    mock_client.send_command.return_value = {"length": 2}
    
    result = await hash_manager.len("user:1")
    
    assert result == 2
    mock_client.send_command.assert_called_once_with("hash.len", {"key": "user:1"})


@pytest.mark.asyncio
async def test_hash_mset(hash_manager: HashManager, mock_client: MagicMock) -> None:
    """Test hash mset operation."""
    mock_client.send_command.return_value = {"success": True}
    
    result = await hash_manager.mset("user:1", {"name": "Alice", "age": 30})
    
    assert result is True
    mock_client.send_command.assert_called_once_with(
        "hash.mset", {"key": "user:1", "fields": {"name": "Alice", "age": "30"}}
    )


@pytest.mark.asyncio
async def test_hash_mget(hash_manager: HashManager, mock_client: MagicMock) -> None:
    """Test hash mget operation."""
    mock_client.send_command.return_value = {"values": {"name": "Alice", "age": "30"}}
    
    result = await hash_manager.mget("user:1", ["name", "age"])
    
    assert result == {"name": "Alice", "age": "30"}
    mock_client.send_command.assert_called_once_with(
        "hash.mget", {"key": "user:1", "fields": ["name", "age"]}
    )


@pytest.mark.asyncio
async def test_hash_incr_by(hash_manager: HashManager, mock_client: MagicMock) -> None:
    """Test hash incr_by operation."""
    mock_client.send_command.return_value = {"value": 5}
    
    result = await hash_manager.incr_by("counters", "visits", 1)
    
    assert result == 5
    mock_client.send_command.assert_called_once_with(
        "hash.incrby", {"key": "counters", "field": "visits", "increment": 1}
    )


@pytest.mark.asyncio
async def test_hash_incr_by_float(hash_manager: HashManager, mock_client: MagicMock) -> None:
    """Test hash incr_by_float operation."""
    mock_client.send_command.return_value = {"value": 3.14}
    
    result = await hash_manager.incr_by_float("metrics", "score", 0.5)
    
    assert result == 3.14
    mock_client.send_command.assert_called_once_with(
        "hash.incrbyfloat", {"key": "metrics", "field": "score", "increment": 0.5}
    )


@pytest.mark.asyncio
async def test_hash_set_nx(hash_manager: HashManager, mock_client: MagicMock) -> None:
    """Test hash set_nx operation."""
    mock_client.send_command.return_value = {"created": True}
    
    result = await hash_manager.set_nx("user:1", "email", "alice@example.com")
    
    assert result is True
    mock_client.send_command.assert_called_once_with(
        "hash.setnx", {"key": "user:1", "field": "email", "value": "alice@example.com"}
    )

