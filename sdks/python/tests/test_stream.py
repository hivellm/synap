"""Tests for StreamManager."""

from unittest.mock import AsyncMock

import pytest

from synap_sdk.client import SynapClient
from synap_sdk.config import SynapConfig
from synap_sdk.modules.stream import StreamManager


@pytest.fixture
def mock_client() -> SynapClient:
    """Create a mock client."""
    config = SynapConfig("http://localhost:15500")
    client = SynapClient(config)
    client.execute = AsyncMock()  # type: ignore[method-assign]
    return client


@pytest.fixture
def stream_manager(mock_client: SynapClient) -> StreamManager:
    """Create a StreamManager instance."""
    return StreamManager(mock_client)


@pytest.mark.asyncio
async def test_create_room(
    stream_manager: StreamManager,
    mock_client: SynapClient,
) -> None:
    """Test create_room sends correct request."""
    mock_client.execute.return_value = {}  # type: ignore[attr-defined]

    await stream_manager.create_room("test-room")

    mock_client.execute.assert_called_once()  # type: ignore[attr-defined]


@pytest.mark.asyncio
async def test_publish_returns_offset(
    stream_manager: StreamManager,
    mock_client: SynapClient,
) -> None:
    """Test publish returns offset."""
    mock_client.execute.return_value = {"offset": 42}  # type: ignore[attr-defined]

    offset = await stream_manager.publish("test-room", "user.created", {"userId": "123"})

    assert offset == 42


@pytest.mark.asyncio
async def test_read_returns_events(
    stream_manager: StreamManager,
    mock_client: SynapClient,
) -> None:
    """Test read returns events."""
    mock_client.execute.return_value = {  # type: ignore[attr-defined]
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


@pytest.mark.asyncio
async def test_list_rooms_returns_rooms(
    stream_manager: StreamManager,
    mock_client: SynapClient,
) -> None:
    """Test list_rooms returns rooms."""
    mock_client.execute.return_value = {"rooms": ["room1", "room2", "room3"]}  # type: ignore[attr-defined]

    rooms = await stream_manager.list_rooms()

    assert rooms == ["room1", "room2", "room3"]
