"""Configuration for Synap client."""

from __future__ import annotations

from synap_sdk.exceptions import SynapException
from synap_sdk.transport import TransportMode


class SynapConfig:
    """Configuration for the Synap client.

    Args:
        base_url: The base URL of the Synap server
        timeout: Request timeout in seconds (default: 30)
        auth_token: Optional API key token (Bearer token)
        username: Optional username for Basic Auth
        password: Optional password for Basic Auth
        max_retries: Maximum number of retries for failed requests (default: 3)
        transport: Transport protocol — ``'synaprpc'`` (default), ``'resp3'``, or ``'http'``
        rpc_host: SynapRPC host (default: ``'127.0.0.1'``)
        rpc_port: SynapRPC port (default: 15501)
        resp3_host: RESP3 host (default: ``'127.0.0.1'``)
        resp3_port: RESP3 port (default: 6379)

    Example:
        >>> # API Key authentication
        >>> config = SynapConfig("http://localhost:15500", auth_token="my-api-key")
        >>> # Basic Auth authentication
        >>> config = SynapConfig("http://localhost:15500", username="user", password="pass")
        >>> # SynapRPC transport (default)
        >>> config = SynapConfig("http://localhost:15500")
        >>> # Plain HTTP transport
        >>> config = SynapConfig("http://localhost:15500", transport="http")
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
        transport: TransportMode = "synaprpc",
        rpc_host: str = "127.0.0.1",
        rpc_port: int = 15501,
        resp3_host: str = "127.0.0.1",
        resp3_port: int = 6379,
    ) -> None:
        """Initialize a new SynapConfig."""
        if not base_url or not base_url.strip():
            raise SynapException("Base URL cannot be empty")

        if auth_token and (username or password):
            raise SynapException("Cannot use both auth_token and Basic Auth (username/password)")

        self._base_url = base_url.rstrip("/")
        self._timeout = timeout
        self._auth_token = auth_token
        self._username = username
        self._password = password
        self._max_retries = max_retries
        self._transport: TransportMode = transport
        self._rpc_host = rpc_host
        self._rpc_port = rpc_port
        self._resp3_host = resp3_host
        self._resp3_port = resp3_port

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
    def create(cls, base_url: str) -> SynapConfig:
        """Create a new configuration with the specified base URL.

        Args:
            base_url: The base URL of the Synap server

        Returns:
            A new SynapConfig instance
        """
        return cls(base_url)

    def _copy(self, **overrides: object) -> SynapConfig:
        """Return a copy of this config with selected fields overridden."""
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
        return SynapConfig(self._base_url, **kwargs)  # type: ignore[arg-type]

    def with_timeout(self, timeout: int) -> SynapConfig:
        """Create a copy with a different timeout."""
        return self._copy(timeout=timeout)

    def with_auth_token(self, token: str) -> SynapConfig:
        """Create a copy with an authentication token (API key)."""
        return self._copy(auth_token=token, username=None, password=None)

    def with_basic_auth(self, username: str, password: str) -> SynapConfig:
        """Create a copy with Basic Auth credentials."""
        return self._copy(auth_token=None, username=username, password=password)

    def with_http_transport(self) -> SynapConfig:
        """Create a copy configured to use HTTP REST transport."""
        return self._copy(transport="http")

    def with_synap_rpc_transport(self) -> SynapConfig:
        """Create a copy configured to use SynapRPC transport (default)."""
        return self._copy(transport="synaprpc")

    def with_resp3_transport(self) -> SynapConfig:
        """Create a copy configured to use RESP3 transport."""
        return self._copy(transport="resp3")

    def with_rpc_addr(self, host: str, port: int) -> SynapConfig:
        """Create a copy with a custom SynapRPC address."""
        return self._copy(rpc_host=host, rpc_port=port)

    def with_resp3_addr(self, host: str, port: int) -> SynapConfig:
        """Create a copy with a custom RESP3 address."""
        return self._copy(resp3_host=host, resp3_port=port)

    def with_max_retries(self, retries: int) -> SynapConfig:
        """Create a copy with a different max retries setting.

        Args:
            retries: The maximum number of retries

        Returns:
            A new SynapConfig instance with the updated max retries
        """
        return self._copy(max_retries=retries)
