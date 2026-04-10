"""Tests for QueueManager."""

from unittest.mock import AsyncMock, MagicMock

import pytest

from synap_sdk.modules.queue import QueueManager


@pytest.fixture
def mock_client() -> MagicMock:
    """Create a mock client."""
    client = MagicMock()
    client.send_command = AsyncMock()
    client.synap_rpc_transport = MagicMock(return_value=None)
    return client


@pytest.fixture
def queue_manager(mock_client: MagicMock) -> QueueManager:
    """Create a QueueManager instance."""
    return QueueManager(mock_client)


@pytest.mark.asyncio
async def test_create_queue(
    queue_manager: QueueManager,
    mock_client: MagicMock,
) -> None:
    """Test create_queue sends correct request."""
    mock_client.send_command.return_value = {}

    await queue_manager.create_queue("test-queue", max_size=1000, message_ttl=3600)

    mock_client.send_command.assert_called_once_with(
        "queue.create",
        {"name": "test-queue", "max_depth": 1000, "ack_deadline_secs": 3600},
    )


@pytest.mark.asyncio
async def test_publish_returns_message_id(
    queue_manager: QueueManager,
    mock_client: MagicMock,
) -> None:
    """Test publish returns message ID."""
    mock_client.send_command.return_value = {"message_id": "msg-123"}

    message_id = await queue_manager.publish("test-queue", {"data": "test"}, priority=9)

    assert message_id == "msg-123"
    mock_client.send_command.assert_called_once_with(
        "queue.publish",
        {"queue": "test-queue", "payload": {"data": "test"}, "priority": 9},
    )


@pytest.mark.asyncio
async def test_consume_returns_message(
    queue_manager: QueueManager,
    mock_client: MagicMock,
) -> None:
    """Test consume returns message."""
    mock_client.send_command.return_value = {
        "message": {
            "id": "msg-456",
            "payload": {"data": "test"},
            "priority": 5,
            "retries": 0,
            "max_retries": 3,
            "timestamp": 1234567890,
        }
    }

    message = await queue_manager.consume("test-queue", "worker-1")

    assert message is not None
    assert message.id == "msg-456"
    assert message.priority == 5
    assert message.retries == 0
    mock_client.send_command.assert_called_once_with(
        "queue.consume",
        {"queue": "test-queue", "consumer_id": "worker-1"},
    )


@pytest.mark.asyncio
async def test_consume_returns_none_when_no_message(
    queue_manager: QueueManager,
    mock_client: MagicMock,
) -> None:
    """Test consume returns None when no message."""
    mock_client.send_command.return_value = {}

    message = await queue_manager.consume("test-queue", "worker-1")

    assert message is None


@pytest.mark.asyncio
async def test_ack_sends_correct_request(
    queue_manager: QueueManager,
    mock_client: MagicMock,
) -> None:
    """Test ack sends correct request."""
    mock_client.send_command.return_value = {"success": True}

    await queue_manager.ack("test-queue", "msg-123")

    mock_client.send_command.assert_called_once_with(
        "queue.ack",
        {"queue": "test-queue", "message_id": "msg-123"},
    )


@pytest.mark.asyncio
async def test_list_returns_queues(
    queue_manager: QueueManager,
    mock_client: MagicMock,
) -> None:
    """Test list returns queues."""
    mock_client.send_command.return_value = {"queues": ["queue1", "queue2", "queue3"]}

    queues = await queue_manager.list()

    assert queues == ["queue1", "queue2", "queue3"]
    mock_client.send_command.assert_called_once_with("queue.list", {})
