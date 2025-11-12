"""Main Synap client."""

from __future__ import annotations

from typing import Any
import base64
import uuid

import httpx

from synap_sdk.config import SynapConfig
from synap_sdk.exceptions import SynapException
from synap_sdk.modules.kv_store import KVStore
from synap_sdk.modules.hash import HashManager
from synap_sdk.modules.list import ListManager
from synap_sdk.modules.set import SetManager
from synap_sdk.modules.pubsub import PubSubManager
from synap_sdk.modules.queue import QueueManager
from synap_sdk.modules.stream import StreamManager
from synap_sdk.modules.bitmap import BitmapManager
from synap_sdk.modules.hyperloglog import HyperLogLogManager
from synap_sdk.modules.geospatial import GeospatialManager
from synap_sdk.modules.transaction import TransactionManager


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
            
            # Add authentication headers
            if config.auth_token:
                headers["Authorization"] = f"Bearer {config.auth_token}"
            elif config.username and config.password:
                credentials = base64.b64encode(
                    f"{config.username}:{config.password}".encode()
                ).decode()
                headers["Authorization"] = f"Basic {credentials}"

            self._http_client = httpx.AsyncClient(
                base_url=config.base_url,
                timeout=config.timeout,
                headers=headers,
            )

        self._kv: KVStore | None = None
        self._hash: HashManager | None = None
        self._list: ListManager | None = None
        self._set: SetManager | None = None
        self._queue: QueueManager | None = None
        self._stream: StreamManager | None = None
        self._pubsub: PubSubManager | None = None
        self._bitmap: BitmapManager | None = None
        self._hyperloglog: HyperLogLogManager | None = None
        self._geospatial: GeospatialManager | None = None
        self._transaction: TransactionManager | None = None

    @property
    def kv(self) -> KVStore:
        """Get the Key-Value Store operations."""
        if self._kv is None:
            self._kv = KVStore(self)
        return self._kv

    @property
    def hash(self) -> HashManager:
        """Get the Hash data structure operations."""
        if self._hash is None:
            self._hash = HashManager(self)
        return self._hash

    @property
    def list(self) -> ListManager:
        """Get the List data structure operations."""
        if self._list is None:
            self._list = ListManager(self)
        return self._list

    @property
    def set(self) -> SetManager:
        """Get the Set data structure operations."""
        if self._set is None:
            self._set = SetManager(self)
        return self._set

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
    def bitmap(self) -> BitmapManager:
        """Get the Bitmap operations."""
        if self._bitmap is None:
            self._bitmap = BitmapManager(self)
        return self._bitmap

    @property
    def hyperloglog(self) -> HyperLogLogManager:
        """Get the HyperLogLog operations."""
        if self._hyperloglog is None:
            self._hyperloglog = HyperLogLogManager(self)
        return self._hyperloglog

    @property
    def geospatial(self) -> GeospatialManager:
        """Get the Geospatial operations."""
        if self._geospatial is None:
            self._geospatial = GeospatialManager(self)
        return self._geospatial

    @property
    def transaction(self) -> TransactionManager:
        """Get the Transaction operations."""
        if self._transaction is None:
            self._transaction = TransactionManager(self)
        return self._transaction

    @property
    def config(self) -> SynapConfig:
        """Get the client configuration."""
        return self._config

    async def send_command(
        self,
        command: str,
        payload: dict[str, Any] | None = None,
    ) -> dict[str, Any]:
        """Send a StreamableHTTP command to the Synap server.

        Args:
            command: The command name (e.g., 'geospatial.geoadd', 'bitmap.setbit')
            payload: The command payload data

        Returns:
            The response payload as a dictionary

        Raises:
            SynapException: If the operation fails
        """
        try:
            request_id = str(uuid.uuid4())
            request_payload = {
                "command": command,
                "request_id": request_id,
                "payload": payload or {},
            }

            response = await self._http_client.post("/api/v1/command", json=request_payload)

            if not response.text:
                return {}

            try:
                result = response.json()
            except Exception as e:
                raise SynapException.invalid_response(f"Failed to parse JSON response: {e}") from e

            # Check StreamableHTTP envelope
            if isinstance(result, dict) and not result.get("success", True):
                error_msg = result.get("error", "Unknown server error")
                raise SynapException.server_error(str(error_msg))

            if not response.is_success:
                raise SynapException.http_error(
                    f"Request failed with status {response.status_code}",
                    response.status_code,
                )

            # Extract payload from StreamableHTTP response
            return result.get("payload", {}) if isinstance(result, dict) else {}

        except httpx.HTTPError as e:
            raise SynapException.network_error(str(e)) from e

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
