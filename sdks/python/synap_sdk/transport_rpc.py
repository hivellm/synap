"""SynapRPC binary TCP transport.

The wire layer is **not implemented here**. It is
`Thunder <https://github.com/hivellm/thunder>`_ (``hivellm-thunder``, imported
as ``thunder_rpc``) — the HiveLLM family's shared binary RPC client, the same
protocol the Synap server runs on, so the two ends cannot drift.

What Thunder brings that the hand-written transport did not:

- the frame cap validated against the length prefix **before** allocating,
  closing an unbounded-allocation hole a remote peer could trigger;
- a real handshake, so the SDK can authenticate on the RPC port;
- connect and per-call timeouts, and lazy reconnect with capped retries;
- a push hook, so a `SUBSCRIBE` cannot lose a message published between the
  server's acknowledgement and the reader starting.

What stays here is Synap's own: the plain-Python ↔ wire value conversion the
SDK's command mappers speak.
"""

from __future__ import annotations

import asyncio
from typing import Any

from thunder_rpc import (
    AsyncClient,
    AuthError,
    ClientConfig,
    Config,
    Credentials,
    ErrorConvention,
    Handshake,
    HelloStyle,
    PushPolicy,
    ServerError,
    ThunderError,
    Value,
)

from synap_sdk.exceptions import SynapException

#: Synap's frame cap, matching the server's ``synap_config()``.
MAX_FRAME_BYTES = 512 * 1024 * 1024

#: Default SynapRPC port.
DEFAULT_RPC_PORT = 15501


def synap_config() -> Config:
    """How Synap uses the Thunder wire, mirroring the server's ``synap_config()``.

    Thunder ships one standard and zero product knowledge, so this description
    lives in Synap's own repository. Every divergence from the standard is
    explicit: Synap authenticates with ``AUTH`` rather than a mandatory
    ``HELLO``, it ships a push-producing command (``SUBSCRIBE``), its errors use
    the Redis-compatible prefixes it shares with its RESP3 port, and its frame
    cap is 512 MiB rather than 64.
    """
    return Config(
        scheme="synap",
        default_port=DEFAULT_RPC_PORT,
        handshake=Handshake.AUTH_COMMAND,
        hello_style=HelloStyle.NOT_USED,
        push=PushPolicy.ENABLED,
        error_codes=ErrorConvention.RESP3_PREFIXES,
        max_frame_bytes=MAX_FRAME_BYTES,
    )


# ── Wire value helpers ─────────────────────────────────────────────────────────


def _to_wire(v: Any) -> Value:  # noqa: ANN401
    """Encode a Python value as a Thunder :class:`Value`.

    ``bool`` is checked before ``int`` deliberately — in Python ``bool`` *is* an
    ``int`` subclass, and encoding ``True`` as ``Int(1)`` would change what the
    server's dispatch tree sees.
    """
    if v is None:
        return Value.null()
    if isinstance(v, bool):
        return Value.bool(v)
    if isinstance(v, int):
        return Value.int(v)
    if isinstance(v, float):
        return Value.float(v)
    if isinstance(v, (bytes, bytearray)):
        return Value.bytes(bytes(v))
    if isinstance(v, list):
        return Value.array([_to_wire(x) for x in v])
    if isinstance(v, dict):
        return Value.map([(_to_wire(str(k)), _to_wire(val)) for k, val in v.items()])
    return Value.str(str(v))


def _from_wire(wire: Any) -> Any:  # noqa: ANN401
    """Decode a Thunder :class:`Value` back to a plain Python value.

    ``Bytes`` decode to ``str`` when they are valid UTF-8 and stay ``bytes``
    otherwise, matching what the SDK's managers have always received. Thunder
    handles both the canonical MessagePack ``bin`` form the server now emits and
    the legacy array-of-integers form, so a pre-1.1 server still interoperates.
    """
    if wire is None:
        return None
    if not isinstance(wire, Value):
        return wire

    if wire.kind == "null":
        return None
    if wire.kind in ("str", "bool", "int", "float"):
        return wire.value
    if wire.kind == "bytes":
        try:
            return wire.value.decode("utf-8")
        except UnicodeDecodeError:
            return wire.value
    if wire.kind == "array":
        return [_from_wire(x) for x in wire.value]
    if wire.kind == "map":
        return {str(_from_wire(k)): _from_wire(v) for k, v in wire.value}
    return wire


