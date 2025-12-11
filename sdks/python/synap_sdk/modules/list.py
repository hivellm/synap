"""List data structure operations for Synap SDK."""

from __future__ import annotations

from typing import TYPE_CHECKING, Any, Literal

if TYPE_CHECKING:
    from synap_sdk.client import SynapClient


class ListManager:
    """Manage List operations (Redis-compatible).

    List is a doubly-linked list data structure with O(1) push/pop at both ends.

    Example:
        >>> async with SynapClient(config) as client:
        ...     await client.list.rpush("tasks", "task1", "task2", "task3")
        ...     tasks = await client.list.range("tasks", 0, -1)
        ...     task = await client.list.lpop("tasks")
    """

    def __init__(self, client: SynapClient) -> None:
        """Initialize ListManager.

        Args:
            client: The Synap client instance
        """
        self._client = client

    async def lpush(self, key: str, *values: str) -> int:
        """Push elements to left (head) of list.

        Args:
            key: List key
            *values: Values to push

        Returns:
            New list length
        """
        response = await self._client.send_command("list.lpush", {"key": key, "values": list(values)})
        return response.get("length", 0)

    async def rpush(self, key: str, *values: str) -> int:
        """Push elements to right (tail) of list.

        Args:
            key: List key
            *values: Values to push

        Returns:
            New list length
        """
        response = await self._client.send_command("list.rpush", {"key": key, "values": list(values)})
        return response.get("length", 0)

    async def lpop(self, key: str, count: int | None = None) -> list[str]:
        """Pop elements from left (head) of list.

        Args:
            key: List key
            count: Number of elements to pop (optional, defaults to 1)

        Returns:
            List of popped values
        """
        payload: dict[str, Any] = {"key": key}
        if count is not None:
            payload["count"] = count
        response = await self._client.send_command("list.lpop", payload)
        return response.get("values", [])

    async def rpop(self, key: str, count: int | None = None) -> list[str]:
        """Pop elements from right (tail) of list.

        Args:
            key: List key
            count: Number of elements to pop (optional, defaults to 1)

        Returns:
            List of popped values
        """
        payload: dict[str, Any] = {"key": key}
        if count is not None:
            payload["count"] = count
        response = await self._client.send_command("list.rpop", payload)
        return response.get("values", [])

    async def range(self, key: str, start: int = 0, stop: int = -1) -> list[str]:
        """Get range of elements from list.

        Args:
            key: List key
            start: Start index (0-based)
            stop: Stop index (-1 for last element)

        Returns:
            List of values in range
        """
        response = await self._client.send_command("list.range", {"key": key, "start": start, "stop": stop})
        return response.get("values", [])

    async def len(self, key: str) -> int:
        """Get list length.

        Args:
            key: List key

        Returns:
            Number of elements
        """
        response = await self._client.send_command("list.len", {"key": key})
        return response.get("length", 0)

    async def index(self, key: str, index: int) -> str | None:
        """Get element at index.

        Args:
            key: List key
            index: Element index

        Returns:
            Value at index or None
        """
        response = await self._client.send_command("list.index", {"key": key, "index": index})
        return response.get("value")

    async def set(self, key: str, index: int, value: str) -> bool:
        """Set element at index.

        Args:
            key: List key
            index: Element index
            value: New value

        Returns:
            True if set successfully
        """
        response = await self._client.send_command("list.set", {"key": key, "index": index, "value": value})
        return response.get("success", False)

    async def trim(self, key: str, start: int, stop: int) -> bool:
        """Trim list to specified range.

        Args:
            key: List key
            start: Start index
            stop: Stop index

        Returns:
            True if trimmed successfully
        """
        response = await self._client.send_command("list.trim", {"key": key, "start": start, "stop": stop})
        return response.get("success", False)

    async def rem(self, key: str, count: int, value: str) -> int:
        """Remove elements from list.

        Args:
            key: List key
            count: Number to remove (0=all, >0 from head, <0 from tail)
            value: Value to remove

        Returns:
            Number of elements removed
        """
        response = await self._client.send_command("list.rem", {"key": key, "count": count, "value": value})
        return response.get("removed", 0)

    async def insert(
        self, key: str, position: Literal["BEFORE", "AFTER"], pivot: str, value: str
    ) -> int:
        """Insert element before/after pivot.

        Args:
            key: List key
            position: "BEFORE" or "AFTER"
            pivot: Reference element
            value: Value to insert

        Returns:
            New list length (-1 if pivot not found)
        """
        response = await self._client.send_command(
            "list.insert",
            {"key": key, "position": position.lower(), "pivot": pivot, "value": value},
        )
        return response.get("length", -1)

    async def rpoplpush(self, source: str, destination: str) -> str | None:
        """Pop from source and push to destination (atomic).

        Args:
            source: Source list key
            destination: Destination list key

        Returns:
            Moved value or None
        """
        response = await self._client.send_command(
            "list.rpoplpush", {"source": source, "destination": destination}
        )
        return response.get("value")

    async def pos(self, key: str, element: str, rank: int = 1) -> int | None:
        """Find first position of element.

        Args:
            key: List key
            element: Element to find
            rank: Rank to find (1=first, 2=second, -1=last)

        Returns:
            Position or None if not found
        """
        response = await self._client.send_command("list.pos", {"key": key, "element": element, "rank": rank})
        pos = response.get("position")
        return pos if pos is not None else None

    async def lpushx(self, key: str, *values: str) -> int:
        """Push to left only if list exists.

        Args:
            key: List key
            *values: Values to push

        Returns:
            New list length (0 if list doesn't exist)
        """
        response = await self._client.send_command("list.lpushx", {"key": key, "values": list(values)})
        return response.get("length", 0)

    async def rpushx(self, key: str, *values: str) -> int:
        """Push to right only if list exists.

        Args:
            key: List key
            *values: Values to push

        Returns:
            New list length (0 if list doesn't exist)
        """
        response = await self._client.send_command("list.rpushx", {"key": key, "values": list(values)})
        return response.get("length", 0)

