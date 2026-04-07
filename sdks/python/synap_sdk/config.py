"""Configuration for Synap client."""

from __future__ import annotations

from synap_sdk.exceptions import SynapException


class SynapConfig:
    """Configuration for the Synap client.

    Args:
        base_url: The base URL of the Synap server
        timeout: Request timeout in seconds (default: 30)
        auth_token: Optional API key token (Bearer token)
        username: Optional username for Basic Auth
        password: Optional password for Basic Auth
        max_retries: Maximum number of retries for failed requests (default: 3)

    Example:
        >>> # API Key authentication
        >>> config = SynapConfig("http://localhost:15500", auth_token="my-api-key")
        >>> # Basic Auth authentication
        >>> config = SynapConfig("http://localhost:15500", username="user", password="pass")
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

    @classmethod
    def create(cls, base_url: str) -> SynapConfig:
        """Create a new configuration with the specified base URL.

        Args:
            base_url: The base URL of the Synap server

        Returns:
            A new SynapConfig instance
        """
        return cls(base_url)

    def with_timeout(self, timeout: int) -> SynapConfig:
        """Create a copy with a different timeout.

        Args:
            timeout: The timeout in seconds

        Returns:
            A new SynapConfig instance with the updated timeout
        """
        return SynapConfig(
            self._base_url,
            timeout=timeout,
            auth_token=self._auth_token,
            username=self._username,
            password=self._password,
            max_retries=self._max_retries,
        )

    def with_auth_token(self, token: str) -> SynapConfig:
        """Create a copy with an authentication token (API key).

        Args:
            token: The authentication token (API key)

        Returns:
            A new SynapConfig instance with the updated token
        """
        return SynapConfig(
            self._base_url,
            timeout=self._timeout,
            auth_token=token,
            username=None,
            password=None,
            max_retries=self._max_retries,
        )

    def with_basic_auth(self, username: str, password: str) -> SynapConfig:
        """Create a copy with Basic Auth credentials.

        Args:
            username: The username for Basic Auth
            password: The password for Basic Auth

        Returns:
            A new SynapConfig instance with Basic Auth credentials
        """
        return SynapConfig(
            self._base_url,
            timeout=self._timeout,
            auth_token=None,
            username=username,
            password=password,
            max_retries=self._max_retries,
        )

    def with_max_retries(self, retries: int) -> SynapConfig:
        """Create a copy with a different max retries setting.

        Args:
            retries: The maximum number of retries

        Returns:
            A new SynapConfig instance with the updated max retries
        """
        return SynapConfig(
            self._base_url,
            timeout=self._timeout,
            auth_token=self._auth_token,
            username=self._username,
            password=self._password,
            max_retries=retries,
        )
