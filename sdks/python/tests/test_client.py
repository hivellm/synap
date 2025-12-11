"""Tests for SynapClient."""

from unittest.mock import AsyncMock, MagicMock, patch

import httpx
import pytest

from synap_sdk.client import SynapClient
from synap_sdk.config import SynapConfig
from synap_sdk.exceptions import SynapException
from synap_sdk.modules.kv_store import KVStore
from synap_sdk.modules.pubsub import PubSubManager
from synap_sdk.modules.queue import QueueManager
from synap_sdk.modules.stream import StreamManager


@pytest.fixture
def config() -> SynapConfig:
    """Create a test configuration."""
    return SynapConfig.create("http://localhost:15500")


def test_client_initialization(config: SynapConfig) -> None:
    """Test client initialization."""
    client = SynapClient(config)

    assert client.config == config
    assert isinstance(client._http_client, httpx.AsyncClient)


def test_client_with_custom_http_client(config: SynapConfig) -> None:
    """Test client with custom HTTP client."""
    http_client = httpx.AsyncClient()
    client = SynapClient(config, http_client)

    assert client._http_client is http_client
    assert not client._owns_client


def test_kv_property_returns_kvstore(config: SynapConfig) -> None:
    """Test kv property returns KVStore."""
    client = SynapClient(config)

    kv = client.kv

    assert isinstance(kv, KVStore)
    assert client.kv is kv  # Same instance


def test_queue_property_returns_queue_manager(config: SynapConfig) -> None:
    """Test queue property returns QueueManager."""
    client = SynapClient(config)

    queue = client.queue

    assert isinstance(queue, QueueManager)
    assert client.queue is queue


def test_stream_property_returns_stream_manager(config: SynapConfig) -> None:
    """Test stream property returns StreamManager."""
    client = SynapClient(config)

    stream = client.stream

    assert isinstance(stream, StreamManager)
    assert client.stream is stream


def test_pubsub_property_returns_pubsub_manager(config: SynapConfig) -> None:
    """Test pubsub property returns PubSubManager."""
    client = SynapClient(config)

    pubsub = client.pubsub

    assert isinstance(pubsub, PubSubManager)
    assert client.pubsub is pubsub


@pytest.mark.asyncio
async def test_execute_sends_correct_request(config: SynapConfig) -> None:
    """Test execute sends correct request."""
    mock_response = MagicMock()
    mock_response.text = '{"result": "success"}'
    mock_response.is_success = True
    mock_response.json.return_value = {"result": "success"}

    with patch.object(httpx.AsyncClient, "post", new_callable=AsyncMock) as mock_post:
        mock_post.return_value = mock_response

        async with SynapClient(config) as client:
            result = await client.execute("kv.set", "test-key", {"value": "test"})

            assert result == {"result": "success"}
            mock_post.assert_called_once()


@pytest.mark.asyncio
async def test_execute_handles_empty_response(config: SynapConfig) -> None:
    """Test execute handles empty response."""
    mock_response = MagicMock()
    mock_response.text = ""
    mock_response.is_success = True

    with patch.object(httpx.AsyncClient, "post", new_callable=AsyncMock) as mock_post:
        mock_post.return_value = mock_response

        async with SynapClient(config) as client:
            result = await client.execute("kv.get", "test-key")

            assert result == {}


@pytest.mark.asyncio
async def test_execute_raises_on_server_error(config: SynapConfig) -> None:
    """Test execute raises on server error."""
    mock_response = MagicMock()
    mock_response.text = '{"error": "Server error"}'
    mock_response.is_success = True
    mock_response.json.return_value = {"error": "Server error"}

    with patch.object(httpx.AsyncClient, "post", new_callable=AsyncMock) as mock_post:
        mock_post.return_value = mock_response

        async with SynapClient(config) as client:
            with pytest.raises(SynapException, match="Server Error"):
                await client.execute("kv.set", "test-key")


@pytest.mark.asyncio
async def test_execute_raises_on_invalid_json(config: SynapConfig) -> None:
    """Test execute raises on invalid JSON."""
    mock_response = MagicMock()
    mock_response.text = "invalid json"
    mock_response.is_success = True
    mock_response.json.side_effect = ValueError("Invalid JSON")

    with patch.object(httpx.AsyncClient, "post", new_callable=AsyncMock) as mock_post:
        mock_post.return_value = mock_response

        async with SynapClient(config) as client:
            with pytest.raises(SynapException, match="Invalid Response"):
                await client.execute("kv.set", "test-key")


@pytest.mark.asyncio
async def test_execute_raises_on_http_error(config: SynapConfig) -> None:
    """Test execute raises on HTTP error."""
    mock_response = MagicMock()
    mock_response.text = '{"error": "Not found"}'
    mock_response.is_success = False
    mock_response.status_code = 404
    mock_response.json.return_value = {"error": "Not found"}

    with patch.object(httpx.AsyncClient, "post", new_callable=AsyncMock) as mock_post:
        mock_post.return_value = mock_response

        async with SynapClient(config) as client:
            with pytest.raises(SynapException, match="Server Error"):
                await client.execute("kv.set", "test-key")


@pytest.mark.asyncio
async def test_execute_raises_on_network_error(config: SynapConfig) -> None:
    """Test execute raises on network error."""
    with patch.object(httpx.AsyncClient, "post", new_callable=AsyncMock) as mock_post:
        mock_post.side_effect = httpx.ConnectError("Connection refused")

        async with SynapClient(config) as client:
            with pytest.raises(SynapException, match="Network Error"):
                await client.execute("kv.set", "test-key")


@pytest.mark.asyncio
async def test_context_manager_closes_client(config: SynapConfig) -> None:
    """Test context manager closes client."""
    async with SynapClient(config) as client:
        assert client._http_client is not None

    # Client should be closed after context exit


@pytest.mark.asyncio
async def test_close_method(config: SynapConfig) -> None:
    """Test close method."""
    client = SynapClient(config)

    await client.close()

    # Should not raise


@pytest.mark.asyncio
async def test_close_does_not_close_custom_client(config: SynapConfig) -> None:
    """Test close does not close custom HTTP client."""
    http_client = httpx.AsyncClient()
    client = SynapClient(config, http_client)

    await client.close()

    # Custom client should not be closed by SynapClient
    assert http_client.is_closed is False
    await http_client.aclose()
