"""Set data structure operations for Synap SDK."""

from __future__ import annotations

from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from synap_sdk.client import SynapClient


class SetManager:
    """Manage Set operations (Redis-compatible).

    Set is a collection of unique strings with set algebra operations.

    Example:
        >>> async with SynapClient(config) as client:
        ...     await client.set.add("tags", "redis", "python", "typescript")
        ...     is_member = await client.set.is_member("tags", "redis")
        ...     all_tags = await client.set.members("tags")
    """

    def __init__(self, client: SynapClient) -> None:
        """Initialize SetManager.

        Args:
            client: The Synap client instance
        """
        self._client = client

    async def add(self, key: str, *members: str) -> int:
        """Add members to set.

        Args:
            key: Set key
            *members: Members to add

        Returns:
            Number of members added
        """
        response = await self._client.send_command("set.add", {"key": key, "members": list(members)})
        return response.get("added", 0)

    async def rem(self, key: str, *members: str) -> int:
        """Remove members from set.

        Args:
            key: Set key
            *members: Members to remove

        Returns:
            Number of members removed
        """
        response = await self._client.send_command("set.rem", {"key": key, "members": list(members)})
        return response.get("removed", 0)

    async def is_member(self, key: str, member: str) -> bool:
        """Check if member exists in set.

        Args:
            key: Set key
            member: Member to check

        Returns:
            True if member exists
        """
        response = await self._client.send_command("set.ismember", {"key": key, "member": member})
        return response.get("is_member", False)

    async def members(self, key: str) -> list[str]:
        """Get all members of set.

        Args:
            key: Set key

        Returns:
            List of members
        """
        response = await self._client.send_command("set.members", {"key": key})
        return response.get("members", [])

    async def card(self, key: str) -> int:
        """Get set cardinality (size).

        Args:
            key: Set key

        Returns:
            Number of members
        """
        response = await self._client.send_command("set.card", {"key": key})
        return response.get("cardinality", 0)

    async def pop(self, key: str, count: int = 1) -> list[str]:
        """Remove and return random members.

        Args:
            key: Set key
            count: Number of members to pop

        Returns:
            List of popped members
        """
        response = await self._client.send_command("set.pop", {"key": key, "count": count})
        return response.get("members", [])

    async def rand_member(self, key: str, count: int = 1) -> list[str]:
        """Get random members without removing.

        Args:
            key: Set key
            count: Number of members to return

        Returns:
            List of random members
        """
        response = await self._client.send_command("set.randmember", {"key": key, "count": count})
        return response.get("members", [])

    async def move(self, source: str, destination: str, member: str) -> bool:
        """Move member from source to destination set.

        Args:
            source: Source set key
            destination: Destination set key
            member: Member to move

        Returns:
            True if member was moved
        """
        response = await self._client.send_command(
            "set.move", {"source": source, "destination": destination, "member": member}
        )
        return response.get("moved", False)

    async def inter(self, *keys: str) -> list[str]:
        """Get intersection of sets.

        Args:
            *keys: Set keys

        Returns:
            List of members in intersection
        """
        response = await self._client.send_command("set.inter", {"keys": list(keys)})
        return response.get("members", [])

    async def union(self, *keys: str) -> list[str]:
        """Get union of sets.

        Args:
            *keys: Set keys

        Returns:
            List of members in union
        """
        response = await self._client.send_command("set.union", {"keys": list(keys)})
        return response.get("members", [])

    async def diff(self, *keys: str) -> list[str]:
        """Get difference of sets (first minus others).

        Args:
            *keys: Set keys

        Returns:
            List of members in difference
        """
        response = await self._client.send_command("set.diff", {"keys": list(keys)})
        return response.get("members", [])

    async def inter_store(self, destination: str, *keys: str) -> int:
        """Store intersection result in destination.

        Args:
            destination: Destination set key
            *keys: Source set keys

        Returns:
            Number of members in result
        """
        response = await self._client.send_command(
            "set.interstore", {"destination": destination, "keys": list(keys)}
        )
        return response.get("cardinality", 0)

    async def union_store(self, destination: str, *keys: str) -> int:
        """Store union result in destination.

        Args:
            destination: Destination set key
            *keys: Source set keys

        Returns:
            Number of members in result
        """
        response = await self._client.send_command(
            "set.unionstore", {"destination": destination, "keys": list(keys)}
        )
        return response.get("cardinality", 0)

    async def diff_store(self, destination: str, *keys: str) -> int:
        """Store difference result in destination.

        Args:
            destination: Destination set key
            *keys: Source set keys

        Returns:
            Number of members in result
        """
        response = await self._client.send_command(
            "set.diffstore", {"destination": destination, "keys": list(keys)}
        )
        return response.get("cardinality", 0)

