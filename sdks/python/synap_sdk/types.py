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
