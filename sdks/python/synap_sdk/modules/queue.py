"""Message Queue operations."""

from __future__ import annotations

from typing import TYPE_CHECKING, Any

from synap_sdk.types import QueueMessage

if TYPE_CHECKING:
    from synap_sdk.client import SynapClient


class QueueManager:
    """Message Queue operations.

    Example:
        >>> await client.queue.create_queue("tasks")
        >>> msg_id = await client.queue.publish("tasks", {"action": "process"})
        >>> message = await client.queue.consume("tasks", "worker-1")
        >>> if message:
        ...     await client.queue.ack("tasks", message.id)
    """

    def __init__(self, client: SynapClient) -> None:
        """Initialize QueueManager with a client."""
        self._client = client

    async def create_queue(
        self,
        name: str,
        max_size: int | None = None,
        message_ttl: int | None = None,
    ) -> None:
        """Create a new queue.

        Args:
            name: The queue name
            max_size: Optional maximum queue size
            message_ttl: Optional message TTL in seconds
        """
        data: dict[str, Any] = {}
        if max_size is not None:
            data["max_size"] = max_size
        if message_ttl is not None:
            data["message_ttl"] = message_ttl

        await self._client.execute("queue.create", name, data)

    async def delete_queue(self, name: str) -> None:
        """Delete a queue.

        Args:
            name: The queue name
        """
        await self._client.execute("queue.delete", name)

    async def publish(
        self,
        queue: str,
        message: Any,
        priority: int | None = None,
        max_retries: int | None = None,
    ) -> str:
        """Publish a message to a queue.

        Args:
            queue: The queue name
            message: The message payload
            priority: Message priority (0-9, higher is more important)
            max_retries: Maximum retry attempts

        Returns:
            The message ID
        """
        data: dict[str, Any] = {"message": message}
        if priority is not None:
            data["priority"] = priority
        if max_retries is not None:
            data["max_retries"] = max_retries

        response = await self._client.execute("queue.publish", queue, data)
        return str(response.get("message_id", ""))

    async def consume(
        self,
        queue: str,
        consumer_id: str,
    ) -> QueueMessage | None:
        """Consume a message from a queue.

        Args:
            queue: The queue name
            consumer_id: The consumer ID

        Returns:
            The queue message, or None if no message is available
        """
        response = await self._client.execute(
            "queue.consume",
            queue,
            {"consumer_id": consumer_id},
        )

        msg_data = response.get("message")
        if not msg_data:
            return None

        return QueueMessage(
            id=str(msg_data.get("id", "")),
            payload=msg_data.get("payload"),
            priority=int(msg_data.get("priority", 0)),
            retries=int(msg_data.get("retries", 0)),
            max_retries=int(msg_data.get("max_retries", 3)),
            timestamp=int(msg_data.get("timestamp", 0)),
        )

    async def ack(self, queue: str, message_id: str) -> None:
        """Acknowledge successful message processing.

        Args:
            queue: The queue name
            message_id: The message ID to acknowledge
        """
        await self._client.execute("queue.ack", queue, {"message_id": message_id})

    async def nack(self, queue: str, message_id: str) -> None:
        """Negative acknowledge a message (requeue for retry).

        Args:
            queue: The queue name
            message_id: The message ID to requeue
        """
        await self._client.execute("queue.nack", queue, {"message_id": message_id})

    async def stats(self, queue: str) -> dict[str, Any]:
        """Get queue statistics.

        Args:
            queue: The queue name

        Returns:
            Statistics as a dictionary
        """
        return await self._client.execute("queue.stats", queue)

    async def list(self) -> list[str]:
        """List all queues.

        Returns:
            List of queue names
        """
        response = await self._client.execute("queue.list", "*")
        return list(response.get("queues", []))
