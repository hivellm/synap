"""Tests for PubSubManager."""

from unittest.mock import AsyncMock

import pytest

from synap_sdk.client import SynapClient
from synap_sdk.config import SynapConfig
from synap_sdk.modules.pubsub import PubSubManager


@pytest.fixture
def mock_client() -> SynapClient:
    """Create a mock client."""
    config = SynapConfig("http://localhost:15500")
    client = SynapClient(config)
    client.execute = AsyncMock()  # type: ignore[method-assign]
    return client


@pytest.fixture
def pubsub_manager(mock_client: SynapClient) -> PubSubManager:
    """Create a PubSubManager instance."""
    return PubSubManager(mock_client)


@pytest.mark.asyncio
async def test_subscribe_topics(
    pubsub_manager: PubSubManager,
    mock_client: SynapClient,
) -> None:
    """Test subscribe_topics sends correct request."""
    mock_client.execute.return_value = {}  # type: ignore[attr-defined]

    await pubsub_manager.subscribe_topics("subscriber-1", ["notifications.*", "alerts.#"])

    mock_client.execute.assert_called_once()  # type: ignore[attr-defined]


@pytest.mark.asyncio
async def test_unsubscribe_topics(
    pubsub_manager: PubSubManager,
    mock_client: SynapClient,
) -> None:
    """Test unsubscribe_topics sends correct request."""
    mock_client.execute.return_value = {}  # type: ignore[attr-defined]

    await pubsub_manager.unsubscribe_topics("subscriber-1", ["notifications.*"])

    mock_client.execute.assert_called_once()  # type: ignore[attr-defined]


@pytest.mark.asyncio
async def test_publish_returns_delivered_count(
    pubsub_manager: PubSubManager,
    mock_client: SynapClient,
) -> None:
    """Test publish returns delivered count."""
    mock_client.execute.return_value = {"delivered": 5}  # type: ignore[attr-defined]

    delivered = await pubsub_manager.publish("notifications.email", {"to": "user@example.com"})

    assert delivered == 5


@pytest.mark.asyncio
async def test_stats_returns_statistics(
    pubsub_manager: PubSubManager,
    mock_client: SynapClient,
) -> None:
    """Test stats returns statistics."""
    mock_client.execute.return_value = {"total_subscribers": 10, "total_topics": 25}  # type: ignore[attr-defined]

    stats = await pubsub_manager.stats()

    assert stats["total_subscribers"] == 10
    assert stats["total_topics"] == 25
