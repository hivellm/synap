"""Tests for Pub/Sub operations."""

from __future__ import annotations

import pytest
from unittest.mock import AsyncMock, MagicMock

from synap_sdk.modules.pubsub import PubSubManager


@pytest.fixture
def mock_client() -> MagicMock:
    """Create a mock SynapClient."""
    client = MagicMock()
    client.send_command = AsyncMock()
    client.synap_rpc_transport = MagicMock(return_value=None)
    return client


@pytest.fixture
def pubsub_manager(mock_client: MagicMock) -> PubSubManager:
    """Create a PubSubManager with mock client."""
    return PubSubManager(mock_client)


@pytest.mark.asyncio
async def test_publish_sends_correct_payload(
    pubsub_manager: PubSubManager, mock_client: MagicMock
) -> None:
    """Test that publish sends correct payload to server."""
    mock_client.send_command.return_value = {"subscribers_matched": 2}

    topic = "test.topic"
    message = {"event": "test", "data": "test-data"}

    result = await pubsub_manager.publish(topic, message)

    mock_client.send_command.assert_called_once_with(
        "pubsub.publish",
        {"topic": topic, "payload": message},
    )
    assert result == 2


@pytest.mark.asyncio
async def test_publish_returns_subscriber_count(
    pubsub_manager: PubSubManager, mock_client: MagicMock
) -> None:
    """Test that publish returns number of subscribers matched."""
    mock_client.send_command.return_value = {"subscribers_matched": 5}

    result = await pubsub_manager.publish("topic", {"data": "test"})

    assert isinstance(result, int)
    assert result == 5


@pytest.mark.asyncio
async def test_publish_handles_zero_subscribers(
    pubsub_manager: PubSubManager, mock_client: MagicMock
) -> None:
    """Test that publish handles no subscribers gracefully."""
    mock_client.send_command.return_value = {"subscribers_matched": 0}

    result = await pubsub_manager.publish("topic", {"data": "test"})

    assert result == 0


@pytest.mark.asyncio
async def test_publish_with_different_payload_types(
    pubsub_manager: PubSubManager, mock_client: MagicMock
) -> None:
    """Test publishing different payload types."""
    mock_client.send_command.return_value = {"subscribers_matched": 1}

    # String payload
    await pubsub_manager.publish("topic", "string message")
    call_args = mock_client.send_command.call_args[0]
    assert call_args[1]["payload"] == "string message"

    # Number payload
    await pubsub_manager.publish("topic", 12345)
    call_args = mock_client.send_command.call_args[0]
    assert call_args[1]["payload"] == 12345

    # Dict payload
    await pubsub_manager.publish("topic", {"key": "value"})
    call_args = mock_client.send_command.call_args[0]
    assert call_args[1]["payload"] == {"key": "value"}

    # List payload
    await pubsub_manager.publish("topic", [1, 2, 3])
    call_args = mock_client.send_command.call_args[0]
    assert call_args[1]["payload"] == [1, 2, 3]

    # None payload
    await pubsub_manager.publish("topic", None)
    call_args = mock_client.send_command.call_args[0]
    assert call_args[1]["payload"] is None


@pytest.mark.asyncio
async def test_subscribe_topics_calls_server(
    pubsub_manager: PubSubManager, mock_client: MagicMock
) -> None:
    """Test that subscribe_topics calls server correctly."""
    mock_client.send_command.return_value = {}

    await pubsub_manager.subscribe_topics("sub-1", ["test.*", "user.*"])

    mock_client.send_command.assert_called_once_with(
        "pubsub.subscribe",
        {"topics": ["test.*", "user.*"], "subscriber_id": "sub-1"},
    )


@pytest.mark.asyncio
async def test_unsubscribe_topics_calls_server(
    pubsub_manager: PubSubManager, mock_client: MagicMock
) -> None:
    """Test that unsubscribe_topics calls server correctly."""
    mock_client.send_command.return_value = {}

    await pubsub_manager.unsubscribe_topics("sub-1", ["test.*"])

    mock_client.send_command.assert_called_once_with(
        "pubsub.unsubscribe",
        {"topics": ["test.*"], "subscriber_id": "sub-1"},
    )


@pytest.mark.asyncio
async def test_stats_calls_server(
    pubsub_manager: PubSubManager, mock_client: MagicMock
) -> None:
    """Test that stats calls server correctly."""
    expected_stats = {"total_topics": 10, "total_subscribers": 5}
    mock_client.send_command.return_value = expected_stats

    result = await pubsub_manager.stats()

    mock_client.send_command.assert_called_once_with("pubsub.stats", {})
    assert result == expected_stats
