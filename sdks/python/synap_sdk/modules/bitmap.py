"""Bitmap operations for Synap SDK."""

from __future__ import annotations

from typing import TYPE_CHECKING, Literal

if TYPE_CHECKING:
    from synap_sdk.client import SynapClient


BitmapOperation = Literal["AND", "OR", "XOR", "NOT"]


class BitmapStats:
    """Bitmap statistics."""

    def __init__(self, data: dict[str, int]) -> None:
        """Initialize BitmapStats from response data."""
        self.total_bitmaps = data.get("total_bitmaps", 0)
        self.total_bits = data.get("total_bits", 0)
        self.setbit_count = data.get("setbit_count", 0)
        self.getbit_count = data.get("getbit_count", 0)
        self.bitcount_count = data.get("bitcount_count", 0)
        self.bitop_count = data.get("bitop_count", 0)
        self.bitpos_count = data.get("bitpos_count", 0)
        self.bitfield_count = data.get("bitfield_count", 0)


class BitmapManager:
    """Manage Bitmap operations (Redis-compatible).

    Bitmap operations allow bit-level manipulation of strings.

    Example:
        >>> async with SynapClient(config) as client:
        ...     await client.bitmap.setbit("visits", 0, 1)
        ...     bit = await client.bitmap.getbit("visits", 0)
        ...     count = await client.bitmap.bitcount("visits")
    """

    def __init__(self, client: SynapClient) -> None:
        """Initialize BitmapManager.

        Args:
            client: The Synap client instance
        """
        self._client = client

    async def setbit(self, key: str, offset: int, value: Literal[0, 1]) -> int:
        """Set bit at offset to value (SETBIT).

        Args:
            key: Bitmap key
            offset: Bit offset (0-based)
            value: Bit value (0 or 1)

        Returns:
            Previous bit value (0 or 1)

        Raises:
            ValueError: If value is not 0 or 1

        Example:
            >>> old = await bitmap.setbit("visits", 5, 1)
        """
        if value not in (0, 1):
            raise ValueError("Bitmap value must be 0 or 1")

        response = await self._client.send_command(
            "bitmap.setbit", {"key": key, "offset": offset, "value": value}
        )
        return response.get("old_value", 0)

    async def getbit(self, key: str, offset: int) -> int:
        """Get bit at offset (GETBIT).

        Args:
            key: Bitmap key
            offset: Bit offset (0-based)

        Returns:
            Bit value (0 or 1)

        Example:
            >>> bit = await bitmap.getbit("visits", 5)
        """
        response = await self._client.send_command(
            "bitmap.getbit", {"key": key, "offset": offset}
        )
        return response.get("value", 0)

    async def bitcount(
        self, key: str, start: int | None = None, end: int | None = None
    ) -> int:
        """Count set bits in bitmap (BITCOUNT).

        Args:
            key: Bitmap key
            start: Optional start offset (inclusive, default: 0)
            end: Optional end offset (inclusive, default: end of bitmap)

        Returns:
            Number of set bits

        Example:
            >>> count = await bitmap.bitcount("visits")
            >>> count_range = await bitmap.bitcount("visits", 0, 15)
        """
        payload: dict[str, int] = {"key": key}
        if start is not None:
            payload["start"] = start
        if end is not None:
            payload["end"] = end

        response = await self._client.send_command("bitmap.bitcount", payload)
        return response.get("count", 0)

    async def bitpos(
        self,
        key: str,
        value: Literal[0, 1],
        start: int | None = None,
        end: int | None = None,
    ) -> int | None:
        """Find first bit set to value (BITPOS).

        Args:
            key: Bitmap key
            value: Bit value to search for (0 or 1)
            start: Optional start offset (inclusive, default: 0)
            end: Optional end offset (inclusive, default: end of bitmap)

        Returns:
            Position of first matching bit, or None if not found

        Raises:
            ValueError: If value is not 0 or 1

        Example:
            >>> pos = await bitmap.bitpos("visits", 1)
            >>> pos_range = await bitmap.bitpos("visits", 1, 5, 20)
        """
        if value not in (0, 1):
            raise ValueError("Bitmap value must be 0 or 1")

        payload: dict[str, int] = {"key": key, "value": value}
        if start is not None:
            payload["start"] = start
        if end is not None:
            payload["end"] = end

        response = await self._client.send_command("bitmap.bitpos", payload)
        position = response.get("position")
        return position if position is not None else None

    async def bitop(
        self,
        operation: BitmapOperation,
        destination: str,
        source_keys: list[str],
    ) -> int:
        """Perform bitwise operation on multiple bitmaps (BITOP).

        Args:
            operation: Bitwise operation (AND, OR, XOR, NOT)
            destination: Destination key for result
            source_keys: Source bitmap keys (NOT requires exactly 1 source)

        Returns:
            Length of resulting bitmap in bits

        Raises:
            ValueError: If NOT operation is used with more than one source key
            ValueError: If no source keys provided

        Example:
            >>> length = await bitmap.bitop("AND", "result", ["bitmap1", "bitmap2"])
            >>> length = await bitmap.bitop("NOT", "result", ["bitmap1"])
        """
        if operation == "NOT" and len(source_keys) != 1:
            raise ValueError("NOT operation requires exactly one source key")

        if not source_keys:
            raise ValueError("BITOP requires at least one source key")

        response = await self._client.send_command(
            "bitmap.bitop",
            {
                "destination": destination,
                "operation": operation,
                "source_keys": source_keys,
            },
        )
        return response.get("length", 0)

    async def bitfield(
        self,
        key: str,
        operations: list[dict[str, int | str | bool | None]],
    ) -> list[int]:
        """Execute bitfield operations (BITFIELD).

        Args:
            key: Bitmap key
            operations: List of operation dictionaries. Each operation must have:
                - operation: "GET", "SET", or "INCRBY"
                - offset: Bit offset (0-based)
                - width: Bit width (1-64)
                - signed: Optional bool (default: False)
                - value: Required for SET operation
                - increment: Required for INCRBY operation
                - overflow: Optional "WRAP", "SAT", or "FAIL" (default: "WRAP")

        Returns:
            List of result values (one per operation)

        Example:
            >>> results = await bitmap.bitfield("mybitmap", [
            ...     {"operation": "SET", "offset": 0, "width": 8, "value": 42},
            ...     {"operation": "GET", "offset": 0, "width": 8},
            ...     {"operation": "INCRBY", "offset": 0, "width": 8, "increment": 10, "overflow": "WRAP"}
            ... ])
        """
        response = await self._client.send_command(
            "bitmap.bitfield", {"key": key, "operations": operations}
        )
        return response.get("results", [])

    async def stats(self) -> BitmapStats:
        """Retrieve bitmap statistics.

        Returns:
            Bitmap statistics

        Example:
            >>> stats = await bitmap.stats()
            >>> print(stats.total_bitmaps)
        """
        response = await self._client.send_command("bitmap.stats", {})
        return BitmapStats(response)

