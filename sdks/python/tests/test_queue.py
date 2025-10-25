"""Tests for QueueManager."""

from unittest.mock import AsyncMock

import pytest

from synap_sdk.client import SynapClient
from synap_sdk.config import SynapConfig
from synap_sdk.modules.queue import QueueManager


@pytest.fixture
def mock_client() -> SynapClient:
    """Create a mock client."""
    config = SynapConfig("http://localhost:15500")
    client = SynapClient(config)
    client.execute = AsyncMock()  # type: ignore[method-assign]
    return client


@pytest.fixture
def queue_manager(mock_client: SynapClient) -> QueueManager:
    """Create a QueueManager instance."""
    return QueueManager(mock_client)


@pytest.mark.asyncio
async def test_create_queue(
    queue_manager: QueueManager,
    mock_client: SynapClient,
) -> None:
    """Test create_queue sends correct request."""
    mock_client.execute.return_value = {}  # type: ignore[attr-defined]

    await queue_manager.create_queue("test-queue", max_size=1000, message_ttl=3600)

    mock_client.execute.assert_called_once()  # type: ignore[attr-defined]


@pytest.mark.asyncio
async def test_publish_returns_message_id(
    queue_manager: QueueManager,
    mock_client: SynapClient,
) -> None:
    """Test publish returns message ID."""
    mock_client.execute.return_value = {"message_id": "msg-123"}  # type: ignore[attr-defined]

    message_id = await queue_manager.publish("test-queue", {"data": "test"}, priority=9)

    assert message_id == "msg-123"


@pytest.mark.asyncio
async def test_consume_returns_message(
    queue_manager: QueueManager,
    mock_client: SynapClient,
) -> None:
    """Test consume returns message."""
    mock_client.execute.return_value = {  # type: ignore[attr-defined]
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


@pytest.mark.asyncio
async def test_consume_returns_none_when_no_message(
    queue_manager: QueueManager,
    mock_client: SynapClient,
) -> None:
    """Test consume returns None when no message."""
    mock_client.execute.return_value = {}  # type: ignore[attr-defined]

    message = await queue_manager.consume("test-queue", "worker-1")

    assert message is None


@pytest.mark.asyncio
async def test_ack_sends_correct_request(
    queue_manager: QueueManager,
    mock_client: SynapClient,
) -> None:
    """Test ack sends correct request."""
    mock_client.execute.return_value = {}  # type: ignore[attr-defined]

    await queue_manager.ack("test-queue", "msg-123")

    call_args = mock_client.execute.call_args  # type: ignore[attr-defined]
    assert call_args[0][0] == "queue.ack"
    assert call_args[0][1] == "test-queue"


@pytest.mark.asyncio
async def test_list_returns_queues(
    queue_manager: QueueManager,
    mock_client: SynapClient,
) -> None:
    """Test list returns queues."""
    mock_client.execute.return_value = {"queues": ["queue1", "queue2", "queue3"]}  # type: ignore[attr-defined]

    queues = await queue_manager.list()

    assert queues == ["queue1", "queue2", "queue3"]
