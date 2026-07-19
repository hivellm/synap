"""Synap SDK - Official Python client for Synap."""

from synap_sdk.client import SynapClient
from synap_sdk.config import SynapConfig
from synap_sdk.exceptions import SynapException, UnsupportedCommandError
from synap_sdk.modules.hash import HashManager
from synap_sdk.modules.list import ListManager
from synap_sdk.modules.set import SetManager
from synap_sdk.transport import TransportMode
from synap_sdk.types import QueueMessage, StreamEvent, WatchEvent

__version__ = "0.11.1"

__all__ = [
    "SynapClient",
    "SynapConfig",
    "SynapException",
    "UnsupportedCommandError",
    "TransportMode",
    "QueueMessage",
    "StreamEvent",
    "WatchEvent",
    "HashManager",
    "ListManager",
    "SetManager",
]
