"""Key-Value Store operations."""

from __future__ import annotations

from typing import TYPE_CHECKING, Any, TypeVar

if TYPE_CHECKING:
    from synap_sdk.client import SynapClient

T = TypeVar("T")


class KVStore:
    """Key-Value Store operations.

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
            value: The value to store
            ttl: Optional time-to-live in seconds
        """
        data: dict[str, Any] = {"value": value}
        if ttl is not None:
            data["ttl"] = ttl

        await self._client.execute("kv.set", key, data)

    async def get(self, key: str) -> Any:
        """Get a value by key.

        Args:
            key: The key to get

        Returns:
            The value, or None if not found
        """
        response = await self._client.execute("kv.get", key)
        return response.get("value")

    async def delete(self, key: str) -> None:
        """Delete a key.

        Args:
            key: The key to delete
        """
        await self._client.execute("kv.delete", key)

    async def exists(self, key: str) -> bool:
        """Check if a key exists.

        Args:
            key: The key to check

        Returns:
            True if the key exists, False otherwise
        """
        response = await self._client.execute("kv.exists", key)
        return bool(response.get("exists", False))

    async def incr(self, key: str, delta: int = 1) -> int:
        """Increment a numeric value.

        Args:
            key: The key to increment
            delta: The amount to increment by (default: 1)

        Returns:
            The new value after incrementing
        """
        response = await self._client.execute("kv.incr", key, {"delta": delta})
        return int(response.get("value", 0))

    async def decr(self, key: str, delta: int = 1) -> int:
        """Decrement a numeric value.

        Args:
            key: The key to decrement
            delta: The amount to decrement by (default: 1)

        Returns:
            The new value after decrementing
        """
        response = await self._client.execute("kv.decr", key, {"delta": delta})
        return int(response.get("value", 0))

    async def scan(self, prefix: str, limit: int = 100) -> list[str]:
        """Scan keys by prefix.

        Args:
            prefix: The prefix to search for
            limit: Maximum number of keys to return (default: 100)

        Returns:
            List of matching keys
        """
        response = await self._client.execute("kv.scan", prefix, {"limit": limit})
        return list(response.get("keys", []))

    async def stats(self) -> dict[str, Any]:
        """Get KV store statistics.

        Returns:
            Statistics as a dictionary
        """
        return await self._client.execute("kv.stats", "*")
