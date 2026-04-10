"""Exceptions for Synap SDK."""


class UnsupportedCommandError(Exception):
    """Raised when a command has no native mapping for the active transport.

    Use an ``http://`` URL if you need HTTP REST for commands that are not yet
    in the native mapper.
    """

    def __init__(self, command: str, transport_mode: str) -> None:
        super().__init__(
            f"command '{command}' is not supported on transport '{transport_mode}'"
        )
        self.command = command
        self.transport_mode = transport_mode


class SynapException(Exception):
    """Base exception for all Synap SDK errors."""

    @classmethod
    def http_error(cls, message: str, status_code: int) -> "SynapException":
        """Create an HTTP error exception.

        Args:
            message: The error message
            status_code: The HTTP status code

        Returns:
            A new SynapException instance
        """
        return cls(f"HTTP Error ({status_code}): {message}")

    @classmethod
    def server_error(cls, message: str) -> "SynapException":
        """Create a server error exception.

        Args:
            message: The error message

        Returns:
            A new SynapException instance
        """
        return cls(f"Server Error: {message}")

    @classmethod
    def network_error(cls, message: str) -> "SynapException":
        """Create a network error exception.

        Args:
            message: The error message

        Returns:
            A new SynapException instance
        """
        return cls(f"Network Error: {message}")

    @classmethod
    def invalid_response(cls, message: str) -> "SynapException":
        """Create an invalid response exception.

        Args:
            message: The error message

        Returns:
            A new SynapException instance
        """
        return cls(f"Invalid Response: {message}")

    @classmethod
    def invalid_config(cls, message: str) -> "SynapException":
        """Create an invalid configuration exception.

        Args:
            message: The error message

        Returns:
            A new SynapException instance
        """
        return cls(f"Invalid Configuration: {message}")
