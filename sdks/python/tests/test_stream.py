"""Tests for StreamManager."""

from unittest.mock import AsyncMock, MagicMock

import pytest

from synap_sdk.modules.stream import StreamManager


@pytest.fixture
def mock_client() -> MagicMock:
    """Create a mock client."""
    client = MagicMock()
    client.send_command = AsyncMock()
    client.synap_rpc_transport = MagicMock(return_value=None)
    return client


@pytest.fixture
def stream_manager(mock_client: MagicMock) -> StreamManager:
    """Create a StreamManager instance."""
    return StreamManager(mock_client)


@pytest.mark.asyncio
async def test_create_room(
    stream_manager: StreamManager,
    mock_client: MagicMock,
) -> None:
    """Test create_room sends correct request."""
    mock_client.send_command.return_value = {}

    await stream_manager.create_room("test-room")

    mock_client.send_command.assert_called_once_with(
        "stream.create", {"room": "test-room"}
    )


@pytest.mark.asyncio
async def test_publish_returns_offset(
    stream_manager: StreamManager,
    mock_client: MagicMock,
) -> None:
    """Test publish returns offset."""
    mock_client.send_command.return_value = {"offset": 42}

    offset = await stream_manager.publish("test-room", "user.created", {"userId": "123"})

    assert offset == 42
    mock_client.send_command.assert_called_once_with(
        "stream.publish",
        {"room": "test-room", "event": "user.created", "data": {"userId": "123"}},
    )


@pytest.mark.asyncio
async def test_read_returns_events(
    stream_manager: StreamManager,
    mock_client: MagicMock,
) -> None:
    """Test read returns events."""
    mock_client.send_command.return_value = {
        "events": [
            {
                "offset": 0,
                "event": "user.created",
                "data": {"userId": "123"},
                "timestamp": 1234567890,
                "room": "test-room",
            },
            {
                "offset": 1,
                "event": "user.updated",
                "data": {"userId": "123", "name": "Alice"},
                "timestamp": 1234567891,
                "room": "test-room",
            },
        ]
    }

    events = await stream_manager.read("test-room", offset=0, limit=10)

    assert len(events) == 2
    assert events[0].event == "user.created"
    assert events[0].offset == 0
    assert events[1].event == "user.updated"
    assert events[1].offset == 1
    mock_client.send_command.assert_called_once_with(
        "stream.consume",
        {"room": "test-room", "subscriber_id": "sdk-reader", "from_offset": 0},
    )


@pytest.mark.asyncio
async def test_list_rooms_returns_rooms(
    stream_manager: StreamManager,
    mock_client: MagicMock,
) -> None:
    """Test list_rooms returns rooms."""
    mock_client.send_command.return_value = {"rooms": ["room1", "room2", "room3"]}

    rooms = await stream_manager.list_rooms()

    assert rooms == ["room1", "room2", "room3"]
    mock_client.send_command.assert_called_once_with("stream.list", {})
