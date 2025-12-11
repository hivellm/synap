"""Hash data structure operations for Synap SDK."""

from __future__ import annotations

from typing import TYPE_CHECKING, Any

if TYPE_CHECKING:
    from synap_sdk.client import SynapClient


class HashManager:
    """Manage Hash operations (Redis-compatible).

    Hash is a field-value map data structure, ideal for storing objects.

    Example:
        >>> async with SynapClient(config) as client:
        ...     await client.hash.set("user:1", "name", "Alice")
        ...     await client.hash.set("user:1", "age", "30")
        ...     name = await client.hash.get("user:1", "name")
        ...     all_fields = await client.hash.get_all("user:1")
    """

    def __init__(self, client: SynapClient) -> None:
        """Initialize HashManager.

        Args:
            client: The Synap client instance
        """
        self._client = client

    async def set(self, key: str, field: str, value: str | int | float) -> bool:
        """Set field in hash.

        Args:
            key: Hash key
            field: Field name
            value: Field value

        Returns:
            True if field was set

        Example:
            >>> await hash.set("user:1", "name", "Alice")
        """
        response = await self._client.send_command(
            "hash.set", {"key": key, "field": field, "value": str(value)}
        )
        return response.get("success", False)

    async def get(self, key: str, field: str) -> str | None:
        """Get field from hash.

        Args:
            key: Hash key
            field: Field name

        Returns:
            Field value or None if not found
        """
        response = await self._client.send_command("hash.get", {"key": key, "field": field})
        return response.get("value")

    async def get_all(self, key: str) -> dict[str, str]:
        """Get all fields and values from hash.

        Args:
            key: Hash key

        Returns:
            Dictionary of field-value pairs
        """
        response = await self._client.send_command("hash.getall", {"key": key})
        return response.get("fields", {})

    async def delete(self, key: str, field: str) -> int:
        """Delete field from hash.

        Args:
            key: Hash key
            field: Field name

        Returns:
            Number of fields deleted (0 or 1)
        """
        response = await self._client.send_command("hash.del", {"key": key, "field": field})
        return response.get("deleted", 0)

    async def exists(self, key: str, field: str) -> bool:
        """Check if field exists in hash.

        Args:
            key: Hash key
            field: Field name

        Returns:
            True if field exists
        """
        response = await self._client.send_command("hash.exists", {"key": key, "field": field})
        return response.get("exists", False)

    async def keys(self, key: str) -> list[str]:
        """Get all field names in hash.

        Args:
            key: Hash key

        Returns:
            List of field names
        """
        response = await self._client.send_command("hash.keys", {"key": key})
        return response.get("fields", [])

    async def values(self, key: str) -> list[str]:
        """Get all values in hash.

        Args:
            key: Hash key

        Returns:
            List of values
        """
        response = await self._client.send_command("hash.values", {"key": key})
        return response.get("values", [])

    async def len(self, key: str) -> int:
        """Get number of fields in hash.

        Args:
            key: Hash key

        Returns:
            Number of fields
        """
        response = await self._client.send_command("hash.len", {"key": key})
        return response.get("length", 0)

    async def mset(
        self, 
        key: str, 
        fields: dict[str, str | int | float] | list[dict[str, str]]
    ) -> bool:
        """Set multiple fields in hash.

        Supports both dict format (backward compatible) and array format (Redis-compatible).

        Args:
            key: Hash key
            fields: Dictionary of field-value pairs OR list of {"field": "...", "value": "..."} dicts

        Returns:
            True if fields were set

        Example:
            >>> # Dict format (backward compatible)
            >>> await hash.mset("user:1", {"name": "Alice", "age": 30})
            >>> # Array format (Redis-compatible)
            >>> await hash.mset("user:1", [{"field": "name", "value": "Alice"}, {"field": "age", "value": "30"}])
        """
        if isinstance(fields, list):
            # Array format: [{"field": "...", "value": "..."}, ...]
            fields_array = [{"field": f["field"], "value": str(f["value"])} for f in fields]
            response = await self._client.send_command("hash.mset", {"key": key, "fields": fields_array})
        else:
            # Dict format (backward compatible)
            str_fields = {k: str(v) for k, v in fields.items()}
            response = await self._client.send_command("hash.mset", {"key": key, "fields": str_fields})
        return response.get("success", False)

    async def mget(self, key: str, fields: list[str]) -> dict[str, str | None]:
        """Get multiple fields from hash.

        Args:
            key: Hash key
            fields: List of field names

        Returns:
            Dictionary of field-value pairs (None for missing fields)
        """
        response = await self._client.send_command("hash.mget", {"key": key, "fields": fields})
        return response.get("values", {})

    async def incr_by(self, key: str, field: str, increment: int) -> int:
        """Increment field value by integer.

        Args:
            key: Hash key
            field: Field name
            increment: Integer to add

        Returns:
            New value after increment
        """
        response = await self._client.send_command(
            "hash.incrby", {"key": key, "field": field, "increment": increment}
        )
        return response.get("value", 0)

    async def incr_by_float(self, key: str, field: str, increment: float) -> float:
        """Increment field value by float.

        Args:
            key: Hash key
            field: Field name
            increment: Float to add

        Returns:
            New value after increment
        """
        response = await self._client.send_command(
            "hash.incrbyfloat", {"key": key, "field": field, "increment": increment}
        )
        return response.get("value", 0.0)

    async def set_nx(self, key: str, field: str, value: str | int | float) -> bool:
        """Set field only if it doesn't exist.

        Args:
            key: Hash key
            field: Field name
            value: Field value

        Returns:
            True if field was created, False if already exists
        """
        response = await self._client.send_command(
            "hash.setnx", {"key": key, "field": field, "value": str(value)}
        )
        return response.get("created", False)

