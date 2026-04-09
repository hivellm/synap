"""Pub/Sub operations."""

from __future__ import annotations

import asyncio
from collections.abc import AsyncIterator
from typing import TYPE_CHECKING, Any

if TYPE_CHECKING:
    from synap_sdk.client import SynapClient


class PubSubManager:
    """Pub/Sub operations.

    Example:
        >>> await client.pubsub.publish("notifications.email", {"to": "user@example.com"})
        >>> async for msg in client.pubsub.observe(["notifications.*"], "sub-1"):
        ...     print(msg["topic"], msg["payload"])
    """

    def __init__(self, client: SynapClient) -> None:
        """Initialize PubSubManager with a client."""
        self._client = client

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
        response = await self._client.send_command(
            "pubsub.publish",
            {"topic": topic, "payload": message},
        )
        return int(response.get("subscribers_matched", 0))

    async def subscribe_topics(
        self,
        subscriber_id: str,
        topics: list[str],
    ) -> None:
        """Register a subscription on the server (HTTP transport only).

        For real-time message delivery on SynapRPC, use :meth:`observe` instead.

        Args:
            subscriber_id: The subscriber ID
            topics: List of topic patterns (supports wildcards like ``user.*``)
        """
        await self._client.send_command(
            "pubsub.subscribe",
            {"topics": topics, "subscriber_id": subscriber_id},
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
        await self._client.send_command(
            "pubsub.unsubscribe",
            {"topics": topics, "subscriber_id": subscriber_id},
        )

    async def list_topics(self) -> list[str]:
        """List all active topics.

        Returns:
            List of topic names
        """
        response = await self._client.send_command("pubsub.topics", {})
        return list(response.get("topics", []))

    async def observe(
        self,
        topics: list[str],
        subscriber_id: str | None = None,
        *,
        queue_size: int = 256,
    ) -> AsyncIterator[dict[str, Any]]:
        """Subscribe and yield push messages as an async iterator.

        On SynapRPC (``synap://`` URL), opens a dedicated server-push TCP
        connection for real-time delivery.  On other transports this falls back
        to a :meth:`subscribe_topics` registration (no real-time delivery).

        Args:
            topics: Topic patterns to subscribe to.
            subscriber_id: Optional subscriber ID; auto-generated if omitted.
            queue_size: Internal queue depth for the push path.

        Yields:
            Dicts with keys: ``topic``, ``payload``, ``id``, ``timestamp``.

        Example:
            >>> async for msg in client.pubsub.observe(["alerts.*"]):
            ...     print(msg["topic"], msg["payload"])
        """
        import time as _time

        sid = subscriber_id or f"py-sub-{int(_time.time() * 1000)}"
        rpc = self._client.synap_rpc_transport()

        if rpc is not None:
            # ── SynapRPC native push path ────────────────────────────────────
            queue: asyncio.Queue[dict[str, Any] | None] = asyncio.Queue(maxsize=queue_size)

            def _on_msg(msg: dict[str, Any]) -> None:
                try:
                    queue.put_nowait(msg)
                except asyncio.QueueFull:
                    pass  # Drop oldest if full

            _, cancel = await rpc.subscribe_push(topics, _on_msg)
            try:
                while True:
                    item = await queue.get()
                    if item is None:
                        break
                    yield item
            finally:
                cancel()
        else:
            # ── HTTP fallback: register subscription (no real-time delivery) ─
            await self.subscribe_topics(sid, topics)
            # Yield nothing — callers must poll via the HTTP API
            return
            yield  # make this an async generator

    async def stats(self) -> dict[str, Any]:
        """Get Pub/Sub statistics.

        Returns:
            Statistics as a dictionary
        """
        return await self._client.send_command("pubsub.stats", {})
