"""Event Stream operations."""

from __future__ import annotations

from typing import TYPE_CHECKING, Any

from synap_sdk.types import StreamEvent

if TYPE_CHECKING:
    from synap_sdk.client import SynapClient


class StreamManager:
    """Event Stream operations.

    Example:
        >>> await client.stream.create_room("events")
        >>> offset = await client.stream.publish("events", "user.created", {"id": "123"})
        >>> events = await client.stream.read("events", offset=0, limit=10)
    """

    def __init__(self, client: SynapClient) -> None:
        """Initialize StreamManager with a client."""
        self._client = client

    async def create_room(self, room: str) -> None:
        """Create a new stream room.

        Args:
            room: The room name
        """
        await self._client.execute("stream.create_room", room)

    async def delete_room(self, room: str) -> None:
        """Delete a stream room.

        Args:
            room: The room name
        """
        await self._client.execute("stream.delete_room", room)

    async def publish(
        self,
        room: str,
        event: str,
        data: Any,
    ) -> int:
        """Publish an event to a stream room.

        Args:
            room: The room name
            event: The event type/name
            data: The event data

        Returns:
            The event offset in the stream
        """
        response = await self._client.execute(
            "stream.publish",
            room,
            {"event": event, "data": data},
        )
        return int(response.get("offset", 0))

    async def read(
        self,
        room: str,
        offset: int = 0,
        limit: int = 100,
    ) -> list[StreamEvent]:
        """Read events from a stream.

        Args:
            room: The room name
            offset: Starting offset (0 for beginning)
            limit: Maximum number of events to read

        Returns:
            List of stream events
        """
        response = await self._client.execute(
            "stream.read",
            room,
            {"offset": offset, "limit": limit},
        )

        events_data = response.get("events", [])
        return [
            StreamEvent(
                offset=int(evt.get("offset", 0)),
                event=str(evt.get("event", "")),
                data=evt.get("data"),
                timestamp=int(evt.get("timestamp", 0)),
                room=evt.get("room"),
            )
            for evt in events_data
        ]

    async def stats(self, room: str) -> dict[str, Any]:
        """Get stream statistics.

        Args:
            room: The room name

        Returns:
            Statistics as a dictionary
        """
        return await self._client.execute("stream.stats", room)

    async def list_rooms(self) -> list[str]:
        """List all stream rooms.

        Returns:
            List of room names
        """
        response = await self._client.execute("stream.list_rooms", "*")
        return list(response.get("rooms", []))
