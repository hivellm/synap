"""Pub/Sub operations."""

from __future__ import annotations

from typing import TYPE_CHECKING, Any

if TYPE_CHECKING:
    from synap_sdk.client import SynapClient


class PubSubManager:
    """Pub/Sub operations.

    Example:
        >>> await client.pubsub.subscribe_topics("sub-1", ["notifications.*"])
        >>> delivered = await client.pubsub.publish("notifications.email", {"to": "user@example.com"})
        >>> await client.pubsub.unsubscribe_topics("sub-1", ["notifications.*"])
    """

    def __init__(self, client: SynapClient) -> None:
        """Initialize PubSubManager with a client."""
        self._client = client

    async def subscribe_topics(
        self,
        subscriber_id: str,
        topics: list[str],
    ) -> None:
        """Subscribe to topics for a subscriber.

        Args:
            subscriber_id: The subscriber ID
            topics: List of topic patterns (supports wildcards like 'user.*')
        """
        await self._client.execute(
            "pubsub.subscribe",
            subscriber_id,
            {"topics": topics},
        )

    async def unsubscribe_topics(
        self,
        subscriber_id: str,
        topics: list[str],
    ) -> None:
        """Unsubscribe from topics for a subscriber.

        Args:
            subscriber_id: The subscriber ID
            topics: List of topic patterns to unsubscribe from
        """
        await self._client.execute(
            "pubsub.unsubscribe",
            subscriber_id,
            {"topics": topics},
        )

    async def publish(
        self,
        topic: str,
        message: Any,
    ) -> int:
        """Publish a message to a topic.

        Args:
            topic: The topic name
            message: The message payload

        Returns:
            Number of subscribers that received the message
        """
        response = await self._client.execute(
            "pubsub.publish",
            topic,
            {"message": message},
        )
        return int(response.get("delivered", 0))

    async def stats(self) -> dict[str, Any]:
        """Get Pub/Sub statistics.

        Returns:
            Statistics as a dictionary
        """
        return await self._client.execute("pubsub.stats", "*")
