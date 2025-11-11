"""HyperLogLog operations for Synap SDK."""

from __future__ import annotations

from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from synap_sdk.client import SynapClient


class HyperLogLogStats:
    """HyperLogLog statistics."""

    def __init__(self, data: dict[str, int]) -> None:
        """Initialize HyperLogLogStats from response data."""
        self.total_hlls = data.get("total_hlls", 0)
        self.total_cardinality = data.get("total_cardinality", 0)
        self.pfadd_count = data.get("pfadd_count", 0)
        self.pfcount_count = data.get("pfcount_count", 0)
        self.pfmerge_count = data.get("pfmerge_count", 0)


class HyperLogLogManager:
    """Manage HyperLogLog operations (Redis-compatible).

    HyperLogLog is a probabilistic data structure for estimating the cardinality
    of a set with minimal memory usage.

    Example:
        >>> async with SynapClient(config) as client:
        ...     await client.hyperloglog.pfadd("visitors", "user:1", "user:2")
        ...     count = await client.hyperloglog.pfcount("visitors")
    """

    def __init__(self, client: SynapClient) -> None:
        """Initialize HyperLogLogManager.

        Args:
            client: The Synap client instance
        """
        self._client = client

    async def pfadd(
        self, key: str, *elements: str | bytes | bytearray
    ) -> int:
        """Add elements to a HyperLogLog structure (PFADD).

        Args:
            key: HyperLogLog key
            *elements: Elements to add (strings, bytes, or bytearrays)

        Returns:
            Number of elements added (approximate)

        Example:
            >>> added = await hyperloglog.pfadd("visitors", "user:1", "user:2")
        """
        # Encode elements to byte arrays
        encoded: list[list[int]] = []
        for element in elements:
            if isinstance(element, str):
                encoded.append(list(element.encode()))
            elif isinstance(element, (bytes, bytearray)):
                encoded.append(list(element))
            else:
                # Convert to bytes
                encoded.append(list(str(element).encode()))

        if not encoded:
            return 0

        response = await self._client.send_command(
            "hyperloglog.pfadd", {"key": key, "elements": encoded}
        )
        return response.get("added", 0)

    async def pfcount(self, key: str) -> int:
        """Estimate cardinality of a HyperLogLog structure (PFCOUNT).

        Args:
            key: HyperLogLog key

        Returns:
            Estimated cardinality (approximate count)

        Example:
            >>> count = await hyperloglog.pfcount("visitors")
        """
        response = await self._client.send_command(
            "hyperloglog.pfcount", {"key": key}
        )
        return response.get("count", 0)

    async def pfmerge(self, destination: str, *sources: str) -> int:
        """Merge multiple HyperLogLog structures (PFMERGE).

        Args:
            destination: Destination key for merged result
            *sources: Source HyperLogLog keys to merge

        Returns:
            Estimated cardinality of merged result

        Raises:
            ValueError: If no source keys provided

        Example:
            >>> count = await hyperloglog.pfmerge("total", "hll:1", "hll:2")
        """
        if not sources:
            raise ValueError("PFMERGE requires at least one source key")

        response = await self._client.send_command(
            "hyperloglog.pfmerge",
            {"destination": destination, "sources": list(sources)},
        )
        return response.get("count", 0)

    async def stats(self) -> HyperLogLogStats:
        """Retrieve HyperLogLog statistics.

        Returns:
            HyperLogLog statistics

        Example:
            >>> stats = await hyperloglog.stats()
            >>> print(stats.total_hlls)
        """
        response = await self._client.send_command("hyperloglog.stats", {})
        return HyperLogLogStats(response)

