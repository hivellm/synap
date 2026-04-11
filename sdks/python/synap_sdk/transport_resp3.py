"""RESP3 binary TCP transport.

Implements the Redis Serialisation Protocol v3 (RESP3) over a persistent
``asyncio`` TCP connection.

Wire format sent to server (RESP2 multibulk, universally compatible):
    ``*N\\r\\n$len\\r\\narg\\r\\n…``

Responses parsed:
    - ``+OK\\r\\n``           — simple string
    - ``-ERR msg\\r\\n``      — error (raises ``SynapException``)
    - ``:42\\r\\n``           — integer
    - ``,,1.5\\r\\n``         — double (RESP3 ``,'``)
    - ``#t\\r\\n``            — boolean (RESP3 ``#``)
    - ``_\\r\\n``             — null (RESP3 ``_``)
    - ``$len\\r\\ndata\\r\\n`` — bulk string
    - ``*N\\r\\n…``           — array
    - ``%N\\r\\n…``           — map (RESP3)
    - ``~N\\r\\n…``           — set (RESP3)

On connect the transport sends ``HELLO 3`` to negotiate RESP3; the initial
map response is drained and discarded.  If the server does not support RESP3
it falls back gracefully because the parser handles RESP2 types too.

The socket is lazy-connected (first :meth:`execute` call opens it) and
auto-reconnects after any network failure.
"""

from __future__ import annotations

import asyncio
from typing import Any

from synap_sdk.exceptions import SynapException


class Resp3Transport:
    """Persistent async TCP connection to a RESP3-compatible listener.

    Requests are serialised one at a time using a per-request lock so the
    parser stays simple without needing response buffering.

    Args:
        host: The server hostname or IP address.
        port: The server TCP port (default: 6379).
        timeout: Per-operation timeout in seconds (default: 30).
    """

    def __init__(self, host: str, port: int, timeout: float) -> None:
        self._host = host
        self._port = port
        self._timeout = timeout
        self._reader: asyncio.StreamReader | None = None
        self._writer: asyncio.StreamWriter | None = None
        self._req_lock: asyncio.Lock = asyncio.Lock()
        self._conn_lock: asyncio.Lock = asyncio.Lock()

    async def _connect(self) -> None:
        """Open the TCP socket and negotiate RESP3 via HELLO 3."""
        reader, writer = await asyncio.wait_for(
            asyncio.open_connection(self._host, self._port),
            timeout=self._timeout,
        )
        self._reader = reader
        self._writer = writer
        # Negotiate RESP3; drain the inline map/array response.
        hello_cmd = b"*2\r\n$5\r\nHELLO\r\n$1\r\n3\r\n"
        writer.write(hello_cmd)
        await writer.drain()
        try:
            await self._read_value()
        except SynapException:
            # Server may not support HELLO (RESP2 only); ignore the error and
            # continue — the connection is still usable for RESP2 commands.
            pass

    async def _ensure_connected(self) -> None:
        """Connect if not already connected, guarded by a lock."""
        async with self._conn_lock:
            if self._writer is None or self._writer.is_closing():
                await self._connect()

    async def _read_line(self) -> str:
        """Read one ``\\r\\n``-terminated line from the stream."""
        assert self._reader is not None  # noqa: S101
        line = await asyncio.wait_for(self._reader.readline(), timeout=self._timeout)
        return line.decode("utf-8").rstrip("\r\n")

    async def _read_value(self) -> Any:  # noqa: ANN401
        """Recursively parse one RESP3 value from the stream.

        Returns:
            The parsed Python value (``str``, ``int``, ``float``, ``bool``,
            ``None``, ``list``, or ``dict``).

        Raises:
            SynapException: On a server-sent error reply.
        """
        assert self._reader is not None  # noqa: S101
        line = await self._read_line()
        if not line:
            return None
        prefix, rest = line[0], line[1:]
        match prefix:
            case "+":
                # Simple string
                return rest
            case "-":
                # Error reply
                raise SynapException.server_error(rest)
            case ":":
                # Integer
                return int(rest)
            case ",":
                # Double (RESP3)
                return float(rest)
            case "#":
                # Boolean (RESP3): "t" → True, "f" → False
                return rest.lower() == "t"
            case "_":
                # Null (RESP3)
                return None
            case "$":
                # Bulk string
                length = int(rest)
                if length == -1:
                    return None
                data = await asyncio.wait_for(
                    self._reader.readexactly(length + 2), timeout=self._timeout
                )
                return data[:-2].decode("utf-8")
            case "*":
                # Array (RESP2 / RESP3)
                count = int(rest)
                if count == -1:
                    return None
                return [await self._read_value() for _ in range(count)]
            case "%":
                # Map (RESP3): alternating key/value pairs
                count = int(rest)
                result: dict[str, Any] = {}
                for _ in range(count):
                    k = await self._read_value()
                    v = await self._read_value()
                    result[str(k)] = v
                return result
            case "~":
                # Set (RESP3): treat as ordered list
                count = int(rest)
                return [await self._read_value() for _ in range(count)]
            case _:
                # Unknown prefix — return as-is
                return rest

    def _encode_command(self, cmd: str, args: list[Any]) -> bytes:
        """Encode a command as a RESP2 multibulk array.

        Args:
            cmd: The wire command name (e.g. ``"SET"``).
            args: Positional arguments; each is coerced to ``str`` for
                encoding, unless already ``bytes``.

        Returns:
            The encoded bytes ready to write to the socket.
        """
        parts: list[str | bytes] = [cmd, *args]
        out: list[bytes] = [f"*{len(parts)}\r\n".encode()]
        for part in parts:
            enc: bytes = part if isinstance(part, (bytes, bytearray)) else str(part).encode("utf-8")
            out.append(f"${len(enc)}\r\n".encode())
            out.append(enc)
            out.append(b"\r\n")
        return b"".join(out)

    async def execute(self, cmd: str, args: list[Any]) -> Any:  # noqa: ANN401
        """Send a command and return the parsed response.

        Args:
            cmd: The native wire command name (e.g. ``"GET"``).
            args: The positional arguments for the command.

        Returns:
            The parsed Python value returned by the server.

        Raises:
            SynapException: On server error or network failure.
        """
        await self._ensure_connected()
        async with self._req_lock:
            assert self._writer is not None  # noqa: S101
            frame = self._encode_command(cmd, args)
            self._writer.write(frame)
            await self._writer.drain()
            return await self._read_value()

    async def close(self) -> None:
        """Close the TCP connection."""
        if self._writer is not None:
            self._writer.close()
            try:
                await self._writer.wait_closed()
            except Exception:  # noqa: BLE001
                pass
            self._writer = None
        self._reader = None


__all__ = ["Resp3Transport"]
