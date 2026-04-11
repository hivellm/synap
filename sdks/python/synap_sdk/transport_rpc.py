"""SynapRPC binary TCP transport.

Wire protocol:
- Every frame is prefixed with a 4-byte little-endian unsigned integer giving
  the length of the MessagePack-encoded body that follows.
- Request body (msgpack array): ``[id: int, command: str, args: [WireValue…]]``
- Response body (msgpack array): ``[id: int, {"Ok": WireValue} | {"Err": str}]``

WireValue uses *serde externally-tagged* encoding (mirrors Rust's ``rmp_serde``):
- ``None``  → bare msgpack string ``"Null"``
- ``str``   → ``{"Str": "…"}``
- ``int``   → ``{"Int": n}``
- ``float`` → ``{"Float": f}``
- ``bool``  → ``{"Bool": b}``
- ``bytes`` → ``{"Bytes": b}``
- ``list``  → ``{"Array": […]}``

The transport is lazy-connected: the first call to :meth:`execute` opens the
TCP socket. On any network error the socket is dropped so the next call
transparently reconnects.
"""

from __future__ import annotations

import asyncio
import struct
from typing import Any

import msgpack

from synap_sdk.exceptions import SynapException


# ── Wire value helpers ─────────────────────────────────────────────────────────


def _to_wire(v: Any) -> Any:  # noqa: ANN401
    """Wrap a Python value in the externally-tagged WireValue envelope."""
    if v is None:
        return "Null"
    if isinstance(v, bool):
        return {"Bool": v}
    if isinstance(v, int):
        return {"Int": v}
    if isinstance(v, float):
        return {"Float": v}
    if isinstance(v, (bytes, bytearray)):
        return {"Bytes": bytes(v)}
    if isinstance(v, list):
        return {"Array": [_to_wire(x) for x in v]}
    return {"Str": str(v)}


def _from_wire(wire: Any) -> Any:  # noqa: ANN401
    """Unwrap a WireValue envelope back to a Python value."""
    if wire == "Null" or wire is None:
        return None
    if isinstance(wire, dict):
        if "Str" in wire:
            return wire["Str"]
        if "Int" in wire:
            return wire["Int"]
        if "Float" in wire:
            return wire["Float"]
        if "Bool" in wire:
            return wire["Bool"]
        if "Bytes" in wire:
            return wire["Bytes"]
        if "Array" in wire:
            return [_from_wire(x) for x in wire["Array"]]
        if "Map" in wire:
            return {str(_from_wire(k)): _from_wire(v) for k, v in wire["Map"]}
    return wire


# ── Transport ──────────────────────────────────────────────────────────────────


