"""Tests for SynapException."""

from synap_sdk.exceptions import SynapException


def test_constructor_with_message() -> None:
    """Test constructor with message."""
    exception = SynapException("Test error")

    assert str(exception) == "Test error"


def test_http_error_creates_formatted_message() -> None:
    """Test http_error factory method."""
    exception = SynapException.http_error("Request failed", 404)

    assert "HTTP Error (404)" in str(exception)
    assert "Request failed" in str(exception)


def test_server_error_creates_formatted_message() -> None:
    """Test server_error factory method."""
    exception = SynapException.server_error("Internal server error")

    assert "Server Error" in str(exception)
    assert "Internal server error" in str(exception)


def test_network_error_creates_formatted_message() -> None:
    """Test network_error factory method."""
    exception = SynapException.network_error("Connection timeout")

    assert "Network Error" in str(exception)
    assert "Connection timeout" in str(exception)


def test_invalid_response_creates_formatted_message() -> None:
    """Test invalid_response factory method."""
    exception = SynapException.invalid_response("Malformed JSON")

    assert "Invalid Response" in str(exception)
    assert "Malformed JSON" in str(exception)


def test_invalid_config_creates_formatted_message() -> None:
    """Test invalid_config factory method."""
    exception = SynapException.invalid_config("Missing URL")

    assert "Invalid Configuration" in str(exception)
    assert "Missing URL" in str(exception)
