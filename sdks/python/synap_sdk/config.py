"""Configuration for Synap client."""

from __future__ import annotations

import warnings

from synap_sdk.exceptions import SynapException
from synap_sdk.transport import TransportMode


def _parse_host_port(authority: str, default_port: int) -> tuple[str, int]:
    """Parse ``host:port`` from a URL authority string."""
    # Strip any path component
    auth = authority.split("/")[0]
    if ":" in auth:
        host, _, port_str = auth.rpartition(":")
        try:
            return host, int(port_str)
        except ValueError:
            return auth, default_port
    return auth, default_port


class SynapConfig:
    """Configuration for the Synap client.

    **Preferred constructor (v0.11.0+):** pass a URL with the transport scheme::

        SynapConfig("synap://localhost:15501")   # SynapRPC (default)
        SynapConfig("resp3://localhost:6379")    # RESP3
        SynapConfig("http://localhost:15500")    # HTTP

    The ``transport``, ``rpc_host``, ``rpc_port``, ``resp3_host``, and
    ``resp3_port`` keyword arguments are kept for backward compatibility but
    are **deprecated** — encode the address in the URL instead.

    Args:
        base_url: Synap server URL. Accepted schemes: ``synap://`` (SynapRPC),
            ``resp3://`` (RESP3), ``http://`` / ``https://`` (HTTP).
        timeout: Request timeout in seconds (default: 30)
        auth_token: Optional API key token (Bearer token)
        username: Optional username for Basic Auth
        password: Optional password for Basic Auth
        max_retries: Maximum number of retries for failed requests (default: 3)
        transport: **Deprecated.** Use the URL scheme instead.
        rpc_host: **Deprecated.** Encode in the ``synap://host:port`` URL.
        rpc_port: **Deprecated.** Encode in the ``synap://host:port`` URL.
        resp3_host: **Deprecated.** Encode in the ``resp3://host:port`` URL.
        resp3_port: **Deprecated.** Encode in the ``resp3://host:port`` URL.

    Example:
        >>> # API Key authentication
        >>> config = SynapConfig("http://localhost:15500", auth_token="my-api-key")
        >>> # SynapRPC transport (new style)
        >>> config = SynapConfig("synap://localhost:15501")
        >>> # RESP3 transport
        >>> config = SynapConfig("resp3://localhost:6379")
    """

    def __init__(
        self,
        base_url: str,
        *,
        timeout: int = 30,
        auth_token: str | None = None,
        username: str | None = None,
        password: str | None = None,
        max_retries: int = 3,
        transport: TransportMode | None = None,
        rpc_host: str | None = None,
        rpc_port: int | None = None,
        resp3_host: str | None = None,
        resp3_port: int | None = None,
    ) -> None:
        """Initialize a new SynapConfig."""
        if not base_url or not base_url.strip():
            raise SynapException("Base URL cannot be empty")

        if auth_token and (username or password):
            raise SynapException("Cannot use both auth_token and Basic Auth (username/password)")

        # ── Deprecation warnings ─────────────────────────────────────────────
        _deprecated = {"transport", "rpc_host", "rpc_port", "resp3_host", "resp3_port"}
        _provided = {
            k for k, v in {
                "transport": transport,
                "rpc_host": rpc_host,
                "rpc_port": rpc_port,
                "resp3_host": resp3_host,
                "resp3_port": resp3_port,
            }.items() if v is not None
        }
        if _provided:
            warnings.warn(
                f"SynapConfig parameters {sorted(_provided)} are deprecated since v0.11.0; "
                "use synap:// or resp3:// URL schemes instead.",
                DeprecationWarning,
                stacklevel=2,
            )

        # ── URL-scheme-based transport inference ─────────────────────────────
        if base_url.startswith("synap://"):
            host, port = _parse_host_port(base_url[len("synap://"):], 15_501)
            self._base_url = f"http://{host}:15500"
            self._transport: TransportMode = "synaprpc"
            self._rpc_host = host
            self._rpc_port = port
            self._resp3_host = "127.0.0.1"
            self._resp3_port = 6_379
        elif base_url.startswith("resp3://"):
            host, port = _parse_host_port(base_url[len("resp3://"):], 6_379)
            self._base_url = f"http://{host}:15500"
            self._transport = "resp3"
            self._rpc_host = "127.0.0.1"
            self._rpc_port = 15_501
            self._resp3_host = host
            self._resp3_port = port
        else:
            # http:// / https:// or legacy builder style
            self._base_url = base_url.rstrip("/")
            self._transport = transport if transport is not None else "http"
            self._rpc_host = rpc_host if rpc_host is not None else "127.0.0.1"
            self._rpc_port = rpc_port if rpc_port is not None else 15_501
            self._resp3_host = resp3_host if resp3_host is not None else "127.0.0.1"
            self._resp3_port = resp3_port if resp3_port is not None else 6_379

        self._timeout = timeout
        self._auth_token = auth_token
        self._username = username
        self._password = password
        self._max_retries = max_retries

    @property
    def base_url(self) -> str:
        """Get the base URL."""
        return self._base_url

    @property
    def timeout(self) -> int:
        """Get the timeout in seconds."""
        return self._timeout

    @property
    def auth_token(self) -> str | None:
        """Get the authentication token (API key)."""
        return self._auth_token

    @property
    def username(self) -> str | None:
        """Get the username for Basic Auth."""
        return self._username

    @property
    def password(self) -> str | None:
        """Get the password for Basic Auth."""
        return self._password

    @property
    def max_retries(self) -> int:
        """Get the maximum number of retries."""
        return self._max_retries

    @property
    def transport(self) -> TransportMode:
        """Get the transport protocol."""
        return self._transport

    @property
    def rpc_host(self) -> str:
        """Get the SynapRPC host."""
        return self._rpc_host

    @property
    def rpc_port(self) -> int:
        """Get the SynapRPC port."""
        return self._rpc_port

    @property
    def resp3_host(self) -> str:
        """Get the RESP3 host."""
        return self._resp3_host

    @property
    def resp3_port(self) -> int:
        """Get the RESP3 port."""
        return self._resp3_port

    @classmethod
    def create(cls, base_url: str) -> "SynapConfig":
        """Create a new configuration with the specified base URL.

        Args:
            base_url: The Synap server URL (supports ``synap://``, ``resp3://``, ``http://``)

        Returns:
            A new SynapConfig instance
        """
        return cls(base_url)

    def _copy(self, **overrides: object) -> "SynapConfig":
        """Return a copy of this config with selected fields overridden."""
        # Build a fresh config from the stored (already-inferred) fields.
        # We pass a plain http:// base_url and set transport/rpc/resp3 explicitly
        # so the copy preserves the resolved state rather than re-parsing a URL.
        kwargs: dict[str, object] = {
            "timeout": self._timeout,
            "auth_token": self._auth_token,
            "username": self._username,
            "password": self._password,
            "max_retries": self._max_retries,
            "transport": self._transport,
            "rpc_host": self._rpc_host,
            "rpc_port": self._rpc_port,
            "resp3_host": self._resp3_host,
            "resp3_port": self._resp3_port,
        }
        kwargs.update(overrides)
        # Suppress deprecation warnings for internal copies
        with warnings.catch_warnings():
            warnings.simplefilter("ignore", DeprecationWarning)
            return SynapConfig(self._base_url, **kwargs)  # type: ignore[arg-type]

    def with_timeout(self, timeout: int) -> "SynapConfig":
        """Create a copy with a different timeout."""
        return self._copy(timeout=timeout)

    def with_auth_token(self, token: str) -> "SynapConfig":
        """Create a copy with an authentication token (API key)."""
        return self._copy(auth_token=token, username=None, password=None)

    def with_basic_auth(self, username: str, password: str) -> "SynapConfig":
        """Create a copy with Basic Auth credentials."""
        return self._copy(auth_token=None, username=username, password=password)

    def with_http_transport(self) -> "SynapConfig":
        """Create a copy configured to use HTTP REST transport."""
        return self._copy(transport="http")

    def with_synap_rpc_transport(self) -> "SynapConfig":
        """Create a copy configured to use SynapRPC transport."""
        return self._copy(transport="synaprpc")

    def with_resp3_transport(self) -> "SynapConfig":
        """Create a copy configured to use RESP3 transport."""
        return self._copy(transport="resp3")

    def with_rpc_addr(self, host: str, port: int) -> "SynapConfig":
        """Create a copy with a custom SynapRPC address."""
        return self._copy(rpc_host=host, rpc_port=port)

    def with_resp3_addr(self, host: str, port: int) -> "SynapConfig":
        """Create a copy with a custom RESP3 address."""
        return self._copy(resp3_host=host, resp3_port=port)

    def with_max_retries(self, retries: int) -> "SynapConfig":
        """Create a copy with a different max retries setting."""
        return self._copy(max_retries=retries)
