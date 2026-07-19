"""Key-Value Store operations."""

from __future__ import annotations

import asyncio
import contextlib
import json
from collections.abc import AsyncIterator
from typing import TYPE_CHECKING, Any, TypeVar

from synap_sdk.exceptions import SynapException
from synap_sdk.types import WatchEvent

if TYPE_CHECKING:
    from synap_sdk.client import SynapClient

T = TypeVar("T")


def _encode_value(value: Any) -> Any:  # noqa: ANN401
    """Encode a value for storage.

    The SDK contract is that non-string values round-trip through JSON, so
    dicts/lists/numbers/bools are JSON-encoded here while strings and raw bytes
    pass through untouched (mirrors the TypeScript SDK).
    """
    if isinstance(value, (str, bytes, bytearray)):
        return value
    return json.dumps(value)


def _field(response: Any, name: str, default: Any = None) -> Any:  # noqa: ANN401
    """Extract a named field from a command response.

    The native transports return the REST-shaped dict (e.g. ``{"value": ...}``)
    via ``map_response``; the HTTP command endpoint returns the raw payload
    directly (a bare string/number/list). Accept both.
    """
    if isinstance(response, dict):
        return response.get(name, default)
    return response if response is not None else default


def _decode_value(raw: Any) -> Any:  # noqa: ANN401
    """Auto-parse JSON-looking string values (inverse of :func:`_encode_value`)."""
    if isinstance(raw, str):
        try:
            return json.loads(raw)
        except (ValueError, TypeError):
            return raw
    return raw


class KVStore:
    """Key-Value Store operations.

    All commands are routed through :meth:`SynapClient.send_command`, which uses
    the native transport (SynapRPC/RESP3) when connected via ``synap://`` /
    ``resp3://`` and the HTTP command endpoint otherwise. (Previously this
    module talked to a legacy ``/api/stream`` endpoint the 1.0 server no longer
    serves, so every call silently returned empty results.)

    Example:
        >>> await client.kv.set("user:1", "John Doe")
        >>> value = await client.kv.get("user:1")
        >>> await client.kv.delete("user:1")
    """

    def __init__(self, client: SynapClient) -> None:
        """Initialize KVStore with a client."""
        self._client = client

    async def set(
        self,
        key: str,
        value: Any,
        ttl: int | None = None,
    ) -> None:
        """Set a key-value pair.

        Args:
            key: The key to set
            value: The value to store (non-strings are JSON-encoded)
            ttl: Optional time-to-live in seconds
        """
        payload: dict[str, Any] = {"key": key, "value": _encode_value(value)}
        if ttl is not None:
            payload["ttl"] = ttl
        await self._client.send_command("kv.set", payload)

    async def get(self, key: str) -> Any:
        """Get a value by key.

        Args:
            key: The key to get

        Returns:
            The value (JSON-looking strings are auto-parsed), or None if not found
        """
        response = await self._client.send_command("kv.get", {"key": key})
        return _decode_value(_field(response, "value"))

    async def delete(self, key: str) -> None:
        """Delete a key.

        Args:
            key: The key to delete
        """
        await self._client.send_command("kv.del", {"key": key})

    async def exists(self, key: str) -> bool:
        """Check if a key exists.

        Args:
            key: The key to check

        Returns:
            True if the key exists, False otherwise
        """
        response = await self._client.send_command("kv.exists", {"key": key})
        return bool(_field(response, "exists", False))

    async def incr(self, key: str, delta: int = 1) -> int:
        """Increment a numeric value.

        Args:
            key: The key to increment
            delta: The amount to increment by (default: 1)

        Returns:
            The new value after incrementing
        """
        if delta == 1:
            response = await self._client.send_command("kv.incr", {"key": key})
        else:
            response = await self._client.send_command(
                "kv.incrby", {"key": key, "amount": delta}
            )
        return int(_field(response, "value", 0))

    async def decr(self, key: str, delta: int = 1) -> int:
        """Decrement a numeric value.

        Args:
            key: The key to decrement
            delta: The amount to decrement by (default: 1)

        Returns:
            The new value after decrementing
        """
        if delta == 1:
            response = await self._client.send_command("kv.decr", {"key": key})
        else:
            response = await self._client.send_command(
                "kv.decrby", {"key": key, "amount": delta}
            )
        return int(_field(response, "value", 0))

    async def scan(self, prefix: str, limit: int = 100) -> list[str]:
        """Scan keys by prefix.

        Args:
            prefix: The prefix to search for
            limit: Maximum number of keys to return (default: 100)

        Returns:
            List of matching keys
        """
        pattern = f"{prefix}*" if prefix else "*"
        response = await self._client.send_command("kv.keys", {"pattern": pattern})
        keys = list(_field(response, "keys", []) or [])
        return keys[:limit]

    async def stats(self) -> dict[str, Any]:
        """Get KV store statistics.

        Returns:
            Statistics as a dictionary
        """
        return await self._client.send_command("kv.stats", {})

    async def watch(
        self,
        pattern: str,
        *,
        mode: str = "value",
        queue_size: int = 256,
    ) -> AsyncIterator[WatchEvent]:
        """Watch a key (or wildcard pattern) and yield its change events.

        Requires the SynapRPC transport (``synap://`` URL). Delivery is
        best-effort, latest-value: a watcher that cannot keep up is
        disconnected by the server and must re-``get`` and re-watch. Use
        :attr:`WatchEvent.version` to detect gaps. Closing the iterator issues
        ``KV.UNWATCH``.

        Args:
            pattern: Key or wildcard pattern (e.g. ``"user:*"``).
            mode: ``"value"`` (default) or ``"notify"`` — notify envelopes
                carry no value, so a watcher that only wants change signals
                pays no value bandwidth.
            queue_size: Internal queue depth for the push path.

        Yields:
            :class:`~synap_sdk.types.WatchEvent` per key change.

        Raises:
            SynapException: When the client is not on the SynapRPC transport.

        Example:
            >>> async for event in client.kv.watch("user:*"):
            ...     print(event.event, event.key, event.version, event.value)
        """
        rpc = self._client.synap_rpc_transport()
        if rpc is None:
            msg = "kv.watch requires the synap:// transport; over HTTP use the /kv/ws WebSocket endpoint"
            raise SynapException(msg)

        queue: asyncio.Queue[WatchEvent] = asyncio.Queue(maxsize=queue_size)

        def _on_event(envelope: dict[str, Any]) -> None:
            event = WatchEvent(
                key=str(envelope.get("key", "")),
                event=str(envelope.get("event", "")),
                version=int(envelope.get("version", 0)),
                value=envelope.get("value"),
                truncated=bool(envelope.get("truncated", False)),
            )
            # Best-effort: the server-side slow-consumer policy is the authority.
            with contextlib.suppress(asyncio.QueueFull):
                queue.put_nowait(event)

        _, cancel = await rpc.watch_push(pattern, mode, _on_event)
        try:
            while True:
                yield await queue.get()
        finally:
            cancel()
