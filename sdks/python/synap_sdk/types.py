"""Type definitions for Synap SDK."""

from dataclasses import dataclass
from typing import Any


@dataclass
class QueueMessage:
    """Represents a message from a queue.

    Attributes:
        id: The message ID
        payload: The message payload
        priority: Message priority (0-9, higher is more important)
        retries: Number of times this message has been retried
        max_retries: Maximum number of retries allowed
        timestamp: Message timestamp (Unix timestamp in seconds)
    """

    id: str
    payload: Any
    priority: int = 0
    retries: int = 0
    max_retries: int = 3
    timestamp: int = 0


@dataclass
class StreamEvent:
    """Represents an event from a stream.

    Attributes:
        offset: The event offset in the stream
        event: The event type/name
        data: The event data
        timestamp: Event timestamp (Unix timestamp in seconds)
        room: The room name (optional)
    """

    offset: int
    event: str
    data: Any
    timestamp: int = 0
    room: str | None = None


@dataclass
class WatchEvent:
    """One KV watch envelope (``docs/features/kv-watch.md`` in the server repository).

    ``value`` is the **post-mutation** value and is ``None`` for terminal
    events (``del``, ``expired``, ``evicted``), TTL-only events (``expire``,
    ``persist``), and envelopes degraded to notify-only (``truncated=True``).

    Attributes:
        key: The key that changed
        event: What happened (``set``, ``del``, ``expired``, ``evicted``, ...)
        version: Per-key counter for gap detection; resets when the key is
            deleted, expires or is evicted — version 1 marks a new incarnation
        value: The post-mutation value, when inlined
        truncated: True when the value was withheld (over the inline cap, or
            not UTF-8)
    """

    key: str
    event: str
    version: int
    value: str | None = None
    truncated: bool = False
