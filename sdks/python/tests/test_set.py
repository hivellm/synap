"""Tests for Set Manager."""

from __future__ import annotations

from unittest.mock import AsyncMock, MagicMock

import pytest

from synap_sdk.modules.set import SetManager


@pytest.fixture
def mock_client() -> MagicMock:
    """Create a mock Synap client."""
    client = MagicMock()
    client.send_command = AsyncMock()
    return client


@pytest.fixture
def set_manager(mock_client: MagicMock) -> SetManager:
    """Create a SetManager instance."""
    return SetManager(mock_client)


@pytest.mark.asyncio
async def test_set_add(set_manager: SetManager, mock_client: MagicMock) -> None:
    """Test set add operation."""
    mock_client.send_command.return_value = {"added": 3}
    
    result = await set_manager.add("tags", "python", "redis", "typescript")
    
    assert result == 3
    mock_client.send_command.assert_called_once_with(
        "set.add", {"key": "tags", "members": ["python", "redis", "typescript"]}
    )


@pytest.mark.asyncio
async def test_set_rem(set_manager: SetManager, mock_client: MagicMock) -> None:
    """Test set rem operation."""
    mock_client.send_command.return_value = {"removed": 1}
    
    result = await set_manager.rem("tags", "typescript")
    
    assert result == 1
    mock_client.send_command.assert_called_once_with(
        "set.rem", {"key": "tags", "members": ["typescript"]}
    )


@pytest.mark.asyncio
async def test_set_is_member(set_manager: SetManager, mock_client: MagicMock) -> None:
    """Test set is_member operation."""
    mock_client.send_command.return_value = {"is_member": True}
    
    result = await set_manager.is_member("tags", "python")
    
    assert result is True
    mock_client.send_command.assert_called_once_with(
        "set.ismember", {"key": "tags", "member": "python"}
    )


@pytest.mark.asyncio
async def test_set_members(set_manager: SetManager, mock_client: MagicMock) -> None:
    """Test set members operation."""
    mock_client.send_command.return_value = {"members": ["python", "redis"]}
    
    result = await set_manager.members("tags")
    
    assert result == ["python", "redis"]
    mock_client.send_command.assert_called_once_with("set.members", {"key": "tags"})


@pytest.mark.asyncio
async def test_set_card(set_manager: SetManager, mock_client: MagicMock) -> None:
    """Test set card operation."""
    mock_client.send_command.return_value = {"cardinality": 3}
    
    result = await set_manager.card("tags")
    
    assert result == 3
    mock_client.send_command.assert_called_once_with("set.card", {"key": "tags"})


@pytest.mark.asyncio
async def test_set_pop(set_manager: SetManager, mock_client: MagicMock) -> None:
    """Test set pop operation."""
    mock_client.send_command.return_value = {"members": ["python"]}
    
    result = await set_manager.pop("tags", 1)
    
    assert result == ["python"]
    mock_client.send_command.assert_called_once_with(
        "set.pop", {"key": "tags", "count": 1}
    )


@pytest.mark.asyncio
async def test_set_rand_member(set_manager: SetManager, mock_client: MagicMock) -> None:
    """Test set rand_member operation."""
    mock_client.send_command.return_value = {"members": ["redis", "python"]}
    
    result = await set_manager.rand_member("tags", 2)
    
    assert result == ["redis", "python"]
    mock_client.send_command.assert_called_once_with(
        "set.randmember", {"key": "tags", "count": 2}
    )


@pytest.mark.asyncio
async def test_set_move(set_manager: SetManager, mock_client: MagicMock) -> None:
    """Test set move operation."""
    mock_client.send_command.return_value = {"moved": True}
    
    result = await set_manager.move("tags1", "tags2", "python")
    
    assert result is True
    mock_client.send_command.assert_called_once_with(
        "set.move", {"source": "tags1", "destination": "tags2", "member": "python"}
    )


@pytest.mark.asyncio
async def test_set_inter(set_manager: SetManager, mock_client: MagicMock) -> None:
    """Test set inter operation."""
    mock_client.send_command.return_value = {"members": ["python"]}
    
    result = await set_manager.inter("tags1", "tags2")
    
    assert result == ["python"]
    mock_client.send_command.assert_called_once_with(
        "set.inter", {"keys": ["tags1", "tags2"]}
    )


@pytest.mark.asyncio
async def test_set_union(set_manager: SetManager, mock_client: MagicMock) -> None:
    """Test set union operation."""
    mock_client.send_command.return_value = {"members": ["python", "redis", "typescript"]}
    
    result = await set_manager.union("tags1", "tags2")
    
    assert result == ["python", "redis", "typescript"]
    mock_client.send_command.assert_called_once_with(
        "set.union", {"keys": ["tags1", "tags2"]}
    )


@pytest.mark.asyncio
async def test_set_diff(set_manager: SetManager, mock_client: MagicMock) -> None:
    """Test set diff operation."""
    mock_client.send_command.return_value = {"members": ["redis"]}
    
    result = await set_manager.diff("tags1", "tags2")
    
    assert result == ["redis"]
    mock_client.send_command.assert_called_once_with(
        "set.diff", {"keys": ["tags1", "tags2"]}
    )


@pytest.mark.asyncio
async def test_set_inter_store(set_manager: SetManager, mock_client: MagicMock) -> None:
    """Test set inter_store operation."""
    mock_client.send_command.return_value = {"cardinality": 1}
    
    result = await set_manager.inter_store("result", "tags1", "tags2")
    
    assert result == 1
    mock_client.send_command.assert_called_once_with(
        "set.interstore", {"destination": "result", "keys": ["tags1", "tags2"]}
    )


@pytest.mark.asyncio
async def test_set_union_store(set_manager: SetManager, mock_client: MagicMock) -> None:
    """Test set union_store operation."""
    mock_client.send_command.return_value = {"cardinality": 5}
    
    result = await set_manager.union_store("result", "tags1", "tags2")
    
    assert result == 5
    mock_client.send_command.assert_called_once_with(
        "set.unionstore", {"destination": "result", "keys": ["tags1", "tags2"]}
    )


@pytest.mark.asyncio
async def test_set_diff_store(set_manager: SetManager, mock_client: MagicMock) -> None:
    """Test set diff_store operation."""
    mock_client.send_command.return_value = {"cardinality": 2}
    
    result = await set_manager.diff_store("result", "tags1", "tags2")
    
    assert result == 2
    mock_client.send_command.assert_called_once_with(
        "set.diffstore", {"destination": "result", "keys": ["tags1", "tags2"]}
    )

