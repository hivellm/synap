"""Pub/Sub Server-to-Server integration tests.

These tests require a running Synap server on localhost:15500.
Run with: SYNAP_S2S=true pytest tests/test_pubsub_s2s.py
"""

from __future__ import annotations

import os
import time
from typing import Any

import pytest

from synap_sdk import SynapClient, SynapConfig

# Skip if S2S tests are disabled
pytestmark = pytest.mark.skipif(
    os.getenv("SYNAP_S2S") != "true",
    reason="S2S tests disabled (set SYNAP_S2S=true to enable)",
)


@pytest.fixture
async def client() -> SynapClient:
    """Create a real Synap client connected to server."""
    config = SynapConfig(url=os.getenv("SYNAP_URL", "http://localhost:15500"))
    return SynapClient(config)


@pytest.mark.asyncio
async def test_publish_message_to_topic(client: SynapClient) -> None:
    """Test publishing a message to a topic."""
    topic = f"test.publish.{int(time.time() * 1000)}"
    message = {"event": "test", "data": "test-data"}

    result = await client.pubsub.publish(topic, message)

    # Should succeed even with 0 subscribers
    assert isinstance(result, int)
    assert result >= 0


@pytest.mark.asyncio
async def test_publish_to_multiple_topics(client: SynapClient) -> None:
    """Test publishing to multiple different topics."""
    topics = [
        f"test.user.created.{int(time.time() * 1000)}",
        f"test.user.updated.{int(time.time() * 1000) + 1}",
        f"test.user.deleted.{int(time.time() * 1000) + 2}",
    ]

    for topic in topics:
        result = await client.pubsub.publish(topic, {"topic": topic})
        assert isinstance(result, int)


@pytest.mark.asyncio
async def test_publish_different_payload_types(client: SynapClient) -> None:
    """Test publishing different types of payloads."""
    topic = f"test.types.{int(time.time() * 1000)}"

    # String payload
    result = await client.pubsub.publish(topic, "string message")
    assert isinstance(result, int)

    # Number payload
    result = await client.pubsub.publish(topic, 12345)
    assert isinstance(result, int)

    # Dict payload
    result = await client.pubsub.publish(topic, {"key": "value"})
    assert isinstance(result, int)

    # List payload
    result = await client.pubsub.publish(topic, [1, 2, 3])
    assert isinstance(result, int)

    # None payload
    result = await client.pubsub.publish(topic, None)
    assert isinstance(result, int)


@pytest.mark.asyncio
async def test_publish_nested_objects(client: SynapClient) -> None:
    """Test publishing nested object structures."""
    topic = f"test.nested.{int(time.time() * 1000)}"
    message = {
        "user": {
            "id": 123,
            "profile": {
                "name": "Alice",
                "settings": {"theme": "dark", "notifications": True},
            },
        },
        "timestamp": time.time(),
    }

    result = await client.pubsub.publish(topic, message)
    assert isinstance(result, int)


@pytest.mark.asyncio
async def test_publish_large_payload(client: SynapClient) -> None:
    """Test publishing large payloads."""
    topic = f"test.large.{int(time.time() * 1000)}"
    large_data = "x" * 50000  # 50KB
    message = {"data": large_data}

    result = await client.pubsub.publish(topic, message)
    assert isinstance(result, int)


@pytest.mark.asyncio
async def test_rapid_publishing(client: SynapClient) -> None:
    """Test rapid message publishing."""
    topic = f"test.rapid.{int(time.time() * 1000)}"
    message_count = 50

    results = []
    for i in range(message_count):
        result = await client.pubsub.publish(topic, {"id": i})
        results.append(result)

    assert len(results) == message_count
    for result in results:
        assert isinstance(result, int)


@pytest.mark.asyncio
async def test_publish_special_characters(client: SynapClient) -> None:
    """Test topic names with special characters."""
    topic = f"test.special-chars_123.{int(time.time() * 1000)}"
    message = {"test": "data"}

    result = await client.pubsub.publish(topic, message)
    assert isinstance(result, int)