def _to_synap_exception(exc: ThunderError) -> SynapException:
    """Map a Thunder error onto the SDK's exception type.

    ``NOAUTH`` / ``WRONGPASS`` / ``NOPERM`` arrive as :class:`AuthError` because
    the config selects the RESP3 prefix convention; everything else keeps the
    server's message verbatim, as before.
    """
    if isinstance(exc, (AuthError, ServerError)):
        return SynapException.server_error(str(exc))
    return SynapException.server_error(f"SynapRPC: {exc}")


# ── Transport ──────────────────────────────────────────────────────────────────


class SynapRpcTransport:
    """Persistent async TCP connection to the SynapRPC listener.

    Concurrent commands multiplex over the one connection, demultiplexed by
    frame id. The connection is opened lazily on the first :meth:`execute`.

    Args:
        host: The SynapRPC server hostname or IP address.
        port: The SynapRPC server TCP port (default: 15501).
        timeout: Per-operation timeout in seconds (default: 30).
        credentials: Optional handshake credentials. Before the Thunder swap
            this transport never authenticated, so it could not reach a
            ``require_auth`` deployment at all.
    """

    def __init__(
        self,
        host: str,
        port: int,
        timeout: float,
        credentials: Credentials | None = None,
    ) -> None:
        self._endpoint = f"synap://{host}:{port}"
        self._client_config = ClientConfig(
            connect_timeout=timeout,
            call_timeout=timeout,
            credentials=credentials,
            client_name="synap-python-sdk",
        )
        self._client: AsyncClient | None = None

    async def _dial(self) -> AsyncClient:
        """Open a fresh Thunder client against the configured endpoint."""
        try:
            return await AsyncClient.connect(
                self._endpoint, synap_config(), self._client_config
            )
        except ThunderError as exc:
            raise _to_synap_exception(exc) from exc

    async def _ensure_connected(self) -> AsyncClient:
        """The shared client, dialed on first use."""
        if self._client is None:
            self._client = await self._dial()
        return self._client

    async def execute(self, cmd: str, args: list[Any]) -> Any:  # noqa: ANN401
        """Send a command and await its response.

        Args:
            cmd: The native wire command name (e.g. ``"SET"``).
            args: The positional arguments for the command.

        Returns:
            The decoded Python value from the ``Ok`` branch of the response.

        Raises:
            SynapException: On server error or network failure.
        """
        client = await self._ensure_connected()
        try:
            result = await client.call(cmd.upper(), [_to_wire(a) for a in args])
        except ThunderError as exc:
            raise _to_synap_exception(exc) from exc
        return _from_wire(result)

    async def subscribe_push(
        self,
        topics: list[str],
        on_message: Any,  # Callable[[dict[str, Any]], None]  # noqa: ANN401
    ) -> tuple[str, Any]:
        """Open a dedicated push connection for pub/sub subscriptions.

        The push hook is registered *before* ``SUBSCRIBE`` is sent, so a message
        published between the server's acknowledgement and the reader starting
        cannot be lost.

        Args:
            topics: List of topic names to subscribe to.
            on_message: Callback invoked for each received push message with a
                ``{topic, payload, id, timestamp}`` dict.

        Returns:
            A ``(subscriber_id, cancel_fn)`` tuple. Call ``cancel_fn()`` to
            close the dedicated connection.
        """
        client = await self._dial()

        def _handle_push(value: Value) -> None:
            frame = _from_wire(value)
            if not isinstance(frame, dict):
                return
            on_message(
                {
                    "topic": str(frame.get("topic", "")),
                    "payload": frame.get("payload"),
                    "id": str(frame.get("id", "")),
                    "timestamp": int(frame.get("timestamp", 0)),
                }
            )

        client.on_push(_handle_push)

        try:
            result = await client.call("SUBSCRIBE", [Value.str(t) for t in topics])
        except ThunderError as exc:
            await client.close()
            raise _to_synap_exception(exc) from exc

        subscriber_id_value = result.map_get("subscriber_id")
        subscriber_id = (
            subscriber_id_value.as_str() if subscriber_id_value is not None else None
        ) or ""

        def cancel() -> None:
            """Close the push connection, ending its reader task."""
            asyncio.ensure_future(client.close())  # noqa: RUF006

        return subscriber_id, cancel

    async def close(self) -> None:
        """Close the connection and fail anything still in flight."""
        client = self._client
        self._client = None
        if client is not None:
            await client.close()


__all__ = [
    "DEFAULT_RPC_PORT",
    "MAX_FRAME_BYTES",
    "SynapRpcTransport",
    "_from_wire",
    "_to_wire",
    "synap_config",
]