class SynapRpcTransport:
    """Persistent async TCP connection to the SynapRPC listener.

    Requests are multiplexed by ID; responses are dispatched to waiting
    coroutines via :class:`asyncio.Future` objects stored in ``_pending``.

    Args:
        host: The SynapRPC server hostname or IP address.
        port: The SynapRPC server TCP port (default: 15501).
        timeout: Per-operation timeout in seconds (default: 30).
    """

    def __init__(self, host: str, port: int, timeout: float) -> None:
        self._host = host
        self._port = port
        self._timeout = timeout
        self._reader: asyncio.StreamReader | None = None
        self._writer: asyncio.StreamWriter | None = None
        self._next_id: int = 1
        self._pending: dict[int, asyncio.Future[Any]] = {}
        self._recv_task: asyncio.Task[None] | None = None
        self._lock: asyncio.Lock = asyncio.Lock()

    async def _connect(self) -> None:
        """Open the TCP connection and start the receive loop."""
        reader, writer = await asyncio.wait_for(
            asyncio.open_connection(self._host, self._port),
            timeout=self._timeout,
        )
        self._reader = reader
        self._writer = writer
        loop = asyncio.get_event_loop()
        self._recv_task = loop.create_task(self._recv_loop())

    async def _recv_loop(self) -> None:
        """Background task: read frames and dispatch to waiting futures."""
        assert self._reader is not None  # noqa: S101
        try:
            while True:
                len_bytes = await self._reader.readexactly(4)
                frame_len = struct.unpack_from("<I", len_bytes)[0]
                body = await self._reader.readexactly(frame_len)
                decoded: list[Any] = msgpack.unpackb(body, raw=False)
                req_id: int = decoded[0]
                result_env: dict[str, Any] = decoded[1]
                fut = self._pending.pop(req_id, None)
                if fut is None:
                    continue
                if "Ok" in result_env:
                    fut.set_result(_from_wire(result_env["Ok"]))
                else:
                    fut.set_exception(
                        SynapException.server_error(str(result_env.get("Err", "unknown error")))
                    )
        except Exception as exc:  # noqa: BLE001
            # Propagate the error to all waiters and reset the connection.
            for fut in self._pending.values():
                if not fut.done():
                    fut.set_exception(exc)
            self._pending.clear()
            self._reader = None
            self._writer = None

    async def _ensure_connected(self) -> None:
        """Connect if not already connected, guarded by a lock."""
        async with self._lock:
            if self._writer is None or self._writer.is_closing():
                await self._connect()

    async def execute(self, cmd: str, args: list[Any]) -> Any:  # noqa: ANN401
        """Send a command and await its response.

        Args:
            cmd: The native wire command name (e.g. ``"SET"``).
            args: The positional arguments for the command.

        Returns:
            The unwrapped Python value from the ``Ok`` branch of the response.

        Raises:
            SynapException: On server error or network failure.
        """
        await self._ensure_connected()
        assert self._writer is not None  # noqa: S101

        loop = asyncio.get_event_loop()
        req_id = self._next_id
        self._next_id += 1

        wire_args = [_to_wire(a) for a in args]
        body = msgpack.packb([req_id, cmd.upper(), wire_args], use_bin_type=True)
        frame = struct.pack("<I", len(body)) + body

        fut: asyncio.Future[Any] = loop.create_future()
        self._pending[req_id] = fut
        self._writer.write(frame)
        await self._writer.drain()

        return await asyncio.wait_for(fut, timeout=self._timeout)

    async def subscribe_push(
        self,
        topics: list[str],
        on_message: Any,  # Callable[[dict[str, Any]], None]
    ) -> tuple[str, Any]:
        """Open a dedicated push connection for pub/sub subscriptions.

        Sends a ``SUBSCRIBE`` command on a fresh TCP socket, reads the initial
        response to extract the ``subscriber_id``, then starts a background
        task that reads incoming push frames (``id == 0xFFFFFFFF``) and calls
        ``on_message`` with ``{topic, payload, id, timestamp}`` dicts.

        Args:
            topics: List of topic names to subscribe to.
            on_message: Callback invoked for each received push message.

        Returns:
            A ``(subscriber_id, cancel_fn)`` tuple. Call ``cancel_fn()`` to
            stop the background task and close the dedicated socket.
        """
        PUSH_ID = 0xFFFF_FFFF

        reader, writer = await asyncio.wait_for(
            asyncio.open_connection(self._host, self._port),
            timeout=self._timeout,
        )

        # Send SUBSCRIBE frame
        wire_args = [_to_wire(t) for t in topics]
        body = msgpack.packb([1, "SUBSCRIBE", wire_args], use_bin_type=True)
        frame = struct.pack("<I", len(body)) + body
        writer.write(frame)
        await writer.drain()

        # Read the initial SUBSCRIBE response to extract subscriber_id
        len_bytes = await asyncio.wait_for(reader.readexactly(4), timeout=self._timeout)
        frame_len = struct.unpack_from("<I", len_bytes)[0]
        init_body = await asyncio.wait_for(reader.readexactly(frame_len), timeout=self._timeout)
        init_decoded: list[Any] = msgpack.unpackb(init_body, raw=False)
        subscriber_id = ""
        if len(init_decoded) >= 2:
            result_env = init_decoded[1]
            if isinstance(result_env, dict) and "Ok" in result_env:
                val = _from_wire(result_env["Ok"])
                if isinstance(val, dict) and "subscriber_id" in val:
                    subscriber_id = str(val["subscriber_id"])

        cancelled = False

        async def _push_loop() -> None:
            try:
                while not cancelled:
                    len_b = await asyncio.wait_for(reader.readexactly(4), timeout=self._timeout)
                    f_len = struct.unpack_from("<I", len_b)[0]
                    f_body = await asyncio.wait_for(
                        reader.readexactly(f_len), timeout=self._timeout
                    )
                    decoded: list[Any] = msgpack.unpackb(f_body, raw=False)
                    if len(decoded) < 2:
                        continue
                    frame_id: int = decoded[0]
                    result_env = decoded[1]
                    if frame_id != PUSH_ID:
                        continue
                    if isinstance(result_env, dict) and "Ok" in result_env:
                        val = _from_wire(result_env["Ok"])
                        if isinstance(val, dict):
                            on_message({
                                "topic": str(val.get("topic", "")),
                                "payload": val.get("payload"),
                                "id": str(val.get("id", "")),
                                "timestamp": int(val.get("timestamp", 0)),
                            })
            except (asyncio.CancelledError, Exception):  # noqa: BLE001
                pass
            finally:
                writer.close()

        loop = asyncio.get_event_loop()
        push_task = loop.create_task(_push_loop())

        def cancel() -> None:
            nonlocal cancelled
            cancelled = True
            push_task.cancel()

        return subscriber_id, cancel

    async def close(self) -> None:
        """Close the TCP connection and cancel the receive loop."""
        if self._recv_task is not None:
            self._recv_task.cancel()
            try:
                await self._recv_task
            except (asyncio.CancelledError, Exception):  # noqa: BLE001
                pass
            self._recv_task = None
        if self._writer is not None:
            self._writer.close()
            try:
                await self._writer.wait_closed()
            except Exception:  # noqa: BLE001
                pass
            self._writer = None
        self._reader = None


__all__ = [
    "SynapRpcTransport",
    "_to_wire",
    "_from_wire",
]
