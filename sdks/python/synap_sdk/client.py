"""Main Synap client."""

from __future__ import annotations

from typing import Any

import httpx

from synap_sdk.config import SynapConfig
from synap_sdk.exceptions import SynapException
from synap_sdk.modules.kv_store import KVStore
from synap_sdk.modules.pubsub import PubSubManager
from synap_sdk.modules.queue import QueueManager
from synap_sdk.modules.stream import StreamManager


class SynapClient:
    """Main Synap SDK client for interacting with the Synap server.

    Args:
        config: The client configuration
        http_client: Optional custom HTTP client

    Example:
        >>> config = SynapConfig("http://localhost:15500")
        >>> async with SynapClient(config) as client:
        ...     await client.kv.set("key", "value")
        ...     value = await client.kv.get("key")
    """

    def __init__(
        self,
        config: SynapConfig,
        http_client: httpx.AsyncClient | None = None,
    ) -> None:
        """Initialize a new SynapClient."""
        self._config = config
        self._owns_client = http_client is None

        if http_client is not None:
            self._http_client = http_client
        else:
            headers = {"Accept": "application/json"}
            if config.auth_token:
                headers["Authorization"] = f"Bearer {config.auth_token}"

            self._http_client = httpx.AsyncClient(
                base_url=config.base_url,
                timeout=config.timeout,
                headers=headers,
            )

        self._kv: KVStore | None = None
        self._queue: QueueManager | None = None
        self._stream: StreamManager | None = None
        self._pubsub: PubSubManager | None = None

    @property
    def kv(self) -> KVStore:
        """Get the Key-Value Store operations."""
        if self._kv is None:
            self._kv = KVStore(self)
        return self._kv

    @property
    def queue(self) -> QueueManager:
        """Get the Queue operations."""
        if self._queue is None:
            self._queue = QueueManager(self)
        return self._queue

    @property
    def stream(self) -> StreamManager:
        """Get the Stream operations."""
        if self._stream is None:
            self._stream = StreamManager(self)
        return self._stream

    @property
    def pubsub(self) -> PubSubManager:
        """Get the Pub/Sub operations."""
        if self._pubsub is None:
            self._pubsub = PubSubManager(self)
        return self._pubsub

    @property
    def config(self) -> SynapConfig:
        """Get the client configuration."""
        return self._config

    async def execute(
        self,
        operation: str,
        target: str,
        data: dict[str, Any] | None = None,
    ) -> dict[str, Any]:
        """Execute a StreamableHTTP operation on the Synap server.

        Args:
            operation: The operation type (e.g., 'kv.set', 'queue.publish')
            target: The target resource (e.g., key name, queue name)
            data: The operation data

        Returns:
            The response as a dictionary

        Raises:
            SynapException: If the operation fails
        """
        try:
            payload = {
                "operation": operation,
                "target": target,
                "data": data or {},
            }

            response = await self._http_client.post("/api/stream", json=payload)

            if not response.text:
                return {}

            try:
                result = response.json()
            except Exception as e:
                raise SynapException.invalid_response(f"Failed to parse JSON response: {e}") from e

            # Check for server error in response
            if isinstance(result, dict) and "error" in result:
                raise SynapException.server_error(str(result["error"]))

            if not response.is_success:
                raise SynapException.http_error(
                    f"Request failed with status {response.status_code}",
                    response.status_code,
                )

            return result if isinstance(result, dict) else {}

        except httpx.HTTPError as e:
            raise SynapException.network_error(str(e)) from e

    async def __aenter__(self) -> SynapClient:
        """Enter async context manager."""
        return self

    async def __aexit__(self, *args: Any) -> None:
        """Exit async context manager."""
        await self.close()

    async def close(self) -> None:
        """Close the HTTP client if we own it."""
        if self._owns_client:
            await self._http_client.aclose()
