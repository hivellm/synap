"""Binary TCP transports for Synap SDK.

This module re-exports the public API from the focused sub-modules:

- :mod:`synap_sdk.transport_rpc`   — SynapRPC (MessagePack-framed binary)
- :mod:`synap_sdk.transport_resp3` — RESP3 (Redis-compatible text protocol)
- :mod:`synap_sdk.command_map`     — command/response mapper

Wire encoding mirrors Rust's ``rmp_serde`` externally-tagged enum format:

=============  ==========================
Python value   Wire envelope
=============  ==========================
``None``       bare msgpack string "Null"
``str``        ``{"Str": "…"}``
``int``        ``{"Int": n}``
``float``      ``{"Float": f}``
``bool``       ``{"Bool": b}``
``bytes``      ``{"Bytes": b}``
``list``       ``{"Array": […]}``
=============  ==========================

Structs (Request, Response) are encoded as msgpack arrays.
"""

from __future__ import annotations

from typing import Literal

# Re-export transport implementations
from synap_sdk.transport_rpc import SynapRpcTransport, _from_wire, _to_wire
from synap_sdk.transport_resp3 import Resp3Transport

# Re-export command / response mapper
from synap_sdk.command_map import map_command as _map_command_public
from synap_sdk.command_map import map_command_optional as _map_command
from synap_sdk.command_map import map_response as _map_response

# ── Transport mode ─────────────────────────────────────────────────────────────

TransportMode = Literal["synaprpc", "resp3", "http"]

# Public aliases (used by client.py and external code)
map_command = _map_command
map_response = _map_response

__all__ = [
    "TransportMode",
    "SynapRpcTransport",
    "Resp3Transport",
    # private helpers re-exported for tests and introspection
    "_to_wire",
    "_from_wire",
    "_map_command",
    "_map_response",
    # public aliases
    "map_command",
    "map_response",
]
