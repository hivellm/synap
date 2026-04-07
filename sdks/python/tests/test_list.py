"""Tests for List Manager."""

from __future__ import annotations

from unittest.mock import AsyncMock, MagicMock

import pytest

from synap_sdk.modules.list import ListManager


@pytest.fixture
def mock_client() -> MagicMock:
    """Create a mock Synap client."""
    client = MagicMock()
    client.send_command = AsyncMock()
    return client


@pytest.fixture
def list_manager(mock_client: MagicMock) -> ListManager:
    """Create a ListManager instance."""
    return ListManager(mock_client)


@pytest.mark.asyncio
async def test_list_lpush(list_manager: ListManager, mock_client: MagicMock) -> None:
    """Test list lpush operation."""
    mock_client.send_command.return_value = {"length": 3}
    
    result = await list_manager.lpush("tasks", "task1", "task2", "task3")
    
    assert result == 3
    mock_client.send_command.assert_called_once_with(
        "list.lpush", {"key": "tasks", "values": ["task1", "task2", "task3"]}
    )


@pytest.mark.asyncio
async def test_list_rpush(list_manager: ListManager, mock_client: MagicMock) -> None:
    """Test list rpush operation."""
    mock_client.send_command.return_value = {"length": 3}
    
    result = await list_manager.rpush("tasks", "task1", "task2")
    
    assert result == 3
    mock_client.send_command.assert_called_once_with(
        "list.rpush", {"key": "tasks", "values": ["task1", "task2"]}
    )


@pytest.mark.asyncio
async def test_list_lpop(list_manager: ListManager, mock_client: MagicMock) -> None:
    """Test list lpop operation."""
    mock_client.send_command.return_value = {"values": ["task1"]}
    
    result = await list_manager.lpop("tasks")
    
    assert result == ["task1"]
    mock_client.send_command.assert_called_once_with(
        "list.lpop", {"key": "tasks", "count": 1}
    )


@pytest.mark.asyncio
async def test_list_rpop(list_manager: ListManager, mock_client: MagicMock) -> None:
    """Test list rpop operation."""
    mock_client.send_command.return_value = {"values": ["task3"]}
    
    result = await list_manager.rpop("tasks", count=2)
    
    assert result == ["task3"]
    mock_client.send_command.assert_called_once_with(
        "list.rpop", {"key": "tasks", "count": 2}
    )


@pytest.mark.asyncio
async def test_list_range(list_manager: ListManager, mock_client: MagicMock) -> None:
    """Test list range operation."""
    mock_client.send_command.return_value = {"values": ["task1", "task2", "task3"]}
    
    result = await list_manager.range("tasks", 0, -1)
    
    assert result == ["task1", "task2", "task3"]
    mock_client.send_command.assert_called_once_with(
        "list.range", {"key": "tasks", "start": 0, "stop": -1}
    )


@pytest.mark.asyncio
async def test_list_len(list_manager: ListManager, mock_client: MagicMock) -> None:
    """Test list len operation."""
    mock_client.send_command.return_value = {"length": 5}
    
    result = await list_manager.len("tasks")
    
    assert result == 5
    mock_client.send_command.assert_called_once_with("list.len", {"key": "tasks"})


@pytest.mark.asyncio
async def test_list_index(list_manager: ListManager, mock_client: MagicMock) -> None:
    """Test list index operation."""
    mock_client.send_command.return_value = {"value": "task2"}
    
    result = await list_manager.index("tasks", 1)
    
    assert result == "task2"
    mock_client.send_command.assert_called_once_with(
        "list.index", {"key": "tasks", "index": 1}
    )


@pytest.mark.asyncio
async def test_list_set(list_manager: ListManager, mock_client: MagicMock) -> None:
    """Test list set operation."""
    mock_client.send_command.return_value = {"success": True}
    
    result = await list_manager.set("tasks", 0, "new_task")
    
    assert result is True
    mock_client.send_command.assert_called_once_with(
        "list.set", {"key": "tasks", "index": 0, "value": "new_task"}
    )


@pytest.mark.asyncio
async def test_list_trim(list_manager: ListManager, mock_client: MagicMock) -> None:
    """Test list trim operation."""
    mock_client.send_command.return_value = {"success": True}
    
    result = await list_manager.trim("tasks", 0, 10)
    
    assert result is True
    mock_client.send_command.assert_called_once_with(
        "list.trim", {"key": "tasks", "start": 0, "stop": 10}
    )


@pytest.mark.asyncio
async def test_list_rem(list_manager: ListManager, mock_client: MagicMock) -> None:
    """Test list rem operation."""
    mock_client.send_command.return_value = {"removed": 2}
    
    result = await list_manager.rem("tasks", 0, "task1")
    
    assert result == 2
    mock_client.send_command.assert_called_once_with(
        "list.rem", {"key": "tasks", "count": 0, "value": "task1"}
    )


@pytest.mark.asyncio
async def test_list_insert(list_manager: ListManager, mock_client: MagicMock) -> None:
    """Test list insert operation."""
    mock_client.send_command.return_value = {"length": 5}
    
    result = await list_manager.insert("tasks", "BEFORE", "task2", "new_task")
    
    assert result == 5
    mock_client.send_command.assert_called_once_with(
        "list.insert",
        {"key": "tasks", "position": "before", "pivot": "task2", "value": "new_task"},
    )


@pytest.mark.asyncio
async def test_list_rpoplpush(list_manager: ListManager, mock_client: MagicMock) -> None:
    """Test list rpoplpush operation."""
    mock_client.send_command.return_value = {"value": "task3"}
    
    result = await list_manager.rpoplpush("source", "dest")
    
    assert result == "task3"
    mock_client.send_command.assert_called_once_with(
        "list.rpoplpush", {"source": "source", "destination": "dest"}
    )


@pytest.mark.asyncio
async def test_list_pos(list_manager: ListManager, mock_client: MagicMock) -> None:
    """Test list pos operation."""
    mock_client.send_command.return_value = {"position": 2}
    
    result = await list_manager.pos("tasks", "task3")
    
    assert result == 2
    mock_client.send_command.assert_called_once_with(
        "list.pos", {"key": "tasks", "element": "task3", "rank": 1}
    )


@pytest.mark.asyncio
async def test_list_lpushx(list_manager: ListManager, mock_client: MagicMock) -> None:
    """Test list lpushx operation."""
    mock_client.send_command.return_value = {"length": 4}
    
    result = await list_manager.lpushx("tasks", "task0")
    
    assert result == 4
    mock_client.send_command.assert_called_once_with(
        "list.lpushx", {"key": "tasks", "values": ["task0"]}
    )


@pytest.mark.asyncio
async def test_list_rpushx(list_manager: ListManager, mock_client: MagicMock) -> None:
    """Test list rpushx operation."""
    mock_client.send_command.return_value = {"length": 4}
    
    result = await list_manager.rpushx("tasks", "task4")
    
    assert result == 4
    mock_client.send_command.assert_called_once_with(
        "list.rpushx", {"key": "tasks", "values": ["task4"]}
    )

